//! **Inline-SVG child geometry — the other half of the tick-393 build spec.**
//!
//! Chrome gives every element *inside* an `<svg>` its own box: `getBoundingClientRect` on a
//! `<path>` returns the path's fill bounds, mapped through the `viewBox` transform into CSS pixels.
//! We laid the `<svg>` out atomically and then let CSS layout loose on its subtree, which produced
//! the worst of both answers — measured on `www.desitales2.com`, every icon `<path>` reported
//!
//! ```text
//!     Chrome  [40 758 12×12]        ours  [40 758 0×22]
//! ```
//!
//! — a **zero-width inline box one line-height tall**, because `<path>` computes `display:inline`,
//! has no text, and an inline box with no text is exactly that. It is not a near miss; it is a
//! number from the wrong formatting model entirely. That is cluster `Ccd7f` in the oracle ledger:
//! **34 sites, 1,658 hits**, the third-ranked geometry family on the board.
//!
//! ⚠ **The fix is NOT to drop the boxes.** Deleting them turns a *misplaced* element into a
//! *missing* one, which trades SHAPE for COVERAGE — the board ranks MISSING_BOX as the worse of the
//! two, so that "fix" is a regression wearing a smaller number. The boxes have to become *right*.
//!
//! **Borrowed, not hand-written** (the spec's whole point): `usvg` is already in the tree decoding
//! `<img src="*.svg">` and inline `<svg>` alike, and it resolves path data, `transform` stacks,
//! `viewBox` and per-node absolute bounding boxes as part of an ordinary parse. Writing a path
//! parser with bezier-extrema and a transform stack by hand is the wrong rung of the ladder.
//!
//! ## What this refuses to answer, and why refusing is the point
//!
//! usvg's tree is a *rendering* tree, not the DOM: it drops `<defs>`/`<title>`, expands `<use>`,
//! and synthesises groups for clips and opacity. So DOM↔usvg correspondence is a **guess**, and a
//! wrong guess silently mis-attributes one shape's bounds to another element — a number that looks
//! plausible and is false, which is worse than the honest 0×22 it replaced. Every constructor here
//! therefore returns `None` rather than pair anything it cannot pair exactly:
//!
//! * the leaf counts must match (usvg paths vs DOM shape elements), else no mapping at all;
//! * `<foreignObject>` anywhere in the subtree → no mapping (it holds real HTML whose boxes CSS
//!   layout owns, and this pass would delete them);
//! * the used box's aspect ratio must match the resolved user-space one (`preserveAspectRatio`
//!   letterboxing is not modelled, so a mismatched aspect means our origin would be wrong).
//!
//! When it refuses, the pre-existing CSS-inline boxes stand. That is the ratchet: a site this
//! cannot map is left exactly as good as it was.
//!
//! **Named residue** (not modelled, deliberately): `padding`/`border` on the `<svg>` itself (the
//! mapping uses the border box as the viewport, which is the same rect for every `<svg>` on the web
//! that has neither) and `preserveAspectRatio` other than a matching aspect. **`<use>`
//! cross-references LANDED at t743** — see [`external_use_defs`] and [`use_leaf_count`]; they were
//! listed here as unmodelled for 350 ticks.

use manuk_dom::{Dom, NodeId};
use manuk_layout::{BoxContent, LayoutBox, Rect};
use std::collections::HashMap;

/// The shape elements that produce a `usvg::Node::Path`, in the order a DOM walk meets them.
///
/// `<image>` and `<text>` are here too because usvg emits a leaf for each; leaving them out would
/// desynchronise the pairing rather than merely skip them, and a desynchronised pairing is the one
/// failure this module exists to avoid.
const SHAPE_TAGS: &[&str] = &[
    "path", "rect", "circle", "ellipse", "line", "polyline", "polygon", "image", "text",
];

/// Subtrees usvg never renders **in place**. Their shapes produce no leaf where they are written,
/// so a DOM walk that descended into them would count more shapes than usvg has and the whole svg
/// would (correctly, but needlessly) be refused.
///
/// ⚠ *In place* is the whole qualification: `<use href="#sym">` renders a `<symbol>`'s contents at
/// the `<use>`'s position. Referenced, they are leaves; written, they are not. That is why
/// [`count_referenced_leaves`] starts **at** the target and does not consult this list for it.
const NON_RENDERED_CONTAINERS: &[&str] = &[
    "defs",
    "clippath",
    "mask",
    "symbol",
    "marker",
    "pattern",
    "lineargradient",
    "radialgradient",
    "filter",
    "metadata",
];

