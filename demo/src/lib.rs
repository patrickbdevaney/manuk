//! **THE ENGINE, RUNNING IN THE VISITOR'S OWN BROWSER.**
//!
//! Not a screenshot. Not a video. Not a description. The visitor's browser downloads this wasm module and
//! **executes our actual pipeline** — html5ever's parser, **Stylo's cascade**, **Taffy's flex/grid**,
//! **tiny-skia's rasterizer**, and this engine's own DOM/layout/paint — then hands the pixels to a
//! `<canvas>`.
//!
//! ## What is real, and what is not — and the second list is why the first one is believable
//!
//! **Real:** the cascade is Stylo. The layout is Taffy plus our own inline/float/table code. The raster is
//! tiny-skia. Scroll is a real re-render at a new offset, not a CSS transform on a bitmap.
//!
//! **Not real, and stated in-product rather than buried here:**
//!   * **No JavaScript.** SpiderMonkey is C++ and does not target `wasm32-unknown-unknown`. That is not a
//!     shortcut — it is the reason the demo is JS-free, and pretending otherwise would be the one thing
//!     that makes the rest untrustworthy.
//!   * **No arbitrary URL fetching.** The pages are **bundled snapshots**, so what you see is what the
//!     engine does with a real document, not what a live site chose to serve a headless client.
//!
//! ## Threading
//!
//! **Single-threaded, deliberately.** One page under one cursor does not need Rayon's multi-tab
//! parallelism — and it is what keeps GitHub Pages hosting clean: **no `SharedArrayBuffer`, so no
//! COOP/COEP headers**, which a static host cannot set anyway.

use manuk_a11y::{A11yNode, Rect as A11yRect};
use manuk_css::Rgba;
use manuk_css::{MinimalCascade, StyleMap, Stylesheet};
use manuk_dom::{Dom, NodeId};
use manuk_layout::{layout_document, LayoutBox};
use manuk_paint::CpuPainter;
use manuk_text::FontContext;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

/// JSON-escape a string's control characters, quotes and backslashes. The inspector and the agent tree
/// carry page-authored text (an element's accessible name, a class attribute), and a stray `"` or `\` in
/// it corrupts the whole JSON payload — which is exactly the kind of silent, one-site-only break this
/// project is built to refuse.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// One rendered page, kept alive across scrolls so the visitor is scrolling a **laid-out document**, not
/// panning a picture of one.
#[wasm_bindgen]
pub struct Renderer {
    dom: Box<Dom>,
    styles: StyleMap,
    root: LayoutBox,
    fonts: FontContext,
    width: u32,
    /// Per-stage timings. **`Instant::now()` PANICS on wasm** (there is no clock), so the clock is the
    /// host's — `Date::now()` through `js_sys`. Coarse (ms), which is exactly the resolution these stages
    /// live at.
    t_parse: f64,
    t_cascade: f64,
    t_layout: f64,
}

/// `Instant::now()` **panics** on wasm (there is no clock), and `Date::now()` is coarse to 1ms — which
/// rounded every pipeline stage to "0ms" and made the provenance panel useless. `performance.now()` is the
/// host's high-resolution monotonic clock. *A measurement that cannot see the thing it measures is not a
/// measurement.*
fn now() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or(0.0)
}

#[wasm_bindgen]
impl Renderer {
    /// Parse → cascade → layout. The same three calls the native browser makes, in the same order.
    #[wasm_bindgen(constructor)]
    pub fn new(html: &str, width: u32) -> Renderer {
        console_error_panic_hook::set_once();
        let fonts = FontContext::new();
        // **wasm has no filesystem, so `load_system_fonts()` finds NOTHING and the engine has nothing to
        // draw text with.** The page laid out correctly and rendered blank — which is exactly the shape
        // of a font bug (`docs/wiki/text-layout.md`: a font problem never looks like a font problem).
        //
        // These are the **Liberation** faces — the very ones Chrome's `Arial`/`Times New Roman` requests
        // resolve to on Linux, and therefore the same metrics the native engine uses. Not a substitute
        // chosen for size; the same faces, so the demo's text measures like the real thing.
        register_bundled_fonts(&fonts);

        let t0 = now();
        let dom = Box::new(manuk_html::parse(html));
        let sheets: Vec<Stylesheet> = MinimalCascade::collect_style_elements(&dom);
        let t1 = now();

        manuk_css::values::set_viewport_width(width as f32);
        let (_, vh) = manuk_css::values::viewport_size();
        // Stylo. The real cascade — the whole point of the demo.
        let styles = manuk_css::stylo_engine::cascade_via_stylo(&dom, &sheets, width as f32, vh);
        let t2 = now();

        let root = layout_document(&dom, &styles, &fonts, width as f32);
        let t3 = now();

        Renderer {
            dom,
            styles,
            root,
            fonts,
            width,
            t_parse: t1 - t0,
            t_cascade: t2 - t1,
            t_layout: t3 - t2,
        }
    }

