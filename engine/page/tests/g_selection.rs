//! **G_SELECTION — `window.getSelection()` is a real, persistent, directional Selection.**
//!
//! The programmatic Selection API — not the mouse-drag one. The failure it replaces was a stub that
//! returned a *fresh inert object on every call*: `rangeCount` 0 forever, every mutator a no-op, and
//! `getSelection() !== getSelection()`. That shape passes any "does the method exist" test and fails
//! every "did the method do anything" one, which is exactly the class of bug this gate exists for.
//!
//! ## The silent failure this is measured against
//!
//! The canonical use is a "copy this code block" button: `sel.selectAllChildren(pre)` then
//! `navigator.clipboard.writeText(sel.toString())`. Under the stub, `toString()` returned `''` — the
//! button copied nothing, and nothing threw. So the load-bearing claim below is **`copyall`**:
//! `selectAllChildren` followed by `toString()` yields the element's actual text.
//!
//! ## Why direction is a claim of its own
//!
//! A `Range` is normalised (`start <= end` always); a `Selection` is *directional* — the anchor is
//! the fixed end, the focus is the one `extend()` moves, and a user can drag left. If Selection were
//! just a Range wrapper, extending *before* the anchor would silently swap the ends and the anchor
//! would jump. The **`backextend`** claim pins this: anchor stays at offset 2 while the focus moves
//! to 0, `toString()` is still the text between the points, and `anchorOffset > focusOffset`.
//!
//! ## The RED probe (run, not imagined)
//!
//! Restoring the old stub (`getSelection` returns a fresh `{rangeCount:0, toString:()=>''}`) drops
//! `same`, `copyall`, `range`, `caret`, `fwd`, `backextend`, `added` and `inst` together — the whole
//! behavioural surface collapses while a `typeof getSelection === 'function'` check stays green.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="t">abcdefgh</div>
<pre id="code">line1
line2</pre>
<div id="out">-</div>
<script>
  var R = [];
  var push = function (s) { R.push(s); document.getElementById('out').textContent = R.join(' '); };
  try {
    var sel = window.getSelection();

    // ── Persistence + identity. The stub failed this first: it minted a new object every call, so no
    //    state could survive between two lines of a caller.
    push('same:' + (sel === window.getSelection() && sel === document.getSelection()));
    push('inst:' + (sel instanceof Selection));

    // ── The empty state, honestly reported.
    push('none:' + (sel.rangeCount === 0 && sel.type === 'None' &&
                    sel.anchorNode === null && sel.toString() === ''));
    // getRangeAt on an empty selection THROWS — a caller that only wrote a try/catch can see it.
    try { sel.getRangeAt(0); push('throws:false'); }
    catch (e) { push('throws:' + (e.name === 'IndexSizeError')); }

    // ── selectAllChildren + toString: the copy-code-block path.
    var code = document.getElementById('code');
    sel.selectAllChildren(code);
    push('copyall:' + (sel.toString() === 'line1\nline2'));
    push('range:' + (sel.rangeCount === 1 && sel.type === 'Range' && sel.isCollapsed === false));
    push('allanchor:' + (sel.anchorNode === code && sel.anchorOffset === 0 &&
                         sel.focusNode === code && sel.focusOffset === code.childNodes.length));

    // ── A directional forward selection over one text node: offsets 2..5 of "abcdefgh" is "cde".
    var tn = document.getElementById('t').firstChild;
    sel.setBaseAndExtent(tn, 2, tn, 5);
    push('fwd:' + (sel.toString() === 'cde' && sel.anchorOffset === 2 && sel.focusOffset === 5 &&
                   sel.type === 'Range'));

    // ── collapse → a caret. Empty text, collapsed, both ends at 3.
    sel.collapse(tn, 3);
    push('caret:' + (sel.isCollapsed === true && sel.type === 'Caret' &&
                     sel.anchorOffset === 3 && sel.focusOffset === 3 && sel.toString() === ''));

    // ── extend past the anchor, then BEFORE it. The anchor must not move; the direction flips.
    sel.collapse(tn, 2);
    sel.extend(tn, 6);
    push('fwdextend:' + (sel.anchorOffset === 2 && sel.focusOffset === 6 && sel.toString() === 'cdef'));
    sel.extend(tn, 0);
    push('backextend:' + (sel.anchorOffset === 2 && sel.focusOffset === 0 &&
                          sel.toString() === 'ab' && sel.anchorOffset > sel.focusOffset));

    // ── addRange over a range built the ordinary way. removeAllRanges first, then one range sticks;
    //    getRangeAt(0) reflects it, and a second range is IGNORED (Chrome's one-range model).
    sel.removeAllRanges();
    push('cleared:' + (sel.rangeCount === 0));
    var r = document.createRange();
    r.setStart(tn, 1); r.setEnd(tn, 4);
    sel.addRange(r);
    push('added:' + (sel.rangeCount === 1 && sel.getRangeAt(0).toString() === 'bcd'));
    var r2 = document.createRange();
    r2.setStart(tn, 0); r2.setEnd(tn, 8);
    sel.addRange(r2);
    push('oneonly:' + (sel.rangeCount === 1 && sel.getRangeAt(0).toString() === 'bcd'));

    document.getElementById('out').textContent = R.join(' ');
  } catch (e) {
    document.getElementById('out').textContent = 'THREW:' + e;
  }
