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
use stylo::stylesheets::{
    AllowImportRules, CssRule, CssRuleType, CustomMediaEvaluator, DocumentStyleSheet, Origin,
    Stylesheet as StyloStylesheet, UrlExtraData,
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

/// A no-op font-metrics provider — enough to build a `Device`.
#[derive(Debug)]
struct StubFontMetrics;

impl FontMetricsProvider for StubFontMetrics {
    fn query_font_metrics(
        &self,
        _vertical: bool,
        _font: &Font,
        _base_size: CSSPixelLength,
        _flags: stylo::values::specified::font::QueryFontMetricsFlags,
    ) -> FontMetrics {
        FontMetrics::default()
    }
    fn base_size_for_generic(&self, _generic: GenericFontFamily) -> Length {
        Length::new(16.0)
    }
}

fn make_device(width: f32, height: f32) -> Device {
    Device::new(
        MediaType::screen(),
        QuirksMode::NoQuirks,
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
form, fieldset, table, caption, center { display: block; }
center { text-align: center; }
/* The elements that are never rendered. Ours was missing the *media* half of the list, and
   `<source>` is the one that matters: `<picture><source>` is how the entire modern web serves
   responsive images, and every one of them was getting a real box with real height. Wikipedia alone
   invented 152px out of eight of them, in the middle of the article. Same shape as the `<script>`
   that painted its own source code down rust-lang.org — a metadata element with no `display:none`
   becomes content. Mirrors Chrome's html.css. */
head, title, meta, link, script, style, base, noscript, template,
source, track, param, area, datalist, basefont, noembed, noframes, rp { display: none; }
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
input[type=hidden] { display: none; }
p, blockquote { margin: 1em 0; }
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
table { display: table; }
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
    // Stylo's `grid_enabled()` reads `layout.grid.enabled` (off by default under the `servo`
    // feature), which makes it drop `display:grid` at parse time. Flip it on once so grid
    // containers cascade. Idempotent + cheap; safe to call every cascade.
    stylo_static_prefs::set_pref!("layout.grid.enabled", true);
    let lock = SharedRwLock::new();
    let Ok(url) = ::url::Url::parse("about:manuk") else {
        return MinimalCascade.cascade(dom, sheets);
    };
    let url_data = UrlExtraData(ServoArc::new(url));

    // Parse each sheet's raw source with Stylo's own parser; keep the Arcs so we can
    // iterate their compiled rules for matching.
    let mut stylo_sheets: Vec<ServoArc<StyloStylesheet>> = Vec::new();
    let mut stylist = Stylist::new(make_device(vw, vh), QuirksMode::NoQuirks);
    // The UA sheet is matched first (lowest priority); author rules override it.
    let ua_sheet = Stylesheet::parse(UA_CSS);
    let all_sheets: Vec<&Stylesheet> = std::iter::once(&ua_sheet).chain(sheets.iter()).collect();
    {
        let guard = lock.read();
        for sheet in &all_sheets {
            let media = ServoArc::new(lock.wrap(MediaList::empty()));
            let parsed = StyloStylesheet::from_str(
                sheet.source(),
                url_data.clone(),
                Origin::Author,
                media,
                lock.clone(),
                None,
                None,
                QuirksMode::NoQuirks,
                AllowImportRules::Yes,
            );
            let arc = ServoArc::new(parsed);
            stylist.append_stylesheet(DocumentStyleSheet(arc.clone()), &guard);
            stylo_sheets.push(arc);
        }
        stylist.flush(&StylesheetGuards::same(&guard));
    }

    CASCADES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let store = ElementDataStore::new();
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
    let index = RuleIndex::build(&stylo_sheets, &guard, stylist.device());
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
    let mut candidates: Vec<u32> = Vec::new();
    let mut caches = SelectorCaches::default();

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
        let cv = cascade_one_element(
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
        );
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
        }
        let mut cs = to_computed_style(&cv);
        apply_presentational_hints(dom, node, &mut cs);
        // `::before` / `::after` — generated content, cascaded against this element as its parent.
        use stylo::selector_parser::PseudoElement as Pe;
        cs.before = cascade_pseudo(
            &stylist,
            &stylo_sheets,
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
            &stylo_sheets,
            &lock,
            &guard,
            &guards,
            &el,
            &cv,
            Pe::After,
        )
        .map(Box::new);
        map.insert(node, cs);
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
            for sh in &has_sheets {
                applied += sh.apply_has_rules(dom, node, cs, parent_fs);
            }
        }
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
    let minimal = MinimalCascade.cascade(dom, sheets);
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
            cs.background_image = m.background_image.clone();
            cs.background_size = m.background_size;
            // `object-fit` recovered from MinimalCascade like the rest of this block, so the shipping
            // Stylo path renders it too: a card grid's `object-fit:cover` thumbnails must not distort.
            cs.object_fit = m.object_fit;
            // `text-transform` recovered from MinimalCascade (inherited there) so the shipping path
            // renders uppercase nav/buttons — Stylo's servo build models it as a bitflags type.
            cs.text_transform = m.text_transform;
            // `overflow-wrap`/`word-wrap` and `word-break` recovered from MinimalCascade so the
            // shipping path also breaks long unbreakable tokens (a URL in a narrow column) instead
            // of letting them overflow. Stylo's servo build models these as keyword enums we don't
            // consume directly.
            cs.overflow_wrap = m.overflow_wrap;
            cs.word_break = m.word_break;
            cs.background_repeat = m.background_repeat;
            cs.text_decoration = m.text_decoration;
            cs.list_style_type = m.list_style_type;
            cs.list_style_inside = m.list_style_inside;
        }
    }

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
/// doesn't synthesize them): replaced-element `width`/`height` attributes and `<td>`/`<th>`
/// default padding. Applied only where the property is still at its initial, so real author
/// CSS wins (presentational hints are lower priority than author rules).
fn apply_presentational_hints(dom: &Dom, node: NodeId, s: &mut crate::ComputedStyle) {
    let Some(el) = dom.element(node) else {
        return;
    };
    let tag = dom.tag_name(node).unwrap_or("");
    if matches!(tag, "td" | "th") && s.padding == crate::Sides::all(crate::Dim::Px(0.0)) {
        s.padding = crate::Sides::all(crate::Dim::Px(1.0));
    }
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
                if s.width == crate::Dim::Auto {
                    s.width = crate::Dim::Px(13.0);
                }
                if s.height == crate::Dim::Auto {
                    s.height = crate::Dim::Px(13.0);
                }
            }
            "hidden" | "submit" | "reset" | "button" | "image" | "file" | "range" | "color" => {}
            // Text-like: `size` characters wide. The 8px-per-character figure is the average
            // advance of the default UI font at 16px — the same approximation Chrome's own default
            // ends up at (`size=20` → ~173px).
            _ => {
                if s.width == crate::Dim::Auto {
                    let cols = el
                        .attr("size")
                        .and_then(|v| v.trim().parse::<f32>().ok())
                        .filter(|n| *n > 0.0)
                        .unwrap_or(20.0);
                    s.width = crate::Dim::Px(cols * 8.0 + 13.0);
                }
            }
        }
    }
    if tag == "textarea" && s.width == crate::Dim::Auto {
        let cols = el
            .attr("cols")
            .and_then(|v| v.trim().parse::<f32>().ok())
            .filter(|n| *n > 0.0)
            .unwrap_or(20.0);
        s.width = crate::Dim::Px(cols * 8.0 + 13.0);
    }
    if matches!(
        tag,
        "img" | "canvas" | "video" | "svg" | "object" | "embed" | "iframe"
    ) {
        if s.display == crate::Display::Inline {
            s.display = crate::Display::InlineBlock;
        }
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
struct RuleIndex {
    rules: Vec<IndexedRule>,
    by_id: std::collections::HashMap<String, Vec<u32>>,
    by_class: std::collections::HashMap<String, Vec<u32>>,
    by_tag: std::collections::HashMap<String, Vec<u32>>,
    universal: Vec<u32>,
}

struct IndexedRule {
    sel: selectors::parser::Selector<stylo::selector_parser::SelectorImpl>,
    spec: u32,
    order: usize,
    block: ServoArc<stylo::shared_lock::Locked<PropertyDeclarationBlock>>,
}

impl RuleIndex {
    fn build(
        sheets: &[ServoArc<StyloStylesheet>],
        guard: &SharedRwLockReadGuard<'_>,
        device: &Device,
    ) -> Self {
        let mut idx = RuleIndex {
            rules: Vec::new(),
            by_id: Default::default(),
            by_class: Default::default(),
            by_tag: Default::default(),
            universal: Vec::new(),
        };
        let mut order = 0usize;
        for sheet in sheets {
            let rules = sheet.contents.read_with(guard).rules(guard);
            idx.add_rules(rules, guard, device, &mut order);
        }
        idx
    }

    fn add_rules(
        &mut self,
        rules: &[CssRule],
        guard: &SharedRwLockReadGuard<'_>,
        device: &Device,
        order: &mut usize,
    ) {
        use selectors::parser::Component;
        for rule in rules {
            match rule {
                CssRule::Style(style_rule) => {
                    let sr = style_rule.read_with(guard);
                    for sel in sr.selectors.slice() {
                        // The rightmost compound is the one that must match THIS element; anything
                        // to its left is an ancestor/sibling constraint checked afterwards.
                        let mut key: Option<(u8, String)> = None;
                        for comp in sel.iter() {
                            let cand = match comp {
                                Component::ID(v) => Some((0u8, v.to_string())),
                                Component::Class(v) => Some((1u8, v.to_string())),
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
                            spec: sel.specificity(),
                            order: *order,
                            block: sr.block.clone(),
                        });
                        match key {
                            Some((0, v)) => self.by_id.entry(v).or_default().push(i),
                            Some((1, v)) => self.by_class.entry(v).or_default().push(i),
                            Some((2, v)) => self.by_tag.entry(v).or_default().push(i),
                            // `*`, `:hover`, `[attr]` and friends have no cheap key: they must be
                            // tried against everything, which is correct and is what `SelectorMap`
                            // does too.
                            _ => self.universal.push(i),
                        }
                        *order += 1;
                    }

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
                        self.add_rules(&nested.0, guard, device, order);
                    }
                }
                CssRule::Media(media_rule) => {
                    let ml = media_rule.media_queries.read_with(guard);
                    let mut custom = CustomMediaEvaluator::none();
                    if ml.evaluate(device, QuirksMode::NoQuirks, &mut custom) {
                        let nested = media_rule.rules.read_with(guard);
                        self.add_rules(&nested.0, guard, device, order);
                    }
                }
                CssRule::Supports(supports_rule) => {
                    if supports_rule.enabled {
                        let nested = supports_rule.rules.read_with(guard);
                        self.add_rules(&nested.0, guard, device, order);
                    }
                }
                CssRule::LayerBlock(layer) => {
                    let nested = layer.rules.read_with(guard);
                    self.add_rules(&nested.0, guard, device, order);
                }
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
            if let Some(id) = e.attr("id") {
                if let Some(v) = self.by_id.get(id) {
                    out.extend_from_slice(v);
                }
            }
            for c in e.classes() {
                if let Some(v) = self.by_class.get(c) {
                    out.extend_from_slice(v);
                }
            }
        }
        // Source order, so the `(specificity, order)` sort downstream is stable and correct.
        out.sort_unstable();
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
                        selectors::context::QuirksMode::NoQuirks,
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
                if ml.evaluate(device, QuirksMode::NoQuirks, &mut custom) {
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
                if supports_rule.enabled {
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
    stylo_sheets: &[ServoArc<StyloStylesheet>],
    lock: &SharedRwLock,
    guard: &SharedRwLockReadGuard<'_>,
    guards: &StylesheetGuards<'_>,
    el: &StyloElement<'_>,
    parent_cv: &ServoArc<ComputedValues>,
    want: stylo::selector_parser::PseudoElement,
) -> Option<crate::ComputedStyle> {
    let mut winners: Vec<(
        u32,
        usize,
        ServoArc<stylo::shared_lock::Locked<PropertyDeclarationBlock>>,
    )> = Vec::new();
    let mut order = 0usize;
    let mut caches = SelectorCaches::default();
    let device = stylist.device();
    for sheet in stylo_sheets {
        let rules = sheet.contents.read_with(guard).rules(guard);
        match_pseudo_rules(
            rules,
            guard,
            device,
            el,
            &want,
            &mut caches,
            &mut winners,
            &mut order,
        );
    }
    if winners.is_empty() {
        return None;
    }
    winners.sort_by_key(|(spec, ord, _)| (*spec, *ord));
    let mut merged = PropertyDeclarationBlock::new();
    for (_, _, block) in &winners {
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
                if let ContentItem::String(sv) = it {
                    out.push_str(sv);
                }
            }
            out
        }
        _ => return None,
    };
    cs.content = Some(text);
    Some(cs)
}

/// Like [`match_rules_recursive`], but matches only selectors whose rightmost part is the wanted
/// **pseudo-element**, in `ForStatelessPseudoElement` mode.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn match_pseudo_rules(
    rules: &[CssRule],
    guard: &SharedRwLockReadGuard<'_>,
    device: &Device,
    el: &StyloElement<'_>,
    want: &stylo::selector_parser::PseudoElement,
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
                    if sel.pseudo_element() != Some(want) {
                        *order += 1;
                        continue;
                    }
                    let mut ctx = MatchingContext::new(
                        MatchingMode::ForStatelessPseudoElement,
                        None,
                        caches,
                        selectors::context::QuirksMode::NoQuirks,
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
                if ml.evaluate(device, QuirksMode::NoQuirks, &mut custom) {
                    let nested = media_rule.rules.read_with(guard);
                    match_pseudo_rules(&nested.0, guard, device, el, want, caches, winners, order);
                }
            }
            CssRule::Supports(supports_rule) => {
                if supports_rule.enabled {
                    let nested = supports_rule.rules.read_with(guard);
                    match_pseudo_rules(&nested.0, guard, device, el, want, caches, winners, order);
                }
            }
            CssRule::LayerBlock(layer_rule) => {
                let nested = layer_rule.rules.read_with(guard);
                match_pseudo_rules(&nested.0, guard, device, el, want, caches, winners, order);
            }
            _ => {}
        }
    }
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
) -> ServoArc<ComputedValues> {
    // Only the rules this element could possibly match — see `RuleIndex`. Everything below is
    // unchanged: each candidate is still fully matched by `matches_selector`, and winners are still
    // ordered by (specificity, source order).
    let mut winners: Vec<(
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
        selectors::context::QuirksMode::NoQuirks,
        NeedsSelectorFlags::No,
        MatchingForInvalidation::No,
    );
    for &i in candidates.iter() {
        let r = &index.rules[i as usize];
        if matches_selector(&r.sel, 0, None, el, &mut ctx) {
            winners.push((r.spec, r.order, r.block.clone()));
        }
    }
    winners.sort_by_key(|(spec, ord, _)| (*spec, *ord));

    // Merge winning declarations (ascending priority: later overrides earlier).
    let mut merged = PropertyDeclarationBlock::new();
    for (_, _, block) in &winners {
        for (decl, importance) in block.read_with(guard).declaration_importance_iter() {
            merged.push(decl.clone(), importance);
        }
    }
    // Inline `style=` wins over all matched rules — append its declarations last.
    if let Some(inline) = el.dom.element(node).and_then(|e| e.attr("style")) {
        let block = parse_style_attribute(
            inline,
            url_data,
            None,
            selectors::context::QuirksMode::NoQuirks,
            CssRuleType::Style,
        );
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

#[cfg(test)]
mod tests {
    use super::*;
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
}
