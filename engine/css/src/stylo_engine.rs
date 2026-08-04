//! Stylo-backed [`StyleEngine`], compiled only under `--features stylo`.
//!
//! CLAUDE.md's reuse target for CSS is Stylo (Servo/Firefox's production engine).
//! Fully driving Stylo's cascade — building its `Device`, `Stylist`, author
//! `CascadeData`, and mapping its `ComputedValues` back onto [`crate::ComputedStyle`]
//! — is a substantial integration and is the follow-on work behind this boundary.
//!
//! For now this adapter *links* Stylo (proving the dependency builds and the
//! feature/trait wiring is correct) and delegates to [`MinimalCascade`] so behavior
//! is well-defined. Replacing the delegation body with a real Stylist run is a
//! change contained entirely to this file — no caller sees the difference.
//!
//! D2 Step-0 (see [`crate::stylo_probe`]) has already proven the *non-DOM half* of
//! that run works here — building a `Device`, parsing with Stylo's own parser, and
//! compiling selectors through a `Stylist`. The `selectors::Element` wall (30 methods)
//! is landed and tested (see [`crate::stylo_dom`]). What still blocks stopping the
//! delegation, confirmed against the on-disk `stylo-0.19.0` source:
//!
//! 1. **The `TElement` type requirement.** Both cascade entry points
//!    (`Stylist::compute_for_declarations` and `properties::cascade`) are
//!    `where E: TElement`, even though the element is passed `None` and no `TElement`
//!    method is called at runtime. Rust still requires *naming* a concrete `E`, so a
//!    type implementing `TElement` must exist — a closed graph of `TDocument` (5) +
//!    `TNode` (20) + `TShadowRoot` (6) + `TElement` (76) methods over the arena.
//! 2. **The `ComputedValues` → [`crate::ComputedStyle`] mapping** (~30 properties over
//!    Stylo's packed computed-value types). Independently testable against
//!    `Device::default_computed_values()` without (1).
//!
//! The **exact, source-verified, step-by-step plan** (signatures, module paths, the
//! `match → merge → compute_for_declarations → read` flow, and the property-mapping
//! table) lives in `docs/parity/STYLO-CASCADE-PLAN.md`. This adapter delegates to
//! [`MinimalCascade`] until that lands, so behaviour is well-defined meanwhile.

use euclid::{Scale, Size2D};
use selectors::context::{
    MatchingContext, MatchingForInvalidation, MatchingMode, NeedsSelectorFlags, SelectorCaches,
};
use selectors::matching::matches_selector;
use stylo::context::QuirksMode;
use stylo::device::servo::FontMetricsProvider;
use stylo::device::Device;
use stylo::font_metrics::FontMetrics;
use stylo::media_queries::{MediaList, MediaType};
use stylo::properties::declaration_block::parse_style_attribute;
use stylo::properties::style_structs::Font;
use stylo::properties::{ComputedValues, PropertyDeclarationBlock};
use stylo::queries::values::PrefersColorScheme;
use stylo::servo_arc::Arc as ServoArc;
use stylo::shared_lock::{SharedRwLock, SharedRwLockReadGuard, StylesheetGuards};
use stylo::stylesheets::container_rule::ContainerCondition;
use stylo::stylesheets::{
    AllowImportRules, CssRule, CssRuleType, CustomMediaEvaluator, DocumentStyleSheet, Namespaces,
    Origin, Stylesheet as StyloStylesheet, UrlExtraData,
};
use stylo::stylist::Stylist;
use stylo::values::computed::font::GenericFontFamily;
use stylo::values::computed::{CSSPixelLength, Length};

use manuk_dom::{Dom, NodeId};

use crate::stylo_dom::{ElementDataStore, StyloElement};
use crate::stylo_map::to_computed_style;
use crate::{MinimalCascade, StyleEngine, StyleMap, Stylesheet};

/// Stylo cascade adapter — a **real** [`StyleEngine`] backed by Stylo's cascade.
///
/// [`Self::cascade`] runs [`cascade_via_stylo`] (UA sheet + author sheets + inline
/// `style=`, matched with Stylo's selector engine, computed with
/// `compute_for_declarations`, mapped to [`ComputedStyle`]) at a default viewport. This is
/// what gives real `var()` / `@media` / spec-complete-selector / `font-family` styling.
/// [`MinimalCascade`] remains the crate default (no heavy build, hand-tuned to the parity
/// harness); Stylo is selected under `--features stylo` by callers that opt in.
#[derive(Debug, Default, Clone, Copy)]
pub struct StyloEngine;

impl StyleEngine for StyloEngine {
    fn cascade(&self, dom: &Dom, sheets: &[Stylesheet]) -> StyleMap {
        // The trait carries no viewport; use a standard one (only affects `@media` /
        // viewport-relative units). Callers with a real viewport use `cascade_via_stylo`.
        cascade_via_stylo(dom, sheets, 1024.0, 768.0)
    }
}

/// The font-metrics provider Stylo queries when resolving font-relative units.
///
/// The one metric it answers with a real number is **`zero_advance_measure`** — the advance
/// of the `0` glyph, which is the definition of the CSS `ch` unit. Left as
/// `FontMetrics::default()` (all `None`), Stylo falls back to the spec's `ch = 0.5em`
/// (`FontMetrics::zero_advance_measure_or_default`), so every `width: Nch` box came out
/// `N * 0.5em` while the text laid into it used the font's REAL advance (monospace `0` ≈
/// `0.6em`): the box was ~17% too narrow and monospace columns / `max-width: 65ch`
/// article measures overflowed. We resolve the family exactly as `layout::text_style` does
/// and measure `0` through the same shaper (`manuk_text::zero_advance_px`), so the metric and
/// the glyphs can never disagree. `x_height`/`cap_height`/`ic_width` stay `None` for now —
/// their spec fallbacks (`ex = 0.5em`, `cap = ascent`, `ic = 1em`) are unchanged, so `ex`
/// is a bounded follow-up and nothing regresses.
#[derive(Debug)]
struct StubFontMetrics;

impl FontMetricsProvider for StubFontMetrics {
    fn query_font_metrics(
        &self,
        _vertical: bool,
        font: &Font,
        base_size: CSSPixelLength,
        _flags: stylo::values::specified::font::QueryFontMetricsFlags,
    ) -> FontMetrics {
        // Family list, in author order, mapped to the plain names the text layer resolves —
        // the SAME extraction `stylo_map` uses when it maps `font-family` onto ComputedStyle.
        use stylo::values::computed::font::SingleFontFamily;
        let mut families: Vec<String> = Vec::new();
        for f in font.font_family.families.list.iter() {
            match f {
                SingleFontFamily::FamilyName(n) => families.push(n.name.to_string()),
                SingleFontFamily::Generic(g) => families.push(
                    match g {
                        GenericFontFamily::Serif => "serif",
                        GenericFontFamily::SansSerif => "sans-serif",
                        GenericFontFamily::Monospace => "monospace",
                        GenericFontFamily::Cursive => "cursive",
                        GenericFontFamily::Fantasy => "fantasy",
                        GenericFontFamily::SystemUi => "system-ui",
                        _ => "sans-serif",
                    }
                    .to_string(),
                ),
            }
        }
        // Bold/italic mirror `layout::text_style`'s FontKey so the resolved face matches.
        let bold = font.font_weight.value() >= 600.0;
        let italic = font.font_style != stylo::values::computed::font::FontStyle::NORMAL;
        let px = base_size.px();
        let zero = manuk_text::zero_advance_px(&families, bold, italic, px);
        // `ex`: the face's real x-height (OS/2 sxHeight), or `None` → Stylo keeps `ex = 0.5em`.
        let x_height = manuk_text::x_height_px(&families, bold, italic, px).map(Length::new);
        // `cap`: the face's real cap-height (OS/2 sCapHeight). Its fallback is `ascent`, which this
        // provider never set — so `cap` resolved to 0px and any `cap`-sized box COLLAPSED. Filling
        // it fixes that; `None` still leaves the (now equally-unset) ascent fallback for a face that
        // declares no cap-height.
        let cap_height = manuk_text::cap_height_px(&families, bold, italic, px).map(Length::new);

        FontMetrics {
            zero_advance_measure: Some(Length::new(zero)),
            x_height,
            cap_height,
            ..FontMetrics::default()
        }
    }
    fn base_size_for_generic(&self, _generic: GenericFontFamily) -> Length {
        Length::new(16.0)
    }
}

/// **The quirks verdict, as Stylo's enum.** Read off the `Dom` every one of these call sites already
/// holds, so no signature has to carry it.
fn qm_of(dom: &Dom) -> QuirksMode {
    if dom.quirks() {
        QuirksMode::Quirks
    } else {
        QuirksMode::NoQuirks
    }
}

/// **Quirks mode matches id and class CASE-INSENSITIVELY**, so the index must be keyed the same way it
/// is queried or the bucket lookup filters candidates out *before* matching ever runs — a half-fix that
/// looks complete, because `MatchingContext` would be saying "case-insensitive" about rules the index
/// had already discarded. Applied at BOTH ends: here when bucketing, and in `candidates` when querying.
fn index_key(v: &str, qm: QuirksMode) -> String {
    if qm == QuirksMode::Quirks {
        v.to_ascii_lowercase()
    } else {
        v.to_string()
    }
}

fn make_device(width: f32, height: f32, quirks: QuirksMode) -> Device {
    Device::new(
        MediaType::screen(),
        quirks,
        Size2D::new(width, height),
        Size2D::new(width, height),
        Scale::new(1.0),
        Box::new(StubFontMetrics),
        ComputedValues::initial_values_with_font_override(Font::initial_values()),
        PrefersColorScheme::Light,
        Default::default(),
        Default::default(),
    )
}

/// A minimal user-agent stylesheet (CSS text, parsed by Stylo like any sheet). Prepended
/// to the author sheets so type selectors get the browser defaults (block/inline/table
/// display, heading sizes, list/table padding) — the Stylo-side analogue of the minimal
/// engine's `apply_ua_defaults`. Author rules win by specificity/order (UA selectors are
/// low-specificity type selectors, parsed first).
const UA_CSS: &str = r#"
html, body, div, section, article, header, footer, nav, main, aside, figure,
figcaption, address, p, blockquote, ul, ol, li, dd, dt, pre, hr, h1, h2, h3, h4, h5, h6,
form, fieldset, table, caption, center, menu, dl { display: block; }
center { text-align: center; }
/* The elements that are never rendered. Ours was missing the *media* half of the list, and
   `<source>` is the one that matters: `<picture><source>` is how the entire modern web serves
   responsive images, and every one of them was getting a real box with real height. Wikipedia alone
   invented 152px out of eight of them, in the middle of the article. Same shape as the `<script>`
   that painted its own source code down rust-lang.org — a metadata element with no `display:none`
   becomes content. Mirrors Chrome's html.css. */
head, title, meta, link, script, style, base, template,
param, datalist, basefont, noembed, noframes, rp { display: none; }
/* ⚠⚠ **`source`, `track`, `area` and `noscript` ARE NOT `display: none` — Chrome computes `inline`
   for all four** (measured with `getComputedStyle`, not recalled; `param`/`datalist`/`template`/`rp`
   really are `none` and stay above). They generate no box because their PARENT consumes them —
   `<picture>`/`<video>` render their `<img>`/media, `<map>` is not a container, `<noscript>` with
   scripting enabled holds raw text — which is a STRUCTURAL fact, not a stylesheet one. Hiding them
   here produced the right box and the wrong answer, and `getComputedStyle(source).display` is exactly
   what a responsive-image shim reads. The structural rule now lives in `layout::never_rendered`,
   where an author's `source { display: block }` cannot override it either. */
/* Form controls are **atomic inline boxes**, not inline elements with children. Left as plain
   `inline`, the inline collector recurses into a `<select>`'s `<option>`s and paints every one of
   them into the surrounding line — rust-lang.org's language picker rendered as a row of twelve
   language names instead of a dropdown. A control shows ITS OWN text (the selected option, the
   value, the placeholder); its children are its data, not its content. */
input, select, textarea, button, meter, progress { display: inline-block; }
option, optgroup { display: none; }
/* Form controls are WIDGETS: a browser draws them, the page does not. Without a border, a
   background and an intrinsic size, a checkbox is nothing at all — every form on the web rendered
   its labels next to empty space. (These are UA rules, lowest specificity: any author styling
   still wins.) */
/* **A FORM CONTROL DOES NOT INHERIT THE PAGE'S FONT.** Chrome's `html.css` gives every control
   `font: -webkit-small-control`, which resolves to the ~13.3px system UI face — NOT the 16px the
   document is using. Inheriting it here made every control ~20% too big in both axes, and since a
   text field's intrinsic width is measured in characters, the error came straight back out as a
   wrong BOX width on every form on the web. Authors who want inheritance ask for it (`input {
   font: inherit }` is in most CSS resets) and this is UA-origin, so they still win. */
/* ⚠⚠ **`line-height` IS THE THIRD PROPERTY OF THAT SHORTHAND, AND LEAVING IT OUT LET THE PAGE'S OWN
   LINE-HEIGHT BACK IN.** `font: -webkit-small-control` is a SHORTHAND, so it resets `line-height` to
   `normal` — and a UA *declared* value beats inheritance, whatever the author put on `<body>`. We set
   only the family and the size, so `body { line-height: 1.7 }` — the single most common typographic
   rule on the modern web — inherited straight into every control and multiplied its height. A
   `<textarea rows=5>` at 16px measured 5 × 27.2 + 2 = **138** against Chrome's 5 × 19 + 2 = **97**,
   and a text field 29 against 20. Because a textarea's height is rows × line-height, the error is
   proportional to the control and lands on every form on the web that styles its body text.
   ⚠ Still UA-origin, so an author's OWN `line-height` on the control wins — asserted in the gate
   (`line-height:2` on a textarea is 98 in both engines, before and after). */