</script>
</body></html>"##;

/// One test in the binary — a second `Page::load` stands up a second SpiderMonkey context in the same
/// process and does not survive teardown (see `g_visibility`, `g_mse`, `g_globals`).
#[test]
fn the_page_can_read_and_drive_its_selection() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://selection.test/", &fonts, 800.0);

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("same:true", "getSelection() must return the SAME persistent object every call (and document.getSelection() the same one) — the stub minted a fresh inert object per call, so no state survived between two lines of a caller"),
        ("inst:true", "the returned object must be a real `Selection` instance — a bundle that does `x instanceof Selection` gets false from an inert placeholder"),
        ("none:true", "an untouched selection reports rangeCount 0, type 'None', null anchorNode and '' — the honest empty state"),
        ("throws:true", "getRangeAt(0) on an empty selection must throw IndexSizeError, not return null; a caller that guards with try/catch depends on the throw"),
        ("copyall:true", "THE CLAIM THIS GATE EXISTS FOR: selectAllChildren(pre) then toString() must yield the element's text ('line1\\nline2'). The stub answered '' and the copy-code-block button silently copied nothing"),
        ("range:true", "after selectAllChildren the selection is a non-collapsed Range with rangeCount 1"),
        ("allanchor:true", "selectAllChildren anchors at (node, 0) and focuses at (node, childNodes.length) — the whole element's contents"),
        ("fwd:true", "setBaseAndExtent(tn,2,tn,5) over 'abcdefgh' selects 'cde' with anchorOffset 2 and focusOffset 5"),
        ("caret:true", "collapse(tn,3) is a caret: collapsed, type 'Caret', both offsets 3, empty string"),
        ("fwdextend:true", "collapse then extend forward keeps the anchor at 2 and moves the focus to 6 → 'cdef'"),
        ("backextend:true", "extend BEFORE the anchor is a backwards selection: anchor stays 2, focus goes to 0, toString is still the text between ('ab') and anchorOffset > focusOffset — a plain Range wrapper would swap the ends and move the anchor"),
        ("cleared:true", "removeAllRanges drops the range"),
        ("added:true", "addRange adopts a range built with createRange/setStart/setEnd; getRangeAt(0).toString() is 'bcd'"),
        ("oneonly:true", "a second addRange is ignored (Chrome keeps at most one range) — the first range still stands"),
    ] {
        assert!(
            got.contains(claim),
            "G_SELECTION: missing {claim:?} — {why}.\n  got: {got}"
        );
    }
}
