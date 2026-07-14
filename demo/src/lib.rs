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
        let dom = Box::new(manuk_html::parse(html));
        let sheets: Vec<Stylesheet> = MinimalCascade::collect_style_elements(&dom);

        manuk_css::values::set_viewport_width(width as f32);
        let (_, vh) = manuk_css::values::viewport_size();
        // Stylo. The real cascade — the whole point of the demo.
        let styles = manuk_css::stylo_engine::cascade_via_stylo(&dom, &sheets, width as f32, vh);
        let root = layout_document(&dom, &styles, &fonts, width as f32);

        Renderer {
            dom,
            styles,
            root,
            fonts,
            width,
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

    /// Which element is under the cursor — the **real hit-test against the laid-out boxes**, the same one
    /// a click goes through. A demo that faked this would be demonstrating nothing.
    pub fn hit_test(&self, x: f32, y: f32) -> String {
        // Deepest-wins over the LAID-OUT boxes — the same rects a click resolves against. Ties break
        // toward the SMALLER box, because a lone `<button>` inside a same-size `<form>` must hit the
        // button (that tie once resolved toward the ancestor, and the click landed on the form).
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
        match best {
            Some((n, _)) => {
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