input, select, textarea, button {
  font-family: Arial, sans-serif;
  font-size: 13.333px;
  line-height: normal;
}
input, textarea, select {
  border: 1px solid #767676;
  background-color: #ffffff;
  padding: 1px 2px;
  color: #000000;
}
input[type=checkbox], input[type=radio] {
  padding: 0;
  background-color: #ffffff;
}
/* `:checked` now matches (it did not, until this tick) — so a ticked box can finally LOOK ticked. */
input[type=checkbox]:checked, input[type=radio]:checked { background-color: #1a73e8; }
input[type=radio] { border-radius: 7px; }
input[type=submit], input[type=reset], input[type=button], button {
  background-color: #efefef;
  border: 1px solid #767676;
  padding: 1px 6px;
  text-align: center;
}
/* **BUTTONS AND `<select>` ARE `border-box`; TEXT FIELDS AND `<textarea>` ARE NOT.** Chrome's UA
   sheet draws that line and it is not intuitive — the controls that look most alike are on opposite
   sides of it. Measured at `height:50px; padding-top:20px`, used border-box height:

     button  submit  text  select  textarea  div
       50      50     70     50       70      70     Chrome
       70      70     70     70       70      70     us, before this rule

   So a button, a submit input and a select were **too tall by exactly their vertical padding plus
   borders** on every page that sets a height and padding on them — which is what every design system
   does to a button. It also blocked the button-centring rule landed at t850: that divides the slack
   in the CONTENT box, so it cannot be right until the content box is.
   ⚠ UA-origin, so an author's own `box-sizing` still wins — asserted in the gate. */
input[type=submit], input[type=reset], input[type=button], button, select {
  box-sizing: border-box;
}
input[type=hidden] { display: none; }
/* `hidden` global attribute — https://html.spec.whatwg.org/#hidden-elements. An element carrying the
   boolean `hidden` attribute is NOT rendered. This is one of the most common visibility toggles on
   the whole web: feature-detect fallbacks, tab panels, initial-collapsed accordions, `el.hidden =
   false` show/hide. Without this rule `<div hidden>` reported `display:block` and painted its
   contents into the page (measured — the plain global attribute was never in the sheet; only the
   `input[type=hidden]` value was). The `:not([hidden="until-found"])` exception matches the spec:
   `until-found` is rendered with `content-visibility: hidden` (findable-but-collapsed), which we do
   not yet support — so we leave it visible rather than falsely collapse content we cannot later
   reveal on find. Keep in lockstep with `apply_ua_defaults` in css/src/lib.rs. */
[hidden]:not([hidden="until-found"]) { display: none; }
/* `<dialog>` — a CLOSED dialog is not rendered. Without this rule a dialog is just a block, so
   every modal's contents (the confirm-delete copy, the cookie-consent form, the command palette)
   were painted into the middle of the page before anyone opened it. Chrome's html.css has the same
   pair: hidden until `open`, then a centered auto-margin box. Keep in lockstep with
   `apply_ua_defaults` in css/src/lib.rs — the two cascades disagreeing about whether a modal
   renders is exactly the `<source>` bug again. */
/* `<details>` — a CLOSED disclosure renders ONLY its summary. Without this every collapsible on
   GitHub (every "Show diff", every folded review comment), MDN and every docs site rendered
   permanently expanded, which is not a cosmetic difference: a page of collapsed sections becomes a
   wall of everything at once, and the summary loses any meaning. Same lockstep requirement as
   `<dialog>` below — `apply_ua_defaults` + `cascade_node` in css/src/lib.rs must agree, or the two
   cascades disagree about whether a section renders. */
summary { display: block; }
details > *:not(summary) { display: none; }
details[open] > * { display: block; }
dialog { display: none; }
dialog[open] {
  display: block;
  margin: auto;
  border: 2px solid #767676;
  background-color: #ffffff;
  color: #000000;
  padding: 1em;
}
/* `[popover]` — the same rule shape as `<dialog>`, for the same reason: a popover (menu, tooltip,
   dropdown, toast) is hidden until shown, and without this its contents render inline in the middle
   of the page. `popover` is a GLOBAL attribute, so this is keyed on the attribute, not a tag.
   `data-manuk-popover-open` is what `showPopover()` sets — the `:popover-open` state, in a form the
   Rust top-layer stacking can also read. */
[popover] { display: none; }
[popover][data-manuk-popover-open] {
  display: block;
  border: 1px solid #767676;
  background-color: #ffffff;
  color: #000000;
  padding: 0.25em;
}
/* ── Vertical block metrics. Measured out of real Chrome (`createElement` + `getComputedStyle`),
   not recalled from the spec. This sheet had `p`/`blockquote`/`h1-h6` and NOTHING else, while
   `apply_ua_defaults` in css/src/lib.rs — the OTHER cascade — already carried `ul`/`ol` at 1em and
   `body` at 8px. The two had drifted apart on the property that decides where everything below a
   list lands, and since Stylo is the live path for every real page, the live path was the wrong one.
   The FID-SWEEP's near-miss population (mdx=0, mdy=12..82, growing with content density) is this. */
body { margin: 8px; }
p { margin: 1em 0; }
/* Chrome indents a blockquote 40px on BOTH sides. `margin: 1em 0` does not merely omit that, it
   explicitly ZEROES it — a quote sat flush with the body text it is quoted from. */
blockquote, figure { margin: 1em 40px; }
ul, ol, menu { margin: 1em 0; }
/* A NESTED list gets NO vertical margin. Chrome's html.css says so, and it is the rule a
   from-memory implementation always misses: giving every list 1em unconditionally fixes the
   top-level case and newly over-spaces every nested menu, sidebar and table of contents on the
   web — which is precisely the shape (Wikipedia's `#p-tb` → `#n-randompage`, dy=-61) that sent
   us looking here. */
ul ul, ul ol, ol ul, ol ol, menu menu, ul menu, menu ul, ol menu, menu ol {
  margin-top: 0; margin-bottom: 0;
}
dl { margin: 1em 0; }
/* `dd` is indented from its `dt`, and `dt` is NOT — the pair is the whole visual grammar of a
   definition list. Indent both and it collapses back to a flat run of alternating lines. */
dd { margin-left: 40px; }
/* 1em of `pre`'s OWN 13px monospace font, so 13px — not 16px. */
pre { margin: 1em 0; }
hr { margin: 0.5em 0; }
h1 { font-size: 2em; font-weight: bold; margin: 0.67em 0; }
h2 { font-size: 1.5em; font-weight: bold; margin: 0.75em 0; }
h3 { font-size: 1.17em; font-weight: bold; margin: 0.83em 0; }
h4 { font-weight: bold; margin: 1.12em 0; }
h5 { font-size: 0.83em; font-weight: bold; margin: 1.5em 0; }
h6 { font-size: 0.75em; font-weight: bold; margin: 1.67em 0; }
b, strong, th { font-weight: bold; }
ul, ol { padding-left: 40px; }
/* Chrome's UA sheet underlines links and puts a marker on list items. Ours did neither, so every
   link on the web was bare text and every list was an indent. */
a:link, a:visited { text-decoration: underline; }
ul { list-style-type: disc; }
ol { list-style-type: decimal; }
u, ins { text-decoration: underline; }
s, del, strike { text-decoration: line-through; }
abbr[title] { text-decoration: underline; }
/* ⚠ **`border-spacing: 2px` IS IN CHROME'S UA SHEET AND WAS MISSING FROM OURS (t908).** The
   separated-borders model insets every cell from the table edge and from its neighbours by this
   much, so a DEFAULT `<table>` — no author CSS at all, which is most of the data tables on the web
   — had every cell 4px too wide, flush against the table edge, and the table itself 4px too short
   per row. Chrome-measured, a 200px table with one `padding:0` cell: `<td>` at x=2 w=196 and the
   table 28 tall; ours was x=0 w=200 and 24. The property was already parsed, applied and
   Chrome-exact when an author SET it (`border-spacing:10px` matched to the pixel) — only the
   default was absent, which is why nothing caught it. */
table { display: table; border-spacing: 2px; }
thead, tbody, tfoot { display: table-row-group; }
tr { display: table-row; }
td, th { display: table-cell; padding: 1px; }
caption { display: table-caption; }
/* `pre` preserves whitespace. Chrome's UA sheet says so; ours did not, so every code block on
   the web folded its newlines into spaces and rendered as one endless line. */
/* Chrome's default MONOSPACE font size is 13px, not 16px — which is why `<code>` famously renders
   smaller than the prose around it. `font-size: medium` resolves against the monospace default when
   the family is monospace. We rendered monospace at 16px, so every code block and every inline
   `<code>` on the web was 23% too large, and every documentation site's layout was pushed down by
   it. (Found by the differential oracle on its first run: our <pre> was 57px where Chromium's was
   45px.) */
pre, code, kbd, samp, tt { font-size: 13px; }
pre { font-family: monospace; white-space: pre; }
textarea { white-space: pre-wrap; }
code, kbd, samp { font-family: monospace; }
"#;

/// The real Stylo value cascade over `sheets`' author rules: build a `Stylist`, and for
/// each element match rules with Stylo's selector matcher (via our `selectors::Element`),
/// merge the winning declarations, compute `ComputedValues` with `compute_for_declarations`
/// (no `TElement` instance — `element = None`), and map the result onto our
/// [`ComputedStyle`], inheriting from each element's parent. This is what gives real
/// `var()` / `@media` / full-selector / `font-family` computation.
pub fn cascade_via_stylo(dom: &Dom, sheets: &[Stylesheet], vw: f32, vh: f32) -> StyleMap {
    cascade_via_stylo_sized(dom, sheets, vw, vh, None)
}

/// [`cascade_via_stylo`] with the previous layout pass's per-node **content-box** sizes, which
/// is what makes `@container` rules live: conditions are evaluated per element against its
/// nearest ancestor container (Stylo's own `ContainerCondition::matches`, driven through our
/// `TElement::query_container_size`). Without sizes (`None` — every first pass), container-gated
/// rules stay off: a container query answered before layout has run would be a guess, and the
/// spec's own model is query-after-container-layout.
pub fn cascade_via_stylo_sized(
    dom: &Dom,
    sheets: &[Stylesheet],
    vw: f32,
    vh: f32,
    container_sizes: Option<std::collections::HashMap<NodeId, (f32, f32)>>,
) -> StyleMap {
    // Stylo's `grid_enabled()` reads `layout.grid.enabled` (off by default under the `servo`
    // feature), which makes it drop `display:grid` at parse time. Flip it on once so grid
    // containers cascade. Idempotent + cheap; safe to call every cascade.
    stylo_static_prefs::set_pref!("layout.grid.enabled", true);
    // Same shape for container queries: `container-type`/`container-name` are dropped at parse
    // time unless this pref is on (the `@container` RULE parses regardless — which is how tick
    // 371's probe saw parse alive while the property silently vanished).
    stylo_static_prefs::set_pref!("layout.container-queries.enabled", true);
    // `user-select` (and its `-moz-`/`-webkit-` prefixes) is gated behind Stylo's shared
    // `layout.unimplemented` servo_pref — off by default, so the servo build drops it at parse and
    // every element's computed value stays `auto`. Flip it on so the property cascades and
    // `getComputedStyle(el).userSelect` reflects it.
    //
    // ⚠ **This comment used to end "…so enabling the other properties it also ungates changes
    // nothing we read", and that was measured FALSE at tick 576.** The pref ungates 35 longhands;
    // four are rendered here (`user-select`, `color-scheme`, `mask-image`, `text-overflow`) and the
    // other 31 became *parseable* as a side effect — which is exactly the question `@supports` and
    // `CSS.supports()` answer. So the flip silently promised `backdrop-filter`, `view-transition-name`,
    // the `mask-*` family and 28 more, and every page that feature-detects them threw away a working
    // fallback. `PARSE_ONLY_LONGHANDS` names the 31 and `honest_supports` subtracts them; keep the
    // two in step when this list changes.
    stylo_static_prefs::set_pref!("layout.unimplemented", true);
    // `contrast-color(<color>)` (CSS Color 5, Baseline 2026) is gated behind its own
    // `layout.css.contrast-color.enabled` pref — off by default, so `color: contrast-color(black)` is
    // dropped at parse and the declaration falls back. Flip it on: Stylo then parses the function and
    // computes a `ComputedColor::ContrastColor`, which our color mapping already resolves to the
    // black/white companion through `resolve_to_absolute` (the accessible-theming idiom: pick the
    // legible text color for a dynamic background without JS).
    stylo_static_prefs::set_pref!("layout.css.contrast-color.enabled", true);
    // **The parser's verdict, read off the `Dom` it already handed us.** Everything below that used to
    // say `QuirksMode::NoQuirks` unconditionally now says `qm`. Stylo already implements the quirks
    // themselves (unitless lengths, case-insensitive id/class matching, the `<font size>` table) — this
    // function was simply never telling it which mode the document was in.
    let qm = qm_of(dom);
    let lock = SharedRwLock::new();
    let Ok(url) = ::url::Url::parse("about:manuk") else {
        return MinimalCascade.cascade(dom, sheets);
    };
    let url_data = UrlExtraData(ServoArc::new(url));

    let mut ph = Phases::default();
    #[cfg(not(target_arch = "wasm32"))]
    let t_all = std::time::Instant::now();
    // Parse each sheet's raw source with Stylo's own parser; keep the Arcs so we can
    // iterate their compiled rules for matching.
    let mut stylo_sheets: Vec<ServoArc<StyloStylesheet>> = Vec::new();
    let mut stylist = Stylist::new(make_device(vw, vh, qm), qm);
    // ── THE UA SHEET IS A UA-ORIGIN SHEET, and saying so is the whole of the cascade's first sort.
    //
    // This used to hand `UA_CSS` to the Stylist as `Origin::Author` on the reasoning that it is
    // appended FIRST, so author rules override it. That confuses two different tie-breaks. The
    // cascade sorts by **origin, then importance, then specificity, and only then document order**
    // — so as an author sheet, `body { margin: 8px }` (specificity 0,0,1) beat an author
    // `* { margin: 0 }` (0,0,0), which is on a large fraction of the open web: it is the first rule
    // of Tailwind's preflight, of Normalize, and of every hand-rolled reset since 2004. A reset is
    // deliberately written with the WEAKEST possible selector, which is exactly the shape that loses
    // a specificity tie-break — so being one origin too high made this sheet beat the rules that
    // exist to override it. Measured against live Chromium at tick 556: Chromium `body [0 0 1200×92]`,
    // ours `[8 8 1184×91]`, and the 8px was the smallest instance (`ul,ol { padding-left: 40px }`
    // and `blockquote { margin: 1em 40px }` survived a reset the same way).
    //
    // The origin is declared HERE and consumed by `origin_rank` in `RuleIndex`/`PseudoIndex`,
    // because the Stylist's own cascade is not what decides this page: this engine matches through
    // its own `RuleIndex` and merges the winners itself (see `cascade_element`). Stamping the sheet
    // truthfully is what lets that merge sort on origin instead of on position.
    let ua_sheet = Stylesheet::parse(UA_CSS);
    let all_sheets: Vec<&Stylesheet> = std::iter::once(&ua_sheet).chain(sheets.iter()).collect();
    {
        let guard = lock.read();
        for (i, sheet) in all_sheets.iter().enumerate() {
            let media = ServoArc::new(lock.wrap(MediaList::empty()));
            let origin = if i == 0 {
                Origin::UserAgent
            } else {
                Origin::Author
            };
            let parsed = StyloStylesheet::from_str(
                sheet.source(),
                url_data.clone(),
                origin,
                media,
                lock.clone(),
                None,
                None,
                // THE load-bearing one for the unitless-length quirk: it reaches
                // `ParserContext::quirks_mode`, which is what `AllowQuirks::allowed` consults when
                // deciding whether `width: 100` is 100px or a parse error.
                qm,
                AllowImportRules::Yes,
            );
            let arc = ServoArc::new(parsed);
            stylist.append_stylesheet(DocumentStyleSheet(arc.clone()), &guard);
            stylo_sheets.push(arc);
        }
        timed(&mut ph.flush_ns, || {
            stylist.flush(&StylesheetGuards::same(&guard))
        });
    }

    CASCADES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut store = ElementDataStore::new();
    // The sized re-pass: install the laid-out sizes and pre-create every element's data cell,
    // because Stylo's container lookup reads each ANCESTOR's `borrow_data()` primary style to
    // filter by `container-type`/`container-name` — the preorder walk below fills the styles in
    // before any descendant queries them. On the unsized pass none of this is needed (container
    // rules are held off wholesale), so non-container pages pay nothing.
    let cq_active = container_sizes.is_some();
    if let Some(sizes) = container_sizes {
        store.set_container_sizes(sizes);
        for n in dom.flat_descendants(dom.root()) {
            if dom.is_element(n) {
                store.ensure(n);
            }
        }
    }
    let store = store;
    let guard = lock.read();
    let guards = StylesheetGuards::same(&guard);

    // Built ONCE for the document, not once per element. This is what turns the cascade from
    // O(elements × rules) into O(elements × rules-that-could-match).
    // ⚠ `Instant::now()` PANICS on `wasm32-unknown-unknown` — there is no clock there
    // (`std::sys::pal::wasm::unsupported::time`). One debug-only timing line took down the ENTIRE
    // cascade in the browser demo, and the failure surfaced as `RuntimeError: unreachable` from inside
    // the wasm module — a diagnosis that points nowhere near a `tracing::debug!`.
    //
    // A measurement must never be able to break the thing it measures.
    #[cfg(not(target_arch = "wasm32"))]
    let _ti = std::time::Instant::now();
    let mut index = RuleIndex::build(&stylo_sheets, &guard, stylist.device(), qm);
    {
        // The `@container` supplement — author sheets only (the UA sheet has none).
        let author: Vec<&Stylesheet> = sheets.iter().collect();
        index.add_container_supplement(&author, &lock, &url_data, &guard, stylist.device(), qm);
    }
    let index = index;
    #[cfg(not(target_arch = "wasm32"))]
    tracing::debug!(
        ms = _ti.elapsed().as_millis(),
        rules = index.rules.len(),
        universal = index.universal.len(),
        by_class = index.by_class.len(),
        by_tag = index.by_tag.len(),
        by_id = index.by_id.len(),
        "RULEINDEX"
    );
    // The ::before/::after rules, hoisted out of the sheet tree ONCE — the same trick `RuleIndex`
    // plays for element rules, applied to the path that never got it. See `PseudoIndex`.
    let pseudo_index = PseudoIndex::build(&stylo_sheets, &guard, stylist.device(), qm);
    let mut candidates: Vec<u32> = Vec::new();
    let mut caches = SelectorCaches::default();
    let mut pseudo_caches = SelectorCaches::default();

    // The recovered-property cascade (see the merge loop below) — computed BEFORE the Stylo walk
    // because ONE recovered property must be visible to `apply_presentational_hints` inside the
    // walk: `field-sizing: content` stands the UA intrinsic-width hints down, so it has to be on
    // the style before the hint decides to fire (the other recovered properties merge after).
    let minimal = timed(&mut ph.minimal_ns, || MinimalCascade.cascade(dom, sheets));
    let mut map: StyleMap = StyleMap::new();
    // Preorder walk so a parent's ComputedValues exists before its children's cascade.
    let mut parent_cv: std::collections::HashMap<NodeId, ServoArc<ComputedValues>> =
        std::collections::HashMap::new();
    let mut stack: Vec<NodeId> = vec![dom.root()];
    while let Some(node) = stack.pop() {
        // Push children (reverse so we pop them in document order).
        // **The FLAT tree.** Walking `children()` skips shadow roots entirely — they hang off the
        // host in their own field — so every node inside every web component went unstyled. And an
        // unstyled node is not merely mis-styled: `is_rendered` drops it from the render tree, so the
        // whole component produced ZERO boxes. Lit rendered nothing; so does every design system on
        // the web.
        let kids: Vec<NodeId> = dom.flat_children(node);
        for &k in kids.iter().rev() {
            stack.push(k);
        }
        if !dom.is_element(node) {
            // Text/other non-element nodes have no cascade of their own but inherit their
            // parent element's computed style. Layout indexes a style for *every* node it
            // walks, so — like MinimalCascade — we must give them one. The preorder walk
            // guarantees the parent is already in `map`.
            if let Some(parent) = dom.parent(node) {
                if let Some(ps) = map.get(&parent).cloned() {
                    map.insert(node, ps);
                }
            }
            continue;
        }
        let el = StyloElement::new(dom, node, &store);
        let cv = timed(&mut ph.element_ns, || {
            cascade_one_element(
                &stylist,
                &index,
                &mut candidates,
                &mut caches,
                &lock,
                &url_data,
                &guard,
                &guards,
                &el,
                node,
                &parent_cv,
                dom,
                cq_active,
            )
        });
        // **`rem` is root-relative.** The device carries the root font size that every `rem` in the
        // document resolves against, and it starts at the initial 16px. Unless it is updated once
        // the root element's own font size is known, `html{font-size:62.5%}` — the "1rem = 10px"
        // idiom half the web is built on — silently leaves every `rem` 60% too large, and
        // `html{font-size:118%}` leaves them all too small. Set it as soon as the root is cascaded;
        // the preorder walk reaches `<html>` first, and its OWN `rem` values still resolve against
        // the initial size, which is exactly what CSS specifies.
        if dom.tag_name(node) == Some("html") {
            stylist
                .device()
                .set_root_font_size(cv.get_font().clone_font_size().computed_size().px());
            // **…and `rlh` is root-relative in exactly the same way, which we were not doing.**
            //
            // Stylo's own `matching.rs` sets these two together, four lines apart, under the comments
            // *"Update root font size for rem units"* and *"Update root line height for rlh units"*.
            // We had the first and not the second, so `rlh` fell back to the device's initial value
            // and resolved against **neither** the root's line-height nor the element's.
            //
            // Chrome-measured, surface audit #44 (t721), root `line-height:2` on `16px` and an
            // element `line-height:20px`: `width:5rlh` is **160** in Chrome and was **96** here —
            // `5 x 19.2`, i.e. `5 x (16 x 1.2)`, the INITIAL `normal` line-height. Not root-relative,
            // not element-relative: initial-relative, which is the one answer no author can predict.
            //
            // ⚠ The map called this capability `works` from tick 509 to tick 721 because the probe
            // behind it tests `width:5lh` and its own receipt says `rlh` was *"not separately
            // geometry-tested"*. The untested half was the broken half.
            // Through the DEVICE's `calc_line_height`, not `ComputedValues`': the latter is
            // gecko-conditional in this build, and the device method is the servo one Stylo itself
            // calls. ⚠ It returns **0** for `line-height: normal` (`device/servo.rs`: *"TODO: compute
            // `normal` from the font metrics"*), so a root that never states a line-height leaves
            // `rlh` at zero rather than at the initial guess — honest, and a named residue, not a
            // regression: the previous value was wrong for every root.
            let root_lh = stylist
                .device()
                .calc_line_height(cv.get_font(), cv.writing_mode, None)
                .0
                .px();
            stylist.device().set_root_line_height(root_lh);
            // **…and the THIRD member of the set, which t722 named and did not build.**
            //
            // `rcap` / `rch` / `rex` / `ric` resolve against the ROOT's font METRICS, and Stylo reads
            // those out of `device.root_style` — a field nothing here ever wrote, so every one of
            // them was measured against the device's default style instead of the document's root.
            // Chrome-measured, root `32px` / element `16px` sans-serif: `10rch` **178 vs 80**,
            // `10rex` **169 vs 73**, `10rcap` **220 vs 105**, while their element-relative twins
            // `10ch`/`10ex`/`10cap` were already **exact** (89/85/110). Every element-relative unit
            // right and every root-relative one wrong is the signature of a root that was never
            // published.
            //
            // `update_root_font_metrics` queries the font stack, which is not free — so it runs only
            // when the document has actually used one of these units, exactly as Stylo's own
            // `matching.rs` gates it (`used_root_font_metrics()`). A page with no `r*` unit pays a
            // bool read.
            stylist.device().set_root_style(&cv);
            if stylist.device().used_root_font_metrics() {
                stylist.device().update_root_font_metrics();
            }
        }
        let mut cs = timed(&mut ph.computed_ns, || to_computed_style(&cv));
        // `field-sizing` predates stylo 0.19, so it is recovered from MinimalCascade — and it must
        // be recovered HERE, before the hints, because its whole job is to veto the UA
        // intrinsic-width hint below.
        if let Some(m) = minimal.get(&node) {
            cs.field_sizing_content = m.field_sizing_content;
            // `appearance: none` — same gecko-only fence, and it must land before the hints for
            // the same reason: a `<select>`'s reserved arrow width is decided downstream of it.
            cs.appearance_none = m.appearance_none;
        }
        timed(&mut ph.hints_ns, || {
            apply_presentational_hints(dom, node, &mut cs)
        });
        // `::before` / `::after` — generated content, cascaded against this element as its parent.
        use stylo::selector_parser::PseudoElement as Pe;
        if !pseudo_index.is_empty() {
            timed(&mut ph.pseudo_ns, || {
                cs.before = cascade_pseudo(
                    &stylist,
                    &pseudo_index,
                    &mut pseudo_caches,
                    &lock,
                    &guard,
                    &guards,
                    &el,
                    &cv,
                    Pe::Before,
                )
                .map(Box::new);
                cs.after = cascade_pseudo(
                    &stylist,
                    &pseudo_index,
                    &mut pseudo_caches,
                    &lock,
                    &guard,
                    &guards,
                    &el,
                    &cv,
                    Pe::After,
                )
                .map(Box::new);
            });
        }
        map.insert(node, cs);
        // On the sized re-pass, publish this element's ComputedValues into Stylo's own data cell:
        // that is where `ContainerCondition::matches` reads an ancestor's `container-type`/`-name`
        // from when a descendant's `@container` rule is evaluated (preorder ⇒ ancestors are
        // published before any descendant asks).
        if cq_active {
            if let Some(mut d) = store.borrow_mut(node) {
                d.styles.primary = Some(cv.clone());
            }
        }
        parent_cv.insert(node, cv);
    }

    // ── **`:has()` — the rules Stylo THREW AWAY.**
    //
    // Stylo's *servo* build hardcodes `parse_has() -> false` (Gecko's returns `true`), so a selector
    // containing `:has()` fails to parse and CSS error-recovery discards the **whole rule**. Its
    // declarations never reach the cascade at all. **13% of the corpus uses `:has()`.**
    //
    // Enabling it upstream costs **vendoring Stylo** — `./stylo` in this repo is a reference checkout
    // that nothing builds; the dependency is `stylo = "0.19"` from crates.io. So this extends the
    // selector engine we already own (the one behind `querySelectorAll`), which is the cheaper rung on
    // the ladder in STATUS.md: *pref → flag delta → **supplement** → module.*
    //
    // Skipped entirely — no walk, no cost — for the ~87% of sheets that contain no `:has()` at all.
    let has_sheets: Vec<&Stylesheet> = sheets.iter().filter(|sh| sh.has_relative_rules()).collect();
    if !has_sheets.is_empty() {
        let mut applied = 0usize;
        let _t_has = ();
        // Lift the `:has()` selectors out ONCE. This used to happen inside the per-element loop —
        // every rule of every `:has()`-carrying sheet re-walked, its `@media` re-evaluated and each
        // selector re-asked whether it was relative, for every element on the page. See
        // `RelativeRule` for the measurement, including which `n` actually drives it.
        let has_index = crate::collect_relative_rules(&has_sheets);
        timed(&mut ph.has_ns, || {
            let nodes: Vec<NodeId> = dom.flat_descendants(dom.root());
            for node in nodes {
                if !dom.is_element(node) {
                    continue;
                }
                let parent_fs = dom
                    .parent(node)
                    .and_then(|p| map.get(&p).map(|s| s.font_size))
                    .unwrap_or(16.0);
                let Some(cs) = map.get_mut(&node) else {
                    continue;
                };
                applied += crate::apply_relative_rules(&has_index, dom, node, cs, parent_fs);
            }
        });
        let _ = _t_has;
        tracing::debug!(
            sheets = has_sheets.len(),
            declarations = applied,
            "applied :has() rules that Stylo discarded"
        );
    }

    // `vertical-align` has no computed longhand accessor in stylo 0.19 (it became a
    // CSS-Inline-3 shorthand of alignment-baseline/baseline-shift/baseline-source, and the
    // legacy line-relative `top`/`bottom` keywords aren't exposed there). Recover *only*
    // that one property from MinimalCascade, which parses it correctly from inline styles
    // and stylesheets alike. Targeted patch — everything else stays Stylo's. Could later be
    // narrowed to a vertical-align-only scan to avoid the second cascade.
    // (`minimal` was computed before the walk — `field-sizing` is recovered in-walk, the rest here.)
    let _recover_guard = ();
    timed(&mut ph.recover_ns, || {
        for (node, cs) in map.iter_mut() {
            if let Some(m) = minimal.get(node) {
                cs.vertical_align = m.vertical_align;
                // `visibility` is not exposed by Stylo's servo build. It is NOT optional: the modern
                // web hides dropdowns/modals/tooltips with `visibility:hidden` (animatable, unlike
                // `display:none`), and without it every one of them paints on top of the page.
                cs.visibility = m.visibility;
                // `mask-image` is likewise not exposed by Stylo's servo build. Without it every icon
                // (an empty span with a background-color shaped by a mask) paints as a black square.
                cs.mask_image = m.mask_image.clone();
                // `background-image` (url + gradients), `text-decoration`, and `list-style` are taken
                // from MinimalCascade for the same reason as `visibility`: Stylo's servo build models
                // them as generic image/keyword types we would have to reimplement to consume. Dropping
                // them was not cosmetic — a gradient hero, an underlined link and a bulleted list are
                // three of the most common things on a web page, and all three rendered as nothing.
                cs.background_images = m.background_images.clone();
                cs.background_size = m.background_size;
                // `background-position` recovered from MinimalCascade so the shipping path places a sprite/
                // logo where the design put it (Stylo's servo build models it as a generic `Position`).
                cs.background_position = m.background_position;
                // `border-style` recovered from MinimalCascade so the shipping path renders dashed/dotted/
                // double borders (drop-zones, dividers, ticket cards) instead of solid.
                cs.border_style = m.border_style;
                // `text-shadow` recovered from MinimalCascade (inherited there) so the shipping path paints
                // the shadow behind hero/heading text — Stylo's servo build models it as a generic list.
                cs.text_shadow = m.text_shadow;
                // `object-fit` recovered from MinimalCascade like the rest of this block, so the shipping
                // Stylo path renders it too: a card grid's `object-fit:cover` thumbnails must not distort.
                cs.object_fit = m.object_fit;
                // `object-position` recovered from MinimalCascade alongside `object-fit` so the shipping
                // path positions a cropped image's subject (Stylo's servo build models it as a
                // `Position` we'd otherwise map by hand).
                cs.object_position = m.object_position;
                // `text-transform` recovered from MinimalCascade (inherited there) so the shipping path
                // renders uppercase nav/buttons — Stylo's servo build models it as a bitflags type.
                cs.text_transform = m.text_transform;
                // `text-overflow` recovered from MinimalCascade so the shipping path truncates clipped
                // single-line titles/labels with `…` (Stylo's servo build models it as a two-value enum).
                cs.text_overflow = m.text_overflow;
                // `-webkit-line-clamp` recovered from MinimalCascade: it is `engine="gecko"` in stylo
                // 0.19, so the servo build never parses it — without this the shipping path shows every
                // line of a clamped card/excerpt instead of N + `…`.
                cs.line_clamp = m.line_clamp;
                // ⚠ **And the DISPLAY that switches that clamp on is dropped by the same build**, which
                // is what made the line above a dead letter on the sites it was written for.
                // `display:-webkit-box` / `-webkit-inline-box` are `#[cfg(feature = "gecko")]` in stylo
                // 0.19's display parser, so the servo build rejects the whole declaration and a clamped
                // `<span>` stays `inline` — the clamp only ever runs on a block, so every card excerpt
                // showed all of its lines. Measured vs live Chromium on a 200px card, `line-height:20px`,
                // `-webkit-line-clamp:2`: Chrome `200×40`, ours `195×57`.
                //
                // The MARKER, not `m.display`, is what is read here: display is a property Stylo
                // resolves correctly, and copying the MinimalCascade's answer wholesale would hand the
                // shipping path the weaker cascade's opinion on every element (the two-cascades trap).
                if let Some(d) = m.legacy_webkit_box {
                    cs.display = d;
                }
                // `overflow-wrap`/`word-wrap` and `word-break` recovered from MinimalCascade so the
                // shipping path also breaks long unbreakable tokens (a URL in a narrow column) instead
                // of letting them overflow. Stylo's servo build models these as keyword enums we don't
                // consume directly.
                cs.overflow_wrap = m.overflow_wrap;
                // `scroll-snap-type`/`scroll-snap-align` recovered from MinimalCascade for the same
                // reason as the properties above: Stylo's servo build models them as typed values we do
                // not consume, and the shipping path needs the axis and the alignment as plain keywords
                // to decide where a scroll lands.
                cs.scroll_snap_type = m.scroll_snap_type;
                cs.scroll_snap_align = m.scroll_snap_align;
                // `scrollbar-width`/`scrollbar-color` recovered from MinimalCascade: both are
                // `engine="gecko"` in stylo 0.19 and never reach the servo build's computed values, so the
                // CSSOM would otherwise report `undefined` for the scrollbar-theming a dark-mode page sets.
                cs.scrollbar_width = m.scrollbar_width;
                cs.scrollbar_color = m.scrollbar_color;
                cs.word_break = m.word_break;
                // `direction` likewise: the bidi base level decides ORDER, and Stylo's servo build
                // does not surface it in a form we consume, so the shipping path would otherwise
                // render every RTL paragraph LTR-ordered.
                cs.direction = m.direction;
                // `letter-spacing`/`word-spacing` recovered from MinimalCascade so the shipping path
                // tracks uppercase nav/buttons/labels too (Stylo's servo build exposes them as a
                // `Spacing<Length>` we'd otherwise map by hand).
                cs.letter_spacing = m.letter_spacing;
                cs.word_spacing = m.word_spacing;
                cs.background_repeat = m.background_repeat;
                // `box-shadow`: stylo_map already fills this from Stylo's own computed value (richer
                // selector matching), so only fall back to MinimalCascade's parse when Stylo left it
                // empty — never overwrite a shadow Stylo already resolved.
                if cs.box_shadows.is_empty() {
                    cs.box_shadows = m.box_shadows.clone();
                }
                cs.text_decoration = m.text_decoration;
                cs.list_style_type = m.list_style_type;
                cs.list_style_inside = m.list_style_inside;
            }
            // Resolve logical `text-align: start`/`end` to physical now that `direction` is final — layout
            // only understands left/center/right/justify. Done here, per node, because direction was just
            // recovered above; in LTR `start`→left (no change), in RTL `start`→right, which is what an
            // unstyled Arabic/Hebrew/Persian paragraph (initial value `start`) must do. Runs even when the
            // node had no MinimalCascade entry, so `Start`/`End` never leak to layout.
            cs.text_align = cs
                .text_align
                .resolve_physical(cs.direction == crate::Direction::Rtl);
        }
    });
    let _ = _recover_guard;

    // CSS `opacity` forms a group: it applies to the whole SUBTREE. Fold each element's own opacity
    // with its ancestors' so every box carries an *effective* opacity and paint needs no ancestor
    // context. Walk the flat tree (shadow content included) in preorder.
    fold_effective_opacity(dom, &mut map);

    // **Shadow trees.** The walk above is over the *node* tree, and a shadow root is deliberately
    // not a child of its host — so shadow content never got a style here. Layout walks the **flat**
    // tree (`flat_children`: shadow content + slot assignment), so those nodes MUST have styles or
    // it panics on the lookup. `MinimalCascade` already implements the N4 flat-tree cascade with
    // tree-scoped matching (a shadow root's own `<style>` applies only inside it), so adopt its
    // result for every node Stylo's walk missed. Document nodes keep Stylo's (richer) cascade;
    // only shadow content falls back. Giving Stylo a scoped flat-tree walk is the follow-on.
    for (node, m) in minimal.iter() {
        map.entry(*node).or_insert_with(|| m.clone());
    }

    if profiling() {
        let el_count = map.len();
        let ms = |ns: u128| ns as f64 / 1.0e6;
        #[cfg(not(target_arch = "wasm32"))]
        let total_ns = t_all.elapsed().as_nanos();
        #[cfg(target_arch = "wasm32")]
        let total_ns = 0u128;
        // Whatever the named phases do not account for is reported as its own line rather than
        // spread across them. An instrument whose parts silently sum to the whole is one that
        // cannot tell you it is missing something.
        let named = ph.flush_ns
            + ph.minimal_ns
            + ph.element_ns
            + ph.pseudo_ns
            + ph.has_ns
            + ph.computed_ns
            + ph.hints_ns
            + ph.recover_ns;
        tracing::warn!(
            nodes = el_count,
            total_ms = ms(total_ns),
            flush_ms = ms(ph.flush_ns),
            computed_ms = ms(ph.computed_ns),
            hints_ms = ms(ph.hints_ns),
            recover_ms = ms(ph.recover_ns),
            minimal_ms = ms(ph.minimal_ns),
            element_ms = ms(ph.element_ns),
            pseudo_ms = ms(ph.pseudo_ns),
            has_ms = ms(ph.has_ns),
            unattributed_ms = ms(total_ns.saturating_sub(named)),
            "CASCADE PHASES"
        );
    }

    map
}

/// Multiply each element's own `opacity` by its ancestors' (CSS opacity applies to the subtree).
fn fold_effective_opacity(dom: &Dom, map: &mut StyleMap) {
    fn walk(dom: &Dom, node: NodeId, parent: f32, map: &mut StyleMap) {
        let eff = match map.get_mut(&node) {
            Some(cs) => {
                cs.opacity = (cs.opacity * parent).clamp(0.0, 1.0);
                cs.opacity
            }
            None => parent,
        };
        for k in dom.flat_children(node) {
            walk(dom, k, eff, map);
        }
    }
    walk(dom, dom.root(), 1.0, map);
}

/// Apply HTML presentational hints that Stylo's cascade doesn't see (our `TElement` wall
/// doesn't synthesize them): replaced-element `width`/`height` attributes and the legacy
/// colour/size attributes. Applied only where the property is still at its initial, so real author
/// CSS wins (presentational hints are lower priority than author rules).
///
/// ⚠⚠⚠ **A cell's 1px padding is NOT here, and must never come back.** It used to be, guarded on
/// `padding == 0` — and *nothing* can be guarded that way, because 0 **IS** the initial value of
/// `padding`. So `td { padding: 0 }` (which is Tailwind's preflight, Normalize, and every
/// hand-rolled `* { padding: 0 }` reset since 2004) computed to 0 through the cascade exactly as the
/// author wrote it, and this function then put the UA 1px straight back — silently reinstating, for
/// table cells only, the very bug the t556 origin fix had removed for every other element. Each
/// cell came out 2px too wide and 2px too tall, so every row was 2px tall and every row below it
/// 2px lower: measured against live Chromium, a reset cell read `43×20` in Chrome and `45×22` here.
/// The default now comes from the ONE place that can express it without guessing — the UA-origin
/// sheet (`td, th { padding: 1px }` in `UA_CSS`), where an author reset legitimately outranks it.
fn apply_presentational_hints(dom: &Dom, node: NodeId, s: &mut crate::ComputedStyle) {
    let Some(el) = dom.element(node) else {
        return;
    };
    let tag = dom.tag_name(node).unwrap_or("");
    // Legacy presentational colour attributes — still load-bearing (Hacker News's whole identity
    // is `bgcolor` on <table>/<td>). Applied only where author CSS left the property initial.
    if s.background_color.is_none() {
        if let Some(c) = el.attr("bgcolor").and_then(crate::values::parse_color) {
            s.background_color = Some(c);
        }
    }
    if let Some(c) = el.attr("text").and_then(crate::values::parse_color) {
        s.color = c;
    }
    // **Presentational sizing.** `width`/`height` attributes are not decoration; on `<table>`,
    // `<td>` and `<img>` they are the layout. Hacker News is `<table width="85%">` — ignore it and
    // the table shrink-to-fits to its text instead of spanning the page.
    if matches!(
        tag,
        "table" | "td" | "th" | "col" | "colgroup" | "iframe" | "hr" | "pre"
    ) {
        if s.width == crate::Dim::Auto {
            if let Some(w) = el.attr("width").and_then(crate::parse_dimension_attr_dim) {
                s.width = w;
            }
        }
        if s.height == crate::Dim::Auto {
            if let Some(h) = el.attr("height").and_then(crate::parse_dimension_attr_dim) {
                s.height = h;
            }
        }
    }
    // `<table cellspacing>` / `<table cellpadding>` — the separated-borders model's two knobs.
    if tag == "table" {
        if let Some(sp) = el.attr("cellspacing").and_then(crate::parse_dimension_attr) {
            s.border_spacing = sp;
        }
        // `align="center"` centres the table; `<center>` does the same thing to its table child
        // (Chrome implements it as `text-align: -webkit-center`, which centres block children too).
        let centered = el
            .attr("align")
            .is_some_and(|a| a.eq_ignore_ascii_case("center"))
            || dom
                .parent(node)
                .and_then(|p| dom.tag_name(p))
                .is_some_and(|t| t == "center");
        if centered && s.margin.left == crate::Dim::Px(0.0) && s.margin.right == crate::Dim::Px(0.0)
        {
            s.margin.left = crate::Dim::Auto;
            s.margin.right = crate::Dim::Auto;
        }
    }
    // `cellpadding` lives on the table but pads the CELLS.
    if matches!(tag, "td" | "th") {
        let table_cellpadding = {
            let mut cur = dom.parent(node);
            let mut found = None;
            while let Some(p) = cur {
                if dom.tag_name(p) == Some("table") {
                    found = dom
                        .element(p)
                        .and_then(|e| e.attr("cellpadding"))
                        .and_then(crate::parse_dimension_attr);
                    break;
                }
                cur = dom.parent(p);
            }
            found
        };
        if let Some(cp) = table_cellpadding {
            s.padding = crate::Sides::all(crate::Dim::Px(cp));
        }
    }
    // A form control has an INTRINSIC size — the browser's, not the content's. A text field is
    // `size` characters wide (20 by default), and a checkbox is a 13px square. Sized from their
    // content instead, a text field collapses to the width of its value ("hi" → 12px) and a
    // checkbox, having no content at all, disappears entirely.
    if tag == "input" {
        let ty = el.attr("type").unwrap_or("text").to_ascii_lowercase();
        match ty.as_str() {
            "checkbox" | "radio" => {
                if s.width == crate::Dim::Auto && !s.width_stretch {
                    s.width = crate::Dim::Px(13.0);
                }
                if s.height == crate::Dim::Auto {
                    s.height = crate::Dim::Px(13.0);
                }
            }
            "hidden" | "submit" | "reset" | "button" | "image" | "file" | "range" | "color" => {}
            // Text-like: `size` characters wide, PLUS a constant — and the constant is not a fudge
            // factor, it is most of the box on a short field. Measured against headless Chrome on
            // `/tmp/ctl.html` (`font: 16px sans-serif` on the body, so the control font is the UA's):
            //
            //   size= 1   Chrome  53px border box        size=20   Chrome 205px
            //   size= 5   Chrome  85px                   size=40   Chrome 365px
            //
            // The slope is exactly 8.0px/char and the intercept is 45px border box — 39px of content
            // once this UA sheet's `padding:1px 2px` + `1px` border are removed. Blink derives that
            // intercept from the face (`maxCharWidth - avgCharWidth`, plus room for the caret); we
            // take the number it arrives at.
            //
            // ⚠ **The comment this replaces asserted `size=20 → ~173px` was "the same approximation
            // Chrome's own default ends up at". Chrome ends up at 205.** Nobody had put the two side
            // by side, so every default-width text field on the web was 26px too narrow here — and a
            // text field's width is a container's width one level up.
            //
            // Both terms scale with the control's own `font-size`, so an author who sets one gets a
            // proportional box rather than a box calibrated for a font it is not using.
            _ => {
                // A UA intrinsic width is a DEFAULT, so an author declaration outranks it — and
                // `width: stretch` is a declaration that merely *looks* absent (`Dim::Auto`). Same
                // guard as the dimension attributes below: without it a `width:stretch` text field
                // stays 173px wide instead of filling its form row.
                // `field-sizing: content` (Baseline June 2026) stands the UA intrinsic width
                // down entirely: the control sizes from its content, like any other box.
                if s.width == crate::Dim::Auto && !s.width_stretch && !s.field_sizing_content {
                    let cols = el
                        .attr("size")
                        .and_then(|v| v.trim().parse::<f32>().ok())
                        .filter(|n| *n > 0.0)
                        .unwrap_or(20.0);
                    s.width = crate::Dim::Px(s.font_size * (cols * 0.6 + 2.925));
                }
            }
        }
    }
    if tag == "textarea" && !s.field_sizing_content {
        // `cols` — same 0.6em/char slope as `<input>`, but a DIFFERENT intercept: Chrome gives a
        // default `<textarea>` 182px border box and a `cols="10"` one 102px, i.e. 8.0px/char with a
        // 22px border-box intercept (16px of content) against the text field's 45. A textarea has no
        // caret-scroll allowance to reserve; a single shared constant would be wrong for one of them.
        if s.width == crate::Dim::Auto && !s.width_stretch {
            let cols = el
                .attr("cols")
                .and_then(|v| v.trim().parse::<f32>().ok())
                .filter(|n| *n > 0.0)
                .unwrap_or(20.0);
            s.width = crate::Dim::Px(s.font_size * (cols * 0.6 + 1.2));
        }
        // ── **`rows` WAS NOT READ AT ALL, SO EVERY `<textarea>` ON THE WEB WAS ONE LINE TALL.**
        //
        // The width half of this control has been honoured since it was written; the height half was
        // never implemented, so an empty textarea sized to its (empty) content and came out 22px
        // against Chrome's 36. Measured: Chrome `rows=1` → 21px border box, no `rows` → 36, `rows=2`
        // → 36, `rows=3` → 51. That is `rows × line-height` of content, with the default **2** the
        // HTML spec names — a slope of 15px/row at the UA control font, and 15/13.333 is `1.125`.
        //
        // Every comment box, contact form and review field on the web is affected, and the error is
        // not confined to the control: a box one line short pulls everything below it up the page,
        // which is the `dy` term the render burndown ranks first.
        //
        // ⚠ `line-height: normal` cannot be resolved here — it is the FACE's ascent+descent+lineGap
        // and the metrics live in the text layer, which this function has no handle on. The 1.125
        // factor is Chrome's own ratio at the control font, and it sizes the BOX only: the lines
        // drawn inside it still come from real font metrics. An authored `line-height` is used
        // directly, because then there is nothing to approximate.
        if s.height == crate::Dim::Auto && !s.height_stretch {
            let rows = el
                .attr("rows")
                .and_then(|v| v.trim().parse::<f32>().ok())
                .filter(|n| *n > 0.0)
                .unwrap_or(2.0);
            let lh = if s.line_height_normal {
                s.font_size * 1.125
            } else {
                s.line_height
            };
            // The `+ 2` is measured, not derived: Chrome's inner editor sits 1px clear of the top
            // and bottom of the content box (`rows=1` → 21px border box, of which this sheet's
            // padding+border account for 4 and one 15px line for 15). Written here rather than as UA
            // padding on purpose — `getComputedStyle(el).padding` must keep reporting `1px 2px`,
            // which is what the page can observe and what Chrome answers.
            s.height = crate::Dim::Px(rows * lh + 2.0);
        }
    }
    if matches!(
        tag,
        "img" | "canvas" | "video" | "svg" | "object" | "embed" | "iframe"
    ) {
        // Computed display stays `inline` — the spec's and Chrome's value for a replaced element
        // (the tick-380 oracle: 81 sites diverged on `<img>`, 80 on `<svg>`, because this used to
        // force `inline-block`). Layout lays an inline replaced box out ATOMICALLY — sized as a
        // block, flowed like a word — which is what the old mutation was standing in for.
        // A presentational hint is the LOWEST-priority source, so it may only fill a genuinely
        // absent width. `width: stretch` and the intrinsic keywords both compute to `Dim::Auto`,
        // which made them look absent — so `<canvas width="40">` beat the author's `width: stretch`
        // and the element kept hugging its 40px instead of filling its column. The flags are what
        // tell "no width was specified" apart from "a width was specified that resolves later".
        if s.width == crate::Dim::Auto && !s.width_stretch && s.width_keyword.is_none() {
            if let Some(w) = el.attr("width").and_then(crate::parse_dimension_attr_dim) {
                s.width = w;
            }
        }
        if s.height == crate::Dim::Auto && !s.height_stretch && !s.height_intrinsic {
            if let Some(h) = el.attr("height").and_then(crate::parse_dimension_attr_dim) {
                s.height = h;
            }
        }
        // The dimension attributes are also an aspect-ratio hint (HTML §"dimension attributes":
        // `aspect-ratio: auto <width> / <height>`). Twin of the block in `apply_ua_defaults` —
        // see there for why the ratio, not the lengths, is the load-bearing half.
        if s.aspect_ratio.is_none() && !matches!(tag, "iframe" | "embed" | "object") {
            if let (Some(crate::Dim::Px(w)), Some(crate::Dim::Px(h))) = (
                el.attr("width").and_then(crate::parse_dimension_attr_dim),
                el.attr("height").and_then(crate::parse_dimension_attr_dim),
            ) {
                if w > 0.0 && h > 0.0 {
                    s.aspect_ratio = Some(w / h);
                }
            }
        }
        // `viewBox` gives an `<svg>` an intrinsic RATIO (SVG2) even with no dimension attributes
        // at all — the icon/logo idiom. Measured Chrome (tick 391): `<svg viewBox="0 0 24 24">`
        // in a 400px block is 400×400 — auto width fills the containing block, height follows the
        // ratio. Layout consumes the ratio; this hint only surfaces it.
        if tag == "svg" && s.aspect_ratio.is_none() {
            if let Some(vb) = el.attr("viewBox").or_else(|| el.attr("viewbox")) {
                let n: Vec<f32> = vb
                    .split(|c: char| c == ',' || c.is_ascii_whitespace())
                    .filter(|t| !t.is_empty())
                    .filter_map(|t| t.parse().ok())
                    .collect();
                if n.len() == 4 && n[2] > 0.0 && n[3] > 0.0 {
                    s.aspect_ratio = Some(n[2] / n[3]);
                }
            }
        }
        // **An unsized `<iframe>` is 300x150.** That is the spec's default, and it is not arbitrary
        // trivia: an iframe has no intrinsic size to fall back on, so with no default it collapses to
        // nothing and the embed is invisible *before* any question of content arises. `iframe` was not
        // in this list at all, which is why it laid out at ZERO WIDTH — 23% of sites, and the box was
        // gone before we ever got as far as failing to fetch its document.
        if tag == "iframe" {
            if s.width == crate::Dim::Auto {
                s.width = crate::Dim::Px(300.0);
            }
            if s.height == crate::Dim::Auto {
                s.height = crate::Dim::Px(150.0);
            }
        }
    }
}

/// Match `rules` against `el`, appending each winning `(specificity, order, block)` to
/// `winners`. Descends into `@media` blocks whose query [evaluates](MediaList::evaluate) true
/// against `device` (built from the real viewport in [`make_device`]) — this is what makes
/// responsive `@media (max-width: …)` rules apply. Nested `@media` recurse; other at-rules
/// (`@supports`, `@layer`, …) are skipped for now (their inner rules are not applied), matching
/// the prior flat behavior except that media rules now work.
#[allow(clippy::type_complexity)]

/// Part 22.3: full-document cascades per navigation. Counted, not assumed.
pub static CASCADES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Per-phase wall time inside one cascade, in nanoseconds. **Opt-in** via
/// `MANUK_CASCADE_PROFILE=1`, and off it costs one relaxed atomic read per phase and no clock
/// reads at all.
///
/// It exists because there was no way to answer "where did the 165 seconds go" from inside the
/// engine. `RULEINDEX` timed the index build and nothing timed the two things that turned out to
/// dominate. A cascade that can only be profiled from outside the process is one whose cost gets
/// attributed by guess — and the guesses were wrong twice before this was added.
#[derive(Default)]
struct Phases {
    flush_ns: u128,
    /// `to_computed_style` — the Stylo `ComputedValues` -> our `ComputedStyle` conversion, once per
    /// element. Split out from `element_ns` because it is a different organ: `element_ns` is
    /// *matching*, this is *materialising*, and the two have completely different fixes.
    computed_ns: u128,
    hints_ns: u128,
    recover_ns: u128,
    minimal_ns: u128,
    element_ns: u128,
    pseudo_ns: u128,
    has_ns: u128,
}

/// Whether phase profiling is on. Read from the environment ONCE.
fn profiling() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("MANUK_CASCADE_PROFILE").is_ok_and(|v| v != "0"))
}

