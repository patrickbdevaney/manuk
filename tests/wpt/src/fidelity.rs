//! G1 — **real-site visual fidelity vs Chromium** (ADR-010, amended).
//!
//! The box-probe parity gate compares `getBoundingClientRect` on 30 *synthetic* pages. That is a
//! rigorous signal but it is **not the user's experience**: a page can pass box tolerance and still
//! look wrong — missing backgrounds, dropped shadows, wrong fonts, an unpainted element. And real
//! modern sites aren't in that corpus at all.
//!
//! So this gate does what a person would do: **render the real page, screenshot Chromium rendering
//! the same page, and compare the pixels.** Both are full renders through the real pipeline
//! (external CSS + images + JS), not a side channel.
//!
//! **Comparison method.** A raw pixel diff is useless here — font hinting and antialiasing differ
//! between any two engines and would swamp the signal. Instead both images are reduced to a coarse
//! **block grid** (mean RGB per cell). That is deliberately blind to glyph-level AA but *very*
//! sensitive to what actually matters: layout displacement, a missing background, an unpainted box,
//! a wrong colour. The score is the fraction of blocks whose mean colour agrees within tolerance.

use std::path::Path;

use anyhow::{Context, Result};

/// Per-page fidelity result — **two** numbers on purpose.
///
/// This session proved repeatedly that a pixel score alone is a poor proxy for correctness: an
/// entirely absent sidebar moved Wikipedia's visual score by <1 point. A missing element is a
/// missing **box**, so the structural half compares Chrome's `getBoundingClientRect` for every
/// `[id]` element against Manuk's, and reports what is MISSING and what is MISPLACED. That number
/// cannot be fooled by white matching white.
#[derive(Clone)]
pub struct Fidelity {
    pub name: String,
    /// Visual: fraction of grid blocks agreeing with Chromium, 0.0–1.0.
    pub score: f64,
    pub differing: usize,
    pub total: usize,
    /// **Structural COVERAGE**: of the elements Chrome renders, what fraction does Manuk render at
    /// all? This is the honest number — a missing region cannot hide in it. `None` if unprobed.
    pub structure: Option<f64>,
    /// **Layer-1 SHAPE** (parent-relative placement, `shape_stats`): of the elements BOTH engines
    /// render, what fraction sits in the right place *relative to its nearest shared ancestor*. This
    /// is the redesign's primary placement number — it cancels a constant page offset that the old
    /// absolute `placement_stats` charged N times. `None` if unprobed. (tick 532)
    pub shape: Option<f64>,
    /// Elements Chrome renders that Manuk does **not** produce a box for at all.
    pub missing: usize,
    /// Elements both render, but Manuk places/sizes wrongly (beyond tolerance).
    pub misplaced: usize,
    pub probed: usize,
    /// **The four JARRING invariants** (FIDELITY-SCORING-REDESIGN.md §2), as counts per site:
    /// horizontal overflow · sibling overlap · reading-order inversion · collapsed interactive
    /// target. They were computed and *printed* per site since brick 4b and then thrown away, so the
    /// certificate — whose bar is *"≥95% of sites CLEAN on each invariant"* — could not be computed
    /// from a sweep at all. A number printed and discarded is not a measurement, it is a log line.
    pub jarring: [usize; 4],
}

/// The four jarring invariants, in the order they sit in [`Fidelity::jarring`]. Named so a report
/// cannot silently reorder them and relabel three columns at once.
pub const JARRING_NAMES: [&str; 4] = ["h-overflow", "overlap", "reading-order", "dead-target"];

/// The Phase-0 exit certificate, evaluated over a sweep's rows (FIDELITY-SCORING-REDESIGN.md §3).
///
/// This exists because the certificate was written in prose and the instrument printed per-site
/// lines: turning one into the other was a human reading 265 stanzas of stderr, which is exactly the
/// kind of step that gets skipped and then estimated. The bar is **mechanical** — *shape ≥ 0.75 on
/// ≥95% of sites, and ≥95% of sites clean on each jarring invariant* — so it is computed here, once,
/// by the thing that measured it.
#[derive(Debug, Default, PartialEq)]
pub struct Cert {
    /// Sites with a SHAPE score at all (an unprobeable page is not a passing page — it is excluded
    /// from the numerator AND named, never averaged in).
    pub scored: usize,
    /// Sites in the sweep, including the ones that could not be scored.
    pub sites: usize,
    /// Sites at or above the shape floor.
    pub shape_ok: usize,
    /// Sites with ZERO divergences on each invariant, in [`JARRING_NAMES`] order.
    pub clean: [usize; 4],
}

