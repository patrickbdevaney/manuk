//! **The differential oracle** (METHODOLOGY Part 2).
//!
//! Chromium as an infinite test generator. Render the *same document* in both engines, diff the
//! geometry and the computed display of every element, **cluster the diffs by root cause**, and rank
//! the clusters by how many distinct sites each one explains.
//!
//! ## Why this replaces "render one page and look at it"
//!
//! Every class bug found so far — `<br>` doing nothing, `display:none` children of a flex container
//! painting their contents, `:checked` never matching, a stylesheet applied against its own media
//! query — was a **machine-visible divergence from Chrome**. They were found by a human rendering
//! one page and looking at it, which is a serial, bandwidth-limited capture process. That is
//! precisely why the discovery rate has not flattened: not enough pages have been looked at.
//!
//! This converts discovery from human-serial to machine-parallel, and self-clustering.
//!
//! ## The two hygiene rules that make the output trustworthy
//!
//! 1. **One snapshot, fed identically to both engines.** Fetching the page independently for each
//!    engine compares two different documents and calls the difference "a bug". That is not
//!    hypothetical: it is exactly what pinned a metric at 5,122px across four *correct* fixes,
//!    because the live origin injected a banner that the `file://` copy never saw.
//! 2. **Never diff against a degraded oracle.** If Chromium's own render is a no-script fallback,
//!    an error page, or an empty shell, the sample is discarded — not scored. MediaWiki serving its
//!    no-script page because we failed an admissions test looked exactly like a catastrophic layout
//!    bug of ours, and it was not.
//!
//! ## Clustering, not listing
//!
//! Raw diffs are a firehose. A cluster with forty site-hits and one root cause outranks forty
//! individual diffs. Three cluster keys, in order of diagnostic power:
//!
//!   * **`display` mismatch** — the box exists in both trees but is a different *kind* of box, or
//!     exists in one and not the other. This is the single most causal signal available: it names
//!     the cascade decision that went wrong.
//!   * **Missing box** — Chrome renders it, we render nothing. Keyed by tag, because a whole tag
//!     going missing is one bug, not N.
//!   * **First divergence** — the first element down the page where geometry breaks. Everything
//!     below it is a consequence, not a cause; reporting the consequences as separate bugs is how
//!     a diff becomes a firehose.
//!
//! ## Geometry is scored parent-relative, not absolute (SHAPE)
//!
//! An earlier version diffed **absolute** boxes, which charged one root cause N times: an ancestor
//! placed 23px too low drags its whole subtree, and every descendant then reads as its own geometry
//! bug — the exact amplification that made a 23px constant offset look like a long tail. Geometry is
//! now scored as **SHAPE**: each element's box relative to the nearest ancestor present in both
//! engines (`common_frame`). A purely inherited translation cancels; only the element where the
//! offset *originates* has a wrong shape and is reported. A page uniformly shifted 23px — which a
//! user does not perceive as broken — collapses from one divergence per element to one, total.

use std::collections::{BTreeMap, HashMap};

use anyhow::Result;

/// One element, as an engine sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct Seen {
    pub tag: String,
    pub display: String,
    pub rect: [i64; 4],
}

/// What the two engines disagreed about, for one element.
#[derive(Debug, Clone)]
pub struct Divergence {
    pub site: String,
    pub id: String,
    pub tag: String,
    /// `"missing" | "display" | "geometry"`.
    pub kind: String,
    pub chrome: String,
    pub manuk: String,
    /// How far off, when it is a geometry divergence.
    pub delta: [i64; 4],
}

/// A root cause, and the sites it explains.
#[derive(Debug, Clone)]
pub struct Cluster {
    /// The signature that groups these — e.g. `display: block → none`, or `missing <input>`.
    pub signature: String,
    pub kind: String,
    /// **The ranking key.** How many distinct sites this one cause explains.
    pub sites: usize,
    /// Total elements affected across all sites.
    pub hits: usize,
    pub examples: Vec<String>,
}

/// Is Chromium's own render usable as an oracle, or is it degraded?
///
/// A no-script fallback, a bot wall, an error page, or an empty shell is not a bug in *our* engine
/// and must never be scored as one. Discard the sample instead.
pub fn oracle_is_healthy(chrome: &HashMap<String, Seen>) -> Result<(), String> {
    // The probe reports what Chromium actually DREW — the element count and the visible text
    // length — rather than how many elements happened to carry an id. A five-element synthetic test
    // page is a perfectly good oracle; a 900-element bot wall with 40 characters of text is not.
    let (elements, text) = match chrome.get("__META__") {
        Some(m) => (m.rect[0], m.rect[1]),
        None => return Err("Chromium's probe produced no health metadata".into()),
    };
    if elements < 4 {
        return Err(format!(
            "Chromium itself drew only {elements} elements — an empty shell, not a document"
        ));
    }
    // A real page has *content*. A bot wall, a cookie interstitial and an error page all have a
    // handful of words and nothing else — and diffing against one scores its emptiness as our bug.
    if text < 20 && elements < 30 {
        return Err(format!(
            "Chromium's render has {elements} elements and {text} characters of visible text — a \
             bot wall, an error page or a no-script fallback, not a document. Discarding rather \
             than diffing against a broken oracle."
        ));
    }
    Ok(())
}

