//! **G_REFLECT_NUMERIC — the integer-coercion rules every reflected number attribute got subtly wrong.**
//!
//! These are `html/dom/reflection.js`'s own `domExpected` functions turned into assertions, because
//! the arithmetic is where ~380 failing subtests lived, and each rule is a *different* failure the
//! generic reflector produced silently:
//!
//! * **`-0` is `+0`.** The HTML "rules for parsing integers" accumulate a magnitude and return a bare
//!   `0` for zero, sign or not. `assert_equals` is `Object.is`-based, so a leaked JS `-0` fails every
//!   `setAttribute() to "-0"` case — and there is one for every numeric attribute on every element
//!   (the single biggest cluster, 143 subtests).
//! * **overflow FALLS BACK, it does not wrap.** `tabindex="2147483648"` is out of the signed-32 range,
//!   so it reads back as the default `0` — NOT `-2147483648`, which is what a naive `n | 0` ToInt32
//!   wrap (a tempting "fix") would produce, and which no browser does.
//! * **`limited long` (maxLength/minLength) defaults to `-1`**, and an invalid *or* overflowing value
//!   returns that `-1`, not `0` (the second cluster, 234 subtests).
//! * **`clamped unsigned long` CLAMPS**: a `colspan` of a billion is `1000` (the max), not the
//!   default — out-of-range clamps to the bound, unlike the plain-unsigned fall-back to default.
//!
//! Kept in its own binary on purpose: two SpiderMonkey-backed `Page::load`s in a single test process
//! reuse the JS runtime and can trip the tracked reflector-teardown UAF (see the flexbox-relayout
//! Bar-0 note). Every JS gate here is one file = one process for exactly that reason.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<input id="ml">
<table><tr><td id="cs"></td></tr></table>
<div id="ti"></div>
<img id="w">
<script>
  var R = [];
  var ml = document.getElementById('ml');
  var cs = document.getElementById('cs'), ti = document.getElementById('ti');
  var w = document.getElementById('w');

  ti.setAttribute('tabindex', '2147483648');
  R.push('overflow:' + ti.tabIndex);                    // out of long range → default 0, NOT wrapped

  ml.setAttribute('maxlength', 'notanumber');
  R.push('limInvalid:' + ml.maxLength);                 // limited long invalid → -1
  ml.setAttribute('maxlength', '2147483648');
  R.push('limOverflow:' + ml.maxLength);                // limited long overflow → -1
  ml.setAttribute('maxlength', '5');
  R.push('limOk:' + ml.maxLength);                      // valid → 5

  cs.setAttribute('colspan', '2147483648');
  R.push('clamp:' + cs.colSpan);                        // clamped [1,1000] → 1000, not the default

  ti.setAttribute('tabindex', '-0');
  R.push('tiVal:' + ti.tabIndex);                       // "-0" parses to +0
  R.push('tiNeg:' + Object.is(ti.tabIndex, -0));        // …and it is NOT negative zero

  w.setAttribute('width', '-0');
  R.push('wNeg:' + Object.is(w.width, -0));             // same for an unsigned attribute

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn numeric_reflection_coercion_matches_the_html_parsing_rules() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://reflect.test/n/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("overflow:0", "a plain `long` that overflows the signed-32 range falls back to the default, it is NOT wrapped mod 2^32"),
        ("limInvalid:-1", "`limited long` (maxLength) with an unparseable value returns its -1 default, not 0"),
        ("limOverflow:-1", "`limited long` that overflows also returns the -1 default"),
        ("limOk:5", "a valid `limited long` parses through"),
        ("clamp:1000", "`clamped unsigned long`: an out-of-range colspan clamps to the max (1000), it does not fall back"),
        ("tiVal:0", "`tabindex=\"-0\"` parses to +0 per the HTML integer rules"),
        ("tiNeg:false", "…and the getter must not leak a NEGATIVE zero — `Object.is` distinguishes it and every `-0` subtest fails on it"),
        ("wNeg:false", "an unsigned attribute set to \"-0\" is +0 too, never -0"),
    ] {
        assert!(
            got.contains(claim),
            "G_REFLECT_NUMERIC: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
