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
/// parent-relative (SHAPE) scoring. Keys are `tag[i]/tag[i]/…` from the root, so an ancestor's key
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
                let axis = if dw.abs() > dx.abs().max(dy.abs()).max(dh.abs()) {
                    "width"
                } else if dh.abs() > dx.abs().max(dy.abs()) {
                    "height"
                } else if dy.abs() > dx.abs() {
                    "y (vertical drift)"
                } else {
                    "x (horizontal)"
                };
                format!("geometry: {axis} wrong   (<{}>)", d.tag)
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
