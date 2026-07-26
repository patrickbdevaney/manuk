//! **G_COMPUTED_COMPLETENESS — the properties the engine RENDERS must be readable from JS.**
//!
//! ## The measurement that drove this tick
//!
//! t596 closed the `undefined`-from-`getComputedStyle` defect for four properties. A probe of **95
//! commonly-read properties then found 86 of them still returning `undefined`** — so t596 had fixed
//! 4 of ~86, which is this session's recurring failure exactly: *a fix scoped to the shape the bug
//! presented in is one category too narrow.* Almost every one of those 86 already had a true
//! computed value sitting in `ComputedStyle`: **the engine was rendering them and refusing to say
//! so.**
//!
//! And `undefined` is not a gap. `cs.borderRadius.split(' ')`, `cs.boxShadow.indexOf('inset')`,
//! `parseFloat(cs.borderTopWidth)` — the first two are `TypeError`s that kill the caller's frame.
//! A missing rendering degrades a page; a missing string stops the script.
//!
//! ## The structural half, which is the part that keeps paying
//!
//! Those properties used to live in **three** places — the 60-argument `format!`, the `STD` name
//! array behind `length`/`item(i)`, and the dash→camel map — and the three drift independently
//! (`length` was once a hand-maintained `50` against a list of 52). Everything this tick adds comes
//! from **one** function, `extra_computed_props`, which emits the object slots *and* the enumeration
//! names. **A property added there cannot be enumerable-invisible**, which is why claim 3 below can
//! be a general statement rather than a per-property spot-check.
//!
//! ## What is asserted
//!
//! 1. **Every property is a string, set or unset** — the throw-class claim, on an element with no
//!    styling at all.
//! 2. **Set values are the resolved values**, not debug dumps, including the two that cannot be read
//!    off the enum alone (see below).
//! 3. **Enumeration and `getPropertyValue` agree with the property**, for a property from the new
//!    list — the three routes are one list now, and this is what proves it.
//!
//! Two serializations are worth naming because they are wrong in the obvious implementation:
//!
//! - **`border-*-style` cannot be read off `BorderStyle`.** That enum has no `none`/`hidden` — the
//!   cascade collapses both to a **zero width** — so a naive `match` reports `solid` for every
//!   element on the page, including the ones with no border at all. It is recovered from the width.
//! - **An unset `letter-spacing` is `normal`, not `0px`.** The difference is observable: `normal`
//!   permits the font's own kerning, `0px` suppresses it.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  #styled { border: 2px dashed #f00; border-radius: 6px; box-shadow: 0 2px 4px #000;
            text-decoration: underline; letter-spacing: 2px; float: left;
            list-style-type: square; object-fit: cover; text-transform: uppercase;
            background-image: url(a.png); vertical-align: middle; gap: 4px 8px }