fn lower_tag(dom: &Dom, n: NodeId) -> String {
    dom.tag_name(n).unwrap_or("").to_ascii_lowercase()
}

fn is_use(dom: &Dom, n: NodeId) -> bool {
    dom.is_element(n) && lower_tag(dom, n) == "use"
}

/// Every `id` in the document, resolved once.
///
/// ⚠ **Built rather than scanned because the population makes it quadratic.** A `<use>` resolves
/// against the *whole document*, and the sites that use the idiom use it hundreds of times —
/// `apnews.com` ships **314** `<use href="#…">` in its initial HTML. A per-reference
/// `descendants(root).find(…)` is `O(uses × nodes)` on exactly the pages this feature exists for,
/// which is how a fidelity fix turns into a load-time regression. Keys are owned so the index does
/// not hold a borrow of the DOM across the caller's own mutations.
pub struct IdIndex(HashMap<String, NodeId>);

impl IdIndex {
    pub fn build(dom: &Dom) -> Self {
        let mut map = HashMap::new();
        for n in dom.descendants(dom.root()) {
            if let Some(id) = dom.element(n).and_then(|e| e.id()) {
                // First wins, which is what every id-based resolver on the web does with a
                // duplicate.
                map.entry(id.to_string()).or_insert(n);
            }
        }
        Self(map)
    }

    fn get(&self, id: &str) -> Option<NodeId> {
        self.0.get(id).copied()
    }
}

/// Follow a `<use>`'s `href` (or the legacy `xlink:href`) to the element it names, **anywhere in
/// the document**.
///
/// Document-wide is the point, and it is the whole bug: the sprite idiom puts the definitions in
/// one `<svg>` and the `<use>` in another, so a resolver scoped to the referencing `<svg>` finds
/// nothing on the exact markup the web ships icons in.
fn resolve_use_target(dom: &Dom, ids: &IdIndex, use_node: NodeId) -> Option<NodeId> {
    let el = dom.element(use_node)?;
    let href = el.attr("href").or_else(|| el.attr("xlink:href"))?;
    let id = href.trim().strip_prefix('#')?;
    if id.is_empty() {
        return None;
    }
    // An external file reference (`icons.svg#check`) is a fetch this pass does not do; `strip_prefix`
    // already excluded it, and a same-document id is what the sprite idiom uses.
    ids.get(id)
}

/// How deep a `<use>` → `<use>` chain is followed before the reference is abandoned. The SVG spec
/// forbids cycles; a hostile or generated document does not have to obey, and an unbounded walk
/// here is a hang, which outranks every geometry number on the board.
const MAX_USE_DEPTH: u32 = 8;

/// How many usvg leaves the element `target` contributes when it is *referenced*.
///
/// Starts **at** `target`: a referenced `<symbol>` or `<g>` renders its children even though the
/// same element renders nothing where it is written.
fn count_referenced_leaves(dom: &Dom, ids: &IdIndex, target: NodeId, depth: u32) -> usize {
    if depth > MAX_USE_DEPTH || !dom.is_element(target) {
        return 0;
    }
    let tag = lower_tag(dom, target);
    if tag == "foreignobject" {
        // Refusing is `dom_shapes`'s job; contributing a count we cannot honour is not this one's.
        return 0;
    }
    if SHAPE_TAGS.contains(&tag.as_str()) {
        return 1;
    }
    if tag == "use" {
        return match resolve_use_target(dom, ids, target) {
            Some(t) => count_referenced_leaves(dom, ids, t, depth + 1),
            None => 0,
        };
    }
    dom.children(target)
        .filter(|&c| dom.is_element(c))
        .map(|c| {
            let ct = lower_tag(dom, c);
            if NON_RENDERED_CONTAINERS.contains(&ct.as_str()) {
                0
            } else {
                count_referenced_leaves(dom, ids, c, depth)
            }
        })
        .sum()
}