/// The certificate's shape floor and its site-fraction bar — the two numbers the exit rule is
/// written in. Constants, not parameters, because *"widen the bar to pass"* is the one move this
/// project refuses; a floor that a caller can pass in is a floor that will eventually be passed in.
pub const CERT_SHAPE_FLOOR: f64 = 0.75;
pub const CERT_SITE_BAR: f64 = 0.95;

/// Evaluate the certificate over a sweep's rows.
///
/// **Unscored sites count against the site bar, not out of it.** A page Chrome could not be probed on
/// (or one we failed to render) is a page we cannot claim; dividing by `scored` instead of `sites`
/// would let the bar be met by failing to measure, which is the same defect
/// `fidelity::report`'s NaN check was added for.
pub fn certificate(rows: &[Fidelity]) -> Cert {
    let mut c = Cert {
        sites: rows.len(),
        ..Default::default()
    };
    for r in rows {
        if let Some(s) = r.shape {
            if !s.is_nan() {
                c.scored += 1;
                if s >= CERT_SHAPE_FLOOR {
                    c.shape_ok += 1;
                }
            }
        }
        for i in 0..4 {
            if r.jarring[i] == 0 {
                c.clean[i] += 1;
            }
        }
    }
    c
}

impl Cert {
    fn frac(n: usize, d: usize) -> f64 {
        if d == 0 {
            0.0
        } else {
            n as f64 / d as f64
        }
    }
    pub fn shape_frac(&self) -> f64 {
        Self::frac(self.shape_ok, self.sites)
    }
    pub fn clean_frac(&self, i: usize) -> f64 {
        Self::frac(self.clean[i], self.sites)
    }
    /// Does the certificate HOLD? Every term at or above the bar — one failing term fails it, which
    /// is the point of a certificate rather than an average.
    pub fn holds(&self) -> bool {
        self.sites > 0
            && self.scored == self.sites
            && self.shape_frac() >= CERT_SITE_BAR
            && (0..4).all(|i| self.clean_frac(i) >= CERT_SITE_BAR)
    }
    /// The terms that are BELOW the bar, named. An unmet certificate must say which term missed, or
    /// the next tick is chosen by guesswork.
    pub fn shortfalls(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.scored != self.sites {
            out.push(format!(
                "{} of {} sites UNSCORED (cannot be claimed, counted against the bar)",
                self.sites - self.scored,
                self.sites
            ));
        }
        if self.shape_frac() < CERT_SITE_BAR {
            out.push(format!(
                "shape ≥{:.2} on {:.1}% of sites (bar {:.0}%)",
                CERT_SHAPE_FLOOR,
                self.shape_frac() * 100.0,
                CERT_SITE_BAR * 100.0
            ));
        }
        for i in 0..4 {
            if self.clean_frac(i) < CERT_SITE_BAR {
                out.push(format!(
                    "{} clean on {:.1}% of sites (bar {:.0}%)",
                    JARRING_NAMES[i],
                    self.clean_frac(i) * 100.0,
                    CERT_SITE_BAR * 100.0
                ));
            }
        }
        out
    }
}

/// Print the certificate block — the one place a sweep's headline is allowed to come from.
pub fn certificate_report(rows: &[Fidelity]) {
    let c = certificate(rows);
    println!("\n=== PHASE-0 EXIT CERTIFICATE (FIDELITY-SCORING-REDESIGN §3) ===\n");
    println!(
        "  sites {} · scored {} · shape ≥{:.2} on {} ({:.1}%)",
        c.sites,
        c.scored,
        CERT_SHAPE_FLOOR,
        c.shape_ok,
        c.shape_frac() * 100.0
    );
    for i in 0..4 {
        println!(
            "  {:<14} clean on {:>4} sites ({:.1}%)",
            JARRING_NAMES[i],
            c.clean[i],
            c.clean_frac(i) * 100.0
        );
    }
    if c.holds() {
        println!(
            "\n  CERTIFICATE HOLDS on this sweep. (Bar 0 and interactivity are scored elsewhere.)"
        );
    } else {
        println!("\n  CERTIFICATE NOT MET — shortfalls, in the order to work them:");
        for s in c.shortfalls() {
            println!("      · {s}");
        }
    }
}

