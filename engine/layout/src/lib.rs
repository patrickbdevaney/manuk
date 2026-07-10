//! manuk-layout — the layout engine.
//!
//! Per CLAUDE.md: `taffy` for flexbox/grid, plus **from-scratch** block, inline,
//! table, positioned, and float layout verified against WPT layout reftests. This
//! first pass implements the two formatting contexts that carry the web —
//! **block** (normal-flow vertical stacking) and **inline** (line-breaking of text
//! and inline boxes) — and routes `display:flex` through `taffy` (see [`flex`]).
//!
//! Table / floats / abs-positioning are the documented next reftests. The output is
//! a **fragment tree** ([`LayoutBox`]) with absolute px rects that paint consumes.
//!
//! Known simplifications (documented, not silent — CLAUDE.md § verification):
//! - Vertical margins do not collapse yet.
//! - Percentage heights resolve only against definite containers.
//! - Inline layout is Latin/LTR and inserts an inter-word space between adjacent
//!   tokens (so `a<b>b</b>` gains a space it should not); Parley-grade segmentation
//!   is the upgrade.

use manuk_css::{ComputedStyle, Dim, Display, Rgba, StyleMap, TextAlign};
use manuk_dom::{Dom, NodeData, NodeId};
use manuk_text::{FontContext, FontFamily, FontKey};

pub mod flex;

/// An axis-aligned rectangle in absolute document px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub const ZERO: Rect = Rect {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    };
}

/// The visual style of a text run, resolved for shaping + paint.
#[derive(Clone, Copy, Debug)]
pub struct TextStyle {
    pub font_key: FontKey,
    pub font_size: f32,
    pub color: Rgba,
    pub line_height: f32,
}

/// A positioned run of text produced by inline layout. `baseline` is the absolute
/// y of the text baseline; paint places glyphs relative to it.
#[derive(Clone, Debug)]
pub struct TextFragment {
    pub x: f32,
    pub line_top: f32,
    pub baseline: f32,
    pub text: String,
    pub style: TextStyle,
}

/// Contents of a laid-out box.
#[derive(Clone, Debug)]
pub enum BoxContent {
    /// Block-level children (already absolutely positioned).
    Block(Vec<LayoutBox>),
    /// An inline formatting context: laid-out line text.
    Inline(Vec<TextFragment>),
}

/// A node in the fragment tree: an absolute border-box rect plus contents.
#[derive(Clone, Debug)]
pub struct LayoutBox {
    /// Border box in absolute coordinates.
    pub rect: Rect,
    pub background: Option<Rgba>,
    /// The DOM node this box came from, if any (anonymous boxes are `None`).
    pub node: Option<NodeId>,
    pub content: BoxContent,
}

impl LayoutBox {
    /// Visit every box in the fragment tree (pre-order), calling `f` on each.
    pub fn walk(&self, f: &mut impl FnMut(&LayoutBox)) {
        f(self);
        if let BoxContent::Block(children) = &self.content {
            for c in children {
                c.walk(f);
            }
        }
    }

    /// The full document height this box occupies (max bottom edge in its subtree).
    pub fn content_bottom(&self) -> f32 {
        let mut max = self.rect.y + self.rect.height;
        self.walk(&mut |b| {
            max = max.max(b.rect.y + b.rect.height);
            if let BoxContent::Inline(frags) = &b.content {
                for fr in frags {
                    max = max.max(fr.baseline + fr.style.font_size);
                }
            }
        });
        max
    }
}

/// Shared inputs for a layout pass.
struct Ctx<'a> {
    dom: &'a Dom,
    styles: &'a StyleMap,
    fonts: &'a FontContext,
}