/// The leaves one `<use>` element expands to — `0` for a dangling reference, which is exactly what
/// usvg emits for one and is the reason this cannot be "one leaf per `<use>`".
fn use_leaf_count(dom: &Dom, ids: &IdIndex, use_node: NodeId) -> usize {
    match resolve_use_target(dom, ids, use_node) {
        Some(t) => count_referenced_leaves(dom, ids, t, 1),
        None => 0,
    }
}

/// The `<defs>` block an inline `<svg>` needs appended before usvg can resolve its `<use>`
/// references — the definitions that live **outside** this `<svg>` element.
///
/// ⚠⚠ **This is the half that makes the icon PAINT, not merely measure.** `decode_inline_svgs`
/// serialises one `<svg>` element and hands that string to usvg, so a `<use href="#check">` naming
/// a `<symbol>` in the page's sprite block is dangling *by construction* — usvg drops it, the
/// raster is empty, and the icon is not drawn at all. Every geometry number downstream is then a
/// measurement of a blank.
///
/// Returns `None` when nothing needs injecting, so the overwhelmingly common `<use>`-free `<svg>`
/// pays one subtree scan and no string work.
pub fn external_use_defs(dom: &Dom, ids: &IdIndex, svg: NodeId) -> Option<String> {
    let mut queue: Vec<NodeId> = dom.descendants(svg).filter(|&n| is_use(dom, n)).collect();
    if queue.is_empty() {
        return None;
    }
    // Everything already serialised as part of this `<svg>`, plus everything already injected —
    // emitting a node twice would put a duplicate id in front of usvg, and a duplicate id makes the
    // *resolver* pick one arbitrarily. Same reason a nested target is skipped: its ancestor carried
    // it in.
    let mut covered: std::collections::HashSet<NodeId> = dom.descendants(svg).collect();
    covered.insert(svg);
    let mut emitted: Vec<NodeId> = Vec::new();
    let mut steps = 0u32;
    while let Some(u) = queue.pop() {
        steps += 1;
        if steps > 1024 {
            break;
        }
        let Some(t) = resolve_use_target(dom, ids, u) else {
            continue;
        };
        if covered.contains(&t) {
            continue;
        }
        for d in dom.descendants(t) {
            covered.insert(d);
            if is_use(dom, d) {
                queue.push(d);
            }
        }
        covered.insert(t);
        emitted.push(t);
    }
    if emitted.is_empty() {
        return None;
    }
    let body: String = emitted
        .iter()
        .map(|&n| manuk_html::serialize_outer(dom, n))
        .collect();
    // `<defs>` is what keeps the injection invisible: the definitions must be reachable by id and
    // must render NOTHING where they are pasted, or every sprite would paint its whole sheet.
    Some(format!("<defs>{body}</defs>"))
}

/// One `<svg>` element's interior, resolved.
#[derive(Clone, Debug)]
pub struct SvgBoxes {
    /// The size usvg resolved for the document (`Tree::size()`) — the space `rects` are expressed
    /// in.
    ///
    /// ⚠ **`abs_bounding_box()` is in CANVAS space, not `viewBox` units** — measured, because the
    /// obvious reading is the wrong one and the first version of the gate asserted it. For
    /// `<svg width="12" height="12" viewBox="0 0 24 24"><path d="M6 6 H18 V18 H6 Z"/>`, usvg reports
    /// the path at **[3 3 6×6]**, not [6 6 12×12]: `Tree::size()` is 12×12 and the `viewBox`
    /// transform is already folded into the absolute transforms. So the only scale left to apply is
    /// `used_box / user_size`, and when the author sized the svg in px (the overwhelming majority)
    /// that scale is exactly 1.
    pub user_size: (f32, f32),
    /// Each DOM descendant paired to its bounding box in `user_size` space, as `[x, y, w, h]`.
    /// Containers (`<g>`, and the `<svg>` itself is excluded) carry the union of their children.
    pub rects: Vec<(NodeId, [f32; 4])>,
}

/// Every inline `<svg>` this page could map, keyed by the `<svg>` element.
pub type SvgGeometry = HashMap<NodeId, SvgBoxes>;

