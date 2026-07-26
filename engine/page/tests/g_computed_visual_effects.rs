//! **G_COMPUTED_VISUAL_EFFECTS — the bundle reads back as STRINGS, because `undefined` throws.**
//!
//! Ticks 592-595 made `filter`, `backdrop-filter`, `clip-path` and `mix-blend-mode` render. All four
//! still read back `undefined` from `getComputedStyle`, and that is not the smaller half of the
//! problem — it is a **different and worse failure**:
//!
//! ```js
//! if (getComputedStyle(el).filter.indexOf('blur') !== -1) { … }   // TypeError, frame dead
//! ```
//!
//! A missing *rendering* degrades the page. A missing *string* throws in the caller and stops the
//! script. The CSSOM contract is that every supported property is a string, **always** — and `"none"`
//! is a perfectly good answer. This is the third sighting of one defect class: t576 found it on
//! `getPropertyValue`, t590 re-found it on `appearance`, and it is here four properties wide.
//!
//! ## What is asserted
//!
//! - **Type before value.** Every property is `typeof === 'string'` even when unset. A test that only
//!   checked the *set* case would pass while `undefined.indexOf` still killed every page that
//!   feature-detects before styling — which is the majority of them.
//! - **The default is `"none"` / `"normal"`, not `""`.** An empty string is falsy, so
//!   `if (cs.filter)` silently takes the wrong branch.
//! - **Round-trip.** A set value serializes to something the page can parse back.
//! - **Prefixed aliases agree.** A page that feature-detects on `webkitFilter` and then reads
//!   `filter` (or the reverse) must not see a hole.
//! - **`getPropertyValue` agrees with the property.** Those two disagreeing about one declaration is
//!   the tick-282 bug: whichever the page consults, it gets a different browser.
//! - **Enumeration reaches them.** `length`/`item(i)` are derived from the same list, so a property
//!   added to one and not the other is unreachable — the exact drift `G_COMPUTED_CUSTOM_PROPERTIES`
//!   caught when `length` was a hand-maintained constant.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  #plain { width: 10px; height: 10px }
  #fx {
    width: 10px; height: 10px;
    filter: blur(4px) grayscale(0.5);
    backdrop-filter: blur(8px);
    clip-path: circle(50%);
    mix-blend-mode: multiply;
  }