/// Lay out a whole document into a fragment tree, given a viewport width in px.
///
/// The root box is `<body>` (falling back to `<html>` or the first element), laid
/// out in an initial containing block of `viewport_width`.
pub fn layout_document(
    dom: &Dom,
    styles: &StyleMap,
    fonts: &FontContext,
    viewport_width: f32,
) -> LayoutBox {
    let ctx = Ctx { dom, styles, fonts };
    let root_el = dom
        .find_first("body")
        .or_else(|| dom.find_first("html"))
        .or_else(|| dom.children(dom.root()).find(|&n| dom.is_element(n)));

    match root_el {
        Some(el) => ctx.layout_block(el, viewport_width, 0.0, 0.0, 0.0).boxx,
        None => LayoutBox {
            rect: Rect::ZERO,
            background: None,
            node: None,
            content: BoxContent::Block(vec![]),
        },
    }
}

/// Is `node` a block-level box in its parent's formatting context?
fn is_block_level(dom: &Dom, styles: &StyleMap, node: NodeId) -> bool {
    if let NodeData::Element(_) = dom.data(node) {
        matches!(
            styles.get(&node).map(|s| s.display),
            Some(Display::Block | Display::Flex | Display::Grid)
        )
    } else {
        false
    }
}

fn is_rendered(dom: &Dom, styles: &StyleMap, node: NodeId) -> bool {
    match dom.data(node) {
        NodeData::Element(_) => {
            !matches!(styles.get(&node).map(|s| s.display), Some(Display::None))
        }
        NodeData::Text(_) => true,
        _ => false,
    }
}

fn text_style(cs: &ComputedStyle) -> TextStyle {
    TextStyle {
        font_key: FontKey {
            family: FontFamily::SansSerif,
            bold: cs.font_weight >= 600,
            italic: cs.italic,
        },
        font_size: cs.font_size,
        color: cs.color,
        line_height: cs.line_height,
    }
}

/// The pieces a parent needs to stack a block child with margin collapsing.
struct BlockResult {
    boxx: LayoutBox,
    /// This block's top margin (already applied to `boxx.rect.y`, reported so a
    /// parent-child collapse could use it later).
    margin_top: f32,
    /// This block's bottom margin — the parent collapses it with the next sibling's
    /// top margin (or applies it fully before non-collapsible content).
    margin_bottom: f32,
}

/// Collapse two adjoining vertical margins (CSS2 §8.3.1): positive margins take the
/// max, negative margins take the min (most negative), mixed signs sum. Passing `0`
/// for one side yields the other unchanged, so the first-in-flow block "collapses"
/// with a zero and keeps its own margin.
fn collapse_margins(a: f32, b: f32) -> f32 {
    if a >= 0.0 && b >= 0.0 {
        a.max(b)
    } else if a < 0.0 && b < 0.0 {
        a.min(b)
    } else {
        a + b
    }
}

