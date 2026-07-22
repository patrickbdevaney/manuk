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

    // Parse each sheet's raw source with Stylo's own parser; keep the Arcs so we can
    // iterate their compiled rules for matching.
    let mut stylo_sheets: Vec<ServoArc<StyloStylesheet>> = Vec::new();
    let mut stylist = Stylist::new(make_device(vw, vh, qm), qm);
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
        stylist.flush(&StylesheetGuards::same(&guard));
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
            cq_active,
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
                if s.width == crate::Dim::Auto && !s.width_stretch {
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
                // A UA intrinsic width is a DEFAULT, so an author declaration outranks it — and
                // `width: stretch` is a declaration that merely *looks* absent (`Dim::Auto`). Same
                // guard as the dimension attributes below: without it a `width:stretch` text field
                // stays 173px wide instead of filling its form row.
                if s.width == crate::Dim::Auto && !s.width_stretch {
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
    if tag == "textarea" && s.width == crate::Dim::Auto && !s.width_stretch {
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
        };
        let mut order = 0usize;
        let mut cq_stack: Vec<ServoArc<Vec<ContainerCondition>>> = Vec::new();
        for sheet in sheets {
            let rules = sheet.contents.read_with(guard).rules(guard);
            idx.add_rules(rules, guard, device, &mut order, qm, &mut cq_stack);
        }
        idx
    }

    fn add_rules(
        &mut self,
        rules: &[CssRule],
        guard: &SharedRwLockReadGuard<'_>,
        device: &Device,
        order: &mut usize,
        qm: QuirksMode,
        cq_stack: &mut Vec<ServoArc<Vec<ContainerCondition>>>,
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
                            spec: sel.specificity(),
                            order: *order,
                            block: sr.block.clone(),
                            cq: cq_stack.clone(),
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
                        self.add_rules(&nested.0, guard, device, order, qm, cq_stack);
                    }
                }
                CssRule::Media(media_rule) => {
                    let ml = media_rule.media_queries.read_with(guard);
                    let mut custom = CustomMediaEvaluator::none();
                    if ml.evaluate(device, qm, &mut custom) {
                        let nested = media_rule.rules.read_with(guard);
                        self.add_rules(&nested.0, guard, device, order, qm, cq_stack);
                    }
                }
                CssRule::Supports(supports_rule) => {
                    if supports_rule.enabled {
                        let nested = supports_rule.rules.read_with(guard);
                        self.add_rules(&nested.0, guard, device, order, qm, cq_stack);
                    }
                }
                CssRule::LayerBlock(layer) => {
                    let nested = layer.rules.read_with(guard);
                    self.add_rules(&nested.0, guard, device, order, qm, cq_stack);
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
                self.add_rules(rules, guard, device, &mut order, qm, &mut cq_stack);
            }
        }
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
    cq_active: bool,
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
        .any(|rule| matches!(rule, CssRule::Supports(s) if s.enabled))
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // Real properties this engine does not implement — the ones whose false "yes" made pages
        // discard a working fallback.
        assert!(!supports_condition("view-transition-name: foo"));
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
