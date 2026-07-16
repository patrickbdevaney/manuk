//! **G_CSS_NESTING — native CSS nesting (`&`) resolves through the cascade to computed style.**
//!
//! CSS Nesting (Baseline 2023) lets a stylesheet nest rules inside a parent rule, with `&` standing for
//! the parent selector. The tick-123 surface audit (#5) found this shipped, load-bearing primitive was
//! **absent from the capability map** — never measured, though Stylo backs it. This gate is the
//! measurement: it proves a nested rule actually reaches `getComputedStyle`, converting the map row from
//! `unknown` to a proven capability. Each assertion is one nesting form:
//!
//! * **nested descendant** — `.a { & .c { … } }` styles `.c` inside `.a`.
//! * **nested bare `&`** — `.d { & { … } }` restyles the element itself.
//! * **it is real cascade, not a fluke** — a plain `.a .b` descendant rule (the non-nested control) also
//!   resolves, so a pass means nesting joined the cascade rather than the whole probe misreporting.
//!
//! Own binary: two SpiderMonkey-backed `Page::load`s in one process reuse the JS runtime and can trip the
//! tracked reflector-teardown UAF (see the flexbox-relayout Bar-0 note). One JS gate = one process.
//!
//! **Falsifiable:** the asserted colours only appear if the *nested* rules apply. An engine that dropped
//! `:nesting` rules at parse (Stylo's `servo` build once did for other selectors) would leave `.c` and
//! `.d` at their defaults, and the assertions would read the wrong colour/weight — RED.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  .a { color: rgb(1, 2, 3); }
  .a .b { color: rgb(4, 5, 6); }
  .a { & .c { color: rgb(7, 8, 9); } }
  .d { color: rgb(10, 11, 12); & { font-weight: 700; } }
</style></head><body>
<div class="a"><span class="b" id="b">B</span><span class="c" id="c">C</span></div>
<div class="d" id="d">D</div>
<div id="out">-</div>
<script>
  var R = [];
  function ck(l, g) { R.push(l + ':' + g); }
  function cs(id, p) { return getComputedStyle(document.getElementById(id))[p]; }
  ck('bControl', cs('b', 'color'));      // non-nested descendant control
  ck('cNested', cs('c', 'color'));       // nested `.a { & .c {} }`
  ck('dNested', cs('d', 'fontWeight'));  // nested bare `& {}`
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn native_css_nesting_reaches_computed_style() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://nest.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "bControl:rgb(4, 5, 6)",
            "the non-nested `.a .b` descendant rule resolves (the control)",
        ),
        (
            "cNested:rgb(7, 8, 9)",
            "the nested `.a { & .c {} }` rule reaches computed style",
        ),
        (
            "dNested:700",
            "the nested bare `.d { & {} }` rule restyles the element itself",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_CSS_NESTING: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
