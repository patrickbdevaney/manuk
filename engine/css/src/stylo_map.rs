//! D2 back-half — mapping Stylo's `ComputedValues` onto our [`crate::ComputedStyle`].
//!
//! Independently testable **without** the `TElement` wall, because an initial
//! `ComputedValues` can be built directly (`ComputedValues::initial_values_with_font_override`),
//! so the accessor + reduction logic is exercised before the cascade itself is wired.
//! Accessor names are verified against the on-disk `stylo-0.19.0` source; the property list
//! + reductions follow `docs/parity/STYLO-CASCADE-PLAN.md`.

use stylo::color::{AbsoluteColor, ColorSpace};
use stylo::properties::ComputedValues;
use stylo::values::computed::font::FontStyle;
use stylo::values::computed::length::{Margin, MaxSize, Size};
use stylo::values::computed::position::Inset;
use stylo::values::computed::{Display as StyloDisplay, LengthPercentage, TextAlign as StyloTextAlign};

use crate::{ComputedStyle, Dim, Display, Rgba, Sides, TextAlign};

/// Convert a Stylo `AbsoluteColor` to our `Rgba` (via the sRGB color space).
fn abs_to_rgba(c: &AbsoluteColor) -> Rgba {
    let s = c.to_color_space(ColorSpace::Srgb);
    let to = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Rgba::new(to(s.components.0), to(s.components.1), to(s.components.2), to(s.alpha))
}

/// A `LengthPercentage` reduced to our `Dim` (pure length → px, pure percentage → %,
/// mixed calc → its length part; percentages are stored 0–100 in `Dim`).
fn lp_to_dim(lp: &LengthPercentage) -> Dim {
    if let Some(l) = lp.to_length() {
        Dim::Px(l.px())
    } else if let Some(p) = lp.to_percentage() {
        Dim::Percent(p.0 * 100.0)
    } else {
        Dim::Px(0.0)
    }
}

/// `width`/`height` `Size` → `Dim` (content-keywords and `auto` collapse to `Dim::Auto`).
fn size_to_dim(s: &Size) -> Dim {
    match s {
        Size::LengthPercentage(nn) => lp_to_dim(&nn.0),
        _ => Dim::Auto,
    }
}

/// `max-width`/`max-height` `MaxSize` → `Dim` (`none`/keywords → `Dim::Auto` = no limit).
fn maxsize_to_dim(s: &MaxSize) -> Dim {
    match s {
        MaxSize::LengthPercentage(nn) => lp_to_dim(&nn.0),
        _ => Dim::Auto,
    }
}

/// `margin` (`GenericMargin`) → `Dim` (anchor functions → `Dim::Auto`).
fn margin_to_dim(m: &Margin) -> Dim {
    match m {
        Margin::LengthPercentage(lp) => lp_to_dim(lp),
        _ => Dim::Auto,
    }
}

/// `top`/`right`/`bottom`/`left` `Inset` → `Dim` (anchor functions → `Dim::Auto`).
fn inset_to_dim(i: &Inset) -> Dim {
    match i {
        Inset::LengthPercentage(lp) => lp_to_dim(lp),
        Inset::Auto => Dim::Auto,
        _ => Dim::Auto,
    }
}

fn map_display(d: StyloDisplay) -> Display {
    // Servo's computed Display exposes its keywords as associated consts.
    if d == StyloDisplay::None {
        Display::None
    } else if d == StyloDisplay::Block {
        Display::Block
    } else if d == StyloDisplay::Inline {
        Display::Inline
    } else if d == StyloDisplay::InlineBlock {
        Display::InlineBlock
    } else if d == StyloDisplay::Flex || d == StyloDisplay::InlineFlex {
        Display::Flex
    } else if d == StyloDisplay::Grid || d == StyloDisplay::InlineGrid {
        Display::Grid
    } else if d == StyloDisplay::Table || d == StyloDisplay::InlineTable {
        Display::Table
    } else if d == StyloDisplay::TableRowGroup {
        Display::TableRowGroup
    } else if d == StyloDisplay::TableRow {
        Display::TableRow
    } else if d == StyloDisplay::TableCell {
        Display::TableCell
    } else if d == StyloDisplay::TableCaption {
        Display::TableCaption
    } else {
        Display::Inline
    }
}

fn map_text_align(t: StyloTextAlign) -> TextAlign {
    match t {
        StyloTextAlign::Right | StyloTextAlign::End | StyloTextAlign::MozRight => TextAlign::Right,
        StyloTextAlign::Center | StyloTextAlign::MozCenter => TextAlign::Center,
        StyloTextAlign::Justify => TextAlign::Justify,
        // Start/Left/End-in-LTR/MozLeft → left.
        _ => TextAlign::Left,
    }
}