/// Diff one page. `tol` is the geometry tolerance in px.
pub fn diff_page(
    site: &str,
    chrome: &HashMap<String, Seen>,
    manuk: &HashMap<String, Seen>,
    tol: i64,
) -> Vec<Divergence> {
    let mut out = Vec::new();
    for (id, c) in chrome {
        match manuk.get(id) {
            None => out.push(Divergence {
                site: site.into(),
                id: id.clone(),
                tag: c.tag.clone(),
                kind: "missing".into(),
                chrome: format!(
                    "{} [{} {} {}×{}]",
                    c.display, c.rect[0], c.rect[1], c.rect[2], c.rect[3]
                ),
                manuk: "(no box)".into(),
                delta: [0; 4],
            }),
            Some(m) => {
                // A `display` mismatch is reported INSTEAD of the geometry that follows from it —
                // the geometry is the symptom, the display is the cause.
                if !display_agrees(&c.display, &m.display) {
                    out.push(Divergence {
                        site: site.into(),
                        id: id.clone(),
                        tag: c.tag.clone(),
                        kind: "display".into(),
                        chrome: c.display.clone(),
                        manuk: m.display.clone(),
                        delta: [0; 4],
                    });
                    continue;
                }
                // **SHAPE (parent-relative) scoring — the Layer-1 gate.** Absolute-position diffing
                // charges one root cause N times: an ancestor placed 23px too low drags its entire
                // subtree 23px, and every descendant then reads as its own "geometry" bug. But the
                // descendants' *shape* — their box **relative to a shared ancestor frame** — is
                // correct; only the ancestor where the offset originates has a genuinely wrong shape.
                // Score each element against the nearest ancestor present in BOTH engines: a purely
                // inherited translation cancels, and the divergence is reported ONCE, at its origin.
                // A page uniformly shifted 23px (not jarring to a user) now yields ONE divergence at
                // the shifted element, not one per element below it.
                let d = match common_frame(id, chrome, manuk) {
                    Some((cf, mf)) => [
                        (c.rect[0] - cf.rect[0]) - (m.rect[0] - mf.rect[0]), // x within parent frame
                        (c.rect[1] - cf.rect[1]) - (m.rect[1] - mf.rect[1]), // y within parent frame
                        m.rect[2] - c.rect[2], // width is translation-invariant
                        m.rect[3] - c.rect[3], // height is translation-invariant
                    ],
                    // No common ancestor (a root-level element) — nothing to subtract, so the
                    // absolute delta *is* the shape delta. This is the offset's origin.
                    None => [
                        m.rect[0] - c.rect[0],
                        m.rect[1] - c.rect[1],
                        m.rect[2] - c.rect[2],
                        m.rect[3] - c.rect[3],
                    ],
                };
                if d.iter().any(|v| v.abs() > tol) {
                    out.push(Divergence {
                        site: site.into(),
                        id: id.clone(),
                        tag: c.tag.clone(),
                        kind: "geometry".into(),
                        chrome: format!(
                            "[{} {} {}×{}]",
                            c.rect[0], c.rect[1], c.rect[2], c.rect[3]
                        ),
                        manuk: format!("[{} {} {}×{}]", m.rect[0], m.rect[1], m.rect[2], m.rect[3]),
                        delta: d,
                    });
                }
            }
        }
    }
    out
}

/// Chrome and Manuk name some displays differently, and some differences are not divergences.
/// `list-item` vs `block` is a naming difference where the *box* is the same kind; `table-*` names
/// line up. What matters is: is it the same KIND of box?
fn display_agrees(chrome: &str, manuk: &str) -> bool {
    fn norm(d: &str) -> &str {
        match d {
            // A list item is a block that also draws a marker. We model the marker on the box.
            "list-item" => "block",
            "flow-root" => "block",
            "inline flow-root" | "inline-block" => "inline-block",
            other => other,
        }
    }
    norm(chrome) == norm(manuk)
}

/// The nearest ancestor of `path` present in **both** engine maps — the reference frame for
/// parent-relative (SHAPE) scoring. Keys are `tag.SIG:nth-child(n)/…` from the root, so an ancestor's key
/// is a prefix of its descendants'; dropping the last `/component` walks up one level. Returns the
/// (chrome, manuk) boxes of the closest such ancestor, or `None` for a root-level element (no `/`),
/// where there is nothing to subtract and the absolute position is itself the shape.
///
/// Requiring the frame to exist in **both** maps is what makes a purely inherited translation
/// cancel: both engines measure the child against the *same* ancestor, so a constant offset present
/// in that ancestor drops out of the difference.
fn common_frame<'a>(
    path: &str,
    chrome: &'a HashMap<String, Seen>,
    manuk: &'a HashMap<String, Seen>,
) -> Option<(&'a Seen, &'a Seen)> {
    let mut p = path;
    while let Some(cut) = p.rfind('/') {
        p = &p[..cut];
        if let (Some(c), Some(m)) = (chrome.get(p), manuk.get(p)) {
            return Some((c, m));
        }
    }
    None
}

/// The order-of-magnitude band of a geometry offset, as a label for clustering.
///
/// The redesign (§3 (b)) clusters geometry failures "by the offset **value**", because the magnitude
/// is what separates the three populations it identifies: a ~20px near-miss (a shared font-metric /
/// margin constant), and a ~1400–6800px page-height collapse (content that never rendered) are
/// *different causes* that must not merge into one cluster just because they share a tag and an axis.
/// Without a magnitude in the signature they DO merge, and the board cannot tell a saturated near-miss
/// from an amplified collapse.
///
/// Banded by power-of-two floor rather than an exact px: a 23px and a 28px drift are the same cause
/// and must cluster (both land in the 16 band), while 23px and 1400px must not (16 vs 1024). Exact-px
/// keys would over-split neighbours; the power-of-two ladder groups within an order of magnitude and
/// separates across one — which is the distinction that matters.
fn mag_band(mag: i64) -> i64 {
    let m = mag.unsigned_abs();
    if m == 0 {
        0
    } else {
        // Largest power of two ≤ m: 23→16, 28→16, 45→32, 82→64, 1400→1024, 6822→4096.
        1i64 << (63 - m.leading_zeros())
    }
}

