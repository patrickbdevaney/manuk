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
use stylo::values::computed::{
    Display as StyloDisplay, LengthPercentage, TextAlign as StyloTextAlign,
};

use crate::{ComputedStyle, Dim, Display, Rgba, Sides, TextAlign};

/// Convert a Stylo `AbsoluteColor` to our `Rgba` (via the sRGB color space).
fn abs_to_rgba(c: &AbsoluteColor) -> Rgba {
    let s = c.to_color_space(ColorSpace::Srgb);
    let to = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Rgba::new(
        to(s.components.0),
        to(s.components.1),
        to(s.components.2),
        to(s.alpha),
    )
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
    } else if d == StyloDisplay::Flex {
        Display::Flex
    } else if d == StyloDisplay::InlineFlex {
        Display::InlineFlex
    } else if d == StyloDisplay::Grid {
        Display::Grid
    } else if d == StyloDisplay::InlineGrid {
        Display::InlineGrid
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

/// One Stylo grid `<track-breadth>` → our `TrackUnit` (for `minmax()` endpoints).
fn breadth_to_unit(
    b: &stylo::values::generics::grid::GenericTrackBreadth<LengthPercentage>,
) -> crate::TrackUnit {
    use stylo::values::generics::grid::GenericTrackBreadth as TB;
    match b {
        TB::Breadth(lp) => match lp_to_dim(lp) {
            Dim::Px(p) => crate::TrackUnit::Px(p),
            Dim::Percent(p) => crate::TrackUnit::Percent(p),
            _ => crate::TrackUnit::Auto,
        },
        TB::Flex(f) => crate::TrackUnit::Fr(f.0),
        TB::Auto => crate::TrackUnit::Auto,
        TB::MinContent => crate::TrackUnit::MinContent,
        TB::MaxContent => crate::TrackUnit::MaxContent,
    }
}

/// One Stylo `<track-size>` → our `TrackSize`.
fn track_size_to_ours(
    ts: &stylo::values::generics::grid::GenericTrackSize<LengthPercentage>,
) -> crate::TrackSize {
    use stylo::values::generics::grid::{GenericTrackBreadth as TB, GenericTrackSize as TS};
    match ts {
        TS::Breadth(b) => match b {
            TB::Breadth(lp) => match lp_to_dim(lp) {
                Dim::Px(p) => crate::TrackSize::Px(p),
                Dim::Percent(p) => crate::TrackSize::Percent(p),
                _ => crate::TrackSize::Auto,
            },
            TB::Flex(f) => crate::TrackSize::Fr(f.0),
            TB::Auto => crate::TrackSize::Auto,
            TB::MinContent => crate::TrackSize::MinContent,
            TB::MaxContent => crate::TrackSize::MaxContent,
        },
        TS::Minmax(a, b) => crate::TrackSize::MinMax(breadth_to_unit(a), breadth_to_unit(b)),
        TS::FitContent(_) => crate::TrackSize::Auto,
    }
}

/// A Stylo `grid-template-columns`/`-rows` component → our flat `Vec<TrackSize>`, expanding
/// integer `repeat()`; `none`/subgrid/masonry/auto-repeat collapse to what we can model.
fn template_to_tracks(c: &stylo::values::computed::GridTemplateComponent) -> Vec<crate::TrackSize> {
    use stylo::values::generics::grid::{
        GenericGridTemplateComponent as GC, GenericTrackListValue as TLV, RepeatCount,
    };
    let mut out = Vec::new();
    if let GC::TrackList(list) = c {
        for v in list.values.iter() {
            match v {
                TLV::TrackSize(ts) => out.push(track_size_to_ours(ts)),
                TLV::TrackRepeat(r) => {
                    let n = match r.count {
                        RepeatCount::Number(i) => i.max(0) as usize,
                        _ => 1,
                    };
                    for _ in 0..n {
                        for ts in r.track_sizes.iter() {
                            out.push(track_size_to_ours(ts));
                        }
                    }
                }
            }
        }
    }
    out
}

/// A Stylo computed `<grid-line>` → our `GridLine`.
fn grid_line_to_ours(l: &stylo::values::computed::GridLine) -> crate::GridLine {
    if l.is_span {
        crate::GridLine::Span(l.line_num.max(1) as u16)
    } else if l.line_num != 0 {
        crate::GridLine::Line(l.line_num as i16)
    } else {
        crate::GridLine::Auto
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

    // background-image, text-decoration, list-style and outline are recovered from MinimalCascade
    // in `stylo_engine` (this Stylo build exposes them only as generic image//keyword types whose
    // shape we would have to re-implement anyway). See the recovery loop there.

    // outline: a width and a colour is all a focus ring needs — but the width is only *used* when
    // a style is set. Stylo computes `outline-width: medium` (3px) regardless, and `outline-color`
    // resolves to `currentColor` (opaque black), so taking the width at face value drew a 3px black
    // ring around EVERY element on the page.
    {
        use stylo::values::specified::outline::OutlineStyle;
        let o = cv.get_outline();
        let styled = !matches!(o.clone_outline_style(), OutlineStyle::BorderStyle(bs) if bs.none_or_hidden());
        s.outline_width = if styled {
            o.clone_outline_width().0.to_f32_px()
        } else {
            0.0
        };
        let oc = o.clone_outline_color().resolve_to_absolute(&current);
        if oc.alpha > 0.0 {
            s.outline_color = abs_to_rgba(&oc);
        }
    }

    // Font / text.
    s.font_size = cv.clone_font_size().computed_size().px();
    s.font_weight = cv.clone_font_weight().value().round().clamp(1.0, 1000.0) as u16;
    s.italic = cv.clone_font_style() != FontStyle::NORMAL;

    // **`font-family` — the shipping cascade was not mapping it AT ALL.**
    //
    // Every page on the web therefore rendered in one default sans-serif face, whatever its CSS
    // said: serif prose in sans, code blocks in a proportional font, every `@font-face` webfont
    // ignored. It is the largest text bug there is, and it is the true source of every "font
    // metrics" divergence the oracle reported — we were not mismeasuring the font, we were not
    // *using* it.
    //
    // Generic keywords are carried through by name (`serif`, `monospace`, …) so the text layer's
    // own generic resolution applies to them; a named family is carried verbatim, in author order,
    // so the fallback list is honoured rather than flattened to its first entry.
    {
        use stylo::values::computed::font::{GenericFontFamily, SingleFontFamily};
        let ff = cv.clone_font_family();
        let mut names: Vec<String> = Vec::new();
        for f in ff.families.list.iter() {
            match f {
                SingleFontFamily::FamilyName(n) => names.push(n.name.to_string()),
                SingleFontFamily::Generic(g) => names.push(
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
        if !names.is_empty() {
            s.font_family = names;
        }
    }
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
        top: if cv.clone_border_top_style().none_or_hidden() {
            0.0
        } else {
            cv.clone_border_top_width().0.to_f32_px()
        },
        right: if cv.clone_border_right_style().none_or_hidden() {
            0.0
        } else {
            cv.clone_border_right_width().0.to_f32_px()
        },
        bottom: if cv.clone_border_bottom_style().none_or_hidden() {
            0.0
        } else {
            cv.clone_border_bottom_width().0.to_f32_px()
        },
        left: if cv.clone_border_left_style().none_or_hidden() {
            0.0
        } else {
            cv.clone_border_left_width().0.to_f32_px()
        },
    };
    s.border_color = abs_to_rgba(&cv.clone_border_top_color().resolve_to_absolute(&current));

    // `opacity` — own value; the *effective* (subtree-folded) value is computed by the caller.
    // (`visibility` is not exposed by Stylo's servo build, so it is recovered from MinimalCascade
    // in `cascade_via_stylo` — the same pattern already used for `vertical-align`.)
    s.opacity = cv.get_effects().clone_opacity().clamp(0.0, 1.0);

    // **An animated element renders its END state, not its first frame.**
    //
    // We cannot animate. The question is what a *static* renderer should show, and the answer is not
    // "the base rule, literally" — because the single most common animation on the web is a fade-in
    // whose base rule is `opacity: 0` and whose keyframes reveal the element. Render that literally and
    // **the content never appears at all**.
    //
    // Measured: **52 of 237 corpus sites (21%)** pair `opacity: 0` with an animation. That is a fifth of
    // the web with invisible content — and it is the reason this is a correctness fix and not a polish
    // one. `prefers-reduced-motion: reduce` is the same idea, blessed by the spec: show the destination,
    // skip the journey.
    //
    // Scoped deliberately to **opacity**, because opacity is the only one of these that makes content
    // *disappear*. A `transform`-based slide-in still renders — merely offset — and a colour transition
    // still renders a colour. Guessing at the end state of an arbitrary keyframe would be worse than
    // this, and this is already the difference between seeing the page and not.
    // Stylo already answers exactly this question — `specifies_animations()` is
    // `animation_name_iter().any(|n| !n.is_none())`, which is the definition we want and one we should
    // not re-derive (a re-derived constant is how a gate ends up checking its own copy of a number).
    s.has_animation = cv.get_ui().specifies_animations();
    if s.has_animation && s.opacity == 0.0 {
        s.opacity = 1.0;
    }

    // `border-radius` — uniform MVP: the top-left corner's horizontal radius (per-corner and
    // elliptical radii are a follow-on). A `%` radius resolves against the box, which we don't
    // have here, so only a px radius is taken.
    s.border_radius = match lp_to_dim(&cv.clone_border_top_left_radius().0.width.0) {
        crate::Dim::Px(px) => px.max(0.0),
        _ => 0.0,
    };

    // `box-shadow` — the first outer shadow (inset / spread / multiple are follow-ons).
    s.box_shadow = cv
        .clone_box_shadow()
        .0
        .iter()
        .find(|sh| !sh.inset)
        .map(|sh| crate::BoxShadow {
            dx: sh.base.horizontal.px(),
            dy: sh.base.vertical.px(),
            blur: sh.base.blur.0.px().max(0.0),
            color: abs_to_rgba(&sh.base.color.clone().resolve_to_absolute(&current)),
        });

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
    let (ox, oy) = (
        map_overflow(cv.clone_overflow_x()),
        map_overflow(cv.clone_overflow_y()),
    );
    s.overflow = if ox != crate::Overflow::Visible {
        ox
    } else {
        oy
    };
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
        let gap_px =
            |g: stylo::values::computed::length::NonNegativeLengthPercentageOrNormal| match g {
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
        let nowrap = wrap == text_wrap_mode::computed_value::T::Nowrap;
        s.white_space = match collapse {
            // `pre` and `pre-wrap` both preserve newlines; they differ only in whether a long line
            // may still wrap. Collapsing them lost that distinction — and mapping `pre-line` to
            // `normal` lost its newlines entirely.
            white_space_collapse::computed_value::T::Preserve if nowrap => crate::WhiteSpace::Pre,
            white_space_collapse::computed_value::T::Preserve => crate::WhiteSpace::PreWrap,
            white_space_collapse::computed_value::T::PreserveBreaks => crate::WhiteSpace::PreLine,
            _ if nowrap => crate::WhiteSpace::NoWrap,
            _ => crate::WhiteSpace::Normal,
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
    s.border_collapse = cv.clone_border_collapse()
        == stylo::properties::longhands::border_collapse::computed_value::T::Collapse;
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
                TOp::Skew(ax, ay) => ops.push(crate::TransformFn::Skew(ax.radians(), ay.radians())),
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

    // Grid tracks + item placement.
    s.grid_template_columns = template_to_tracks(&cv.clone_grid_template_columns());
    s.grid_template_rows = template_to_tracks(&cv.clone_grid_template_rows());
    s.grid_column = (
        grid_line_to_ours(&cv.clone_grid_column_start()),
        grid_line_to_ours(&cv.clone_grid_column_end()),
    );
    s.grid_row = (
        grid_line_to_ours(&cv.clone_grid_row_start()),
        grid_line_to_ours(&cv.clone_grid_row_end()),
    );

    // grid-template-areas: Stylo pre-resolves the ASCII art to `NamedArea`s with
    // 1-indexed line ranges. Carry them so the item's `grid-area: name` can resolve.
    if let stylo::values::computed::position::GridTemplateAreas::Areas(a) =
        cv.clone_grid_template_areas()
    {
        s.grid_template_areas =
            a.0.areas
                .iter()
                .map(|na| crate::GridAreaRect {
                    name: na.name.to_string(),
                    row: (na.rows.start as u16, na.rows.end as u16),
                    col: (na.columns.start as u16, na.columns.end as u16),
                })
                .collect();
    }
    // Item placement by area name: `grid-area: main` sets all four grid-line idents to
    // "main"; the row-start ident is representative. A bare custom-ident (no line number,
    // no span) is a named-area/named-line reference.
    {
        let rs = cv.clone_grid_row_start();
        let name = rs.ident.0.to_string();
        if !rs.is_span && rs.line_num == 0 && !name.is_empty() {
            s.grid_area = Some(name);
        }
    }

    // Insets.
    s.inset.top = inset_to_dim(&cv.clone_top());
    s.inset.right = inset_to_dim(&cv.clone_right());
    s.inset.bottom = inset_to_dim(&cv.clone_bottom());
    s.inset.left = inset_to_dim(&cv.clone_left());

    // line-height: a fixed 1.2×font-size approximation (Stylo's `normal` needs font
    // metrics we stub); explicit lengths/numbers are honoured.
    s.line_height = match cv.clone_line_height() {
        stylo::values::computed::font::LineHeight::Length(l) => {
            s.line_height_normal = false;
            l.px()
        }
        stylo::values::computed::font::LineHeight::Number(n) => {
            s.line_height_normal = false;
            s.font_size * n.0
        }
        // `normal` — the FONT decides, not a multiplier. Layout substitutes the face's real
        // ascent + descent + lineGap; this value is only a fallback for when no face is available.
        _ => {
            s.line_height_normal = true;
            s.font_size * 1.2
        }
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

        assert_eq!(
            style.color,
            Rgba::new(0, 0, 0, 255),
            "initial color is black"
        );
        assert_eq!(
            style.background_color, None,
            "initial background is transparent"
        );
        assert_eq!(style.font_size, 16.0, "initial medium font-size");
        assert_eq!(style.font_weight, 400, "initial normal weight");
        assert!(!style.italic, "initial font-style is normal");
        assert_eq!(style.display, Display::Inline, "initial display is inline");
        assert_eq!(style.width, Dim::Auto, "initial width is auto");
        assert_eq!(style.margin.top, Dim::Px(0.0), "initial margin is 0");
    }
}