/// Map a Stylo `ComputedValues` onto our `ComputedStyle`, starting from initial and
/// overriding every property we model.
pub fn to_computed_style(cv: &ComputedValues) -> ComputedStyle {
    let mut s = ComputedStyle::initial();

    // Color / background (currentColor resolved against the element's color).
    let current = cv.clone_color();
    s.color = abs_to_rgba(&current);
    let bg = cv.clone_background_color().resolve_to_absolute(&current);
    s.background_color = (bg.alpha > 0.0).then(|| abs_to_rgba(&bg));

    // Font / text.
    s.font_size = cv.clone_font_size().computed_size().px();
    s.font_weight = cv.clone_font_weight().value().round().clamp(1.0, 1000.0) as u16;
    s.italic = cv.clone_font_style() != FontStyle::NORMAL;
    s.text_align = map_text_align(cv.clone_text_align());

    // Display.
    s.display = map_display(cv.clone_display());

    // Box model — sizing.
    s.width = size_to_dim(&cv.clone_width());
    s.height = size_to_dim(&cv.clone_height());
    s.min_width = size_to_dim(&cv.clone_min_width());
    s.min_height = size_to_dim(&cv.clone_min_height());
    s.max_width = maxsize_to_dim(&cv.clone_max_width());
    s.max_height = maxsize_to_dim(&cv.clone_max_height());

    // Margin / padding.
    s.margin = Sides {
        top: margin_to_dim(&cv.clone_margin_top()),
        right: margin_to_dim(&cv.clone_margin_right()),
        bottom: margin_to_dim(&cv.clone_margin_bottom()),
        left: margin_to_dim(&cv.clone_margin_left()),
    };
    s.padding = Sides {
        top: lp_to_dim(&cv.clone_padding_top().0),
        right: lp_to_dim(&cv.clone_padding_right().0),
        bottom: lp_to_dim(&cv.clone_padding_bottom().0),
        left: lp_to_dim(&cv.clone_padding_left().0),
    };

    // Borders (widths + a single color taken from the top edge, matching our model).
    // Stylo zeroes a border-width for `none`/`hidden` only at *resolved*-value time, so the
    // computed width is still `medium` (3px). Replicate that zeroing here or every block
    // paints a spurious 3px border.
    s.border_width = Sides {
        top: if cv.clone_border_top_style().none_or_hidden() { 0.0 } else { cv.clone_border_top_width().0.to_f32_px() },
        right: if cv.clone_border_right_style().none_or_hidden() { 0.0 } else { cv.clone_border_right_width().0.to_f32_px() },
        bottom: if cv.clone_border_bottom_style().none_or_hidden() { 0.0 } else { cv.clone_border_bottom_width().0.to_f32_px() },
        left: if cv.clone_border_left_style().none_or_hidden() { 0.0 } else { cv.clone_border_left_width().0.to_f32_px() },
    };
    s.border_color = abs_to_rgba(&cv.clone_border_top_color().resolve_to_absolute(&current));

    // Insets.
    s.inset.top = inset_to_dim(&cv.clone_top());
    s.inset.right = inset_to_dim(&cv.clone_right());
    s.inset.bottom = inset_to_dim(&cv.clone_bottom());
    s.inset.left = inset_to_dim(&cv.clone_left());

    // line-height: a fixed 1.2×font-size approximation (Stylo's `normal` needs font
    // metrics we stub); explicit lengths/numbers are honoured.
    s.line_height = match cv.clone_line_height() {
        stylo::values::computed::font::LineHeight::Length(l) => l.px(),
        stylo::values::computed::font::LineHeight::Number(n) => s.font_size * n.0,
        _ => s.font_size * 1.2,
    };

    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use stylo::properties::style_structs::Font;

    #[test]
    fn maps_initial_computed_values_to_sane_defaults() {
        let cv = ComputedValues::initial_values_with_font_override(Font::initial_values());
        let style = to_computed_style(&cv);

        assert_eq!(style.color, Rgba::new(0, 0, 0, 255), "initial color is black");
        assert_eq!(style.background_color, None, "initial background is transparent");
        assert_eq!(style.font_size, 16.0, "initial medium font-size");
        assert_eq!(style.font_weight, 400, "initial normal weight");
        assert!(!style.italic, "initial font-style is normal");
        assert_eq!(style.display, Display::Inline, "initial display is inline");
        assert_eq!(style.width, Dim::Auto, "initial width is auto");
        assert_eq!(style.margin.top, Dim::Px(0.0), "initial margin is 0");
    }
}
