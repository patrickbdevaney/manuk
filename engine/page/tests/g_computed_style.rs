//! **G_COMPUTED_STYLE — getComputedStyle exposes ALREADY-COMPUTED properties, not `undefined`.**
//!
//! `computed_style_js` built a fixed ~30-property object and dropped several fields the cascade already
//! computes — `visibility`, `white-space`, `opacity`. A test (or real script) reading
//! `getComputedStyle(el).visibility` got **`undefined`**, not `"hidden"`. These are not new capabilities;
//! the values existed in `ComputedStyle` and were simply not surfaced to JS. Both the camelCase property
//! and `getPropertyValue('white-space')` must resolve.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<div id="v" style="visibility:hidden"></div>
<div id="w" style="white-space:pre-wrap"></div>
<div id="o" style="opacity:0.5"></div>
<div id="plain"></div>
<div id="f1" style="font-family:serif;font-size:16px"></div>
<div id="f2" style="font-family:serif;font-size:16px;line-height:1.5"></div>
<div id="f3" style="font-family:serif;font-size:16px;font-weight:bold"></div>
<div id="f4" style="font-family:serif;font-size:16px;font-weight:400;line-height:normal"></div>
<div id="f5" style="font-family:serif;font-size:16px;font-style:italic;font-weight:bold;line-height:2"></div>
<div id="q1" style="font-family:'New Century Schoolbook', serif"></div>
<div id="q2" style="font-family:'21st Century', fantasy"></div>
<div id="q3" style="font-family:my-font"></div>
<div id="q4" style="font-family:'素象'"></div>
<div id="q5" style="font-family:'a  b'"></div>
<div id="q6" style="font-family:'inherit'"></div>
<script>
  var R = [], cs = function (id) { return getComputedStyle(document.getElementById(id)); };
  R.push('vis:' + cs('v').visibility);                       // "hidden"
  R.push('ws:' + cs('w').whiteSpace);                        // "pre-wrap"
  R.push('wsPV:' + cs('w').getPropertyValue('white-space')); // "pre-wrap" via kebab accessor
  R.push('op:' + cs('o').opacity);                           // "0.5"
  R.push('visDflt:' + cs('plain').visibility);               // "visible" (initial), NOT undefined
  R.push('opDflt:' + cs('plain').opacity);                   // "1" (initial), NOT undefined
  // ── ⭐ THE `font` SHORTHAND'S COMPUTED VALUE (t1353). `ctx.font = getComputedStyle(el).font` is
  //    the one-liner every canvas text-measurement shim, chart library and autosizing input is
  //    built on, and it was `undefined`. CSS Fonts §2.6 order, defaults omitted; all rows
  //    Chrome-measured on `font-family: serif`.
  R.push('font1:[' + cs('f1').font + ']');
  R.push('font2:[' + cs('f2').font + ']');
  R.push('font3:[' + cs('f3').font + ']');
  R.push('font4:[' + cs('f4').font + ']');
  R.push('font5:[' + cs('f5').font + ']');
  R.push('fontType:' + (typeof cs('f1').font));
  R.push('fontPV:[' + cs('f2').getPropertyValue('font') + ']');
  // ── ⭐ FONT-FAMILY SERIALIZATION (t1354): a SEQUENCE of identifiers when it can be, a string
  //    when it cannot. Every row is the CSSOM rule that WPT encodes — see the gate's claims for
  //    where that deliberately parts company with measured Chrome.
  'q1 q2 q3 q4 q5 q6'.split(' ').forEach(function (i) {
    R.push(i + ':[' + cs(i).fontFamily + ']');
  });
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn getcomputedstyle_exposes_visibility_whitespace_opacity() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://cs.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("vis:hidden", "getComputedStyle(el).visibility must resolve the computed keyword, not undefined"),
        ("ws:pre-wrap", "…and whiteSpace — a property the cascade already computes and JS could not read"),
        ("wsPV:pre-wrap", "getPropertyValue('white-space') (kebab) must map to the same value"),
        ("op:0.5", "opacity serializes as a bare number, not undefined"),
        // ── ⭐ THE `font` SHORTHAND (t1353), every row Chrome-measured on `font-family: serif`:
        //
        //   font-size:16px                     16px serif
        //   …line-height:1.5                   16px / 24px serif   ← space-slash-space, USED px
        //   …font-weight:bold                  700 16px serif      ← NUMERIC, not the keyword
        //   …font-weight:400; line-height:normal  16px serif       ← both omitted
        //   …italic + bold + line-height:2     italic 700 16px / 32px serif
        (
            "font1:[16px serif]",
            "⭐ THE LOAD-BEARING ROW. `getComputedStyle(el).font` was `undefined`, so              `ctx.font = getComputedStyle(el).font` — the canvas text-measurement one-liner — set              the literal text `undefined` and every measurement silently used the 10px sans-serif              default. It also fails WPT's `font-computed.html` at its PRECONDITION, 309 subtests              before a value is compared",
        ),
        (
            "font2:[16px / 24px serif]",
            "a non-`normal` line-height joins the SIZE with ` / `, spaces included, and serializes              as the USED px (24, not `1.5`) — the size and line-height are ONE component, which is              why a naive `parts.join(' ')` of five fields puts the slash in the wrong place",
        ),
        (
            "font3:[700 16px serif]",
            "`font-weight: bold` computes to the NUMBER 700 — printing `bold` here is the shape of              a serializer that echoes the specified value instead of the computed one",
        ),
        (
            "font4:[16px serif]",
            "⚠ THE OMISSION ROW: an initial weight (400) and a `normal` line-height are both absent              from the shorthand, not printed as `400` or `/ normal`. Without this row a serializer              that always emits every component passes all three rows above",
        ),
        (
            "font5:[italic 700 16px / 32px serif]",
            "and all four together in CSS Fonts §2.6 order — style, weight, size/line-height,              family. Three components in one row is what catches an ORDERING bug that any              single-component row is blind to",
        ),
        (
            "fontType:string",
            "⚠ VACUITY GUARD: `undefined` stringifies into every row above as the literal text, so              `font1:[undefined]` would `contains`-match nothing and this row says why in one word",
        ),
        // ── ⭐⭐ FONT-FAMILY SERIALIZATION (t1354). CSSOM serializes a family name as a SEQUENCE OF
        //    IDENTIFIERS when every space-separated part is one, and as a STRING otherwise. The
        //    quoting in the SOURCE is not the question; whether the NAME can be spelled as
        //    identifiers is.
        (
            "q1:[New Century Schoolbook, serif]",
            "⚠⚠⚠ AND THIS IS ONE OF THE FEW PLACES THIS ENGINE DELIBERATELY DOES NOT MATCH MEASURED              CHROME. Headless Chrome returns `\"New Century Schoolbook\", serif` — it quotes every              multi-word family. WPT encodes the CSSOM rule and expects the identifiers back              UNQUOTED, and the first version of this fix followed Chrome and cost three net              subtests. It is a pure serialization detail with no capability behind it, so the metric              wins over the oracle. Written down so the next reader who probes Chrome and finds a              'divergence' does not fix it back",
        ),
        (
            "q2:[\"21st Century\", fantasy]",
            "…and the row that makes the rule a RULE rather than 'never quote': `21st` starts with              a digit, so it is not an identifier and the name must be a string. Unquoted it would              re-parse as something else entirely",
        ),
        (
            "q3:[my-font]",
            "a single valid identifier stays unquoted even though it was WRITTEN quoted — the              source's quoting is not the question",
        ),
        (
            "q4:[素象]",
            "non-ASCII is an ordinary identifier character, which is why the check is `>= U+0080`              and not an ASCII allow-list. Quoting this would be the Latin-1 reflex one layer up",
        ),
        (
            "q5:[\"a  b\"]",
            "⚠ TWO spaces. `split(' ')` yields an EMPTY middle part, and an empty string is not an \
             identifier, so the one condition rejects it. Worth a row because the first \
             implementation carried a SECOND, redundant guard for exactly this case and the mutation \
             pass could not make that guard go red — a green that cannot go red measured nothing. \
             The guard was deleted; this row is what covers the case now",
        ),
        (
            "q6:[\"inherit\"]",
            "a CSS-WIDE KEYWORD stays quoted whatever it looks like — unquoted it would not be a              family name at all, it would be `inherit`",
        ),
        (
            "fontPV:[16px / 24px serif]",
            "…and the same value through `getPropertyValue('font')`, which is the spelling the CSSOM              tests use and a separate lookup path from the camelCase property",
        ),
        (
            "visDflt:visible",
            "the INITIAL value must resolve too — `undefined` for an unset property is the bug, not a value",
        ),
        ("opDflt:1", "initial opacity is the number 1, serialized without trailing zeros"),
    ] {
        assert!(
            got.contains(claim),
            "G_COMPUTED_STYLE: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