/// Grid resolution — coarse enough to ignore glyph AA, fine enough to catch a missing element.
const GRID: u32 = 40;
/// Per-channel mean tolerance for a block to count as "agreeing".
const TOL: f64 = 26.0;

/// Mean RGB of each grid cell of an RGBA8 image.
fn block_means(rgba: &[u8], w: u32, h: u32) -> Vec<[f64; 3]> {
    let mut out = Vec::with_capacity((GRID * GRID) as usize);
    for gy in 0..GRID {
        for gx in 0..GRID {
            let (x0, x1) = (gx * w / GRID, ((gx + 1) * w / GRID).min(w));
            let (y0, y1) = (gy * h / GRID, ((gy + 1) * h / GRID).min(h));
            let (mut r, mut g, mut b, mut n) = (0f64, 0f64, 0f64, 0f64);
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = ((y * w + x) * 4) as usize;
                    if i + 2 < rgba.len() {
                        r += rgba[i] as f64;
                        g += rgba[i + 1] as f64;
                        b += rgba[i + 2] as f64;
                        n += 1.0;
                    }
                }
            }
            let n = n.max(1.0);
            out.push([r / n, g / n, b / n]);
        }
    }
    out
}

fn load_rgba(path: &Path) -> Result<(Vec<u8>, u32, u32)> {
    let img = image::open(path).with_context(|| format!("opening {}", path.display()))?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Ok((rgba.into_raw(), w, h))
}

/// Compare two rendered PNGs; returns the fraction of grid blocks that agree.
pub fn compare(manuk: &Path, chrome: &Path, name: &str) -> Result<Fidelity> {
    let (a, aw, ah) = load_rgba(manuk)?;
    let (b, bw, bh) = load_rgba(chrome)?;
    let ma = block_means(&a, aw, ah);
    let mb = block_means(&b, bw, bh);
    let total = ma.len().min(mb.len());
    let mut differing = 0usize;
    for i in 0..total {
        let d = (0..3)
            .map(|c| (ma[i][c] - mb[i][c]).abs())
            .fold(0.0f64, f64::max);
        if d > TOL {
            differing += 1;
        }
    }
    let score = if total == 0 {
        0.0
    } else {
        1.0 - (differing as f64 / total as f64)
    };
    Ok(Fidelity {
        name: name.to_string(),
        score,
        differing,
        total,
        structure: None,
        shape: None,
        missing: 0,
        misplaced: 0,
        probed: 0,
        jarring: [0; 4],
    })
}

/// Write a **side-by-side** composite (Manuk left, Chromium right, a divider between) so the pair
/// can be inspected as ONE image — the eyeball check the numeric score cannot replace.
pub fn write_side_by_side(manuk: &Path, chrome: &Path, dest: &Path) -> Result<()> {
    let (a, aw, ah) = load_rgba(manuk)?;
    let (b, bw, bh) = load_rgba(chrome)?;
    let h = ah.max(bh);
    let gap = 8u32;
    let w = aw + gap + bw;
    let mut out = vec![255u8; (w * h * 4) as usize];
    let mut blit = |src: &[u8], sw: u32, sh: u32, ox: u32| {
        for y in 0..sh {
            for x in 0..sw {
                let si = ((y * sw + x) * 4) as usize;
                let di = ((y * w + x + ox) * 4) as usize;
                if si + 3 < src.len() && di + 3 < out.len() {
                    out[di..di + 4].copy_from_slice(&src[si..si + 4]);
                }
            }
        }
    };
    blit(&a, aw, ah, 0);
    blit(&b, bw, bh, aw + gap);
    // Divider.
    for y in 0..h {
        for x in aw..(aw + gap) {
            let di = ((y * w + x) * 4) as usize;
            if di + 3 < out.len() {
                out[di..di + 4].copy_from_slice(&[255, 0, 0, 255]);
            }
        }
    }
    let img = image::RgbaImage::from_raw(w, h, out).context("composite")?;
    img.save(dest)
        .with_context(|| format!("writing {}", dest.display()))?;
    Ok(())
}

