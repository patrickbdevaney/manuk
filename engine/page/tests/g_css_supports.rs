//! **G_CSS_SUPPORTS — `CSS.supports()` answers from the CSS engine, and agrees with `@supports`.**
//!
//! **The failure this gate exists for.** `CSS.supports` was `function () { return true; }`. Not
//! "approximately right" — it returned `true` for `notaproperty: 1`, for `color`, for `width: 10zz`
//! and for the bare string `": "`. Measured before the fix: **21 of 21 probe cases yes**, including
//! every piece of nonsense in the list.
//!
//! That is the worst available answer, and worse than no API at all. Progressive enhancement is
//! built on this call: a page asks whether a property works and, on "yes", *hides its fallback* and
//! commits to the modern path. So a page asking `CSS.supports('container-type: inline-size')` was
//! told yes, threw away the layout its author had shipped and tested, and rendered the enhanced
//! branch against a property this engine ignores entirely. A "no" would have left it looking right.
//!
//! **What made it a bug rather than a gap: the engine already knew the answer.** `@supports` has
//! been honest since tick 276, because the cascade asks Stylo and Stylo really parses the
//! condition. Measured, before this tick, on the *identical* declarations:
//!
//! | condition | `@supports` (Stylo) | `CSS.supports` (JS) |
//! |---|---|---|
//! | `display: grid` | applies | true |
//! | `notaproperty: 1` | does not apply | **true** |
//! | `container-type: inline-size` | does not apply | **true** |
//!
//! Two sources of truth for one question, which is this project's dominant bug class — and the JS
//! one was wrong in the direction that costs a page its layout. So `agree_*` are the claims that
//! carry this gate: each asks the same condition **both ways** and asserts the answers match. They
//! would fail against any independent reimplementation that drifted, which is exactly what a
//! hand-maintained list of supported properties becomes the first time the engine gains or loses
//! one.
//!
//! **`compound`** is the evidence that the real evaluator is being reached rather than imitated.
//! `and` / `or` / `not` were never implemented here; they work because the condition is handed to
//! Stylo's own parser. A lookup table would have had to grow a boolean-expression parser to pass
//! this, and would still not be the same evaluator the cascade uses.
//!
//! RED: restoring `return true` flips `unimpl`, `nonsense`, `notadecl`, `compound_false`,
//! `twoarg_bogus`, `twoarg_badval` and both negative `agree_*` claims while every positive claim
//! still passes — the shape of the original bug, which said yes to everything and was therefore
//! *right whenever the answer happened to be yes*. Answering a flat `false` instead flips the
//! positive claims and leaves the negative ones green, which is why both directions are asserted.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html>
<html><body>
  <style>
    #s1,#s2,#s3,#s4 { color: rgb(1, 1, 1); }
    @supports (display: grid)               { #s1 { color: rgb(2, 2, 2); } }
    @supports (notaproperty: 1)             { #s2 { color: rgb(2, 2, 2); } }
    @supports (container-type: inline-size) { #s3 { color: rgb(2, 2, 2); } }
    @supports (position: sticky)            { #s4 { color: rgb(2, 2, 2); } }
  </style>
  <div id="s1">1</div><div id="s2">2</div><div id="s3">3</div><div id="s4">4</div>
  <div id="out">-</div>
  <script>
    var R = { a: [], push: function (s) { this.a.push(s);
      var o = document.getElementById('out'); if (o) { o.textContent = this.a.join(' '); } } };

    try {
      var every = function (list, want) {
        for (var i = 0; i < list.length; i++) { if (CSS.supports(list[i]) !== want) { return false; } }
        return true;
      };

      // Properties this engine really honours.
      R.push('impl:' + every(['display: flex', 'display: grid', 'color: red',
                              'position: sticky', 'gap: 10px'], true));

      // Real CSS properties this engine does NOT implement. Saying yes to these is what threw
      // away the page's fallback.
      R.push('unimpl:' + every(['container-type: inline-size', 'view-transition-name: foo',
                                'animation-timeline: scroll()', 'anchor-name: --a'], false));

      // Outright nonsense — a property that does not exist, and a value that is not valid for a
      // property that does.
      R.push('nonsense:' + every(['notaproperty: 1', 'color: notacolor',
                                  'display: notavalue', 'width: 10zz'], false));

      // Not a declaration at all. `return true` said yes to both of these.
      R.push('notadecl:' + every(['color', ': '], false));

      // Compound conditions — never implemented here; they work only because the condition
      // reaches Stylo's own parser.
      R.push('compound:' + every(['(display: flex)',
                                  '(display: flex) and (color: red)',
                                  'not (notaprop: 1)',
                                  '(notaprop: 1) or (display: flex)'], true));
      R.push('compound_false:' + every(['(display: flex) and (notaprop: 1)',
                                        'not (display: flex)'], false));

      // The two-argument form.
      R.push('twoarg:' + (CSS.supports('display', 'flex') === true));
      R.push('twoarg_bogus:' + (CSS.supports('notaproperty', '1') === false));
      R.push('twoarg_badval:' + (CSS.supports('color', 'notacolor') === false));

      // ── The claims that carry the gate: the SAME question, asked both ways, must get the
      // same answer. `applies` reads what the cascade actually did with `@supports`.
      var applies = function (id) {
        return getComputedStyle(document.getElementById(id)).color === 'rgb(2, 2, 2)';
      };
      R.push('agree_grid:'   + (applies('s1') === CSS.supports('display: grid')));
      R.push('agree_bogus:'  + (applies('s2') === CSS.supports('notaproperty: 1')));
      R.push('agree_unimpl:' + (applies('s3') === CSS.supports('container-type: inline-size')));
      R.push('agree_sticky:' + (applies('s4') === CSS.supports('position: sticky')));

      // ...and the agreement must not be the trivial one where both sides always say the same
      // thing. Without this, an engine answering `false` to everything AND applying no
      // `@supports` block would satisfy every `agree_*` claim above.
      R.push('agree_nontrivial:' + (applies('s1') === true && applies('s2') === false));
    } catch (e) {
      R.push('THREW:' + e);
    }
  </script>
</body></html>"##;

/// One test in the binary — a second `Page::load` stands up a second SpiderMonkey context in the
/// same process and does not survive teardown (see `g_mse`, `g_web_worker`, `g_globals`).
#[test]
fn css_supports_answers_from_the_css_engine_and_agrees_with_at_supports() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://supports.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("impl:true", "properties the engine really honours must report true, or every page loses an enhancement it could have had"),
        ("unimpl:true", "real properties the engine ignores must report FALSE — saying yes is what makes a page hide the fallback it shipped and render an enhanced branch that cannot work"),
        ("nonsense:true", "an unknown property, and an invalid value for a known one, are both unsupported; `return true` said yes to both"),
        ("notadecl:true", "`color` and `: ` are not declarations and cannot be supported"),
        ("compound:true", "`and`/`or`/`not` conditions must evaluate — they work only because the condition reaches the real parser, so this is the evidence that the engine is being ASKED rather than imitated"),
        ("compound_false:true", "a compound condition containing an unsupported term must be false; `or`/`not` must not degrade into always-true"),
        ("twoarg:true", "the two-argument form `supports(property, value)` must work"),
        ("twoarg_bogus:true", "the two-argument form must reject an unknown property"),
        ("twoarg_badval:true", "the two-argument form must reject an invalid value"),
        ("agree_grid:true", "`CSS.supports('display: grid')` must agree with what `@supports (display: grid)` did — one question, one answer"),
        ("agree_bogus:true", "the JS and CSS halves must agree that nonsense is unsupported; before this tick the cascade declined it and the JS API said yes"),
        ("agree_unimpl:true", "the JS and CSS halves must agree about `container-type` — the exact declaration where they disagreed, and the one that costs a page its layout"),
        ("agree_sticky:true", "agreement must hold for a supported property too, not only for rejections"),
        ("agree_nontrivial:true", "the agreement must be non-trivial: `@supports` really applies for grid and really does not for nonsense, so the matching answers are not two constants lining up"),
    ] {
        assert!(
            got.contains(claim),
            "G_CSS_SUPPORTS: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