/// **Cluster the firehose into root causes**, ranked by how many distinct sites each explains.
pub fn cluster(divs: &[Divergence]) -> Vec<Cluster> {
    // signature -> (kind, sites, hits, examples)
    let mut acc: BTreeMap<
        String,
        (
            String,
            std::collections::BTreeSet<String>,
            usize,
            Vec<String>,
        ),
    > = BTreeMap::new();

    for d in divs {
        let sig = match d.kind.as_str() {
            // The most causal key available: the cascade produced a different KIND of box.
            "display" => format!("display: {} → {}   (<{}>)", d.chrome, d.manuk, d.tag),
            // A whole tag going missing is ONE bug, not N. Keyed by tag, not by element.
            "missing" => format!("missing box: <{}>", d.tag),
            // Geometry is bucketed by which dimension is wrong — a systematic width error and a
            // systematic vertical drift are different bugs with different causes.
            _ => {
                let [dx, dy, dw, dh] = d.delta;
                // The dominant axis names WHICH dimension is wrong; its magnitude band names HOW wrong,
                // which is what separates a near-miss from a page-height collapse (see `mag_band`).
                let (axis, mag) = if dw.abs() > dx.abs().max(dy.abs()).max(dh.abs()) {
                    ("width", dw)
                } else if dh.abs() > dx.abs().max(dy.abs()) {
                    ("height", dh)
                } else if dy.abs() > dx.abs() {
                    ("y (vertical drift)", dy)
                } else {
                    ("x (horizontal)", dx)
                };
                format!("geometry: {axis} ~{}px   (<{}>)", mag_band(mag), d.tag)
            }
        };
        let e = acc
            .entry(sig)
            .or_insert_with(|| (d.kind.clone(), Default::default(), 0, Vec::new()));
        e.1.insert(d.site.clone());
        e.2 += 1;
        if e.3.len() < 3 {
            e.3.push(format!("{}#{}: {} vs {}", d.site, d.id, d.chrome, d.manuk));
        }
    }

    let mut out: Vec<Cluster> = acc
        .into_iter()
        .map(|(signature, (kind, sites, hits, examples))| Cluster {
            signature,
            kind,
            sites: sites.len(),
            hits,
            examples,
        })
        .collect();
    // **Rank by distinct sites explained** — that is the whole point. A cause that breaks forty
    // sites outranks one that breaks forty elements of one site.
    out.sort_by(|a, b| (b.sites, b.hits).cmp(&(a.sites, a.hits)));
    out
}

/// **Jarring invariant — horizontal overflow (Layer 2 of FIDELITY-SCORING-REDESIGN.md).**
///
/// SHAPE scoring (above) certifies that boxes are the right size and in the right place *relative to
/// their parents*; it deliberately forgives a constant page offset because a user does not perceive
/// one. But a box whose right edge runs past the viewport is a different failure: content is cut off
/// or an unexpected horizontal scrollbar appears — one of the most-perceived "this page is broken"
/// signals, and one SHAPE cannot see because the overflowing box may be perfectly shaped relative to
/// an over-wide parent. This counts the elements that spill past `vw` in **Manuk** while Chrome keeps
/// the *same* element within the viewport — attributing the overflow to us, not to a site that
/// legitimately scrolls sideways. `tol` absorbs sub-pixel/scrollbar-gutter noise.
///
/// Returns `(ours_only, examples)`: the count, and up to three `path → right-edge` strings for
/// diagnosis. Chrome-also-overflows elements are excluded — the page, not the engine, is wide there.
pub fn jarring_h_overflow(
    chrome: &HashMap<String, Seen>,
    manuk: &HashMap<String, Seen>,
    vw: i64,
    tol: i64,
) -> (usize, Vec<String>) {
    let edge = |b: &[i64; 4]| b[0] + b[2]; // x + width
    let mut count = 0usize;
    let mut examples: Vec<String> = Vec::new();
    for (id, m) in manuk {
        if edge(&m.rect) <= vw + tol {
            continue; // within our own viewport — not overflowing
        }
        // Only OUR fault: Chrome must render the SAME element AND keep it inside the viewport.
        match chrome.get(id) {
            Some(c) if edge(&c.rect) <= vw + tol => {
                count += 1;
                if examples.len() < 3 {
                    examples.push(format!("{id} → right {} > vw {vw}", edge(&m.rect)));
                }
            }
            _ => {}
        }
    }
    examples.sort();
    (count, examples)
}

