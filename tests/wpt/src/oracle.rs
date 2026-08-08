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
    /// **The computed font that produced this box** — `"<first family>/<used px>"`, or empty when the
    /// producer does not supply it.
    ///
    /// A rect cannot say which FACE or what SIZE made it, and by t562 every remaining text-metric lead
    /// was blocked on exactly that: `martinfowler.com` reports `[74×16] vs [76×18]` and the 2px is
    /// **unattributable** — a different face, a different used size, or a different line-box rule all
    /// look identical in a rect. A per-line delta compounds down a block *and* moves inline wrap points,
    /// so it surfaces as "displacement" far from its cause; without the font, the next question has
    /// nowhere to start.
    ///
    /// So the diff carries it, and a 2px height divergence reads as *"Chromium used Face A at 13px, we
    /// used Face B at 14px"*. This is the same move as `.SIG` off the key (t550), `median_mag` (t552),
    /// printed instances (t553) and the displaced/mis-sized split (t554): **make the diff carry the
    /// datum the next question needs.** Empty is a legitimate value — a non-text element has no font
    /// worth reporting — and an empty string prints as nothing rather than as a guess.
    pub font: String,
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
    /// **The MEDIAN of the actual dominant-axis deltas in this cluster, in px** — 0 for non-geometry
    /// clusters.
    ///
    /// This exists because the signature's `~Npx` is a [`mag_band`] value, i.e. **rounded down to a
    /// power of two**, and a reader cannot tell that from looking. At tick 551 the pooled sweep output
    /// read `width ~16px · height ~128px · width ~8px · height ~16px · ~64px · ~32px` and I recorded in
    /// the roadmap anchor that the deltas were *"QUANTISED — 8/16/32/64/128 — the signature of ONE
    /// systematic box-model delta, not a thousand independent bugs."* **They are quantised by the
    /// BANDING.** Every geometry delta this instrument has ever reported is a power of two by
    /// construction, so the pattern I read as evidence was a property of the printer.
    ///
    /// Lesson #4 in `STATUS.md` — *every number has a harness, and the harness is part of the number* —
    /// firing on a conclusion drawn ONE TICK after writing the constitution check that warns about it.
    /// The band still earns its place (it separates a 20px near-miss from a 4,000px collapse, which are
    /// genuinely different bugs); it just must not be mistaken for the measurement. So both travel now.
    pub median_mag: i64,
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

/// Render a `Seen.font` for an instance line: ` {Open Sans/13}`, or nothing when it is empty. Kept
/// separate so an absent font prints as ABSENCE rather than as `{/0}`, which would read like a measured
/// zero.
fn fontsuffix(font: &str) -> String {
    if font.is_empty() {
        String::new()
    } else {
        format!("  {{{font}}}")
    }
}