/// Run `f`, adding its duration to `slot` when profiling is on.
///
/// ⚠ `Instant::now()` PANICS on `wasm32-unknown-unknown` — there is no clock there. One
/// debug-only timing line once took down the entire cascade in the browser demo, surfacing as
/// `RuntimeError: unreachable` from inside the wasm module. **A measurement must never be able to
/// break the thing it measures**, so the clock is behind both the target guard and the flag.
#[inline]
fn timed<T>(slot: &mut u128, f: impl FnOnce() -> T) -> T {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if profiling() {
            let t = std::time::Instant::now();
            let r = f();
            *slot += t.elapsed().as_nanos();
            return r;
        }
    }
    let _ = &mut *slot;
    f()
}

/// **A rule index — so an element only tests rules it could possibly match.**
///
/// `cascade_one_element` used to walk **every rule in every stylesheet, for every element**. We built
/// a full `Stylist` — with its bucketed `SelectorMap`, its rule hashes and its ancestor Bloom filter —
/// and then never used it for matching, borrowing only its `Device`. The cascade was therefore
/// O(elements × rules): on Wikipedia, 18,631 elements against thousands of rules, **339ms**, which is
/// ~18µs per element and about twenty times what it should cost.
///
/// (It also explains why implementing `TElement::each_class` changed nothing: the fast path it feeds
/// was never entered.)
///
/// This is the same trick `SelectorMap` plays, and the same one `MinimalCascade::build_index`
/// already played on the other cascade: file each rule under the **rightmost** simple selector that
/// must match — its id, else a class, else a tag — and at match time only look in the buckets that
/// this element could be in, plus the universal one. A `.reference` rule is never tested against a
/// `<div>` that has no classes.
///
/// Correctness is unchanged because the *matching* is unchanged: every candidate still goes through
/// `matches_selector`, and winners are still ordered by `(specificity, source order)`. The index only
/// removes candidates that could not have matched.
/// The author origin's rank in the cascade sort — see [`origin_rank`].
const ORIGIN_AUTHOR: u8 = 2;

