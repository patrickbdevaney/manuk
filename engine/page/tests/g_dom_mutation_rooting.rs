//! **G_DOM_MUTATION_ROOTING — `appendChild` / `insertBefore` / `removeChild` must ROOT the node
//! object they are about to return, because `record_mutation` ALLOCATES.**
//!
//! ⚠⚠⚠ **BAR 0, AND IT WAS NOT WHAT ITS ONE KNOWN SYMPTOM SAID IT WAS.** The whole-suite runner had
//! been reporting one `CRASH (killed by a signal)` in `css/selectors`, on
//! `invalidation/has-complexity.html` — a test whose title is *":has() invalidation should not be
//! O(n^2)"* — and three ticks' worth of notes attributed it to the `:has()` cascade, then to a
//! quadratic recascade, then to *"the JS↔DOM binding surface at 75,000 elements"*. A nine-cell
//! probe ladder (t1164) says the crash has **nothing to do with any of those**:
//!
//! ```text
//!                                                        SEGV rate (release, run ALONE)
//!   25,000 appendChild, plain stylesheet, no getCS   CONTROL     2/4      ← no :has() at all
//!   25,000 appendChild, plain stylesheet, getCS      CONTROL     1/4
//!    1,000 appendChild + :has() rules                            0/4      ← only 1,000 elements
//!    5,000 appendChild + :has() rules                            1/4
//!   10,000 appendChild + :has() rules                            3/4
//!   25,000 appendChild + :has() rules                            2/4
//!   25,000 appendChild + :has() rules, no getCS                  4/4
//!   10,000 createElement, NEVER APPENDED             CONTROL     0/4      ← the negative control
//!   static page, no createElement at all             CONTROL     0/4      ← the negative control
//! ```
//!
//! **The two negative controls are the finding.** Ten thousand `createElement` calls that are never
//! appended never crash; a static page never crashes; and *every* row that calls `appendChild` in a
//! loop crashes, at rates that rise with the count and are independent of `:has()`, of
//! `getComputedStyle`, and of page size. It is not a complexity bug in a complexity test. It is
//! **`appendChild`**, and a page appending a thousand script-created elements is every SPA, every
//! framework render and every list build on the web.
//!
//! ## The mechanism, and the rule this file states in one place and broke in three
//!
//! `el_append_child` read the child's reflector out of the argument vector as a raw
//! `*mut JSObject`, then called `record_mutation`, then stored that pointer as the return value.
//! `record_mutation` is **not** a no-op when nothing is observing: it calls `new_reflector` for the
//! target and for every added or removed node, and builds jsvals for the call — allocations, any of
//! which can trigger a **moving** GC. The pointer held across it is then stale.
//!
//! `new_reflector`'s own body has said so since it was written:
//!
//! > ROOT THE CACHE IMMEDIATELY. A raw `*mut JSObject` held across ANY allocation is a dangling
//! > pointer waiting to happen … a bug that Rust's type system cannot see, because to it a
//! > `*mut JSObject` is just a number.
//!
//! The same file then did exactly that in **three** bindings — `appendChild`, `insertBefore`,
//! `removeChild` — which are the three most-used DOM mutation methods there are. All three are
//! fixed in one tick, because *one rule / N implementations* is the shape this repo has paid for at
//! t720, t1027, t1131 and t1134. (`moveBefore` returns `undefined` and was never exposed.)
//!
//! ## Measured
//!
//! ```text
//!   WPT probe, release, the two worst cells, 8 runs each   BEFORE 8/16 SEGV   AFTER 0/16
//!   css/selectors/invalidation/has-complexity.html         BEFORE CRASH       AFTER HANG
//!   WPT dom              4004/7193 before, 4004/7193 after   (same-hour old-binary control)
//!   WPT css/selectors    2905/5215 before, 2905/5215 after   (same-hour old-binary control)
//! ```
//!
//! ⚠⚠ **THE REMAINING `has-complexity` FAILURE IS A HANG, NOT A CRASH, AND THAT IS A DIFFERENT
//! MECHANISM WITH ITS OWN NAME.** `Page::relayout` *"recascades only when the node count outgrew the
//! style map"* (`engine/page/src/lib.rs`), so each of that test's 75,000 `appendChild` calls drives a
//! full re-cascade. Incremental style invalidation closes it. Saying *"the Bar 0 is fixed"* would be
//! the easy lie: the **segfault** is fixed and it was never that test's property, while that test is
//! still red for a reason this tick did not touch.
//!
//! ## How this goes RED
//!
//! Delete any one of the three `rooted!` lines. The assertion below is on **object identity** —
//! `parent.appendChild(n) === n` — which is what a stale pointer breaks, and the identity check runs
//! on every one of the iterations rather than only at the end, so corruption is caught whether or not
//! it happens to land on an unmapped page. ⚠ **The RED is probabilistic and this gate says so rather
//! than pretending otherwise**: it depends on a GC landing inside `record_mutation`. Measured on the
//! reverted tree in this crate's own build:
//!
//! ```text
//!   reverted tree   RED  6/10   (5 x SIGSEGV `signal: 11`, 1 x identity assertion)
//!   fixed tree      RED  0/10
//! ```
//!
//! ⚠ Both halves are stated because only the pair is evidence. 6/10 alone could be a flaky gate;
//! 10/10 green alone could be a gate that stopped looking. ⚠⚠ **One of the six REDs was NOT a
//! crash** — it reached the assertion and failed it, which is the case that matters most: a
//! relocated pointer that stays mapped hands script a live-looking object with the wrong contents,
//! and no amount of crash-watching would ever see it.