/// Diff one page. `tol` is the geometry tolerance in px.
pub fn diff_page(
    site: &str,
    chrome: &HashMap<String, Seen>,
    manuk: &HashMap<String, Seen>,
    tol: i64,
) -> Vec<Divergence> {
    let mut out = Vec::new();
    // ⚠⚠⚠ **`missing` MEANS THE KEY IS ABSENT, NOT THE BOX — AND UNTIL t912 THE RANKER DID NOT
    // KNOW THE DIFFERENCE.**
    //
    // `manuk.get(id) == None` has three causes and they are not the same bug: the node is absent
    // from our DOM, the node exists and we gave it no box, or **the node exists WITH a box under a
    // different path** — `nth-of-type` is absolute, so one inserted sibling re-numbers every key
    // beneath it (t780-783). All three were being counted as *"Chrome renders it, we render
    // nothing"*, which is the sentence the board has ranked #1 since t684.
    //
    // Measured at t911 over the t909 sweep, from counts the instrument was already printing and
    // nobody had read against each other: **of the 58 sites carrying a missing-`<div>` count, 22
    // render AS MANY OR MORE box-bearing paths than Chrome.**
    //
    // ```text
    //   div_miss  oracle    ours  missing   site
    //        471    2407    2380     1247   sip777man.site      99% as many boxes, 1247 "missing"
    //        322     665     625      625   www.kroftools.com   94% as many, EVERY path missing
    //        220     458     601      456   www.jatekshop.eu    WE DRAW MORE, share 2 of 458
    //        181     696     676      680   a1.ro               676 vs 696, 16 paths in common
    // ```
    //
    // Two engines that each draw ~690 boxes and agree on sixteen paths are not one engine failing
    // to render; they are two trees numbered differently.
    //
    // ⚠ **THIS IS THE SAME CORRECTION t782 MADE, ONE LEVEL OUT.** `TreeDivergence` was split from
    // `ThinOverlap` after measuring *"the one thing this variant never looked at: our own element
    // count"* — and that fix reached the UNSCORED path only. A site that SCORES kept feeding raw
    // `missing` divergences into the ranked cause list, where the question t782 added was never
    // asked. One rule, two implementations, and the quiet one publishes the priority ledger.
    //
    // ⚠ **AND IT IS NOT AN EXONERATION, exactly as t782's is not.** `unaligned` says only *"our map
    // is not smaller, so this absence is not evidence of a dropped box"*. It is still a divergence,
    // it is still counted, and the arithmetic of the certificate is unchanged. What changes is that
    // the loop stops being told a coverage bug is waiting where the evidence does not support one.
    let we_drew_as_many = manuk.len() >= chrome.len();
    for (id, c) in chrome {
        match manuk.get(id) {
            None => out.push(Divergence {
                site: site.into(),
                id: id.clone(),
                tag: c.tag.clone(),
                kind: if we_drew_as_many {
                    "unaligned".into()
                } else {
                    "missing".into()
                },
                chrome: format!(
                    "{} [{} {} {}×{}]{}",
                    c.display,
                    c.rect[0],
                    c.rect[1],
                    c.rect[2],
                    c.rect[3],
                    fontsuffix(&c.font)
                ),
                // ⚠⚠⚠ **AN `unaligned` ROW USED TO SAY ONLY "(no box)", WHICH NAMES THE SYMPTOM
                //     AND HIDES THE CAUSE (t951).** `unaligned` means the two trees are *numbered*
                //     differently — one same-tag sibling somewhere up the path shifts every
                //     `nth-of-type` beneath it — so the useful fact is not that THIS 14-deep path is
                //     absent, it is **WHERE the numbering stopped agreeing**. t949 found 66 of these
                //     on one scored site and could only report the leaf; t950 then spent a tick
                //     failing to attribute it from outside the harness.
                //
                //     The alignment point is free to compute: both maps are in hand and keyed by the
                //     same selector paths, so walk this id's prefixes from the root and report the
                //     LAST one both engines have. Everything below it is re-numbered, and 66 leaves
                //     collapse to one address.
                manuk: format!(
                    "(no box; our tree has nothing below {})",
                    align_point(id, manuk)
                ),
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
                            "[{} {} {}×{}]{}",
                            c.rect[0],
                            c.rect[1],
                            c.rect[2],
                            c.rect[3],
                            fontsuffix(&c.font)
                        ),
                        manuk: format!(
                            "[{} {} {}×{}]{}",
                            m.rect[0],
                            m.rect[1],
                            m.rect[2],
                            m.rect[3],
                            fontsuffix(&m.font)
                        ),
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
/// parent-relative (SHAPE) scoring. Keys are `tag.SIG:nth-of-type(n)/…` from the root, so an ancestor's key
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

/// **The deepest prefix of `id` below which OUR tree still has SOMETHING** — the last level at which
/// the two trees are still talking about the same place.
///
/// ⚠ **What this is and is not.** It reports, exactly, *"we have at least one element at or under
/// this path, and none at or under the next segment"*. That is a literal fact about our map and the
/// string says only that. It is **evidence about** where a re-numbering began — one inserted or
/// missing same-tag sibling shifts every `nth-of-type` beneath it, so N unaligned leaves collapse to
/// one address — but it does **not prove** the sibling difference is at that level: an element we
/// genuinely failed to render produces the identical reading. Distinguishing those needs the child
/// counts on both sides, which is the next step and is not this one.
///
/// An `unaligned` divergence reports a leaf 12-14 levels deep; the leaf is never the cause. One
/// inserted or missing same-tag sibling shifts every `nth-of-type` below it, so N unaligned leaves
/// share ONE origin, and this finds it by walking the path from the root and keeping the last prefix
/// present in both engines. Returns `<root>` when even the first segment disagrees.
fn align_point(id: &str, manuk: &HashMap<String, Seen>) -> String {
    // Ids are `site#seg/seg/seg`; the prefix walk is over the path part only.
    let (head, path) = match id.split_once('#') {
        Some((h, p)) => (h, p),
        None => ("", id),
    };
    let mut best = "<root>".to_string();
    let mut acc = String::new();
    for seg in path.split('/') {
        if !acc.is_empty() {
            acc.push('/');
        }
        acc.push_str(seg);
        let probe = if head.is_empty() {
            acc.clone()
        } else {
            format!("{head}#{acc}")
        };
        // ⚠ **`contains_key` IS THE WRONG TEST AND THE FIRST DRAFT USED IT (caught before landing).**
        // Our map holds the elements the probe RECORDED, not every ancestor on the way to them, so
        // an exact-prefix lookup fails at the first unrecorded ancestor — which is level 1 on every
        // real page. It reported `body:nth-of-type(1)` for all 66 of tz.de's unaligned rows: a
        // constant, which is the signature of a predicate that is not measuring what it names.
        //
        // The question is *"does OUR tree have anything at this path?"*, so test for a key that is
        // this prefix or a descendant of it. The `/` boundary matters: without it
        // `…/div:nth-of-type(1)` also matches `…/div:nth-of-type(10)`.
        let is_desc = |k: &String| {
            k.as_str() == probe.as_str()
                || k.strip_prefix(probe.as_str())
                    .is_some_and(|rest| rest.starts_with('/'))
        };
        if manuk.keys().any(is_desc) {
            best = acc.clone();
        } else {
            break;
        }
    }
    best
}

/// **Which of the four deltas is the divergence, and how big is it** — `("height", -24)`.
///
/// Pulled out of [`cluster`] because it was written twice inside it (once for the signature, once for
/// `median_mag`) and is now needed a third time by the merge. Two copies of a discriminator is how
/// they drift; three is a promise that they will.
///
/// Size axes beat position axes when they are larger, because a mis-sized box is a fact about the
/// element and a displaced one is a fact about its ancestor's frame — see the `sized_ok` note below.
pub fn dominant_axis(delta: [i64; 4]) -> (&'static str, i64) {
    let [dx, dy, dw, dh] = delta;
    if dw.abs() > dx.abs().max(dy.abs()).max(dh.abs()) {
        ("width", dw)
    } else if dh.abs() > dx.abs().max(dy.abs()) {
        ("height", dh)
    } else if dy.abs() > dx.abs() {
        ("y (vertical drift)", dy)
    } else {
        ("x (horizontal)", dx)
    }
}

/// **THE definition of what a cluster IS** — one divergence in, its root-cause key out.
///
/// ⚠⚠⚠ **This function exists because there were two of these, and the ledger the loop RANKS BY was
/// produced by the poorer one.** [`cluster`] (the in-process `oracle --urls` path) built the full
/// mechanism key — displaced-vs-mis-sized, the axis, the magnitude band. `run_oracle_merge` (the path
/// that reads the crawl's JSONL and WRITES `docs/loop/CLUSTERS.md`) built its own, and its geometry arm
/// was the single line `format!("geometry: <{tag}>")`. So every geometry divergence in the corpus
/// ledger — 1781 sites, 37184 hits — collapsed to twenty tag-named rows that name an HTML ELEMENT
/// rather than a MECHANISM, and the board's #1 priority read `geometry: <div>  14002 hits`: a row that
/// merges a 129px column swap with a 2px line-height residue and cannot be attacked as one cause.
/// The doc comment on `sized_ok` below had already recorded that merging them *"is what let two
/// consecutive ticks read a cause off this ranking and be wrong both times"* — and the ranking it was
/// warning about was still being generated by the code path that had never been fixed.
///
/// One rule, two implementations, and the quiet one publishes the priority ledger. Now there is one.
pub fn signature_of(d: &Divergence) -> String {
    match d.kind.as_str() {
        // The most causal key available: the cascade produced a different KIND of box.
        "display" => format!("display: {} → {}   (<{}>)", d.chrome, d.manuk, d.tag),
        // A whole tag going missing is ONE bug, not N. Keyed by tag, not by element.
        "missing" => format!("missing box: <{}>", d.tag),
        // ⚠ **THE SAME ABSENCE ON A PAGE WHERE OUR MAP IS NOT SMALLER** — see `diff_page`. Ranked
        // separately because it is a different bug: the board's #1 row was a MIXTURE of these two
        // populations and had been ranked as their sum since t684. The wording states the evidence
        // and stops there — `unaligned` is not an exoneration, it is a refusal to call an absence a
        // dropped box when our own count says otherwise.
        "unaligned" => format!("unaligned key (we drew as many): <{}>", d.tag),
        // Geometry is bucketed by which dimension is wrong — a systematic width error and a
        // systematic vertical drift are different bugs with different causes.
        _ => {
            let [_, _, dw, dh] = d.delta;
            // The dominant axis names WHICH dimension is wrong; its magnitude band names HOW wrong,
            // which is what separates a near-miss from a page-height collapse (see `mag_band`).
            let (axis, mag) = dominant_axis(d.delta);
            // ── DISPLACED vs MIS-SIZED are DIFFERENT BUGS and must not share a cluster
            // (tick 554). t553 printed the first instance ever and it read
            // `[434 183 92×17] vs [305 183 92×17]` — identical size, 129px displaced — sitting in the
            // same cluster as `[130 471 18×46] vs [0 459 30×46]`, a 12→30px mis-size. A right-sized
            // box in the wrong place is an ANCESTOR-layout fact (one parent's frame is off and every
            // child inherits it); a wrong-sized box is a sizing fact in the element itself. They have
            // different fixes, and grouping them is what let two consecutive ticks read a cause off
            // this ranking and be wrong both times.
            //
            // The test is the SIZE axes only: a box whose width and height both match within the
            // same tolerance the diff used is correctly sized, wherever it ended up.
            let sized_ok = dw.abs() <= 2 && dh.abs() <= 2;
            let kindword = if sized_ok { "displaced" } else { "mis-sized" };
            format!(
                "geometry/{kindword}: {axis} ~{}px   (<{}>)",
                mag_band(mag),
                d.tag
            )
        }
    }
}

/// **One `"kind":"div"` line of the crawl's JSONL — carrying every field the signature needs.**
///
/// ⚠⚠ **The writer this replaces did not emit `delta` at all.** It wrote `tag`, `dkind`, `chrome`,
/// `manuk` and `id`, and the four deltas — the only field that distinguishes a wrong WIDTH from a
/// wrong HEIGHT from a pure displacement — were dropped on the floor. So `run_oracle_merge` was not
/// *choosing* a coarser key: **the information had already been destroyed at the serialisation
/// boundary**, and no fix to the merge alone could have restored it.
///
/// This is t743's lesson at the other end of the same pipe: *a serialisation boundary is a semantic
/// one.* There, serialising an `<svg>` subtree silently answered a question about reference SCOPE;
/// here, serialising a `Divergence` silently answered *"what is a root cause?"* — and answered it
/// with the tag, because the tag was what survived the write.
pub fn div_to_jsonl(d: &Divergence, class: &str) -> String {
    format!(
        "{{\"kind\":\"div\",\"site\":{},\"class\":{},\"tag\":{},\"dkind\":{},\"chrome\":{},\"manuk\":{},\"id\":{},\"delta\":[{},{},{},{}]}}\n",
        json_str(&d.site),
        json_str(class),
        json_str(&d.tag),
        json_str(&d.kind),
        json_str(&d.chrome),
        json_str(&d.manuk),
        json_str(&d.id),
        d.delta[0],
        d.delta[1],
        d.delta[2],
        d.delta[3],
    )
}

/// Read back exactly what [`div_to_jsonl`] wrote. `None` when the line is not a `div` record **or
/// when a geometry record carries no `delta`.**
///
/// ⚠⚠⚠ **A MISSING DELTA MUST NOT READ AS `[0,0,0,0]`.** That is the whole reason this returns an
/// `Option` instead of defaulting: a geometry divergence with a zeroed delta produces the *perfectly
/// plausible* signature `geometry/displaced: x (horizontal) ~0px`, which is a wrong answer of exactly
/// the right type — the shape this project has now caught six times (`typeof null`, `CSS.supports`,
/// `getEntriesByType`, `root.host`, `composedPath`). Every ledger row would look measured and none of
/// them would be. An old-format crawl (no `delta`) is therefore REFUSED and counted, not silently
/// re-keyed to a fabricated zero.
///
/// `display` and `missing` records carry no meaningful delta and are accepted without one.
pub fn div_from_jsonl(line: &str) -> Option<(Divergence, String)> {
    if !line.contains("\"kind\":\"div\"") {
        return None;
    }
    let kind = json_field(line, "dkind")?;
    let delta = match json_i64_array4(line, "delta") {
        Some(d) => d,
        // A geometry row without its delta is unkeyable. Say so by refusing it.
        None if kind == "geometry" => return None,
        None => [0; 4],
    };
    Some((
        Divergence {
            site: json_field(line, "site")?,
            id: json_field(line, "id")?,
            tag: json_field(line, "tag")?,
            kind,
            chrome: json_field(line, "chrome")?,
            manuk: json_field(line, "manuk")?,
            delta,
        },
        json_field(line, "class").unwrap_or_else(|| "?".into()),
    ))
}

/// Minimal JSON string escaping — the crawl's own output must never be the thing that breaks it.
/// Kept beside its reader so the pair cannot drift; the `chrome`/`manuk` sides carry `[`, `×` and the
/// `{Open Sans/13}` font suffix, and the `id` is a selector path full of `.`, `:` and `/`.
fn json_str(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// Read one `"key":"value"` string field, undoing [`json_str`]'s escaping.
fn json_field(line: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\":\"");
    let i = line.find(&pat)? + pat.len();
    let mut out = String::new();
    let mut it = line[i..].chars();
    while let Some(c) = it.next() {
        match c {
            '"' => return Some(out),
            '\\' => match it.next()? {
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                'u' => {
                    let hex: String = (0..4).filter_map(|_| it.next()).collect();
                    let cp = u32::from_str_radix(&hex, 16).ok()?;
                    out.push(char::from_u32(cp)?);
                }
                esc => out.push(esc),
            },
            c => out.push(c),
        }
    }
    None
}

/// Read one `"key":[a,b,c,d]` field of exactly four signed integers.
fn json_i64_array4(line: &str, key: &str) -> Option<[i64; 4]> {
    let pat = format!("\"{key}\":[");
    let i = line.find(&pat)? + pat.len();
    let end = line[i..].find(']')? + i;
    let mut out = [0i64; 4];
    let mut n = 0;
    for part in line[i..end].split(',') {
        if n == 4 {
            return None;
        }
        out[n] = part.trim().parse().ok()?;
        n += 1;
    }
    if n == 4 {
        Some(out)
    } else {
        None
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
            Vec<i64>,
        ),
    > = BTreeMap::new();

    for d in divs {
        // ONE definition, shared with `run_oracle_merge` — see `signature_of`.
        let sig = signature_of(d);
        let e = acc.entry(sig).or_insert_with(|| {
            (
                d.kind.clone(),
                Default::default(),
                0,
                Vec::new(),
                Vec::new(),
            )
        });
        e.1.insert(d.site.clone());
        e.2 += 1;
        if e.3.len() < 3 {
            e.3.push(format!("{}#{}: {} vs {}", d.site, d.id, d.chrome, d.manuk));
        }
        // Keep the RAW dominant-axis magnitude so the cluster can report what the delta actually was,
        // not only which power-of-two bucket it fell into.
        if d.kind != "display" && d.kind != "missing" {
            e.4.push(dominant_axis(d.delta).1.abs());
        }
    }

    let mut out: Vec<Cluster> = acc
        .into_iter()
        .map(|(signature, (kind, sites, hits, examples, mut mags))| {
            mags.sort_unstable();
            let median_mag = if mags.is_empty() {
                0
            } else {
                mags[mags.len() / 2]
            };
            Cluster {
                signature,
                kind,
                sites: sites.len(),
                hits,
                examples,
                median_mag,
            }
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
    // Delegate to the box-only core so the oracle and the G1 fidelity probe (which carries Box4 maps,
    // not `Seen`) score horizontal overflow through ONE definition — the same one-definition discipline
    // SHAPE uses. Cheap rect-only views; the invariant never reads tag/display.
    let cb: HashMap<&str, [i64; 4]> = chrome.iter().map(|(k, v)| (k.as_str(), v.rect)).collect();
    let mb: HashMap<&str, [i64; 4]> = manuk.iter().map(|(k, v)| (k.as_str(), v.rect)).collect();
    h_overflow_boxes(&cb, &mb, vw, tol)
}

/// The rect-only core of the horizontal-overflow invariant, generic over the key's borrow so both the
/// oracle (`&str` from `Seen` maps) and the G1 fidelity probe (owned `String` keys) call the SAME
/// logic. An element counts only when **Manuk** pushes its right edge past `vw + tol` while **Chrome**
/// renders the *same* element inside the viewport — attributing the spill to us, never to a page that
/// legitimately scrolls sideways. Returns `(ours_only, up-to-3 examples)`.
pub fn h_overflow_boxes<K>(
    chrome: &HashMap<K, [i64; 4]>,
    manuk: &HashMap<K, [i64; 4]>,
    vw: i64,
    tol: i64,
) -> (usize, Vec<String>)
where
    K: std::hash::Hash + Eq + std::fmt::Display,
{
    let edge = |b: &[i64; 4]| b[0] + b[2]; // x + width
    let mut count = 0usize;
    let mut examples: Vec<String> = Vec::new();
    for (id, m) in manuk {
        if edge(m) <= vw + tol {
            continue; // within our own viewport — not overflowing
        }
        // Only OUR fault: Chrome must render the SAME element AND keep it inside the viewport.
        match chrome.get(id) {
            Some(c) if edge(c) <= vw + tol => {
                count += 1;
                if examples.len() < 3 {
                    examples.push(format!("{id} → right {} > vw {vw}", edge(m)));
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
/// is returned so a bounded scan is never mistaken for a clean page. Keys are `tag.SIG:nth-of-type(n)/…` paths, so the
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
    // t1034 diagnostic partition of `count` — see the block below. NOT part of the invariant.
    let (mut zero_area, mut parked, mut onscreen) = (0usize, 0usize, 0usize);
    // ── **t1041: HOW MANY DISTINCT CONTAINERS IS THIS COUNT, AND HOW BIG IS THE BIGGEST?**
    //
    // This invariant counts PAIRS, so one mis-laid row of `n` siblings contributes `n(n-1)/2` all by
    // itself — a 7-anchor footer row is **21**. A site reported at `reading_order 24` may therefore be
    // ONE broken container, not 24 problems, and the number a tick would rank on says nothing about
    // which. `jarring-clean` is TOL 2, so a single broken 3-sibling row already fails it.
    //
    // Report-only, behind the same env var as t1034's partition: the row schema and every banked
    // number are untouched, so no re-baseline is owed.
    let mut bad_groups: Vec<(usize, usize)> = Vec::new(); // (inversions, siblings) per parent
    let mut examples: Vec<String> = Vec::new();
    for (_, ids) in groups {
        if ids.len() < 2 {
            continue;
        }
        if ids.len() > MAX_GROUP {
            skipped += 1;
            continue;
        }
        let group_start = count;
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let co = order(&chrome[ids[i]].rect, &chrome[ids[j]].rect, tol);
                let mo = order(&manuk[ids[i]].rect, &manuk[ids[j]].rect, tol);
                // Both engines must be sure, and they must disagree — that is an inversion we caused.
                if co != 0 && mo != 0 && co != mo {
                    count += 1;
                    // ── **DIAGNOSTIC ONLY (t1034). This does NOT filter — it COUNTS.**
                    //
                    // This invariant is named `jarring`: it claims to measure what a USER
                    // PERCEIVES as out of sequence. It compares every sibling pair by rect with no
                    // notion of whether either box is on the page at all, and t1033's oracle dump
                    // found the shape that matters — a nav dropdown parked at
                    // `x = -199385, 225x0` in Chrome and `x = -199294, 225x0` here. Both engines
                    // agree it is hidden; they disagree by 91px about WHERE off-screen it is, and
                    // that lands in the count as an inversion a user could see.
                    //
                    // **A box with zero area cannot be read, and a box parked entirely left of the
                    // viewport is not in the reading order at all.** Whether that is a large share
                    // of the count or a rounding error decides whether `reading_order` — the
                    // conjunct t1031 proved M1 is gated on — is an engine target or an instrument
                    // property, so it is MEASURED before anything is changed. **Report first,
                    // filter later and deliberately**, so this can never be a threshold tuned to
                    // move a number.
                    let degenerate = |r: &[i64; 4]| r[2] <= 0 || r[3] <= 0;
                    let offscreen = |r: &[i64; 4]| r[0] + r[2] <= 0;
                    let (ca, cb) = (&chrome[ids[i]].rect, &chrome[ids[j]].rect);
                    let (ma, mb) = (&manuk[ids[i]].rect, &manuk[ids[j]].rect);
                    if degenerate(ca) || degenerate(cb) || degenerate(ma) || degenerate(mb) {
                        zero_area += 1;
                    } else if offscreen(ca) || offscreen(cb) || offscreen(ma) || offscreen(mb) {
                        parked += 1;
                    } else {
                        onscreen += 1;
                    }
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
        if count > group_start {
            bad_groups.push((count - group_start, ids.len()));
        }
    }
    examples.sort();
    if count > 0 && std::env::var("MANUK_RO_PARTITION").is_ok() {
        eprintln!(
            "  RO-PARTITION: {count} inversion(s) = {onscreen} on-screen \u{00b7} {zero_area} involve a ZERO-AREA box \u{00b7} {parked} involve a box parked entirely LEFT of the viewport"
        );
        bad_groups.sort_by_key(|g| std::cmp::Reverse(g.0));
        let biggest = bad_groups.first().copied().unwrap_or((0, 0));
        let top3: usize = bad_groups.iter().take(3).map(|g| g.0).sum();
        eprintln!(
            "  RO-GROUPS: {} distinct container(s) \u{00b7} biggest contributes {} of {count} (a {}-sibling group) \u{00b7} top 3 = {top3}",
            bad_groups.len(),
            biggest.0,
            biggest.1
        );
    }
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
            font: String::new(),
        }
    }

    fn seen_font(tag: &str, rect: [i64; 4], font: &str) -> Seen {
        Seen {
            tag: tag.into(),
            display: "block".into(),
            rect,
            font: font.into(),
        }
    }

    /// **A geometry instance must NAME THE FONT on both sides, or a 2px height divergence stays
    /// unattributable.**
    ///
    /// t562's blocked question, made mechanical. `martinfowler.com` reported `[74×16] vs [76×18]` and
    /// the 2px could equally be a different face, a different used size, or a different line-box rule —
    /// three different fixes, indistinguishable in a rect. And an ABSENT font must print as absence: a
    /// `{/0}` would read like a measured zero, which is the kind of fabricated datum this project keeps
    /// catching in its own instruments.
    #[test]
    fn a_geometry_instance_names_the_font_on_both_sides() {
        let mut c = HashMap::new();
        let mut m = HashMap::new();
        c.insert(
            "a".to_string(),
            seen_font("a", [0, 0, 74, 16], "Open Sans/13"),
        );
        m.insert(
            "a".to_string(),
            seen_font("a", [0, 0, 76, 18], "sans-serif/14"),
        );
        let d = diff_page("s.example", &c, &m, 1);
        assert_eq!(
            d.len(),
            1,
            "a 2px height delta beyond tolerance is one divergence"
        );
        assert!(
            d[0].chrome.contains("{Open Sans/13}"),
            "the CHROME side names its face and used size: {}",
            d[0].chrome
        );
        assert!(
            d[0].manuk.contains("{sans-serif/14}"),
            "…and so does OURS — the comparison is the whole diagnostic: {}",
            d[0].manuk
        );

        // No font supplied → the instance says NOTHING about the font, rather than `{/0}`.
        let mut c2 = HashMap::new();
        let mut m2 = HashMap::new();
        c2.insert("a".to_string(), seen("a", [0, 0, 74, 16]));
        m2.insert("a".to_string(), seen("a", [0, 0, 76, 18]));
        let d2 = diff_page("s.example", &c2, &m2, 1);
        assert!(
            !d2[0].chrome.contains('{') && !d2[0].manuk.contains('{'),
            "an ABSENT font prints as absence, never as a fabricated `{{/0}}`: {} / {}",
            d2[0].chrome,
            d2[0].manuk
        );
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

    /// **A cluster must retain THREE instances, because one cannot tell you whether it is homogeneous.**
    ///
    /// t554 left the ranking a real cause list and immediately hit the next limit: `mis-sized: width ~8px
    /// (<a>)` spans three sites and 139 hits, and whether those are TEXT anchors or ICON anchors decides
    /// which subsystem the next tick touches. With three, t555 read them as homogeneous text anchors
    /// (widths off ±9–22px in BOTH directions, height a constant +2px) and the forecast resolved. With
    /// one, t553 read a displacement and concluded the opposite. The printer depends on this cap, so it is
    /// pinned here rather than left as an implementation detail two files apart.
    #[test]
    fn a_cluster_retains_three_instances_so_homogeneity_is_visible() {
        let mk = |site: &str, dw: i64| Divergence {
            site: site.into(),
            id: "body[0]/a[0]".into(),
            kind: "geometry".into(),
            tag: "a".into(),
            chrome: format!("[0 0 {}×30]", 100 + dw),
            manuk: "[0 0 100×32]".into(),
            delta: [0, 0, dw, 2],
        };
        // Four divergences in ONE cluster (same tag, same axis, same band, all mis-sized).
        let c = cluster(&[
            mk("a.example", 9),
            mk("b.example", 11),
            mk("c.example", 12),
            mk("d.example", 10),
        ]);
        assert_eq!(c.len(), 1, "one cause");
        assert_eq!(c[0].hits, 4);
        assert_eq!(
            c[0].examples.len(),
            3,
            "THREE instances retained — one is a door, three are a sample, and the difference is whether \
             a reader can see that the cluster is homogeneous before choosing a subsystem"
        );
        // Every retained instance must be openable in its own right.
        for ex in &c[0].examples {
            assert!(
                ex.contains('#') && ex.contains(" vs "),
                "each is a full comparison: {ex}"
            );
        }
    }

    /// **A right-sized box in the wrong place and a wrong-sized box are DIFFERENT BUGS, and the
    /// signature must say which.**
    ///
    /// t553 printed the first cluster instance ever and found `[434 183 92×17] vs [305 183 92×17]` —
    /// identical size, 129px displaced — sharing a cluster with `[130 471 18×46] vs [0 459 30×46]`, a
    /// 12→30px mis-size. Displacement is an ANCESTOR-layout fact (one parent's frame is off and every
    /// descendant inherits it, so the fix is upstream and fixes many at once); mis-sizing is a fact about
    /// the element itself. Merging them is what let t551 and t552 each read a cause off this ranking and
    /// be wrong in a different direction.
    #[test]
    fn displaced_and_mis_sized_are_different_clusters() {
        let displaced = Divergence {
            site: "a.example".into(),
            id: "body[0]/a[0]".into(),
            kind: "geometry".into(),
            tag: "a".into(),
            chrome: "[434 183 92×17]".into(),
            manuk: "[305 183 92×17]".into(),
            delta: [129, 0, 0, 0], // x moved, size identical
        };
        let mis_sized = Divergence {
            site: "b.example".into(),
            id: "body[0]/a[0]".into(),
            kind: "geometry".into(),
            tag: "a".into(),
            chrome: "[130 471 18×46]".into(),
            manuk: "[0 459 30×46]".into(),
            delta: [130, 12, 12, 0], // and 12px too wide
        };
        let c = cluster(&[displaced.clone(), mis_sized.clone()]);
        assert_eq!(
            c.len(),
            2,
            "a pure displacement and a mis-size must never share a cluster — they have different fixes: \
             {:?}",
            c.iter().map(|x| x.signature.clone()).collect::<Vec<_>>()
        );
        let sigs: Vec<String> = c.iter().map(|x| x.signature.clone()).collect();
        assert!(
            sigs.iter().any(|s| s.contains("geometry/displaced:")),
            "the right-sized one is DISPLACED: {sigs:?}"
        );
        assert!(
            sigs.iter().any(|s| s.contains("geometry/mis-sized:")),
            "…and the other is MIS-SIZED: {sigs:?}"
        );

        // Two displacements of the same tag and band DO still merge — the split must separate causes,
        // not shatter the ranking into one cluster per element.
        let mut d2 = displaced.clone();
        d2.site = "c.example".into();
        let merged = cluster(&[displaced, d2]);
        assert_eq!(merged.len(), 1, "same cause, two sites, one cluster");
        assert_eq!(merged[0].sites, 2);
    }

    /// **A cluster must carry an OPENABLE instance — the signature is a grouping hypothesis, not a
    /// cause, and it has to be possible to look at one.**
    ///
    /// Written at tick 553 after two consecutive wrong inferences drawn from cluster HEADLINES: t551 read
    /// a power-of-two pattern that was the printer's banding, and t552 re-aimed the lead at
    /// text-measurement because `<a>` width was the biggest cluster. The first instance printed
    /// falsified that too — `lobste.rs …/a:nth-of-type(3): [434 183 92×17] vs [305 183 92×17]` is
    /// **identical in size** and 129px displaced, which is an ancestor-layout fact, not a text one, while
    /// the width cases in the same band are 12–30px icon-ish anchors. **One signature, at least two
    /// causes.** The examples field existed the whole time and nothing printed it, so the ledger could
    /// name a cause with no way to open it.
    #[test]
    fn a_cluster_carries_an_openable_instance_with_both_engines_rects() {
        let d = Divergence {
            site: "lobste.rs".into(),
            id: "body:nth-of-type(2)/a:nth-of-type(3)".into(),
            kind: "geometry".into(),
            tag: "a".into(),
            chrome: "[434 183 92×17]".into(),
            manuk: "[305 183 92×17]".into(),
            delta: [129, 0, 0, 0],
        };
        let c = cluster(&[d]);
        assert_eq!(c.len(), 1);
        let ex = c[0]
            .examples
            .first()
            .expect("a cluster must carry at least one instance");
        assert!(
            ex.contains("lobste.rs"),
            "the instance names its SITE: {ex}"
        );
        assert!(
            ex.contains("body:nth-of-type(2)/a:nth-of-type(3)"),
            "…and the selector-path, so it can be opened in a browser: {ex}"
        );
        assert!(
            ex.contains("[434 183 92×17]") && ex.contains("[305 183 92×17]"),
            "…and BOTH engines' rects, because the whole diagnostic value is in the comparison — a \
             pure displacement (same size, different x) and a sizing error are different bugs that the \
             signature groups together: {ex}"
        );
    }

    /// **The band is a POWER OF TWO, and a reader cannot tell that from the signature — so the real
    /// median must travel with it.**
    ///
    /// Written after the t551 sweep: the pooled output read `width ~16px · height ~128px · width ~8px ·
    /// height ~16px · ~64px · ~32px`, and I recorded in the roadmap anchor that the deltas were
    /// "QUANTISED — the signature of ONE systematic box-model delta". `mag_band` rounds DOWN to the
    /// largest power of two, so **every** geometry delta this instrument reports is a power of two by
    /// construction. The pattern I read as evidence was a property of the printer. This test makes the
    /// distinction mechanical: two clusters with the SAME band and very different real medians.
    #[test]
    fn the_band_is_a_power_of_two_so_the_real_median_travels_with_it() {
        // 17, 20 and 31 px all band to 16 — one cluster, and its real median is 20, not 16.
        let same_band = vec![
            geom_div("a.example", "div", [0, 0, 0, 17]),
            geom_div("b.example", "div", [0, 0, 0, 20]),
            geom_div("c.example", "div", [0, 0, 0, 31]),
        ];
        let c = cluster(&same_band);
        assert_eq!(c.len(), 1, "17/20/31 all band to 16 — one cluster");
        assert!(
            c[0].signature.contains("~16px"),
            "the signature reports the BAND: {}",
            c[0].signature
        );
        assert_eq!(
            c[0].median_mag, 20,
            "…and the cluster carries the REAL median (20px), which the band cannot express. Without \
             this, a 17px delta and a 31px delta are indistinguishable in the output and every geometry \
             headline looks like a power of two."
        );

        // A cluster whose deltas are all exactly 16 is a genuinely different fact from the one above,
        // and it must be distinguishable — that is the whole point.
        let truly_16 = vec![
            geom_div("a.example", "div", [0, 0, 0, 16]),
            geom_div("b.example", "div", [0, 0, 0, 16]),
        ];
        assert_eq!(cluster(&truly_16)[0].median_mag, 16);

        // Non-geometry clusters have no magnitude at all — 0, not a fabricated number.
        let missing = vec![Divergence {
            site: "a.example".into(),
            id: "x".into(),
            kind: "missing".into(),
            tag: "div".into(),
            chrome: "box".into(),
            manuk: "absent".into(),
            delta: [0, 0, 0, 0],
        }];
        assert_eq!(
            cluster(&missing)[0].median_mag,
            0,
            "a missing box has no magnitude to report"
        );
    }

    // ⚠⚠⚠ **THIS TEST HAD NO `#[test]` AND HAD NEVER RUN.** Its attribute and the doc block above
    // it were 190 lines further up, stranded before `a_cluster_retains_three_instances…` — a later
    // test inserted between the attribute and its function. The compiler said so, quietly, as a
    // `duplicate_macro_attributes` warning on the function that ended up with two. Reunited at t744.
    //
    // What it asserts is what that tick was about: the magnitude band. So the RED proof for the
    // band was dead in the same subject where the ledger was discarding the band — the datum was
    // lost twice, independently, and neither loss made a sound.
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

    /// **The G1 exit gate's §3b path, and its RED proof: SHAPE-cancelled descendants do NOT amplify a
    /// root cause.** This is the property `run_fidelity_cmd` now depends on — pool every page's
    /// `diff_page` divergences, then `cluster`. Three sites each shift their root `<header>` down (and,
    /// by inheritance, its two `<p>` children) by 23px, 28px, and 1400px. Because `diff_page` scores
    /// SHAPE (parent-relative), the children cancel against the shifted header frame and only the
    /// header itself diverges — each site contributes EXACTLY ONE divergence. So the pool holds 3
    /// divergences (not 9), the near-miss (23/28px) clusters as 2 sites / 2 hits, and the 1400px
    /// collapse is its own 1-site cause.
    ///
    /// If SHAPE cancellation were dropped (absolute diffing) each child would diverge too, the pool
    /// would hold 9, and the near-miss cluster would read 4 hits — this assertion fails. That is the
    /// RED proof that one root cause counts ONCE per site through the whole pool→cluster path the exit
    /// gate walks, never once per inheriting element (FIDELITY-SCORING-REDESIGN.md §3b).
    #[test]
    fn exit_gate_clusters_shape_cancelled_descendants_once_per_site() {
        fn shifted_site(off: i64) -> (HashMap<String, Seen>, HashMap<String, Seen>) {
            let hk = "header.h:nth-of-type(1)".to_string();
            let c1 = format!("{hk}/p.p:nth-of-type(1)");
            let c2 = format!("{hk}/p.p:nth-of-type(2)");
            let mut chrome = HashMap::new();
            chrome.insert(hk.clone(), seen("header", [0, 0, 1000, 80]));
            chrome.insert(c1.clone(), seen("p", [10, 10, 200, 20]));
            chrome.insert(c2.clone(), seen("p", [10, 40, 200, 20]));
            let mut manuk = HashMap::new();
            // The header itself is genuinely shifted; both children INHERIT that same offset, so their
            // shape (box relative to the header) is unchanged — SHAPE must forgive them.
            manuk.insert(hk, seen("header", [0, off, 1000, 80]));
            manuk.insert(c1, seen("p", [10, 10 + off, 200, 20]));
            manuk.insert(c2, seen("p", [10, 40 + off, 200, 20]));
            (chrome, manuk)
        }
        let mut all = Vec::new();
        for (i, off) in [23i64, 28, 1400].into_iter().enumerate() {
            let (c, m) = shifted_site(off);
            all.extend(diff_page(&format!("site{i}"), &c, &m, 8));
        }
        assert_eq!(
            all.len(),
            3,
            "each site's inherited children must CANCEL under SHAPE — one divergence per site (the \
             header), not one per element; got {} divergences",
            all.len()
        );
        let clusters = cluster(&all);
        assert_eq!(clusters.len(), 2, "near-miss vs collapse are two causes");
        assert_eq!(
            clusters[0].sites, 2,
            "the 23/28px near-miss explains 2 sites"
        );
        assert_eq!(
            clusters[0].hits, 2,
            "one header per site — NOT amplified by the two inheriting children (would be 4)"
        );
        assert!(clusters[0].signature.contains("~16px"));
        assert_eq!(clusters[1].sites, 1, "the 1400px collapse is its own cause");
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

    /// **The Box4 core the G1 fidelity probe calls (tick 532), RED-proven.** Same three cases as the
    /// `Seen` test above, on `HashMap<String,[i64;4]>` maps — the shape the fidelity probe carries.
    /// Proves ONE definition scores identically for both callers. RED-PROVE: dropping the
    /// `edge(c) <= vw + tol` guard would let the both-engines-wide case (b) count, flipping 1 → 2.
    #[test]
    fn h_overflow_boxes_scores_the_g1_box_maps_identically() {
        let (vw, tol) = (1200, 8);
        let mut chrome: HashMap<String, [i64; 4]> = HashMap::new();
        let mut manuk: HashMap<String, [i64; 4]> = HashMap::new();
        // (a) OUR fault: Chrome fits, Manuk spills.
        chrome.insert(
            "body:nth-of-type(2)/div:nth-of-type(1)".into(),
            [0, 0, 1200, 50],
        );
        manuk.insert(
            "body:nth-of-type(2)/div:nth-of-type(1)".into(),
            [0, 0, 1400, 50],
        );
        // (b) The SITE is wide: both spill — not our bug.
        chrome.insert(
            "body:nth-of-type(2)/div:nth-of-type(2)".into(),
            [0, 60, 2000, 50],
        );
        manuk.insert(
            "body:nth-of-type(2)/div:nth-of-type(2)".into(),
            [0, 60, 2000, 50],
        );
        // (c) Within tolerance.
        chrome.insert(
            "body:nth-of-type(2)/div:nth-of-type(3)".into(),
            [0, 120, 1200, 50],
        );
        manuk.insert(
            "body:nth-of-type(2)/div:nth-of-type(3)".into(),
            [0, 120, 1205, 50],
        );

        let (count, examples) = h_overflow_boxes(&chrome, &manuk, vw, tol);
        assert_eq!(count, 1, "only our-alone spill counts");
        assert!(examples[0].starts_with("body:nth-of-type(2)/div:nth-of-type(1)"));
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

    fn div(tag: &str, kind: &str, delta: [i64; 4]) -> Divergence {
        Divergence {
            site: "s.example".into(),
            id: "body[0]/div.a1b2:nth-of-type(3)".into(),
            tag: tag.into(),
            kind: kind.into(),
            chrome: "[10 20 300×40]{Open Sans/13}".into(),
            manuk: "[10 20 300×64]{Liberation Sans/13}".into(),
            delta,
        }
    }

    /// ⚠⚠⚠ **THE LEDGER'S KEY MUST SURVIVE THE CRAWL'S SERIALISATION BOUNDARY.**
    ///
    /// This is the whole tick. `oracle --emit` writes each divergence to JSONL and `oracle-merge` reads
    /// it back to build `docs/loop/CLUSTERS.md`; the writer did not emit `delta`, so the mechanism was
    /// destroyed *before* the merge could key on it. Asserting the round-tripped signature is
    /// BYTE-IDENTICAL is the only assertion that can fail if either half regresses.
    #[test]
    fn a_divergences_mechanism_key_survives_the_jsonl_round_trip() {
        // A 24px height error: mis-sized, height axis, band 16.
        let d = div("div", "geometry", [0, 0, 0, 24]);
        let want = signature_of(&d);
        assert_eq!(
            want, "geometry/mis-sized: height ~16px   (<div>)",
            "the mechanism key names size-vs-place, the axis and the band — not the tag"
        );
        let line = div_to_jsonl(&d, "news");
        let (back, class) = div_from_jsonl(&line).expect("a delta-carrying div line parses");
        assert_eq!(class, "news", "the site class rides along");
        assert_eq!(
            signature_of(&back),
            want,
            "the signature after the round trip must be byte-identical to the signature before it"
        );
        // The diagnostic payload has to survive too: the `{face/px}` suffix is what makes a 2px
        // height delta attributable at all (t562), and the selector path is what `boxes --why` takes.
        assert_eq!(
            back.chrome, d.chrome,
            "the Chrome side keeps its font suffix"
        );
        assert_eq!(back.manuk, d.manuk, "and so does ours");
        assert_eq!(
            back.id, d.id,
            "the selector path is the handle for the next question"
        );
        assert_eq!(back.delta, d.delta, "and the four deltas are the mechanism");
    }

    /// **A MISSING DELTA IS NOT A ZERO DELTA.** The available quiet failure here is to default the
    /// delta and emit `geometry/displaced: x (horizontal) ~0px` — a wrong answer of exactly the right
    /// type, and every row would look measured. An old-format line is refused instead.
    #[test]
    fn a_geometry_record_without_a_delta_is_refused_not_zeroed() {
        let pre_t744 = "{\"kind\":\"div\",\"site\":\"s.example\",\"class\":\"news\",\
                        \"tag\":\"div\",\"dkind\":\"geometry\",\"chrome\":\"[10 20 300x40]\",\
                        \"manuk\":\"[10 20 300x64]\",\"id\":\"body[0]\"}";
        assert!(
            div_from_jsonl(pre_t744).is_none(),
            "a geometry record with no `delta` is UNKEYABLE and must be refused, never re-keyed to \
             a fabricated [0,0,0,0] that prints as a measured ~0px row"
        );
        // But `missing` and `display` records genuinely have no delta, and must still parse — they are
        // 401 sites' worth of the ledger and are keyed by tag by definition.
        let miss = pre_t744.replace("\"dkind\":\"geometry\"", "\"dkind\":\"missing\"");
        let (d, _) = div_from_jsonl(&miss).expect("a missing-box record needs no delta");
        assert_eq!(signature_of(&d), "missing box: <div>");
    }

    /// **G_UNALIGNED_KEY_IS_NOT_A_MISSING_BOX — an absence is only evidence of a dropped box when
    /// OUR MAP IS SMALLER (t912).**
    ///
    /// `manuk.get(id) == None` has three causes: the node is absent from our DOM, the node exists
    /// with no box, or **the node exists WITH a box under a different path** — `nth-of-type` is
    /// absolute, so one inserted sibling re-numbers every key beneath it (t780-783). All three were
    /// ranked as *"Chrome renders it, we render nothing"*, which is the row the board has had at #1
    /// since t684.
    ///
    /// Measured at t911 over the banked t909 sweep, from counts the instrument was already printing:
    /// **of the 58 sites carrying a missing-`<div>` count, 22 render AS MANY OR MORE box-bearing
    /// paths than Chrome** — `a1.ro` draws 676 against Chrome's 696 and shares sixteen.
    ///
    /// This is t782's correction one level out: `TreeDivergence` was split from `ThinOverlap` after
    /// measuring *"the one thing this variant never looked at: our own element count"*, and that fix
    /// reached the UNSCORED path only. A scoring site kept feeding raw `missing` divergences into
    /// the ranked cause list.
    ///
    /// ⚠ **BOTH DIRECTIONS ARE ASSERTED, and the second is the one that keeps this honest**: on a
    /// page where we genuinely drew fewer boxes, the absence must STILL be `missing`. A change that
    /// relabelled every absence would empty the board's top row and look like progress.
    #[test]
    fn an_absence_is_only_a_missing_box_when_our_map_is_smaller() {
        let seen = |tag: &str| Seen {
            tag: tag.into(),
            display: "block".into(),
            rect: [0, 0, 10, 10],
            font: String::new(),
        };
        let chrome: HashMap<String, Seen> = (0..4)
            .map(|i| (format!("body[0]/div[{i}]"), seen("div")))
            .collect();

        // ── WE DREW FEWER: one box against Chrome's four. The absences are real evidence.
        let thin: HashMap<String, Seen> =
            std::iter::once(("body[0]/div[0]".to_string(), seen("div"))).collect();
        let d = diff_page("t", &chrome, &thin, 8);
        assert_eq!(
            d.len(),
            3,
            "three of Chrome's four keys are absent from ours"
        );
        assert!(
            d.iter().all(|x| x.kind == "missing"),
            "our map is SMALLER, so an absent key is evidence of a dropped box — got {:?}",
            d.iter().map(|x| x.kind.clone()).collect::<Vec<_>>()
        );
        assert_eq!(signature_of(&d[0]), "missing box: <div>");

        // ── WE DREW AS MANY, under different keys. Same absences, different bug.
        let shifted: HashMap<String, Seen> = (1..5)
            .map(|i| (format!("body[0]/div[{i}]"), seen("div")))
            .collect();
        let d = diff_page("t", &chrome, &shifted, 8);
        assert_eq!(
            d.len(),
            1,
            "only `div[0]` is absent once the tree is shifted by one"
        );
        assert!(
            d.iter().all(|x| x.kind == "unaligned"),
            "our map is NOT smaller, so an absent key is NOT evidence of a dropped box — got {:?}",
            d.iter().map(|x| x.kind.clone()).collect::<Vec<_>>()
        );
        assert_eq!(
            signature_of(&d[0]),
            "unaligned key (we drew as many): <div>",
            "and it must rank on its own row — the board's #1 was the SUM of these two populations"
        );

        // ── THE BOUNDARY, stated because `>=` and `>` disagree here and the data does not say which
        // is right: an equal-sized map is NOT smaller, so it takes the `unaligned` reading.
        let equal: HashMap<String, Seen> = (10..14)
            .map(|i| (format!("body[0]/div[{i}]"), seen("div")))
            .collect();
        assert!(
            diff_page("t", &chrome, &equal, 8)
                .iter()
                .all(|x| x.kind == "unaligned"),
            "an EQUAL count is not a smaller one"
        );
    }

    /// **DISPLACED and MIS-SIZED must not share a row, and the tag-only key merged them.** The old
    /// merge keyed both of these as `geometry: <div>`; one is an ancestor-frame bug and one is a
    /// sizing bug, and they have different fixes.
    #[test]
    fn the_ledger_splits_displaced_from_mis_sized_and_by_axis() {
        let divs = vec![
            div("div", "geometry", [129, 0, 0, 0]), // right size, 129px to the side
            div("div", "geometry", [0, 0, 18, 0]),  // 18px too wide
            div("div", "geometry", [0, 0, 0, 24]),  // 24px too tall
            div("div", "geometry", [0, 31, 0, 0]),  // dragged down 31px
        ];
        let cs = cluster(&divs);
        assert_eq!(
            cs.len(),
            4,
            "four different mechanisms on ONE tag are four rows, not one `geometry: <div>`: {:?}",
            cs.iter().map(|c| &c.signature).collect::<Vec<_>>()
        );
        let sigs: Vec<&str> = cs.iter().map(|c| c.signature.as_str()).collect();
        assert!(sigs.contains(&"geometry/displaced: x (horizontal) ~128px   (<div>)"));
        assert!(sigs.contains(&"geometry/mis-sized: width ~16px   (<div>)"));
        assert!(sigs.contains(&"geometry/mis-sized: height ~16px   (<div>)"));
        assert!(sigs.contains(&"geometry/displaced: y (vertical drift) ~16px   (<div>)"));
        // And the hits are CONSERVED — a re-key that invents or loses rows is worse than a coarse one.
        assert_eq!(
            cs.iter().map(|c| c.hits).sum::<usize>(),
            divs.len(),
            "every divergence lands in exactly one row"
        );
        // The REAL median rides beside the band, because the band is a printer artifact (t552).
        let tall = cs
            .iter()
            .find(|c| c.signature.contains("mis-sized: height"))
            .unwrap();
        assert_eq!(
            tall.median_mag, 24,
            "the band says ~16px; the delta was 24px"
        );
    }

    /// `cluster()` (in-process) and the crawl→merge path must agree, element for element. They are the
    /// two implementations this tick collapsed into one; this is the assertion that keeps them one.
    #[test]
    fn the_in_process_and_serialised_paths_agree_on_every_row() {
        let divs = vec![
            div("div", "geometry", [129, 0, 0, 0]),
            div("span", "geometry", [0, 0, 0, 24]),
            div("a", "missing", [0; 4]),
        ];
        let direct: Vec<String> = divs.iter().map(signature_of).collect();
        let via_jsonl: Vec<String> = divs
            .iter()
            .map(|d| div_to_jsonl(d, "news"))
            .map(|l| signature_of(&div_from_jsonl(&l).expect("round trip").0))
            .collect();
        assert_eq!(direct, via_jsonl, "one rule, ONE implementation");
    }

    /// The JSONL is written by `format!`, so a `"` or a `\` in a selector path or a font name is a
    /// broken line — and a broken line is a silently dropped divergence, which reads as a fixed bug.
    #[test]
    fn a_quote_or_backslash_in_a_field_does_not_break_the_line() {
        let mut d = div("div", "geometry", [0, 0, 0, 24]);
        d.id = "body[0]/div[a=\"x\"]\\/y:nth-of-type(2)".into();
        d.chrome = "[0 0 1×1]{\"Weird\\Face\"/13}".into();
        let line = div_to_jsonl(&d, "news");
        assert_eq!(line.matches('\n').count(), 1, "exactly one record per line");
        let (back, _) = div_from_jsonl(&line).expect("escaped fields still parse");
        assert_eq!(back.id, d.id);
        assert_eq!(back.chrome, d.chrome);
    }
}
