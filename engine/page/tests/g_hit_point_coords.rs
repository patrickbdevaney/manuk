//! **G_HIT_POINT_COORDS — `elementFromPoint` takes a CLIENT point, not a document one.**
//!
//! The mirror image of `G_CLIENT_COORDS`, and deliberately a separate gate: that one holds the way
//! **out** of the geometry snapshot (`getBoundingClientRect` hands back a rect), this one holds the
//! way **in** (`elementFromPoint` takes a point). CSSOM View defines both on **client** coordinates;
//! `LAYOUT_RECTS_PTR` holds **document** boxes; the conversion belongs at the boundary and had been
//! written at neither end.
//!
//! On a page scrolled to 300, `elementFromPoint(10, 10)` asks *"what is at the top-left of the
//! screen?"* and used to be answered with *"what is at the top-left of the document."*
//!
//! ⚠ **Zero percent wrong until the page scrolls** — every fixture that hit-tests an unscrolled page
//! passes either way, which is why the engine's own comment carried this as an accepted bound
//! (*"scroll offset is assumed zero"*) rather than as a bug. What it breaks on a scrolled page is
//! every drag-and-drop library (a `dragover` handler resolves its drop target this way), tooltip and
//! popover occlusion checks, canvas/overlay hit routing, and `caretRangeFromPoint`.
//!
//! **To watch it go RED:** drop the `from_client(x, y)` from `doc_element_from_point` — the scrolled
//! rows below return the element that is 300px further UP the document. Dropping it from
//! `doc_elements_from_point` alone breaks only the `plural:`/`agree:` rows, which is what makes the
//! singular/plural agreement a claim of its own rather than a restatement.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<div id="a" style="height:300px">A — document y 0..300</div>
<div id="b" style="height:300px">B — document y 300..600</div>
<div id="c" style="height:300px">C — document y 600..900</div>
<div style="height:2000px">tail</div>
<div id="out">-</div>
<script>
  var R = [], D = document;
  var id = function (e) { return e ? (e.id || e.tagName) : 'null'; };

  // Unscrolled: client and document coordinates coincide. The control — a fix that adds the scroll
  // unconditionally, or subtracts it, fails HERE.
  R.push('at10:' + id(D.elementFromPoint(10, 10)));      // a
  R.push('at400:' + id(D.elementFromPoint(10, 400)));    // b

  window.scrollTo(0, 300);
  // THE GATE. The top of the SCREEN is now document y 300, which is B — not A.
  R.push('s10:' + id(D.elementFromPoint(10, 10)));       // b
  R.push('s400:' + id(D.elementFromPoint(10, 400)));     // c

  // The plural sibling must convert identically: `elementsFromPoint(x,y)[0]` is required to equal
  // `elementFromPoint(x,y)` for every point, and two call sites converting differently is exactly
  // how that invariant breaks.
  R.push('plural:' + id(D.elementsFromPoint(10, 10)[0]));
  R.push('agree:' + (D.elementsFromPoint(10, 10)[0] === D.elementFromPoint(10, 10)));

  // A non-finite argument must still THROW its own message — the conversion happens AFTER the
  // WebIDL check, so a NaN never becomes `NaN + scroll` and silently misses everything instead.
  try { D.elementFromPoint(NaN, 0); R.push('nan:none'); }
  catch (e) { R.push('nan:' + (e instanceof TypeError ? 'TypeError' : 'other')); }

  D.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

// ⚠ ONE `#[test]` fn per JS gate binary — two live `PageContext`s on two threads fault SpiderMonkey.
#[test]
fn element_from_point_takes_a_viewport_point() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://hitpoint.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "at10:a",
            "unscrolled, the top of the screen is the top of the document — the control that stops a \
             blanket conversion from passing",
        ),
        (
            "at400:b",
            "a second unscrolled point, so `at10` cannot be satisfied by always returning the first \
             element",
        ),
        (
            "s10:b",
            "THE GATE. Scrolled to 300, the top of the SCREEN is document y 300, which is B. \
             Answering `a` here is the whole defect: every drag-and-drop drop target on a scrolled \
             page resolves to the element 300px further up",
        ),
        (
            "s400:c",
            "a second scrolled point — `s10` alone could be satisfied by an off-by-one-element \
             accident; two points 390px apart cannot",
        ),
        (
            "plural:b",
            "elementsFromPoint converts identically. Two call sites reading one boundary is exactly \
             how they drift",
        ),
        (
            "agree:true",
            "`elementsFromPoint(x,y)[0] === elementFromPoint(x,y)` is required for every point, and \
             a library that checks it finds any drift immediately",
        ),
        (
            "nan:TypeError",
            "the conversion runs AFTER the WebIDL finite check, so a non-finite argument still \
             throws its own message rather than becoming `NaN + scroll` and quietly matching nothing",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_HIT_POINT_COORDS: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