/// Structural comparison: how many of Chrome's rendered `[id]` boxes does Manuk reproduce?
/// Returns `(score, missing, misplaced, probed)`.
pub fn compare_structure(
    chrome: &std::collections::HashMap<String, [i64; 4]>,
    manuk: &std::collections::HashMap<String, [i64; 4]>,
    tol: i64,
) -> (f64, usize, usize, usize) {
    let (c, m, mi, p, _) = compare_structure_detail(chrome, manuk, tol);
    (c, m, mi, p)
}

/// Same, but also returns the **ids Manuk failed to render at all** — the diagnostic that turns a
/// coverage number into actionable work. 1,402 missing elements are almost never 1,402 bugs; they
/// are a handful of CLASS bugs with huge blast radius, and the ids tell you which.
pub fn compare_structure_detail(
    chrome: &std::collections::HashMap<String, [i64; 4]>,
    manuk: &std::collections::HashMap<String, [i64; 4]>,
    tol: i64,
) -> (f64, usize, usize, usize, Vec<String>) {
    let probed = chrome.len();
    let (mut missing, mut misplaced) = (0usize, 0usize);
    let mut missing_ids: Vec<String> = Vec::new();
    for (id, c) in chrome {
        match manuk.get(id) {
            None => {
                missing += 1;
                missing_ids.push(id.clone());
            }
            Some(m) => {
                let off = (0..4).map(|i| (c[i] - m[i]).abs()).fold(0, i64::max);
                if off > tol {
                    misplaced += 1;
                }
            }
        }
    }
    // **COVERAGE** is the honest, unambiguous signal: of the elements Chrome actually renders, what
    // fraction does Manuk render *at all*? A missing sidebar, an unpainted infobox, a dropped
    // section — all show up here and cannot be averaged away by white-matching-white. Placement
    // drift (`misplaced`) is reported separately because on real pages it is dominated by font-
    // metric differences, which are a *fidelity* concern, not a *correctness* one.
    let rendered = probed.saturating_sub(missing);
    // **A page we cannot PROBE must not score 100%.**
    //
    // `probed` counts the `[id]` elements Chrome rendered. `example.com` — which was in this gate's
    // DEFAULT url list — has **no `id` attributes at all**, so it probed nothing, returned a perfect
    // 1.0, and inflated the mean of a gate whose whole job is to catch missing content.
    //
    // Found by mutation-testing: emptying `node_rects()` entirely — so the browser renders NOTHING —
    // still scored 100% coverage on that URL. A gate that cannot fail on a blank render is not a gate.
    //
    // `f64::NAN` is the honest answer to "what fraction did we render, of nothing?", and `report`
    // excludes it from the mean rather than counting it as success.
    let coverage = if probed == 0 {
        f64::NAN
    } else {
        rendered as f64 / probed as f64
    };
    missing_ids.sort();
    (coverage, missing, misplaced, probed, missing_ids)
}

/// The **placement** half of the honest number, now that COVERAGE is near-saturated: for every
/// element BOTH engines render, how far off is Manuk? Returns `(median_dx, median_dy, median_dw,
/// median_dh, within_tol_fraction)`.
///
/// A count of "misplaced" says nothing about *why*: 6,000 elements each off by 4px is a font-metric
/// difference, while 6,000 elements each off by 200px is one displaced container dragging its whole
/// subtree. The medians separate those two worlds, which is the whole point of measuring.
pub fn placement_stats(
    chrome: &std::collections::HashMap<String, [i64; 4]>,
    manuk: &std::collections::HashMap<String, [i64; 4]>,
    tol: i64,
) -> (i64, i64, i64, i64, f64) {
    let mut d: [Vec<i64>; 4] = Default::default();
    let (mut within, mut n) = (0usize, 0usize);
    for (id, c) in chrome {
        let Some(m) = manuk.get(id) else { continue };
        n += 1;
        let mut worst = 0i64;
        for i in 0..4 {
            let off = (c[i] - m[i]).abs();
            d[i].push(off);
            worst = worst.max(off);
        }
        if worst <= tol {
            within += 1;
        }
    }
    let med = |v: &mut Vec<i64>| -> i64 {
        if v.is_empty() {
            return 0;
        }
        v.sort_unstable();
        v[v.len() / 2]
    };
    let frac = if n == 0 {
        1.0
    } else {
        within as f64 / n as f64
    };
    (
        med(&mut d[0]),
        med(&mut d[1]),
        med(&mut d[2]),
        med(&mut d[3]),
        frac,
    )
}

