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

/// A `LengthPercentage` reduced to our `Dim`. Sampling the used value at two bases makes
/// this work for the mixed `calc()` case too: at basis 0 the result is the pure length
/// part, and the slope to basis 100px is the percentage fraction — so `calc(50% - 10px)`
/// maps to `Dim::Calc { px: -10, pct: 50 }`. Percentages are stored 0–100 in `Dim`.
fn lp_to_dim(lp: &LengthPercentage) -> Dim {
    use app_units::Au;
    let at = |b: f32| lp.to_used_value(Au::from_f32_px(b)).to_f32_px();
    // Sample at two *large* bases: `to_used_value` applies Stylo's non-negative clamping
    // for widths/paddings, which would corrupt the decomposition near basis 0 (a value like
    // `calc(100% - 40px)` clamps to 0 there). At 1000/2000px the true linear form shows.
    let (b1, b2) = (1000.0_f32, 2000.0_f32);
    let (v1, v2) = (at(b1), at(b2));
    let frac = (v2 - v1) / (b2 - b1);
    let px = v1 - frac * b1;
    let pct = frac * 100.0;
    if pct.abs() < 1e-3 {
        Dim::Px(px)
    } else if px.abs() < 1e-3 {
        Dim::Percent(pct)
    } else {
        Dim::Calc { px, pct }
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

    // Position mode — drives whether the insets below are actually applied by layout.
    use stylo::values::computed::{
        Clear as SClear, Float as SFloat, Overflow as SOverflow, PositionProperty, ZIndex,
    };
    s.position = match cv.clone_position() {
        PositionProperty::Relative => crate::Position::Relative,
        PositionProperty::Absolute => crate::Position::Absolute,
        PositionProperty::Fixed => crate::Position::Fixed,
        PositionProperty::Sticky => crate::Position::Sticky,
        PositionProperty::Static => crate::Position::Static,
    };
    s.float = match cv.clone_float() {
        SFloat::Left | SFloat::InlineStart => crate::Float::Left,
        SFloat::Right | SFloat::InlineEnd => crate::Float::Right,
        SFloat::None => crate::Float::None,
    };
    s.clear = match cv.clone_clear() {
        SClear::Left | SClear::InlineStart => crate::Clear::Left,
        SClear::Right | SClear::InlineEnd => crate::Clear::Right,
        SClear::Both => crate::Clear::Both,
        SClear::None => crate::Clear::None,
    };
    // `overflow`: our model keeps one axis (the more-clipping of x/y).
    let map_overflow = |o: SOverflow| match o {
        SOverflow::Hidden => crate::Overflow::Hidden,
        SOverflow::Scroll => crate::Overflow::Scroll,
        SOverflow::Auto => crate::Overflow::Auto,
        SOverflow::Clip => crate::Overflow::Clip,
        SOverflow::Visible => crate::Overflow::Visible,
    };
    let (ox, oy) = (map_overflow(cv.clone_overflow_x()), map_overflow(cv.clone_overflow_y()));
    s.overflow = if ox != crate::Overflow::Visible { ox } else { oy };
    s.z_index = match cv.clone_z_index() {
        ZIndex::Integer(i) => Some(i),
        ZIndex::Auto => None,
    };
    // Flex container + item properties. Stylo's alignment values are `AlignFlags` bitflags
    // (value in the low bits, `safe`/`unsafe`/`legacy` in the high bits) — mask to the value.
    {
        use stylo::values::specified::align::AlignFlags;
        let av = |f: AlignFlags| f.bits() & 0b0001_1111;
        use stylo::properties::longhands::{flex_direction, flex_wrap};
        s.flex_direction = match cv.clone_flex_direction() {
            flex_direction::computed_value::T::RowReverse => crate::FlexDirection::RowReverse,
            flex_direction::computed_value::T::Column => crate::FlexDirection::Column,
            flex_direction::computed_value::T::ColumnReverse => crate::FlexDirection::ColumnReverse,
            flex_direction::computed_value::T::Row => crate::FlexDirection::Row,
        };
        s.flex_wrap = match cv.clone_flex_wrap() {
            flex_wrap::computed_value::T::Wrap => crate::FlexWrap::Wrap,
            flex_wrap::computed_value::T::WrapReverse => crate::FlexWrap::WrapReverse,
            flex_wrap::computed_value::T::Nowrap => crate::FlexWrap::NoWrap,
        };
        s.flex_grow = cv.clone_flex_grow().0;
        s.flex_shrink = cv.clone_flex_shrink().0;
        s.flex_basis = match cv.clone_flex_basis() {
            stylo::values::computed::FlexBasis::Size(sz) => size_to_dim(&sz),
            _ => Dim::Auto,
        };
        s.justify_content = match av(cv.clone_justify_content().primary()) {
            5 | 3 => crate::JustifyContent::FlexEnd,
            6 => crate::JustifyContent::Center,
            14 => crate::JustifyContent::SpaceBetween,
            15 => crate::JustifyContent::SpaceAround,
            16 => crate::JustifyContent::SpaceEvenly,
            _ => crate::JustifyContent::FlexStart,
        };
        let map_ai = |v: u8| match v {
            5 | 3 | 13 => crate::AlignItems::FlexEnd,
            6 => crate::AlignItems::Center,
            9 | 10 => crate::AlignItems::Baseline,
            4 | 2 | 12 => crate::AlignItems::FlexStart,
            _ => crate::AlignItems::Stretch,
        };
        s.align_items = map_ai(av(cv.clone_align_items().0));
        s.align_self = match av(cv.clone_align_self().0) {
            0 => None,
            v => Some(map_ai(v)),
        };
        // row-gap / column-gap: `normal` → 0, else the length part.
        use stylo::values::generics::length::GenericLengthPercentageOrNormal as GapVal;
        let gap_px = |g: stylo::values::computed::length::NonNegativeLengthPercentageOrNormal| match g {
            GapVal::Normal => 0.0,
            GapVal::LengthPercentage(lp) => match lp_to_dim(&lp.0) {
                Dim::Px(p) => p,
                _ => 0.0,
            },
        };
        s.row_gap = gap_px(cv.clone_row_gap());
        s.column_gap = gap_px(cv.clone_column_gap());
    }

    // box-sizing.
    s.box_sizing = match cv.clone_box_sizing() {
        stylo::properties::longhands::box_sizing::computed_value::T::BorderBox => {
            crate::BoxSizing::BorderBox
        }
        _ => crate::BoxSizing::ContentBox,
    };

    // white-space (0.19 shorthand: text-wrap-mode + white-space-collapse).
    {
        use stylo::properties::longhands::{text_wrap_mode, white_space_collapse};
        let collapse = cv.clone_white_space_collapse();
        let wrap = cv.clone_text_wrap_mode();
        s.white_space = if collapse == white_space_collapse::computed_value::T::Preserve {
            crate::WhiteSpace::Pre
        } else if wrap == text_wrap_mode::computed_value::T::Nowrap {
            crate::WhiteSpace::NoWrap
        } else {
            crate::WhiteSpace::Normal
        };
    }

    // vertical-align: not exposed as a computed longhand accessor in this Stylo 0.19 build
    // (only appears in the shorthand table), so it stays at the initial `baseline`. TODO if
    // the accessor becomes available. (Affects the `valign` parity page: 2 probes.)

    // table-layout / border-collapse / border-spacing.
    s.table_layout = match cv.clone_table_layout() {
        stylo::properties::longhands::table_layout::computed_value::T::Fixed => {
            crate::TableLayout::Fixed
        }
        _ => crate::TableLayout::Auto,
    };
    s.border_collapse =
        cv.clone_border_collapse() == stylo::properties::longhands::border_collapse::computed_value::T::Collapse;
    s.border_spacing = cv.clone_border_spacing().horizontal().to_f32_px();

    // transform: map the 2D operations onto our affine list (3D/perspective skipped — our
    // paint model is 2D). Angles are taken in radians; translate lengths keep %/calc via `Dim`.
    {
        use stylo::values::computed::TransformOperation as TOp;
        let mut ops = Vec::new();
        for op in cv.clone_transform().0.iter() {
            match op {
                TOp::Translate(x, y) => {
                    ops.push(crate::TransformFn::Translate(lp_to_dim(x), lp_to_dim(y)))
                }
                TOp::TranslateX(x) => {
                    ops.push(crate::TransformFn::Translate(lp_to_dim(x), Dim::Px(0.0)))
                }
                TOp::TranslateY(y) => {
                    ops.push(crate::TransformFn::Translate(Dim::Px(0.0), lp_to_dim(y)))
                }
                TOp::Scale(x, y) => ops.push(crate::TransformFn::Scale(*x, *y)),
                TOp::ScaleX(x) => ops.push(crate::TransformFn::Scale(*x, 1.0)),
                TOp::ScaleY(y) => ops.push(crate::TransformFn::Scale(1.0, *y)),
                TOp::Rotate(a) | TOp::RotateZ(a) => {
                    ops.push(crate::TransformFn::Rotate(a.radians()))
                }
                TOp::Skew(ax, ay) => {
                    ops.push(crate::TransformFn::Skew(ax.radians(), ay.radians()))
                }
                TOp::SkewX(ax) => ops.push(crate::TransformFn::Skew(ax.radians(), 0.0)),
                TOp::SkewY(ay) => ops.push(crate::TransformFn::Skew(0.0, ay.radians())),
                TOp::Matrix(m) => {
                    ops.push(crate::TransformFn::Matrix([m.a, m.b, m.c, m.d, m.e, m.f]))
                }
                _ => {}
            }
        }
        s.transform = ops;
    }

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