/// Collect usvg's rendered leaves in document order.
fn usvg_leaves(group: &resvg::usvg::Group, out: &mut Vec<Rect>) {
    for node in group.children() {
        match node {
            resvg::usvg::Node::Group(g) => usvg_leaves(g, out),
            other => {
                // ⚠ **`getBoundingClientRect` IS THE *DECORATED* BOUNDING BOX — IT INCLUDES THE
                //    STROKE.** SVG 2 gives an element two different boxes and the DOM methods return
                //    different ones: `getBBox()` is the fill/geometry box, and
                //    `getBoundingClientRect()` is the transformed *decorated* box. The oracle probes
                //    every element with `getBoundingClientRect`, so the stroke box is the one being
                //    compared against.
                //
                //    This is not cosmetic and it is not free to guess: `abs_bounding_box()` (fill
                //    only) matched `desitales2` EXACTLY — 7×12, 10×9, 12×12 against Chrome — because
                //    its icons are unstroked paths, for which the two boxes are identical. So the
                //    site that motivated the work could not distinguish them, and the fill box
                //    measured a −0.3 SHAPE regression on `en.wikipedia.org`, whose icons ARE stroked.
                //    A fixture that cannot tell two rules apart has not chosen between them.
                let b = other.abs_stroke_bounding_box();
                out.push(Rect {
                    x: b.x(),
                    y: b.y(),
                    width: b.width(),
                    height: b.height(),
                });
            }
        }
    }
}

/// Collect the DOM shape elements under `svg` in document order, and separately the container
/// elements with the index range of the shapes they contain (so a `<g>` can take their union).
///
/// Returns `None` if the subtree holds a `<foreignObject>` — see the module note.
///
/// The third list is the elements that render **nothing** and still have a box: a `<use>` whose
/// reference does not resolve. Chrome gives it a zero-area box at the `<svg>`'s origin, and
/// reporting that is not a formality — dropping it entirely would trade a wrong number for a
/// missing one, which the cluster ledger ranks as the worse of the two.
fn dom_shapes(
    dom: &Dom,
    ids: &IdIndex,
    svg: NodeId,
) -> Option<(
    Vec<NodeId>,
    Vec<(NodeId, std::ops::Range<usize>)>,
    Vec<NodeId>,
)> {
    let mut shapes = Vec::new();
    let mut containers = Vec::new();
    let mut zero = Vec::new();
    let mut ok = true;
    fn walk(
        dom: &Dom,
        ids: &IdIndex,
        n: NodeId,
        shapes: &mut Vec<NodeId>,
        containers: &mut Vec<(NodeId, std::ops::Range<usize>)>,
        zero: &mut Vec<NodeId>,
        ok: &mut bool,
    ) {
        for c in dom.children(n) {
            if !dom.is_element(c) {
                continue;
            }
            let tag = dom.tag_name(c).unwrap_or("").to_ascii_lowercase();
            if tag == "foreignobject" {
                *ok = false;
                return;
            }
            if NON_RENDERED_CONTAINERS.contains(&tag.as_str()) {
                continue;
            }
            if SHAPE_TAGS.contains(&tag.as_str()) {
                shapes.push(c);
                // A `<text>`'s children are tspans usvg folds into the one leaf; do not descend.
                continue;
            }
            if tag == "use" {
                // ⚠ **A `<use>` is not one leaf, and that is not a detail.** usvg expands the
                // reference, so a `<use>` naming a two-shape `<g>` emits TWO leaves and one naming
                // nothing emits none. Pushing the `<use>`'s own id once per leaf keeps the pairing
                // positional — which is the invariant this whole module is built on — and lets the
                // rect builder fold the run into the union `getBoundingClientRect` returns.
                let k = use_leaf_count(dom, ids, c);
                if k == 0 {
                    zero.push(c);
                }
                for _ in 0..k {
                    shapes.push(c);
                }
                continue;
            }
            let start = shapes.len();
            walk(dom, ids, c, shapes, containers, zero, ok);
            if !*ok {
                return;
            }
            containers.push((c, start..shapes.len()));
        }
    }
    walk(
        dom,
        ids,
        svg,
        &mut shapes,
        &mut containers,
        &mut zero,
        &mut ok,
    );
    ok.then_some((shapes, containers, zero))
}