/// **Layer 1 — SHAPE (parent-relative), the redesign's new primary gate**
/// (`docs/loop/FIDELITY-SCORING-REDESIGN.md` §2). Score every element against **its parent's box**,
/// not the document origin: `rel = (x - px, y - py, w, h)`. This is the metric that separates a
/// genuinely-wrong box from a whole page shifted by a constant.
///
/// `placement_stats` above charges one root cause N times — a page shifted 23px at its header scores
/// `PLACE(ok) 0%` because every downstream element inherits the same 23px, though the layout is
/// otherwise correct and a user notices nothing. Under SHAPE that constant offset **cancels**: only
/// the one element where the offset *originates* fails, so one root cause counts once.
///
/// **Keys are selector-paths** (`tag.SIG:nth-child(n)/…` from the root, the SAME convention the
/// differential oracle uses in `oracle::diff_page`), so an ancestor's key is a prefix of its
/// descendants'. Each element is scored against the **nearest ancestor present in BOTH maps** — the
/// shared reference frame (`oracle::common_frame`): both engines measure the child against the *same*
/// ancestor, so a constant offset in that ancestor drops out of the difference. Width/height are
/// translation-invariant and stay absolute. A root-level element (no `/`, or no shared ancestor) has
/// nothing to subtract, so its absolute box IS its shape — the offset is charged there, exactly once.
///
/// This is the fidelity-probe half of the SHAPE metric the oracle proved at tick 335; the redesign
/// names this probe (the agent-editable `manuk-wpt` fidelity code) as the Phase-0 EXIT instrument,
/// and SHAPE replacing `placement_stats` as its Layer-1 gate.
///
/// Returns `(within_tol_fraction, scored_count)`. Only elements BOTH engines rendered are scored.
pub fn shape_stats(
    chrome: &std::collections::HashMap<String, [i64; 4]>,
    manuk: &std::collections::HashMap<String, [i64; 4]>,
    tol: i64,
) -> (f64, usize) {
    // The nearest ancestor of `path` present in BOTH maps — the shared reference frame. Walks up by
    // dropping the last `/component`; `None` at the root (no `/`) or when no ancestor is shared.
    // Mirrors `oracle::common_frame` exactly so the instrument has ONE definition of SHAPE.
    fn common_frame<'a>(
        path: &str,
        chrome: &'a std::collections::HashMap<String, [i64; 4]>,
        manuk: &'a std::collections::HashMap<String, [i64; 4]>,
    ) -> Option<([i64; 4], [i64; 4])> {
        let mut p = path;
        while let Some(cut) = p.rfind('/') {
            p = &p[..cut];
            if let (Some(c), Some(m)) = (chrome.get(p), manuk.get(p)) {
                return Some((*c, *m));
            }
        }
        None
    }
    let (mut within, mut n) = (0usize, 0usize);
    for (path, c) in chrome {
        let Some(m) = manuk.get(path) else { continue };
        n += 1;
        // Subtract each element's box from its shared frame's box (x,y only — w,h are invariant).
        let (cr, mr) = match common_frame(path, chrome, manuk) {
            Some((cf, mf)) => (
                [c[0] - cf[0], c[1] - cf[1], c[2], c[3]],
                [m[0] - mf[0], m[1] - mf[1], m[2], m[3]],
            ),
            None => (*c, *m),
        };
        let worst = (0..4).map(|i| (cr[i] - mr[i]).abs()).max().unwrap_or(0);
        if worst <= tol {
            within += 1;
        }
    }
    let frac = if n == 0 {
        1.0
    } else {
        within as f64 / n as f64
    };
    (frac, n)
}

