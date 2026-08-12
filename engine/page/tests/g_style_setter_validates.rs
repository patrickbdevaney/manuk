//! **G_STYLE_SETTER_VALIDATES — `el.style.color = "yelow"` does not stick.**
//!
//! `element.style` is a `Proxy` over the `style` attribute text, and its `set` trap wrote
//! `String(v)` straight in. Nothing was validated, ever. CSSOM says a declaration whose value does
//! not parse is simply **not set**; we stored anything.
//!
//! ## Why this is a capability bug and not a conformance nicety
//!
//! It breaks the feature-detection idiom that essentially every CSS-touching library on the web
//! ships, in the direction that costs the most:
//!
//! ```js
//! const e = document.createElement('div');
//! e.style[prop] = value;
//! return e.style[prop] !== '';        // TRUE FOR EVERY VALUE, ALWAYS
//! ```
//!
//! Every probe answered *supported*, so a page took the modern branch for capabilities this engine
//! does not have — and threw away the fallback it had shipped for exactly that case. It is the
//! mirror of t1172's `'display' in el.style === false`, which is the same object answering
//! *unsupported* for everything we DO have: **one object, two detection idioms, both lying, in
//! opposite directions.** t1172 fixed one; this fixes the other.
//!
//! In WPT it is `test_invalid_value` — 1,978 call sites across `~/wpt/css`, each asserting that an
//! invalid declaration leaves `getPropertyValue` empty.
//!
//! ## What the negative and control rows are for
//!
//! The obvious over-correction is to validate *everything*, which deletes working declarations:
//!
//!   * a **custom property** (`--brand: whatever`) has no grammar to check against — validating one
//!     would delete every design token on the page;
//!   * `setProperty(k, v, 'important')` must still work, because it is the ONLY path to a priority;
//!   * the **IDL setter** must reject a value carrying `!important` (measured against Chrome at
//!     t1177 — the one row in that battery whose outcome does not track `CSS.supports`);
//!   * an empty value is `removeProperty`, not a declaration to validate;
//!   * `el.setAttribute('style', …)` is a DIFFERENT path and is deliberately untouched here.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
  <div id="d">hi</div><div id="out">-</div><script>
  var r = [];
  var d = document.getElementById('d');
  var s = d.style;

  // ── CONTROL ROWS FIRST — everything that already worked and must not move. ─────────────
  s.color = 'red';                       r.push('ctl-set:' + s.color);
  s.marginLeft = '11px';                 r.push('ctl-camel:' + s.marginLeft);
  s.setProperty('padding-top', '3px');   r.push('ctl-sp:' + s.getPropertyValue('padding-top'));
  s.setProperty('z-index', '4', 'important');
  r.push('ctl-imp:' + s.getPropertyValue('z-index') + '/' + s.getPropertyPriority('z-index'));
  s.setProperty('--brand', 'whatever you like');
  r.push('ctl-custom:' + s.getPropertyValue('--brand'));
  s['--tok'] = '#bada55';                r.push('ctl-custom2:' + s.getPropertyValue('--tok'));
  s.color = '';                          r.push('ctl-empty:' + (s.color === '' ? 'removed' : s.color));
  s.color = 'red';
  // A value that needs the modern colour syntax we DO have.
  s.backgroundColor = 'rgb(1 2 3 / 50%)'; r.push('ctl-modern:' + (s.backgroundColor !== ''));
  // `setAttribute` is a different path and stays a raw write — narrowing the fix to the
  // CSSOM setter is deliberate, and this row says so out loud.
  d.setAttribute('style', 'color: yelow');
  r.push('ctl-attr:' + (d.getAttribute('style').indexOf('yelow') >= 0));
  d.setAttribute('style', '');

  // ── THE SUBJECT — an invalid value must NOT be stored. ─────────────────────────────────
  s.color = 'blue';
  s.color = 'yelow';                     r.push('bad-keyword:' + s.color);
  s.setProperty('width', 'banana');      r.push('bad-sp:' + s.getPropertyValue('width'));
  s.color = 'rgb(255 0)';                r.push('bad-fn:' + s.color);
  s.zIndex = 'abc';                      r.push('bad-int:' + s.zIndex);
  s.notaproperty = 'x';                  r.push('bad-prop:' + (s.notaproperty === '' ? 'dropped' : s.notaproperty));
  // The IDL path forbids a priority; `setProperty(k,v,'important')` is the only way.
  s.setProperty('top', '1px');
  s.top = '2px !important';              r.push('bad-imp:' + s.getPropertyValue('top'));

  // ── THE IDIOM THIS EXISTS FOR — feature detection must now DISCRIMINATE. ───────────────
  var probe = document.createElement('div');
  var det = function (p, v) { probe.style[p] = ''; probe.style[p] = v; return probe.style[p] !== ''; };
  r.push('det-real:' + det('color', 'red'));
  r.push('det-fake:' + det('color', 'yelow'));
  r.push('det-modern:' + det('display', 'flex'));
  r.push('det-absent:' + det('display', 'run-in'));

  // ── THE MEMO MUST NOT LEAK ACROSS PROPERTIES — same value, two properties. ─────────────
  var p2 = document.createElement('div');
  p2.style.color = 'red';                 // caches ("color red" -> true)
  p2.style.width = 'red';                 // must NOT be served from that entry
  r.push('memo:' + (p2.style.width === '' ? 'ok' : p2.style.width));

  document.getElementById('out').textContent = r.join(' ');
  </script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn the_style_setter_drops_a_declaration_that_does_not_parse() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://setter.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("STYLE SETTER: {got}");

    for (claim, why) in [
        // ── CONTROL: the surface that already worked.
        ("ctl-set:red", "an ordinary valid declaration still round-trips"),
        ("ctl-camel:11px", "the camelCase IDL attribute is untouched"),
        ("ctl-sp:3px", "`setProperty` with a valid value still sets"),
        (
            "ctl-imp:4/important",
            "**`setProperty(k, v, 'important')` IS THE ONLY PATH TO A PRIORITY** and must keep \
             working. Validating the value must not swallow the priority argument with it",
        ),
        (
            "ctl-custom:whatever you like",
            "**A CUSTOM PROPERTY HAS NO GRAMMAR TO VALIDATE AGAINST.** `--brand: whatever you like` \
             is a valid declaration; running it through `CSS.supports` would delete every design \
             token on the page, which is the over-correction this row exists to catch",
        ),
        ("ctl-custom2:#bada55", "…through the IDL setter path too, not just `setProperty`"),
        (
            "ctl-empty:removed",
            "an empty value is `removeProperty`, not a declaration to validate — a validator that \
             ran first would answer false for `''` and turn removal into a no-op",
        ),
        (
            "ctl-modern:true",
            "space-separated `rgb()` with a slash alpha is CSS Color 4 syntax this engine really \
             does parse. If the fix rejected it, the validator would be denying what we render",
        ),
        (
            "ctl-attr:true",
            "**`setAttribute('style', …)` IS A DIFFERENT PATH AND IS DELIBERATELY UNTOUCHED.** The \
             attribute text is the source of truth this Proxy reads; dropping invalid declarations \
             there is a separate, larger job (it needs a per-property serializer). Narrowing the \
             fix is a choice, and this row records it rather than leaving it to be discovered",
        ),
        // ── SUBJECT: the lie itself, on five shapes of invalid value.
        (
            "bad-keyword:blue",
            "**THE HEADLINE.** `e.style.color = 'yelow'` must leave the PREVIOUS value standing. \
             Note the assertion is `blue`, not `''` — a validator that cleared the slot before \
             deciding would pass an `''` test while still destroying a working declaration",
        ),
        ("bad-sp:", "`setProperty('width','banana')` sets nothing"),
        (
            "bad-fn:blue",
            "`rgb(255 0)` is well-formed FUNCTION syntax with the wrong argument count — the shape \
             that a regex-based validator waves through",
        ),
        ("bad-int:", "`z-index: abc` is not an `<integer>`"),
        ("bad-prop:dropped", "an unknown PROPERTY is dropped, not just an unknown value"),
        (
            "bad-imp:1px",
            "the IDL setter REJECTS a value carrying `!important`, leaving the previous declaration \
             — measured against Chrome at t1177, and the one row in that battery whose outcome does \
             NOT track `CSS.supports` (which says true for `color: red !important`)",
        ),
        // ── THE IDIOM: detection must now discriminate, in BOTH directions.
        (
            "det-real:true",
            "**THE POINT OF THE WHOLE TICK, POSITIVE HALF.** The library idiom \
             `e.style[p]=v; return e.style[p]!==''` must still say YES for what we support — a fix \
             that made it say no for everything would be the t1172 lie in a new place",
        ),
        (
            "det-fake:false",
            "…and NO for what we do not. Before this tick it answered YES to every value ever \
             passed, so a page probing for a capability we lack was told yes and dropped its \
             fallback",
        ),
        ("det-modern:true", "`display: flex` — a real capability, still detected"),
        (
            "det-absent:false",
            "`display: run-in` is valid CSS that this engine does not implement. It is one of the \
             425 valid-in-Chrome declarations the priced corpus says we decline, and declining it \
             is CORRECT: `CSS.supports` is a question about THIS engine (t1180)",
        ),
        (
            "memo:ok",
            "**THE CACHE IS KEYED ON THE PAIR, NOT THE VALUE.** `color: red` is valid and \
             `width: red` is not; a memo keyed on the value alone would serve the first answer to \
             the second question and silently accept it. The memo exists because `__cssSupports` \
             parses a Stylo stylesheet and `el.style.x = …` is a per-frame path — buying \
             conformance with a per-assignment stylesheet parse would be a trade the ratchet refuses",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_STYLE_SETTER_VALIDATES: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
