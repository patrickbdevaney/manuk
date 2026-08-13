//! **G_COMPUTED_PSEUDO — `getComputedStyle(el, '::before')` reports the PSEUDO, not the element.**
//!
//! `G_COMPUTED_STYLE_PSEUDO_ELEMENT` — **the filename-derived name, stated here on purpose.** A gate has TWO
//! names, its FILE and the one its own first line declares (`G_COMPUTED_PSEUDO`), and until tick 1203
//! this file declared only the second while `CONSTELLATION.tsv` cited only the first. Each
//! instrument was then blind to exactly the gates the other could see, and a real, passing,
//! shipped gate read as a PHANTOM to the reconciler. Both dialects now appear in the file, so
//! either reader validates the citation. (Surface audit #60; the same shape as audit #36's
//! case dialect, with a different pair.)
//!
//!
//! The second argument was **ignored**, and that is a strictly worse failure than not supporting it.
//! `getComputedStyle(div, '::before')` returned the *div's* style object, so a page asking about a
//! generated box was answered about a different box entirely, with no way to tell:
//!
//! ```text
//!                        Chrome                     what we returned
//!   content              "sm"        (the pseudo)   undefined   (never published at all)
//!   display              inline      (the pseudo)   block       (the DIV's)
//!   width                auto        (the pseudo)   200px       (the DIV's)
//! ```
//!
//! Every value is present, plausible, and about the wrong thing — the *wrong answer of the right
//! type* this project rates as the most dangerous shape a defect takes, because nothing downstream
//! can detect it.
//!
//! **What reads it.** The breakpoint-detection idiom, which predates `matchMedia` in JS and is still
//! shipped by Bootstrap-era and Foundation-era code and by every hand-rolled version of it:
//!
//! ```css
//!   body::before { content: "sm"; display: none }
//!   @media (min-width: 768px) { body::before { content: "md" } }
//! ```
//! ```js
//!   var bp = getComputedStyle(document.body, '::before').content.replace(/["']/g, '');
//! ```
//!
//! `undefined.replace(...)` is a TypeError, so the frame dies at boot — the throw-class killer the
//! board ranks first — and on the branch where the page merely gets `block` back instead of `none`
//! it silently picks the wrong layout.
//!
//! **Every row here was MEASURED against Chrome** (`/tmp/pseudo-probe*.html`, three batteries) rather
//! than derived from the spec text, because three of the parse rules are quirks the spec does not
//! predict: the `::` form is ASCII case-insensitive while the one-colon legacy form is
//! case-SENSITIVE; a bare `before` is honoured but a bare `Before` is ignored; and an unknown
//! `::bogus` returns an EMPTY declaration while a bare `bogus` returns the element's.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  #plain { width: 200px }
  #b::before { content: "B" }
  #a::after  { content: "A" }
  #blk::before { content: "K"; display: block; width: 50px; height: 20px }
  #hid::before { content: "H"; display: none }
  #inh { color: rgb(1,2,3); font-size: 20px }
  #inh::before { content: "I" }
  #own::before { content: "O"; color: rgb(4,5,6) }
  #multi::before { content: "a" counter(x) }