/// **Where does the layout first diverge?** Sort every element both engines render by Chrome's `y`
/// and walk down the page; report the first id whose vertical offset exceeds `jump`, plus the last
/// id that was still in agreement. Downstream drift is almost always ONE upstream box with the
/// wrong height — a median tells you drift exists, this tells you where it started.
pub fn first_divergence(
    chrome: &std::collections::HashMap<String, [i64; 4]>,
    manuk: &std::collections::HashMap<String, [i64; 4]>,
    jump: i64,
) -> Option<(String, i64, String, i64)> {
    let mut pairs: Vec<(&String, &[i64; 4], &[i64; 4])> = chrome
        .iter()
        .filter_map(|(id, c)| manuk.get(id).map(|m| (id, c, m)))
        .collect();
    pairs.sort_by_key(|(_, c, _)| c[1]);
    let mut last_ok = String::from("(document start)");
    for (id, c, m) in pairs {
        let dy = (c[1] - m[1]).abs();
        if dy > jump {
            return Some((last_ok, 0, id.clone(), c[1] - m[1]));
        }
        last_ok = id.clone();
    }
    None
}

/// Print the report + the gate verdict against `floor` (applied to the STRUCTURAL score when it is
/// available — it is the honest one).
pub fn report(rows: &[Fidelity], floor: f64) -> bool {
    println!("\n=== G1 · REAL-SITE PARITY vs Chromium ===\n");
    println!(
        "{:<24} {:>8} {:>10} {:>8} {:>9} {:>7}",
        "page", "visual", "COVERAGE", "missing", "misplaced", "verdict"
    );
    let mut all_ok = true;
    for r in rows {
        // Gate on structure when we have it (a missing sidebar must FAIL, not be averaged away).
        let gated = r.structure.unwrap_or(r.score);
        // **A page we could not PROBE is a broken gate CONFIG, not a pass.** `coverage` is NaN when
        // Chrome rendered no `[id]` elements — and `example.com`, which was in this gate's default URL
        // list, has none. It scored a perfect 100% and inflated the mean of the gate whose entire job is
        // to catch missing content. Mutation-testing found it: emptying `node_rects()` so the browser
        // renders NOTHING still scored 100% there.
        if gated.is_nan() {
            eprintln!(
                "  ⚠ {}: Chrome rendered NO [id] elements — this URL cannot be structurally probed, \
                 so it measures nothing. Choose a URL with ids. Counting it as a pass is how a gate \
                 that cannot fail looks green forever.",
                r.name
            );
            all_ok = false;
        }
        let ok = gated >= floor;
        if !ok {
            all_ok = false;
        }
        println!(
            "{:<24} {:>7.1}% {:>8} {:>8} {:>9} {:>7}",
            r.name,
            r.score * 100.0,
            r.structure
                .map(|s| format!("{:.1}%", s * 100.0))
                .unwrap_or_else(|| "—".into()),
            r.missing,
            r.misplaced,
            if ok { "ok" } else { "BELOW" }
        );
    }
    let n = rows.len().max(1) as f64;
    let mean_v = rows.iter().map(|r| r.score).sum::<f64>() / n;
    let structs: Vec<f64> = rows.iter().filter_map(|r| r.structure).collect();
    let mean_s = if structs.is_empty() {
        None
    } else {
        Some(structs.iter().sum::<f64>() / structs.len() as f64)
    };
    let shapes: Vec<f64> = rows.iter().filter_map(|r| r.shape).collect();
    let mean_shape = if shapes.is_empty() {
        None
    } else {
        Some(shapes.iter().sum::<f64>() / shapes.len() as f64)
    };
    println!("\nMEAN VISUAL:    {:.1}%", mean_v * 100.0);
    if let Some(ms) = mean_s {
        println!(
            "MEAN COVERAGE:  {:.1}%   <-- THE HONEST NUMBER: of the elements Chrome renders, the\n\
             \t\t\tfraction Manuk renders AT ALL (floor {:.0}%). A missing region\n\
             \t\t\tcannot hide in this the way it hides in a pixel score.",
            ms * 100.0,
            floor * 100.0
        );
    }
    if let Some(msh) = mean_shape {
        println!(
            "MEAN SHAPE:     {:.1}%   <-- LAYER-1 (parent-relative): of elements BOTH render, the\n\
             \t\t\tfraction placed right vs their nearest SHARED ancestor. Unlike\n\
             \t\t\tthe old absolute placement, a constant page offset cancels here —\n\
             \t\t\tone root cause counts once (FIDELITY-SCORING-REDESIGN.md Layer 1).",
            msh * 100.0
        );
    }
    println!(
        "\nSide-by-side composites written — LOOK at them. The visual score is a poor proxy: an\n\
         entirely absent sidebar moved it <1 point. THE SCORE GATES; THE EYEBALL DIAGNOSES.\n"
    );
    all_ok
}

