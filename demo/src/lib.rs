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

use manuk_css::Rgba;
use manuk_css::{MinimalCascade, StyleMap, Stylesheet};
use manuk_dom::{Dom, NodeId};
use manuk_layout::{layout_document, LayoutBox};
use manuk_paint::CpuPainter;
use manuk_text::FontContext;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

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
            tag,
            id,
            class.chars().take(40).collect::<String>(),
            href,
            path.join(" › "),
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
}

impl Renderer {
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
