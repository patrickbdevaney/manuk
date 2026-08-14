//! # G_INTRINSIC_MIN_MAX_CSSOM — the keyword must READ BACK, not just lay out
//!
//! The layout half is `g_intrinsic_min_max`. This is the I3 half: a value the engine now honours
//! must also be **published through the semantic model**, or a script asking what it just set gets a
//! different answer from the one the pixels are using.
//!
//! Before t930 the four min/max properties had no keyword sidecar at all, so `min-content` reached
//! `getComputedStyle` as the `Dim::Auto` it collapsed to — i.e. as `"auto"` on a min and `"none"` on
//! a max, which are the strings for **unset**. A script reading `cs.maxWidth === 'none'` concluded
//! there was no cap while the box was capped.
//!
//! ## Chrome-measured on this exact declaration set
//!
//! `google-chrome --headless=new --dump-dom`, `<div style="min-width:min-content;
//! max-width:fit-content; min-height:max-content; max-height:min-content">`:
//!
//! ```text
//!                     Chrome          before      after
//!   minWidth          min-content     auto        min-content
//!   maxWidth          fit-content     none        fit-content
//!   minHeight         max-content     auto        max-content
//!   maxHeight         min-content     none        min-content
//!   minInlineSize     min-content     auto        min-content
//!   maxInlineSize     fit-content     none        fit-content
//!   minBlockSize      max-content     auto        max-content
//!   maxBlockSize      min-content     none        min-content
//! ```
//!
//! ⚠ **`fit-content(<length>)` is INVALID on all four**, and that is measured, not inferred from the
//! grammar: Chrome reads `min-width:fit-content(50px)` back as `0px` and `max-width:fit-content(50px)`
//! as `none` — the declaration is dropped. The parser has a separate `intrinsic_kw_bare` for exactly
//! this, because accepting it would make us *more* permissive than Chrome and lay out a box Chrome
//! does not.
//!
//! ⚠⚠ **The logical spellings are asserted beside the physical ones on purpose.** `extra_computed_props`
//! already records catching a drift where the logical `max-inline-size` said `auto` while the physical
//! `max-width` said `none` about the same box — two serialisers, one property. t930 routes both
//! through one function; these rows are what would catch the next split.
//!
//! ## How this goes RED
//!
//! Make `min_dim_css`/`max_dim_css` ignore their keyword argument (i.e. restore plain `dim_css` /
//! the inline `Dim::Auto => "none"` match) → all eight rows below fall back to `auto`/`none` while
//! the CONTROL rows, which carry real lengths, stay green.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="k" style="min-width:min-content;max-width:fit-content;min-height:max-content;max-height:min-content">x</div>
<div id="c" style="min-width:30px;max-width:40px;min-height:50px;max-height:60px">x</div>
<div id="u">x</div>
<div id="fn" style="min-width:fit-content(50px);max-width:fit-content(50px)">x</div>
<div id="s" style="min-width:stretch;max-width:stretch;min-height:stretch;max-height:stretch">x</div>
<div id="sa" style="min-height:-webkit-fill-available;max-width:-webkit-fill-available">x</div>
<div id="sl" style="min-block-size:stretch;max-inline-size:stretch">x</div>
<pre id="out"></pre>
<script>
  var R = [], k = getComputedStyle(document.getElementById('k')),
      c = getComputedStyle(document.getElementById('c')),
      u = getComputedStyle(document.getElementById('u')),
      f = getComputedStyle(document.getElementById('fn')),
      st = getComputedStyle(document.getElementById('s')),
      sa = getComputedStyle(document.getElementById('sa')),
      sl = getComputedStyle(document.getElementById('sl'));
  // ── THE KEYWORD ROUND-TRIPS, on both spellings of each property.
  R.push('minW:' + k.minWidth);
  R.push('maxW:' + k.maxWidth);
  R.push('minH:' + k.minHeight);
  R.push('maxH:' + k.maxHeight);
  R.push('minIS:' + k.minInlineSize);
  R.push('maxIS:' + k.maxInlineSize);
  R.push('minBS:' + k.minBlockSize);
  R.push('maxBS:' + k.maxBlockSize);
  // ── getPropertyValue agrees with the camelCase accessor (one backing list, two doors).
  R.push('gpv:' + (k.getPropertyValue('max-width') === k.maxWidth));
  // ── CONTROLS: real lengths are untouched, and UNSET still says auto/none.
  R.push('cMinW:' + c.minWidth);
  R.push('cMaxW:' + c.maxWidth);
  R.push('cMinH:' + c.minHeight);
  R.push('cMaxH:' + c.maxHeight);
  R.push('uMinW:' + u.minWidth);
  R.push('uMaxW:' + u.maxWidth);
  // ── `fit-content(<length>)` is not valid here — the declaration is dropped (Chrome-measured).
  R.push('fnMaxW:' + f.maxWidth);
  // ── `stretch` IS A KEYWORD ON THESE FOUR TOO, and it needs the same sidecar the intrinsic ones
  //    have: it collapses to `Dim::Auto` identically, so without it a box sized correctly by
  //    `min-height:stretch` reads back as UNSET. Chrome-measured — all four are the string
  //    "stretch", the `-webkit-fill-available` ALIAS normalises to it, and the LOGICAL spelling
  //    lands on the physical accessor.
  R.push('sMinW:' + st.minWidth);
  R.push('sMaxW:' + st.maxWidth);
  R.push('sMinH:' + st.minHeight);
  R.push('sMaxH:' + st.maxHeight);
  R.push('saMinH:' + sa.minHeight);
  R.push('saMaxW:' + sa.maxWidth);
  R.push('slMinH:' + sl.minHeight);
  R.push('slMaxW:' + sl.maxWidth);
  // CONTROL — the axis the stretch rows did NOT set must still read unset, so "stretch" cannot be
  // leaking onto every property from a shared flag.
  R.push('saMaxH:' + sa.maxHeight);
  // ⚠ Asserted as a PREDICATE, not as a string, and deliberately: our unset `min-width` reads
  //   `auto` where Chrome reads `0px` (measured this tick, /tmp/t1251-q.html). That is a real and
  //   SEPARATE divergence, and writing `saMinW:auto` here would PIN the engine to it (t1004). The
  //   claim this control exists to make is only that `stretch` does not LEAK onto a property the
  //   author never set — which this states exactly, and which stays true after that bug is fixed.
  R.push('saMinWnotstretch:' + (sa.minWidth !== 'stretch'));
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn the_intrinsic_min_max_keywords_read_back() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://imm-cssom.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("INTRINSIC MIN/MAX CSSOM: {got}");

    for (claim, why) in [
        (
            "minW:min-content",
            "`getComputedStyle(el).minWidth` on `min-width:min-content` is the KEYWORD. It read \
             `auto` — the string for unset — while layout was honouring the floor",
        ),
        (
            "maxW:fit-content",
            "`max-width:fit-content` reads back the keyword; it read `none`, which is what a script \
             checks to conclude there is NO cap",
        ),
        (
            "sMinW:stretch",
            "⚠ I3: `min-width:stretch` is SIZED correctly by layout since t1250 and must also be              VISIBLE — `stretch` collapses to `Dim::Auto` exactly as `min-content` does, so without              its own sidecar the box reads back as UNSET (Chrome: \"stretch\")",
        ),
        ("sMaxW:stretch", "the inline max, same rule"),
        ("sMinH:stretch", "the block min, same rule"),
        ("sMaxH:stretch", "the block max, same rule"),
        (
            "saMinH:stretch",
            "the `-webkit-fill-available` ALIAS normalises to `stretch` (Chrome-measured), so a              script comparing against one spelling is not sent down a polyfill path",
        ),
        ("saMaxW:stretch", "the alias on the inline max"),
        (
            "slMinH:stretch",
            "the LOGICAL spelling `min-block-size:stretch` reaches the PHYSICAL accessor — the              drift `extra_computed_props` has already caught once",
        ),
        ("slMaxW:stretch", "logical `max-inline-size:stretch`"),
        (
            "saMaxH:none",
            "CONTROL: an axis the stretch rows did not set still reads UNSET — `stretch` must not              leak across properties from a shared flag",
        ),
        (
            "saMinWnotstretch:true",
            "CONTROL: the same on a min. Asserted as a predicate because our unset `min-width` says              `auto` where Chrome says `0px` — a separate divergence, and pinning this gate to              either string would pin the engine to one of them",
        ),
        ("minH:max-content", "the block-axis min, same rule"),
        ("maxH:min-content", "the block-axis max, same rule"),
        (
            "minIS:min-content",
            "the LOGICAL spelling must agree with the physical one — `extra_computed_props` has \
             already caught these two drifting apart once",
        ),
        ("maxIS:fit-content", "logical `max-inline-size`"),
        ("minBS:max-content", "logical `min-block-size`"),
        ("maxBS:min-content", "logical `max-block-size`"),
        (
            "gpv:true",
            "`getPropertyValue('max-width')` and `.maxWidth` are two doors onto one value and must \
             not answer differently",
        ),
        // ── CONTROLS. A serialiser that returned the keyword unconditionally passes everything
        //    above and destroys every ordinary length.
        ("cMinW:30px", "a real `min-width` length is untouched"),
        ("cMaxW:40px", "a real `max-width` length is untouched"),
        ("cMinH:50px", "a real `min-height` length is untouched"),
        ("cMaxH:60px", "a real `max-height` length is untouched"),
        (
            "uMinW:auto",
            "an UNSET min still serialises `auto` — the keyword sidecar must be absent, not empty",
        ),
        (
            "uMaxW:none",
            "an UNSET max still serialises `none`, NOT `auto`; that asymmetry is the CSSOM rule and \
             it survives the change",
        ),
        (
            "fnMaxW:none",
            "`max-width:fit-content(50px)` is INVALID and is dropped — Chrome-measured. Accepting \
             the functional form here would make us lay out a box Chrome does not",
        ),
    ] {
        assert!(
            got.split_whitespace().any(|t| t == claim),
            "G_INTRINSIC_MIN_MAX_CSSOM: missing `{claim}`.\n  {why}\n  got: {got}"
        );
    }
}