/// **The cascade's first sort criterion, as one number.**
///
/// CSS Cascade §6 orders declarations by *origin and importance* first and by *specificity* only
/// third. Our matcher merges the winners itself (`cascade_element`), so it needs the origin as a
/// sortable key — higher wins. `User` sits between the two because a user stylesheet outranks the
/// UA sheet and loses to the author's; we ship none today, but ranking it correctly costs nothing
/// and stops the next reader having to re-derive it. (Importance is resolved *inside* the merged
/// block by Stylo's `PropertyDeclarationBlock::push`, not here.)
fn origin_rank(origin: Origin) -> u8 {
    match origin {
        Origin::UserAgent => 0,
        Origin::User => 1,
        Origin::Author => ORIGIN_AUTHOR,
    }
}

struct RuleIndex {
    rules: Vec<IndexedRule>,
    by_id: std::collections::HashMap<String, Vec<u32>>,
    by_class: std::collections::HashMap<String, Vec<u32>>,
    by_tag: std::collections::HashMap<String, Vec<u32>>,
    universal: Vec<u32>,
    /// **CASCADE LAYERS — the sort criterion between origin and specificity.** Named layers in the
    /// order they were first declared, so `@layer reset, theme;` fixes the order before either
    /// block appears (which is the whole point of the statement form). A layer's rank is its index
    /// here; anonymous `@layer { … }` blocks take a fresh rank each and are never looked up.
    layer_names: Vec<String>,
    /// The layer the walk is currently inside — `UNLAYERED` at the top level. Carried as state
    /// rather than as a ninth `add_rules` parameter: the walk is single-threaded and every
    /// recursion site would otherwise have to remember to thread it, which is exactly how the
    /// `origin_rank` argument got dropped on one path once already.
    cur_layer: u16,
    /// The next rank an anonymous layer takes. Anonymous layers are ordered among themselves and
    /// against named ones by declaration order, so they share the same counter space.
    next_layer: u16,
}

/// **Unlayered author declarations BEAT layered ones, regardless of document order** (CSS Cascade 5
/// §6.4.4), which is the entire reason an author moves a component's styles into a layer: the layer
/// exists to LOSE to the page's own rules. Measured at t787 (audit #50) with layers flattened into
/// document order: `#h { width:100px }` then `@layer L { #h { width:333px } }` read Chrome **100**
/// and ours **333**.
///
/// So unlayered takes the TOP rank and layers count up from zero in declaration order (a later
/// layer beats an earlier one). Sorting ascending and merging in that order means the last block
/// merged wins, which is the convention the rest of this sort already uses.
const UNLAYERED: u16 = u16::MAX;

/// A layer's name as written, for identity only — `@layer a` reopened later is the SAME layer, and
/// two blocks must not take two ranks. Serialised through Stylo's own `ToCss` so a dotted sublayer
/// name (`@layer framework.base`) keeps the form the author wrote rather than one we invent.
fn layer_name_string(n: &stylo::stylesheets::layer_rule::LayerName) -> String {
    use style_traits::ToCss;
    n.to_css_string()
}

struct IndexedRule {
    sel: selectors::parser::Selector<stylo::selector_parser::SelectorImpl>,
    /// **The cascade's FIRST sort, and this index did not have it.** 0 = user-agent, 1 = author.
    /// Specificity is only the *third* criterion (origin, then importance, then specificity, then
    /// document order) — so without this a UA `body { margin: 8px }` outranked an author
    /// `* { margin: 0 }`, which is the first rule of every CSS reset on the web. See the sort in
    /// `cascade_element`/`cascade_pseudo` and the note on the UA sheet's `Origin` at its parse site.
    origin_rank: u8,
    /// The rule's cascade LAYER rank — `UNLAYERED` (the maximum) when it is not in a layer at all,
    /// which is the case for the overwhelming majority of rules and is also the WINNING value.
    layer_rank: u16,
    spec: u32,
    order: usize,
    block: ServoArc<stylo::shared_lock::Locked<PropertyDeclarationBlock>>,
    /// The `@container` condition levels this rule is nested under, outermost first — empty for
    /// the vast majority. Unlike `@media` (device-scoped, resolved once at index build), a
    /// container condition is **per-element**: it must be evaluated at match time against the
    /// matching element's nearest ancestor container. Nesting levels AND together; the comma
    /// list inside one level ORs (Stylo's own `container_condition_matches` semantics).
    cq: Vec<ServoArc<Vec<ContainerCondition>>>,
}

impl RuleIndex {
    fn build(
        sheets: &[ServoArc<StyloStylesheet>],
        guard: &SharedRwLockReadGuard<'_>,
        device: &Device,
        qm: QuirksMode,
    ) -> Self {
        let mut idx = RuleIndex {
            rules: Vec::new(),
            by_id: Default::default(),
            by_class: Default::default(),
            by_tag: Default::default(),
            universal: Vec::new(),
            layer_names: Vec::new(),
            cur_layer: UNLAYERED,
            next_layer: 0,
        };
        let mut order = 0usize;
        let mut cq_stack: Vec<ServoArc<Vec<ContainerCondition>>> = Vec::new();
        for sheet in sheets {
            let rank = origin_rank(sheet.contents.read_with(guard).origin);
            let rules = sheet.contents.read_with(guard).rules(guard);
            idx.add_rules(
                rules,
                guard,
                device,
                &mut order,
                qm,
                &mut cq_stack,
                rank,
                None,
            );
        }
        idx
    }

    /// The rank of a NAMED layer, allocating it on first sight. Order of first declaration is the
    /// layer's order — whether that first sight is a `@layer a, b;` statement or the block itself.
    fn layer_rank_for(&mut self, name: &str) -> u16 {
        if let Some(i) = self.layer_names.iter().position(|n| n == name) {
            return i as u16;
        }
        let r = self.next_layer;
        self.next_layer = self.next_layer.saturating_add(1);
        self.layer_names.push(name.to_string());
        // `layer_names` is indexed BY RANK, so a name allocated after an anonymous block has to sit
        // at its own rank's index — pad rather than push out of alignment.
        while self.layer_names.len() <= r as usize {
            self.layer_names.push(String::new());
        }
        self.layer_names[r as usize] = name.to_string();
        r
    }

