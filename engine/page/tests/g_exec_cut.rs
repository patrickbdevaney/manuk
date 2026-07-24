//! **G_EXEC_CUT — `execCommand('cut')` copies the selection to the clipboard AND removes it.**
//!
//! Cut (Ctrl+X, the "cut" toolbar button) was honestly `false` — the deletion half of the editing
//! subsystem wasn't built. Now that a contenteditable can delete (t473/t474), cut is real: it copies the
//! selected text to the host clipboard (the same bridge `copy` uses) and then removes it from the DOM,
//! firing `beforeinput`→`input` (`inputType:'deleteByCut'`). It requires a non-collapsed selection inside
//! an editing host — a cut with nothing selected, or a selection in a non-editable region, stays `false`.
//!
//! ## Each claim, and how it goes RED
//!
//!   * `supported=true` — `queryCommandSupported('cut')` is now `true`.
//!   * `ret=true`/`text=Hello ` — selecting "World" in "Hello World" and cutting returns `true` and the
//!     editable is left "Hello " (the selection removed). RED: drop the `cut` branch → returns `false`,
//!     the text is unchanged.
//!   * `evs=bi:deleteByCut|in:deleteByCut` — the removal fires beforeinput then input with the cut inputType.
//!   * `noedit=false` — a cut whose selection is in a NON-editable `<pre>` returns `false` (you cannot
//!     remove text you cannot edit) — honest scope.
//!   * clipboard — the cut text "World" reached the host clipboard queue (checked in Rust).

use manuk_page::Page;
use manuk_text::FontContext;

const W: f32 = 800.0;

fn node(p: &Page, sel: &str) -> manuk_dom::NodeId {
    let root = p.dom().root();
    manuk_css::query_selector_all(p.dom(), root, sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"))
}

#[test]
fn exec_command_cut_copies_the_selection_to_the_clipboard_and_removes_it() {
    let _ = manuk_js::take_clipboard_writes(); // drain prior state

    let fonts = FontContext::new();
    let p = Page::load(
        r#"<!doctype html><body>
<div id="ed" contenteditable="true">Hello World</div>
<pre id="ro">read only</pre>
<div id="log"></div>
<script>
  var evs = [];
  var ed = document.getElementById('ed');
  ed.addEventListener('beforeinput', function (e) { evs.push('bi:' + e.inputType); });
  ed.addEventListener('input',       function (e) { evs.push('in:' + e.inputType); });

  var r = [];
  function k(n, v) { r.push(n + '=' + v); }
  try {
    // Select "World" (offsets 6..11) in the editable and cut it.
    var t = ed.firstChild;
    window.getSelection().setBaseAndExtent(t, 6, t, 11);
    k('supported', document.queryCommandSupported('cut'));   // true
    k('ret', document.execCommand('cut'));                   // true
    k('text', ed.textContent);                               // "Hello "
    k('evs', evs.join('|'));                                 // bi:deleteByCut|in:deleteByCut

    // A cut whose selection is in a NON-editable <pre> is a no-op.
    var ro = document.getElementById('ro');
    window.getSelection().selectAllChildren(ro);
    k('noedit', document.execCommand('cut'));                // false — not editable
  } catch (e) { k('THREW', e); }
  document.getElementById('log').textContent = r.join(' ');
</script></body>"#,
        "https://cut.test/",
        &fonts,
        W,
    );

    let out = p.dom().text_content(node(&p, "#log"));
    println!("EXEC-CUT RESULT: {out}");

    for claim in [
        "supported=true",
        "ret=true",
        "text=Hello ", // "World" removed from the editable
        "evs=bi:deleteByCut|in:deleteByCut",
        "noedit=false", // a non-editable selection cannot be cut
    ] {
        assert!(
            out.contains(claim),
            "G_EXEC_CUT: expected `{claim}` in {out:?}\n  \
             execCommand('cut') must copy the selection to the clipboard and remove it from an editable, \
             firing beforeinput→input (inputType:deleteByCut); a selection outside an editable stays false."
        );
    }

    // The strongest tooth: the cut text actually reached the host clipboard queue.
    let writes = manuk_js::take_clipboard_writes();
    println!("EXEC-CUT CLIPBOARD WRITES: {writes:?}");
    assert!(
        writes.iter().any(|w| w == "World"),
        "G_EXEC_CUT: execCommand('cut') must put the CUT TEXT on the host clipboard — \
         queued writes were {writes:?}, expected to contain \"World\""
    );
}