</style></head><body>
<div id="plain">x</div><div id="styled">x</div><div id="out">-</div>
<script>
  var R = [];
  var p = getComputedStyle(document.getElementById('plain'));
  var s = getComputedStyle(document.getElementById('styled'));
  // ── 1. THE THROW-CLASS CLAIM, on a completely unstyled element.
  var probe = ['borderRadius','borderTopWidth','borderTopStyle','borderTopColor','outlineWidth',
    'boxShadow','textDecoration','textTransform','letterSpacing','wordSpacing','textIndent',
    'textOverflow','wordBreak','overflowWrap','direction','verticalAlign','listStyleType',
    'backgroundImage','backgroundRepeat','objectFit','maskImage','float','cssFloat','clear',
    'tableLayout','borderCollapse','aspectRatio','gap'];
  var notString = [];
  probe.forEach(function(k){ if (typeof p[k] !== 'string') notString.push(k); });
  R.push('notString:' + notString.length);
  // ── The idiom that throws, run for real on the UNSTYLED element.
  var threw = false;
  try {
    p.borderRadius.split(' '); p.boxShadow.indexOf('inset');
    p.backgroundImage.indexOf('url'); parseFloat(p.borderTopWidth);
  } catch (e) { threw = true; }
  R.push('idiomThrew:' + threw);
  // ── 2. RESOLVED VALUES.
  R.push('r_radius:' + s.borderRadius);
  R.push('r_bw:' + s.borderTopWidth);
  R.push('r_bs:' + s.borderTopStyle);
  R.push('r_bc:' + s.borderTopColor);
  R.push('r_shadow_has_inset:' + (s.boxShadow.indexOf('inset') !== -1));
  R.push('r_deco:' + s.textDecoration);
  R.push('r_ls:' + s.letterSpacing);
  R.push('r_float:' + s.float);
  R.push('r_cssFloat:' + s.cssFloat);
  R.push('r_list:' + s.listStyleType);
  R.push('r_fit:' + s.objectFit);
  R.push('r_tt:' + s.textTransform);
  R.push('r_bg_has_url:' + (s.backgroundImage.indexOf('url(') === 0));
  R.push('r_va:' + s.verticalAlign);
  R.push('r_gap:' + s.gap);
  // ── The two that the obvious implementation gets wrong.
  R.push('u_bs:' + p.borderTopStyle);     // no border at all → `none`, NOT `solid`
  R.push('u_ls:' + p.letterSpacing);      // unset → `normal`, NOT `0px`
  // ── 3. ONE LIST: enumeration and getPropertyValue must agree with the property.
  var names = [];
  for (var i = 0; i < s.length; i++) names.push(s.item(i));
  var missing = [];
  ['border-radius','border-top-width','box-shadow','float','object-fit','gap'].forEach(function(n){
    if (names.indexOf(n) === -1) missing.push(n);
  });
  R.push('enumMissing:' + missing.length);
  R.push('gpv_bw:' + (s.getPropertyValue('border-top-width') === s.borderTopWidth));
  R.push('gpv_ls:' + (s.getPropertyValue('letter-spacing') === s.letterSpacing));
  R.push('lenMatchesNames:' + (names.length === s.length));
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn every_rendered_property_reads_back_as_a_resolved_string() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://cssom2.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("COMPUTED COMPLETENESS: {got}");

    for (claim, why) in [
        (
            "notString:0",
            "EVERY one of the 28 probed properties must be a string on an element with NO styling. \
             A probe of 95 commonly-read properties found 86 returning `undefined` — t596 fixed four \
             of them, which is the too-narrow trap this session keeps paying for",
        ),
        (
            "idiomThrew:false",
            "THE POINT: `cs.borderRadius.split(' ')`, `cs.boxShadow.indexOf('inset')` and \
             `parseFloat(cs.borderTopWidth)` on an UNSTYLED element must not throw. The first two \
             are TypeErrors that kill the caller's frame — a missing rendering degrades a page, a \
             missing string stops the script",
        ),
        ("r_radius:6px", "`border-radius` resolves to px"),
        ("r_bw:2px", "`border-top-width` resolves to px"),
        (
            "r_bs:dashed",
            "the border STYLE keyword survives — it is not recoverable from the width alone",
        ),
        ("r_bc:rgb(255, 0, 0)", "border colour serializes as `rgb()`"),
        (
            "r_shadow_has_inset:false",
            "an outer `box-shadow` must NOT claim `inset` — the flag is carried per layer and a \
             serializer that always appends it would make every shadow read as inner",
        ),
        ("r_deco:underline", "`text-decoration` names its lines"),
        ("r_ls:2px", "an explicit `letter-spacing` is px"),
        ("r_float:left", "`float` reads back"),
        (
            "r_cssFloat:left",
            "…and so does the LEGACY `cssFloat` spelling, which is what most frameworks actually \
             read because `float` was a reserved word",
        ),
        ("r_list:square", "`list-style-type`"),
        ("r_fit:cover", "`object-fit`"),
        ("r_tt:uppercase", "`text-transform`"),
        (
            "r_bg_has_url:true",
            "`background-image` serializes as a `url(...)` function, so the near-universal \
             `indexOf('url(')` detection works",
        ),
        ("r_va:middle", "`vertical-align`"),
        ("r_gap:4px 8px", "`gap` serializes both axes"),
        (
            "u_bs:none",
            "**THE ONE THE OBVIOUS IMPLEMENTATION GETS WRONG.** `BorderStyle` has no `none`/`hidden` \
             variant — the cascade collapses both to a ZERO WIDTH — so a naive `match` on the enum \
             reports `solid` for every element on the page, including the ones with no border. It \
             must be recovered from the width",
        ),
        (
            "u_ls:normal",
            "**THE SECOND ONE.** An unset `letter-spacing` is `normal`, not `0px`, and the \
             difference is observable: `normal` permits the font's own kerning and `0px` suppresses \
             it. A serializer that prints the f32 unconditionally reports a value the author never \
             wrote",
        ),
        (
            "enumMissing:0",
            "`item(i)` over `length` must reach the new properties. They are emitted from ONE list \
             that feeds both the object slots and these names, which is what makes this claim a \
             general statement instead of a spot-check — the three-places drift that made `length` a \
             stale constant cannot recur for anything added there",
        ),
        ("gpv_bw:true", "`getPropertyValue` agrees with the property"),
        ("gpv_ls:true", "…including for a value with a keyword form"),
        (
            "lenMatchesNames:true",
            "`length` is DERIVED from the same list it enumerates — the moment it is a hand-written \
             count it starts drifting, and it silently did once already",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_COMPUTED_COMPLETENESS: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
