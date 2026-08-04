//! **G_COMPUTED_STYLE_PUBLISHES_THE_CASCADE — every property the cascade HOLDS must be readable, and
//! every property it does NOT hold must stay absent.**
//!
//! ⚠⚠⚠ **THE CLASS, NOT A MEMBER.** `getComputedStyle` had been failing one property per tick:
//! `transform` (applied for sixty ticks before the number reached JS), `width`/`height` (t897),
//! `zoom` and `containerType` (t900's surface audit). t901's constitution check named it as an **I3**
//! defect class — *the semantic model silently declining to publish what the pipeline already
//! computed* — and ranked the enumeration over the next member. One diff of the whole object against
//! Chrome, **132 properties × 7 representative elements**, found **411 differing readings of 924**,
//! and the dominant shape was not a wrong value: it was `undefined`. Fifty-one properties were
//! absent from the object entirely.
//!
//! ⚠⚠⚠ **THE SPLIT IS THE DELIVERABLE, AND IT IS WHAT THIS GATE ASSERTS FROM BOTH SIDES.** Chrome
//! emits an initial value for every property it supports. Emitting one for a property this engine
//! does not honour would be `@supports`-style false presence — t772's *"absence routes to the
//! fallback; HALF-presence routes into a wall"*, and t608's *"a name is defined IFF the thing it
//! names exists"*. So:
//!
//! * **PUBLISHED** — the properties `ComputedStyle` genuinely holds. Fourteen of them were
//!   `undefined`: `order`, `background-size`, `object-position`, `text-shadow`, `inset`,
//!   `grid-column-start`/`-end`, and the logical family (`margin-inline-*`, `padding-inline-*`,
//!   `inset-inline-*`, `inline-size`, `block-size`, `min`/`max-inline-size`, `min`/`max-block-size`).
//! * **DELIBERATELY ABSENT** — the 41 with no cascade field (`hyphens`, `touchAction`, `willChange`,
//!   `writingMode`, `tabSize`, `containerType`, …). A page's feature detection keeps working.
//!
//! **Measured: 411 → 321 differing readings, fourteen properties fixed, ZERO newly broken.**
//!
//! ⚠ **`grid-template-columns` IS THE INSTRUCTIVE OMISSION.** The cascade holds it, so it looks
//! publishable — but Chrome reports the **USED track sizes in px** (`98.6562px 197.344px` for
//! `1fr 2fr` in a 300px container), not the author's list. Emitting `1fr 2fr` would be a **wrong
//! answer of the RIGHT TYPE**, which is the shape this project rates most dangerous: a grid library
//! parsing px out of it gets `NaN` from a string that looked valid. It stays absent and is asserted
//! absent.
//!
//! ⚠ **AND ONE OF THIS TICK'S OWN NEW ROWS WAS WRONG, CAUGHT BY THE RE-SWEEP IN ONE LINE.**
//! `max-inline-size`/`max-block-size` first went out through `dim_css`, which serialises an unset
//! value as `auto`; the physical `max-width`/`max-height` use `max_dim`, which correctly says
//! `none`. **The logical spelling disagreed with the physical one about the same box** — the exact
//! two-spellings-one-box drift, appearing the moment a second serialiser was used. `inline-size` and
//! `block-size` therefore go through the SAME `used_dim_css` as `width`/`height`, and this gate
//! asserts the identity rather than the values.
//!
//! **Every expectation is Chrome's, captured from a real `google-chrome --headless --dump-dom` run of
//! this exact fixture — with ONE deliberate exception, labelled at its assertion**: `stillAbsent`
//! states this engine's honesty boundary, and Chrome (which supports all seventeen) reports every one
//! of them. A gate that copied Chrome there would demand we fabricate values for capabilities we do
//! not have. **Proven RED** by reverting the batch: the fourteen published rows read `undefined`, and
//! `wordSpacing` reads `normal`.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  #host { width: 600px; font: 16px/20px monospace; position: relative; }
  #a { display:block; padding:5px 6px 7px 8px; border:2px solid red; margin:3px; width:40%;
       position:relative; inset:0; order:2; letter-spacing:1px; word-spacing:2px;
       text-shadow:1px 2px 3px rgb(0,0,0); min-width:10px; max-width:900px;
       min-height:2px; max-height:800px; background-size:auto; }
  #g { display:grid; grid-template-columns:1fr 2fr; width:300px; }
  #gi { grid-column: 1 / 2; height:11px; }
  #p { display:block; width:70px; height:30px; }
