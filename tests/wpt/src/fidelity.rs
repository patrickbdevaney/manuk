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
pub struct Fidelity {
    pub name: String,
    /// Visual: fraction of grid blocks agreeing with Chromium, 0.0–1.0.
    pub score: f64,
    pub differing: usize,
    pub total: usize,
    /// **Structural COVERAGE**: of the elements Chrome renders, what fraction does Manuk render at
    /// all? This is the honest number — a missing region cannot hide in it. `None` if unprobed.
    pub structure: Option<f64>,
    /// Elements Chrome renders that Manuk does **not** produce a box for at all.
    pub missing: usize,
    /// Elements both render, but Manuk places/sizes wrongly (beyond tolerance).
    pub misplaced: usize,
    pub probed: usize,
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
        missing: 0,
        misplaced: 0,
        probed: 0,
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
    img.save(dest).with_context(|| format!("writing {}", dest.display()))?;
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
    let coverage = if probed == 0 { 1.0 } else { rendered as f64 / probed as f64 };
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
    let frac = if n == 0 { 1.0 } else { within as f64 / n as f64 };
    (med(&mut d[0]), med(&mut d[1]), med(&mut d[2]), med(&mut d[3]), frac)
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
        let ok = gated >= floor;
        if !ok {
            all_ok = false;
        }
        println!(
            "{:<24} {:>7.1}% {:>8} {:>8} {:>9} {:>7}",
            r.name,
            r.score * 100.0,
            r.structure.map(|s| format!("{:.1}%", s * 100.0)).unwrap_or_else(|| "—".into()),
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
    println!(
        "\nSide-by-side composites written — LOOK at them. The visual score is a poor proxy: an\n\
         entirely absent sidebar moved it <1 point. THE SCORE GATES; THE EYEBALL DIAGNOSES.\n"
    );
    all_ok
}
