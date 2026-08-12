//! **G_GET_ELEMENT_BY_ID_INDEX — `getElementById` must not be a document SCAN, because
//! `window.<id>` calls it on every access.**
//!
//! ⚠⚠⚠ **THE COST WAS NOT WHERE THREE TICKS OF NOTES SAID IT WAS.** `css/selectors`'
//! `invalidation/has-complexity.html` had been Bar 0 for ~100 ticks — first a CRASH (fixed at t1164,
//! an unrooted reflector), then a HANG, which was attributed to a quadratic *recascade*. It is
//! neither. `document.getElementById` was `descendants(root).find(...)` — **O(document) per call** —
//! and HTML §7.3.3 named access publishes `window.container` for `<div id=container>` as a **getter
//! that calls it on every access**. So this, the shape of every list build on the web:
//!
//! ```js
//! for (let i = 0; i < n; i++) container.appendChild(document.createElement('span'));
//! ```
//!
//! is **quadratic in document size** — not because of `appendChild`, but because the bare
//! identifier `container` is a full document scan, once per iteration.
//!
//! ## The ladder that localised it (t1165) — every row a control for the row above
//!
//! ```text
//!                                                              BEFORE      AFTER
//!   A. N connected appends, empty doc      N=2000                104 ms      46 ms
//!                                          N=16000              4075 ms     299 ms   (3.8x/doubling -> ~linear)
//!   B. the SAME 2000 appends, after M      M=0                   117 ms      30 ms
//!      pre-existing nodes                  M=4000               3648 ms      28 ms
//!                                          M=16000             14029 ms      32 ms   (linear in M -> FLAT)
//!   C. same appends, parent DETACHED       N=16000  CONTROL      186 ms     209 ms   (never used the id path)
//!   D. createElement only, M=16000         CONTROL                 8 ms       8 ms   (flat before and after)
//!      appendChild of PREALLOCATED nodes   M=16000             14976 ms      21 ms
//!   E. bare `container` vs hoisted local   M=16000             14018 ms      27 ms
//!      `var c = container` (same loop!)    M=16000  CONTROL       14 ms      15 ms
//! ```
//!
//! ⚠⚠⚠ **ROW E IS THE PROOF, AND IT NEEDED NO ENGINE CHANGE TO MAKE IT.** Identical appends,
//! identical document, identical everything — the only difference is whether the parent is reached
//! through the bare identifier or through a local hoisted out of the loop. **14018 ms vs 14 ms, a
//! 1000× difference.** Rows C and D are what ruled out `appendChild` and `createElement`
//! respectively: a detached parent and a bare `createElement` loop were flat all along, because
//! neither touches a named global.
//!
//! ## The fix, and why it cannot change an answer
//!
//! `Dom::id_index` maps `id → Vec<NodeId>`, populated in `set_attr` (every id in the engine arrives
//! there — the HTML parser routes through it too). It is **explicitly allowed to be stale**: entries
//! are never eagerly removed, so it may name nodes whose id changed, that were detached, or that
//! live in another tree. `Dom::get_element_by_id` therefore **verifies every candidate against the
//! live tree** and **falls back to the original full scan** whenever it cannot produce a unique
//! verified answer (including two verified candidates — duplicate ids are legal, and the spec wants
//! the first in TREE order, which an insertion-ordered index cannot answer). The index can only make
//! the lookup faster, never different: its worst case is the behaviour it replaced.
//!
//! ⚠⚠⚠ **THE FIRST VERSION OF THAT PREDICATE WAS WRONG, AND ONE WPT TEST CAUGHT IT.** Verification
//! used `is_inclusive_ancestor`, which walks `parent()` — and `parent()` **crosses the shadow
//! boundary** while `descendants()`, which seeds from `children()`, does not. So an element moved
//! INTO a shadow root still verified as a descendant of the document, `document.getElementById` kept
//! finding it, and `window.target2` stayed defined where the spec requires `undefined`.
//! `dom/nodes/moveBefore/moveBefore-id-map.html` went **4/4 → 3/4** — the whole `dom` area moved by
//! exactly **−1**, which is the ratchet doing its job on a change whose headline was a 1000× win.
//! The lesson is narrower than "be careful": **a fast path must be predicate-IDENTICAL to the slow
//! path it stands in for, not merely close to it.** Hence `Dom::light_tree_contains`.
//!
//! ## Measured, same-hour old-binary control
//!
//! ```text
//!   WPT dom             4004/7193  ->  4004/7193   (=)   [4003 with the wrong predicate]
//!   WPT html/dom       56438/59922 -> 56440/59922  (+2)
//!   WPT css/selectors   2905/5215  ->  2912/5222   (+7)  HANG/CRASH 1 -> 0
//!   css/selectors/invalidation/has-complexity.html: Bar 0 CLOSED
//! ```
//!
//! ## How this goes RED
//!
//! Replace `Dom::get_element_by_id`'s body with the fallback scan alone → the scaling assertion
//! below fails (the ratio goes from ~1 to ~30). Swap `light_tree_contains` back to
//! `is_inclusive_ancestor` → the shadow-boundary assertion fails.

use manuk_text::FontContext;

