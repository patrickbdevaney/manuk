//! **G_FORCED_REFLOW — a geometry read after a mid-script mutation sees the NEW layout.**
//!
//! The engine lays out in a **batch**: script runs against the layout snapshot taken before the
//! script started, and one relayout happens after. That is correct for `measure` and correct for
//! `mutate`, and wrong for the pattern every virtualized list is built out of:
//!
//! ```text
//!   measure  ->  mutate  ->  measure     (all inside ONE task/rAF)
//! ```
//!
//! react-window, react-virtuoso and every data grid size their rows by writing to the DOM and then
//! *immediately reading it back*. Against a pre-script snapshot the second read returns the geometry
//! the element had **before** the write — usually `0` for a node that did not exist yet — so rows
//! collapse, overlap, or render blank.
//!
//! A real browser answers this by **forcing a synchronous reflow**: a geometry read on a dirty DOM
//! lays out first, then returns. That is what this gate holds. It is the read path's job, not the
//! writer's — the page never asks for the reflow, it just reads.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<div id="out">-</div>
<div id="host" style="width:200px"></div>
<div id="grow" style="width:100px; height:10px"></div>
<script>
  var R = [], host = document.getElementById('host'), grow = document.getElementById('grow');

  // 1. measure -> mutate -> measure, on a node that did not exist at snapshot time.
  //    The appended child is 40px tall, so the host must report 40 immediately.
  R.push('before:' + host.getBoundingClientRect().height);   // 0 — empty host
  var row = document.createElement('div');
  row.setAttribute('style', 'height:40px');
  host.appendChild(row);
  R.push('after:' + host.getBoundingClientRect().height);    // 40 — FORCED REFLOW

  // 2. the appended node measures itself (the react-window row-sizing read).
  R.push('row:' + row.getBoundingClientRect().height);       // 40

  // 3. a style write on an EXISTING node is visible to the very next read.
  grow.setAttribute('style', 'width:100px; height:70px');
  R.push('grown:' + grow.getBoundingClientRect().height);    // 70

  // 4. offsetHeight travels the same read path as getBoundingClientRect.
  R.push('offset:' + host.offsetHeight);                     // 40

  // 5. getComputedStyle is a forced-reflow trigger too — a style write must be visible to the
  //    very next read of it, or every "measure my own new height" helper reads its old value.
  R.push('cs:' + getComputedStyle(grow).height);             // 70px

  // 6. a read that dirties nothing must NOT change the answer (reflow is idempotent).
  R.push('again:' + host.getBoundingClientRect().height);    // 40

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

// ⚠ ONE `#[test]` fn for this whole binary, deliberately — see `g_canvas.rs`, which does the same.
// A test fn that dispatches a click leaves a live `PageContext` (SpiderMonkey runtime) parked on
// its thread; a *second* test fn then loads a page on another thread and the two runtimes fault.
// Sequential `Page::load`s inside ONE fn are fine, so the cases are ordered here rather than split.
#[test]
fn a_geometry_read_after_a_mutation_forces_layout() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://reflow.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "before:0",
            "an empty block has no height — the baseline read, so a later 40 cannot be luck",
        ),
        (
            "after:40",
            "appending a 40px row and immediately measuring the parent must report 40. Reading the \
             pre-script snapshot here returns 0, which is how a virtualized list renders blank rows",
        ),
        (
            "row:40",
            "the node created THIS tick can measure itself — it has no snapshot geometry to fall \
             back on, so this fails as `undefined`/0 without a forced reflow",
        ),
        (
            "grown:70",
            "a style write on an existing node is visible to the next read, not deferred to the \
             post-script batch",
        ),
        (
            "offset:40",
            "offsetHeight is a geometry read too — it must force the same reflow, or the two APIs \
             disagree about the same element",
        ),
        (
            "cs:70px",
            "getComputedStyle is a forced-reflow trigger as much as getBoundingClientRect is — a \
             style written a line earlier must be what it reports, not the pre-write cascade",
        ),
        (
            "again:40",
            "a second read with nothing dirtied returns the same answer — the forced reflow is \
             idempotent and does not re-lay-out a clean tree",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_FORCED_REFLOW: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }

    // The load-time script above is the easy half: nothing has happened yet. The load-more button
    // on a feed appends rows and measures them in the click handler, one round, and that round
    // enters through `dispatch_click` rather than through the document load.
    const CLICK_HTML: &str = r##"<!doctype html><html><body style="margin:0">
<div id="out">-</div>
<div id="list"></div>
<button id="more">Load more</button>
<script>
  document.getElementById('more').addEventListener('click', function () {
    var list = document.getElementById('list');
    var before = list.getBoundingClientRect().height;
    for (var i = 0; i < 3; i++) {
      var row = document.createElement('div');
      row.setAttribute('style', 'height:25px');
      list.appendChild(row);
    }
    var after = list.getBoundingClientRect().height;
    document.getElementById('out').textContent = 'grew:' + before + '->' + after;
  });
</script></body></html>"##;

    // ── AND THE SAME GUARANTEE INSIDE AN EVENT HANDLER, which is where a real feed does it.
    let mut page = manuk_page::Page::load(CLICK_HTML, "https://reflow.test/", &fonts, 800.0);
    let root = page.dom().root();
    let more = manuk_css::query_selector_all(page.dom(), root, "#more")[0];
    page.dispatch_click(more, &fonts, 800.0);

    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    assert!(
        got.contains("grew:0->75"),
        "G_FORCED_REFLOW: expected `grew:0->75`\n  got: {got}\n\n  A click handler that appends \
         three 25px rows and measures the container must read 75. Reading the pre-dispatch \
         snapshot returns 0, which is a feed whose 'load more' appends rows it then sizes to \
         nothing."
    );
}