/// Resolve one inline `<svg>`'s interior from its serialized markup.
///
/// `markup` is the same string `decode_inline_svgs` hands to the rasteriser, so the two halves can
/// never disagree about what document they are describing.
pub fn map_inline_svg(dom: &Dom, ids: &IdIndex, svg: NodeId, markup: &str) -> Option<SvgBoxes> {
    let tree =
        resvg::usvg::Tree::from_data(markup.as_bytes(), &resvg::usvg::Options::default()).ok()?;
    let size = tree.size();
    let (uw, uh) = (size.width(), size.height());
    if !(uw.is_finite() && uh.is_finite()) || uw <= 0.0 || uh <= 0.0 {
        return None;
    }

    let mut leaves = Vec::new();
    usvg_leaves(tree.root(), &mut leaves);
    let (shapes, containers, zero) = dom_shapes(dom, ids, svg)?;

    // **The pairing guard.** Not a heuristic with a fallback — a fallback here is precisely the
    // "plausible value" that turns one wrong box into thirty.
    if leaves.len() != shapes.len() || shapes.is_empty() {
        return None;
    }

    // One rect per DOM element, not per leaf: a `<use>` occupies a RUN of the pairing (one entry
    // per leaf its expansion emitted) and its box is that run's union — which is the box Chrome
    // reports for a `<use>` of a multi-shape group. For every other element the run is length 1 and
    // this is the zip it replaced.
    let mut rects: Vec<(NodeId, [f32; 4])> = Vec::new();
    let mut i = 0;
    while i < shapes.len() {
        let n = shapes[i];
        let mut j = i + 1;
        while j < shapes.len() && shapes[j] == n {
            j += 1;
        }
        let mut acc = leaves[i];
        for r in &leaves[i + 1..j] {
            acc = acc.union(r);
        }
        rects.push((n, [acc.x, acc.y, acc.width, acc.height]));
        i = j;
    }
    // A container's box is the union of the shapes it holds — which is what `getBoundingClientRect`
    // on a `<g>` returns, and it costs one fold over a range we already have.
    for (n, range) in containers {
        let mut acc: Option<Rect> = None;
        for r in &leaves[range] {
            acc = Some(match acc {
                Some(a) => a.union(r),
                None => *r,
            });
        }
        if let Some(a) = acc {
            rects.push((n, [a.x, a.y, a.width, a.height]));
        }
    }
    // A `<use>` that resolves to nothing renders nothing and still HAS a box — Chrome-measured,
    // zero-area at the viewport origin. See `dom_shapes`.
    rects.extend(zero.into_iter().map(|n| (n, [0.0, 0.0, 0.0, 0.0])));
    Some(SvgBoxes {
        user_size: (uw, uh),
        rects,
    })
}

/// How far the used box's aspect may drift from the resolved user-space aspect before the mapping
/// is refused. `preserveAspectRatio` letterboxing is not modelled, so beyond this the origin would
/// be offset by an amount this pass cannot compute.
const ASPECT_TOLERANCE: f32 = 0.02;

