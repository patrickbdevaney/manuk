//! PERF.1.2.b — the dirty-set planning shared by the incremental restyle of both cascades
//! (`MinimalCascade::cascade_incremental` and the Stylo one in `stylo_engine`). See the long note in
//! `stylo_engine.rs` for why the plan is computed from a fingerprint of the `Dom` and not from the
//! dirty bits.

use crate::Stylesheet;
use manuk_dom::{Dom, NodeData, NodeId};

pub(crate) fn env_on(name: &str) -> bool {
    std::env::var(name).is_ok_and(|v| !v.is_empty() && v != "0")
}

#[inline]
fn fnv(h: &mut u64, bytes: &[u8]) {
    for &b in bytes {
        *h ^= b as u64;
        *h = h.wrapping_mul(0x100_0000_01b3);
    }
    // a separator so ("ab","c") and ("a","bc") differ
    *h ^= 0xff;
    *h = h.wrapping_mul(0x100_0000_01b3);
}

pub(crate) fn incr_key(dom: &Dom, sheets: &[Stylesheet], vw: f32, vh: f32) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    fnv(&mut h, &vw.to_bits().to_le_bytes());
    fnv(&mut h, &vh.to_bits().to_le_bytes());
    fnv(&mut h, &[dom.quirks() as u8]);
    fnv(&mut h, &(sheets.len() as u64).to_le_bytes());
    for sh in sheets {
        fnv(&mut h, sh.source().as_bytes());
    }
    h
}

/// Does the author CSS contain selectors whose match depends on a node's SIBLINGS (combinators `+` `~`
/// or structural pseudo-classes), and ones that depend on whether it has children (`:empty`)?
/// Textual and deliberately conservative: only the prelude of a rule (text that ends in `{`) is
/// searched for `+`/`~`, so `calc(1px + 2px)` in a declaration does not count.
pub(crate) fn incr_sheet_flags(sheets: &[Stylesheet]) -> (bool, bool) {
    let (mut sib, mut empty) = (false, false);
    for sh in sheets {
        let src = sh.source().to_ascii_lowercase();
        for t in [
            ":nth-",
            ":first-child",
            ":last-child",
            ":only-child",
            ":first-of-type",
            ":last-of-type",
            ":only-of-type",
        ] {
            if src.contains(t) {
                sib = true;
            }
        }
        if src.contains(":empty") {
            empty = true;
        }
        if !sib {
            let b = src.as_bytes();
            let mut seg_start = 0usize;
            let mut in_comment = false;
            let mut k = 0usize;
            while k < b.len() {
                if in_comment {
                    if b[k] == b'*' && b.get(k + 1) == Some(&b'/') {
                        in_comment = false;
                        k += 1;
                        seg_start = k + 1;
                    }
                } else if b[k] == b'/' && b.get(k + 1) == Some(&b'*') {
                    in_comment = true;
                    k += 1;
                } else if matches!(b[k], b';' | b'}') {
                    seg_start = k + 1;
                } else if b[k] == b'{' {
                    let prelude = &b[seg_start..k];
                    let at = prelude.iter().find(|c| !c.is_ascii_whitespace()) == Some(&b'@');
                    if !at {
                        for (q, &c) in prelude.iter().enumerate() {
                            if (c == b'+' || c == b'~') && prelude.get(q + 1) != Some(&b'=') {
                                sib = true;
                            }
                        }
                    }
                    seg_start = k + 1;
                }
                k += 1;
            }
        }
        if sib && empty {
            break;
        }
    }
    (sib, empty)
}

pub(crate) fn incr_fingerprints(dom: &Dom, sib: bool, empty: bool) -> Vec<(NodeId, u64)> {
    let nodes = dom.flat_descendants(dom.root());
    let mut out = Vec::with_capacity(nodes.len());
    for n in nodes {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        fnv(&mut h, &dom.parent(n).map_or(0, |p| p.0).to_le_bytes());
        match dom.data(n) {
            NodeData::Element(el) => {
                fnv(&mut h, b"E");
                fnv(&mut h, el.name.as_bytes());
                if let Some(ns) = &el.namespace {
                    fnv(&mut h, ns.as_bytes());
                }
                for a in &el.attrs {
                    fnv(&mut h, a.name.as_bytes());
                    fnv(&mut h, a.value.as_bytes());
                }
                let st = (dom.is_hovered(n) as u8)
                    | (dom.is_focused(n) as u8) << 1
                    | (dom.is_focus_within(n) as u8) << 2
                    | (dom.is_focus_visible(n) as u8) << 3
                    | (dom.is_active(n) as u8) << 4;
                fnv(&mut h, &[st]);
                if empty {
                    fnv(&mut h, &[dom.first_child(n).is_some() as u8]);
                }
            }
            NodeData::Text(t) => {
                fnv(&mut h, b"T");
                if empty {
                    fnv(&mut h, &[!t.trim().is_empty() as u8]);
                }
            }
            _ => fnv(&mut h, b"O"),
        }
        if sib {
            fnv(
                &mut h,
                &dom.prev_sibling(n).map_or(0, |p| p.0).to_le_bytes(),
            );
            fnv(
                &mut h,
                &dom.next_sibling(n).map_or(0, |p| p.0).to_le_bytes(),
            );
        }
        out.push((n, h));
    }
    out
}