    /// The document's full laid-out height — what the page's scrollbar is measuring.
    pub fn content_height(&self) -> f32 {
        self.root.content_bottom()
    }

    pub fn title(&self) -> String {
        self.dom
            .find_first("title")
            .map(|t| self.dom.text_content(t))
            .unwrap_or_default()
    }

    /// Rasterize the viewport at `scroll_y` and return RGBA bytes for `ImageData`.
    ///
    /// **This re-rasterizes.** It does not translate a cached bitmap — which is why `position: fixed`,
    /// `sticky` and scroll-dependent painting behave like a browser rather than like a screenshot.
    pub fn render(&self, height: u32, scroll_y: f32) -> Vec<u8> {
        let z = self.z_index_map();
        let clip = self.clip_map();
        let images: HashMap<NodeId, std::rc::Rc<manuk_paint::DecodedImage>> = HashMap::new();
        let canvas = CpuPainter::with_layers(&self.fonts, &images, &z, &clip).render_scrolled(
            &self.root,
            self.width,
            height,
            self.canvas_background(),
            scroll_y,
        );
        canvas.rgba_bytes().to_vec()
    }

    /// **The engine's own numbers — the provenance panel.**
    ///
    /// These are not a description of the pipeline; they are the pipeline reporting on itself. A visitor
    /// can watch html5ever, Stylo and Taffy each cost what they cost, on a real document, on their machine.
    pub fn stats(&self) -> String {
        let shadow = self.dom.all_shadow_roots().len();
        let nodes = self.dom.len();
        let styled = self.styles.len();
        format!(
            "{{\"nodes\":{},\"styled\":{},\"shadowRoots\":{},\"contentHeight\":{:.0},\
             \"parseMs\":{:.1},\"cascadeMs\":{:.1},\"layoutMs\":{:.1}}}",
            nodes,
            styled,
            shadow,
            self.root.content_bottom(),
            self.t_parse,
            self.t_cascade,
            self.t_layout
        )
    }

    /// **INSPECT what is under the cursor — straight out of Stylo and Taffy, not a re-derivation.**
    ///
    /// This is the thing a wasm demo can do that a screenshot never can, and that Chromium's own DevTools
    /// cannot do for *this* engine: show the visitor the **actual computed style Stylo produced** and the
    /// **actual box Taffy solved**, for the element they are pointing at. The provenance IS the product.
    pub fn inspect(&self, x: f32, y: f32) -> String {
        let Some(n) = self.node_at(x, y) else {
            return String::new();
        };
        let rects = self.root.node_rects(&self.dom);
        let r = rects.get(&n).copied();
        let tag = self.dom.tag_name(n).unwrap_or("#text").to_string();
        let el = self.dom.element(n);
        let id = el.and_then(|e| e.attr("id")).unwrap_or("").to_string();
        let class = el.and_then(|e| e.attr("class")).unwrap_or("").to_string();
        let href = el.and_then(|e| e.attr("href")).unwrap_or("").to_string();

        // The DOM path — the same structural key the differential oracle uses to compare us to Chromium.
        let mut path = Vec::new();
        let mut cur = Some(n);
        while let Some(c) = cur {
            if let Some(t) = self.dom.tag_name(c) {
                path.push(t.to_string());
            }
            cur = self.dom.parent(c);
        }
        path.reverse();

        let (disp, pos, color, bg, fs) = match self.styles.get(&n) {
            Some(s) => (
                format!("{:?}", s.display).to_lowercase(),
                format!("{:?}", s.position).to_lowercase(),
                format!("#{:02x}{:02x}{:02x}", s.color.r, s.color.g, s.color.b),
                s.background_color
                    .map(|c| format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b))
                    .unwrap_or_else(|| "—".into()),
                s.font_size,
            ),
            None => ("—".into(), "—".into(), "—".into(), "—".into(), 0.0),
        };
        let (bx, by, bw, bh) = r
            .map(|r| (r.x, r.y, r.width, r.height))
            .unwrap_or((0.0, 0.0, 0.0, 0.0));
        let in_shadow = self.dom.all_shadow_roots().iter().any(|&sr| {
            let mut c = Some(n);
            while let Some(k) = c {
                if k == sr {
                    return true;
                }
                c = self.dom.parent(k);
            }
            false
        });

