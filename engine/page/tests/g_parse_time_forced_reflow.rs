//! **G_PARSE_TIME_FORCED_REFLOW — a blocking `<script>` measures the tree it just mutated.**
//!
//! `force_reflow_if_stale` guards every geometry read: a browser answers `getBoundingClientRect()`
//! on a dirtied DOM by laying out *before* it answers, which is what makes `measure → mutate →
//! measure` work — the shape every virtualized list, equal-height grid, masonry column and
//! sticky-footer routine is built out of.
//!
//! ⚠⚠⚠ **THIS GATE EXISTS TO RETIRE A FALSE FINDING, AND IT PASSES.** t1236 recorded, as a side
//! effect of building `G_REFLOW_ACCOUNTING`, that *"a parse-time inline `<script>` reports ZERO
//! forced reflows"* — its first fixture ran at parse time and the counter stayed at 0 — and filed
//! that as a candidate defect consistent with t1183-1188's *"`ReflowScope` missing from 2 of 19
//! rounds"*. **It was an artefact of the very bug that same tick fixed.** The counter was still
//! being RESET by each drain when the observation was made; it was made monotonic later in the tick
//! and the parse-time case was never re-checked. Both halves are measured here and both are correct:
//! the geometry is FRESH, and the accounting SEES it.
//!
//! It is kept rather than deleted because the claim it retires is written in a journal, a wiki page
//! and a memory file, and *a suspicion recorded in three places will be re-derived by whoever reads
//! them*. This is the cheapest possible refutation: it runs in 0.3s and it fails the day either half
//! stops being true.
//!
//! **This is the load path, not an edge case** — a blocking `<script>` in `<head>` or mid-`<body>`
//! is where jQuery-era pages do their first layout pass, before `DOMContentLoaded`. Which is exactly
//! why the false version of this finding was worth the tick it took to kill.
//!
//! **How to break it:** remove the `ReflowScope` from the parse-time script path. The second
//! measurement goes back to reporting the pre-mutation height.

use manuk_text::FontContext;

/// `measure → mutate → measure`, entirely inside a PARSE-TIME blocking `<script>`. The `<div>`s are
/// given an explicit height so the expected number is arithmetic and not a font metric.
const HTML: &str = r##"<!doctype html><html><body>
<div id="host"></div>
<script>
  var host = document.getElementById('host');
  var before = host.getBoundingClientRect().height;
  for (var i = 0; i < 4; i++) {
    var d = document.createElement('div');
    d.style.height = '25px';
    host.appendChild(d);
  }
  var after = host.getBoundingClientRect().height;
  window.__R = 'before:' + before + ' after:' + after;
</script>
<div id="out">-</div>
<script>document.getElementById('out').textContent = window.__R;</script>
</body></html>"##;

#[test]
fn a_parse_time_script_measures_the_tree_it_just_mutated() {
    let fonts = FontContext::new();
    let (before_reflows, _) = manuk_js::dom_bindings::reflow_cost();
    let page = manuk_page::Page::load(HTML, "https://ptreflow.test/", &fonts, 800.0);
    let reflows = manuk_js::dom_bindings::reflow_cost().0 - before_reflows;
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    assert!(
        got.contains("before:0"),
        "G_PARSE_TIME_FORCED_REFLOW: the empty host should measure 0 high before the mutation.\n  \
         got: {got}\n\n  If this is not 0 the fixture is wrong, not the engine — fix the fixture \
         before reading the second half."
    );
    assert!(
        got.contains("after:100"),
        "G_PARSE_TIME_FORCED_REFLOW: four 25px children were appended and the host still measures \
         the PRE-MUTATION height.\n  got: {got}\n\n  A geometry read on a dirtied DOM must lay out \
         before it answers, and the parse-time round DOES arm a ReflowScope. If this reads the \
         pre-mutation height, `measure → mutate → measure` is broken on the load path and every \
         virtualized list, equal-height grid and masonry routine that runs before DOMContentLoaded \
         divides by a zero it should never have seen."
    );

    // ⚠⚠⚠ **AND THE ACCOUNTING MUST SEE IT.** t1236 recorded, as a side finding, that a parse-time
    // script reports ZERO forced reflows — and that was an artefact of the very bug t1236 fixed in
    // the same tick: the counter was still being RESET by each drain when the observation was made,
    // and it was never re-checked after being made monotonic. This arm pins the correction, so the
    // false residue cannot be re-derived from the journal.
    assert!(
        reflows > 0,
        "G_PARSE_TIME_FORCED_REFLOW: geometry is fresh but the reflow counter saw {reflows} \
         reflows.\n\n  Either the parse-time path answers geometry without going through \
         `force_reflow_if_stale` — in which case `reflow_ms` UNDER-REPORTS layout work and t1236's \
         attribution has a blind spot — or the counter is broken again. Both are worth knowing and \
         they are different bugs."
    );
}
