//! **G_CSS_UTF8 — every stylesheet in the engine was mojibake'd, by one character of Rust.**
//!
//! `strip_comments` runs on the way IN to `Stylesheet::parse`, and it walked the source as BYTES:
//!
//! ```ignore
//! out.push(b[i] as char);   // identity for ASCII; Latin-1 widening for everything else
//! ```
//!
//! So `–` (U+2013, UTF-8 `E2 80 93`) became the three characters `â€“`. `Stylesheet::parse` stores
//! that string as `source`, and `source` is handed verbatim to `StyloStylesheet::from_str` — so
//! **Stylo never saw a correctly-decoded stylesheet.** The DOM was fine the whole time
//! (`style.textContent` read back U+2013 exactly), which is why this hid: the corruption happened
//! between a correct DOM and the cascade.
//!
//! Found on `255md.com`, whose list markers are `li::before { content: "–" }`: we drew `â` glued to
//! each bullet where Chrome draws an en dash.
//!
//! ## Why this is much bigger than one bullet
//!
//! * every non-ASCII `content:` string — arrows, checkmarks, quotes, currency, the icon glyphs half
//!   the web puts in `::before`;
//! * **`font-family` names written in their own script** — `"微软雅黑"`, `"ヒラギノ角ゴ"`,
//!   `"맑은 고딕"`. A mangled family name matches no font, so an entire CJK font stack silently
//!   falls through to a default. That is a large share of the CrUX tail this corpus is stratified
//!   to reach.
//! * custom properties, `quotes:`, non-ASCII identifiers, `url()` with a non-ASCII path, and
//!   attribute selectors matching non-ASCII values.
//!
//! ## Why it survived this long, which is the transferable part
//!
//! **The escape form was never affected.** `content: "\2013"` is pure ASCII, so it always worked —
//! and every CSS test in this repo was written in ASCII. A bug that is invisible to every test in
//! the file is not caught by adding more tests of the same kind; it is caught by a test whose INPUT
//! leaves the alphabet the tests were written in. This gate is therefore deliberately written in
//! four scripts.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><head><meta charset="UTF-8"><style>
  /* a comment containing 日本語 and – must still be stripped whole */
  #f { font-family: "微软雅黑", "ヒラギノ角ゴ", "맑은 고딕", sans-serif; }
  #v { --dash: "–"; --arrow: "→"; --emoji: "🌍"; }
  #a[data-x="café"] { color: rgb(1, 2, 3); }
  #b { color: rgb(4, 5, 6) }/* comment flush against a rule */#c { color: rgb(7, 8, 9) }
  #d { --tight: "é"/* comment abutting a multi-byte char */; color: rgb(10, 11, 12) }
  #e { --esc: "\2013"; }
</style></head><body>
  <div id="f">f</div><div id="v">v</div><div id="a" data-x="café">a</div>
  <div id="b">b</div><div id="c">c</div><div id="d">d</div><div id="e">e</div>
  <div id="cssom">-</div>
  <div id="out">-</div>
  <script>
    var R = [], g = getComputedStyle, $ = function (i) { return document.getElementById(i); };
    // ── ⚠⚠⚠ THE RETURN DIRECTION, WHICH THIS GATE DID NOT ASK ABOUT FOR TWO HUNDRED TICKS (t1352).
    //    Everything above measures the way IN — a stylesheet decoded before Stylo sees it. The way
    //    OUT had the SAME defect at a different seam: `JS_NewStringCopyZ` reads its input as
    //    Latin-1, one byte per character, and the CSSOM natives handed it a Rust UTF-8 `String`.
    //    Reported as CODE POINTS, for the same reason every row above is.
    (function () {
      var C = [];
      function cps(str) { var o = []; for (var i = 0; i < str.length; i++) { o.push(str.charCodeAt(i)); } return o.join(','); }
      var d1 = document.createElement('div'); d1.style.fontFamily = '\u7d20\u8c61';
      C.push('setget:' + cps(d1.style.fontFamily));
      var d2 = document.createElement('div'); d2.style.setProperty('font-family', '\u00e9x');
      C.push('getprop:' + cps(d2.style.getPropertyValue('font-family')));
      var d3 = document.createElement('div'); d3.style.cssText = 'font-family:\u7d20';
      C.push('csstext:' + cps(d3.style.fontFamily));
      var d4 = document.createElement('div'); d4.style.fontFamily = 'simple';
      C.push('ascii:' + d4.style.fontFamily);
      var d5 = document.createElement('div'); d5.setAttribute('title', '\u7d20');
      C.push('attr:' + cps(d5.getAttribute('title')));
      $('cssom').textContent = C.join(' ');
    })();
    // Report the >127 code points, not the rendered text: a mojibake'd string still *renders*
    // something, and eyeballing it is how this survived. The code points cannot be misread.
    function hi(s) {
      var o = [];
      for (var i = 0; i < s.length; i++) { var c = s.charCodeAt(i); if (c > 127) { o.push(c); } }
      return o.join(',');
    }

    // ── 1. font-family in its own script. THE CJK claim: a mangled family name matches no font.
    R.push('ff:' + hi(g($('f')).fontFamily));

    // ── 2. custom properties, which is the most direct read of what the cascade actually stored.
    R.push('dash:' + hi(g($('v')).getPropertyValue('--dash')));
    R.push('arrow:' + hi(g($('v')).getPropertyValue('--arrow')));
    // An astral character is TWO UTF-16 units and FOUR UTF-8 bytes — the byte-widening bug turns it
    // into four, so this distinguishes "decoded correctly" from "happened to be short".
    R.push('emoji:' + hi(g($('v')).getPropertyValue('--emoji')));

    // ── 3. an attribute selector whose VALUE is non-ASCII must still match.
    R.push('attrsel:' + g($('a')).color);

    // ── 4. THE COMMENT LOGIC ITSELF, which is what the byte walk was there for. These are the
    // regression guards: the fix changed what gets copied, and must not change what gets skipped.
    R.push('beforeC:' + g($('b')).color);           // rule ending flush against a comment
    R.push('afterC:' + g($('c')).color);            // rule starting flush after one
    R.push('abut:' + hi(g($('d')).getPropertyValue('--tight')));  // comment abutting a multi-byte char
    R.push('abutColor:' + g($('d')).color);
    // ...and the comment full of non-ASCII at the top must be GONE, not emitted as a rule.
    R.push('commentGone:' + (g($('f')).color !== 'rgb(1, 2, 3)'));

    // ── 5. The form that always worked must keep working — the fix must not trade one for the
    // other. A custom property keeps its RAW token sequence, so the escape stays as the six ASCII
    // characters it was written as; that is the correct answer and it is also the reason this whole
    // bug hid — nothing about `\2013` ever left the alphabet the tests were written in.
    R.push('esc:[' + g($('e')).getPropertyValue('--esc').replace(/\s+/g, '') + ']');

    document.getElementById('out').textContent = R.join(' ');
  </script>