    /// Index one declaration block against one already-`&`-resolved selector list.
    ///
    /// Split out of `add_rules` because **two different `CssRule` variants carry the same payload**:
    /// a `Style` rule (selectors + block) and a `NestedDeclarations` rule (block only, borrowing its
    /// enclosing rule's selectors). Sharing the body is what keeps the two from drifting — the index
    /// KEY derivation, the specificity, the `@container` stack and the document-order counter all
    /// have to be identical or a nested declaration would cascade differently from the sibling
    /// declaration written one line above it.
    fn index_block(
        &mut self,
        selectors: &selectors::SelectorList<stylo::selector_parser::SelectorImpl>,
        block: &ServoArc<stylo::shared_lock::Locked<PropertyDeclarationBlock>>,
        order: &mut usize,
        qm: QuirksMode,
        cq_stack: &[ServoArc<Vec<ContainerCondition>>],
        origin_rank: u8,
    ) {
        use selectors::parser::Component;
        for sel in selectors.slice() {
            // The rightmost compound is the one that must match THIS element; anything to its left
            // is an ancestor/sibling constraint checked afterwards.
            let mut key: Option<(u8, String)> = None;
            for comp in sel.iter() {
                let cand = match comp {
                    Component::ID(v) => Some((0u8, index_key(&v.to_string(), qm))),
                    Component::Class(v) => Some((1u8, index_key(&v.to_string(), qm))),
                    Component::LocalName(n) => Some((2u8, n.lower_name.to_string())),
                    _ => None,
                };
                // Prefer the most selective key available: id > class > tag.
                if let Some(c) = cand {
                    if key.as_ref().map(|k| c.0 < k.0).unwrap_or(true) {
                        key = Some(c);
                    }
                }
            }
            let i = self.rules.len() as u32;
            self.rules.push(IndexedRule {
                sel: sel.clone(),
                origin_rank,
                layer_rank: self.cur_layer,
                spec: sel.specificity(),
                order: *order,
                block: block.clone(),
                cq: cq_stack.to_vec(),
            });
            match key {
                Some((0, v)) => self.by_id.entry(v).or_default().push(i),
                Some((1, v)) => self.by_class.entry(v).or_default().push(i),
                Some((2, v)) => self.by_tag.entry(v).or_default().push(i),
                // `*`, `:hover`, `[attr]` and friends have no cheap key: they must be tried against
                // everything, which is correct and is what `SelectorMap` does too.
                _ => self.universal.push(i),
            }
            *order += 1;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn add_rules(
        &mut self,
        rules: &[CssRule],
        guard: &SharedRwLockReadGuard<'_>,
        device: &Device,
        order: &mut usize,
        qm: QuirksMode,
        cq_stack: &mut Vec<ServoArc<Vec<ContainerCondition>>>,
        origin_rank: u8,
        // The ENCLOSING style rule's selector list, with `&` in it already substituted — `None` at
        // the top level. Threaded so nested `&` can be resolved; see the substitution below.
        parent: Option<&selectors::SelectorList<stylo::selector_parser::SelectorImpl>>,
    ) {
        for rule in rules {
            match rule {
                CssRule::Style(style_rule) => {
                    let sr = style_rule.read_with(guard);
                    // ── **CSS NESTING: `&` MUST BE SUBSTITUTED, OR IT SILENTLY MEANS `<html>`.**
                    //
                    // t659 taught this walk to RECURSE into `sr.rules`, so nested rules are indexed —
                    // but it indexed their selectors VERBATIM, and a verbatim `&` is
                    // `Component::ParentSelector`, which the matcher resolves as:
                    //
                    //     Component::ParentSelector => match context.shared.scope_element {
                    //         Some(ref scope_element) => element.opaque() == *scope_element,
                    //         None => element.is_root(),          // <-- ours: no scope is ever set
                    //     }
                    //
                    // So every `&` in every stylesheet matched **the root element**. Measured against
                    // live Chromium, with `body{margin:0}` so the numbers are comparable:
                    //
                    //   `#a { width:50px; & { width:300px } }`          Chrome 300   ours **50**
                    //   `#h { width:40px; &:not(.x){ width:260px } }`   Chrome 260   ours **40**
                    //   `.p { & > span { width:240px } }`               Chrome 240   ours **73**
                    //   `#other { & .leak { width:500px } }`            Chrome 500   ours **100**
                    //
                    // ⚠ The failure is not a clean "dropped", which is why it survived: a DESCENDANT
                    // form like `& .child` matches *by accident*, because `<html>` really is an
                    // ancestor of everything — so it applies to `.child` **anywhere in the document**,
                    // while carrying the wrong SPECIFICITY (`&` should contribute the parent
                    // selector's, an unresolved one contributes nothing), so it then loses cascade
                    // fights it should win: `#other { & .leak }` reads 100 instead of 500 for exactly
                    // that reason. Over-matching, under-specified, and right often enough to look fine.
                    //
                    // The fix is the one Stylo's own `stylist` uses at rule-collection time
                    // (`replace_parent_selector`): substitute the enclosing rule's selectors into `&`
                    // BEFORE indexing, which corrects matching, specificity and the index KEY together
                    // — a substituted `& .leak` keys on `.leak` with `#other` as a real ancestor
                    // constraint. Implicit nesting (`.child {}` with no `&`) already parses as
                    // `& .child`, so the same call covers it.
                    //
                    // Population is not a guess: the comment on the recursion below records that
                    // **41% of the corpus uses CSS nesting** in its inline `<style>` blocks alone.
                    let resolved: selectors::SelectorList<stylo::selector_parser::SelectorImpl> =
                        match parent {
                            Some(p) => sr.selectors.replace_parent_selector(p),
                            None => sr.selectors.clone(),
                        };
                    self.index_block(&resolved, &sr.block, order, qm, cq_stack, origin_rank);

                    // **CSS NESTING — and this walk was silently dropping all of it.**
                    //
                    // A `StyleRule` carries `rules: Option<Arc<Locked<CssRules>>>` — its *nested* rules.
                    // Stylo parses them correctly and has done all along. This index — added as a cascade
                    // optimisation — read `selectors` and `block` and **never looked at `rules`**, so
                    // every nested rule in every stylesheet was thrown away before it could ever match.
                    //
                    // Measured: **41% of the corpus uses CSS nesting** inside its inline `<style>` blocks
                    // alone (external sheets are not even scanned, so that is a FLOOR). It is the single
                    // largest cause of the "we lose flex/grid on this node" divergence, and of the
                    // "we show what Chrome hides" one — a nested `display: none` never applied either.
                    //
                    // The lesson is the one this project keeps re-learning from the other side: an
                    // optimisation that makes a data structure *smaller* must be asked what it dropped.
                    // This one was measured for speed (cascade 339ms → 199ms) and never once asked
                    // whether the rules it indexed were all the rules there were.
                    if let Some(nested) = &sr.rules {
                        let nested = nested.read_with(guard);
                        // The RESOLVED list, not `sr.selectors`: nesting composes, so `&` inside a
                        // doubly-nested rule must resolve against its parent's already-substituted
                        // selector rather than against another unresolved `&`.
                        self.add_rules(
                            &nested.0,
                            guard,
                            device,
                            order,
                            qm,
                            cq_stack,
                            origin_rank,
                            Some(&resolved),
                        );
                    }
                }
                // ── **A NESTED `@media` LOST ITS DECLARATIONS, AND ONLY ITS DECLARATIONS** (t785).
                //
                // t659 taught this walk to recurse into `sr.rules`, so a nested STYLE rule is
                // indexed. But declarations written *directly* inside a nested group rule are not a
                // style rule at all — CSS Nesting wraps them in an implicit `& { … }`, and Stylo
                // materialises that as its own variant, `CssRule::NestedDeclarations`, carrying a
                // block and NO selectors. It fell into the `_ => {}` arm below and was dropped in
                // silence, which is the shape this project keeps getting caught by: the rule that
                // *has* a selector survives, the one that borrows its parent's does not.
                //
                //     article {
                //       max-width: 423px;                                  <- indexed
                //       @media (min-width: 1018px) { max-width: 974px; }   <- DROPPED, every time
                //     }
                //
                // Measured on `secure5.entertimeonline.com` (a board §8 near-bar site), viewport
                // 1200: Chrome lays the `<article>` out at **487px**, we gave it **1134px** — the
                // page's whole content column, and every descendant with it (the oracle's #1 cause
                // there is `displaced: x ~256px`). A width error is the burndown's ranked #1
                // mechanism precisely because it does not stay a width error: a container a few
                // hundred px too wide re-wraps its prose, and the wrong line count cascades down the
                // rest of the page as `dy`.
                //
                // The enclosing selectors arrive as `parent` — already `&`-substituted by the Style
                // arm above — so this is the same index call with the block that came in here.
                // A `NestedDeclarations` with no parent cannot be produced by the grammar (there is
                // nothing for the implicit `&` to mean at the top level); it is skipped rather than
                // guessed at, because inventing a selector is how a dropped rule becomes a WRONG one.
                CssRule::NestedDeclarations(ndr) => {
                    if let Some(parent) = parent {
                        let ndr = ndr.read_with(guard);
                        self.index_block(parent, &ndr.block, order, qm, cq_stack, origin_rank);
                    }
                }
                CssRule::Media(media_rule) => {
                    let ml = media_rule.media_queries.read_with(guard);
                    let mut custom = CustomMediaEvaluator::none();
                    if ml.evaluate(device, qm, &mut custom) {
                        let nested = media_rule.rules.read_with(guard);
                        self.add_rules(
                            &nested.0,
                            guard,
                            device,
                            order,
                            qm,
                            cq_stack,
                            origin_rank,
                            parent,
                        );
                    }
                }
                CssRule::Supports(supports_rule) => {
                    // **The CASCADE must take the same answer `CSS.supports()` gives**, or a page
                    // gets a different browser depending on which one it asked (the tick-282 bug,
                    // one level down). Stylo's `enabled` says "it parses"; `honest_supports` says
                    // "we render it", and returns `None` — costing nothing — for the conditions
                    // where those are the same question, which is nearly all of them.
                    if supports_rule.enabled
                        && honest_supports(&supports_rule.condition).unwrap_or(true)
                    {
                        let nested = supports_rule.rules.read_with(guard);
                        self.add_rules(
                            &nested.0,
                            guard,
                            device,
                            order,
                            qm,
                            cq_stack,
                            origin_rank,
                            parent,
                        );
                    }
                }
                CssRule::LayerBlock(layer) => {
                    // A named layer keeps ONE rank across every block that reopens it — `@layer a`
                    // written twice is one layer, not two — so the name is looked up rather than
                    // counted. An anonymous block is its own layer by definition and can never be
                    // reopened, so it takes a fresh rank and is not recorded under any name.
                    let outer = self.cur_layer;
                    self.cur_layer = match &layer.name {
                        Some(n) => self.layer_rank_for(&layer_name_string(n)),
                        None => {
                            let r = self.next_layer;
                            self.next_layer = self.next_layer.saturating_add(1);
                            r
                        }
                    };
                    let nested = layer.rules.read_with(guard);
                    self.add_rules(
                        &nested.0,
                        guard,
                        device,
                        order,
                        qm,
                        cq_stack,
                        origin_rank,
                        parent,
                    );
                    self.cur_layer = outer;
                }
                // `@layer reset, theme;` — the STATEMENT form, which declares the order before
                // either block exists. Ignoring it is not a small loss: the statement is the
                // idiomatic way to fix layer order at the top of a sheet precisely so the blocks
                // can then appear in any order, so a walk that ranked layers by first BLOCK would
                // get the common case backwards.
                CssRule::LayerStatement(stmt) => {
                    for name in stmt.names.iter() {
                        self.layer_rank_for(&layer_name_string(name));
                    }
                }
                // `CssRule::Container` never appears here: stylo's servo build parses the
                // `@container` at-rule only under `cfg!(feature = "gecko")` (rule_parser.rs), so
                // the whole block is dropped as an unknown at-rule. The supplement that recovers
                // them is `add_container_supplement` — it lifts the blocks from the sheet SOURCE,
                // parses conditions with Stylo's own public parser, and calls back into
                // `add_rules` with the condition stack.
                _ => {}
            }
        }
    }

    /// Candidate rules for one element: the universal bucket plus the buckets this element's own
    /// tag, id and classes can be in. Every candidate is still fully matched afterwards.
    fn candidates(&self, dom: &Dom, node: NodeId, out: &mut Vec<u32>) {
        out.clear();
        out.extend_from_slice(&self.universal);
        if let Some(tag) = dom.tag_name(node) {
            if let Some(v) = self.by_tag.get(tag) {
                out.extend_from_slice(v);
            }
        }
        if let Some(e) = dom.element(node) {
            // Same key shape as `add_rules` used when bucketing — see `index_key`.
            let qm = qm_of(dom);
            if let Some(id) = e.attr("id") {
                if let Some(v) = self.by_id.get(&index_key(id, qm)) {
                    out.extend_from_slice(v);
                }
            }
            for c in e.classes() {
                if let Some(v) = self.by_class.get(&index_key(&c, qm)) {
                    out.extend_from_slice(v);
                }
            }
        }
        // Source order, so the `(specificity, order)` sort downstream is stable and correct.
        out.sort_unstable();
    }
}

/// A `@container` block lifted from raw sheet source: every enclosing `@container` prelude plus
/// its own (outermost first), the enclosing conditional at-rule preludes to re-wrap the body in
/// (so their gates still apply), and the body source itself.
struct CqBlock {
    cq_preludes: Vec<String>,
    wrappers: Vec<String>,
    body: String,
}

/// Find every `@container` block in `src` — comment- and string-aware, tracking the prelude of
/// each enclosing `{}` so a block nested in `@media`/`@supports`/`@layer` keeps those gates and
/// a block nested in another `@container` stacks both conditions.
///
/// This scanner exists because stylo's servo build parses the `@container` AT-RULE only under
/// `cfg!(feature = "gecko")` (rule_parser.rs) — a compile-time cfg, not a pref, so the whole
/// block is discarded as an unknown at-rule before the cascade ever sees it. The ladder's rung 3
/// (supplement) applies: the conditions and bodies are handed back to Stylo's own PUBLIC parsers
/// (`ContainerCondition::parse`, `Stylesheet::from_str`) — no grammar of our own.
///
/// A `@container` nested inside a STYLE rule (CSS nesting with `&`) is skipped: its inner
/// selectors are relative to the enclosing rule and would match wrongly if re-parsed standalone.
/// Named residue, not silent — the block simply stays off, which is the pre-supplement state.
fn extract_container_blocks(src: &str) -> Vec<CqBlock> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    // (prelude, body_start) for every open `{`.
    let mut stack: Vec<(String, usize)> = Vec::new();
    let mut seg_start = 0usize;
    let mut i = 0usize;
    while i < b.len() {
        match b[i] {
            b'/' if i + 1 < b.len() && b[i + 1] == b'*' => {
                let mut j = i + 2;
                while j + 1 < b.len() && !(b[j] == b'*' && b[j + 1] == b'/') {
                    j += 1;
                }
                i = (j + 2).min(b.len());
            }
            q @ (b'"' | b'\'') => {
                let mut j = i + 1;
                while j < b.len() && b[j] != q {
                    if b[j] == b'\\' {
                        j += 1;
                    }
                    j += 1;
                }
                i = (j + 1).min(b.len());
            }
            b'{' => {
                let prelude = src[seg_start..i].trim().to_string();
                stack.push((prelude, i + 1));
                seg_start = i + 1;
                i += 1;
            }
            b'}' => {
                if let Some((prelude, body_start)) = stack.pop() {
                    if prelude.starts_with("@container") {
                        let mut cq_preludes = Vec::new();
                        let mut wrappers = Vec::new();
                        let mut ok = true;
                        for (p, _) in &stack {
                            if p.starts_with("@container") {
                                cq_preludes.push(p.clone());
                            } else if p.starts_with("@media")
                                || p.starts_with("@supports")
                                || p.starts_with("@layer")
                            {
                                wrappers.push(p.clone());
                            } else {
                                ok = false; // style-rule nesting — named residue, see above
                                break;
                            }
                        }
                        if ok {
                            cq_preludes.push(prelude);
                            out.push(CqBlock {
                                cq_preludes,
                                wrappers,
                                body: src[body_start..i].to_string(),
                            });
                        }
                    }
                }
                seg_start = i + 1;
                i += 1;
            }
            b';' => {
                seg_start = i + 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    out
}

/// Parse one `@container` prelude's comma list of conditions with Stylo's OWN parser — the exact
/// grammar (`<container-name>? <container-condition>`, cq units, `and`/`or`/`not`) the gecko
/// build runs, reached through its public API.
fn parse_container_conditions(
    prelude: &str,
    url_data: &UrlExtraData,
    qm: QuirksMode,
) -> Option<Vec<ContainerCondition>> {
    let rest = prelude.strip_prefix("@container")?.trim();
    if rest.is_empty() {
        return None;
    }
    let context = stylo::parser::ParserContext::new(
        Origin::Author,
        url_data,
        Some(CssRuleType::Container),
        style_traits::ParsingMode::DEFAULT,
        qm,
        std::borrow::Cow::Owned(Namespaces::default()),
        None,
        None,
        stylo::custom_properties::AttrTaint::default(),
    );
    let mut input = cssparser37::ParserInput::new(rest);
    let mut parser = cssparser37::Parser::new(&mut input);
    parser
        .parse_comma_separated(|i| ContainerCondition::parse(&context, i))
        .ok()
        .filter(|c| !c.is_empty())
}

impl RuleIndex {
    /// The `@container` supplement: recover the blocks stylo's servo parse discarded (see
    /// [`extract_container_blocks`]) and index their rules with the condition stack attached.
    /// Supplemented rules are ordered after the sheet's own — a same-specificity BASE rule
    /// written after its `@container` override would wrongly lose to it (Chrome keeps source
    /// order). Named residue: overrides overwhelmingly follow their base rule.
    fn add_container_supplement(
        &mut self,
        sheets: &[&Stylesheet],
        lock: &SharedRwLock,
        url_data: &UrlExtraData,
        guard: &SharedRwLockReadGuard<'_>,
        device: &Device,
        qm: QuirksMode,
    ) {
        let mut order = self.rules.len();
        for sheet in sheets {
            let src = sheet.source();
            if !src.contains("@container") {
                continue;
            }
            for block in extract_container_blocks(src) {
                let mut levels: Vec<ServoArc<Vec<ContainerCondition>>> = Vec::new();
                let mut all_ok = true;
                for p in &block.cq_preludes {
                    match parse_container_conditions(p, url_data, qm) {
                        Some(c) => levels.push(ServoArc::new(c)),
                        None => {
                            all_ok = false;
                            break;
                        }
                    }
                }
                if !all_ok {
                    continue;
                }
                // Re-wrap the body in its enclosing conditional at-rules (outermost first) so
                // their gates re-apply on the standalone parse; a nested `@container` inside
                // this body is dropped by that parse and picked up as its own deeper block.
                let mut text = block.body;
                for w in block.wrappers.iter().rev() {
                    text = format!("{w} {{ {text} }}");
                }
                let parsed = StyloStylesheet::from_str(
                    &text,
                    url_data.clone(),
                    Origin::Author,
                    ServoArc::new(lock.wrap(MediaList::empty())),
                    lock.clone(),
                    None,
                    None,
                    qm,
                    AllowImportRules::Yes,
                );
                // `IndexedRule` owns its selector clone and refcounts its declaration block, so
                // the temporary stylesheet itself does not need to be kept alive.
                let mut cq_stack = levels;
                let rules = parsed.contents.read_with(guard).rules(guard);
                // Author sheets only (the caller says so, and the UA sheet has no `@container`).
                self.add_rules(
                    rules,
                    guard,
                    device,
                    &mut order,
                    qm,
                    &mut cq_stack,
                    ORIGIN_AUTHOR,
                    None,
                );
            }
        }
    }
}

/// One `::before` / `::after` rule, hoisted out of the sheet tree at index-build time.
struct PseudoRule {
    sel: selectors::parser::Selector<stylo::selector_parser::SelectorImpl>,
    /// Same first-sort as [`IndexedRule::origin_rank`] — the UA sheet's `summary::before` marker
    /// must lose to an author `*::before` the same way its `body` margin loses to an author `*`.
    origin_rank: u8,
    spec: u32,
    /// Global source order across all sheets — counted over *every* selector, matching or not,
    /// so the cascade order is identical to the per-element tree walk this replaces.
    ///
    /// **Belt-and-braces, and measured to be so.** G_PSEUDO_CASCADE's RED patches show that
    /// neither zeroing this field nor skipping the increment on non-pseudo selectors changes any
    /// result: rules are collected in source order and the winner sort is stable, so source order
    /// already survives. It is kept because it makes the ordering explicit rather than a property
    /// of two other things staying true — but it should not be mistaken for what is load-bearing.
    order: usize,
    block: ServoArc<stylo::shared_lock::Locked<PropertyDeclarationBlock>>,
}

/// **The `::before`/`::after` rules, collected ONCE per document instead of per element.**
///
/// `RuleIndex`'s doc comment above records that `cascade_one_element` used to walk every rule in
/// every sheet for every element, that this made the cascade O(elements × rules), and that
/// bucketing fixed it. **The pseudo-element path never got that fix**, and it is worse than the
/// original: `cascade_pseudo` ran the whole recursive descent over all 69 sheets *twice per
/// element* — re-reading every `Locked` rule list under the guard, re-evaluating every `@media`
/// query against the same unchanged device, and re-testing every selector's `pseudo_element()` —
/// to find the handful of rules that carry a pseudo at all.
///
/// Measured on a wix.com snapshot (10,424 nodes, 1.8 MB of CSS in 68 blocks) with
/// `MANUK_CASCADE_PROFILE=1`: **9.0 s of each 19.5 s cascade — 46% — was this function**, and the
/// cascade runs 8× per page load, so it was ~72 s of a 165 s load.
///
/// The hoist changes no semantics. The same selectors are tested with the same
/// `ForStatelessPseudoElement` matching mode, the same specificity, and the same global source
/// order — the `order` counter is advanced over every selector during collection exactly as the
/// walk advanced it. What disappears is only work whose result could not vary by element: the tree
/// descent, the lock reads, and the media-query evaluation, all of which depend on the device and
/// the sheets, not on the element being styled.
struct PseudoIndex {
    before: Vec<PseudoRule>,
    after: Vec<PseudoRule>,
}

impl PseudoIndex {
    fn build(
        sheets: &[ServoArc<StyloStylesheet>],
        guard: &SharedRwLockReadGuard<'_>,
        device: &Device,
        qm: QuirksMode,
    ) -> Self {
        let mut idx = PseudoIndex {
            before: Vec::new(),
            after: Vec::new(),
        };
        let mut order = 0usize;
        for sheet in sheets {
            let rank = origin_rank(sheet.contents.read_with(guard).origin);
            let rules = sheet.contents.read_with(guard).rules(guard);
            idx.collect(&rules, guard, device, qm, &mut order, rank);
        }
        idx
    }

    fn collect(
        &mut self,
        rules: &[CssRule],
        guard: &SharedRwLockReadGuard<'_>,
        device: &Device,
        qm: QuirksMode,
        order: &mut usize,
        origin_rank: u8,
    ) {
        use stylo::selector_parser::PseudoElement as Pe;
        for rule in rules {
            match rule {
                CssRule::Style(style_rule) => {
                    let sr = style_rule.read_with(guard);
                    for sel in sr.selectors.slice() {
                        // Advance the counter for EVERY selector, matching or not — the source
                        // order this produces has to be the same one the per-element walk
                        // produced, or rules that tie on specificity would reorder.
                        let bucket = match sel.pseudo_element() {
                            Some(&Pe::Before) => Some(false),
                            Some(&Pe::After) => Some(true),
                            _ => None,
                        };
                        if let Some(is_after) = bucket {
                            let r = PseudoRule {
                                sel: sel.clone(),
                                origin_rank,
                                spec: sel.specificity(),
                                order: *order,
                                block: sr.block.clone(),
                            };
                            if is_after {
                                self.after.push(r);
                            } else {
                                self.before.push(r);
                            }
                        }
                        *order += 1;
                    }
                }
                // `@media` is device-scoped, so it is evaluated ONCE here rather than once per
                // element. That is the single largest saving in this type: the old path called
                // `ml.evaluate` for every media block for every element.
                CssRule::Media(media_rule) => {
                    let ml = media_rule.media_queries.read_with(guard);
                    let mut custom = CustomMediaEvaluator::none();
                    if ml.evaluate(device, qm, &mut custom) {
                        let nested = media_rule.rules.read_with(guard);
                        self.collect(&nested.0, guard, device, qm, order, origin_rank);
                    }
                }
                CssRule::Supports(supports_rule) => {
                    // Same honest verdict as `RuleIndex::add_rules` — a `::before` rule inside an
                    // `@supports` the page should not have entered must not apply either.
                    if supports_rule.enabled
                        && honest_supports(&supports_rule.condition).unwrap_or(true)
                    {
                        let nested = supports_rule.rules.read_with(guard);
                        self.collect(&nested.0, guard, device, qm, order, origin_rank);
                    }
                }
                CssRule::LayerBlock(layer_rule) => {
                    let nested = layer_rule.rules.read_with(guard);
                    self.collect(&nested.0, guard, device, qm, order, origin_rank);
                }
                _ => {}
            }
        }
    }

    fn rules_for(&self, want: &stylo::selector_parser::PseudoElement) -> &[PseudoRule] {
        use stylo::selector_parser::PseudoElement as Pe;
        match want {
            Pe::Before => &self.before,
            Pe::After => &self.after,
            // Only ::before/::after are generated here; anything else has no rules to offer.
            _ => &[],
        }
    }

    /// Whether any rule in the document targets a generated-content pseudo at all.
    ///
    /// The overwhelmingly common case on real pages is a handful; a fair number of documents have
    /// none, and those should not pay a per-element call at all.
    fn is_empty(&self) -> bool {
        self.before.is_empty() && self.after.is_empty()
    }
}

fn match_rules_recursive(
    rules: &[CssRule],
    guard: &SharedRwLockReadGuard<'_>,
    device: &Device,
    el: &StyloElement<'_>,
    caches: &mut SelectorCaches,
    winners: &mut Vec<(
        u32,
        usize,
        ServoArc<stylo::shared_lock::Locked<PropertyDeclarationBlock>>,
    )>,
    order: &mut usize,
) {
    for rule in rules {
        match rule {
            CssRule::Style(style_rule) => {
                let sr = style_rule.read_with(guard);
                for sel in sr.selectors.slice() {
                    let mut ctx = MatchingContext::new(
                        MatchingMode::Normal,
                        None,
                        caches,
                        qm_of(el.dom),
                        NeedsSelectorFlags::No,
                        MatchingForInvalidation::No,
                    );
                    if matches_selector(sel, 0, None, el, &mut ctx) {
                        winners.push((sel.specificity(), *order, sr.block.clone()));
                    }
                    *order += 1;
                }
            }
            CssRule::Media(media_rule) => {
                let ml = media_rule.media_queries.read_with(guard);
                let mut custom = CustomMediaEvaluator::none();
                if ml.evaluate(device, qm_of(el.dom), &mut custom) {
                    let nested = media_rule.rules.read_with(guard);
                    match_rules_recursive(&nested.0, guard, device, el, caches, winners, order);
                }
            }
            // `@supports` — feature queries. Skipping these was NOT a harmless simplification: the
            // modern web uses `@supports` for progressive enhancement, hiding a legacy fallback and
            // revealing the real layout inside the block. Ignoring it means we silently rendered
            // the FALLBACK of every such site. (Wikipedia hides its whole TOC sidebar with
            // `display:none`, then re-shows it inside `@supports (display:grid)` — so the sidebar
            // simply never appeared.) Stylo evaluates the condition at parse time into `enabled`.
            CssRule::Supports(supports_rule) => {
                // Same honest verdict as `RuleIndex::add_rules` — this is the per-element matcher
                // the index replaced, and the two must not disagree about which branch a page took.
                if supports_rule.enabled
                    && honest_supports(&supports_rule.condition).unwrap_or(true)
                {
                    let nested = supports_rule.rules.read_with(guard);
                    match_rules_recursive(&nested.0, guard, device, el, caches, winners, order);
                }
            }
            // `@layer` — a cascade layer's rules still apply (layer *ordering* is not modelled, so
            // they cascade by specificity/order like any author rule). Dropping them entirely would
            // lose real styles; modern design systems ship whole sheets inside `@layer`.
            CssRule::LayerBlock(layer_rule) => {
                let nested = layer_rule.rules.read_with(guard);
                match_rules_recursive(&nested.0, guard, device, el, caches, winners, order);
            }
            _ => {}
        }
    }
}

/// Compute one element's `ComputedValues`: match author rules, merge, cascade.
#[allow(clippy::too_many_arguments)]
/// Cascade a `::before` / `::after` **pseudo-element** and return its style, if any rule gives it
/// `content`.
///
/// Generated content is not a DOM node — script must never see it — so it is computed here and
/// carried on the originating element's style, then materialised as inline items at layout time.
/// Without it the web loses its icons, its quotation marks, its counters, its dividers and a great
/// deal of its layout scaffolding, all silently.
#[allow(clippy::too_many_arguments)]
fn cascade_pseudo(
    stylist: &Stylist,
    pseudo_index: &PseudoIndex,
    caches: &mut SelectorCaches,
    lock: &SharedRwLock,
    guard: &SharedRwLockReadGuard<'_>,
    guards: &StylesheetGuards<'_>,
    el: &StyloElement<'_>,
    parent_cv: &ServoArc<ComputedValues>,
    want: stylo::selector_parser::PseudoElement,
) -> Option<crate::ComputedStyle> {
    let candidates = pseudo_index.rules_for(&want);
    if candidates.is_empty() {
        return None;
    }
    let mut winners: Vec<(
        u8,
        u16,
        u32,
        usize,
        ServoArc<stylo::shared_lock::Locked<PropertyDeclarationBlock>>,
    )> = Vec::new();
    for r in candidates {
        let mut ctx = MatchingContext::new(
            MatchingMode::ForStatelessPseudoElement,
            None,
            caches,
            qm_of(el.dom),
            NeedsSelectorFlags::No,
            MatchingForInvalidation::No,
        );
        if matches_selector(&r.sel, 0, None, el, &mut ctx) {
            // ⚠ The PSEUDO index does not carry a layer rank yet — `UNLAYERED` for every rule, so
            // this sort behaves exactly as it did before layers were ranked. Stated rather than
            // silently omitted: `@layer { .x::before { … } }` still loses to nothing, which is the
            // pre-t790 behaviour and a named residue, not a claim.
            winners.push((r.origin_rank, UNLAYERED, r.spec, r.order, r.block.clone()));
        }
    }
    if winners.is_empty() {
        return None;
    }
    // ORIGIN FIRST, then LAYER, then specificity, then document order (CSS Cascade §6). Sorting on
    // `(spec, order)` alone let the UA sheet's type selectors beat an author reset's `*`; sorting
    // without the layer term let a layer beat the unlayered rules it exists to lose to.
    winners.sort_by_key(|(rank, layer, spec, ord, _)| (*rank, *layer, *spec, *ord));
    let mut merged = PropertyDeclarationBlock::new();
    for (_, _, _, _, block) in &winners {
        for (decl, importance) in block.read_with(guard).declaration_importance_iter() {
            merged.push(decl.clone(), importance);
        }
    }
    let arc = ServoArc::new(lock.wrap(merged));
    let cv = stylist.compute_for_declarations::<StyloElement>(guards, parent_cv, arc);
    let mut cs = to_computed_style(&cv);
    // Only a pseudo with `content` generates a box at all.
    use stylo::values::generics::counters::{Content, ContentItem};
    let text = match cv.get_counters().clone_content() {
        Content::Items(items) => {
            let mut out = String::new();
            for it in items.items.iter() {
                match it {
                    ContentItem::String(sv) => out.push_str(sv),
                    // `content: attr(name)` — the element's attribute value, or the EMPTY string
                    // when the attribute is absent (CSS2.1). Pushing nothing on a miss is exactly
                    // that: `a::after{content:" ("attr(href)")"}` still renders the parentheses,
                    // and `[data-x]::before{content:attr(data-x)}` draws the datum. Namespace is
                    // ignored — attributes are keyed by qualified (already-lowercased for HTML)
                    // name here, and a namespaced `attr()` in `content` is vanishingly rare.
                    ContentItem::Attr(a) => {
                        if let Some(v) = el.attr(&a.attribute) {
                            out.push_str(v);
                        }
                    }
                    _ => {}
                }
            }
            out
        }
        _ => return None,
    };
    cs.content = Some(text);
    Some(cs)
}

#[allow(clippy::too_many_arguments)]
fn cascade_one_element(
    stylist: &Stylist,
    index: &RuleIndex,
    candidates: &mut Vec<u32>,
    caches: &mut SelectorCaches,
    lock: &SharedRwLock,
    url_data: &UrlExtraData,
    guard: &SharedRwLockReadGuard<'_>,
    guards: &StylesheetGuards<'_>,
    el: &StyloElement<'_>,
    node: NodeId,
    parent_cv: &std::collections::HashMap<NodeId, ServoArc<ComputedValues>>,
    dom: &Dom,
    cq_active: bool,
) -> ServoArc<ComputedValues> {
    // Only the rules this element could possibly match — see `RuleIndex`. Everything below is
    // unchanged: each candidate is still fully matched by `matches_selector`, and winners are still
    // ordered by (specificity, source order).
    let mut winners: Vec<(
        u8,
        u16,
        u32,
        usize,
        ServoArc<stylo::shared_lock::Locked<PropertyDeclarationBlock>>,
    )> = Vec::new();
    index.candidates(dom, node, candidates);
    // ONE `MatchingContext` for the whole element, not one per candidate rule. `SelectorCaches` is a
    // real allocation (it is the ancestor/nth-index cache), and it was being built fresh for every
    // rule of every element — thrown away before it could cache anything, which is the exact
    // opposite of what a cache is for.
    let mut ctx = MatchingContext::new(
        MatchingMode::Normal,
        None,
        caches,
        qm_of(el.dom),
        NeedsSelectorFlags::No,
        MatchingForInvalidation::No,
    );
    for &i in candidates.iter() {
        let r = &index.rules[i as usize];
        if matches_selector(&r.sel, 0, None, el, &mut ctx) {
            // A rule nested under `@container` applies only if EVERY nesting level has a matching
            // condition (comma list within a level = OR — Stylo's `container_condition_matches`
            // semantics). On the unsized first pass (`!cq_active`) they are held off wholesale:
            // no layout has run, so the honest answer to "is the container ≥ 400px?" is unknown,
            // and unknown must never style (`to_bool(false)` — the same call Stylo makes).
            if !r.cq.is_empty() {
                if !cq_active {
                    continue;
                }
                let mut cq_flags = stylo::computed_value_flags::ComputedValueFlags::empty();
                let all_levels = r.cq.iter().all(|level| {
                    level.iter().any(|cond| {
                        cond.matches(stylist, *el, None, &mut cq_flags)
                            .to_bool(false)
                    })
                });
                if !all_levels {
                    continue;
                }
            }
            winners.push((
                r.origin_rank,
                r.layer_rank,
                r.spec,
                r.order,
                r.block.clone(),
            ));
        }
    }
    // ORIGIN FIRST, then LAYER, then specificity, then document order (CSS Cascade §6). Sorting on
    // `(spec, order)` alone let the UA sheet's type selectors beat an author reset's `*`; sorting
    // without the layer term let a layer beat the unlayered rules it exists to lose to.
    winners.sort_by_key(|(rank, layer, spec, ord, _)| (*rank, *layer, *spec, *ord));

    // Merge winning declarations (ascending priority: later overrides earlier).
    let mut merged = PropertyDeclarationBlock::new();
    for (_, _, _, _, block) in &winners {
        for (decl, importance) in block.read_with(guard).declaration_importance_iter() {
            merged.push(decl.clone(), importance);
        }
    }
    // Inline `style=` wins over all matched rules — append its declarations last.
    if let Some(inline) = el.dom.element(node).and_then(|e| e.attr("style")) {
        // **The inline `style=` attribute needs the quirks verdict as much as a stylesheet does**, and
        // it is a SEPARATE parse: `StyloStylesheet::from_str` handles `<style>`/linked CSS, this
        // handles the attribute. Wiring only the first left `style="width: 100"` still dropped on a
        // quirks page while the same rule in a `<style>` block worked — and legacy markup, which is
        // exactly the markup that lands in quirks mode, is overwhelmingly inline-styled. `el.dom` is
        // already in scope, so this is a field read rather than another parameter.
        let block =
            parse_style_attribute(inline, url_data, None, qm_of(el.dom), CssRuleType::Style);
        for (decl, importance) in block.declaration_importance_iter() {
            merged.push(decl.clone(), importance);
        }
    }
    let merged_arc = ServoArc::new(lock.wrap(merged));

    // Inherit from the nearest element ancestor's ComputedValues (already computed, since
    // we cascade in preorder); the root inherits from the device defaults.
    let default = stylist.device().default_computed_values();
    let mut ancestor = el.dom.parent(node);
    let parent_style = loop {
        match ancestor {
            Some(p) => {
                if let Some(cv) = parent_cv.get(&p) {
                    break &**cv;
                }
                ancestor = el.dom.parent(p);
            }
            None => break default,
        }
    };

    stylist.compute_for_declarations::<StyloElement>(guards, parent_style, merged_arc)
}

/// Does this engine actually honour `condition`? — the ONE answer, for both `@supports` and
/// `CSS.supports()`.
///
/// **The bug this exists to delete.** `@supports` has been honest since tick 276, because the
/// cascade asks Stylo and Stylo really parses the condition: `@supports (notaproperty: 1)` and
/// `@supports (container-type: inline-size)` both correctly fail to apply. `CSS.supports()` — the
/// JS half of the identical question — was a literal `return true`. So the two disagreed about the
/// same declaration, and the JS one was wrong in the direction that hurts: a page asking
/// `CSS.supports('container-type: inline-size')` was told **yes**, took its modern-layout branch,
/// and rendered it against a property this engine ignores. A "no" would have kept the fallback and
/// the page would have looked right.
///
/// **Why it is answered by PARSING A STYLESHEET rather than by a lookup table.** The temptation is a
/// list of supported properties. A list is a second source of truth: it is right the day it is
/// written and wrong the first time the engine gains or loses a property, and nothing makes it fail
/// loudly when it drifts. Instead this builds `@supports <condition> { ... }`, hands it to the same
/// `StyloStylesheet::from_str` the cascade uses, and reads back the `enabled` flag Stylo itself
/// computed. There is no second evaluator to keep in step — it is the *same* one, reached by a
/// different door.
///
/// **A measured caveat, pinned here so nobody re-derives it.** Some properties sit behind Stylo
/// runtime prefs that `Page::load` turns on — `display: grid` is one. Called from a bare unit test
/// with those prefs unset this returns `false` for grid; called from a loaded page it returns
/// `true`, and so does the cascade. The two agree *in every context where `CSS.supports` actually
/// exists*, because JS only runs inside a page — which is why `G_CSS_SUPPORTS` asserts the
/// agreement from inside a real `Page::load`, and why the unit tests below stay off pref-gated
/// properties rather than pinning a configuration the browser never runs in.
pub fn supports_condition(condition: &str) -> bool {
    // A condition containing a block delimiter could otherwise close the `@supports` block and
    // inject rules, which would make the probe answer a question nobody asked.
    if condition.is_empty() || condition.contains('{') || condition.contains('}') {
        return false;
    }
    // The same pref set the cascade flips (see `cascade_via_stylo_sized`) — `@supports` must
    // answer from the SAME parser configuration the cascade styles with, or the answer here and
    // the behaviour there disagree depending on which ran first (a global pref set only on the
    // cascade path made this function's verdict order-dependent).
    stylo_static_prefs::set_pref!("layout.grid.enabled", true);
    stylo_static_prefs::set_pref!("layout.container-queries.enabled", true);
    stylo_static_prefs::set_pref!("layout.unimplemented", true);
    stylo_static_prefs::set_pref!("layout.css.contrast-color.enabled", true);

    // `CSS.supports(cond)` takes a <supports-condition>, but every browser also accepts a bare
    // declaration (`CSS.supports('display: flex')`). Wrap only when the caller did not, and leave
    // compound conditions (`(a) and (b)`, `not (a)`) alone.
    let trimmed = condition.trim();
    let wrapped = if trimmed.starts_with('(') || trimmed.starts_with("not ") {
        trimmed.to_string()
    } else {
        format!("({trimmed})")
    };

    let source = format!("@supports {wrapped} {{ manukprobe {{ color: red; }} }}");

    let lock = SharedRwLock::new();
    let Ok(url) = ::url::Url::parse("about:manuk") else {
        return false;
    };
    let url_data = UrlExtraData(ServoArc::new(url));
    let media = ServoArc::new(lock.wrap(MediaList::empty()));
    let parsed = StyloStylesheet::from_str(
        &source,
        url_data,
        Origin::Author,
        media,
        lock.clone(),
        None,
        None,
        QuirksMode::NoQuirks,
        AllowImportRules::No,
    );

    let guard = lock.read();
    // A condition Stylo could not parse produces no `@supports` rule at all — which is a "no", not
    // an error, exactly as the spec's "return false" for an unparseable condition.
    parsed
        .contents
        .read_with(&guard)
        .rules(&guard)
        .iter()
        .find_map(|rule| match rule {
            // Stylo's `enabled` answers *"does this parse?"*. That is the right question for every
            // property except the ones we made parseable on purpose — see `honest_supports`.
            CssRule::Supports(s) => Some(honest_supports(&s.condition).unwrap_or(s.enabled)),
            _ => None,
        })
        .unwrap_or(false)
}

/// **The 31 properties `layout.unimplemented` ungates that this engine does NOT render.**
///
/// Stylo's servo build hides 35 longhands behind one shared pref. The cascade flips it on because
/// **four** of them are real here — `user-select`, `color-scheme`, `mask-image`, `text-overflow` —
/// and Stylo drops those at parse time otherwise. The flip's comment claimed the rest were
/// harmless: *"we consume a fixed set of computed values via explicit `clone_*` calls, so enabling
/// the other properties it also ungates changes nothing we read."* **`@supports` reads them**, and
/// so does `CSS.supports()`, because both answer *"does this parse?"*.
///
/// Which four are honest was **measured, not recalled**: a property counts only if it reaches a
/// `ComputedStyle` field. Three of the four arrive through the MinimalCascade recovery block rather
/// than a `clone_*` accessor, which is why grepping for `clone_*` under-counts them.
///
/// **The list is a denylist rather than an allowlist deliberately.** A new property Stylo adds
/// behind this pref should default to *unsupported* — the failure mode of a missing denylist entry
/// is a false "yes", which throws away a page's fallback; the failure mode of a stale one is a
/// false "no", which keeps a working page. When one of these is genuinely implemented, delete its
/// line here and `G_SUPPORTS_HONESTY` will hold the new answer.
const PARSE_ONLY_LONGHANDS: &[&str] = &[
    "animation-composition",
    "animation-range-end",
    "animation-range-start",
    "animation-timeline",
    "contain",
    // ── **THE SHORTHANDS WERE MISSED WHILE ALL EIGHT OF THEIR LONGHANDS WERE LISTED** (surface
    // audit #34, tick 679). `CSS.supports('corner-shape','squircle')` answered **true** and
    // `getComputedStyle(el).cornerShape` is `undefined`; same for `mask-position` against
    // `mask-position-x`/`-y` below. This list names LONGHANDS, `honest_supports` subtracts what it
    // names, and a page asks about whichever spelling it uses — so listing every longhand of a
    // property and not the shorthand leaves the false YES fully intact for the spelling authors
    // actually write.
    //
    // **The rule, stated so it can be applied rather than remembered: a shorthand must answer NO iff
    // EVERY one of its longhands is parse-only.** `mask` deliberately does NOT qualify — `mask-image`
    // is real (the icon-mask paint phase reads it), so `mask` is partly implemented and answering no
    // would be a false NO, which costs a page its enhancement branch just as surely.
    //
    // This is "one rule, N implementations — fix one, GREP FOR THE OTHER", which is now the ninth
    // time that class has fired here.
    "corner-shape",
    "mask-position",
    "corner-bottom-left-shape",
    "corner-bottom-right-shape",
    "corner-end-end-shape",
    "corner-end-start-shape",
    "corner-start-end-shape",
    "corner-start-start-shape",
    "corner-top-left-shape",
    "corner-top-right-shape",
    "counter-increment",
    "counter-reset",
    "mask-clip",
    "mask-composite",
    "mask-mode",
    "mask-origin",
    "mask-position-x",
    "mask-position-y",
    "mask-repeat",
    "mask-size",
    "mask-type",
    "offset-path",
    "position-area",
    "position-try-fallbacks",
    "view-transition-class",
    "view-transition-name",
    // `zoom` LEFT THIS LIST at tick 601, and it is the first entry to leave for the OPPOSITE
    // reason to the rest: it was never unimplemented. **Stylo applies it inside its own length
    // computation** (`effective_zoom`), so it works without this engine reading a `zoom` field at
    // all — measured end to end: a `zoom: 2` 50px box lays out at 100px, its `font-size: 10px`
    // computes to 20px, and a 20px CHILD comes out at 40px, which is inheritance behaving too.
    // Denying it was a FALSE NO, and a false no costs a page its enhancement branch just as surely
    // as a false yes costs it a fallback.
];

/// **Properties Stylo's servo build parses NATIVELY and this engine still does not render.**
///
/// t576 closed this defect for the 35 longhands behind the `layout.unimplemented` pref, and scoped the
/// fix to that pref's property set — which was the shape of the bug as it presented, and one category
/// too narrow. **The general defect is "Stylo parses it, we never consume it, and `@supports` says
/// yes"**, and the pref is only one reason a property can land in that state. These need no pref at
/// all: Stylo computes them correctly and nothing reads the result.
///
/// Found by surface audit #32 (t588) pulling the Blink use counters, and each verified here by all
/// three routes a computed value can reach us — no `clone_*` in `stylo_map.rs`, no `ComputedStyle`
/// field, and no entry in the MinimalCascade recovery block:
///
/// | property | % of page loads |
/// |---|---|
/// | `filter` | **51.9%** |
/// | `clip-path` | **43.8%** |
/// | `backdrop-filter` | 34.3% |
/// | `isolation` | 18.0% |
/// | `mix-blend-mode` | 12.9% |
/// | `writing-mode` | 8.3% (+5.4% prefixed) |
///
/// `filter` is the expensive one to get wrong: **there is no cascade-level workaround for a blur**, so
/// a page told yes drops the opaque fallback it shipped for engines that cannot blur and puts its text
/// unreadably over a photograph — the exact scenario t576 was written about, still live, one category
/// over. Delete a line here the moment its property is genuinely rendered; `G_SUPPORTS_HONESTY` holds
/// the answer either way.
const UNRENDERED_LONGHANDS: &[&str] = &[
    // `text-justify` JOINED at tick 601, found by the same probe that freed `zoom`. Stylo's servo
    // build parses it natively and nothing in this engine reads it — the t591 category exactly.
    // One probe, two corrections, in OPPOSITE directions.
    "text-justify",
    // `filter` LEFT THIS LIST at tick 592 — it is rendered now (`stylo_map` reads the computed
    // list, `manuk-paint` runs the pipeline over an offscreen group). The list is meant to shorten
    // exactly this way, one entry per landed capability, each with its own evidence.
    //
    // `backdrop-filter` LEFT at tick 595 — the last member of the visual-effects bundle, and the
    // one that genuinely needed the new input rather than a new field. This list is now down to the
    // three properties that really are unread.
    //
    // `clip-path` LEFT at tick 593 — the four basic shapes (`inset`/`circle`/`ellipse`/`polygon`)
    // clip the group's offscreen surface. `path()`/`shape()`/`url()` still do not, which is a
    // narrower "no" than this list can express: `@supports (clip-path: circle(50%))` is now honestly
    // yes and `@supports (clip-path: path(...))` is honestly-yes-but-unrendered. Taking the yes is
    // the right trade — the basic shapes are what pages branch on.
    "isolation",
    "writing-mode",
    "text-orientation",
];

/// A declaration that no engine supports, substituted for a parse-only one so that **Stylo** — not
/// hand-rolled boolean logic — resolves the surrounding `and`/`or`/`not`.
const NEVER_SUPPORTED: &str = "-manuk-not-a-property: 1";

/// **The honest verdict for an `@supports` condition, or `None` when Stylo's own is already right.**
///
/// The whole difficulty is composition: `not (backdrop-filter: blur(1px))` must be **true** while
/// `(backdrop-filter: blur(1px))` must be **false**, and a filter that merely asks *"does the text
/// mention a banned property?"* gets that exactly backwards. So nothing here evaluates anything:
/// the condition tree is **rewritten** — every declaration naming a parse-only property becomes
/// [`NEVER_SUPPORTED`] — and the rewrite is handed back to Stylo, which already knows how `and`,
/// `or` and `not` compose. `None` means the rewrite changed nothing, so Stylo's answer stands and
/// no second parse is paid for.
///
/// Depth-bounded: a condition is untrusted input and a stack overflow in the cascade is Bar 0.
fn honest_supports(cond: &stylo::stylesheets::supports_rule::SupportsCondition) -> Option<bool> {
    let (rewritten, changed) = rewrite_parse_only(cond, 0);
    if !changed {
        return None;
    }
    use style_traits::ToCss;
    Some(supports_condition(&rewritten.to_css_string()))
}

/// Replace every declaration naming a [`PARSE_ONLY_LONGHANDS`] property with [`NEVER_SUPPORTED`],
/// reporting whether anything changed.
fn rewrite_parse_only(
    cond: &stylo::stylesheets::supports_rule::SupportsCondition,
    depth: u32,
) -> (stylo::stylesheets::supports_rule::SupportsCondition, bool) {
    use stylo::stylesheets::supports_rule::{Declaration, SupportsCondition as SC};
    if depth > 32 {
        return (cond.clone(), false);
    }
    match cond {
        SC::Declaration(d) => {
            // `Declaration` holds the raw `prop: value` slice; the property is everything before
            // the first colon. Compared case-insensitively, as CSS property names are.
            let name = d.0.split(':').next().unwrap_or("").trim();
            // Both lists answer the same question — *do we RENDER this?* — and differ only in why the
            // property became parseable. Kept separate so each carries its own evidence and can be
            // shortened independently as capabilities land.
            if PARSE_ONLY_LONGHANDS
                .iter()
                .chain(UNRENDERED_LONGHANDS.iter())
                .any(|p| name.eq_ignore_ascii_case(p))
            {
                (
                    SC::Declaration(Declaration(NEVER_SUPPORTED.to_string())),
                    true,
                )
            } else {
                (cond.clone(), false)
            }
        }
        SC::Not(inner) => {
            let (i, c) = rewrite_parse_only(inner, depth + 1);
            (SC::Not(Box::new(i)), c)
        }
        SC::Parenthesized(inner) => {
            let (i, c) = rewrite_parse_only(inner, depth + 1);
            (SC::Parenthesized(Box::new(i)), c)
        }
        SC::And(list) => {
            let mut changed = false;
            let out = list
                .iter()
                .map(|c| {
                    let (i, ch) = rewrite_parse_only(c, depth + 1);
                    changed |= ch;
                    i
                })
                .collect();
            (SC::And(out), changed)
        }
        SC::Or(list) => {
            let mut changed = false;
            let out = list
                .iter()
                .map(|c| {
                    let (i, ch) = rewrite_parse_only(c, depth + 1);
                    changed |= ch;
                    i
                })
                .collect();
            (SC::Or(out), changed)
        }
        // `selector()`, `font-format()`, `font-tech()` and future syntax name no property, so the
        // pref cannot have inflated their answer.
        other => (other.clone(), false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⚠⚠⚠ **THE TWO UA SHEETS ARE HAND-MAINTAINED TWINS AND THEY DRIFT — SO MAKE THE DRIFT A
    /// GATE INSTEAD OF A COMMENT.**
    ///
    /// `UA_CSS` (Stylo, the SHIPPING cascade) and `apply_ua_defaults` (`MinimalCascade`, which is
    /// what `manuk-agent` and every non-`stylo` build run) must agree about which elements are
    /// **block**. They have now disagreed three ticks running, in both directions:
    ///
    /// * **t851** — `MinimalCascade` gave `box-sizing: border-box` to all four form tags and
    ///   `stylo_engine.rs` gave it to none. Each sheet's error hid the other's from whichever test
    ///   you happened to write.
    /// * **t853** — `UA_CSS` blocks `form, fieldset, center, menu, dl` and `summary`; this list
    ///   carried the table family and **none of the rest**, so under `MinimalCascade` a `<form>`
    ///   was a *boxless inline*. Once an inline reported its own content area, the form's box came
    ///   out **smaller than the button inside it**, and `A11yNode::hit_test` — smallest-area-wins —
    ///   gave the agent's coordinate click to the form instead of the control.
    ///
    /// The comment above `apply_ua_defaults` has said *"keep in lockstep"* the whole time. A
    /// comment cannot go red. This reads the shipping sheet's own `display: block` rule as the
    /// source of truth and asserts the minimal cascade agrees, tag by tag.
    ///
    /// **How it goes RED:** delete any tag from either list. Removing `form` from
    /// `apply_ua_defaults` reproduces the t853 mis-actuation exactly.
    #[test]
    fn both_ua_sheets_agree_on_which_elements_are_block() {
        // The shipping sheet's block rule, read out of the sheet rather than re-typed — a copy
        // would drift for the same reason the two sheets did.
        let decl = "{ display: block; }";
        let rule = UA_CSS
            .split(decl)
            .next()
            .expect("UA_CSS has a `display: block` rule");
        let selectors = &rule[rule
            .rfind("*/")
            .map(|i| i + 2)
            .unwrap_or(0)
            .max(rule.rfind('}').map(|i| i + 1).unwrap_or(0))..];
        let tags: Vec<&str> = selectors
            .split(',')
            .map(|t| t.trim())
            .filter(|t| !t.is_empty() && t.chars().all(|c| c.is_ascii_alphanumeric()))
            .collect();
        assert!(
            tags.len() > 20 && tags.contains(&"form") && tags.contains(&"div"),
            "the UA_CSS block rule did not parse — got {tags:?}. This assertion exists so a \
             reformatting of the sheet cannot silently turn this gate into a no-op over an empty \
             list, which is the vacuous-gate failure mode."
        );

        let mut drifted = Vec::new();
        for tag in &tags {
            let mut s = crate::ComputedStyle::initial();
            let el = manuk_dom::ElementData {
                name: (*tag).to_string(),
                attrs: Vec::new(),
                namespace: None,
            };
            crate::apply_ua_defaults(&mut s, &el);
            // `table` and `caption` are block-level in CSS's sense and carry their own more
            // specific inner display in the minimal cascade; everything else must be plain Block.
            let ok = matches!(
                s.display,
                crate::Display::Block | crate::Display::Table | crate::Display::TableCaption
            );
            if !ok {
                drifted.push(format!("{tag} -> {:?}", s.display));
            }
        }
        assert!(
            drifted.is_empty(),
            "THE TWO UA SHEETS HAVE DRIFTED. `stylo_engine.rs`'s UA_CSS says these are \
             `display: block`; `apply_ua_defaults` (MinimalCascade — what manuk-agent runs) does \
             not: {drifted:?}.\n  A block element laid out as a boxless inline gets its geometry \
             lifted from its children, which is how a <form> ended up SMALLER than the <button> \
             inside it and stole the agent's click (t853)."
        );
    }

    /// `supports_condition` is the ONE evaluator behind both `@supports` and `CSS.supports()`.
    /// These assert it at the Rust boundary, so a JS-side regression and an engine-side one fail
    /// in different places.
    #[test]
    fn supports_condition_answers_from_the_real_parser() {
        // Implemented.
        assert!(supports_condition("display: flex"));
        assert!(supports_condition("(display: flex)"));
        assert!(supports_condition("position: sticky"));
        assert!(supports_condition("color: red"));
        // Container queries LANDED (tick 379: sized cascade re-pass + the @container supplement),
        // so the honest answer FLIPPED — the old `assert!(!...)` here was the honest "no" of an
        // engine without them, and keeping it now would be the inverse lie. This is the documented
        // moment from the honest-answer rule: the gate follows the capability, never the reverse.
        assert!(supports_condition("container-type: inline-size"));
        // ── Real properties this engine does not implement — the ones whose false "yes" made pages
        //    discard a working fallback. All of these PARSE (the cascade flips `layout.unimplemented`
        //    for the four properties in that set it really renders), so Stylo alone answers yes and
        //    `PARSE_ONLY_LONGHANDS` is what makes the answer honest.
        assert!(!supports_condition("view-transition-name: foo"));
        assert!(!supports_condition("offset-path: none"));
        assert!(!supports_condition("mask-repeat: no-repeat"));
        // ── The SECOND category (t591): properties Stylo's servo build parses NATIVELY, behind no
        //    pref at all, that this engine still does not render. t576 scoped its fix to the
        //    `layout.unimplemented` set and was one category too narrow — `filter` is on 51.9% of page
        //    loads and answered YES, which is the costliest possible wrong answer here because there
        //    is no cascade-level workaround for a blur.
        //
        //    **`filter` LEFT THIS SET AT TICK 592** — it is rendered now, so the honest answer
        //    flipped a second time and its assertion moved down to the rendered group. Two flips in
        //    two ticks is not churn: t591 corrected a lie, t592 removed the reason for it.
        // `clip-path` moved to the rendered set at t593 (basic shapes).
        // `mix-blend-mode` moved to the rendered set at t594.
        assert!(!supports_condition("isolation: isolate"));
        assert!(!supports_condition("text-justify: inter-word"));
        assert!(!supports_condition("writing-mode: vertical-rl"));
        // …and composition still resolves through Stylo for the remaining list too.
        assert!(supports_condition("not (isolation: isolate)"));
        assert!(!supports_condition(
            "(display: flex) and (isolation: isolate)"
        ));
        assert!(supports_condition(
            "(display: flex) or (isolation: isolate)"
        ));
        // ── …and the properties that ARE rendered must keep answering yes, or the fix has traded a
        //    false yes for a worse false no. Three of them arrive through the MinimalCascade
        //    recovery block rather than a `clone_*` accessor.
        assert!(supports_condition("filter: blur(4px)"));
        assert!(supports_condition("clip-path: circle(50%)"));
        assert!(supports_condition("mix-blend-mode: multiply"));
        assert!(supports_condition("backdrop-filter: blur(4px)"));
        // `zoom` is RENDERED — by Stylo's own effective-zoom machinery, not by a field we read —
        // so the honest answer is yes. It was a FALSE NO until tick 601 measured it.
        assert!(supports_condition("zoom: 2"));
        assert!(supports_condition(
            "(display: flex) and (filter: blur(4px))"
        ));
        assert!(!supports_condition("not (filter: blur(4px))"));
        assert!(supports_condition("user-select: none"));
        assert!(supports_condition("color-scheme: dark"));
        assert!(supports_condition("mask-image: url(a.svg)"));
        assert!(supports_condition("text-overflow: ellipsis"));
        // ── COMPOSITION, which is the whole difficulty. `not (<unsupported>)` is TRUE — the case a
        //    filter that merely asked "does the text mention a banned property?" gets backwards. The
        //    condition tree is rewritten and handed back to Stylo precisely so this comes out right.
        assert!(supports_condition("not (offset-path: none)"));
        assert!(!supports_condition(
            "(display: flex) and (offset-path: none)"
        ));
        assert!(supports_condition("(display: flex) or (offset-path: none)"));
        assert!(supports_condition(
            "(user-select: none) and (display: flex)"
        ));
        // Nonsense.
        assert!(!supports_condition("notaproperty: 1"));
        assert!(!supports_condition("color: notacolor"));
        assert!(!supports_condition("color"));
        assert!(!supports_condition(""));
        // Compound conditions come free from Stylo — a lookup table would need its own parser.
        assert!(supports_condition("(display: flex) and (color: red)"));
        assert!(!supports_condition("(display: flex) and (notaprop: 1)"));
        assert!(supports_condition("not (notaprop: 1)"));
        assert!(!supports_condition("not (display: flex)"));
    }

    /// A condition carrying a block delimiter must not be able to close the probe stylesheet and
    /// have its own rules parsed — that would answer a question nobody asked.
    #[test]
    fn supports_condition_cannot_be_escaped_with_a_brace() {
        assert!(!supports_condition(
            "(display:flex) { } @supports (display:flex)"
        ));
        assert!(!supports_condition("}"));
    }

    /// `text-align: start` (the INITIAL value) and `end` resolve to physical left/right against the
    /// element's `direction`. The map hard-wired `end`→right and `start`→left, so an RTL paragraph —
    /// the whole Arabic/Hebrew/Persian web — left-aligned its body text instead of right-aligning.
    ///
    /// RED, run: delete the `resolve_physical` line in `cascade_via_stylo_sized` (or revert
    /// `map_text_align` to `End=>Right, _=>Left`). The `dir=rtl` default reads `Left`, not `Right`.
    #[test]
    fn text_align_start_and_end_resolve_against_direction() {
        let dom = manuk_html::parse(
            r#"<p id="l">a</p><p id="r" dir="rtl">ب</p><p id="re" dir="rtl" style="text-align:end">ب</p><p id="rl" dir="rtl" style="text-align:left">ب</p>"#,
        );
        let sheet = Stylesheet::parse("");
        let map = cascade_via_stylo(&dom, std::slice::from_ref(&sheet), 800.0, 600.0);
        let id = |v: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(v))
                .unwrap()
        };
        use crate::TextAlign as A;
        // LTR default: initial `start` → left.
        assert_eq!(map[&id("l")].text_align, A::Left, "LTR start→left");
        // RTL default: initial `start` → RIGHT — the fix.
        assert_eq!(map[&id("r")].text_align, A::Right, "RTL start→right");
        // `text-align:end` in RTL → left.
        assert_eq!(map[&id("re")].text_align, A::Left, "RTL end→left");
        // An explicit PHYSICAL `left` is honoured even in RTL (not logical).
        assert_eq!(
            map[&id("rl")].text_align,
            A::Left,
            "explicit physical left stays left in RTL"
        );
    }

    /// **CSS NESTING: `&` MUST BE SUBSTITUTED WITH THE ENCLOSING SELECTOR, NOT LEFT TO MEAN `<html>`.**
    ///
    /// `RuleIndex` recurses into a style rule's nested rules (t659) but used to index their selectors
    /// verbatim — and a verbatim `&` is `Component::ParentSelector`, which the matcher resolves as
    /// `scope_element` if one is set and **`element.is_root()`** if not. We never set one, so every
    /// `&` on the web matched `<html>`.
    ///
    /// ⚠ It did not fail as a clean "nested rules are dropped", which is why it survived: the
    /// DESCENDANT form matched *by accident* (`<html>` is an ancestor of everything), so it applied
    /// document-wide while carrying no specificity from `&`. Measured against live Chromium:
    ///
    /// | rule | Chrome | was |
    /// |---|---|---|
    /// | `#a { width:50px; & { width:300px } }` | 300 | **50** |
    /// | `#h { width:40px; &:not(.x){ width:260px } }` | 260 | **40** |
    /// | `.p { & > span { width:240px } }` | 240 | **73** |
    /// | `#other { & .leak { width:500px } }` (a `.leak` INSIDE `#other`) | 500 | **100** |
    ///
    /// This asserts all four shapes plus the two that already worked, and — the load-bearing one —
    /// that a nested descendant rule does **NOT** leak to a `.leak` outside `#other`, which is the
    /// over-match the root accident produced and which a fix that merely "makes `&` match anything"
    /// would leave in place.
    ///
    /// RED, run: replace `sr.selectors.replace_parent_selector(p)` with `sr.selectors.clone()` in
    /// `RuleIndex::add_rules` — `bare`, `compound` and `child` read their un-nested widths (50/40/30)
    /// and `inside` reads 100 instead of 500.
    #[test]
    fn a_nested_rules_ampersand_resolves_to_the_enclosing_selector_not_the_root() {
        let dom = manuk_html::parse(
            r#"<div id="a">x</div>
               <div id="h">x</div>
               <div class="p"><span id="c3">x</span></div>
               <div id="other"><div class="leak" id="inside">x</div></div>
               <div><div class="leak" id="outside">x</div></div>"#,
        );
        let sheet = Stylesheet::parse(
            "#a { width: 50px; & { width: 300px } }
             #h { width: 40px; &:not(.x) { width: 260px } }
             .p { & > span { width: 240px; display: block } }
             #other { & .leak { width: 500px } }
             .leak { width: 100px }",
        );
        let map = cascade_via_stylo(&dom, std::slice::from_ref(&sheet), 1200.0, 800.0);
        let id = |v: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(v))
                .unwrap()
        };
        let w = |v: &str| map[&id(v)].width;
        use crate::Dim;
        assert_eq!(
            w("a"),
            Dim::Px(300.0),
            "a bare `&` must select the parent rule's subject"
        );
        assert_eq!(
            w("h"),
            Dim::Px(260.0),
            "`&:not(.x)` is a compound on the parent's subject, not a selector for the root"
        );
        assert_eq!(
            w("c3"),
            Dim::Px(240.0),
            "`& > span` must resolve `&` to `.p`, so the child combinator has a real left-hand side"
        );
        // The two halves of the descendant case. `inside` proves the rule APPLIES (and, because
        // `.leak{width:100px}` follows it in source order, that `&` contributed `#other`'s
        // specificity — an unresolved `&` loses this tie and reads 100).
        assert_eq!(
            w("inside"),
            Dim::Px(500.0),
            "`& .leak` must match inside #other AND carry #other's specificity"
        );
        // …and `outside` proves it does NOT leak, which the `<html>`-matching accident did.
        assert_eq!(
            w("outside"),
            Dim::Px(100.0),
            "`& .leak` must NOT match a .leak outside #other — the root accident made every nested \
             descendant rule document-wide"
        );
    }

    /// **An author `padding: 0` on a table cell survives the presentational hints.** The UA default
    /// (1px) belongs to the UA-origin sheet, where a reset outranks it; it must NOT be re-applied
    /// afterwards on a `padding == 0` test, because 0 is `padding`'s *initial value* — so that test
    /// cannot distinguish "the author reset it" from "nobody set it", and it answered the wrong one.
    /// Every `* { padding: 0 }` reset (Tailwind preflight, Normalize, every hand-rolled reset) got
    /// its table cells silently re-padded: 2px too wide, 2px too tall, and every row below shifted.
    ///
    /// Both halves are asserted, because the fix is only correct if the DEFAULT still arrives:
    /// a bare `<td>` keeps 1px, and a reset `<td>` gets 0. Chromium, live, on the same markup:
    /// reset cell `43×20`, default cell `45×22`.
    ///
    /// RED, run: restore `if matches!(tag, "td"|"th") && s.padding == Sides::all(Dim::Px(0.0)) {
    /// s.padding = Sides::all(Dim::Px(1.0)) }` in `apply_presentational_hints` — the three reset
    /// assertions read `Px(1.0)`. Delete `td, th { padding: 1px }` from `UA_CSS` and the default
    /// assertion reads `Px(0.0)` instead.
    #[test]
    fn an_author_padding_reset_on_a_table_cell_is_not_undone_by_the_ua_default() {
        let dom = manuk_html::parse(
            r#"<table><tr>
                 <td id="bare">d</td>
                 <td id="reset">r</td>
                 <td id="inline" style="padding:0">i</td>
                 <th id="star">s</th>
               </tr></table>"#,
        );
        // Three shapes of the same author intent: the weak-selector reset that a UA-origin sheet
        // must lose to, an id rule, and an inline style.
        let sheet = Stylesheet::parse("#reset { padding: 0 } * { padding: 0 } th { padding: 0 }");
        let map = cascade_via_stylo(&dom, std::slice::from_ref(&sheet), 800.0, 600.0);
        let id = |v: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(v))
                .unwrap()
        };
        use crate::Dim;
        for who in ["reset", "inline", "star"] {
            assert_eq!(
                map[&id(who)].padding,
                crate::Sides::all(Dim::Px(0.0)),
                "author `padding:0` must reach the cell ({who})"
            );
        }
        // …and the UA default must still be delivered, by the sheet, to a cell nobody styled.
        // (`* { padding: 0 }` above is specificity 0,0,0 in the AUTHOR origin, so it still wins
        // over the UA sheet here — that is why `bare` is asserted from a second cascade.)
        let plain = Stylesheet::parse("");
        let map2 = cascade_via_stylo(&dom, std::slice::from_ref(&plain), 800.0, 600.0);
        assert_eq!(
            map2[&id("bare")].padding,
            crate::Sides::all(Dim::Px(1.0)),
            "an unstyled cell still gets the UA 1px from UA_CSS"
        );
    }

