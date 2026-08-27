//! **Vertical writing modes, by TRANSPOSING the subtree rather than by teaching every rule two
//! spellings of "length".**
//!
//! ## The problem this solves, and why it was silent
//!
//! `writing-mode: vertical-rl` swaps which physical axis the *inline* direction runs along. In
//! `horizontal-tb` a box's `width` is its inline size and its `height` is its block size, and this
//! engine's twenty-six thousand lines of block, inline, float and table layout have that pairing
//! welded in — not as an assumption anyone wrote down, but as the meaning of the words `width` and
//! `cw` and `content_height` in every function signature. In a vertical mode the pairing is the
//! other way round: `width` is a **block** size and `height` is an **inline** size.
//!
//! Nothing in the engine noticed, because a vertical page is not *malformed* — it is laid out
//! perfectly, at ninety degrees to where it belongs. Measured against headless Chrome on
//! `<div style="width:400px;writing-mode:vertical-rl"><div>x</div></div>` (16px/20px monospace):
//!
//! ```text
//!                              Chrome                    ours (before)
//!   container               400 x 10                    400 x 20
//!   child (parent-rel)      [380  0  20x10]             [0 0 400x20]
//! ```
//!
//! Every number differs, and the child is at the wrong end of the box.
//!
//! ## The approach: one coordinate change, not a thousand branches
//!
//! Two implementations were possible. The first — thread a "logical axis" concept through every
//! sizing rule — is the one that keeps a browser's layout code honest long-term, and it is also the
//! one that touches every line of it, i.e. exactly the kind of change that trades a working engine
//! for a hoped-for one. The second, taken here: **run the existing horizontal engine in a
//! transposed coordinate space and map the answers back.**
//!
//! It works because the transposition is total. In a vertical subtree:
//!
//! - a box's `height` **is** the inline size the horizontal engine calls `width`,
//! - `margin-top` **is** the inline-start margin it calls `margin-left`,
//! - a text run's advance **is** its inline extent either way — the same shaping, the same numbers,
//!   just pointed down the page instead of across it,
//! - and `flex-direction: row`, `grid-template-columns`, `column-gap`, `justify-content` are
//!   already *logical* in CSS, so they need no swap at all: `row` means "along the inline axis",
//!   which is precisely what the transposed engine's x axis is.
//!
//! So the whole of layout runs unmodified, on styles whose physical fields have been swapped, and a
//! single affine map turns the resulting fragment tree back into page coordinates.
//!
//! ⚠⚠⚠ **A PAGE WITH NO VERTICAL WRITING MODE MUST BE BYTE-IDENTICAL, and that is enforced by
//! construction rather than by care:** [`plan`] returns `None` unless some computed style actually
//! carries a vertical mode, and `None` means the original `StyleMap` is used and none of this code
//! runs. The transposed map is a clone, so it also cannot corrupt the cascade's own output.
//!
//! ## The two maps, measured
//!
//! With the container's content box at `(cx, cy, cw)` and a subtree laid out at logical origin
//! `(0, 0)` — `ex` along the inline axis, `ey` along the block axis:
//!
//! ```text
//!   vertical-rl   px = cx + cw - (ey + eh)     py = cy + ex     pw = eh   ph = ew
//!   vertical-lr   px = cx + ey                 py = cy + ex     pw = eh   ph = ew
//! ```
//!
//! Both were checked against Chrome on a thirteen-row fixture; the `vertical-lr` row that names the
//! difference is two block children landing at `x=0` and `x=20` where `vertical-rl` puts them at
//! `x=380` and `x=360`.
//!
//! ## What is deliberately NOT modelled yet, and is named rather than hidden
//!
//! - **An orthogonal flow nested inside another one** — a `horizontal-tb` element inside a
//!   `vertical-rl` subtree stays transposed with its ancestor instead of switching back. Handling it
//!   means a second boundary in the opposite direction; it is rare enough on the open web that
//!   shipping the common case first is the right order.
//! - **The central baseline.** Chrome aligns mixed-size runs on one vertical line by their em-box
//!   CENTRES, not their alphabetic baselines (measured: a 10px and a 40px monospace `a`/`b` share a
//!   centre at 277.5/277.0, not a baseline). The transposed engine aligns alphabetically, which is
//!   identical for a single-font run — i.e. for almost every run — and off by the ascent/descent
//!   asymmetry for a mixed-size one.
//! - **Upright CJK glyphs.** `text-orientation: mixed` leaves ideographs upright and their advance
//!   is the em box, not the horizontal advance. Latin text — which is what `writing-mode` is used
//!   for on the CrUX corpus, on rotated table headers and sidebar labels — is rotated, and its
//!   horizontal advance is exactly right.

