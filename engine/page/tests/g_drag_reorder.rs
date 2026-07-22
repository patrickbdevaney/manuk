//! **G_DRAG_REORDER — a page can DRAG one of its own elements onto another, and the drop reads what
//! the drag wrote.**
//!
//! The EDITOR half of drag-and-drop. `G_DROP_UPLOAD` covers the *target* side of an OS-file drag (the
//! dashed rectangle that reads `e.dataTransfer.files`). This covers the *source* side the page itself
//! originates: a sortable list, a kanban board, a reorderable table — every one of them is built by a
//! `dragstart` handler that calls `e.dataTransfer.setData('text/plain', id)` and a `drop` handler on
//! the target that reads that id back with `getData` and moves the row.
//!
//! ## The capability is the setData→getData HANDOFF, threaded through ONE DataTransfer
//!
//! A reorder works only because the value the source writes on `dragstart` is the value the target
//! reads on `drop` — *the same object*. Dispatch `drop` alone and there is no `dragstart` to write the
//! id, so `getData` comes back empty and the card moves nowhere; the round-trip is the whole point.
//! And the full protocol matters at both ends: the target opts in by cancelling `dragover` (a target
//! that never does never receives a drop in a real browser), and the source's `dragend` is the
//! notification every library uses to clear its "dragging" class and finalize the move.
//!
//! ## The RED probe (run, not imagined)
//!
//! Neutering the `dragstart` dispatch in `PageContext::dispatch_drag` (so only the target sequence
//! runs) takes the gate RED at `handoff:` — the `drop` handler's `getData('text/plain')` returns `''`
//! because nothing wrote it, so the reorder resolves to moving "" and the assertion fails.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<ul id="list">
  <li id="a" draggable="true">A</li>
  <li id="b" draggable="true">B</li>
</ul>
<div id="ev">-</div>
<script>
  var seen = [];
  var $ = function (i) { return document.getElementById(i); };
  var src = $('a');
  var tgt = $('b');

  // A sortable list written the way the web writes them: the source stashes its id on dragstart,
  // the target cancels dragover to opt in, and the drop reads the id back to know what moved.
  src.addEventListener('dragstart', function (e) {
    // If dataTransfer is undefined this throws — the same failure shape the file-drop half had.
    e.dataTransfer.setData('text/plain', 'a');
    e.dataTransfer.effectAllowed = 'move';
    seen.push('start:true');
  });
  src.addEventListener('dragend', function (e) {
    // dragend is the LAST event in the gesture, so it writes the final record — capturing
    // start/handoff/order (pushed earlier) plus its own 'end'. A drop-time write would miss this.
    seen.push('end:true');
    $('ev').textContent = seen.join(' ');
  });
  tgt.addEventListener('dragover', function (e) {
    e.preventDefault();   // THE OPT-IN. Without this a real browser never delivers the drop.
  });
  tgt.addEventListener('drop', function (e) {
    var moved = e.dataTransfer.getData('text/plain');
    // The handoff: the id the SOURCE wrote on dragstart is the id the TARGET reads on drop.
    seen.push('handoff:' + (moved === 'a'));
    seen.push('order:' + (seen[0] === 'start:true'));
    e.preventDefault();
    $('ev').textContent = seen.join(' ');
  });
</script>
</body></html>"##;

/// One test in the binary — a second `Page::load` stands up a second SpiderMonkey context in the same
/// process and does not survive teardown (see `g_drop_upload`, `g_file_input`, `g_visibility`).
#[test]
fn dragging_a_list_item_onto_another_hands_off_setdata_to_getdata() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://reorder.test/", &fonts, 800.0);

    let root = page.dom().root();
    let src = manuk_css::query_selector_all(page.dom(), root, "#a")[0];
    let root = page.dom().root();
    let tgt = manuk_css::query_selector_all(page.dom(), root, "#b")[0];

    let proceed = page.dispatch_drag(src, tgt, &fonts, 800.0);
    assert!(
        !proceed,
        "the target called preventDefault() on the drop, so the host must NOT also perform a default \
         action — a reorder that also let the browser navigate to the dragged content would blow the \
         list away mid-sort"
    );

    let root = page.dom().root();
    let ev = manuk_css::query_selector_all(page.dom(), root, "#ev")[0];
    let got = page.dom().text_content(ev);

    for (claim, why) in [
        ("start:true", "dragstart must fire on the SOURCE with a real DataTransfer — this is the source half a file drop never exercises, and where a sortable list writes the id of the thing being dragged"),
        ("handoff:true", "the id the source wrote with setData on dragstart is the id the target reads with getData on drop — the setData->getData handoff through ONE DataTransfer IS the reorder; without dragstart it comes back '' and the card moves nowhere"),
        ("order:true", "dragstart precedes the drop: the source populates the transfer before the target reads it, which is the only order in which the handoff can carry a value"),
        ("end:true", "dragend fires on the source to finalize the gesture — the notification every drag library uses to clear its 'dragging' styling and commit the move"),
    ] {
        assert!(
            got.contains(claim),
            "G_DRAG_REORDER: missing {claim:?} — {why}.\n  got: {got}"
        );
    }
}
