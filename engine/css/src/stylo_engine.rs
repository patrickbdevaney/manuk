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
use stylo::properties::style_structs::Font;
use stylo::properties::{ComputedValues, PropertyDeclarationBlock};
use stylo::queries::values::PrefersColorScheme;
use stylo::servo_arc::Arc as ServoArc;
use stylo::shared_lock::{SharedRwLock, SharedRwLockReadGuard, StylesheetGuards};
use stylo::stylesheets::{
    AllowImportRules, CssRule, DocumentStyleSheet, Origin, Stylesheet as StyloStylesheet,
    UrlExtraData,
};
use stylo::stylist::Stylist;
use stylo::values::computed::font::GenericFontFamily;
use stylo::values::computed::{CSSPixelLength, Length};

use manuk_dom::{Dom, NodeId};

use crate::stylo_dom::{ElementDataStore, StyloElement};
use crate::stylo_map::to_computed_style;
use crate::{MinimalCascade, StyleEngine, StyleMap, Stylesheet};

/// Stylo cascade adapter.
///
/// [`Self::cascade`] still delegates to [`MinimalCascade`] so the default styling
/// (UA-default display, inline `style=`, etc.) is applied while the Stylo path is
/// completed. [`cascade_via_stylo`] is the real Stylo value cascade over author sheets —
/// wall + matching + `compute_for_declarations` + `ComputedValues` mapping, end to end —
/// and is exercised by the tests. The remaining wiring (a UA stylesheet + inline-style
/// blocks fed through Stylo, then swapping the delegation) is tracked in
/// `docs/parity/STYLO-CASCADE-PLAN.md`.
#[derive(Debug, Default, Clone, Copy)]
pub struct StyloEngine;

impl StyleEngine for StyloEngine {
    fn cascade(&self, dom: &Dom, sheets: &[Stylesheet]) -> StyleMap {
        MinimalCascade.cascade(dom, sheets)
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

/// The real Stylo value cascade over `sheets`' author rules: build a `Stylist`, and for
/// each element match rules with Stylo's selector matcher (via our `selectors::Element`),
/// merge the winning declarations, compute `ComputedValues` with `compute_for_declarations`
/// (no `TElement` instance — `element = None`), and map the result onto our
/// [`ComputedStyle`], inheriting from each element's parent. This is what gives real
/// `var()` / `@media` / full-selector / `font-family` computation.
pub fn cascade_via_stylo(dom: &Dom, sheets: &[Stylesheet], vw: f32, vh: f32) -> StyleMap {
    let lock = SharedRwLock::new();
    let Ok(url) = ::url::Url::parse("about:manuk") else {
        return MinimalCascade.cascade(dom, sheets);
    };
    let url_data = UrlExtraData(ServoArc::new(url));

    // Parse each sheet's raw source with Stylo's own parser; keep the Arcs so we can
    // iterate their compiled rules for matching.
    let mut stylo_sheets: Vec<ServoArc<StyloStylesheet>> = Vec::new();
    let mut stylist = Stylist::new(make_device(vw, vh), QuirksMode::NoQuirks);
    {
        let guard = lock.read();
        for sheet in sheets {
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

    let store = ElementDataStore::new();
    let guard = lock.read();
    let guards = StylesheetGuards::same(&guard);

    let mut map: StyleMap = StyleMap::new();
    // Preorder walk so a parent's ComputedValues exists before its children's cascade.
    let mut parent_cv: std::collections::HashMap<NodeId, ServoArc<ComputedValues>> =
        std::collections::HashMap::new();
    let mut stack: Vec<NodeId> = vec![dom.root()];
    while let Some(node) = stack.pop() {
        // Push children (reverse so we pop them in document order).
        let kids: Vec<NodeId> = dom.children(node).collect();
        for &k in kids.iter().rev() {
            stack.push(k);
        }
        if !dom.is_element(node) {
            continue;
        }
        let el = StyloElement::new(dom, node, &store);
        let cv = cascade_one_element(
            &stylist, &stylo_sheets, &lock, &guard, &guards, &el, node, &parent_cv,
        );
        map.insert(node, to_computed_style(&cv));
        parent_cv.insert(node, cv);
    }
    map
}

/// Compute one element's `ComputedValues`: match author rules, merge, cascade.
#[allow(clippy::too_many_arguments)]
fn cascade_one_element(
    stylist: &Stylist,
    stylo_sheets: &[ServoArc<StyloStylesheet>],
    lock: &SharedRwLock,
    guard: &SharedRwLockReadGuard<'_>,
    guards: &StylesheetGuards<'_>,
    el: &StyloElement<'_>,
    node: NodeId,
    parent_cv: &std::collections::HashMap<NodeId, ServoArc<ComputedValues>>,
) -> ServoArc<ComputedValues> {
    // Gather matching (specificity, order, block) across all sheets, document order.
    let mut winners: Vec<(u32, usize, ServoArc<stylo::shared_lock::Locked<PropertyDeclarationBlock>>)> =
        Vec::new();
    let mut order = 0usize;
    let mut caches = SelectorCaches::default();
    for sheet in stylo_sheets {
        for rule in sheet.contents.read_with(guard).rules(guard).iter() {
            if let CssRule::Style(style_rule) = rule {
                let sr = style_rule.read_with(guard);
                for sel in sr.selectors.slice() {
                    let mut ctx = MatchingContext::new(
                        MatchingMode::Normal,
                        None,
                        &mut caches,
                        selectors::context::QuirksMode::NoQuirks,
                        NeedsSelectorFlags::No,
                        MatchingForInvalidation::No,
                    );
                    if matches_selector(sel, 0, None, el, &mut ctx) {
                        winners.push((sel.specificity(), order, sr.block.clone()));
                    }
                    order += 1;
                }
            }
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
        dom.append_child(dom.root(), body);
        dom.append_child(body, p);
        dom.append_child(p, em);

        // A class selector sets color via a custom property; children inherit it.
        let sheet = Stylesheet::parse(
            ":root { --brand: rgb(10, 20, 30); }              .lead { color: var(--brand); font-weight: 700; width: 200px; margin-top: 10px;                      display: block; padding: 4px; }",
        );
        let map = cascade_via_stylo(&dom, std::slice::from_ref(&sheet), 800.0, 600.0);

        let ps = &map[&p];
        assert_eq!(ps.color, Rgba::new(10, 20, 30, 255), "var() resolved on .lead");
        assert_eq!(ps.font_weight, 700, "author weight applied");
        assert_eq!(ps.width, crate::Dim::Px(200.0), "width mapped through the cascade");
        assert_eq!(ps.margin.top, crate::Dim::Px(10.0), "margin-top mapped");
        assert_eq!(ps.padding.left, crate::Dim::Px(4.0), "padding shorthand mapped");
        assert_eq!(ps.display, crate::Display::Block, "display mapped");
        // Both color and font-weight are inherited CSS properties, so <em> gets them
        // from .lead even though no rule targets <em> directly.
        let ems = &map[&em];
        assert_eq!(ems.color, Rgba::new(10, 20, 30, 255), "color inherited by <em>");
        assert_eq!(ems.font_weight, 700, "font-weight inherited by <em>");
    }
}
