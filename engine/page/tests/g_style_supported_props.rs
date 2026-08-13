//! # G_STYLE_SUPPORTED_PROPS — `'gridTemplateColumns' in el.style` was FALSE for every feature we have
//!
//! CSSOM says a `CSSStyleDeclaration` exposes an IDL attribute for **every supported property**, set
//! or not. `el.style` is a `Proxy` whose `has` trap was `dash(prop) in parse()` — *"is this property
//! currently SET in the style attribute"*. Tick 1171 measured the result:
//!
//! ```text
//!   'display' in el.style                    FALSE   ← and 27 other property names, 0/28
//!   el.style.gridTemplateColumns = '1fr 2fr' → reads back "1fr 2fr"   ✓  set/get works
//! ```
//!
//! **`'prop' in el.style` is THE CSS feature-detection idiom** — the one Modernizr and every polyfill
//! loader is built on. Answering `false` for a feature the engine *has* is the inverse of the usual
//! failure and strictly worse: it makes a page take its fallback path **against a working engine**.
//!
//! ## The blocker t1171 named, and the half that was missing
//!
//! > *"`has` must answer for the set of SUPPORTED property names, and the engine's honest oracle for
//! > that is `supports_condition` — but it answers one declaration at a time and is not enumerable,
//! > so there is no list to hand the Proxy. Building that registry is the tick; guessing a list would
//! > re-create the `PARSE_ONLY_LONGHANDS` drift."*
//!
//! **One-at-a-time is fine if you have candidates to ask.** `CANDIDATE_PROPERTIES` is that list, and
//! `supported_property_names()` filters it through `supports_condition` — the *same* evaluator
//! `@supports` and `CSS.supports()` consult, so the three cannot drift, which is exactly what the
//! warning about guessing was protecting. Measured cost: **21ms**, paid **lazily** — a page that
//! never feature-detects never pays it.
//!
//! ## ⚠ The registry is a LOWER BOUND, and that is why it cannot regress anything
//!
//! A name outside the candidate list is never asked, so it answers exactly as it did before. A set
//! property still answers `true` regardless of the list. **The change can only turn `false` into
//! `true`, never the reverse** — `setStillWins` and `unknownStaysFalse` below pin both edges.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
  <div id="d"></div>
  <div id="out">-</div>
  <script>
    var R = [];
    function p(s) { R.push(s); document.getElementById('out').textContent = R.join(' '); }
    var el = document.getElementById('d');

    // ── 1. THE LOAD-BEARING CLAIM. Not one property name answered `in` — `display` included.
    p('display:' + ('display' in el.style));
    p('gridTemplateColumns:' + ('gridTemplateColumns' in el.style));
    p('flexGrow:' + ('flexGrow' in el.style));
    p('transform:' + ('transform' in el.style));

    // ── 2. BOTH SPELLINGS. `in` is asked with camelCase by libraries and with the dashed form by
    //    CSSOM code; one value, two spellings, and a fix that handles only one is half a fix.
    p('dashed:' + ('grid-template-columns' in el.style));
    p('dashedDisplay:' + ('display' in el.style && 'display' in el.style));

    // ── 3. ⚠ THE EDGES THAT MAKE THIS UNABLE TO REGRESS ANYTHING.
    el.style.color = 'red';
    p('setStillWins:' + ('color' in el.style));
    p('unknownStaysFalse:' + ('totallyMadeUpProperty' in el.style));
    p('setUnknownName:' + ('--a-custom-prop' in el.style));

    // ── 4. THE RATCHET — set/get, which always worked, must be untouched.
    el.style.gridTemplateColumns = '1fr 2fr';
    p('setGet:' + el.style.gridTemplateColumns);
    p('colorValue:' + el.style.color);
    p('cssText:' + (el.style.cssText.indexOf('color') >= 0));
    p('removeProperty:' + (el.style.removeProperty('color'), el.style.color));

    // ── 5. AND THE ORACLE AGREES WITH ITSELF. The registry is filtered through the SAME evaluator
    //    `CSS.supports` uses, so the two must never disagree about one name.
    p('agreesWithSupports:' + (('display' in el.style) === CSS.supports('display', 'flex')));
  </script>
</body></html>"##;

#[test]
fn el_style_exposes_every_supported_property_not_only_the_set_ones() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://styleprops.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("STYLE-PROPS: {got}");

    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_STYLE_SUPPORTED_PROPS: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}

const CLAIMS: &[(&str, &str)] = &[
    (
        "display:true",
        "THE LOAD-BEARING CLAIM. `'display' in el.style` was FALSE — t1171 measured 0 of 28 property \
         names answering `in`, `display` included. This is THE CSS feature-detection idiom, and a \
         capability we POSSESS reported as absent makes a page take its fallback against a working \
         engine",
    ),
    (
        "gridTemplateColumns:true",
        "the name t1171 probed with, and the one every grid polyfill loader asks about",
    ),
    ("flexGrow:true", "a longhand"),
    ("transform:true", "a property this engine renders and gates"),
    (
        "dashed:true",
        "⚠ BOTH SPELLINGS. Libraries ask with camelCase and CSSOM code with the dashed form; a fix \
         that normalises only one direction is half a fix",
    ),
    ("dashedDisplay:true", "and the shorthand of that check"),
    (
        "setStillWins:true",
        "⚠ AN EDGE THAT MAKES THIS UNABLE TO REGRESS. A property that IS set answers true whether or \
         not it is in the registry — the registry is consulted only after the set-check fails",
    ),
    (
        "unknownStaysFalse:false",
        "⚠ THE OTHER EDGE. A name that is not a CSS property must still answer false; the registry \
         is a membership test, not a blanket `true`. If this flipped, `in` would stop being a \
         feature detect in the opposite direction",
    ),
    (
        "setGet:1fr 2fr",
        "THE RATCHET. set/get always worked — t1171 measured it working while `in` said the property \
         did not exist — and it must be untouched",
    ),
    ("cssText:true", "THE RATCHET. `cssText` still reflects a set property"),
    ("removeProperty:", "THE RATCHET. `removeProperty` still clears it"),
    (
        "agreesWithSupports:true",
        "⚠ THE ORACLE AGREES WITH ITSELF. The registry is filtered through `supports_condition` — \
         the SAME evaluator `CSS.supports()` and `@supports` consult — so the three can never \
         disagree about a name. That is exactly what t1171's warning against 'guessing a list' was \
         protecting, and this claim is what keeps the guarantee rather than the intention",
    ),
];