impl Ctx<'_> {
    /// Lay out a block box in a containing block of `cw` px. `y` is the border-bottom
    /// edge of the preceding in-flow sibling (or the container's content-top for the
    /// first child); `prev_margin` is that sibling's trailing collapsible margin (0
    /// if none). The block's top margin collapses with `prev_margin` to decide its
    /// border-box top. Returns the positioned box and its own top/bottom margins.
    ///
    /// Simplification (documented, CLAUDE.md § verification): parent↔first/last-child
    /// margin collapsing is not yet modeled — only adjacent-sibling collapsing is.
    fn layout_block(&self, node: NodeId, cw: f32, x: f32, y: f32, prev_margin: f32) -> BlockResult {
        let s = self.styles[&node].clone();

        let mut ml = s.margin.left.resolve(cw, 0.0);
        let mr = s.margin.right.resolve(cw, 0.0);
        let mt = s.margin.top.resolve(cw, 0.0);
        let mb = s.margin.bottom.resolve(cw, 0.0);
        let (pl, pr) = (
            s.padding.left.resolve(cw, 0.0),
            s.padding.right.resolve(cw, 0.0),
        );
        let (pt, pb) = (
            s.padding.top.resolve(cw, 0.0),
            s.padding.bottom.resolve(cw, 0.0),
        );
        let (bl, br) = (s.border_width.left, s.border_width.right);
        let (bt, bb) = (s.border_width.top, s.border_width.bottom);

        // Resolve width. `auto` fills the available inline space.
        let extra = ml + mr + pl + pr + bl + br;
        let mut width = match s.width {
            Dim::Auto => (cw - extra).max(0.0),
            other => other.resolve(cw, (cw - extra).max(0.0)),
        };
        width = width.max(0.0);

        // Horizontal auto-margin centering when width is definite. Only the left
        // margin shifts the box; the right margin absorbs the remainder implicitly.
        if s.width != Dim::Auto {
            let leftover = cw - (width + pl + pr + bl + br);
            match (s.margin.left.is_auto(), s.margin.right.is_auto()) {
                (true, true) => ml = (leftover / 2.0).max(0.0),
                (true, false) => ml = (leftover - mr).max(0.0),
                _ => {}
            }
        }
        let _ = mr; // right margin does not affect downstream positioning here

        let border_x = x + ml;
        // Collapse this block's top margin with the preceding sibling's trailing
        // margin to place the border-box top.
        let border_y = y + collapse_margins(prev_margin, mt);
        let content_x = border_x + bl + pl;
        let content_y = border_y + bt + pt;

        let (content, content_height) = self.layout_children(node, content_x, content_y, width);
        let content_height = match s.height {
            Dim::Auto => content_height,
            other => other.resolve(0.0, content_height),
        };

        let border_box_w = bl + pl + width + pr + br;
        let border_box_h = bt + pt + content_height + pb + bb;
        let rect = Rect {
            x: border_x,
            y: border_y,
            width: border_box_w,
            height: border_box_h,
        };

        BlockResult {
            boxx: LayoutBox {
                rect,
                background: s.background_color,
                node: Some(node),
                content,
            },
            margin_top: mt,
            margin_bottom: mb,
        }
    }

    /// Lay out the children of a container whose content box starts at `(cx, cy)`
    /// with content width `cw`. Returns the content and its height.
    fn layout_children(&self, node: NodeId, cx: f32, cy: f32, cw: f32) -> (BoxContent, f32) {
        let display = self.styles[&node].display;
        let kids: Vec<NodeId> = self
            .dom
            .children(node)
            .filter(|&k| is_rendered(self.dom, self.styles, k))
            .collect();

        // Flex containers route through taffy.
        if display == Display::Flex {
            return self.layout_flex(cx, cy, cw, &kids);
        }

        let has_block = kids
            .iter()
            .any(|&k| is_block_level(self.dom, self.styles, k));
        if !has_block {
            // Pure inline formatting context.
            let items = self.collect_inline_group(&kids);
            let align = self.styles[&node].text_align;
            let (frags, h) = self.layout_inline(&items, cx, cy, cw, align);
            return (BoxContent::Inline(frags), h);
        }

        // Block container: block children stack with adjacent-sibling margin
        // collapsing; runs of inline siblings become anonymous block boxes. `cur_y`
        // tracks the border-bottom of the last in-flow block (its trailing margin
        // held in `prev_margin` so the next sibling can collapse against it).
        let mut boxes = Vec::new();
        let mut cur_y = cy;
        let mut prev_margin = 0.0f32;
        let mut inline_run: Vec<NodeId> = Vec::new();

        for &k in &kids {
            if is_block_level(self.dom, self.styles, k) {
                (cur_y, prev_margin) =
                    self.flush_inline_run(&mut inline_run, &mut boxes, cx, cur_y, prev_margin, cw);
                let r = self.layout_block(k, cw, cx, cur_y, prev_margin);
                cur_y = r.boxx.rect.y + r.boxx.rect.height;
                prev_margin = r.margin_bottom;
                boxes.push(r.boxx);
            } else {
                inline_run.push(k);
            }
        }
        (cur_y, prev_margin) =
            self.flush_inline_run(&mut inline_run, &mut boxes, cx, cur_y, prev_margin, cw);

        // The last in-flow block's trailing margin still occupies the container.
        (BoxContent::Block(boxes), cur_y + prev_margin - cy)
    }

    /// Turn a pending run of inline-level siblings into an anonymous block box.
    /// Returns the updated `(cur_y, prev_margin)`: a whitespace-only run produces no
    /// box and preserves the pending block margin (so `<p>a</p>\n<p>b</p>` still
    /// collapses); real inline content is not collapsible, so the pending margin is
    /// committed before it.
    fn flush_inline_run(
        &self,
        run: &mut Vec<NodeId>,
        boxes: &mut Vec<LayoutBox>,
        cx: f32,
        cur_y: f32,
        prev_margin: f32,
        cw: f32,
    ) -> (f32, f32) {
        if run.is_empty() {
            return (cur_y, prev_margin);
        }
        let items = self.collect_inline_group(run);
        run.clear();
        if items.is_empty() {
            return (cur_y, prev_margin); // whitespace-only: keep the pending margin
        }
        let start = cur_y + prev_margin;
        let (frags, h) = self.layout_inline(&items, cx, start, cw, TextAlign::Left);
        boxes.push(LayoutBox {
            rect: Rect {
                x: cx,
                y: start,
                width: cw,
                height: h,
            },
            background: None,
            node: None,
            content: BoxContent::Inline(frags),
        });
        (start + h, 0.0)
    }

    /// Lay out flex children as a row using taffy for main-axis sizing/positioning.
    /// Each child is then laid out as a block within its taffy-assigned slot.
    fn layout_flex(&self, cx: f32, cy: f32, cw: f32, kids: &[NodeId]) -> (BoxContent, f32) {
        let block_kids: Vec<NodeId> = kids
            .iter()
            .copied()
            .filter(|&k| self.dom.is_element(k))
            .collect();
        if block_kids.is_empty() {
            return (BoxContent::Block(vec![]), 0.0);
        }
        let items: Vec<flex::FlexItem> = block_kids
            .iter()
            .map(|&k| {
                let s = &self.styles[&k];
                flex::FlexItem {
                    width: match s.width {
                        Dim::Px(p) => Some(p),
                        _ => None,
                    },
                    height: match s.height {
                        Dim::Px(p) => Some(p),
                        _ => None,
                    },
                    grow: if s.width == Dim::Auto { 1.0 } else { 0.0 },
                }
            })
            .collect();
        let slots = flex::solve_row(cw, &items);

        let mut boxes = Vec::new();
        let mut max_h = 0.0f32;
        for (&k, slot) in block_kids.iter().zip(slots.iter()) {
            let r = self.layout_block(k, slot.width, cx + slot.x, cy, 0.0);
            // Flex item advance = its full margin box height.
            let adv = r.margin_top + (r.boxx.rect.height) + r.margin_bottom;
            max_h = max_h.max(adv);
            boxes.push(r.boxx);
        }
        (BoxContent::Block(boxes), max_h)
    }

    /// Collect inline tokens (words) from a run of inline-level siblings, tracking
    /// inter-word spacing.
    fn collect_inline_group(&self, nodes: &[NodeId]) -> Vec<InlineItem> {
        let mut out = Vec::new();
        let mut pending_space = false;
        let mut first = true;
        for &n in nodes {
            self.collect_inline_node(n, &mut out, &mut pending_space, &mut first);
        }
        out
    }

    fn collect_inline_node(
        &self,
        node: NodeId,
        out: &mut Vec<InlineItem>,
        pending_space: &mut bool,
        first: &mut bool,
    ) {
        match self.dom.data(node) {
            NodeData::Text(t) => {
                let style = text_style(&self.styles[&node]);
                let mut buf = String::new();
                for ch in t.chars() {
                    if ch.is_whitespace() {
                        if !buf.is_empty() {
                            push_word(out, &mut buf, style, pending_space, first);
                        }
                        *pending_space = true;
                    } else {
                        buf.push(ch);
                    }
                }
                if !buf.is_empty() {
                    push_word(out, &mut buf, style, pending_space, first);
                }
            }
            NodeData::Element(_) => {
                if self.styles.get(&node).map(|s| s.display) == Some(Display::None) {
                    return;
                }
                let children: Vec<NodeId> = self.dom.children(node).collect();
                for c in children {
                    self.collect_inline_node(c, out, pending_space, first);
                }
            }
            _ => {}
        }
    }

    /// Greedy line-breaking of inline items across `cw` px. Returns fragments (with
    /// absolute positions) and the total inline block height.
    fn layout_inline(
        &self,
        items: &[InlineItem],
        cx: f32,
        cy: f32,
        cw: f32,
        align: TextAlign,
    ) -> (Vec<TextFragment>, f32) {
        // Build lines as fragment-builders, deferring y until line metrics are known.
        struct FB {
            x: f32,
            text: String,
            style: TextStyle,
            ascent: f32,
            descent: f32,
        }
        let mut lines: Vec<Vec<FB>> = Vec::new();
        let mut cur: Vec<FB> = Vec::new();
        let mut pen = 0.0f32;

        for item in items {
            let key = item.style.font_key;
            let size = item.style.font_size;
            let lm = self.fonts.line_metrics(key, size);
            let word_w = self.fonts.measure(&item.text, key, size);
            let space_w = if item.space_before {
                self.fonts.measure(" ", key, size)
            } else {
                0.0
            };

            if !cur.is_empty() && pen + space_w + word_w > cw {
                lines.push(std::mem::take(&mut cur));
                pen = 0.0;
                cur.push(FB {
                    x: 0.0,
                    text: item.text.clone(),
                    style: item.style,
                    ascent: lm.ascent,
                    descent: lm.descent,
                });
                pen += word_w;
            } else {
                cur.push(FB {
                    x: pen + space_w,
                    text: item.text.clone(),
                    style: item.style,
                    ascent: lm.ascent,
                    descent: lm.descent,
                });
                pen += space_w + word_w;
            }
        }
        if !cur.is_empty() {
            lines.push(cur);
        }

        let mut frags = Vec::new();
        let mut y = cy;
        for line in &lines {
            let ascent = line.iter().map(|f| f.ascent).fold(0.0, f32::max);
            let descent = line.iter().map(|f| f.descent).fold(0.0, f32::max);
            let pref = line.iter().map(|f| f.style.line_height).fold(0.0, f32::max);
            let content_h = ascent + descent;
            let line_h = pref.max(content_h);
            let leading = (line_h - content_h) / 2.0;
            let baseline = y + leading + ascent;

            let line_width = line
                .last()
                .map(|f| {
                    f.x + self
                        .fonts
                        .measure(&f.text, f.style.font_key, f.style.font_size)
                })
                .unwrap_or(0.0);
            let offset = match align {
                TextAlign::Center => (cw - line_width).max(0.0) / 2.0,
                TextAlign::Right => (cw - line_width).max(0.0),
                _ => 0.0,
            };

            for f in line {
                frags.push(TextFragment {
                    x: cx + offset + f.x,
                    line_top: y,
                    baseline,
                    text: f.text.clone(),
                    style: f.style,
                });
            }
            y += line_h;
        }

        (frags, y - cy)
    }
}