/// Replace each mapped `<svg>` box's CSS-derived interior with the real SVG geometry.
///
/// Runs **after** layout, on the finished fragment tree, and deliberately so: it needs the `<svg>`'s
/// used box (only layout knows it) and it changes no input layout ever reads, so it cannot perturb
/// the geometry of anything outside an `<svg>`. That is what makes it a strictly-additive pass.
pub fn apply(root: &mut LayoutBox, geom: &SvgGeometry) -> usize {
    if geom.is_empty() {
        return 0;
    }
    let mut mapped = 0usize;
    root.walk_mut(&mut |b| {
        let Some(node) = b.node else { return };
        let Some(boxes) = geom.get(&node) else {
            return;
        };
        let (uw, uh) = boxes.user_size;
        let (w, h) = (b.rect.width, b.rect.height);
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let (sx, sy) = (w / uw, h / uh);
        if (sx - sy).abs() > ASPECT_TOLERANCE * sx.max(sy) {
            return;
        }
        let kids = boxes
            .rects
            .iter()
            .map(|&(n, [x, y, cw, ch])| {
                LayoutBox::inert(
                    Rect {
                        x: b.rect.x + x * sx,
                        y: b.rect.y + y * sy,
                        width: cw * sx,
                        height: ch * sy,
                    },
                    n,
                )
            })
            .collect();
        b.content = BoxContent::Block(kids);
        mapped += 1;
    });
    mapped
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole mechanism on the shape the web actually ships: a 24-unit `viewBox` displayed at
    /// 12px, holding one path that fills the middle half of it.
    ///
    /// Measured over headless Chrome (`--headless=new`, 1200×800) rather than recalled — the
    /// `<path>` reports **12×12 at the svg's origin** for a full-viewBox path, and 6×6 at +3,+3 for
    /// the half-size one below. The number this asserts is Chrome's.
    #[test]
    fn a_path_reports_its_own_bounds_scaled_by_the_viewbox() {
        let markup = r#"<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24"><path d="M6 6 H18 V18 H6 Z"/></svg>"#;
        let dom = manuk_html::parse(&format!("<div>{markup}</div>"));
        let svg = dom
            .descendants(dom.root())
            .find(|&n| dom.tag_name(n) == Some("svg"))
            .expect("svg in the tree");
        let path = dom
            .descendants(dom.root())
            .find(|&n| dom.tag_name(n) == Some("path"))
            .expect("path in the tree");

        let boxes = map_inline_svg(&dom, &IdIndex::build(&dom), svg, markup)
            .expect("a one-path svg must map");
        assert_eq!(
            boxes.rects.len(),
            1,
            "one DOM shape must pair with one usvg leaf, got {:?}",
            boxes.rects
        );
        let (n, r) = boxes.rects[0];
        assert_eq!(n, path);
        assert_eq!(
            (boxes.user_size.0 as i32, boxes.user_size.1 as i32),
            (12, 12),
            "`Tree::size()` is the width/height attributes, not the viewBox"
        );
        // The path spans 6..18 of a 24-unit viewBox — half the box, centred — and usvg reports it
        // ALREADY mapped through that viewBox into the 12×12 canvas: [3 3 6×6]. See `user_size`.
        assert!(
            (r[0] - 3.0).abs() < 0.01 && (r[2] - 6.0).abs() < 0.01,
            "bounds must arrive in CANVAS space (3..9 of 12), not viewBox units (6..18 of 24), \
             got {r:?}"
        );

        // Now the half this exists for: the used box is 12px, so the path must land at +3 and be 6px.
        let mut root = LayoutBox::inert(
            Rect {
                x: 40.0,
                y: 758.0,
                width: 12.0,
                height: 12.0,
            },
            svg,
        );
        let geom: SvgGeometry = [(svg, boxes)].into_iter().collect();
        assert_eq!(apply(&mut root, &geom), 1, "the svg box must be mapped");
        let rects = root.node_rects(&dom);
        let got = rects.get(&path).expect("the path must now have a rect");
        assert!(
            (got.x - 43.0).abs() < 0.01
                && (got.y - 761.0).abs() < 0.01
                && (got.width - 6.0).abs() < 0.01
                && (got.height - 6.0).abs() < 0.01,
            "a path filling 6..18 of a 24-unit viewBox shown at 12px is 6×6 at the svg's origin +3; \
             got [{} {} {}×{}]",
            got.x,
            got.y,
            got.width,
            got.height
        );
    }

    /// **RED-proof for the pairing guard.** usvg drops `<defs>`, so a DOM walk that counted the
    /// shapes inside one would find more shapes than leaves — and pairing by index would hand the
    /// visible path the *defs* path's bounds. Skipping non-rendered containers is what keeps the
    /// counts equal; if that skip is removed this returns `None` and the site keeps its old boxes.
    #[test]
    fn a_defs_subtree_does_not_desynchronise_the_pairing() {
        let markup = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><defs><path id="d" d="M0 0 H2 V2 H0 Z"/></defs><path d="M6 6 H18 V18 H6 Z"/></svg>"#;
        let dom = manuk_html::parse(&format!("<div>{markup}</div>"));
        let svg = dom
            .descendants(dom.root())
            .find(|&n| dom.tag_name(n) == Some("svg"))
            .expect("svg");
        let boxes = map_inline_svg(&dom, &IdIndex::build(&dom), svg, markup)
            .expect("a defs sibling must not stop the visible path from mapping");
        assert_eq!(
            boxes.rects.len(),
            1,
            "only the RENDERED path pairs; the one in <defs> has no usvg leaf"
        );
        let (_, r) = boxes.rects[0];
        assert!(
            (r[0] - 6.0).abs() < 0.01,
            "the visible path must get ITS OWN bounds (x=6), not the defs path's (x=0) — got {r:?}"
        );
    }

    /// **The aspect guard.** `preserveAspectRatio` letterboxing is not modelled, so a used box whose
    /// aspect does not match the resolved one would place every child at a wrong origin. Refuse,
    /// and leave the CSS boxes standing.
    #[test]
    fn a_mismatched_aspect_is_refused_rather_than_placed_wrong() {
        let markup = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><path d="M6 6 H18 V18 H6 Z"/></svg>"#;
        let dom = manuk_html::parse(&format!("<div>{markup}</div>"));
        let svg = dom
            .descendants(dom.root())
            .find(|&n| dom.tag_name(n) == Some("svg"))
            .expect("svg");
        let geom: SvgGeometry = [(
            svg,
            map_inline_svg(&dom, &IdIndex::build(&dom), svg, markup).expect("map"),
        )]
        .into_iter()
        .collect();
        let mut square = LayoutBox::inert(
            Rect {
                x: 0.0,
                y: 0.0,
                width: 48.0,
                height: 48.0,
            },
            svg,
        );
        assert_eq!(apply(&mut square, &geom), 1, "a 1:1 box maps");
        let mut stretched = LayoutBox::inert(
            Rect {
                x: 0.0,
                y: 0.0,
                width: 48.0,
                height: 12.0,
            },
            svg,
        );
        assert_eq!(
            apply(&mut stretched, &geom),
            0,
            "a 4:1 box against a 1:1 document must be REFUSED — placing the children as if the \
             scale were uniform is a wrong number that looks right"
        );
    }

    /// **RED-proof for the refusal.** A `<foreignObject>` holds real HTML whose boxes CSS layout
    /// owns; this pass replaces an svg box's children wholesale, so mapping such an svg would
    /// DELETE that HTML's geometry. It must refuse, and the old boxes must stand.
    #[test]
    fn a_foreign_object_refuses_the_whole_svg() {
        let markup = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><path d="M6 6 H18 V18 H6 Z"/><foreignObject width="10" height="10"><div>hi</div></foreignObject></svg>"#;
        let dom = manuk_html::parse(&format!("<div>{markup}</div>"));
        let svg = dom
            .descendants(dom.root())
            .find(|&n| dom.tag_name(n) == Some("svg"))
            .expect("svg");
        assert!(
            map_inline_svg(&dom, &IdIndex::build(&dom), svg, markup).is_none(),
            "an svg holding HTML must be refused outright — its <div> has a CSS box this pass \
             would otherwise delete"
        );
    }

    /// A container takes the union of the shapes it holds, which is what Chrome reports for a `<g>`.
    #[test]
    fn a_group_is_the_union_of_its_shapes() {
        let markup = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><g><path d="M2 2 H6 V6 H2 Z"/><path d="M14 14 H22 V22 H14 Z"/></g></svg>"#;
        let dom = manuk_html::parse(&format!("<div>{markup}</div>"));
        let svg = dom
            .descendants(dom.root())
            .find(|&n| dom.tag_name(n) == Some("svg"))
            .expect("svg");
        let g = dom
            .descendants(dom.root())
            .find(|&n| dom.tag_name(n) == Some("g"))
            .expect("g");
        let boxes = map_inline_svg(&dom, &IdIndex::build(&dom), svg, markup).expect("map");
        let (_, r) = boxes
            .rects
            .iter()
            .find(|(n, _)| *n == g)
            .expect("the <g> must get a rect too");
        assert!(
            (r[0] - 2.0).abs() < 0.01
                && (r[1] - 2.0).abs() < 0.01
                && (r[2] - 20.0).abs() < 0.01
                && (r[3] - 20.0).abs() < 0.01,
            "the group spans both paths: 2..22 on both axes; got {r:?}"
        );
    }
}