</body></html>"#;

#[test]
fn stylesheets_are_decoded_as_utf8_not_widened_byte_by_byte() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://utf8.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        // 微软雅黑 · ヒラギノ角ゴ · 맑은 고딕 — the exact code points, in order.
        "ff:24494,36719,38597,40657,12498,12521,12462,12494,35282,12468,47569,51008,44256,46357",
        // – → 🌍 (the emoji is a surrogate pair: 55356,57101)
        "dash:8211",
        "arrow:8594",
        "emoji:55356,57101",
        // café in an attribute selector
        "attrsel:rgb(1, 2, 3)",
        // the comment logic, unchanged
        "beforeC:rgb(4, 5, 6)",
        "afterC:rgb(7, 8, 9)",
        "abut:233",
        "abutColor:rgb(10, 11, 12)",
        "commentGone:true",
        // the ASCII escape form, which always worked and must keep working
        r#"esc:["\2013"]"#,
    ] {
        assert!(
            got.contains(claim),
            "G_CSS_UTF8: expected `{claim}`\n  got: {got}\n\n  \
             `strip_comments` walked the stylesheet as BYTES and emitted `b[i] as char`, widening \
             every UTF-8 byte into its own Latin-1 code point — so `–` reached Stylo as `â€“` and \
             NO stylesheet in this engine was ever correctly decoded. `255md.com` drew `â` for each \
             list bullet. `ff:` is the expensive one: a mangled `font-family: \"微软雅黑\"` matches \
             no font, so an entire CJK font stack falls through to a default with nothing logged. \
             The escape form (`esc:`) always worked, which is exactly why this hid — every CSS test \
             in this repo was written in ASCII, and a bug invisible to the whole alphabet your \
             tests use is not found by writing more of them."
        );
    }

    // ── ⭐⭐⭐ THE RETURN DIRECTION (t1352). The same Latin-1 widening, at the other boundary, found
    //    two hundred ticks after this file documented the mechanism in its own opening lines. It
    //    said `out.push(b[i] as char)` was *"identity for ASCII; Latin-1 widening for everything
    //    else"* — and nobody asked whether the way OUT did the same thing. It did:
    //    `JS_NewStringCopyZ` reads its input as Latin-1 and SEVEN natives handed it Rust UTF-8.
    //
    //    ```text
    //                                             expected            before
    //      style.fontFamily = 素象                32032,35937   231,180,160,232,177,161
    //      setProperty('font-family','éx')        233,120       195,169,120
    //      cssText = 'font-family:素'             32032         231,180,160
    //      style.fontFamily = 'simple'  CONTROL   simple        simple
    //      setAttribute('title','素')   CONTROL   32032         32032
    //    ```
    let cssom = {
        let n = manuk_css::query_selector_all(page.dom(), root, "#cssom")[0];
        page.dom().text_content(n)
    };
    for (claim, why) in [
        (
            "setget:32032,35937",
            "⭐ THE LOAD-BEARING ROW. `el.style.fontFamily = '素象'` must read back the SAME TWO code \
             points. `231,180,160,232,177,161` is that string's UTF-8 BYTES widened one per \
             character — mojibake, not loss, so nothing throws and the value is not empty: the CSSOM \
             answers a string in the wrong alphabet, which renders as something and reads as working",
        ),
        (
            "getprop:233,120",
            "…and through `setProperty`/`getPropertyValue`, the other spelling of the same seam. \
             `195,169,120` is `é` as its two UTF-8 bytes",
        ),
        (
            "csstext:32032",
            "…and through `cssText`, which reaches the seam via the CSS parser rather than the \
             property setter. Three spellings, one boundary — a fix at a single call site passes one \
             of these rows and fails the others",
        ),
        (
            "ascii:simple",
            "⚠ THE CONTROL, AND THE REASON THIS SURVIVED: ASCII through the same seam is IDENTITY, \
             so every CSS test in this repo — all written in ASCII — passed straight over the \
             defect. It is the same sentence the assertion above already carries, one boundary over",
        ),
        (
            "attr:32032",
            "⚠ THE SECOND CONTROL: the DOM seam was CORRECT all along, because it goes through \
             `return_string`/`to_jsval`. Without this row, a fix that mangled both directions equally \
             would look symmetric and pass",
        ),
    ] {
        assert!(
            cssom.contains(claim),
            "G_CSS_UTF8: expected `{claim}`\n  got: {cssom}\n\n  {why}."
        );
    }
}