/// An inline token: one word plus whether whitespace preceded it.
struct InlineItem {
    text: String,
    style: TextStyle,
    space_before: bool,
}

fn push_word(
    out: &mut Vec<InlineItem>,
    buf: &mut String,
    style: TextStyle,
    pending_space: &mut bool,
    first: &mut bool,
) {
    out.push(InlineItem {
        text: std::mem::take(buf),
        style,
        space_before: *pending_space && !*first,
    });
    *first = false;
    *pending_space = false;
}

#[cfg(test)]
mod tests {
    use super::*;
    use manuk_css::{MinimalCascade, StyleEngine, Stylesheet};

    fn layout_html(html: &str, css: &str, width: f32) -> (Dom, LayoutBox) {
        let dom = manuk_html::parse(html);
        let sheets = vec![Stylesheet::parse(css)];
        let styles = MinimalCascade.cascade(&dom, &sheets);
        let fonts = FontContext::new();
        let root = layout_document(&dom, &styles, &fonts, width);
        (dom, root)
    }

    #[test]
    fn blocks_stack_vertically() {
        let (_dom, root) = layout_html(
            "<body><div style='height:50px'></div><div style='height:30px'></div></body>",
            "",
            800.0,
        );
        // body has 8px UA margin; its two block children stack.
        let BoxContent::Block(children) = &root.content else {
            panic!("expected block content");
        };
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].rect.height, 50.0);
        assert_eq!(children[1].rect.height, 30.0);
        // Second div starts below the first.
        assert!(children[1].rect.y >= children[0].rect.y + 50.0);
    }

    #[test]
    fn adjacent_sibling_margins_collapse() {
        // bottom:20 meets top:30 → the gap is max(20,30)=30, not 50.
        let (_dom, root) = layout_html(
            "<body><div style='height:10px;margin:0 0 20px 0'></div>\
             <div style='height:10px;margin:30px 0 0 0'></div></body>",
            "",
            800.0,
        );
        let BoxContent::Block(children) = &root.content else {
            panic!("expected block content");
        };
        assert_eq!(children.len(), 2);
        let gap = children[1].rect.y - (children[0].rect.y + children[0].rect.height);
        assert!(
            (gap - 30.0).abs() < 0.01,
            "collapsed gap should be 30, got {gap}"
        );
    }

    #[test]
    fn margins_do_not_collapse_across_inline_content() {
        // A text line between two blocks blocks the collapse; both margins apply.
        let (_dom, root) = layout_html(
            "<body><div style='height:10px;margin-bottom:20px'></div>hi\
             <div style='height:10px;margin-top:30px'></div></body>",
            "",
            800.0,
        );
        let BoxContent::Block(children) = &root.content else {
            panic!("expected block content");
        };
        // div, anonymous(inline "hi"), div
        assert_eq!(children.len(), 3);
        // The trailing 20px margin is committed before the inline box.
        let after_first = children[0].rect.y + children[0].rect.height;
        assert!(
            children[1].rect.y >= after_first + 20.0 - 0.01,
            "inline box should sit below the first div's full bottom margin"
        );
    }

    #[test]
    fn text_wraps_to_multiple_lines() {
        // Narrow container forces wrapping.
        let (_dom, root) = layout_html(
            "<body><p>the quick brown fox jumps over the lazy dog again and again</p></body>",
            "p{margin:0}",
            80.0,
        );
        let mut line_tops = std::collections::BTreeSet::new();
        root.walk(&mut |b| {
            if let BoxContent::Inline(frags) = &b.content {
                for f in frags {
                    line_tops.insert(f.line_top as i32);
                }
            }
        });
        assert!(line_tops.len() > 1, "text should wrap onto multiple lines");
    }

    #[test]
    fn document_height_grows_with_content() {
        let (_dom, tall) = layout_html("<body><div style='height:500px'></div></body>", "", 800.0);
        let (_dom2, short) = layout_html("<body><div style='height:20px'></div></body>", "", 800.0);
        assert!(tall.content_bottom() > short.content_bottom() + 400.0);
    }

    #[test]
    fn centered_text_is_offset() {
        let (_dom, root) = layout_html(
            "<body><p style='text-align:center'>hi</p></body>",
            "p{margin:0}",
            800.0,
        );
        let mut first_x = None;
        root.walk(&mut |b| {
            if let BoxContent::Inline(frags) = &b.content {
                if let Some(f) = frags.first() {
                    first_x.get_or_insert(f.x);
                }
            }
        });
        assert!(
            first_x.unwrap() > 100.0,
            "centered text should be pushed right"
        );
    }
}