</style></head><body style="margin:0">
<div id="out">-</div>
<div id="plain">plain</div>
<div id="b">b</div><div id="a">a</div><div id="blk">blk</div><div id="hid">hid</div>
<div id="inh">inh</div><div id="own">own</div><div id="multi">multi</div>
<script>
  var R = [];
  function el(id){ return document.getElementById(id); }
  function cs(id, pe){ return getComputedStyle(el(id), pe); }
  function push(k, v){ R.push(k + '=' + v); }

  // ── NEGATIVE ROWS FIRST: an element with NO pseudo rule at all. Chrome still answers, and it
  //    answers about the PSEUDO — the row that the ignored-argument bug got wrong on every page.
  push('elem-content', getComputedStyle(el('plain')).content);   // the ELEMENT's own: "normal"
  push('absent-content', cs('plain', '::before').content);       // "none", NOT "normal", NOT undefined
  push('absent-display', cs('plain', '::before').display);       // "inline", NOT the div's "block"
  push('absent-width',   cs('plain', '::before').width);         // "auto",   NOT the div's "200px"

  // ── The pseudo's own declarations.
  push('before', cs('b', '::before').content);
  push('after',  cs('a', '::after').content);
  push('multi',  cs('multi', '::before').content);

  // ── The PARSE surface, all four spellings Chrome accepts and the three it does not.
  push('legacy1', cs('b', ':before').content);      // CSS2 one-colon
  push('bare',    cs('b', 'before').content);       // bare, exact lowercase
  push('mixcase', cs('b', '::BeFoRe').content);     // `::` form is case-INsensitive
  push('badcase', cs('b', ':BEFORE').length);       // one-colon form is case-SENSITIVE -> EMPTY (0)
  push('bogus',   cs('b', '::bogus').length);       // unknown pseudo-element         -> EMPTY (0)
  push('bogusprop', typeof cs('b', '::bogus').content);  // an empty declaration reads '' — a STRING
  push('barebogus', cs('plain', 'Before').display); // NOT a pseudo request -> the ELEMENT ("block")
  push('nullarg',   cs('plain', null).display);     // null means "no pseudo"        -> the ELEMENT

  // ── The pseudo's own box and inheritance.
  push('blk-display', cs('blk', '::before').display);
  push('blk-width',   cs('blk', '::before').width);
  push('hid-display', cs('hid', '::before').display);
  push('inherit-color', cs('inh', '::before').color);
  push('inherit-size',  cs('inh', '::before').fontSize);
  push('own-color',     cs('own', '::before').color);

  // ── The kebab route half the web uses.
  push('kebab', cs('b', '::before').getPropertyValue('content'));

  // ── The idiom the whole feature exists for, end to end.
  var bp = cs('b', '::before').content.replace(/["']/g, '');
  push('breakpoint', bp);

  // `|`-joined, not space-joined: two of the claims (`"a" "b"`, `rgb(1, 2, 3)`) CONTAIN spaces,
  // and a space-split assertion would silently never match them — a gate that cannot go red.
  el('out').textContent = R.join('|');
</script></body></html>"##;

#[test]
fn get_computed_style_reports_the_pseudo_element_not_the_originating_element() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://pseudo.test/", &fonts, 400.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "elem-content=normal",
            "`content` was `undefined` on ELEMENTS too — the initial value is the string `normal`, \
             and `cs.content.replace(...)` is a TypeError against `undefined`",
        ),
        (
            "absent-content=none",
            "`normal` COMPUTES TO `none` on ::before/::after, and only there (Chrome-measured: \
             ::first-line still reports `normal`)",
        ),
        (
            "absent-display=inline",
            "THE WHOLE DEFECT: with the second argument ignored this reported the DIV's `block`. A \
             pseudo with no rule is still an inline, and a caller cannot tell a wrong-box answer \
             from a right one",
        ),
        (
            "absent-width=auto",
            "and this reported the div's `200px` — a real number about the wrong box, which is \
             worse than no number at all",
        ),
        ("before=\"B\"", "a string content value is DOUBLE-QUOTED in the resolved value"),
        ("after=\"A\"", "::after is the same seam, and it was equally blind"),
        (
            "multi=\"a\" counter(x)",
            "multiple content terms join with ONE space and a counter term keeps its function \
             form, Chrome-measured. ⚠ NAMED LIMITATION, deliberately not asserted: two ADJACENT \
             string terms (`content: \"a\" \"b\"`) are concatenated by the cascade into one part, \
             so they read back `\"ab\"` where Chrome says `\"a\" \"b\"`. The rendering is identical \
             and the fix is in the content parser, not this seam — asserting the wrong answer here \
             would PIN the engine to it",
        ),
        (
            "legacy1=\"B\"",
            "`:before` — the CSS2 one-colon spelling is what most of the code that reads this was \
             written against",
        ),
        ("bare=\"B\"", "a bare `before` is honoured (Chrome-measured)"),
        (
            "mixcase=\"B\"",
            "the `::` form is ASCII case-INSENSITIVE — `::BeFoRe` is the pseudo",
        ),
        (
            "badcase=0",
            "…while the ONE-COLON form is case-SENSITIVE: `:BEFORE` is an EMPTY declaration, not \
             the pseudo and not the element. Lower-casing both arms is the plausible wrong answer",
        ),
        (
            "bogus=0",
            "an unknown pseudo-element is an EMPTY CSSStyleDeclaration (length 0) — not null, not \
             a throw, and NOT the element's style",
        ),
        (
            "bogusprop=string",
            "and every property of that empty declaration reads `''`, a STRING. `undefined` here \
             is a TypeError one method call later, which is the bug class this gate belongs to",
        ),
        (
            "barebogus=block",
            "a bare name that is not a legacy pseudo is IGNORED — Chrome reports the element. This \
             is the row that keeps `Unknown` from swallowing every odd argument",
        ),
        (
            "nullarg=block",
            "null/undefined mean `no pseudo` — the pre-existing one-argument behaviour must not \
             move an inch",
        ),
        (
            "blk-display=block",
            "the pseudo's OWN display, not the originating element's",
        ),
        (
            "blk-width=50px",
            "a specified width on the pseudo reads back in px (Chrome agrees; it reports `auto` \
             only when the pseudo's width IS auto)",
        ),
        ("hid-display=none", "`display:none` on the pseudo is the pseudo's, not the div's"),
        (
            "inherit-color=rgb(1, 2, 3)",
            "a pseudo INHERITS from its originating element — this is why the absent-pseudo \
             fallback must be `inherit_from`, not `initial`",
        ),
        ("inherit-size=20px", "font-size inherits into the pseudo as well"),
        (
            "own-color=rgb(4, 5, 6)",
            "…and the pseudo's own declaration beats what it inherits",
        ),
        (
            "kebab=\"B\"",
            "`getPropertyValue('content')` must find it — half the web asks for computed values \
             that way",
        ),
        (
            "breakpoint=B",
            "END TO END: the responsive-breakpoint idiom (`cs.content.replace(/[\"']/g,'')`) that \
             this call exists to serve. Against `undefined` it is a TypeError that kills the frame",
        ),
    ] {
        assert!(
            got.split('|').any(|t| t == claim),
            "{claim}\n  {why}\n  got: {got}"
        );
    }
}