</style></head><body>
<div id="host"><div id="a">A</div><div id="g"><div id="gi">g</div></div><div id="p"></div></div>
<div id="out">-</div>
<script>
  var R = [];
  var p = function (k, v) { R.push(k + '=' + v); };
  var q = function (id, pr) { var v = getComputedStyle(document.getElementById(id))[pr];
                              return v === undefined ? 'UNDEF' : String(v); };

  // ── PUBLISHED: properties the cascade holds, and every one of these read `undefined` before.
  p('order',            q('a', 'order'));
  p('backgroundSize',   q('a', 'backgroundSize'));
  p('objectPosition',   q('a', 'objectPosition'));
  p('textShadow',       q('a', 'textShadow'));
  p('inset',            q('a', 'inset'));
  p('gridColumnStart',  q('gi', 'gridColumnStart'));
  p('gridColumnEnd',    q('gi', 'gridColumnEnd'));
  p('marginInlineStart',  q('a', 'marginInlineStart'));
  p('marginBlockStart',   q('a', 'marginBlockStart'));
  p('paddingInlineStart', q('a', 'paddingInlineStart'));
  p('paddingInlineEnd',   q('a', 'paddingInlineEnd'));
  p('paddingBlockStart',  q('a', 'paddingBlockStart'));
  p('insetInlineStart',   q('a', 'insetInlineStart'));
  p('minInlineSize',      q('a', 'minInlineSize'));
  p('maxInlineSize',      q('a', 'maxInlineSize'));
  p('minBlockSize',       q('a', 'minBlockSize'));
  p('maxBlockSize',       q('a', 'maxBlockSize'));
  p('inlineSize',         q('p', 'inlineSize'));
  p('blockSize',          q('p', 'blockSize'));

  // ── The `max-*` serialisation that this tick got wrong once: unset must be `none`, not `auto`.
  p('maxInlineSizeUnset', q('p', 'maxInlineSize'));
  p('maxBlockSizeUnset',  q('p', 'maxBlockSize'));

  // ── RECONCILIATION: two spellings of one box must never disagree.
  p('inlineSizeEqWidth',  q('p', 'inlineSize')  === q('p', 'width'));
  p('blockSizeEqHeight',  q('p', 'blockSize')   === q('p', 'height'));
  p('maxInlineEqMaxWidth', q('a', 'maxInlineSize') === q('a', 'maxWidth'));
  p('marginInlineEqLeft',  q('a', 'marginInlineStart') === q('a', 'marginLeft'));

  // ── The asymmetry a lumped comment hid: word-spacing's initial is a LENGTH, letter-spacing's is
  //    the keyword `normal`. They look symmetric and are not.
  p('wordSpacingUnset',   q('p', 'wordSpacing'));
  p('letterSpacingUnset', q('p', 'letterSpacing'));
  p('wordSpacingSet',     q('a', 'wordSpacing'));
  p('letterSpacingSet',   q('a', 'letterSpacing'));

  // ── DELIBERATELY ABSENT — the guard that stops a later "be helpful" edit from emitting initial
  //    values for properties this engine does not honour. Each of these is a real capability a page
  //    feature-detects, and a fabricated answer routes the caller into a wall instead of a fallback.
  var absent = ['hyphens','touchAction','willChange','writingMode','tabSize','containerType',
                'scrollBehavior','overscrollBehavior','caretColor','accentColor','isolation',
                'contain','columnCount','breakInside','unicodeBidi','fontStretch',
                'gridTemplateColumns'];
  var leaked = [];
  for (var i = 0; i < absent.length; i++) {
    if (getComputedStyle(document.getElementById('a'))[absent[i]] !== undefined) { leaked.push(absent[i]); }
  }
  p('stillAbsent', leaked.length === 0 ? 'yes' : ('LEAKED:' + leaked.join(',')));

  // ── The enumeration must move with the object, or `item(i)` cannot reach the new names.
  var cs = getComputedStyle(document.getElementById('a'));
  var names = [];
  for (var j = 0; j < cs.length; j++) { names.push(cs.item(j)); }
  p('enumHasMarginInline', names.indexOf('margin-inline-start') >= 0);
  p('enumHasInlineSize', names.indexOf('inline-size') >= 0);
  p('enumHasOrder',      names.indexOf('order') >= 0);
  p('getPropertyValueInlineSize', cs.getPropertyValue('inline-size') === q('a', 'inlineSize'));

  document.getElementById('out').textContent = R.join(' ');