use std::collections::HashMap;

use manuk_css::{ComputedStyle, Sides, StyleMap, WritingMode};
use manuk_dom::{Dom, NodeId};

use crate::{BoxContent, LayoutBox};

/// A transposed style map plus the nodes that begin a transposed subtree.
pub(crate) struct WmPlan {
    /// Every node's style, with the geometry fields of nodes *inside* a vertical subtree swapped.
    /// A clone of the cascade's map: layout must not be able to write back into the cascade.
    pub styles: StyleMap,
    /// The ORTHOGONAL ROOTS — elements whose own box is physical (their parent is horizontal) but
    /// whose children are laid out transposed. Keyed by node, valued by the mode to map back with.
    pub roots: HashMap<NodeId, WritingMode>,
}

/// Build the plan, or `None` when the document has no vertical writing mode at all.
///
/// The `None` is the whole safety argument: it is the common case by an enormous margin, it costs
/// one pass over the style map's values, and it makes "did this change anything for an ordinary
/// page?" answerable without reading any of the code below.
pub(crate) fn plan(dom: &Dom, styles: &StyleMap) -> Option<WmPlan> {
    if !styles.values().any(|s| s.writing_mode.is_vertical()) {
        return None;
    }
    let mut out = styles.clone();
    let mut roots = HashMap::new();
    let mut stack: Vec<(NodeId, Option<WritingMode>)> = vec![(dom.root(), None)];
    while let Some((node, parent_space)) = stack.pop() {
        // The mode this node's own children will be laid out in.
        let child_space = match parent_space {
            // Parent's content space is physical: a vertical node OPENS a transposed subtree, and
            // its own box stays physical because its parent placed it with physical rules.
            None => {
                let m = styles
                    .get(&node)
                    .map(|s| s.writing_mode)
                    .unwrap_or_default();
                if m.is_vertical() {
                    roots.insert(node, m);
                    Some(m)
                } else {
                    None
                }
            }
            // Already inside a transposed subtree: this node's own box is transposed, and so is
            // everything below it. (The nested-orthogonal case named in the module docs.)
            //
            // ⚠⚠⚠ **EXCEPT AN OUT-OF-FLOW BOX, WHOSE OWN GEOMETRY IS PHYSICAL.** `top`, `left`,
            // `width` and `height` on an absolutely positioned box are PHYSICAL properties resolved
            // against its containing block's padding box — and that containing block is the
            // orthogonal root itself, whose own box is physical. Transposing the abspos and then
            // never mapping it back (it is placed by `position_absolutes`, LONG after
            // `map_subtree` has run over the in-flow children) left it transposed twice over.
            // Chrome-measured, a 5x6 abspos in a 300x200 `vertical-lr` container:
            //
            // ```text
            //                                  Chrome        before
            //   top:10px; left:20px           @20,10 5x6    @10,20 6x5   ← position AND size swapped
            //   top:10px                      @0,10         @10,-200
            //   left:20px                     @20,0         @0,20
            //   …the same, horizontal  CONTROL @20,10 5x6   @20,10 5x6   ✓
            // ```
            //
            // ⭐ So it is not a transposed descendant — it is a NEW ORTHOGONAL ROOT: its own box is
            // physical (its parent placed it with physical rules) and its CONTENT is still vertical,
            // because `writing-mode` inherits. That is exactly the state `roots` already models, and
            // `layout_abs` already knows how to lay one out (t1347). The fix is to classify it
            // correctly rather than to add a second mechanism.
            Some(pm) => {
                // ⚠⚠ **NOT AN OUT-OF-FLOW CHILD OF A FLEX OR GRID CONTAINER**, and that exclusion
                // cost 38 subtests to find. Those are placed by the flex/grid machinery against a
                // grid AREA or the container's content box — paths that already fold the
                // transposition in — so re-rooting them applies it twice.
                // `css/css-grid/alignment/grid-*-axis-alignment-positioned-items-*` went red with
                // `width expected 10 but got 60` on the first version, which had no such guard;
                // t1347's `orthogonal-positioned-grid-descendants` (an abspos inside a grid ITEM,
                // not a grid item itself) is the shape that DOES need it, and stayed green.
                let parent_is_flex_or_grid = dom.parent(node).is_some_and(|p| {
                    matches!(
                        styles.get(&p).map(|s| s.display),
                        Some(manuk_css::Display::Flex)
                            | Some(manuk_css::Display::Grid)
                            | Some(manuk_css::Display::InlineFlex)
                            | Some(manuk_css::Display::InlineGrid)
                    )
                });
                let out_of_flow = !parent_is_flex_or_grid
                    && matches!(
                        styles.get(&node).map(|s| s.position),
                        Some(manuk_css::Position::Absolute) | Some(manuk_css::Position::Fixed)
                    );
                if out_of_flow {
                    roots.insert(node, pm);
                } else if let Some(s) = out.get_mut(&node) {
                    transpose_in_place(s, pm);
                }
                Some(pm)
            }
        };
        for c in dom.children(node) {
            stack.push((c, child_space));
        }
    }
    Some(WmPlan { styles: out, roots })
}

