//! **G_TRANSFORM — `getComputedStyle(el).transform` returns the resolved matrix, not `undefined`.**
//!
//! The pattern ledger said *"CSS `transform` — **not in computed style** — a real gap"*, and it made it
//! sound like a layout bug. It is not. **The transform has always been applied**: `translateX(100px)`
//! really moves the box, and `getBoundingClientRect()` agrees to the pixel (`G_CAPABILITY` asserts it).
//!
//! What was missing is the *read-back*, and that is its own kind of damage. Every animation library reads
//! the current transform before composing its own —
//!
//! ```js
//! el.style.transform = getComputedStyle(el).transform + ' scale(2)';
//! ```
//!
//! — and `undefined + ' scale(2)'` is the **string** `"undefined scale(2)"`, which is not a parse error,
//! not an exception, and not a transform. The element simply stops moving. GSAP, Framer Motion, and every
//! hand-rolled tween on the web do exactly this.
//!
//! The spec's *resolved value* is a `matrix(a, b, c, d, e, f)` string — never the author's shorthand — so
//! `translateX(10px)` reads back as `matrix(1, 0, 0, 1, 10, 0)`. A library that expects to re-parse it
//! depends on that.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<div id="out">-</div>
<div id="plain" style="width:100px;height:20px">a</div>
<div id="moved" style="width:100px;height:20px;transform:translateX(10px)">b</div>
<div id="scaled" style="width:100px;height:20px;transform:scale(2)">c</div>
<div id="both" style="width:100px;height:20px;transform:translateX(10px) scale(2)">d</div>
<div id="pct" style="width:100px;height:20px;transform:translateX(50%)">e</div>
<script>
  var R = [];
  function cs(id){ return getComputedStyle(document.getElementById(id)).transform; }

  R.push('none:' + cs('plain'));          // no transform is the string "none", not "" and not undefined
  R.push('tx:' + cs('moved'));
  R.push('scale:' + cs('scaled'));
  // Composition is left-to-right, exactly as CSS multiplies it.
  R.push('both:' + cs('both'));
  // A percentage translate resolves against the element's OWN border box (100px wide → 50px).
  R.push('pct:' + cs('pct'));
  // getPropertyValue must find it under its kebab-case name too — half the web asks for it that way.
  R.push('kebab:' + getComputedStyle(document.getElementById('moved')).getPropertyValue('transform'));

  // And the thing the whole feature exists for: re-composing the current transform must not produce
  // the string "undefined scale(2)".
  var composed = cs('moved') + ' scale(2)';
  R.push('composable:' + (composed.indexOf('undefined') === -1));

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn computed_transform_resolves_to_a_matrix_that_a_library_can_re_parse() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://transform.test/", &fonts, 400.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "none:none",
            "no transform is the STRING \"none\" — not \"\", not undefined. Libraries branch on it",
        ),
        (
            "tx:matrix(1, 0, 0, 1, 10, 0)",
            "the spec's resolved value is a matrix, never the author's `translateX(10px)` shorthand — a \
             library that re-parses it depends on that",
        ),
        ("scale:matrix(2, 0, 0, 2, 0, 0)", "scale composes into a, d"),
        (
            "both:matrix(2, 0, 0, 2, 10, 0)",
            "composition is left-to-right, exactly as CSS multiplies it",
        ),
        (
            "pct:matrix(1, 0, 0, 1, 50, 0)",
            "a percentage translate resolves against the element's OWN border box — 50% of 100px is 50px, \
             and getting this wrong moves every centred modal on the web",
        ),
        ("kebab:matrix(1, 0, 0, 1, 10, 0)", "getPropertyValue('transform') must find it too"),
        (
            "composable:true",
            "THE POINT. `getComputedStyle(el).transform + ' scale(2)'` used to be the string \
             \"undefined scale(2)\" — not a parse error, not an exception, just an element that stops \
             moving. Every animation library on the web does exactly this",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_TRANSFORM: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