    /// `text-indent` reaches the SHIPPING cascade: Stylo computes it and `stylo_map` consumes the
    /// `.length` into `text_indent`. Without the map arm the field stays 0, so first-line
    /// indentation and the image-replacement idiom (`text-indent:-9999px`) both silently no-op.
    ///
    /// RED, run: delete the `s.text_indent = lp_to_dim(...)` line in `stylo_map.rs`. Both the 40px
    /// and the −9999px assertions read `Dim::Px(0.0)`.
    #[test]
    fn text_indent_maps_through_the_stylo_cascade() {
        let dom = manuk_html::parse(
            r#"<p id="a" style="text-indent:40px">x</p><p id="b" style="text-indent:-9999px">x</p>"#,
        );
        let sheet = Stylesheet::parse("");
        let map = cascade_via_stylo(&dom, std::slice::from_ref(&sheet), 800.0, 600.0);
        let id = |v: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(v))
                .unwrap()
        };
        use crate::Dim;
        assert_eq!(map[&id("a")].text_indent, Dim::Px(40.0), "40px indent maps");
        assert_eq!(
            map[&id("b")].text_indent,
            Dim::Px(-9999.0),
            "the image-replacement −9999px indent maps"
        );
    }

    /// `-webkit-line-clamp` reaches the SHIPPING cascade via the MinimalCascade recovery merge —
    /// stylo 0.19 gates the property to `engine="gecko"`, so the servo build never parses it and the
    /// field would stay `None` (every line of a clamped card/excerpt shown) without the recovery line.
    ///
    /// RED, run: delete `cs.line_clamp = m.line_clamp;` in the merge loop. The assertion reads `None`.
    #[test]
    fn line_clamp_recovers_through_the_stylo_cascade() {
        let dom = manuk_html::parse(
            r#"<div id="a" style="-webkit-line-clamp:3;overflow:hidden">x</div><div id="b">y</div>"#,
        );
        let sheet = Stylesheet::parse("");
        let map = cascade_via_stylo(&dom, std::slice::from_ref(&sheet), 800.0, 600.0);
        let id = |v: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(v))
                .unwrap()
        };
        assert_eq!(map[&id("a")].line_clamp, Some(3), "line-clamp:3 recovers");
        assert_eq!(
            map[&id("b")].line_clamp,
            None,
            "unset stays None (not inherited)"
        );
    }

    /// **`display: -webkit-box` reaches the SHIPPING cascade** — and it is the half that switches on
    /// the `-webkit-line-clamp` recovery directly above it. Both keywords are
    /// `#[cfg(feature = "gecko")]` in stylo 0.19's display parser, so the servo build rejects the
    /// declaration outright and a clamped `<span>` stays `inline`; the clamp only ever fires on a
    /// block, so the card-excerpt idiom showed every one of its lines.
    ///
    /// Measured vs live Chromium (200px card, `font:16px/20px sans-serif`, the SPAN's box):
    ///
    /// | markup | Chrome | was |
    /// |---|---|---|
    /// | `-webkit-box` + `-webkit-line-clamp:2` | `200×40` (computes `flow-root`) | `195×57` ❌ |
    /// | `-webkit-box` alone | `200×60` (computes `-webkit-box`) | `182×57` ❌ |
    /// | `-webkit-inline-box` | `108×20`, shrink-to-fit | `108×17` ❌ |
    ///
    /// All three are Chrome-exact after the fix. On `momon-ga.com` (48 hits of
    /// `display: -webkit-box → inline` in the mechanism oracle) shape went 0.509 → 0.565.
    ///
    /// RED, run: delete the `m.legacy_webkit_box` recovery in the merge loop — `a` reads `Inline`.
    ///
    /// ⚠ The last assertion is the load-bearing one: the recovery must copy the MARKER, not the
    /// MinimalCascade's `display`, or the shipping path silently adopts the weaker cascade's opinion
    /// on every element that has one (the two-cascades trap). A later `display` declaration that wins
    /// the cascade clears the marker and must survive.
    #[test]
    fn webkit_box_display_recovers_through_the_stylo_cascade() {
        let dom = manuk_html::parse(
            r#"<span id="a" style="display:-webkit-box">x</span>
               <span id="b" style="display:-webkit-inline-box">y</span>
               <span id="c" style="display:-webkit-box;display:flex">z</span>
               <span id="d">w</span>"#,
        );
        let sheet = Stylesheet::parse("");
        let map = cascade_via_stylo(&dom, std::slice::from_ref(&sheet), 800.0, 600.0);
        let id = |v: &str| {
            dom.descendants(dom.root())
                .find(|&n| dom.element(n).and_then(|e| e.attr("id")) == Some(v))
                .unwrap()
        };
        assert_eq!(
            map[&id("a")].display,
            crate::Display::Block,
            "-webkit-box is BLOCK-level — the clamp only runs on a block"
        );
        assert_eq!(
            map[&id("b")].display,
            crate::Display::InlineBlock,
            "-webkit-inline-box is inline-level and shrink-to-fit"
        );
        assert_eq!(
            map[&id("c")].display,
            crate::Display::Flex,
            "a later display declaration WINS — the recovery must not resurrect the legacy value"
        );
        assert_eq!(
            map[&id("d")].display,
            crate::Display::Inline,
            "an untouched span is unaffected (the recovery is not a blanket display copy)"
        );
    }

    /// `content: attr(name)` in a `::before`/`::after` resolves to the element's live attribute
    /// value — the whole of CSS tooltips (`[data-tip]::after`), print link URLs (`a::after{
    /// content:" ("attr(href)")"}`), breadcrumb separators and generated data labels. Before this
    /// the extraction loop kept only `String` items and dropped `Attr`, so every such pseudo drew
    /// an EMPTY box — present in the tree, invisible on the page.
    ///
    /// RED, run: revert the `ContentItem::Attr` arm. `after` reads `" ()"` (parentheses, no href)
    /// and `before` reads `""` — both assertions fail, which is exactly the silent blank the fix
    /// removes. A missing attribute yields the empty string, never a dropped pseudo (CSS2.1).
    #[test]
    fn content_attr_resolves_the_elements_attribute() {
        let dom = manuk_html::parse(r#"<a href="/x" data-tip="hi">link</a>"#);
        let sheet = Stylesheet::parse(
            r#"a::after{content:" ("attr(href)")"} a::before{content:attr(data-tip)}"#,
        );
        let map = cascade_via_stylo(&dom, std::slice::from_ref(&sheet), 800.0, 600.0);
        let a = dom
            .descendants(dom.root())
            .find(|&n| dom.tag_name(n) == Some("a"))
            .unwrap();
        let s = &map[&a];
        assert_eq!(
            s.after.as_ref().and_then(|p| p.content.as_deref()),
            Some(" (/x)"),
            "content:attr(href) with surrounding strings must resolve the href"
        );
        assert_eq!(
            s.before.as_ref().and_then(|p| p.content.as_deref()),
            Some("hi"),
            "content:attr(data-tip) must resolve the data attribute"
        );
    }
    use crate::Rgba;

    /// End-to-end: Stylo parses + matches + cascades a real author sheet over the arena
    /// DOM, and the ComputedValues map back onto our style — including inheritance and
    /// the `var()` custom-property resolution the minimal engine can't do.
    #[test]
    fn stylo_cascade_matches_and_inherits() {
        // <body><p class="lead">hi<em>x</em></p></body>
        let mut dom = Dom::new();
        let body = dom.create_element("body");
        let p = dom.create_element("p");
        dom.set_attr(p, "class", "lead");
        let em = dom.create_element("em");
        dom.set_attr(em, "style", "color: rgb(0, 128, 0)");
        dom.append_child(dom.root(), body);
        dom.append_child(body, p);
        dom.append_child(p, em);

        // A class selector sets color via a custom property; children inherit it.
        let sheet = Stylesheet::parse(
            ":root { --brand: rgb(10, 20, 30); }              .lead { color: var(--brand); font-weight: 700; width: 200px; margin-top: 10px;                      display: block; padding: 4px; }",
        );
        let map = cascade_via_stylo(&dom, std::slice::from_ref(&sheet), 800.0, 600.0);

        let ps = &map[&p];
        assert_eq!(
            ps.color,
            Rgba::new(10, 20, 30, 255),
            "var() resolved on .lead"
        );
        assert_eq!(ps.font_weight, 700, "author weight applied");
        assert_eq!(
            ps.width,
            crate::Dim::Px(200.0),
            "width mapped through the cascade"
        );
        assert_eq!(ps.margin.top, crate::Dim::Px(10.0), "margin-top mapped");
        assert_eq!(
            ps.padding.left,
            crate::Dim::Px(4.0),
            "padding shorthand mapped"
        );
        assert_eq!(ps.display, crate::Display::Block, "display mapped");
        // UA defaults flow through Stylo: <body> is block even with no author rule; the
        // inline <em> stays inline (CSS initial).
        assert_eq!(
            map[&body].display,
            crate::Display::Block,
            "UA default: body is block"
        );
        assert_eq!(map[&em].display, crate::Display::Inline, "em stays inline");
        // Both color and font-weight are inherited CSS properties, so <em> gets them
        // from .lead even though no rule targets <em> directly.
        let ems = &map[&em];
        // Inline style on <em> overrides the inherited color; weight still inherits.
        assert_eq!(
            ems.color,
            Rgba::new(0, 128, 0, 255),
            "inline style= overrides inherited color"
        );
        assert_eq!(ems.font_weight, 700, "font-weight inherited by <em>");
    }

    /// W3 regression. `@supports` is how the modern web does progressive enhancement: hide a
    /// legacy fallback, then reveal the real layout inside `@supports (display:grid)`. Skipping the
    /// block meant we silently rendered the FALLBACK of every such site — Wikipedia hides its whole
    /// TOC sidebar with `display:none` and re-shows it inside `@supports (display:grid)`, so the
    /// sidebar simply never appeared.
    #[test]
    fn supports_block_rules_apply_when_the_feature_is_supported() {
        let mut dom = Dom::new();
        let body = dom.create_element("body");
        let side = dom.create_element("div");
        dom.set_attr(side, "class", "sidebar");
        dom.append_child(dom.root(), body);
        dom.append_child(body, side);

        // The exact pattern Wikipedia uses.
        let sheet = Stylesheet::parse(
            ".sidebar { display: none; }              @supports (display: grid) { .sidebar { display: block; width: 200px; } }",
        );
        let map = cascade_via_stylo(&dom, std::slice::from_ref(&sheet), 1200.0, 800.0);
        assert_eq!(
            map[&side].display,
            crate::Display::Block,
            "the @supports block must apply — grid IS supported, so the sidebar is shown, not hidden"
        );
        assert_eq!(map[&side].width, crate::Dim::Px(200.0));
    }

    /// Responsive `@media`: a media block's rules apply only when its query matches the current
    /// viewport (evaluated against the real width the render path threads in).
    #[test]
    fn media_query_applies_by_viewport_width() {
        // <body><div class="box"></div></body>
        let mut dom = Dom::new();
        let body = dom.create_element("body");
        let bx = dom.create_element("div");
        dom.set_attr(bx, "class", "box");
        dom.append_child(dom.root(), body);
        dom.append_child(body, bx);

        let sheet = Stylesheet::parse(
            ".box { display: block; width: 500px; } \
             @media (max-width: 600px) { .box { display: none; width: 100px; } } \
             @media (min-width: 1000px) { .box { width: 900px; } }",
        );

        // Narrow (400px): the max-width:600 block matches → display:none, width:100. The
        // min-width:1000 block does NOT match.
        let narrow = cascade_via_stylo(&dom, std::slice::from_ref(&sheet), 400.0, 800.0);
        assert_eq!(
            narrow[&bx].display,
            crate::Display::None,
            "@media(max-width:600) applies at 400px"
        );
        assert_eq!(narrow[&bx].width, crate::Dim::Px(100.0));

        // Mid (800px): neither media block matches → base rule only.
        let mid = cascade_via_stylo(&dom, std::slice::from_ref(&sheet), 800.0, 800.0);
        assert_eq!(
            mid[&bx].display,
            crate::Display::Block,
            "no @media matches at 800px"
        );
        assert_eq!(mid[&bx].width, crate::Dim::Px(500.0));

        // Wide (1200px): the min-width:1000 block matches → width:900 (later rule wins over base).
        let wide = cascade_via_stylo(&dom, std::slice::from_ref(&sheet), 1200.0, 800.0);
        assert_eq!(
            wide[&bx].display,
            crate::Display::Block,
            "base display at 1200px"
        );
        assert_eq!(
            wide[&bx].width,
            crate::Dim::Px(900.0),
            "@media(min-width:1000) applies at 1200px"
        );
    }

    /// `@container`: a container-gated rule applies only on the SIZED re-pass, and only when the
    /// nearest ancestor container's laid-out inline size crosses the condition. The unsized pass
    /// (every first cascade — no layout has run) must hold container rules off entirely: an
    /// engine that guessed would style feature-detecting fallback pages wrong both ways.
    #[test]
    fn container_query_applies_by_container_size() {
        // <body><div id=outer><div id=inner></div></div></body>
        let mut dom = Dom::new();
        let body = dom.create_element("body");
        let outer = dom.create_element("div");
        dom.set_attr(outer, "id", "outer");
        let inner = dom.create_element("div");
        dom.set_attr(inner, "id", "inner");
        dom.append_child(dom.root(), body);
        dom.append_child(body, outer);
        dom.append_child(outer, inner);

        let sheet = Stylesheet::parse(
            "#outer { container-type: inline-size; } \
             #inner { width: 50px; } \
             @container (min-width: 400px) { #inner { width: 300px; } }",
        );

        // Unsized pass: the @container rule is held off — base width only.
        let first = cascade_via_stylo(&dom, std::slice::from_ref(&sheet), 800.0, 600.0);
        assert_eq!(
            first[&inner].width,
            crate::Dim::Px(50.0),
            "@container held off on the unsized pass"
        );

        // Sized re-pass, container content-box 500px: min-width:400 crosses → rule applies.
        let mut sizes = std::collections::HashMap::new();
        sizes.insert(outer, (500.0, 40.0));
        let wide = cascade_via_stylo_sized(
            &dom,
            std::slice::from_ref(&sheet),
            800.0,
            600.0,
            Some(sizes),
        );
        assert_eq!(
            wide[&inner].width,
            crate::Dim::Px(300.0),
            "@container(min-width:400) applies when the container is 500px"
        );

        // Sized re-pass, container 300px: condition fails → base rule stays.
        let mut sizes = std::collections::HashMap::new();
        sizes.insert(outer, (300.0, 40.0));
        let narrow = cascade_via_stylo_sized(
            &dom,
            std::slice::from_ref(&sheet),
            800.0,
            600.0,
            Some(sizes),
        );
        assert_eq!(
            narrow[&inner].width,
            crate::Dim::Px(50.0),
            "@container(min-width:400) does not apply when the container is 300px"
        );
    }
}