#[cfg(test)]
mod shape_tests {
    use super::{certificate, shape_stats, Fidelity};
    use std::collections::HashMap;

    // A realistic selector-path box tree modelling the microsoft.com artifact from the redesign:
    // the page top matches Chrome, a taller-than-Chrome HEADER then pushes the whole content column
    // down by `header_extra` px. That offset originates at ONE element (the content container) and
    // every descendant inherits it — which is exactly the "one cause counted N times" trap.
    //
    // `bad_child` corrupts one leaf's HEIGHT instead — a genuine layout bug that no offset explains.
    const KIDS: usize = 8;
    fn tree(header_extra: i64, bad_child: bool) -> HashMap<String, [i64; 4]> {
        let mut m = HashMap::new();
        // Root + header positions match Chrome exactly (the page top is not shifted).
        m.insert("html/body".to_string(), [0, 0, 1000, 3000]);
        m.insert(
            "html/body/header:nth-child(1)".to_string(),
            [0, 0, 1000, 80 + header_extra], // Manuk's header is `header_extra` px too tall
        );
        // Content container: pushed down by the taller header → its box vs body is off by header_extra.
        let content_y = 80 + header_extra;
        m.insert(
            "html/body/main:nth-child(2)".to_string(),
            [0, content_y, 1000, 2000],
        );
        // Content's children: absolutely shifted by header_extra too, but their position RELATIVE to
        // the content container is unchanged — so SHAPE cancels the offset for every one of them.
        for k in 0..KIDS {
            let ky = content_y + 100 + (k as i64) * 200;
            let h = if bad_child && k == 0 { 999 } else { 150 };
            m.insert(
                format!("html/body/main:nth-child(2)/div:nth-child({})", k + 1),
                [20, ky, 960, h],
            );
        }
        m
    }
    const TOTAL: usize = 3 + KIDS; // body + header + main + KIDS

    #[test]
    fn constant_offset_charged_once_under_shape() {
        let chrome = tree(0, false);
        let manuk = tree(23, false); // header 23px too tall → content column shifted 23px
        let (shape, n) = shape_stats(&chrome, &manuk, 8);
        assert_eq!(n, TOTAL, "every element both engines rendered is scored");
        // SHAPE charges the 23px exactly ONCE — at the content container where it originates. The
        // header (its own height is wrong) also fails; its KIDS all cancel. So exactly 2 of 11 fail.
        let failed = TOTAL - (shape * TOTAL as f64).round() as usize;
        assert_eq!(
            failed, 2,
            "SHAPE must charge a constant offset at its ORIGIN only (header + content container), \
             not once per inheriting descendant — got {failed} failures, shape {shape}"
        );

        // Contrast — absolute placement charges the SAME offset to the container AND all KIDS: the
        // content container + 8 kids = 9 of 11 shifted, so placement collapses. This divergence is
        // the whole point of the redesign; if shape_stats ignored parents the two would be equal.
        let (_, mdy, _, _, place_frac) = super::placement_stats(&chrome, &manuk, 8);
        assert_eq!(mdy, 23, "median absolute dy is the raw 23px offset");
        assert!(
            place_frac <= 2.0 / TOTAL as f64 + 1e-9,
            "absolute placement must be dragged down by the offset it cannot cancel, got {place_frac}"
        );
    }