/// The subtree roots to re-walk and the nodes that left the document, or `None` = do the full cascade.
pub(crate) fn incr_plan(
    cache_fp: &std::collections::HashMap<NodeId, u64>,
    cache_sib: bool,
    dom: &Dom,
    new_fp: &[(NodeId, u64)],
) -> Option<(Vec<NodeId>, Vec<NodeId>)> {
    let mut changed: std::collections::HashSet<NodeId> = std::collections::HashSet::new();
    for (n, h) in new_fp {
        if cache_fp.get(n) != Some(h) {
            changed.insert(*n);
        }
    }
    let live: std::collections::HashSet<NodeId> = new_fp.iter().map(|(n, _)| *n).collect();
    let removed: Vec<NodeId> = cache_fp
        .keys()
        .copied()
        .filter(|n| !live.contains(n))
        .collect();
    if cache_sib {
        for c in changed.clone() {
            if let Some(p) = dom.parent(c) {
                for s in dom.children(p) {
                    changed.insert(s);
                }
            }
        }
    }
    if changed.len() * 3 > new_fp.len().max(3) {
        return None;
    }
    // `<html>` itself moved -> the root font size (every `rem`) may have moved with it.
    if let Some(de) = dom.children(dom.root()).find(|&c| dom.is_element(c)) {
        if changed.contains(&de) {
            return None;
        }
    }
    let mut roots = Vec::new();
    for &c in &changed {
        let mut cur = dom.parent(c);
        let mut covered = false;
        while let Some(p) = cur {
            if changed.contains(&p) {
                covered = true;
                break;
            }
            cur = dom.parent(p);
        }
        if !covered {
            roots.push(c);
        }
    }
    Some((roots, removed))
}

pub(crate) static INCR_PASSES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub(crate) static INCR_FULL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub(crate) static INCR_RESTYLED: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);
pub(crate) static INCR_MISMATCH: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// `(incremental passes, full passes, nodes restyled by incremental passes, verify mismatches)`.
pub fn incr_stats() -> (u64, u64, u64, u64) {
    use std::sync::atomic::Ordering::Relaxed;
    (
        INCR_PASSES.load(Relaxed),
        INCR_FULL.load(Relaxed),
        INCR_RESTYLED.load(Relaxed),
        INCR_MISMATCH.load(Relaxed),
    )
}

/// The incremental MinimalCascade (the agent's cascade) must give, after every kind of script mutation,
/// exactly the style map the full cascade gives; the full arm is a fresh thread (the cache is
/// thread-local, so an empty cache can only do the full walk).
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MinimalCascade, StyleMap};

    const HTML: &str = "<html><head><style>\
        .a .b { color: red } .open > p { margin-left: 7px } li + li { padding-top: 3px }\
        li:nth-child(odd) { font-weight: 700 } li:last-child { color: blue } p:empty { display: none }\
        div { opacity: .5 }\
        </style></head><body>\
        <div id=m class=a><span class=b>x</span><span id=s>y</span></div>\
        <ul id=l><li>1</li><li>2</li><li>3</li></ul>\
        <div id=o><p>t</p><p></p></div></body></html>";

    fn by_id(dom: &Dom, id: &str) -> NodeId {
        dom.descendants(dom.root())
            .find(|&n| dom.element(n).and_then(|e| e.id()) == Some(id))
            .unwrap()
    }

    fn full_fresh(dom: &Dom, sheets: &[Stylesheet]) -> StyleMap {
        std::thread::scope(|s| {
            s.spawn(|| MinimalCascade.cascade_incremental(dom, sheets, 800.0, 600.0))
                .join()
                .unwrap()
        })
    }

    /// Returns the number of nodes the incremental pass restyled.
    fn check(dom: &Dom, sheets: &[Stylesheet], what: &str) -> u64 {
        let before = incr_stats();
        let inc = MinimalCascade.cascade_incremental(dom, sheets, 800.0, 600.0);
        let after = incr_stats();
        let full = full_fresh(dom, sheets);
        assert_eq!(inc.len(), full.len(), "{what}: node count");
        for (n, cs) in &full {
            assert!(
                inc.get(n) == Some(cs),
                "{what}: node {:?} <{}> differs from the full cascade",
                n,
                dom.tag_name(*n).unwrap_or("#text")
            );
        }
        assert_eq!(after.0, before.0 + 1, "{what}: pass was not incremental");
        after.2 - before.2
    }

    #[test]
    fn incremental_minimal_cascade_equals_full_after_mutations() {
        let mut dom = manuk_html::parse(HTML);
        let sheets = MinimalCascade::collect_style_elements(&dom);
        let _ = MinimalCascade.cascade_incremental(&dom, &sheets, 800.0, 600.0); // full, primes
        let total = dom.flat_descendants(dom.root()).len() as u64;

        let m = by_id(&dom, "m");
        dom.set_attr(m, "class", "");
        check(&dom, &sheets, "class removed (descendant selector)");
        dom.set_attr(m, "class", "a");
        check(&dom, &sheets, "class restored");
        let o = by_id(&dom, "o");
        dom.set_attr(o, "class", "open");
        check(&dom, &sheets, "class added (child combinator)");
        let l = by_id(&dom, "l");
        let li = dom.create_element("li");
        let t = dom.create_text("4");
        dom.append_child(li, t);
        dom.append_child(l, li);
        check(&dom, &sheets, "li appended (sibling + nth + last-child)");
        let first = dom.first_child(l).unwrap();
        dom.detach(first);
        check(&dom, &sheets, "first li removed (nth-child parity flips)");
        let p2 = dom.children(o).nth(1).unwrap();
        let tx = dom.create_text("now not empty");
        dom.append_child(p2, tx);
        check(&dom, &sheets, "text added to an empty p (:empty)");
        let s = by_id(&dom, "s");
        dom.set_attr(s, "style", "color: green; opacity: .25");
        let walked = check(&dom, &sheets, "inline style on a leaf");
        assert!(
            walked * 2 < total,
            "a one-leaf change restyled {walked} of {total} nodes"
        );
    }
}