/// Swap a style's PHYSICAL geometry fields into the transposed space.
///
/// ⚠ Only the physical ones. CSS's own logical properties (`flex-direction`, `justify-content`,
/// `column-gap`, the grid track lists) are defined against the inline/block axes already, and the
/// transposed engine's x axis *is* the inline axis — swapping them would rotate them twice.
fn transpose_in_place(s: &mut ComputedStyle, m: WritingMode) {
    // A side that is *block-start* physically depends on which way blocks stack.
    let (bs, be) = if m.is_rl() {
        // vertical-rl: blocks stack right-to-left, so block-start is the RIGHT edge.
        (Side::Right, Side::Left)
    } else {
        (Side::Left, Side::Right)
    };
    swap_sides(&mut s.margin, bs, be);
    swap_sides(&mut s.padding, bs, be);
    swap_sides(&mut s.border_width, bs, be);
    swap_sides(&mut s.border_color, bs, be);
    swap_sides(&mut s.border_style, bs, be);
    swap_sides(&mut s.inset, bs, be);

    std::mem::swap(&mut s.width, &mut s.height);
    std::mem::swap(&mut s.min_width, &mut s.min_height);
    std::mem::swap(&mut s.max_width, &mut s.max_height);
    std::mem::swap(&mut s.min_width_keyword, &mut s.min_height_keyword);
    std::mem::swap(&mut s.max_width_keyword, &mut s.max_height_keyword);
    std::mem::swap(&mut s.min_width_stretch, &mut s.min_height_stretch);
    std::mem::swap(&mut s.max_width_stretch, &mut s.max_height_stretch);
    std::mem::swap(&mut s.width_stretch, &mut s.height_stretch);
    std::mem::swap(&mut s.width_is_natural, &mut s.height_is_natural);
    std::mem::swap(&mut s.overflow_x, &mut s.overflow_y);
    std::mem::swap(&mut s.border_spacing, &mut s.border_spacing_v);
    // ⚠ The two intrinsic-keyword sidecars are NOT symmetric in this engine — the inline axis
    // carries a full `Option<IntrinsicSize>` and the block axis a bare `height_intrinsic: bool`
    // (nothing consumed a block-axis `fit-content` before). Carry what can be carried and say so
    // rather than pretending the swap is lossless.
    let had_width_kw = s.width_keyword.is_some();
    s.width_keyword = s
        .height_intrinsic
        .then_some(manuk_css::IntrinsicSize::FitContent);
    s.height_intrinsic = had_width_kw;
    // An intrinsic ratio is width/height, and both names just changed meaning.
    if let Some(r) = s.aspect_ratio {
        if r > 0.0 {
            s.aspect_ratio = Some(1.0 / r);
        }
    }
    std::mem::swap(&mut s.transform_origin.0, &mut s.transform_origin.1);
}

#[derive(Clone, Copy, PartialEq)]
enum Side {
    Left,
    Right,
}

/// Rewrite one `Sides<T>` so that the transposed engine's `left`/`top` mean inline-start /
/// block-start.
///
/// The engine reads `left` as inline-start and `top` as block-start. In a vertical mode
/// inline-start is physically the TOP and block-start is the physical side named by `bs`.
fn swap_sides<T: Copy>(sides: &mut Sides<T>, bs: Side, be: Side) {
    let (t, r, b, l) = (sides.top, sides.right, sides.bottom, sides.left);
    let pick = |s: Side| if s == Side::Left { l } else { r };
    sides.left = t; // inline-start  ← physical top
    sides.right = b; // inline-end    ← physical bottom
    sides.top = pick(bs); // block-start   ← physical right (rl) / left (lr)
    sides.bottom = pick(be); // block-end     ← physical left  (rl) / right (lr)
}