/// **Jarring invariant — sibling overlap (Layer 2 of FIDELITY-SCORING-REDESIGN.md).**
///
/// The redesign names overlap the *#1* "broken page" perception: text on text, a control under a
/// banner. SHAPE cannot see it — two boxes can each be shaped correctly relative to their parent and
/// still land on top of each other if a gap/height is wrong. This counts pairs of **siblings** (same
/// parent path) that Chrome renders **disjoint** but Manuk renders **overlapping** in both axes by
/// more than `tol` — attributing the collision to us, never to a design that legitimately stacks
/// (Chrome overlaps them too). Scoped to siblings on purpose: it is where perceived collisions cluster
/// (flex/flow items, list rows, stacked cards) and it keeps the cost bounded — cross-subtree occlusion
/// is the hittability invariant's job (occlusion-aware hit-test), not this one.
///
/// Groups larger than `MAX_GROUP` siblings skip the O(n²) pairwise scan; the count of skipped groups
/// is returned so a bounded scan is never mistaken for a clean page. Keys are `tag.SIG:nth-child(n)/…` paths, so the
/// parent is the prefix before the last `/`.
pub fn jarring_overlap(
    chrome: &HashMap<String, Seen>,
    manuk: &HashMap<String, Seen>,
    tol: i64,
) -> (usize, usize, Vec<String>) {
    const MAX_GROUP: usize = 128;
    // Both engines must render the element, and it must have a parent (a `/` in the key).
    let mut groups: BTreeMap<&str, Vec<&String>> = BTreeMap::new();
    for id in manuk.keys() {
        if !chrome.contains_key(id) {
            continue;
        }
        if let Some(cut) = id.rfind('/') {
            groups.entry(&id[..cut]).or_default().push(id);
        }
    }
    // Overlap extent along one axis: how far the two intervals [p, p+len) intersect (≤0 = disjoint).
    let ov = |p0: i64, l0: i64, p1: i64, l1: i64| (p0 + l0).min(p1 + l1) - p0.max(p1);
    let overlaps = |a: &[i64; 4], b: &[i64; 4], t: i64| {
        ov(a[0], a[2], b[0], b[2]) > t && ov(a[1], a[3], b[1], b[3]) > t
    };

    let (mut count, mut skipped) = (0usize, 0usize);
    let mut examples: Vec<String> = Vec::new();
    for (_, ids) in groups {
        if ids.len() < 2 {
            continue;
        }
        if ids.len() > MAX_GROUP {
            skipped += 1;
            continue;
        }
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let (ma, mb) = (&manuk[ids[i]].rect, &manuk[ids[j]].rect);
                let (ca, cb) = (&chrome[ids[i]].rect, &chrome[ids[j]].rect);
                // OUR fault only: they collide for us but Chrome keeps them apart.
                if overlaps(ma, mb, tol) && !overlaps(ca, cb, tol) {
                    count += 1;
                    if examples.len() < 3 {
                        let (lo, hi) = if ids[i] <= ids[j] {
                            (ids[i], ids[j])
                        } else {
                            (ids[j], ids[i])
                        };
                        examples.push(format!("{lo} × {hi}"));
                    }
                }
            }
        }
    }
    examples.sort();
    (count, skipped, examples)
}

/// **Jarring invariant — reading-order inversion (Layer 2 of FIDELITY-SCORING-REDESIGN.md).**
///
/// The redesign names "reading order preserved" a Phase-0 bar: screen order must match the order a
/// user reads in (top-to-bottom, then left-to-right). A float, an abspos, or a mis-placed grid item
/// that escapes its slot makes a later element render *before* an earlier one — the content jumps out
/// of sequence even when nothing overlaps and nothing shapes wrong. SHAPE cannot see it (both boxes
/// can be individually well-shaped) and overlap cannot see it (two disjoint boxes can still read out
/// of order).
///
/// It counts pairs of **siblings** (same parent path) whose reading order **Chrome and Manuk disagree
/// about**: Chrome reads A-before-B while Manuk reads B-before-A, each with a clear margin. Chrome is
/// the reference for the intended order — a normal-flow engine lays siblings out in DOM order, so a
/// disagreement is Manuk pulling one out of place, never a legitimately reordered design (if the site
/// itself reorders, Chrome reflects it and the pair agrees). Both orders must be **definite** (past
/// `tol` on the deciding axis); a pair too close to call in either engine is skipped, so tolerance
/// jitter never manufactures an inversion.
///
/// Same bound and skipped-group accounting as [`jarring_overlap`]: groups above `MAX_GROUP` skip the
/// O(n²) scan and the skipped count is surfaced so a bounded scan is never read as a clean page.
pub fn jarring_reading_order(
    chrome: &HashMap<String, Seen>,
    manuk: &HashMap<String, Seen>,
    tol: i64,
) -> (usize, usize, Vec<String>) {
    const MAX_GROUP: usize = 128;
    let mut groups: BTreeMap<&str, Vec<&String>> = BTreeMap::new();
    for id in manuk.keys() {
        if !chrome.contains_key(id) {
            continue;
        }
        if let Some(cut) = id.rfind('/') {
            groups.entry(&id[..cut]).or_default().push(id);
        }
    }
    // Reading order of `a` vs `b`: -1 = a first, 1 = b first, 0 = too close to call. Vertical wins
    // (a row above reads first); within a row, leftmost reads first. `rect` is [x, y, w, h].
    let order = |a: &[i64; 4], b: &[i64; 4], t: i64| -> i8 {
        if a[1] + t < b[1] {
            return -1;
        }
        if b[1] + t < a[1] {
            return 1;
        }
        if a[0] + t < b[0] {
            return -1;
        }
        if b[0] + t < a[0] {
            return 1;
        }
        0
    };

    let (mut count, mut skipped) = (0usize, 0usize);
    let mut examples: Vec<String> = Vec::new();
    for (_, ids) in groups {
        if ids.len() < 2 {
            continue;
        }
        if ids.len() > MAX_GROUP {
            skipped += 1;
            continue;
        }
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let co = order(&chrome[ids[i]].rect, &chrome[ids[j]].rect, tol);
                let mo = order(&manuk[ids[i]].rect, &manuk[ids[j]].rect, tol);
                // Both engines must be sure, and they must disagree — that is an inversion we caused.
                if co != 0 && mo != 0 && co != mo {
                    count += 1;
                    if examples.len() < 3 {
                        let (lo, hi) = if ids[i] <= ids[j] {
                            (ids[i], ids[j])
                        } else {
                            (ids[j], ids[i])
                        };
                        examples.push(format!("{lo} ⇄ {hi}"));
                    }
                }
            }
        }
    }
    examples.sort();
    (count, skipped, examples)
}