        format!(
            "{{\"tag\":\"{}\",\"id\":\"{}\",\"class\":\"{}\",\"href\":\"{}\",\
             \"path\":\"{}\",\"display\":\"{}\",\"position\":\"{}\",\"color\":\"{}\",\
             \"background\":\"{}\",\"fontSize\":{:.1},\"shadow\":{},\
             \"box\":[{:.1},{:.1},{:.1},{:.1}]}}",
            json_escape(&tag),
            json_escape(&id),
            json_escape(&class.chars().take(40).collect::<String>()),
            json_escape(&href),
            json_escape(&path.join(" › ")),
            disp,
            pos,
            color,
            bg,
            fs,
            in_shadow,
            bx,
            by,
            bw,
            bh
        )
    }

    /// The `href` under the cursor, if any — so the demo can NAVIGATE. A browser that cannot follow a
    /// link is a picture of a browser.
    pub fn link_at(&self, x: f32, y: f32) -> String {
        let mut cur = self.node_at(x, y);
        while let Some(n) = cur {
            if let Some(h) = self.dom.element(n).and_then(|e| e.attr("href")) {
                return h.to_string();
            }
            cur = self.dom.parent(n); // the click target is often a <span> INSIDE the <a>
        }
        String::new()
    }

    /// Which element is under the cursor — the **real hit-test against the laid-out boxes**, the same one
    /// a click goes through. A demo that faked this would be demonstrating nothing.
    pub fn hit_test(&self, x: f32, y: f32) -> String {
        match self.node_at(x, y) {
            Some(n) => {
                let tag = self.dom.tag_name(n).unwrap_or("?").to_string();
                let id = self
                    .dom
                    .element(n)
                    .and_then(|e| e.attr("id"))
                    .map(|i| format!("#{i}"))
                    .unwrap_or_default();
                format!("<{tag}>{id}")
            }
            None => String::new(),
        }
    }

    /// **THE AGENT'S VIEW — the accessibility tree an LLM drives, not a screenshot.**
    ///
    /// `role` + accessible `name` + interaction `state` + a click box, computed by `manuk-a11y` straight
    /// from the DOM and the laid-out fragment tree — the *same* structured observation channel the
    /// headless `agent` front-end feeds a model. It needs no JavaScript, which is exactly why it is
    /// honest even in this JS-free demo: what makes this a browser an agent can *drive* is the semantic
    /// tree, and the tree is a pure function of the parse and the layout.
    ///
    /// Returns a pre-order, semantically-pruned flat list — landmarks, headings, links, controls and any
    /// named element; the anonymous `<div>` skeleton is folded away, exactly as an agent's observation
    /// elides it. Each row carries its indentation `d`(epth), `role`, `name`, whether it is actionable
    /// (`act`), its rendered interaction `state`, and its absolute `box` (`null` when the element was
    /// never boxed — an agent has nowhere to click those).
    pub fn agent_tree(&self) -> String {
        let rects = self.a11y_rects();
        let z = self.z_index_map();
        let tree = manuk_a11y::build_tree_with_geometry(&self.dom, &rects, &z);
        let mut out = String::from("[");
        let mut idx: i32 = 0;
        emit_agent_node(&tree, 0, &mut out, &mut idx);
        out.push(']');
        out
    }

    /// The affordance census an agent uses to orient before it acts: how many actionable controls,
    /// landmark regions and headings the page actually exposes. The structured map, counted.
    pub fn agent_summary(&self) -> String {
        let rects = self.a11y_rects();
        let z = self.z_index_map();
        let tree = manuk_a11y::build_tree_with_geometry(&self.dom, &rects, &z);
        let (mut act, mut land, mut head, mut named, mut total) = (0, 0, 0, 0, 0);
        count_agent(
            &tree, &mut act, &mut land, &mut head, &mut named, &mut total,
        );
        format!(
            "{{\"interactive\":{},\"landmarks\":{},\"headings\":{},\"named\":{},\"nodes\":{}}}",
            act, land, head, named, total
        )
    }

    /// Every laid-out box, for the geometry overlay — proof the rectangles are *solved*, not a
    /// decorative grid drawn over a bitmap. Each entry is `[x, y, w, h, depth]`; `depth` (nesting in the
    /// fragment tree) drives the overlay's colour, so the structure of the layout is legible at a glance.
    pub fn layout_boxes(&self) -> String {
        let rects = self.root.node_rects(&self.dom);
        let depth = self.depth_map();
        let mut out = String::from("[");
        let mut first = true;
        for (n, r) in &rects {
            if r.width <= 0.0 || r.height <= 0.0 {
                continue;
            }
            if !first {
                out.push(',');
            }
            first = false;
            out.push_str(&format!(
                "[{:.0},{:.0},{:.0},{:.0},{}]",
                r.x,
                r.y,
                r.width,
                r.height,
                depth.get(n).copied().unwrap_or(0)
            ));
        }
        out.push(']');
        out
    }
}