/// `loop(M=8000) / loop(M=0)` for the SAME 2,000 appends. A **ratio**, not a millisecond budget, for
/// the reason `g_hot_dom_no_compile` spells out: an absolute floor measures the CPU as much as the
/// code. RED-proven by severing the index: **70.5x** in this crate's own build, against ~1.0x with
/// it. 6.0 leaves generous headroom on a loaded box while still being an order of magnitude below
/// the defect.
const SCALING_LIMIT: f64 = 6.0;

fn load(body: &str) -> String {
    let html = format!(
        r##"<!doctype html><html><body style="margin:0">
<div id="ballast"></div><div id="container"></div><div id="target"></div>
<div id="out">-</div>
<script>
var out = [];
try {{ {body} }} catch (e) {{ out.push('THREW:' + e); }}
document.getElementById('out').textContent = out.join(' ');
</script></body></html>"##
    );
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(&html, "https://idindex.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    page.dom().text_content(out)
}

fn ms(out: &str, key: &str) -> f64 {
    out.split_whitespace()
        .find_map(|t| t.strip_prefix(key))
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or_else(|| panic!("G_GET_ELEMENT_BY_ID_INDEX: no `{key}` in output: {out}"))
}

#[test]
fn get_element_by_id_is_not_a_document_scan() {
    // ── CORRECTNESS FIRST. A fast lookup that returns the wrong element would satisfy every timing
    //    assertion below, and the shadow-boundary row is the one this tick actually got wrong once.
    let c = load(
        "var host = document.createElement('div'); document.body.appendChild(host); \
         var sr = host.attachShadow({mode:'open'}); \
         out.push('plain:' + (document.getElementById('target') === target)); \
         var dup = document.createElement('div'); dup.id = 'target'; \
         document.body.appendChild(dup); \
         out.push('dupFirstInTreeOrder:' + (document.getElementById('target') === target)); \
         var inner = document.createElement('span'); inner.id = 'shadowed'; sr.appendChild(inner); \
         out.push('shadowScoped:' + (sr.getElementById('shadowed') === inner)); \
         out.push('shadowHidden:' + (document.getElementById('shadowed') === null)); \
         out.push('windowNamed:' + (window.shadowed === undefined)); \
         var gone = document.createElement('div'); gone.id = 'gone'; \
         document.body.appendChild(gone); gone.remove(); \
         out.push('detachedNotFound:' + (document.getElementById('gone') === null)); \
         var re = document.getElementById('container'); re.id = 'renamed'; \
         out.push('renamedOld:' + (document.getElementById('container') === null)); \
         out.push('renamedNew:' + (document.getElementById('renamed') === re));",
    );
    for claim in [
        "plain:true",
        // Duplicate ids are legal and the spec wants the FIRST in tree order — the case the index
        // deliberately refuses to answer, falling back to the scan.
        "dupFirstInTreeOrder:true",
        "shadowScoped:true",
        // ⚠ The two rows that went RED when verification used `is_inclusive_ancestor`: `parent()`
        // crosses the shadow boundary and `descendants()` does not.
        "shadowHidden:true",
        "windowNamed:true",
        // A stale index entry must not resurrect a removed or renamed element.
        "detachedNotFound:true",
        "renamedOld:true",
        "renamedNew:true",
    ] {
        assert!(
            c.contains(claim),
            "G_GET_ELEMENT_BY_ID_INDEX: expected `{claim}`\n  got: {c}\n\n  \
             The id index is a HINT: every candidate must be verified against the live tree, and \
             anything it cannot answer uniquely must fall back to the full scan. A wrong answer \
             here means the fast path is not predicate-identical to the scan it replaced."
        );
    }

    // ── THEN COST. The same 2,000 appends, with and without 8,000 unrelated nodes present. The
    //    ballast is built through a DocumentFragment and committed in ONE insertion so its own cost
    //    is not inside the measured loop (the first draft of this measurement put it there).
    let loop_ms = |m: usize| -> f64 {
        let out = load(&format!(
            "var f=document.createDocumentFragment(); \
             for(var i=0;i<{m};i++) f.appendChild(document.createElement('b')); ballast.appendChild(f); \
             var pre=[]; for(var j=0;j<2000;j++) pre.push(document.createElement('span')); \
             var t=Date.now(); for(var j2=0;j2<2000;j2++) container.appendChild(pre[j2]); \
             out.push('loop:'+(Date.now()-t));"
        ));
        ms(&out, "loop:").max(1.0)
    };
    let base = loop_ms(0);
    let big = loop_ms(8000);
    let ratio = big / base;
    assert!(
        ratio <= SCALING_LIMIT,
        "G_GET_ELEMENT_BY_ID_INDEX: the SAME 2000 appends took {base}ms in an empty document and \
         {big}ms with 8000 unrelated nodes present — a {ratio:.1}x scaling factor against a limit \
         of {SCALING_LIMIT}.\n\n  \
         `container` is a bare identifier, so every iteration resolves `window.container` through \
         `document.getElementById`. If that is a document scan again, this loop — which is how every \
         list, feed, table and virtualised scroller on the web is built — is quadratic in page size. \
         Measured before the id index: 117ms vs 3648ms at M=4000, and 14029ms at M=16000."
    );
}