/// The interactive tags a user is expected to be able to click, tab to, or type into. A control
/// among these that renders with no clickable area is a *dead control* — the hittability failure the
/// redesign names ("a button you cannot click is a dead page"). Tag-only because the box dump carries
/// no attributes; `[role=button]`-style ARIA controls are invisible to it and left for a later pass.
const INTERACTIVE_TAGS: &[&str] = &[
    "a", "button", "input", "select", "textarea", "summary", "details", "label",
];

/// **Jarring invariant — collapsed interactive target (Layer 2 of FIDELITY-SCORING-REDESIGN.md).**
///
/// The redesign names "interactive targets hittable" a Phase-0 bar. Hittability fails two ways: a
/// control **collapses** so it has no clickable area, or a control is **covered** by something painted
/// over it (a button under a banner). This checks the first — the box-dump-computable half. The
/// occlusion-cover half needs paint order / opacity, which the geometry snapshot does not carry, and
/// is left for a later pass (partially surfaced already by [`jarring_overlap`]); this function does
/// not claim to be the whole invariant.
///
/// It counts elements with an interactive tag that Chrome renders with a real clickable box (both axes
/// ≥ `min_hit`) but Manuk **collapses** (either axis < `min_hit`) — a control the user cannot click.
/// The "Chrome gives it area" guard is load-bearing: a control the *site* itself collapses (hidden in
/// both engines) is not our bug, exactly as the overlap guard forgives a deliberate stack. It is
/// **offset-invariant** — a page shifted 23px collapses nothing — so it never re-charges the constant
/// offset SHAPE already forgives. Fully-collapsed (0×0) controls never reach here: the probe drops
/// them, so they surface as a *missing* divergence instead; this catches the single-axis collapse
/// (a zero-height button from a collapsed flex/grid track) that keeps a box but kills the target.
pub fn jarring_collapsed_target(
    chrome: &HashMap<String, Seen>,
    manuk: &HashMap<String, Seen>,
    min_hit: i64,
) -> (usize, Vec<String>) {
    let hittable = |r: &[i64; 4]| r[2] >= min_hit && r[3] >= min_hit;
    let mut count = 0usize;
    let mut examples: Vec<String> = Vec::new();
    for (id, m) in manuk {
        if !INTERACTIVE_TAGS.contains(&m.tag.as_str()) {
            continue;
        }
        let Some(c) = chrome.get(id) else { continue };
        // Chrome gives it a clickable box; we collapse it. That collapse is ours.
        if hittable(&c.rect) && !hittable(&m.rect) {
            count += 1;
            if examples.len() < 3 {
                examples.push(format!("{id} ({}×{})", m.rect[2], m.rect[3]));
            }
        }
    }
    examples.sort();
    (count, examples)
}

/// The four jarring invariants a per-site oracle run emits, in fixed order for aggregation.
/// Mirrors the `--emit` meta fields `overlap` / `h_overflow` / `reorder` / `dead_target`.
pub const JARRING_LABELS: [&str; 4] = ["overlap", "h-overflow", "reorder", "dead-target"];

/// **Aggregate the per-site jarring-invariant counts into the corpus Phase-0 tally.**
///
/// The invariants are computed and emitted per site, but the number that certifies Phase 0 is
/// corpus-wide: *how many sites* exhibit each jarring failure, and how many instances in total. This
/// rolls a slice of per-site `[overlap, h_overflow, reorder, dead_target]` rows into
/// `(sites_affected, total)` per invariant — sites-affected first because the redesign gates on the
/// fraction of the corpus that is *not* jarring, not on the raw instance count (one site with 40
/// overlaps must not outweigh 40 sites with one each). A site contributes to `sites_affected` for an
/// invariant only when its count for that invariant is > 0.
pub fn tally_jarring(per_site: &[[i64; 4]]) -> [(usize, i64); 4] {
    let mut agg = [(0usize, 0i64); 4];
    for row in per_site {
        for (k, slot) in agg.iter_mut().enumerate() {
            if row[k] > 0 {
                slot.0 += 1;
                slot.1 += row[k];
            }
        }
    }
    agg
}