/// The affine map from a transposed subtree's logical coordinates back into page coordinates.
#[derive(Clone, Copy, Debug)]
pub struct VerticalRun {
    /// Do blocks stack right-to-left? Decides which of the two rows in the module docs applies.
    pub rl: bool,
    /// `cx + cw` for `vertical-rl` (the block-start edge), `cx` for `vertical-lr`.
    pub bx: f32,
    /// The container content box's top — logical inline offsets are measured down from here.
    pub by: f32,
}

impl VerticalRun {
    /// Physical x of a logical block offset `ey` whose extent is `eh`.
    pub fn px(&self, ey: f32, eh: f32) -> f32 {
        if self.rl {
            self.bx - (ey + eh)
        } else {
            self.bx + ey
        }
    }

    /// Physical y of a logical inline offset.
    pub fn py(&self, ex: f32) -> f32 {
        self.by + ex
    }
}

/// Map a whole transposed subtree — boxes and the text runs inside them — into page coordinates.
///
/// Boxes get real physical rects, so `getBoundingClientRect`, hit-testing and the a11y tree all
/// read the truth. Text runs keep their LOGICAL fields and carry the map instead: a run's `width`
/// is an advance along the inline axis, which after the map is a *vertical* extent, and silently
/// re-pointing `x`/`width` at a different axis is how a field ends up meaning two things.
pub(crate) fn map_subtree(boxes: &mut [LayoutBox], v: VerticalRun) {
    for b in boxes.iter_mut() {
        let (ex, ey, ew, eh) = (b.rect.x, b.rect.y, b.rect.width, b.rect.height);
        b.rect.x = v.px(ey, eh);
        b.rect.y = v.py(ex);
        b.rect.width = eh;
        b.rect.height = ew;
        // The border widths ride with the box and were transposed in the STYLE, so the box the
        // engine built already has them in engine order — put them back on the physical edges.
        if let Some(bd) = b.border.as_mut() {
            unswap_quad(&mut bd.widths, v.rl);
            unswap_quad(&mut bd.colors, v.rl);
            unswap_quad(&mut bd.styles, v.rl);
        }
        match &mut b.content {
            BoxContent::Block(children) => map_subtree(children, v),
            BoxContent::Inline(frags) => {
                for f in frags.iter_mut() {
                    mark_vertical(f, v);
                }
            }
        }
    }
}

/// Mark one run as living in a vertical writing mode: it carries the map, and its style tells
/// paint to lay the glyphs on their side.
///
/// Both halves are needed and they are separate facts. The map is GEOMETRY — where the run's box
/// is, which `getBoundingClientRect` and the a11y tree read. `sideways` is PAINT — how the glyphs
/// are oriented inside it. Setting only the first is how a correct box comes to hold text running
/// off its side, which is a *visual* regression bought with a geometry win; the ratchet refuses
/// that trade, so they land together.
pub(crate) fn mark_vertical(f: &mut crate::TextFragment, v: VerticalRun) {
    f.vertical = Some(v);
    f.style.sideways = true;
}

/// The inverse of [`swap_sides`] — engine order (`left`=inline-start, `top`=block-start) back to
/// physical order, for the per-side arrays baked into a `LayoutBox` rather than read from a style.
///
/// `Border`'s three arrays are indexed `[top, right, bottom, left]` and are kept parallel on
/// purpose, so all three go through the same permutation and a mismatched pairing cannot be
/// written.
fn unswap_quad<T: Copy>(q: &mut [T; 4], rl: bool) {
    let (t, r, b, l) = (q[0], q[1], q[2], q[3]);
    q[0] = l; // physical top    ← inline-start
    q[2] = r; // physical bottom ← inline-end
    if rl {
        q[1] = t; // block-start is the right edge
        q[3] = b;
    } else {
        q[3] = t; // block-start is the left edge
        q[1] = b;
    }
}

/// How far the transposed content reached along the INLINE axis — which, after the map, is the
/// container's physical **height**.
///
/// `layout_children` reports the block extent (its `height` return), because in a horizontal world
/// that is the only axis that grows. The inline extent has to be read off the fragments, and it is
/// the number an auto-height vertical container is sized by: Chrome's `400x10` for a one-glyph
/// child is this function returning 10.
pub(crate) fn inline_extent(content: &BoxContent) -> f32 {
    let mut max = 0.0f32;
    match content {
        BoxContent::Block(children) => {
            for c in children {
                max = max.max(c.rect.x + c.rect.width);
                max = max.max(inline_extent(&c.content));
            }
        }
        BoxContent::Inline(frags) => {
            for f in frags {
                max = max.max(f.x + f.width);
            }
        }
    }
    max
}
