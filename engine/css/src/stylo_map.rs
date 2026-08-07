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
// `DisplayInside`/`DisplayOutside` live on the SPECIFIED module (computed `Display` is a re-export of
// the specified type), and they are the only route to `flow-root` in the servo build — see `map_display`.
use stylo::values::specified::box_::{
    DisplayInside as StyloDisplayInside, DisplayOutside as StyloDisplayOutside,
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

/// A computed `filter` / `backdrop-filter` function list → ours.
///
/// **One function, two properties, on purpose.** `filter` and `backdrop-filter` take the *same*
/// grammar and differ only in what they are applied to; two copies of this map would be two things
/// that must stay identical forever, which is how a `drop-shadow` gets fixed in one and not the
/// other. `Filter::Url` is `Impossible` in the servo build — the variant cannot be constructed, so
/// there is no arm to write and none to forget.
fn map_filter_list(
    list: &[stylo::values::computed::effects::Filter],
    current: &AbsoluteColor,
) -> Vec<crate::FilterOp> {
    use stylo::values::computed::effects::Filter as SFilter;
    list.iter()
        .filter_map(|f| {
            Some(match f {
                SFilter::Blur(l) => crate::FilterOp::Blur(l.0.px().max(0.0)),
                SFilter::Brightness(n) => crate::FilterOp::Brightness(n.0.max(0.0)),
                SFilter::Contrast(n) => crate::FilterOp::Contrast(n.0.max(0.0)),
                SFilter::Grayscale(n) => crate::FilterOp::Grayscale(n.0.clamp(0.0, 1.0)),
                SFilter::HueRotate(a) => crate::FilterOp::HueRotate(a.degrees()),
                SFilter::Invert(n) => crate::FilterOp::Invert(n.0.clamp(0.0, 1.0)),
                SFilter::Opacity(n) => crate::FilterOp::Opacity(n.0.clamp(0.0, 1.0)),
                SFilter::Saturate(n) => crate::FilterOp::Saturate(n.0.max(0.0)),
                SFilter::Sepia(n) => crate::FilterOp::Sepia(n.0.clamp(0.0, 1.0)),
                SFilter::DropShadow(sh) => crate::FilterOp::DropShadow {
                    dx: sh.horizontal.px(),
                    dy: sh.vertical.px(),
                    blur: sh.blur.0.px().max(0.0),
                    color: abs_to_rgba(&sh.color.clone().resolve_to_absolute(current)),
                },
                _ => return None,
            })
        })
        .collect()
}

/// `width`/`height` `Size` → `Dim` (content-keywords and `auto` collapse to `Dim::Auto`).
fn size_to_dim(s: &Size) -> Dim {
    match s {
        Size::LengthPercentage(nn) => lp_to_dim(&nn.0),
        _ => Dim::Auto,
    }
}

/// `true` when a `Size` is an **intrinsic sizing keyword** (`min-content`/`max-content`/
/// `fit-content` / `fit-content(...)`), which `size_to_dim` collapses to `Dim::Auto` but which are
/// *indefinite* — unlike `auto`, `stretch` or `fill-available`, which are definite. Layout needs the
/// distinction for the abspos both-insets constraint-equation (see `ComputedStyle::height_intrinsic`).
fn size_is_intrinsic(s: &Size) -> bool {
    use stylo::values::generics::length::GenericSize as GS;
    matches!(
        s,
        GS::MinContent | GS::MaxContent | GS::FitContent | GS::FitContentFunction(_)
    )
}

/// `true` when a `Size` is `stretch` / `-webkit-fill-available` / `-moz-available` — a DEFINITE size
/// that FILLS the containing block (unlike `auto` = content, and unlike the intrinsic keywords).
fn size_is_stretch(s: &Size) -> bool {
    use stylo::values::generics::length::GenericSize as GS;
    // crates.io stylo 0.19 models `-moz-available` as `WebkitFillAvailable` (no separate variant).
    matches!(s, GS::Stretch | GS::WebkitFillAvailable)
}

/// Which intrinsic sizing keyword a `Size` carries, if any — for `width`, where the specific keyword
/// (not just "is intrinsic") decides whether the box hugs its longest word, its whole content, or the
/// shrink-to-fit clamp between them. `fit-content(<length>)` is treated as plain `fit-content`.
fn size_intrinsic_kw(s: &Size) -> Option<crate::IntrinsicSize> {
    use crate::IntrinsicSize as IS;
    use stylo::values::generics::length::GenericSize as GS;
    match s {
        GS::MinContent => Some(IS::MinContent),
        GS::MaxContent => Some(IS::MaxContent),
        GS::FitContent | GS::FitContentFunction(_) => Some(IS::FitContent),
        _ => None,
    }
}

/// The [`size_intrinsic_kw`] twin for `max-width`/`max-height`, whose stylo type is a separate
/// `GenericMaxSize` (its "unset" variant is `None`, not `Auto`) and therefore needs its own match.
///
/// ⚠ Without this, `maxsize_to_dim` folded `min-content`/`max-content`/`fit-content` into the same
/// `Dim::Auto` it uses for `none` — i.e. **the cap simply did not apply**, which is how
/// `max-width: min-content` left a box filling its container on the shipping cascade (t930).
/// ⚠ `FitContentFunction` is deliberately absent: `fit-content(<length>)` is **not valid** on the
/// min/max properties (Chrome-measured — it reads back as the initial value), so it must not become
/// a constraint here. Same reason `intrinsic_kw_bare` exists on the minimal-cascade side.
fn maxsize_intrinsic_kw(s: &MaxSize) -> Option<crate::IntrinsicSize> {
    use crate::IntrinsicSize as IS;
    use stylo::values::generics::length::GenericMaxSize as GM;
    match s {
        GM::MinContent => Some(IS::MinContent),
        GM::MaxContent => Some(IS::MaxContent),
        GM::FitContent => Some(IS::FitContent),
        _ => None,
    }
}

/// The `min-width`/`min-height` twin — a `Size` like `width`, but with the same
/// `fit-content(<length>)` exclusion as [`maxsize_intrinsic_kw`], which is why it cannot just be
/// [`size_intrinsic_kw`].
fn minsize_intrinsic_kw(s: &Size) -> Option<crate::IntrinsicSize> {
    use crate::IntrinsicSize as IS;
    use stylo::values::generics::length::GenericSize as GS;
    match s {
        GS::MinContent => Some(IS::MinContent),
        GS::MaxContent => Some(IS::MaxContent),
        GS::FitContent => Some(IS::FitContent),
        _ => None,
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
    } else if d == StyloDisplay::TableHeaderGroup {
        Display::TableHeaderGroup
    } else if d == StyloDisplay::TableFooterGroup {
        Display::TableFooterGroup
    } else if d == StyloDisplay::TableRow {
        Display::TableRow
    } else if d == StyloDisplay::TableCell {
        Display::TableCell
    } else if d == StyloDisplay::TableCaption {
        Display::TableCaption
    } else if d == StyloDisplay::Contents {
        // Stylo parses `display: contents` perfectly well. **We threw it away here** — the catch-all
        // below turned it into `Inline`, which is the worst available answer: the wrapper stays in the
        // box tree as a real inline box that DOES participate in layout, so its children stop being the
        // grandparent's grid/flex items. The grid then sees one anonymous inline child instead of three,
        // and the layout silently collapses into a single cell with every element still present and
        // still styled. `display: none` would at least have been visibly wrong.
        Display::Contents
    } else if d.outside() == StyloDisplayOutside::Block
        && d.inside() == StyloDisplayInside::FlowRoot
    {
        // `display: flow-root` — a block box that **establishes a block formatting context**, which
        // is the modern, explicit way to say *"contain my floats"* without the side effects of
        // `overflow: hidden` (which also clips) or a `::after` clearfix. Stylo parses it; we ate it
        // here, so the element became INLINE: measured [0 0 0x19] against Chrome's [0 0 1200x70].
        //
        // ⚠ Read through `outside()`/`inside()` rather than `StyloDisplay::FlowRoot`, because that
        //   const is `#[cfg(feature = "gecko")]` and we build Stylo's *servo* configuration. The
        //   **parser is not gated** — `"flow-root" => Inside(DisplayInside::FlowRoot)` is in the
        //   servo build too — so the value arrives correctly and only the convenience constant is
        //   missing. This is a build-flag gap read around with public API, not a fork: nothing is
        //   patched, and a Stylo bump cannot silently revert it (it would fail to compile).
        Display::FlowRoot
    } else if d == StyloDisplay::TableColumn {
        Display::TableColumn
    } else if d == StyloDisplay::TableColumnGroup {
        Display::TableColumnGroup
    } else if d.is_list_item() {
        // `list-item` is a MODIFIER BIT in Stylo (`LIST_ITEM_MASK`), not a distinct display value, so
        // it never matched any const above and fell through to the catch-all. Every `<li>` the author
        // re-declares — and every custom `display: list-item` list on the web — became INLINE.
        // Block-level is the correct outer display; the marker is generated elsewhere.
        Display::Block
    } else {
        // ⚠⚠ **THIS CATCH-ALL IS A BUG FACTORY, AND THIS IS THE THIRD TIME IT HAS BEEN CAUGHT.**
        //    It silently answers `Inline` for any keyword nobody mapped — which is the *worst*
        //    available answer, because an inline box still participates in layout and so the failure
        //    looks like a subtle geometry bug rather than an unsupported value. `display: contents`
        //    was the first (the comment above it is the post-mortem), and a 23-keyword sweep against
        //    Chrome found four more sitting here: `flow-root`, `list-item`, `table-column` and
        //    `table-column-group` — the last two having had variants in our own enum the whole time.
        //
        //    Still unmapped, deliberately and named rather than discovered later: `ruby` (Chrome:
        //    `ruby`) and MathML's `math` (Chrome: `inline`, which this happens to get right). Both
        //    are on the declared post-Phase-0 list; the point of naming them here is that the next
        //    person reading this knows the remainder is two values, not "everything else".
        Display::Inline
    }
}

fn map_text_align(t: StyloTextAlign) -> TextAlign {
    match t {
        // `start`/`end` are LOGICAL and stay logical here: direction is not yet known at map time
        // (it is recovered from MinimalCascade after this runs), so the cascade resolves them to a
        // physical value once it has the direction. Mapping `End` straight to `Right` — as this did —
        // right-aligned an RTL paragraph's *end* (which is the LEFT) and left-aligned its default
        // `start` text, so the whole Arabic/Hebrew/Persian web read left-aligned.
        StyloTextAlign::Start => TextAlign::Start,
        StyloTextAlign::End => TextAlign::End,
        StyloTextAlign::Right | StyloTextAlign::MozRight => TextAlign::Right,
        StyloTextAlign::Center | StyloTextAlign::MozCenter => TextAlign::Center,
        StyloTextAlign::Justify => TextAlign::Justify,
        // Left / MozLeft → left.
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

/// A Stylo `grid-template-columns`/`-rows` component → our `Vec<TrackComponent>`.
///
/// An integer `repeat(N, …)` is expanded here. An **`auto-fill`/`auto-fit`** repeat is carried
/// through intact: Stylo keeps it in `list.values` at `auto_repeat_index` with a non-`Number` count,
/// and the arm below used to fold every such count into `_ => 1` — so
/// `repeat(auto-fill, minmax(18em, 1fr))`, the responsive-card idiom, became a SINGLE column on
/// every page that used it. The repetition count belongs to layout (CSS Grid §7.2.3.1: the largest
/// N that fits the container), not to the cascade. `none`/subgrid/masonry still collapse to what we
/// can model.
fn template_to_tracks(
    c: &stylo::values::computed::GridTemplateComponent,
) -> Vec<crate::TrackComponent> {
    use stylo::values::generics::grid::{
        GenericGridTemplateComponent as GC, GenericTrackListValue as TLV, RepeatCount,
    };
    let mut out = Vec::new();
    if let GC::TrackList(list) = c {
        for v in list.values.iter() {
            match v {
                TLV::TrackSize(ts) => {
                    out.push(crate::TrackComponent::Single(track_size_to_ours(ts)))
                }
                TLV::TrackRepeat(r) => {
                    let tracks: Vec<crate::TrackSize> =
                        r.track_sizes.iter().map(track_size_to_ours).collect();
                    if tracks.is_empty() {
                        continue;
                    }
                    match r.count {
                        // Bounded for the same Bar-0 reason as the text cascade: `repeat(100000,
                        // 1fr)` parses, and an unbounded track list is a hang, not a layout.
                        RepeatCount::Number(i) => {
                            for _ in 0..(i.max(0) as usize).min(1000) {
                                out.extend(
                                    tracks.iter().copied().map(crate::TrackComponent::Single),
                                );
                            }
                        }
                        RepeatCount::AutoFill => {
                            out.push(crate::TrackComponent::AutoRepeat { fit: false, tracks })
                        }
                        RepeatCount::AutoFit => {
                            out.push(crate::TrackComponent::AutoRepeat { fit: true, tracks })
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
    // `text-indent`: Stylo computes the value (incl. the `hanging`/`each_line` keywords); we consume
    // its `.length` — the inline-start indent applied to the first line box in layout.
    s.text_indent = lp_to_dim(&cv.clone_text_indent().length);

    // `pointer-events` — inherited; Stylo's servo build models only `auto`/`none`. `None` drops the
    // element out of hit-testing (`elementFromPoint`, click dispatch) so clicks reach what is behind it.
    s.pointer_events = match cv.clone_pointer_events() {
        stylo::values::computed::ui::PointerEvents::None => crate::PointerEvents::None,
        _ => crate::PointerEvents::Auto,
    };

    // `user-select` — Stylo's servo build parses it only with `layout.unimplemented` flipped on
    // (done in `cascade_via_stylo`); before that the property was dropped and every value read
    // `auto`. We resolve the COMPUTED keyword so `getComputedStyle(el).userSelect` reflects what the
    // stylesheet set — the value toolbars/editors feature-detect. (The `-moz-`/`-webkit-` prefixes
    // Stylo aliases to the same longhand, so `-webkit-user-select: none` lands here too.)
    s.user_select = match cv.clone_user_select() {
        stylo::values::computed::ui::UserSelect::None => crate::UserSelect::None,
        stylo::values::computed::ui::UserSelect::Text => crate::UserSelect::Text,
        stylo::values::computed::ui::UserSelect::All => crate::UserSelect::All,
        stylo::values::computed::ui::UserSelect::Auto => crate::UserSelect::Auto,
    };

    // `color-scheme` — inherited; parses only with `layout.unimplemented` on (same flip as
    // `user_select`). Stylo computes a `ColorScheme { bits }` bitfield of the LIGHT/DARK keywords the
    // author listed. We collapse it to the four cases that decide the canvas default and the
    // `getComputedStyle` string. `only` is a preference-strength hint, not a scheme, so it does not
    // change which keyword set we resolve.
    {
        use stylo::values::specified::color::ColorSchemeFlags as F;
        let bits = cv.clone_color_scheme().bits;
        let light = bits.contains(F::LIGHT);
        let dark = bits.contains(F::DARK);
        s.color_scheme = match (light, dark) {
            (true, true) => crate::ColorScheme::LightDark,
            (false, true) => crate::ColorScheme::Dark,
            (true, false) => crate::ColorScheme::Light,
            (false, false) => crate::ColorScheme::Normal,
        };
    }

    // `scrollbar-width`/`scrollbar-color` are `engine="gecko"` in stylo 0.19 (absent from the servo
    // build), so — like `-webkit-line-clamp` — they are recovered from `MinimalCascade` and merged in
    // `stylo_engine`, not mapped here.

    // Display.
    s.display = map_display(cv.clone_display());
    // `order` — the flex/grid item's VISUAL position, and only its visual position. Sorting items by
    // it is what a `order: -1` "pull this to the front on desktop" rule means, and it is invisible to
    // the DOM, to the a11y tree and to tab order by design (which is why the spec warns about using
    // it for meaning).
    s.order = cv.clone_order();

    // Box model — sizing.
    let cw = cv.clone_width();
    s.width_keyword = size_intrinsic_kw(&cw);
    s.width_stretch = size_is_stretch(&cw);
    s.width = size_to_dim(&cw);
    let ch = cv.clone_height();
    s.height_intrinsic = size_is_intrinsic(&ch);
    s.height_stretch = size_is_stretch(&ch);
    s.height = size_to_dim(&ch);
    // ⚠ The four min/max properties take the SAME intrinsic keywords `width`/`height` do, and until
    // t930 only the `Dim` was read — so a keyword landed on `Dim::Auto`, which the clamp reads as 0
    // on a min and as no-limit on a max. The keyword sidecar is what makes the declaration
    // representable at all; see `ComputedStyle::min_width_keyword`.
    let cmnw = cv.clone_min_width();
    let cmnh = cv.clone_min_height();
    let cmxw = cv.clone_max_width();
    let cmxh = cv.clone_max_height();
    s.min_width_keyword = minsize_intrinsic_kw(&cmnw);
    s.min_height_keyword = minsize_intrinsic_kw(&cmnh);
    s.max_width_keyword = maxsize_intrinsic_kw(&cmxw);
    s.max_height_keyword = maxsize_intrinsic_kw(&cmxh);
    s.min_width = size_to_dim(&cmnw);
    s.min_height = size_to_dim(&cmnh);
    s.max_width = maxsize_to_dim(&cmxw);
    s.max_height = maxsize_to_dim(&cmxh);

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

    // `box-shadow` — the full layer list in source order (first on top). Spread and `inset` are
    // carried per layer; Tailwind's elevation utilities stack two layers with a negative spread.
    s.box_shadows = cv
        .clone_box_shadow()
        .0
        .iter()
        .map(|sh| crate::BoxShadow {
            dx: sh.base.horizontal.px(),
            dy: sh.base.vertical.px(),
            blur: sh.base.blur.0.px().max(0.0),
            spread: sh.spread.px(),
            inset: sh.inset,
            color: abs_to_rgba(&sh.base.color.clone().resolve_to_absolute(&current)),
        })
        .collect();

    // `filter` — the function list, in SOURCE ORDER, because the list is a pipeline: `grayscale(1)
    // blur(2px)` and `blur(2px) grayscale(1)` are different pictures. Stylo's servo build parses and
    // computes this correctly and always has; the defect (t591) was that nothing ever *read* it, so
    // `@supports` answered yes to a capability that painted nothing.
    s.filter = map_filter_list(&cv.clone_filter().0, &current);

    // `backdrop-filter` — the same function list as `filter`, so the same mapping, deliberately
    // sharing one closure rather than a second copy that could drift from it.
    s.backdrop_filter = map_filter_list(&cv.clone_backdrop_filter().0, &current);

    // `clip-path` — the four BASIC SHAPES. `path()`/`shape()`/`url()` need an SVG path graph and are
    // mapped to `None` (unclipped) rather than to a variant nothing draws: an unclipped element is
    // visibly wrong, a shape we pretend to honour is *silently* wrong, and this engine has just spent
    // two ticks removing that second kind.
    {
        use stylo::values::generics::basic_shape::{
            GenericBasicShape as GBS, GenericClipPath as GCP, GenericShapeRadius as GSR,
        };
        use stylo::values::generics::position::GenericPositionOrAuto as GPA;
        // A `<position>` component → our `Dim`, measured from the box's top-left. `auto` is the
        // shape's own default centre, which for both circle and ellipse is 50%.
        let pos_or_auto = |p: &GPA<stylo::values::computed::Position>| match p {
            GPA::Auto => (crate::Dim::Percent(50.0), crate::Dim::Percent(50.0)),
            GPA::Position(p) => (lp_to_dim(&p.horizontal), lp_to_dim(&p.vertical)),
        };
        let radius = |r: &GSR<LengthPercentage>| match r {
            GSR::Length(l) => crate::ShapeRadius::Len(lp_to_dim(&l.0)),
            GSR::ClosestSide => crate::ShapeRadius::ClosestSide,
            GSR::FarthestSide => crate::ShapeRadius::FarthestSide,
            // The CORNER keywords are not spellable in `circle()`/`ellipse()` — Stylo shares this
            // enum with `radial-gradient()`, where they are. Mapped to their side counterparts
            // rather than left to a catch-all, so a future grammar change surfaces as a compile
            // error instead of a silently wrong radius.
            GSR::ClosestCorner => crate::ShapeRadius::ClosestSide,
            GSR::FarthestCorner => crate::ShapeRadius::FarthestSide,
        };
        s.clip_path = match cv.clone_clip_path() {
            GCP::Shape(shape, _geometry_box) => match &*shape {
                GBS::Rect(inset) => Some(crate::ClipShape::Inset {
                    top: lp_to_dim(&inset.rect.0),
                    right: lp_to_dim(&inset.rect.1),
                    bottom: lp_to_dim(&inset.rect.2),
                    left: lp_to_dim(&inset.rect.3),
                    round: match lp_to_dim(&inset.round.top_left.0.width.0) {
                        crate::Dim::Px(px) => px.max(0.0),
                        _ => 0.0,
                    },
                }),
                GBS::Circle(c) => {
                    let (cx, cy) = pos_or_auto(&c.position);
                    Some(crate::ClipShape::Circle {
                        cx,
                        cy,
                        r: radius(&c.radius),
                    })
                }
                GBS::Ellipse(e) => {
                    let (cx, cy) = pos_or_auto(&e.position);
                    Some(crate::ClipShape::Ellipse {
                        cx,
                        cy,
                        rx: radius(&e.semiaxis_x),
                        ry: radius(&e.semiaxis_y),
                    })
                }
                GBS::Polygon(p) => {
                    use stylo::values::generics::basic_shape::FillRule;
                    Some(crate::ClipShape::Polygon {
                        even_odd: matches!(p.fill, FillRule::Evenodd),
                        points: p
                            .coordinates
                            .iter()
                            .map(|c| (lp_to_dim(&c.0), lp_to_dim(&c.1)))
                            .collect(),
                    })
                }
                // `path()` / `shape()` — an SVG path graph, honestly unclipped.
                GBS::PathOrShape(_) => None,
            },
            // `none`, a bare `<geometry-box>` (which clips to a box we already clip to), or `url()`.
            _ => None,
        };
    }

    // `mix-blend-mode` — a straight keyword map. Exhaustive on purpose: a catch-all arm here would
    // turn a future keyword into a silent `normal`, which is the "renders unblended and says
    // nothing" failure this bundle exists to remove.
    {
        use stylo::properties::longhands::mix_blend_mode::computed_value::T as SBlend;
        s.mix_blend_mode = match cv.clone_mix_blend_mode() {
            SBlend::Normal => crate::BlendMode::Normal,
            SBlend::Multiply => crate::BlendMode::Multiply,
            SBlend::Screen => crate::BlendMode::Screen,
            SBlend::Overlay => crate::BlendMode::Overlay,
            SBlend::Darken => crate::BlendMode::Darken,
            SBlend::Lighten => crate::BlendMode::Lighten,
            SBlend::ColorDodge => crate::BlendMode::ColorDodge,
            SBlend::ColorBurn => crate::BlendMode::ColorBurn,
            SBlend::HardLight => crate::BlendMode::HardLight,
            SBlend::SoftLight => crate::BlendMode::SoftLight,
            SBlend::Difference => crate::BlendMode::Difference,
            SBlend::Exclusion => crate::BlendMode::Exclusion,
            SBlend::Hue => crate::BlendMode::Hue,
            SBlend::Saturation => crate::BlendMode::Saturation,
            SBlend::Color => crate::BlendMode::Color,
            SBlend::Luminosity => crate::BlendMode::Luminosity,
            // `plus-lighter` has no `tiny-skia` counterpart (it is additive with a clamp, close to
            // `Plus` but defined on premultiplied values). Honestly `normal` rather than a
            // near-miss: a wrong blend is harder to spot than none.
            SBlend::PlusLighter => crate::BlendMode::Normal,
        };
    }

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
    s.overflow_x = ox;
    s.overflow_y = oy;
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
        // AlignFlags: 0 AUTO, 1 NORMAL, 2 START, 3 END, 4 FLEX_START, 5 FLEX_END, 6 CENTER,
        // 7 LEFT, 8 RIGHT, 11 STRETCH, 14/15/16 SPACE_{BETWEEN,AROUND,EVENLY}.
        // This is the LIVE cascade, so `normal` MUST land on `Normal` here — the arm below is the
        // one that decides whether a grid's `auto` tracks stretch (CSS Grid §11.8).
        let map_cd = |v: u8| match v {
            5 | 3 | 8 => crate::JustifyContent::FlexEnd,
            6 => crate::JustifyContent::Center,
            14 => crate::JustifyContent::SpaceBetween,
            15 => crate::JustifyContent::SpaceAround,
            16 => crate::JustifyContent::SpaceEvenly,
            2 | 4 | 7 => crate::JustifyContent::FlexStart,
            _ => crate::JustifyContent::Normal,
        };
        s.justify_content = map_cd(av(cv.clone_justify_content().primary()));
        // `align-content` — the CROSS/BLOCK-axis twin, and the half that did not exist. A wrapped
        // flex container laid every line from the top and a grid left its rows at the start of the
        // box, whatever the author declared: Chrome puts `align-content: flex-end`'s last line at
        // y=160 in a 200px box against our y=100. `stretch` (11) lands on `Normal` because that IS
        // stretch on this axis in both formatting contexts, so the mapping is exact rather than
        // approximate. Shares `map_cd` with its twin so a future value can only be added to both.
        s.align_content = map_cd(av(cv.clone_align_content().primary()));
        let map_ai = |v: u8| match v {
            5 | 3 | 13 => crate::AlignItems::FlexEnd,
            6 => crate::AlignItems::Center,
            9 | 10 => crate::AlignItems::Baseline,
            4 | 2 | 12 => crate::AlignItems::FlexStart,
            _ => crate::AlignItems::Stretch,
        };
        s.align_items = map_ai(av(cv.clone_align_items().0));
        // `justify-items` — the INLINE-axis twin of `align-items`, and the default every grid item
        // inherits unless it sets `justify-self`. Its initial value is `legacy`, which is the LEGACY
        // *flag* (bit 5) over an otherwise empty value; `av` masks to the low five bits, so the
        // initial arrives here as `normal` and maps to `Stretch` — the behaviour `normal` has in a
        // grid. `.computed` (never `.specified`) is the right half of Stylo's pair: the specified
        // half can still carry the bare `legacy` keyword, which the computed half has already
        // resolved away.
        s.justify_items = map_ai(av(cv.clone_justify_items().computed.0 .0));
        s.align_self = match av(cv.clone_align_self().0) {
            0 => None,
            v => Some(map_ai(v)),
        };
        // `justify-self` — the same shape one axis over, and the half that was missing. `align_self`
        // reached taffy and this did not, so a grid item asking for `justify-self: end` sat at the
        // START of its track: Chrome x=140 in a 200px track against our x=0.
        s.justify_self = match av(cv.clone_justify_self().0) {
            0 => None,
            v => Some(map_ai(v)),
        };
        // row-gap / column-gap: `normal` → 0, else the length-or-PERCENTAGE.
        //
        // ⚠ The old body funnelled `lp_to_dim` through `Dim::Px(p) => p, _ => 0.0` — so a percentage
        // arrived from Stylo intact and was thrown away one line later, which is the same
        // "arrived and dropped" shape t981 found in the `place-*` shorthands. Carrying the `Dim`
        // through is the whole change on this side.
        use stylo::values::generics::length::GenericLengthPercentageOrNormal as GapVal;
        let gap_dim =
            |g: stylo::values::computed::length::NonNegativeLengthPercentageOrNormal| match g {
                GapVal::Normal => Dim::Px(0.0),
                GapVal::LengthPercentage(lp) => lp_to_dim(&lp.0),
            };
        s.row_gap = gap_dim(cv.clone_row_gap());
        s.column_gap = gap_dim(cv.clone_column_gap());
    }

    // ── `will-change` / `contain` / `perspective` — a containing block for out-of-flow descendants
    //    without being positioned and without a transform (CSS Transforms §3, CSS Contain §).
    //
    // Stylo has already done the hard half: `WillChangeBits::FIXPOS_CB_NON_SVG` is precisely
    // *"a property that creates a containing block for fixed-position descendants will change"*,
    // so the keyword list does not have to be re-derived here — and re-deriving it is how the
    // `opacity` case would have been got wrong, since `will-change: opacity` creates a stacking
    // context but NOT a containing block (Chrome-measured).
    {
        use stylo::values::specified::box_::{Contain, WillChangeBits};
        let wc = cv.clone_will_change();
        let contain = cv.clone_contain();
        s.establishes_containing_block = wc.bits.intersects(
            WillChangeBits::TRANSFORM
                | WillChangeBits::PERSPECTIVE
                | WillChangeBits::FIXPOS_CB_NON_SVG,
        ) || contain.intersects(Contain::LAYOUT | Contain::PAINT)
            || !matches!(
                cv.clone_perspective(),
                stylo::values::generics::box_::GenericPerspective::None
            );
    }

    // box-sizing.
    s.box_sizing = match cv.clone_box_sizing() {
        stylo::properties::longhands::box_sizing::computed_value::T::BorderBox => {
            crate::BoxSizing::BorderBox
        }
        _ => crate::BoxSizing::ContentBox,
    };

    // aspect-ratio — the CSS property (not an image's intrinsic ratio, which the page layer sets from
    // the decoded bytes). Stylo's computed value is `auto || <ratio>`; we take the `<ratio>`'s
    // width/height whenever one is present (for a non-replaced box the specified ratio always applies,
    // `auto` or not). `s.aspect_ratio` is a plain `width/height` f32 that the abspos and in-flow
    // sizing paths transfer a definite length through. Without this the property was silently dropped:
    // an `aspect-ratio: 16/9` box got no ratio at all, so `layout_abs`/the in-flow path never fired.
    {
        use stylo::values::generics::position::PreferredRatio;
        if let PreferredRatio::Ratio(r) = cv.clone_aspect_ratio().ratio {
            let (w, h) = ((r.0).0, (r.1).0);
            if w > 0.0 && h > 0.0 {
                s.aspect_ratio = Some(w / h);
            }
        }
    }

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
    // The vertical half — see `ComputedStyle::border_spacing_v`. Stylo models the pair; taking only
    // `horizontal()` is what made a two-value `border-spacing` inset rows by the column value.
    s.border_spacing_v = cv.clone_border_spacing().vertical().to_f32_px();

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
                // ⚠⚠⚠ **THE 3D SPELLINGS WERE FALLING INTO `_ => {}`, AND THE COMMENT ABOVE IS WHAT
                // HID IT.** *"3D/perspective skipped — our paint model is 2D"* is true of a genuine
                // 3D effect and false of `translate3d(x, y, 0)`, which is not a 3D transform at all:
                // it is **the** idiom for putting an element on its own compositor layer, and it is
                // how every animation library, carousel, drawer and sticky header on the modern web
                // writes a plain translation. Dropped, the element stays at its **untransformed**
                // position — the largest error the property can produce. Measured against Chrome on
                // a 100×40 box:
                //
                // ```text
                //                                         Chrome        before        after
                //   translate3d(20px,10px,0)             [ 20, 1070]   [  0, 1060]   [ 20, 1070]
                //   scale3d(2,2,1)                       [-50, 1110]   [  0, 1130]   [-50, 1110]
                //   matrix3d(… 30,15,0,1)                [ 30, 1355]   [  0, 1340]   [ 30, 1355]
                //   rotate3d(0,0,1,45deg)                [0.5, 1170]   [  0, 1200]   [0.5, 1170]
                //   rotateZ(90deg)                       [ 30, 1240]   [ 30, 1240]   unchanged ✓
                //   translateZ(50px)  (no 2D effect)     [  0, 1410]   [  0, 1410]   unchanged ✓
                // ```
                //
                // With no `perspective` in force `z` contributes nothing to the on-screen position,
                // so the x/y terms of each 3D function **are** its rendered effect — this is an
                // exact projection, not an approximation. `rotate3d` is taken **only about the z
                // axis** for the opposite reason: a rotation about x or y foreshortens, which a 2D
                // pipeline cannot express, and inventing one would be a wrong answer of the right
                // type. `TranslateZ`/`Perspective` are matched explicitly so their omission reads as
                // a decision rather than as the `_` arm that hid this.
                TOp::Translate3D(x, y, _z) => {
                    ops.push(crate::TransformFn::Translate(lp_to_dim(x), lp_to_dim(y)))
                }
                TOp::Scale3D(x, y, _z) => ops.push(crate::TransformFn::Scale(*x, *y)),
                // ⚠⚠⚠ A rotation about x or y is NOT inexpressible in 2D — the note above said it
                // was, and Chrome disagrees: with no `perspective` in force the projection is
                // exactly a scale by `cos θ` on the perpendicular axis (`rotateX(45deg)` on a
                // 100x40 box measures 100 x 28.28, not 100 x 40). `axis_rotation_2d` owns that
                // rule; only a genuinely MIXED axis is still dropped, and that exclusion is
                // measured too (`rotate3d(1,1,0,45deg)` is 91.21 x 48.79, not a scale on either).
                TOp::Rotate3D(x, y, z, a) => {
                    if let Some(t) = crate::axis_rotation_2d(*x, *y, *z, a.radians()) {
                        ops.push(t)
                    }
                }
                TOp::RotateX(a) => {
                    if let Some(t) = crate::axis_rotation_2d(1.0, 0.0, 0.0, a.radians()) {
                        ops.push(t)
                    }
                }
                TOp::RotateY(a) => {
                    if let Some(t) = crate::axis_rotation_2d(0.0, 1.0, 0.0, a.radians()) {
                        ops.push(t)
                    }
                }
                TOp::Matrix3D(m) => ops.push(crate::TransformFn::Matrix([
                    m.m11, m.m12, m.m21, m.m22, m.m41, m.m42,
                ])),
                _ => {}
            }
        }
        s.transform = ops;
    }

    // ⚠⚠⚠ **`translate` / `rotate` / `scale` — THE INDIVIDUAL TRANSFORM PROPERTIES, AND THIS PATH
    // IS THE ONE THAT DECIDES REAL PAGES.** They are properties of their own (CSS Transforms 2) so
    // that setting one does not clobber the others, which is why every animation library now writes
    // them — and neither cascade read them, so the element sat UNTRANSFORMED. Measured against
    // Chrome on a 40x20 box: `translate:30px 15px` belongs at [50, 2005] and sat at [20, 1990];
    // `rotate:90deg` belongs at [30, 2070] 20x40 and sat at [20, 2080] 40x20; `scale:2` belongs at
    // [0, 2160] 80x40 and sat at [20, 2170] 40x20. Priced at 33/171 = **19.3%** of the burndown
    // corpus, fetched with their linked stylesheets.
    //
    // The composition order is fixed by the spec and applied in `effective_transform`, not here:
    // translate, then rotate, then scale, then the `transform` list, whatever order they cascaded
    // in. Only the z rotation has a 2D effect, the same exact-projection rule `rotate3d` gets above.
    {
        use stylo::values::generics::transform::{Rotate, Scale, Translate};
        s.translate = match cv.clone_translate() {
            Translate::None => None,
            Translate::Translate(x, y, _z) => Some((lp_to_dim(&x), lp_to_dim(&y))),
        };
        s.rotate = match cv.clone_rotate() {
            Rotate::None => None,
            Rotate::Rotate(a) => Some(crate::TransformFn::Rotate(a.radians())),
            Rotate::Rotate3D(x, y, z, a) => crate::axis_rotation_2d(x, y, z, a.radians()),
        };
        s.scale = match cv.clone_scale() {
            Scale::None => None,
            Scale::Scale(x, y, _z) => Some((x, y)),
        };
    }

    // `transform-origin`: the point the matrix above is applied ABOUT. Recovered from Stylo's own
    // computed value — the engine hard-coded the box centre at three call sites, so an author who
    // wrote `transform-origin: 0 0` got a transform about the centre and a box in the wrong place
    // (Chrome [0, 220] against our [-50, 200] on a `scale(2)`). The z component is dropped: it only
    // matters under a perspective context, which this 2D pipeline does not model.
    {
        let o = cv.clone_transform_origin();
        s.transform_origin = (lp_to_dim(&o.horizontal), lp_to_dim(&o.vertical));
    }

    // Grid tracks + item placement.
    s.grid_template_columns = template_to_tracks(&cv.clone_grid_template_columns());
    s.grid_template_rows = template_to_tracks(&cv.clone_grid_template_rows());
    // The IMPLICIT half of the same pair. `grid-template-*` sizes the tracks the author wrote down;
    // these size the ones auto-placement invents when the items outrun them, and they were the two
    // lines missing beside the two above. Stylo's `ImplicitGridTracks` is a plain slice of
    // `<track-size>` — no `repeat()` is representable, which is exactly the grammar difference the
    // minimal cascade's separate parser records.
    s.grid_auto_rows = cv
        .clone_grid_auto_rows()
        .0
        .iter()
        .map(track_size_to_ours)
        .collect();
    s.grid_auto_columns = cv
        .clone_grid_auto_columns()
        .0
        .iter()
        .map(track_size_to_ours)
        .collect();
    // `grid-auto-flow` is BITFLAGS in Stylo (`ROW|COLUMN|DENSE`, with row/column mutually exclusive
    // and `dense` alone normalised to `row dense`), so the axis is read as "is COLUMN set" rather
    // than by matching a variant — an absent ROW bit on a value that also lacks COLUMN would
    // otherwise fall through to the wrong axis.
    {
        use stylo::values::specified::position::GridAutoFlow as SF;
        let f = cv.clone_grid_auto_flow();
        s.grid_auto_flow = match (f.contains(SF::COLUMN), f.contains(SF::DENSE)) {
            (false, false) => crate::GridAutoFlow::Row,
            (false, true) => crate::GridAutoFlow::RowDense,
            (true, false) => crate::GridAutoFlow::Column,
            (true, true) => crate::GridAutoFlow::ColumnDense,
        };
    }
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

    // Computed CSS custom properties (`--foo`). Stylo has already resolved the cascade and expanded
    // `var()`, so this is the value `getComputedStyle(el).getPropertyValue('--foo')` must return.
    // `property_at` walks the inherited then non-inherited maps; a `None` value is a
    // guaranteed-invalid registered property, skipped. Unregistered custom properties (the common
    // case — `--foo: 42px` with no `@property`) carry their CSS text in the universal variant.
    {
        let cp = cv.custom_properties();
        // **`iter()`, NOT `property_at(i)` in a loop — the difference is O(n) against O(n²).**
        // `property_at` forwards to `CustomPropertiesMap::get_index`, whose body is
        // `self.0.iter().nth(index)` under a comment in Stylo reading *"FIXME: This is O(n) which
        // is a bit unfortunate."* Indexing it in a `while` loop makes the whole walk quadratic, and
        // it reads as linear at the call site because the only visible operation is `i += 1`.
        //
        // Custom properties INHERIT, so on a page with a design-token sheet n is the size of the
        // whole token vocabulary at every element. Measured on wix.com — 575 tokens, 10,424
        // elements, 1.44M entries per cascade — this loop alone was **7.5 s of an 11.3 s cascade**.
        //
        // ⚠ **FIRST OCCURRENCE WINS, and that is a correctness fix, not a tidy-up.** The map is
        // copy-on-write with a PARENT CHAIN (Stylo switches to chaining above 8 own properties),
        // and the chain iterator yields a shadowing element's own entry first and then the
        // ancestor's entry for the SAME name. Every consumer of this list — the `__custom` object
        // literal that `getPropertyValue` reads, and the `item(i)` enumeration — takes the LAST
        // write, so an overridden token resolved to the value it overrode. `#shadow{--brand:green}`
        // under `:root{--brand:red}` computed to RED, and `--brand` enumerated twice.
        //
        // This predates the rewrite: the original `property_at` walk produced the identical wrong
        // answer on the same fixture, which G_COMPUTED_CUSTOM_PROPERTIES now pins. It stayed
        // invisible because it needs >8 custom properties to form a chain at all, and the old
        // fixture had two.
        let (n_inh, n_non) = (cp.inherited.len(), cp.non_inherited.len());
        s.custom_properties.reserve(n_inh + n_non);
        SEEN.with(|seen| {
            let mut seen = seen.borrow_mut();
            seen.clear();
            for (name, value) in cp.inherited.iter().chain(cp.non_inherited.iter()) {
                let Some(v) = value else { continue };
                let Some(var) = v.as_universal() else {
                    continue;
                };
                // Both halves are interned: the NAME repeats once per element that inherits it, and
                // the VALUE repeats just as hard, because an inherited token has the same computed
                // text on every descendant. See the field's doc comment.
                let key = intern_dashed(name);
                if !seen.insert(std::sync::Arc::clone(&key)) {
                    continue; // already taken from a more-derived level of the chain
                }
                s.custom_properties.push((key, intern(var.css.trim())));
            }
        });
    }

    s
}

thread_local! {
    /// Scratch de-duplication set for the custom-property walk, reused across elements so a
    /// document does not allocate one hash set per element.
    static SEEN: std::cell::RefCell<std::collections::HashSet<std::sync::Arc<str>>> =
        std::cell::RefCell::new(std::collections::HashSet::new());
}

/// A process-wide interner for the strings that end up in
/// [`ComputedStyle::custom_properties`](crate::ComputedStyle::custom_properties).
///
/// **Thread-local, deliberately.** The cascade is single-threaded per document and a global lock
/// here would serialise the one loop this exists to speed up. The cost is one table per thread,
/// which is bounded by the number of DISTINCT custom-property strings a page uses — hundreds, not
/// millions.
///
/// It is never cleared. That is a bounded leak by design: the keys are custom-property names and
/// their computed values, so the ceiling is the vocabulary of the pages this thread has rendered,
/// and re-rendering the same page (which is the common case — the cascade runs ~8× per load) hits
/// every entry rather than growing the table.
mod interner {
    use std::cell::RefCell;
    use std::collections::{HashMap, HashSet};
    use std::sync::Arc;

    thread_local! {
        static POOL: RefCell<HashSet<Arc<str>>> = RefCell::new(HashSet::new());
        /// Bare custom-property name -> the `--`-prefixed interned form.
        static DASHED: RefCell<HashMap<Box<str>, Arc<str>>> = RefCell::new(HashMap::new());
    }

    pub fn intern(s: &str) -> Arc<str> {
        POOL.with(|p| {
            let mut p = p.borrow_mut();
            if let Some(a) = p.get(s) {
                return Arc::clone(a);
            }
            let a: Arc<str> = Arc::from(s);
            p.insert(Arc::clone(&a));
            a
        })
    }

    /// `intern` for a custom-property NAME, returning it WITH its leading `--`.
    ///
    /// Keyed by the **bare** name in its own table, so the lookup hashes the `&str` Stylo already
    /// has and the `format!("--{name}")` runs only on a genuine miss — 575 times for a page with
    /// 575 tokens, not 1.44 million.
    ///
    /// (The first draft of this probed the shared pool with `.iter().find(…)` to dodge that
    /// `format!`. A linear scan of a 575-entry table, 1.44M times, is 800M string comparisons —
    /// **slower than the allocation it was avoiding.** A second table is the right trade: interning
    /// is only a win if the lookup is O(1).)
    pub fn intern_dashed(name: &str) -> Arc<str> {
        DASHED.with(|p| {
            if let Some(a) = p.borrow().get(name) {
                return Arc::clone(a);
            }
            let a: Arc<str> = Arc::from(format!("--{name}").as_str());
            p.borrow_mut().insert(name.into(), Arc::clone(&a));
            a
        })
    }
}
use interner::{intern, intern_dashed};

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