use manuk_text::FontContext;

/// The reverted-tree RED rate measured for THIS gate in THIS crate's build (t1164). A gate whose
/// RED is probabilistic must publish the number, or a green run cannot be distinguished from a gate
/// that stopped working.
const MEASURED_RED_RATE: &str = "6/10 reverted (5 SIGSEGV + 1 identity), 0/10 fixed";

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<main><div id="container"><span></span></div><div id="subject" class="subject"></div></main>
<div id="out">-</div>
<script>
var out = [];
try {
  var c = document.getElementById('container');
  var N = 20000;

  // ── appendChild: the returned object must BE the node passed in. A raw pointer that a moving GC
  //    relocated during `record_mutation` is no longer that object — and may not be an object.
  var badAppend = 0;
  for (var i = 0; i < N; i++) {
    var s = document.createElement('span');
    if (c.appendChild(s) !== s) badAppend++;
  }
  out.push('appendIdentity:' + (badAppend === 0));
  out.push('appendCount:' + (c.childNodes.length === N + 1));

  // ── insertBefore: same hazard, same shape. Insert before the first child each time.
  var badInsert = 0;
  for (var j = 0; j < 2000; j++) {
    var t = document.createElement('i');
    if (c.insertBefore(t, c.firstChild) !== t) badInsert++;
  }
  out.push('insertIdentity:' + (badInsert === 0));

  // ── removeChild: returns the detached node, and must return THAT node.
  var badRemove = 0;
  for (var k = 0; k < 2000; k++) {
    var f = c.firstChild;
    if (c.removeChild(f) !== f) badRemove++;
  }
  out.push('removeIdentity:' + (badRemove === 0));

  // The nodes are still usable objects afterwards — a relocated pointer that happens to stay mapped
  // gives a live-looking object with the wrong contents, which an identity check alone can miss.
  var last = c.lastChild;
  out.push('usable:' + (last && typeof last.nodeName === 'string' && last.nodeName.length > 0));
} catch (err) {
  out.push('THREW:' + err);
}
document.getElementById('out').textContent = out.join(' ');
</script></body></html>"##;

#[test]
fn dom_mutation_bindings_root_the_node_they_return() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://mutroot.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("G_DOM_MUTATION_ROOTING RESULT: {got}");

    for claim in [
        "appendIdentity:true",
        "appendCount:true",
        "insertIdentity:true",
        "removeIdentity:true",
        "usable:true",
    ] {
        assert!(
            got.contains(claim),
            "G_DOM_MUTATION_ROOTING: expected `{claim}`\n  got: {got}\n\n  \
             `appendChild`/`insertBefore`/`removeChild` must root the node object across \
             `record_mutation`, which allocates (`new_reflector` per referenced node) and can \
             therefore move it. A failure here is a stale `*mut JSObject` being returned to script \
             — the Bar 0 that a WPT complexity test had been taking the blame for. \
             RED rate on the reverted tree: {MEASURED_RED_RATE}."
        );
    }
}
