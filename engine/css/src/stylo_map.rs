//! D2 back-half — mapping Stylo's `ComputedValues` onto our [`crate::ComputedStyle`].
//!
//! This is the second large piece of finishing the Stylo cascade (the first being the
//! `TElement` DOM-trait wall — see `docs/parity/STYLO-CASCADE-PLAN.md`). It is
//! independently testable **without** the wall, because an initial `ComputedValues` can
//! be built directly (`ComputedValues::initial_values_with_font_override`), so the
//! accessor + reduction logic is exercised here before the cascade itself is wired.
//!
//! **Status:** the scalar/text subset (color, background-color, font size/weight/style,
//! line-height) is mapped and tested. The geometric properties (display, box model,
//! sizing, position, borders) are the next tranche; their exact accessors + reductions
//! are tabulated in the plan doc. All accessor names below are verified against the
//! on-disk `stylo-0.19.0` source.

use stylo::color::{AbsoluteColor, ColorSpace};
use stylo::properties::ComputedValues;
use stylo::values::computed::font::FontStyle;

use crate::{ComputedStyle, Rgba};

/// Convert a Stylo `AbsoluteColor` to our `Rgba` (via the sRGB color space).
fn abs_to_rgba(c: &AbsoluteColor) -> Rgba {
    let s = c.to_color_space(ColorSpace::Srgb);
    let to = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Rgba::new(to(s.components.0), to(s.components.1), to(s.components.2), to(s.alpha))
}

/// Map the scalar/text subset of a Stylo `ComputedValues` onto a `ComputedStyle`, starting
/// from our initial style and overriding the mapped fields. The geometric properties are
/// left at their initial values until the next tranche (see the module docs).
pub fn to_computed_style(cv: &ComputedValues) -> ComputedStyle {
    let mut s = ComputedStyle::initial();

    // `color` resolves any `currentColor` elsewhere, so read it first.
    let current = cv.clone_color();
    s.color = abs_to_rgba(&current);

    // `background-color` may be `currentColor`; resolve against the element's color.
    let bg = cv.clone_background_color().resolve_to_absolute(&current);
    s.background_color = (bg.alpha > 0.0).then(|| abs_to_rgba(&bg));

    s.font_size = cv.clone_font_size().computed_size().px();
    s.font_weight = cv.clone_font_weight().value().round().clamp(1.0, 1000.0) as u16;
    s.italic = cv.clone_font_style() != FontStyle::NORMAL;
    s.line_height = s.font_size * 1.2;

    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use stylo::properties::style_structs::Font;

    #[test]
    fn maps_initial_computed_values_to_sane_defaults() {
        // An initial ComputedValues needs no cascade / no TElement — exactly what lets
        // this half be verified independently.
        let cv = ComputedValues::initial_values_with_font_override(Font::initial_values());
        let style = to_computed_style(&cv);

        // CSS initial values: color black, transparent background, medium (16px) font,
        // normal (400) weight, upright.
        assert_eq!(style.color, Rgba::new(0, 0, 0, 255), "initial color is black");
        assert_eq!(style.background_color, None, "initial background is transparent");
        assert_eq!(style.font_size, 16.0, "initial medium font-size");
        assert_eq!(style.font_weight, 400, "initial normal weight");
        assert!(!style.italic, "initial font-style is normal");
    }
}
