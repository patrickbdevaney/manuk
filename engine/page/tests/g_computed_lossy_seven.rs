//! # G_COMPUTED_LOSSY_SEVEN — seven properties the cascade resolved and the CSSOM would not say
//!
//! Tick 1214's census asked `getComputedStyle` for **215 properties** and found **107 silent**, then
//! split them mechanically against `ComputedStyle`'s own fields:
//!
//! ```text
//!   LOSSY   the cascade resolves it and the serializer omits it      15
//!   HONEST  not in `ComputedStyle` at all — we do not model it       92
//! ```
//!
//! Silence is the **correct** answer for a property this engine does not implement. It is a defect
//! only for the fifteen, and this gate closes the seven of those that carry real-web weight and have
//! no recorded reason to stay silent.
//!
//! ## Why these seven
//!
//! * **`rotate` / `scale` / `translate`** — the *individual* transform properties. Every animation
//!   library reads them before it animates, and this project already owns the scar:
//!   `undefined + ' scale(2)'` is the string `"undefined scale(2)"`, which is `G_TRANSFORM`'s whole
//!   reason for existing. **The same failure, four properties over.**
//! * **`transform-origin`** — read by every library that sets one.
//! * **`align-content` / `justify-items` / `justify-self`** — the flex and grid alignment reads.
//!
//! ## ⚠ What is deliberately NOT here
//!
//! The other eight lossy names. **Six are the grid family** (`grid-template-*`, `grid-auto-*`), whose
//! silence is recorded as deliberate at t1171-74: Chrome reports the **used** track sizes, so echoing
//! what the cascade holds would be *a wrong answer of the right type* — worse than the silence.
//! `background-position` and `tab-size` are the remaining two and are left for a tick that can price
//! their serialization forms rather than guess them.
//!
//! ## ⚠⚠ `transform-origin` RESOLVES ITS PERCENTAGES, and that is the claim a naive fix fails
//!
//! The initial value is `50% 50%` and Chrome reports it in **used pixels** — half the border box. A
//! serializer that echoed `50% 50%` would have added a property and kept the defect, which is this
//! file's recurring shape. `originResolvesToPx` below is that claim: on a 200×100 box the answer is
//! `100px 50px`, not `50% 50%`.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  #b { width: 200px; height: 100px; transform-origin: 50% 50%; }
  #t { rotate: 45deg; scale: 2; translate: 10px 20px; }
  #u { scale: 2 3; translate: 5px; }
  #f { display: flex; align-content: space-between; justify-items: center; }
  #f > i { justify-self: flex-end; }
</style></head><body>
  <div id="b"></div><div id="t"></div><div id="u"></div>
  <div id="f"><i id="i">x</i></div>
  <div id="plain"></div>
  <div id="out">-</div>
  <script>
    var R = [];
    function p(s) { R.push(s); document.getElementById('out').textContent = R.join(' '); }
    function g(id, prop) { return getComputedStyle(document.getElementById(id))[prop]; }

    // ── 1. ⚠ THE CLAIM A NAIVE FIX FAILS: percentages resolve against the box.
    p('originResolvesToPx:' + g('b', 'transformOrigin'));

    // ── 2. THE INDIVIDUAL TRANSFORM PROPERTIES — `undefined` before this, which is how
    //    `undefined + ' scale(2)'` becomes the string "undefined scale(2)".
    p('rotate:' + g('t', 'rotate'));
    p('scale:' + g('t', 'scale'));
    p('translate:' + g('t', 'translate'));
    p('scaleXY:' + g('u', 'scale'));
    p('translateX:' + g('u', 'translate'));

    // ── 3. THE INITIAL VALUES — `none`, not the empty string. A property that answers `""` is
    //    indistinguishable from one the engine does not support, which is the whole census.
    p('rotateNone:' + g('plain', 'rotate'));
    p('scaleNone:' + g('plain', 'scale'));
    p('translateNone:' + g('plain', 'translate'));

    // ── 4. THE ALIGNMENT READS.
    p('alignContent:' + g('f', 'alignContent'));
    p('justifyItems:' + g('f', 'justifyItems'));
    p('justifySelf:' + g('i', 'justifySelf'));
    p('justifySelfAuto:' + g('plain', 'justifySelf'));

    // ── 5. REACHABLE BY BOTH SPELLINGS. `getPropertyValue('align-content')` and `.alignContent`
    //    are one value; a fix that adds only the camelCase key leaves every dashed read silent.
    p('dashed:' + getComputedStyle(document.getElementById('f')).getPropertyValue('align-content'));
    p('dashedOrigin:' + getComputedStyle(document.getElementById('b')).getPropertyValue('transform-origin'));

    // ── 6. THE RATCHET — the neighbours these were spliced in beside must be unmoved.
    p('alignItems:' + g('f', 'alignItems'));
    p('justifyContent:' + g('f', 'justifyContent'));
    p('display:' + g('f', 'display'));
    p('width:' + g('b', 'width'));
  </script>
</body></html>"##;

#[test]
fn the_seven_lossy_properties_report_what_the_cascade_resolved() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://lossy.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("LOSSY-SEVEN: {got}");

    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_COMPUTED_LOSSY_SEVEN: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}

const CLAIMS: &[(&str, &str)] = &[
    (
        "originResolvesToPx:100px 50px",
        "⚠ THE CLAIM A NAIVE FIX FAILS. `transform-origin: 50% 50%` on a 200×100 box resolves to \
         `100px 50px` — Chrome reports USED pixels. A serializer that echoed `50% 50%` would have \
         added the property and kept the defect, which is this file's recurring shape",
    ),
    (
        "rotate:45deg",
        "THE LOAD-BEARING FAMILY. The individual transform properties were `undefined`, and \
         `undefined + ' scale(2)'` is the string `\"undefined scale(2)\"` — `G_TRANSFORM`'s whole \
         reason for existing, four properties over",
    ),
    ("scale:2", "`scale: 2` — an equal second component is omitted, as Chrome does"),
    ("translate:10px 20px", "both components when they differ"),
    ("scaleXY:2 3", "and both when they differ"),
    (
        "translateX:5px",
        "a single-component `translate` keeps one component — the second is an implicit zero and \
         Chrome does not print it",
    ),
    (
        "rotateNone:none",
        "⚠ THE INITIAL VALUE IS `none`, NOT `\"\"`. A property answering the empty string is \
         indistinguishable from one the engine does not support — which is exactly what the t1214 \
         census measured, and adding the property without its initial value would leave the \
         detection broken for every element that has not set it",
    ),
    ("scaleNone:none", "same"),
    ("translateNone:none", "same"),
    ("alignContent:space-between", "the flex/grid alignment reads"),
    ("justifyItems:center", "same"),
    ("justifySelf:flex-end", "same, on the item"),
    (
        "justifySelfAuto:auto",
        "`justify-self`'s initial defers to the container, and its resolved value is `auto` — not \
         the empty string, and not the container's value",
    ),
    (
        "dashed:space-between",
        "⚠ BOTH SPELLINGS. `getPropertyValue('align-content')` and `.alignContent` are ONE value; a \
         fix that adds only the camelCase key leaves every dashed read silent, and the dashed form \
         is what the census itself asked with",
    ),
    ("dashedOrigin:100px 50px", "and the dashed form of the resolved origin"),
    (
        "alignItems:stretch",
        "THE RATCHET. The neighbours these were spliced in beside — a positional format template \
         where one misplaced value shifts every property after it",
    ),
    ("justifyContent:normal", "THE RATCHET"),
    ("display:flex", "THE RATCHET"),
    ("width:200px", "THE RATCHET"),
];
