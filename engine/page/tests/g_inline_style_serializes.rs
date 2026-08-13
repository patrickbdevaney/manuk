//! **G_INLINE_STYLE_SERIALIZES — `el.style` reports CSSOM's normal form, not the author's bytes.**
//!
//! ⚠⚠⚠ **IT DID NOT SERIALIZE AT ALL — IT ECHOED.** `el.style` is a live view over the `style`
//! **attribute**, and every read handed back the exact text the author typed. CSSOM says
//! `getPropertyValue` returns *"the result of serializing the declaration's value"* — a normal form.
//! Measured against WPT's own `css/cssom/serialize-values.html`:
//!
//! ```text
//!   style="…"                          Chrome                   ours (before)
//!   background-position: 5% .5%        "5% 0.5%"                "5% .5%"
//!   background-position: 5% -.5%       "5% -0.5%"               "5% -.5%"
//!   background-position: 5% -0px       "5% 0px"                 "5% -0px"
//!   background-image: url(http://x/)   "url(\"http://x/\")"     "url(http://x/)"
//! ```
//!
//! **t1220 sized this family at 164 subtests and refused the obvious fix IN ADVANCE**, and this gate
//! exists to hold that refusal: *"a targeted 'prepend 0 to a leading dot' fix would pass all 164 and
//! be a band-aid — `el.style` does not SERIALIZE values, it ECHOES them, and every other CSSOM
//! normalisation (unit case, colour form, shorthand ordering) is silently wrong the same way."*
//!
//! So the value is round-tripped through **Stylo's own parser and serializer**
//! (`stylo_engine::serialize_declaration`), which is the same evaluator `@supports` and
//! `CSS.supports()` consult. Three surfaces asking about one declaration, one answer — the same
//! discipline that exists because `@supports` and `CSS.supports()` once disagreed. The leading zero,
//! the quoted URL and the negative zero all fall out of it, and **so do the normalisations nobody
//! has written a test for yet**, which is the whole difference between this and a regex.
//!
//! **Two refusals, both asserted below, because each is a way a plausible implementation loses data:**
//! - A **custom property** (`--brand: .5rem`) has no grammar to normalise against. It is echoed
//!   verbatim; normalising it would corrupt every design token on the page.
//! - A declaration Stylo **declines** is echoed too, not deleted. `''` from the seam means *leave it
//!   alone*, never *it is empty* — the opposite polarity from `CSS.supports`'s conservative `false`,
//!   because there guessing "yes" invents a capability while here it would **lose a style the page
//!   set**.
//!
//! ⚠ The read is memoised on `(property, value)`, like the validator beside it: `el.style.transform`
//! in a `requestAnimationFrame` loop must not pay a Stylo parse per frame. Buying conformance with a
//! per-frame regression is a trade, and the ratchet refuses trades.
//!
//! **Proven RED**: return the raw text from `read()` and **six of the ten claims fail** — the five
//! serialization rows plus the setter round-trip. The other four stay GREEN by design: they assert
//! the REFUSALS (a custom property echoed, a sibling kept, a declined declaration not deleted) and
//! the two-spellings reconciliation, all of which must hold whether or not the mechanism is on. A
//! gate whose every claim fails under one mutation is not testing more than one thing.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head></head><body>
<div id="a" style="background-position: 5% .5%"></div>
<div id="b" style="background-position: 5% -.5%"></div>
<div id="c" style="background-position: 5% -0px"></div>
<div id="d" style="background-image: url(http://localhost/)"></div>
<div id="e" style="margin-left: .1em"></div>
<div id="f" style="--brand: .5rem; color: red"></div>
<div id="g" style="width: 10px"></div>
<div id="out">-</div>
<script>
  var R = [];
  var p = function (k, v) { R.push(k + '=' + v); };
  var el = function (id) { return document.getElementById(id); };

  // ── The rule: a read returns the SERIALIZED value.
  p('pct',      el('a').style.backgroundPosition);   // .5%  -> 0.5%
  p('negpct',   el('b').style.backgroundPosition);   // -.5% -> -0.5%
  p('negzero',  el('c').style.backgroundPosition);   // -0px -> 0px
  p('url',      el('d').style.backgroundImage);      // url(x) -> url("x")
  p('len',      el('e').style.marginLeft);           // .1em -> 0.1em

  // ── The IDL attribute and getPropertyValue are two spellings of ONE read.
  p('agrees', el('a').style.getPropertyValue('background-position')
              === el('a').style.backgroundPosition);

  // ── The refusals. Each is a way a plausible implementation LOSES data.
  p('custom',   el('f').style.getPropertyValue('--brand'));  // echoed verbatim, never normalised
  p('kept',     el('f').style.color);                        // a sibling decl still reads back

  // ── A round trip through the setter must still land somewhere readable.
  var g = el('g');
  g.style.marginTop = '.25em';
  p('setter',   g.style.marginTop);
  p('nonEmpty', g.style.width !== '');

  document.getElementById('out').textContent = R.join(' ');
</script>
</body></html>"##;

#[test]
fn inline_style_reads_back_the_serialized_value_not_the_authors_text() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://is.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("INLINE STYLE SERIALIZE: {got}");

    for (claim, why) in [
        (
            "pct=5% 0.5%",
            "THE DEFECT: CSSOM requires a number below 1 to serialize WITH a leading zero. We \
             returned the author's `.5%` because `el.style` echoed the style attribute's bytes",
        ),
        ("negpct=5% -0.5%", "…and the sign goes BEFORE the inserted zero, not after it"),
        (
            "negzero=5% 0px",
            "`-0px` serializes as `0px`. A regex that only inserts leading zeros passes the two \
             rows above and fails here, which is why the value goes through a real serializer",
        ),
        (
            "url=url(\"http://localhost/\")",
            "…and an unquoted `url()` serializes QUOTED. Nothing about leading zeros produces this \
             — it is the second, independent proof that the normal form is Stylo's and not a patch",
        ),
        ("len=0.1em", "the same rule on a length, and on a different property"),
        (
            "agrees=true",
            "RECONCILIATION: `style.backgroundPosition` and `getPropertyValue('background-position')` \
             are two spellings of ONE read. Serializing in one and echoing in the other is the drift \
             that made `max-inline-size` and `top` disagree with themselves in earlier ticks",
        ),
        (
            "custom=.5rem",
            "REFUSAL: a CUSTOM PROPERTY has no grammar to normalise against and is echoed VERBATIM. \
             An implementation that normalised it would rewrite every design token on the page — \
             `--brand: .5rem` is a token, not a length the engine may reinterpret",
        ),
        (
            "kept=red",
            "…and normalising one declaration must not disturb its siblings in the same attribute",
        ),
        (
            "setter=0.25em",
            "a value written THROUGH the setter reads back serialized too — the write path stores \
             the author's text and the read path is what normalises, so both spellings agree",
        ),
        (
            "nonEmpty=true",
            "GUARD, and it is the one that matters most: `''` from the seam means LEAVE IT ALONE, \
             never `it is empty`. A declaration the serializer declines must still read back. The \
             opposite polarity to `CSS.supports`'s conservative `false` — there guessing yes \
             invents a capability, here it would DELETE a style the page set",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_INLINE_STYLE_SERIALIZES: missing `{claim}` — {why}\n  got: {got}"
        );
    }
}