    #[test]
    fn a_genuinely_wrong_box_still_fails_shape() {
        let chrome = tree(0, false);
        let manuk = tree(0, true); // one leaf's height wrong by 849px — a REAL layout bug
        let (shape, n) = shape_stats(&chrome, &manuk, 8);
        assert_eq!(n, TOTAL);
        let failed = TOTAL - (shape * TOTAL as f64).round() as usize;
        assert_eq!(
            failed, 1,
            "SHAPE must NOT be blind to a real box error — the one bad leaf must fail, got shape {shape}"
        );
    }

    #[test]
    fn only_common_elements_scored() {
        let chrome = tree(0, false);
        let mut manuk = tree(0, false);
        manuk.remove("html/body/main:nth-child(2)/div:nth-child(1)"); // Manuk dropped one leaf
        let (shape, n) = shape_stats(&chrome, &manuk, 8);
        assert_eq!(
            n,
            TOTAL - 1,
            "a box only Chrome rendered is a COVERAGE miss, not a SHAPE miss"
        );
        assert!((shape - 1.0).abs() < f64::EPSILON);
    }

    fn row(name: &str, shape: Option<f64>, jarring: [usize; 4]) -> Fidelity {
        Fidelity {
            name: name.into(),
            score: 1.0,
            differing: 0,
            total: 1,
            structure: Some(1.0),
            shape,
            missing: 0,
            misplaced: 0,
            probed: 10,
            jarring,
        }
    }

    /// The certificate is a CONJUNCTION, and every way of accidentally turning it into an average is
    /// a way of passing it without meeting it. These pin all four.
    #[test]
    fn the_certificate_is_a_conjunction_not_an_average() {
        // 20 sites, all shaped and all clean → holds.
        let all_good: Vec<Fidelity> = (0..20)
            .map(|i| row(&format!("s{i}"), Some(0.9), [0; 4]))
            .collect();
        let c = certificate(&all_good);
        assert_eq!(c.sites, 20);
        assert_eq!(c.scored, 20);
        assert_eq!(c.shape_ok, 20);
        assert!(c.holds(), "20/20 shaped and clean must hold");
        assert!(c.shortfalls().is_empty());

        // ONE invariant below the bar fails the whole thing — 2 of 20 sites with an overlap is 90%
        // clean, and the bar is 95%.
        let mut one_bad = all_good.clone();
        one_bad[0].jarring[1] = 3;
        one_bad[1].jarring[1] = 1;
        let c = certificate(&one_bad);
        assert!(
            !c.holds(),
            "90% clean on ONE invariant must fail the certificate — averaging the four terms \
             together is how a certificate becomes a vibe"
        );
        let sf = c.shortfalls();
        assert_eq!(
            sf.len(),
            1,
            "and it must name exactly the term that missed: {sf:?}"
        );
        assert!(sf[0].starts_with("overlap "), "got {}", sf[0]);

        // A site that could not be SCORED counts AGAINST the bar, never out of it — otherwise the
        // certificate is met by failing to measure, which is the same defect the NaN check in
        // `report` exists for.
        let mut unscored = all_good.clone();
        unscored[0].shape = None;
        unscored[1].shape = Some(f64::NAN);
        let c = certificate(&unscored);
        assert_eq!(c.scored, 18);
        assert_eq!(c.shape_ok, 18);
        assert!(
            !c.holds(),
            "18 of 20 scored is 90% — below the bar, not 100% of what we measured"
        );
        assert!(
            c.shortfalls().iter().any(|s| s.contains("UNSCORED")),
            "the unscored sites must be NAMED: {:?}",
            c.shortfalls()
        );

        // The shape FLOOR is per-site and strict-at-the-boundary-from-below: 0.75 passes, 0.74 does not.
        assert_eq!(certificate(&[row("a", Some(0.75), [0; 4])]).shape_ok, 1);
        assert_eq!(certificate(&[row("a", Some(0.74), [0; 4])]).shape_ok, 0);

        // An EMPTY sweep never holds. A certificate over zero sites is the most flattering possible
        // reading of an engine and the least informative.
        assert!(!certificate(&[]).holds(), "zero sites is not a pass");
    }
}
