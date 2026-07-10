//! D2 **Step 0 probe** — prove that real Stylo (the servo-flavored `style` crate)
//! can be *driven* here: build a `Device`, parse a real author stylesheet with
//! Stylo's own parser, feed it to a `Stylist`, flush it into `CascadeData`, and read
//! back how many selectors/declarations Stylo compiled.
//!
//! What this de-risks: the *non-DOM half* of D2 (parser + `Device` + `Stylist` +
//! cascade-data build) works in this workspace with only `url`/`euclid` added. What
//! it deliberately does **not** attempt: the `TElement`/`TNode`/`selectors::Element`
//! trait wall (measured at 126+ methods) that Stylo needs to *match* selectors
//! against our arena DOM — that is the large, multi-session part, and this probe
//! exists to size the decision before committing to it.
//!
//! Modification boundary: Stylo is the sanctioned CSS-engine reuse target (CLAUDE.md
//! § CSS). This is ordinary library use — no engine internals are patched.

use std::fmt;

use euclid::{Scale, Size2D};
use stylo::context::QuirksMode;
use stylo::device::servo::FontMetricsProvider;
use stylo::device::Device;
use stylo::font_metrics::FontMetrics;
use stylo::media_queries::{MediaList, MediaType};
use stylo::properties::style_structs::Font;
use stylo::properties::ComputedValues;
use stylo::queries::values::PrefersColorScheme;
use stylo::servo_arc::Arc as ServoArc;
use stylo::shared_lock::{SharedRwLock, StylesheetGuards};
use stylo::stylesheets::{AllowImportRules, DocumentStyleSheet, Origin, Stylesheet, UrlExtraData};
use stylo::stylist::Stylist;
use stylo::values::computed::font::GenericFontFamily;
use stylo::values::computed::{CSSPixelLength, Length};

/// What Stylo compiled from the probe stylesheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeResult {
    /// Selectors Stylo's `Stylist` compiled into its cascade data.
    pub num_selectors: usize,
    /// Declarations across those rules.
    pub num_declarations: usize,
}

impl fmt::Display for ProbeResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "stylo compiled {} selectors / {} declarations",
            self.num_selectors, self.num_declarations
        )
    }
}

/// A no-op font-metrics provider (2-method trait): enough for `Device` construction
/// in the probe; real metrics come from `manuk-text` when D2 is fully wired.
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

/// Build a viewport `Device` (servo flavor) at `width`×`height` CSS px.
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

/// Parse `css` with Stylo and compile it through a `Stylist`, returning the
/// selector/declaration counts Stylo built. Errors as a `String` if any stage fails.
pub fn probe(css: &str) -> Result<ProbeResult, String> {
    let lock = SharedRwLock::new();
    let url = ::url::Url::parse("about:manuk-probe").map_err(|e| e.to_string())?;
    let url_data = UrlExtraData(ServoArc::new(url));
    let media = ServoArc::new(lock.wrap(MediaList::empty()));

    let sheet = Stylesheet::from_str(
        css,
        url_data,
        Origin::Author,
        media,
        lock.clone(),
        None,
        None,
        QuirksMode::NoQuirks,
        AllowImportRules::Yes,
    );

    let device = make_device(1024.0, 768.0);
    let mut stylist = Stylist::new(device, QuirksMode::NoQuirks);

    let guard = lock.read();
    stylist.append_stylesheet(DocumentStyleSheet(ServoArc::new(sheet)), &guard);
    stylist.flush(&StylesheetGuards::same(&guard));

    Ok(ProbeResult {
        num_selectors: stylist.num_selectors(),
        num_declarations: stylist.num_declarations(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stylo_compiles_a_real_stylesheet() {
        // Three rules with distinct selectors; Stylo should report ≥3 selectors.
        let css = "p { color: red } .lead { font-weight: bold } #x a { margin: 4px }";
        let r = probe(css).expect("stylo probe");
        eprintln!("D2 Step-0 probe: {r}");
        assert!(
            r.num_selectors >= 3,
            "expected ≥3 selectors compiled, got {r}"
        );
        assert!(r.num_declarations >= 3, "expected ≥3 declarations, got {r}");
    }
}