impl Renderer {
    /// The laid-out boxes, converted into `manuk-a11y`'s dependency-lean `Rect` (the a11y crate stays
    /// DOM-only so the agent and the screen-reader bridge can use it without the layout stack).
    fn a11y_rects(&self) -> HashMap<NodeId, A11yRect> {
        self.root
            .node_rects(&self.dom)
            .into_iter()
            .map(|(n, r)| {
                (
                    n,
                    A11yRect {
                        x: r.x,
                        y: r.y,
                        width: r.width,
                        height: r.height,
                    },
                )
            })
            .collect()
    }

    /// Nesting depth per DOM node (one pre-order walk) — colours the geometry overlay.
    fn depth_map(&self) -> HashMap<NodeId, i32> {
        let mut m = HashMap::new();
        let mut stack = vec![(self.dom.root(), 0i32)];
        while let Some((n, d)) = stack.pop() {
            m.insert(n, d);
            for c in self.dom.children(n) {
                stack.push((c, d + 1));
            }
        }
        m
    }

    /// Deepest-wins over the LAID-OUT boxes — the same rects a click resolves against. Ties break toward
    /// the SMALLER box, because a lone `<button>` inside a same-size `<form>` must hit the button (that tie
    /// once resolved toward the ancestor, and the click landed on the form).
    fn node_at(&self, x: f32, y: f32) -> Option<NodeId> {
        let rects = self.root.node_rects(&self.dom);
        let mut best: Option<(NodeId, f32)> = None;
        for (n, r) in &rects {
            if x >= r.x && x <= r.x + r.width && y >= r.y && y <= r.y + r.height {
                let area = r.width * r.height;
                if best.map(|(_, a)| area <= a).unwrap_or(true) {
                    best = Some((*n, area));
                }
            }
        }
        best.map(|(n, _)| n)
    }
}

/// The fonts, compiled INTO the wasm. Roughly 1.5 MB, and non-negotiable: an engine with no font renders
/// a perfectly-laid-out blank page.
fn register_bundled_fonts(fonts: &FontContext) {
    for data in [
        &include_bytes!("../fonts/LiberationSans-Regular.ttf")[..],
        &include_bytes!("../fonts/LiberationSans-Bold.ttf")[..],
        &include_bytes!("../fonts/LiberationSerif-Regular.ttf")[..],
        &include_bytes!("../fonts/LiberationMono-Regular.ttf")[..],
    ] {
        fonts.register_font(data.to_vec());
    }
}