/// The report a tick actually reads.
pub fn report(clusters: &[Cluster], sites: usize, skipped: usize) {
    println!("\n=== DIFFERENTIAL ORACLE — root causes, ranked by sites explained ===\n");
    println!(
        "  {sites} site(s) diffed, {skipped} discarded (Chromium's own render was degraded)\n"
    );
    println!("{:>6} {:>6}  {}", "sites", "hits", "root cause");
    for c in clusters.iter().take(30) {
        println!("{:>6} {:>6}  {}", c.sites, c.hits, c.signature);
        for e in c.examples.iter().take(1) {
            println!("{:>14}  e.g. {e}", "");
        }
    }
    println!(
        "\nRanked by DISTINCT SITES, not by hit count: a cause that breaks forty sites outranks one\n\
         that breaks forty elements of a single site. This ordering is the priority ledger\n\
         (METHODOLOGY Part 4) — no judgement required.\n"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seen(tag: &str, rect: [i64; 4]) -> Seen {
        Seen {
            tag: tag.into(),
            display: "block".into(),
            rect,
        }
    }

    fn geom_div(site: &str, tag: &str, delta: [i64; 4]) -> Divergence {
        Divergence {
            site: site.into(),
            id: format!("body[0]/{tag}[0]"),
            tag: tag.into(),
            kind: "geometry".into(),
            chrome: String::new(),
            manuk: String::new(),
            delta,
        }
    }

    /// **Offset-magnitude banding in the geometry cluster key, and its RED proof.** Two sites drift a
    /// `<header>` down by 23px and 28px — the same near-miss cause — and must cluster together (both
    /// land in the 16px band). A third site collapses the same `<header>` by 1400px (content that never
    /// rendered) — a *different* cause that must NOT merge with the near-miss (band 1024). So `cluster`
    /// must yield TWO geometry causes, the near-miss explaining 2 sites and the collapse explaining 1.
    ///
    /// Dropping `mag_band` from the signature (keying on axis+tag alone, as before) merges all three
    /// into ONE cluster of 3 sites — and this assertion fails. The magnitude band is what lets the
    /// board tell a saturated near-miss from an amplified page collapse.
    #[test]
    fn cluster_bands_geometry_by_offset_magnitude() {
        let divs = vec![
            geom_div("a.example", "header", [0, 23, 0, 0]),
            geom_div("b.example", "header", [0, 28, 0, 0]),
            geom_div("c.example", "header", [0, 1400, 0, 0]),
        ];
        let clusters = cluster(&divs);
        assert_eq!(
            clusters.len(),
            2,
            "near-miss (23/28px) and collapse (1400px) are distinct causes, not one"
        );
        // Ranked by distinct sites: the near-miss (2 sites) leads the collapse (1 site).
        assert_eq!(
            clusters[0].sites, 2,
            "the 23/28px near-miss clusters two sites"
        );
        assert!(
            clusters[0].signature.contains("~16px"),
            "near-miss lands in the 16px band, got {:?}",
            clusters[0].signature
        );
        assert_eq!(clusters[1].sites, 1);
        assert!(
            clusters[1].signature.contains("~1024px"),
            "the 1400px collapse lands in the 1024px band, got {:?}",
            clusters[1].signature
        );
    }

    /// **The corpus jarring tally, and its RED proof.** Three sites: site A has 2 overlaps + 1
    /// reorder, site B has 3 overlaps, site C is clean. The tally must report overlap as (2 sites, 5
    /// total) and reorder as (1 site, 1 total) — sites-affected counts only the sites with a nonzero
    /// count, so one busy site does not masquerade as many.
    ///
    /// Dropping the `row[k] > 0` guard on the sites-affected increment (counting every site) makes
    /// overlap read (3 sites, 5) — the clean site C then falsely counts as affected, and this fails.
    /// That guard is what makes "fraction of the corpus that is jarring" an honest number.
    #[test]
    fn tally_jarring_counts_sites_affected_not_just_instances() {
        // rows are [overlap, h_overflow, reorder, dead_target].
        let per_site = [
            [2, 0, 1, 0], // site A
            [3, 0, 0, 0], // site B
            [0, 0, 0, 0], // site C — clean
        ];
        let agg = tally_jarring(&per_site);
        assert_eq!(
            agg[0],
            (2, 5),
            "overlap: 2 sites affected, 5 instances total"
        );
        assert_eq!(agg[1], (0, 0), "no h-overflow anywhere");
        assert_eq!(agg[2], (1, 1), "reorder: 1 site, 1 instance");
        assert_eq!(agg[3], (0, 0), "no dead targets");
    }

    /// **The Layer-1 SHAPE gate, and its RED proof.** A page uniformly shifted 23px down: the
    /// `<body>` is the origin (its own box is 23px wrong), every descendant merely inherits the
    /// translation, and one genuinely misshapen box (`div[1]`, 73px too high *within its parent* and
    /// 50px too wide) is a real bug.
    ///
    /// Parent-relative scoring must report exactly the origin and the real bug — NOT the pure
    /// inheritors. Reverting `diff_page` to absolute-box diffing (`m.rect[i] - c.rect[i]`) makes the
    /// two inheritors reappear and this assertion fail, which is what makes it a ratchet tooth.
    #[test]
    fn shape_scoring_suppresses_inherited_offset_keeps_real_bug() {
        let tol = 8;
        // Chrome — the ground truth.
        let mut chrome: HashMap<String, Seen> = HashMap::new();
        chrome.insert("body[0]".into(), seen("body", [0, 0, 1000, 2000]));
        chrome.insert("body[0]/div[0]".into(), seen("div", [0, 100, 1000, 500]));
        chrome.insert(
            "body[0]/div[0]/span[0]".into(),
            seen("span", [0, 150, 200, 20]),
        );
        chrome.insert("body[0]/div[1]".into(), seen("div", [0, 700, 1000, 300]));

        // Manuk — everything shifted +23px in y (a constant page offset), EXCEPT div[1], which is a
        // genuine shape bug: 50px too high relative to body, and 50px too wide.
        let mut manuk: HashMap<String, Seen> = HashMap::new();
        manuk.insert("body[0]".into(), seen("body", [0, 23, 1000, 2000]));
        manuk.insert("body[0]/div[0]".into(), seen("div", [0, 123, 1000, 500]));
        manuk.insert(
            "body[0]/div[0]/span[0]".into(),
            seen("span", [0, 173, 200, 20]),
        );
        // div[1]: body-relative y is 650-23=627 vs Chrome's 700 (−73px), width 1050 vs 1000 (+50px).
        manuk.insert("body[0]/div[1]".into(), seen("div", [0, 650, 1050, 300]));

        let divs = diff_page("t", &chrome, &manuk, tol);
        let ids: std::collections::BTreeSet<&str> = divs.iter().map(|d| d.id.as_str()).collect();

        // The origin (its own box is wrong: no common frame, absolute delta = shape delta = 23px).
        assert!(
            ids.contains("body[0]"),
            "origin of the offset must be reported"
        );
        // The genuine misshapen box.
        assert!(
            ids.contains("body[0]/div[1]"),
            "a box wrong relative to its parent must be reported"
        );
        // The pure inheritors — correct SHAPE, only inherited translation — must NOT be reported.
        assert!(
            !ids.contains("body[0]/div[0]"),
            "an inherited offset is not an independent bug"
        );
        assert!(
            !ids.contains("body[0]/div[0]/span[0]"),
            "a deep inheritor is not an independent bug"
        );
        assert_eq!(
            divs.len(),
            2,
            "exactly the origin and the real bug, nothing amplified"
        );
    }

    /// **The horizontal-overflow jarring invariant, and its RED proof.** One box spills past the
    /// viewport in Manuk while Chrome keeps it inside (our fault); one spills in BOTH (the site
    /// scrolls sideways — not our bug); one is within tolerance. Only the first must count.
    ///
    /// Dropping the "Chrome keeps the same element inside" guard (the `Some(c) if …` arm) makes the
    /// legitimately-wide element count too, and this assertion fails — the guard is what keeps the
    /// invariant from blaming us for a page that is simply wide.
    #[test]
    fn jarring_h_overflow_blames_only_our_own_spill() {
        let vw = 1200;
        let tol = 8;
        let mut chrome: HashMap<String, Seen> = HashMap::new();
        let mut manuk: HashMap<String, Seen> = HashMap::new();
        // (a) OUR fault: Chrome fits (right 1200), Manuk spills (right 1400).
        chrome.insert("body[0]/div[0]".into(), seen("div", [0, 0, 1200, 50]));
        manuk.insert("body[0]/div[0]".into(), seen("div", [0, 0, 1400, 50]));
        // (b) The SITE is wide: both spill (right 2000) — not our bug.
        chrome.insert("body[0]/div[1]".into(), seen("div", [0, 60, 2000, 50]));
        manuk.insert("body[0]/div[1]".into(), seen("div", [0, 60, 2000, 50]));
        // (c) Within tolerance: right 1205 ≤ vw+tol.
        chrome.insert("body[0]/div[2]".into(), seen("div", [0, 120, 1200, 50]));
        manuk.insert("body[0]/div[2]".into(), seen("div", [0, 120, 1205, 50]));

        let (count, examples) = jarring_h_overflow(&chrome, &manuk, vw, tol);
        assert_eq!(
            count, 1,
            "only the element we alone push past the viewport counts"
        );
        assert!(
            examples[0].starts_with("body[0]/div[0]"),
            "the example names the offending element, got {examples:?}"
        );
    }

    /// **The sibling-overlap jarring invariant, and its RED proof.** Two siblings Chrome keeps
    /// disjoint (stacked 0–40 and 40–80) collide in Manuk (both at 0–60); a second sibling pair
    /// overlaps in BOTH engines (a deliberate stack) and must not count; a pair in a different parent
    /// never collides. Only the first pair is our bug.
    ///
    /// Dropping the `&& !overlaps(ca, cb, tol)` guard makes the both-engines-overlap pair count too,
    /// and this assertion fails — the guard is what keeps a legitimate stack from being blamed on us.
    #[test]
    fn jarring_overlap_blames_only_collisions_chrome_keeps_apart() {
        let tol = 4;
        let mut chrome: HashMap<String, Seen> = HashMap::new();
        let mut manuk: HashMap<String, Seen> = HashMap::new();
        // Pair A (our bug): Chrome stacks them (y 0–40, 40–80); Manuk overlaps (both y 0–60, x 0–100).
        chrome.insert("body[0]/div[0]".into(), seen("div", [0, 0, 100, 40]));
        chrome.insert("body[0]/div[1]".into(), seen("div", [0, 40, 100, 40]));
        manuk.insert("body[0]/div[0]".into(), seen("div", [0, 0, 100, 60]));
        manuk.insert("body[0]/div[1]".into(), seen("div", [0, 0, 100, 60]));
        // Pair B (intentional stack): overlaps in BOTH engines — not our bug.
        chrome.insert("body[0]/span[0]".into(), seen("span", [0, 0, 50, 50]));
        chrome.insert("body[0]/span[1]".into(), seen("span", [10, 10, 50, 50]));
        manuk.insert("body[0]/span[0]".into(), seen("span", [0, 0, 50, 50]));
        manuk.insert("body[0]/span[1]".into(), seen("span", [10, 10, 50, 50]));

        let (count, skipped, examples) = jarring_overlap(&chrome, &manuk, tol);
        assert_eq!(skipped, 0);
        assert_eq!(count, 1, "only the collision Chrome keeps disjoint is ours");
        assert_eq!(
            examples,
            vec!["body[0]/div[0] × body[0]/div[1]".to_string()]
        );
    }

    /// **The reading-order-inversion jarring invariant, and its RED proof.** Pair A: Chrome reads
    /// `div[0]` before `div[1]` (stacked, y 0 then 100); Manuk renders them swapped (y 100 then 0), so
    /// a user reads them out of sequence — our bug. Pair B: both engines agree on the order (a design
    /// Chrome reflects too) and must not count. Pair C: a pair too close to call (within tol on both
    /// axes) in Manuk is skipped, so jitter never manufactures an inversion. A pair in another parent
    /// is never compared.
    ///
    /// Dropping the `co != mo` disagreement check (counting whenever both orders are definite) makes
    /// the AGREEING pair B count too — count becomes 2, and this assertion fails. That check is what
    /// distinguishes an inversion from a page that simply has an order.
    #[test]
    fn jarring_reading_order_blames_only_orders_chrome_disagrees_with() {
        let tol = 4;
        let mut chrome: HashMap<String, Seen> = HashMap::new();
        let mut manuk: HashMap<String, Seen> = HashMap::new();
        // Each pair sits under its OWN parent so only intended pairs are compared (siblings share a
        // parent path — mixing tags under one parent would compare across pairs, which is correct but
        // not what this fixture isolates). Parent wrappers need not be in the map; grouping is by key.
        // Pair A (our bug): Chrome reads div[0] then div[1] (y 0, 100); Manuk swaps them (y 100, 0).
        chrome.insert(
            "body[0]/section[0]/div[0]".into(),
            seen("div", [0, 0, 100, 40]),
        );
        chrome.insert(
            "body[0]/section[0]/div[1]".into(),
            seen("div", [0, 100, 100, 40]),
        );
        manuk.insert(
            "body[0]/section[0]/div[0]".into(),
            seen("div", [0, 100, 100, 40]),
        );
        manuk.insert(
            "body[0]/section[0]/div[1]".into(),
            seen("div", [0, 0, 100, 40]),
        );
        // Pair B (order agrees): both engines read p[0] before p[1] — a real order, not our bug.
        chrome.insert("body[0]/section[1]/p[0]".into(), seen("p", [0, 0, 100, 20]));
        chrome.insert(
            "body[0]/section[1]/p[1]".into(),
            seen("p", [0, 40, 100, 20]),
        );
        manuk.insert("body[0]/section[1]/p[0]".into(), seen("p", [0, 0, 100, 20]));
        manuk.insert(
            "body[0]/section[1]/p[1]".into(),
            seen("p", [0, 40, 100, 20]),
        );
        // Pair C (too close to call in Manuk): Chrome orders them, Manuk stacks them at the same spot.
        chrome.insert(
            "body[0]/section[2]/span[0]".into(),
            seen("span", [0, 0, 50, 10]),
        );
        chrome.insert(
            "body[0]/section[2]/span[1]".into(),
            seen("span", [60, 0, 50, 10]),
        );
        manuk.insert(
            "body[0]/section[2]/span[0]".into(),
            seen("span", [0, 0, 50, 10]),
        );
        manuk.insert(
            "body[0]/section[2]/span[1]".into(),
            seen("span", [1, 1, 50, 10]),
        );

        let (count, skipped, examples) = jarring_reading_order(&chrome, &manuk, tol);
        assert_eq!(skipped, 0);
        assert_eq!(
            count, 1,
            "only the pair Chrome and Manuk order differently is ours"
        );
        assert_eq!(
            examples,
            vec!["body[0]/section[0]/div[0] ⇄ body[0]/section[0]/div[1]".to_string()]
        );
    }

    /// **The collapsed-target jarring invariant, and its RED proof.** A `<button>` Chrome renders
    /// 100×30 (hittable) collapses to 100×0 in Manuk — a dead control, our bug. A `<button>` collapsed
    /// in BOTH engines (the site hides it) must not count. A `<div>` collapsed by us is not a control,
    /// so it is ignored. A `<button>` hittable in both is fine.
    ///
    /// Dropping the `hittable(&c.rect)` guard makes the both-engines-collapsed button count too — the
    /// guard is what keeps a control the SITE collapses from being blamed on us.
    #[test]
    fn jarring_collapsed_target_blames_only_controls_chrome_gives_area() {
        let min_hit = 2;
        let mut chrome: HashMap<String, Seen> = HashMap::new();
        let mut manuk: HashMap<String, Seen> = HashMap::new();
        // Our bug: Chrome gives the button a box (100×30); Manuk collapses its height to 0.
        chrome.insert("body[0]/button[0]".into(), seen("button", [0, 0, 100, 30]));
        manuk.insert("body[0]/button[0]".into(), seen("button", [0, 0, 100, 0]));
        // Site-hidden: collapsed in BOTH engines — not our bug.
        chrome.insert("body[0]/button[1]".into(), seen("button", [0, 0, 100, 0]));
        manuk.insert("body[0]/button[1]".into(), seen("button", [0, 0, 100, 0]));
        // Not a control: a collapsed div is ignored.
        chrome.insert("body[0]/div[0]".into(), seen("div", [0, 0, 100, 30]));
        manuk.insert("body[0]/div[0]".into(), seen("div", [0, 0, 100, 0]));
        // A control hittable in both is fine.
        chrome.insert("body[0]/a[0]".into(), seen("a", [0, 0, 50, 20]));
        manuk.insert("body[0]/a[0]".into(), seen("a", [0, 0, 50, 20]));

        let (count, examples) = jarring_collapsed_target(&chrome, &manuk, min_hit);
        assert_eq!(count, 1, "only the control we alone collapse is ours");
        assert_eq!(examples, vec!["body[0]/button[0] (100×0)".to_string()]);
    }

    /// `common_frame` walks to the nearest ancestor **present in both** maps, skipping any absent
    /// intermediate level, and yields `None` at the root.
    #[test]
    fn common_frame_finds_nearest_shared_ancestor() {
        let mut chrome: HashMap<String, Seen> = HashMap::new();
        let mut manuk: HashMap<String, Seen> = HashMap::new();
        chrome.insert("body[0]".into(), seen("body", [0, 0, 10, 10]));
        manuk.insert("body[0]".into(), seen("body", [0, 5, 10, 10]));
        // "body[0]/div[0]" is absent from both — the walk must skip it and land on "body[0]".
        let f = common_frame("body[0]/div[0]/span[0]", &chrome, &manuk);
        assert!(f.is_some(), "must fall back to the nearest shared ancestor");
        assert_eq!(f.unwrap().0.rect, [0, 0, 10, 10]);
        assert!(
            common_frame("body[0]", &chrome, &manuk).is_none(),
            "a root-level element has no frame to subtract"
        );
    }
}