</script>
</body></html>"##;

#[test]
fn computed_style_publishes_what_the_cascade_holds_and_nothing_it_does_not() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://cs.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("COMPUTED-STYLE SWEEP: {got}");

    for (claim, why) in [
        // ── PUBLISHED. Every one of these read `undefined` before this tick.
        ("order=2", "THE CLASS: `order` is cascade-held and was `undefined` — a flex library reading back an item's order got nothing"),
        ("backgroundSize=auto", ""),
        ("objectPosition=50% 50%", ""),
        ("textShadow=rgb(0, 0, 0) 1px 2px 3px", "Chrome's order is colour first, then the three lengths"),
        ("inset=0px", "the four-value shorthand collapses when the sides agree"),
        ("gridColumnStart=1", "an explicit grid line — `auto` everywhere else would pass a one-element check"),
        ("gridColumnEnd=2", "and its pair, so a single-side implementation fails"),
        ("marginInlineStart=3px", "the LOGICAL family is what a modern stylesheet is authored in"),
        ("marginBlockStart=3px", ""),
        ("paddingInlineStart=8px", "asymmetric padding on purpose: 5px 6px 7px 8px, so a wrong side is visible"),
        ("paddingInlineEnd=6px", ""),
        ("paddingBlockStart=5px", ""),
        ("insetInlineStart=0px", ""),
        ("minInlineSize=10px", ""),
        ("maxInlineSize=900px", ""),
        ("minBlockSize=2px", ""),
        ("maxBlockSize=800px", ""),
        ("inlineSize=70px", "the logical size resolves to the USED value, like `width` (t897)"),
        ("blockSize=30px", ""),
        // ── The serialisation this tick got wrong once and the re-sweep caught.
        (
            "maxInlineSizeUnset=none",
            "AN UNSET `max-*` IS `none`, NOT `auto` — the first version of this batch used `dim_css` \
             for all four and the logical spelling disagreed with the physical one about the same box",
        ),
        ("maxBlockSizeUnset=none", ""),
        // ── RECONCILIATION.
        (
            "inlineSizeEqWidth=true",
            "RECONCILIATION: two spellings of ONE box must never disagree — they go through the same \
             `used_dim_css` for exactly this reason",
        ),
        ("blockSizeEqHeight=true", ""),
        ("maxInlineEqMaxWidth=true", "…and the logical max must equal the physical max"),
        ("marginInlineEqLeft=true", "…and the logical margin the physical one, under horizontal-tb"),
        // ── The asymmetry.
        (
            "wordSpacingUnset=0px",
            "`word-spacing`'s initial value is the LENGTH 0; a lumped comment claimed it shared \
             `letter-spacing`'s `normal` and it does not",
        ),
        (
            "letterSpacingUnset=normal",
            "…while `letter-spacing`'s initial IS the keyword, and it must stay that way — `normal` \
             permits the font's own kerning and `0px` does not",
        ),
        ("wordSpacingSet=2px", "both still report an explicit value"),
        ("letterSpacingSet=1px", ""),
        // ── The guard.
        (
            "stillAbsent=yes",
            "GUARD, and the ONE row here that is deliberately NOT Chrome's answer — Chrome supports \
             all seventeen and reports them, so it 'leaks' every one. This is a statement about THIS \
             engine's honesty boundary: a property with no cascade field must stay `undefined`, \
             because emitting an initial value for something we do not honour is @supports-style \
             false presence and routes a feature-detecting caller into a wall instead of a fallback",
        ),
        // ── The enumeration.
        (
            "enumHasMarginInline=true",
            "the name list and the object slots are ONE list — a published property that `item(i)` \
             cannot reach is unreachable to every library that copies a computed style. (A \
             SHORTHAND like `inset` is deliberately not asserted here: Chrome enumerates longhands \
             only, so a shorthand claim would pin OUR list shape as if it were Chrome's.)",
        ),
        ("enumHasInlineSize=true", ""),
        ("enumHasOrder=true", ""),
        ("getPropertyValueInlineSize=true", "and `getPropertyValue` must agree with the camelCase slot"),
    ] {
        assert!(
            got.contains(claim),
            "G_COMPUTED_STYLE_PUBLISHES_THE_CASCADE: missing `{claim}`{}\n  got: {got}",
            if why.is_empty() { String::new() } else { format!("\n  — {why}") }
        );
    }
}