/// Truncate to `max` characters (char-safe), with an ellipsis — an accessible name can be a whole
/// paragraph, and the agent tree wants the identifying head of it, not the essay.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let head: String = s.chars().take(max).collect();
        format!("{head}…")
    }
}

/// Pre-order emit of the semantically-pruned agent tree (see [`Renderer::agent_tree`]). A node is kept
/// if it is actionable, has an accessible name, or carries a real ARIA role — the anonymous-container
/// skeleton is folded away and its meaningful descendants re-attach at the reduced depth, which is the
/// same elision the headless agent's observation channel performs.
fn emit_agent_node(n: &A11yNode, depth: i32, out: &mut String, idx: &mut i32) {
    use manuk_a11y::Role;
    let keep = n.role.is_interactive() || !n.name.is_empty() || n.role != Role::Generic;
    if keep {
        if *idx > 0 {
            out.push(',');
        }
        let state = n.state.render();
        let state = state.trim().trim_start_matches('[').trim_end_matches(']');
        let bx = match n.bbox {
            Some(r) => format!("[{:.0},{:.0},{:.0},{:.0}]", r.x, r.y, r.width, r.height),
            None => "null".to_string(),
        };
        out.push_str(&format!(
            "{{\"i\":{},\"d\":{},\"role\":\"{}\",\"name\":\"{}\",\"act\":{},\"state\":\"{}\",\"box\":{}}}",
            *idx,
            depth,
            n.role.as_str(),
            json_escape(&truncate(&n.name, 80)),
            n.role.is_interactive(),
            json_escape(state),
            bx
        ));
        *idx += 1;
    }
    let child_depth = if keep { depth + 1 } else { depth };
    for c in &n.children {
        emit_agent_node(c, child_depth, out, idx);
    }
}

/// Tally the affordance census over the whole tree (see [`Renderer::agent_summary`]).
fn count_agent(
    n: &A11yNode,
    act: &mut i32,
    land: &mut i32,
    head: &mut i32,
    named: &mut i32,
    total: &mut i32,
) {
    use manuk_a11y::Role;
    *total += 1;
    if n.role.is_interactive() {
        *act += 1;
    }
    if !n.name.is_empty() {
        *named += 1;
    }
    if matches!(n.role, Role::Heading { .. }) {
        *head += 1;
    }
    if matches!(
        n.role,
        Role::Banner
            | Role::Navigation
            | Role::Main
            | Role::Complementary
            | Role::ContentInfo
            | Role::Region
            | Role::Search
            | Role::Form
            | Role::Article
    ) {
        *land += 1;
    }
    for c in &n.children {
        count_agent(c, act, land, head, named, total);
    }
}

impl Renderer {
    fn canvas_background(&self) -> Rgba {
        // The root's background paints the WHOLE canvas, and if the root has none, `<body>`'s propagates
        // up to it. Skip this and every dark-themed page is a dark box floating in a white void.
        for tag in ["html", "body"] {
            if let Some(n) = self.dom.find_first(tag) {
                if let Some(bg) = self.styles.get(&n).and_then(|s| s.background_color) {
                    if bg.a > 0 {
                        return bg;
                    }
                }
            }
        }
        Rgba {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        }
    }

    fn z_index_map(&self) -> HashMap<NodeId, i32> {
        use manuk_css::Position;
        let mut map = HashMap::new();
        let mut stack = vec![(self.dom.root(), 0i32)];
        while let Some((n, inherited)) = stack.pop() {
            let z = match self.styles.get(&n) {
                Some(s) if s.position != Position::Static => s.z_index.unwrap_or(0),
                _ => inherited,
            };
            map.insert(n, z);
            for c in self.dom.children(n) {
                stack.push((c, z));
            }
        }
        map
    }

    fn clip_map(&self) -> HashMap<NodeId, manuk_layout::Rect> {
        use manuk_css::Overflow;
        let rects = self.root.node_rects(&self.dom);
        let mut map = HashMap::new();
        for (n, r) in &rects {
            if let Some(s) = self.styles.get(n) {
                if s.overflow == Overflow::Hidden {
                    map.insert(*n, *r);
                }
            }
        }
        map
    }
}