</style></head><body>
<div id="plain"></div><div id="fx"></div>
<div id="out">-</div>
<script>
  var R = [];
  var p = getComputedStyle(document.getElementById('plain'));
  var f = getComputedStyle(document.getElementById('fx'));
  // ── TYPE FIRST: the throw-class claim. `undefined.indexOf` is what kills the page.
  ['filter','backdropFilter','clipPath','mixBlendMode'].forEach(function(k){
    R.push('t_' + k + ':' + (typeof p[k]));
  });
  // ── The UNSET defaults must be the CSS initial keywords, not ''.
  R.push('d_filter:' + p.filter);
  R.push('d_backdrop:' + p.backdropFilter);
  R.push('d_clip:' + p.clipPath);
  R.push('d_blend:' + p.mixBlendMode);
  // ── The SET values round-trip.
  R.push('s_filter:' + f.filter);
  R.push('s_backdrop:' + f.backdropFilter);
  R.push('s_blend:' + f.mixBlendMode);
  R.push('s_clip_has_circle:' + (f.clipPath.indexOf('circle') === 0));
  // ── Prefixed aliases agree with the unprefixed property.
  R.push('pfx_filter:' + (f.webkitFilter === f.filter));
  R.push('pfx_backdrop:' + (f.webkitBackdropFilter === f.backdropFilter));
  // ── getPropertyValue agrees, including through the prefixed spelling.
  R.push('gpv_filter:' + (f.getPropertyValue('filter') === f.filter));
  R.push('gpv_blend:' + (f.getPropertyValue('mix-blend-mode') === f.mixBlendMode));
  R.push('gpv_wk:' + (f.getPropertyValue('-webkit-filter') === f.filter));
  R.push('gpv_unset:' + (p.getPropertyValue('filter') === 'none'));
  // ── THE IDIOM THAT WAS THROWING, run for real on an unstyled element.
  var threw = false;
  try { p.filter.indexOf('blur'); p.clipPath.indexOf('circle'); } catch (e) { threw = true; }
  R.push('idiom_threw:' + threw);
  // ── Enumeration reaches them (length/item are derived from one list).
  var names = [];
  for (var i = 0; i < f.length; i++) names.push(f.item(i));
  R.push('enum_filter:' + (names.indexOf('filter') !== -1));
  R.push('enum_clip:' + (names.indexOf('clip-path') !== -1));
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn the_visual_effects_bundle_reads_back_as_strings() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://cssom.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("COMPUTED VISUAL EFFECTS: {got}");

    for (claim, why) in [
        // ── The throw-class claim, one per property.
        (
            "t_filter:string",
            "`getComputedStyle(el).filter` must be a STRING even when unset. `undefined.indexOf` is \
             a TypeError that kills the caller's frame, so this is not a missing feature — it stops \
             the script. A rendering gap degrades a page; this one ends it",
        ),
        ("t_backdropFilter:string", "the same, for `backdrop-filter`"),
        ("t_clipPath:string", "the same, for `clip-path`"),
        ("t_mixBlendMode:string", "the same, for `mix-blend-mode`"),
        // ── The unset value must be the CSS initial keyword, not the empty string.
        (
            "d_filter:none",
            "an unset `filter` is the string `none`, NOT `''` — an empty string is falsy, so \
             `if (cs.filter)` would silently take the wrong branch",
        ),
        ("d_backdrop:none", "an unset `backdrop-filter` is `none`"),
        ("d_clip:none", "an unset `clip-path` is `none`"),
        (
            "d_blend:normal",
            "an unset `mix-blend-mode` is `normal` — its own initial keyword, not `none`",
        ),
        // ── Set values round-trip in source order.
        (
            "s_filter:blur(4px) grayscale(0.5)",
            "the resolved `filter` list serializes in SOURCE ORDER with computed units — the list \
             is a pipeline, so a serializer that sorted or normalised the order would hand the page \
             back a different picture than the one on screen",
        ),
        ("s_backdrop:blur(8px)", "`backdrop-filter` likewise"),
        ("s_blend:multiply", "the blend keyword round-trips"),
        (
            "s_clip_has_circle:true",
            "`clip-path: circle(50%)` serializes as a `circle(...)` function, not as a debug dump",
        ),
        // ── Prefixed aliases.
        (
            "pfx_filter:true",
            "`webkitFilter` and `filter` are the SAME resolved value — a page that feature-detects \
             on the prefixed name and reads the unprefixed one must not find a hole",
        ),
        ("pfx_backdrop:true", "the same for `-webkit-backdrop-filter`"),
        // ── getPropertyValue must not disagree with the property.
        (
            "gpv_filter:true",
            "`getPropertyValue('filter')` and `.filter` must agree. Those two answering differently \
             about one declaration is the tick-282 bug wearing new clothes: whichever the page \
             consults, it gets a different browser",
        ),
        ("gpv_blend:true", "`getPropertyValue('mix-blend-mode')` agrees"),
        (
            "gpv_wk:true",
            "the PREFIXED spelling routes to the same value — the auto-camelCase fallback turns \
             `-webkit-filter` into `WebkitFilter`, which is not a property, so this needs a real map \
             entry and would silently return `''` without one",
        ),
        (
            "gpv_unset:true",
            "`getPropertyValue('filter')` on an unstyled element is `'none'`, not `''`",
        ),
        // ── The idiom itself.
        (
            "idiom_threw:false",
            "THE WHOLE POINT: `getComputedStyle(el).filter.indexOf('blur')` on an element with no \
             filter must not throw. This is the expression half the web writes in one line",
        ),
        // ── Enumeration.
        (
            "enum_filter:true",
            "`item(i)` over `length` must reach `filter` — the name list and the property list are \
             two places one set of properties lives, and a property added to one and not the other \
             is enumerable-invisible. That drift is exactly what G_COMPUTED_CUSTOM_PROPERTIES caught \
             when `length` was a hand-maintained constant",
        ),
        ("enum_clip:true", "…and `clip-path`"),
    ] {
        assert!(
            got.contains(claim),
            "G_COMPUTED_VISUAL_EFFECTS: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
