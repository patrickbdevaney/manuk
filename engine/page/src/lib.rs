//! manuk-page — the shared page pipeline.
//!
//! `bytes → DOM → style → layout → paint`, wired end to end. This is the common
//! engine core CLAUDE.md calls for: the **headful shell** and the **headless
//! agent** both drive these functions and diverge only at how they consume the
//! output — the shell presents to a window, the agent screenshots + reads it.

/// N1 — the one session-history model, shared by the shell, the agent, and BiDi.
/// Re-export the shared session-history model (moved to `manuk-dom` to break the
/// page↔js dependency cycle). `manuk_page::history::SessionHistory` still resolves.
pub use manuk_dom::history;

use std::collections::HashMap;

use anyhow::{Context, Result};
use manuk_css::{
    diff_style, MinimalCascade, RestyleDamage, Rgba, StyleEngine, StyleMap, Stylesheet,
};
use manuk_dom::{Dom, NodeId};
use manuk_layout::{layout_document, BoxContent, LayoutBox};
use manuk_paint::{Canvas, CpuPainter, Painter};
use manuk_text::FontContext;
use url::Url;

/// A hyperlink discovered in the page, with its href resolved to an absolute URL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub text: String,
    pub href: String,
}

/// The kind of external resource a page references, with its load semantics — the
/// data a WHATWG-ordered fetch scheduler / preload scanner (D4) operates on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubresourceKind {
    /// `<link rel="stylesheet">` — **render-blocking**: fetched and applied before
    /// first paint (see [`Page::apply_stylesheets`]).
    Stylesheet,
    /// `<script src>` — `defer`/`async` drive scheduling. Fetching retrieves the text;
    /// execution is the JS path (`manuk-js`, feature-gated) and is not done here.
    Script { defer: bool, is_async: bool },
    /// `<img src>` — fetched for decode/paint (image rendering is a follow-on).
    Image,
}

/// An external subresource, its kind + load semantics, and its resolved absolute URL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subresource {
    pub kind: SubresourceKind,
    pub url: String,
}

/// The result of a streaming load: the optional **first paint** (a layout of the
/// head-complete partial document, available before the tail streams in) and the
/// finished [`Page`].
pub struct StreamingLoad {
    pub first_paint: Option<LayoutBox>,
    pub page: Page,
}

/// One entry in a page's ordered stylesheet list — an inline `<style>` body or an
/// external `<link>` URL — preserving document order for correct cascade precedence.
#[derive(Clone, Debug, PartialEq, Eq)]
enum StyleSource {
    Inline(String),
    External(String),
}

/// Resolve `href` against `base` to an absolute URL string (falling back to `href`).
fn resolve_url(base: &str, href: &str) -> String {
    Url::parse(base)
        .ok()
        .and_then(|b| b.join(href).ok())
        .map(|u| u.to_string())
        .unwrap_or_else(|| href.to_string())
}

/// Fetch + decode every `<img src>` in the tree into a node→bitmap map. Failures are
/// skipped (the element keeps its box, empty of pixels). Natural sizing is applied by the
/// caller from each [`DecodedImage`]'s dimensions.
async fn fetch_images(
    dom: &Dom,
    base: &str,
) -> std::collections::HashMap<manuk_dom::NodeId, std::rc::Rc<manuk_paint::DecodedImage>> {
    use std::rc::Rc;
    let mut out = std::collections::HashMap::new();
    let targets: Vec<(manuk_dom::NodeId, String)> = dom
        .descendants(dom.root())
        .filter(|&n| dom.tag_name(n) == Some("img"))
        .filter_map(|n| {
            let src = dom.element(n)?.attr("src")?.trim().to_string();
            if src.is_empty() {
                None
            } else {
                Some((n, resolve_url(base, &src)))
            }
        })
        .collect();
    for (node, url) in targets {
        let Some(bytes) = fetch_image_bytes(&url).await else {
            continue;
        };
        // Try raster decode; fall back to SVG (usvg/resvg) for vector sources.
        let decoded = match image::load_from_memory(&bytes) {
            Ok(img) => {
                let rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                Some(manuk_paint::DecodedImage {
                    width: w,
                    height: h,
                    rgba: rgba.into_raw(),
                })
            }
            Err(_) => decode_svg(&bytes, &url),
        };
        if let Some(img) = decoded {
            if img.width > 0 && img.height > 0 {
                out.insert(node, Rc::new(img));
            }
        }
    }
    out
}

/// Rasterize an SVG document to a [`DecodedImage`] (non-premultiplied RGBA) at its
/// intrinsic size via usvg + resvg. `None` if the bytes are not valid SVG.
fn decode_svg(bytes: &[u8], url: &str) -> Option<manuk_paint::DecodedImage> {
    // Quick reject non-SVG-looking bytes (unless the URL says .svg) to avoid the parse cost.
    let looks_svg = url.ends_with(".svg")
        || bytes.windows(4).take(512).any(|w| w == b"<svg")
        || bytes.starts_with(b"<?xml");
    if !looks_svg {
        return None;
    }
    let tree = resvg::usvg::Tree::from_data(bytes, &resvg::usvg::Options::default()).ok()?;
    let size = tree.size();
    let (w, h) = (size.width().ceil() as u32, size.height().ceil() as u32);
    if w == 0 || h == 0 || w > 8192 || h > 8192 {
        return None;
    }
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)?;
    resvg::render(&tree, resvg::tiny_skia::Transform::identity(), &mut pixmap.as_mut());
    // resvg output is premultiplied; store straight-alpha RGBA for our blitter.
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for px in pixmap.pixels() {
        let c = px.demultiply();
        rgba.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    Some(manuk_paint::DecodedImage {
        width: w,
        height: h,
        rgba,
    })
}

/// Fetch the raw bytes of an image URL: `data:` (base64 or literal), `http(s)://`, or a
/// local `file://`/path (for the render CLI on local pages).
async fn fetch_image_bytes(url: &str) -> Option<Vec<u8>> {
    if let Some(rest) = url.strip_prefix("data:") {
        let comma = rest.find(',')?;
        let data = &rest[comma + 1..];
        return if rest[..comma].contains("base64") {
            use base64::Engine;
            base64::engine::general_purpose::STANDARD.decode(data).ok()
        } else {
            // Non-base64 data URLs are percent-encoded (e.g. `%23` for `#` in inline SVG).
            Some(percent_decode(data))
        };
    }
    if url.starts_with("http://") || url.starts_with("https://") {
        let resp = manuk_net::fetch(url).await.ok()?;
        if resp.status >= 400 {
            return None;
        }
        return Some(resp.body.to_vec());
    }
    let path = url.strip_prefix("file://").unwrap_or(url);
    std::fs::read(path).ok()
}

/// Fetch a web-font URL and return OpenType/TrueType bytes fontdb can load. Raw TTF/OTF
/// (and TrueType collections) pass through; WOFF/WOFF2 are skipped (no working pure-Rust
/// WOFF2 decompressor builds in the current ecosystem — a documented gap).
async fn fetch_font_bytes(url: &str) -> Option<Vec<u8>> {
    let bytes = fetch_image_bytes(url).await?; // reuses data:/http(s)/file handling
    let sig = bytes.get(..4)?;
    // sfnt magics: TrueType (0x00010000 / "true"), OpenType ("OTTO"), collection ("ttcf").
    let is_sfnt = matches!(sig, b"\x00\x01\x00\x00" | b"true" | b"OTTO" | b"ttcf");
    is_sfnt.then_some(bytes)
}

/// Collect the page's stylesheet sources (inline + external) in document order.
fn collect_style_sources(dom: &Dom, base: &str) -> Vec<StyleSource> {
    let mut out = Vec::new();
    for n in dom.descendants(dom.root()) {
        match dom.tag_name(n) {
            Some("style") => out.push(StyleSource::Inline(dom.text_content(n))),
            Some("link") => {
                if let Some(el) = dom.element(n) {
                    let is_sheet = el
                        .attr("rel")
                        .map(|r| {
                            r.split_ascii_whitespace()
                                .any(|t| t.eq_ignore_ascii_case("stylesheet"))
                        })
                        .unwrap_or(false);
                    if is_sheet {
                        if let Some(href) = el.attr("href").map(str::trim).filter(|h| !h.is_empty())
                        {
                            out.push(StyleSource::External(resolve_url(base, href)));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// Enumerate all external subresources (`<link rel=stylesheet>`, `<script src>`,
/// `<img src>`) in document order — the scheduler's work-list.
fn collect_subresources(dom: &Dom, base: &str) -> Vec<Subresource> {
    let attr = |n: NodeId, name: &str| {
        dom.element(n)
            .and_then(|e| e.attr(name))
            .map(str::to_string)
    };
    let has = |n: NodeId, name: &str| {
        dom.element(n)
            .map(|e| e.attr(name).is_some())
            .unwrap_or(false)
    };
    let mut out = Vec::new();
    for n in dom.descendants(dom.root()) {
        match dom.tag_name(n) {
            Some("link") => {
                let is_sheet = attr(n, "rel")
                    .map(|r| {
                        r.split_ascii_whitespace()
                            .any(|t| t.eq_ignore_ascii_case("stylesheet"))
                    })
                    .unwrap_or(false);
                if is_sheet {
                    if let Some(href) = attr(n, "href").filter(|h| !h.trim().is_empty()) {
                        out.push(Subresource {
                            kind: SubresourceKind::Stylesheet,
                            url: resolve_url(base, href.trim()),
                        });
                    }
                }
            }
            Some("script") => {
                if let Some(src) = attr(n, "src").filter(|s| !s.trim().is_empty()) {
                    out.push(Subresource {
                        kind: SubresourceKind::Script {
                            defer: has(n, "defer"),
                            is_async: has(n, "async"),
                        },
                        url: resolve_url(base, src.trim()),
                    });
                }
            }
            Some("img") => {
                if let Some(src) = attr(n, "src").filter(|s| !s.trim().is_empty()) {
                    out.push(Subresource {
                        kind: SubresourceKind::Image,
                        url: resolve_url(base, src.trim()),
                    });
                }
            }
            _ => {}
        }
    }
    out
}

/// A loaded, styled, laid-out page. Retains the DOM + computed styles so it can be
/// re-laid-out at a new width (window resize / different agent viewport) and
/// queried for links/text without re-fetching.
pub struct Page {
    pub final_url: String,
    pub title: String,
    pub content_height: f32,
    pub root_box: LayoutBox,
    dom: Dom,
    /// The **base** cascade at 100% zoom. Zoomed layouts always derive from this, so
    /// repeated zooming never compounds.
    styles: StyleMap,
    zoom: f32,
    /// Decoded raster images keyed by their `<img>` node, painted into each element's box.
    images: std::collections::HashMap<manuk_dom::NodeId, std::rc::Rc<manuk_paint::DecodedImage>>,
}

/// E1 full-page zoom bounds (matching what mainstream browsers offer).
pub const MIN_ZOOM: f32 = 0.25;
pub const MAX_ZOOM: f32 = 5.0;

impl Page {
    /// Parse + style + lay out `html` for a viewport of `viewport_width` px.
    pub fn load(html: &str, final_url: &str, fonts: &FontContext, viewport_width: f32) -> Page {
        Page::from_dom(manuk_html::parse(html), final_url, fonts, viewport_width)
    }

    /// As [`load`](Self::load), but first **fetches external `<script src>`** so they run
    /// alongside inline scripts (only under the `spidermonkey` feature; otherwise identical to
    /// `load`). Callers on the async fetch path (shell/render) use this so a page's real
    /// scripts execute.
    pub async fn load_async(
        html: &str,
        final_url: &str,
        fonts: &FontContext,
        viewport_width: f32,
    ) -> Page {
        #[allow(unused_mut)]
        let mut dom = manuk_html::parse(html);
        #[cfg(feature = "spidermonkey")]
        fetch_external_scripts(&mut dom, final_url).await;
        let mut page = Page::from_dom(dom, final_url, fonts, viewport_width);
        page.fetch_and_apply_images(fonts, viewport_width).await;
        page
    }

    /// Fetch + decode this page's `<img>` resources and paint them. An image without an
    /// explicit width/height (attribute or CSS) is sized to its natural pixel dimensions;
    /// then the page is re-laid-out so the boxes are correct. Returns how many images
    /// decoded. Safe to call after stylesheets are applied (it patches only auto sizes and
    /// does not re-cascade, so external CSS is preserved).
    pub async fn fetch_and_apply_images(
        &mut self,
        fonts: &FontContext,
        viewport_width: f32,
    ) -> usize {
        let images = fetch_images(&self.dom, &self.final_url).await;
        if images.is_empty() {
            return 0;
        }
        // Natural sizing: fill in only dimensions the cascade left auto (explicit
        // attr/CSS width/height already resolved to a definite value and must win).
        for (&node, img) in &images {
            if let Some(style) = self.styles.get_mut(&node) {
                if style.width == manuk_css::Dim::Auto {
                    style.width = manuk_css::Dim::Px(img.width as f32);
                }
                if style.height == manuk_css::Dim::Auto {
                    style.height = manuk_css::Dim::Px(img.height as f32);
                }
            }
        }
        let count = images.len();
        self.images = images;
        self.relayout(fonts, viewport_width);
        count
    }

    /// Build a page from an already-parsed [`Dom`] (shared by [`load`](Self::load) and
    /// [`load_streaming`](Self::load_streaming)).
    pub fn from_dom(
        mut dom: Dom,
        final_url: &str,
        fonts: &FontContext,
        viewport_width: f32,
    ) -> Page {
        // Style + lay out once, then run the document's inline scripts against that layout
        // snapshot (so `getBoundingClientRect` works), letting them mutate the DOM. If they
        // did, re-style and re-lay-out so script-built content is rendered. All a no-op unless
        // the `spidermonkey` feature is on; scripts that throw are logged and the rest run.
        let sheets: Vec<Stylesheet> = MinimalCascade::collect_style_elements(&dom);
        let mut styles = MinimalCascade.cascade(&dom, &sheets);
        let mut root_box = layout_document(&dom, &styles, fonts, viewport_width);

        let rects: std::collections::HashMap<manuk_dom::NodeId, [f32; 4]> = root_box
            .node_rects(&dom)
            .into_iter()
            .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
            .collect();
        match manuk_js::run_document_scripts(&mut dom, &rects, &styles) {
            Ok(n) if n > 0 => {
                tracing::debug!(scripts = n, "executed page scripts");
                let sheets2: Vec<Stylesheet> = MinimalCascade::collect_style_elements(&dom);
                styles = MinimalCascade.cascade(&dom, &sheets2);
                root_box = layout_document(&dom, &styles, fonts, viewport_width);
            }
            Ok(_) => {}
            Err(e) => tracing::warn!("page scripts: {e}"),
        }

        let title = dom
            .find_first("title")
            .map(|t| {
                dom.text_content(t)
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| final_url.to_string());

        let content_height = root_box.content_bottom();
        // The tree is now laid out and clean; later mutations mark fresh dirtiness.
        dom.clear_all_dirty();

        Page {
            final_url: final_url.to_string(),
            title,
            content_height,
            root_box,
            dom,
            styles,
            zoom: 1.0,
            images: std::collections::HashMap::new(),
        }
    }

    /// **Streaming load with a first-paint checkpoint (B-latency).** Feeds `chunks` to
    /// an incremental parser ([`manuk_html::StreamParser`]); at the head-complete
    /// checkpoint (`<head>` + its render-blocking CSS parsed, `<body>` reached) it lays
    /// out the *partial* DOM — the **first paint**, available before the tail of the
    /// document arrives — and returns it alongside the finished [`Page`]. A real socket
    /// source hands chunks as they arrive; the win is first-paint before full-load.
    pub fn load_streaming<'a>(
        chunks: impl IntoIterator<Item = &'a str>,
        final_url: &str,
        fonts: &FontContext,
        viewport_width: f32,
    ) -> StreamingLoad {
        let mut sp = manuk_html::StreamParser::new();
        let mut first_paint = None;
        for chunk in chunks {
            sp.feed(chunk);
            if first_paint.is_none() && sp.body_started() {
                // Lay out the DOM-so-far (inline styles only; external CSS is applied
                // once fetched). This is the paint the user sees first.
                let partial = sp.snapshot();
                let sheets = MinimalCascade::collect_style_elements(&partial);
                let styles = MinimalCascade.cascade(&partial, &sheets);
                first_paint = Some(layout_document(&partial, &styles, fonts, viewport_width));
            }
        }
        let page = Page::from_dom(sp.finish(), final_url, fonts, viewport_width);
        StreamingLoad { first_paint, page }
    }

    /// Re-run layout at a new viewport width (reuses the DOM + computed styles).
    pub fn relayout(&mut self, fonts: &FontContext, viewport_width: f32) {
        self.relayout_zoomed(fonts, viewport_width, self.zoom);
    }

    /// The current full-page zoom factor (1.0 = 100%).
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// E1 **full-page zoom**: re-lay-out at `zoom`, scaling every *absolute* length
    /// (including `font_size`, hence **crisp** text) rather than magnifying a bitmap.
    ///
    /// The scaled styles are always derived from the **base** cascade, so calling this
    /// repeatedly never compounds. `zoom` is clamped to a usable range.
    pub fn relayout_zoomed(&mut self, fonts: &FontContext, viewport_width: f32, zoom: f32) {
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        let scaled;
        let styles = if (self.zoom - 1.0).abs() < 1e-6 {
            &self.styles
        } else {
            scaled = manuk_css::zoom_styles(&self.styles, self.zoom);
            &scaled
        };
        self.root_box = layout_document(&self.dom, styles, fonts, viewport_width);
        self.content_height = self.root_box.content_bottom();
        self.dom.clear_all_dirty();
    }

    /// **Incremental relayout (A2 activation).** After DOM mutations (e.g. from the
    /// JS bindings), re-cascade only if something changed and classify the change:
    ///
    /// - clean tree (or a mutation that produced no style/structure change) → **`None`**,
    ///   reuse the existing layout and paint (zero work);
    /// - otherwise re-cascade, compute the tree's [`RestyleDamage`] (structural change →
    ///   at least `Reflow`; per-node style diffs give `Repaint`/`Reflow`/`Rebuild`), and
    ///   re-lay-out.
    ///
    /// The returned damage lets the caller drive the compositor (repaint region vs
    /// reflow). *Current scope:* any `>= Repaint` change re-lays-out the whole tree —
    /// subtree-partial reuse (skipping clean subtrees via the DOM's summary bit) and a
    /// paint-only refresh that skips layout are the documented next fill-ins, since
    /// paint attributes are currently baked into the fragment tree.
    pub fn relayout_incremental(
        &mut self,
        fonts: &FontContext,
        viewport_width: f32,
    ) -> RestyleDamage {
        // Nothing changed since the last clean pass → reuse everything.
        if self.dom.subtree_clean(self.dom.root()) {
            return RestyleDamage::None;
        }

        let sheets: Vec<Stylesheet> = MinimalCascade::collect_style_elements(&self.dom);
        let new_styles = MinimalCascade.cascade(&self.dom, &sheets);

        // A structural mutation adds/removes boxes → reflow at minimum.
        let mut damage = if self.dom.structure_changed() {
            RestyleDamage::Reflow
        } else {
            RestyleDamage::None
        };
        for (node, new_s) in &new_styles {
            let d = match self.styles.get(node) {
                Some(old_s) => diff_style(old_s, new_s),
                None => RestyleDamage::Rebuild, // a node that did not exist before
            };
            damage = damage.max(d);
        }
        self.styles = new_styles;

        if damage >= RestyleDamage::Repaint {
            self.root_box = layout_document(&self.dom, &self.styles, fonts, viewport_width);
            self.content_height = self.root_box.content_bottom();
        }
        self.dom.clear_all_dirty();
        damage
    }

    /// Shared access to the DOM (e.g. to build the §4a accessibility tree).
    pub fn dom(&self) -> &Dom {
        &self.dom
    }

    /// §4a — the accessibility / semantic tree for this page, **with element geometry**
    /// taken from the current fragment tree. Shared by the agent's observation channel
    /// and (eventually) the `accesskit` screen-reader bridge.
    pub fn a11y_tree(&self) -> manuk_a11y::A11yNode {
        let rects: std::collections::HashMap<_, _> = self
            .root_box
            .node_rects(&self.dom)
            .into_iter()
            .map(|(node, r)| {
                (
                    node,
                    manuk_a11y::Rect {
                        x: r.x,
                        y: r.y,
                        width: r.width,
                        height: r.height,
                    },
                )
            })
            .collect();
        manuk_a11y::build_tree_with_rects(&self.dom, &rects)
    }

    /// Mutable access to the DOM (so a caller/JS binding can mutate the tree, then
    /// call [`relayout_incremental`](Self::relayout_incremental)).
    pub fn dom_mut(&mut self) -> &mut Dom {
        &mut self.dom
    }

    /// A rough estimate of the retained heap this page holds — the fragment tree,
    /// DOM, and computed styles — for C1 per-tab memory accounting. It is a *proxy*
    /// (not a true RSS figure): what a **discard** reclaims by dropping the `Page`.
    pub fn estimated_bytes(&self) -> usize {
        let mut n = 0usize;
        self.root_box.walk(&mut |b| {
            n += std::mem::size_of::<LayoutBox>();
            if let BoxContent::Inline(frags) = &b.content {
                for f in frags {
                    n += std::mem::size_of_val(f) + f.text.len();
                }
            }
        });
        // DOM nodes + per-node computed style (approximate fixed cost each).
        n += self.dom.len() * 96;
        n += self.styles.len() * std::mem::size_of::<manuk_css::ComputedStyle>();
        n
    }

    /// The external subresources this page references (`<link rel=stylesheet>`,
    /// `<script src>`, `<img src>`), in document order — the fetch scheduler's
    /// work-list (D4). Stylesheets are render-blocking; scripts carry `defer`/`async`.
    pub fn subresources(&self) -> Vec<Subresource> {
        collect_subresources(&self.dom, &self.final_url)
    }

    /// Re-style + re-lay-out with external stylesheets applied (D4 render-blocking
    /// CSS). `external` maps each `<link>`'s resolved URL → its CSS text; inline
    /// `<style>` and external sheets are combined in **document order** so cascade
    /// precedence is correct. Returns the resulting [`RestyleDamage`].
    ///
    /// Callers fetch the CSS (via [`fetch_and_apply_stylesheets`](Self::fetch_and_apply_stylesheets)
    /// or their own network path) and hand the texts here — keeping this core
    /// deterministic and testable.
    pub fn apply_stylesheets(
        &mut self,
        external: &HashMap<String, String>,
        fonts: &FontContext,
        viewport_width: f32,
    ) -> RestyleDamage {
        let sources = collect_style_sources(&self.dom, &self.final_url);
        let sheets: Vec<Stylesheet> = sources
            .iter()
            .filter_map(|s| match s {
                StyleSource::Inline(css) => Some(Stylesheet::parse(css)),
                StyleSource::External(url) => external.get(url).map(|css| Stylesheet::parse(css)),
            })
            .collect();
        let new_styles = MinimalCascade.cascade(&self.dom, &sheets);
        // Classify the change vs the pre-external styling (usually Reflow — external
        // rules add geometry).
        let mut damage = RestyleDamage::None;
        for (node, new_s) in &new_styles {
            let d = match self.styles.get(node) {
                Some(old_s) => diff_style(old_s, new_s),
                None => RestyleDamage::Rebuild,
            };
            damage = damage.max(d);
        }
        self.styles = new_styles;
        self.root_box = layout_document(&self.dom, &self.styles, fonts, viewport_width);
        self.content_height = self.root_box.content_bottom();
        self.dom.clear_all_dirty();
        damage
    }

    /// Fetch this page's external render-blocking stylesheets (via `manuk-net`) and
    /// apply them ([`apply_stylesheets`](Self::apply_stylesheets)). Returns how many
    /// external sheets were successfully fetched. Failed fetches are skipped (the page
    /// still renders with the sheets that loaded).
    pub async fn fetch_and_apply_stylesheets(
        &mut self,
        fonts: &FontContext,
        viewport_width: f32,
    ) -> usize {
        let sources = collect_style_sources(&self.dom, &self.final_url);
        let mut external: HashMap<String, String> = HashMap::new();
        for s in &sources {
            if let StyleSource::External(url) = s {
                if external.contains_key(url) {
                    continue;
                }
                if let Ok(resp) = manuk_net::fetch(url).await {
                    external.insert(url.clone(), resp.decoded_text());
                }
            }
        }
        // Web fonts: fetch @font-face sources (from inline + external CSS) and register
        // them BEFORE the relayout, so the cascade's font-family resolves to them.
        for s in &sources {
            let css = match s {
                StyleSource::Inline(c) => c.clone(),
                StyleSource::External(url) => external.get(url).cloned().unwrap_or_default(),
            };
            for ff in Stylesheet::parse(&css).font_faces() {
                for src in &ff.srcs {
                    let url = resolve_url(&self.final_url, src);
                    if let Some(data) = fetch_font_bytes(&url).await {
                        fonts.register_named_font(&ff.family, data);
                        break; // first usable source wins
                    }
                }
            }
        }

        let count = external.len();
        if count > 0 {
            self.apply_stylesheets(&external, fonts, viewport_width);
        } else {
            // Even with no external CSS, an inline @font-face may have registered a font.
            self.relayout(fonts, viewport_width);
        }
        count
    }

    /// Effective z-index per node for stacking-ordered paint: a positioned element with an
    /// explicit `z-index` establishes a layer that applies to its whole subtree (an
    /// approximation of CSS stacking contexts). Non-positioned / `z-index:auto` inherit the
    /// nearest such ancestor's layer (0 at the root).
    fn z_index_map(&self) -> HashMap<manuk_dom::NodeId, i32> {
        use manuk_css::Position;
        let mut map = HashMap::new();
        let mut stack = vec![(self.dom.root(), 0i32)];
        while let Some((node, parent_z)) = stack.pop() {
            let z = match self.styles.get(&node) {
                Some(s) if s.position != Position::Static => s.z_index.unwrap_or(parent_z),
                _ => parent_z,
            };
            map.insert(node, z);
            for c in self.dom.children(node) {
                stack.push((c, z));
            }
        }
        map
    }

    /// Per-node clip rect for `overflow` clipping: the intersection of the padding boxes of
    /// all ancestors with `overflow != visible`. A node not under any clipping ancestor is
    /// absent (unclipped). An element's own box is clipped by its ancestors, not itself; its
    /// descendants additionally get its padding box.
    fn clip_map(&self) -> HashMap<manuk_dom::NodeId, manuk_layout::Rect> {
        use manuk_css::Overflow;
        let rects = self.root_box.node_rects(&self.dom);
        let mut map = HashMap::new();
        let mut stack: Vec<(manuk_dom::NodeId, Option<manuk_layout::Rect>)> =
            vec![(self.dom.root(), None)];
        while let Some((node, clip)) = stack.pop() {
            if let Some(c) = clip {
                map.insert(node, c);
            }
            // If this node clips, its descendants are additionally bounded by its padding box.
            let child_clip = match self.styles.get(&node) {
                Some(s) if s.overflow != Overflow::Visible => rects
                    .get(&node)
                    .map(|br| {
                        let bw = s.border_width;
                        let pad = manuk_layout::Rect {
                            x: br.x + bw.left,
                            y: br.y + bw.top,
                            width: (br.width - bw.left - bw.right).max(0.0),
                            height: (br.height - bw.top - bw.bottom).max(0.0),
                        };
                        match clip {
                            Some(c) => c.intersect(&pad),
                            None => pad,
                        }
                    })
                    .or(clip),
                _ => clip,
            };
            for c in self.dom.children(node) {
                stack.push((c, child_clip));
            }
        }
        map
    }

    /// Rasterize the whole page to a canvas of the given pixel size (CPU tier).
    pub fn paint(&self, fonts: &FontContext, width: u32, height: u32) -> Canvas {
        let z = self.z_index_map();
        let clip = self.clip_map();
        CpuPainter::with_layers(fonts, &self.images, &z, &clip)
            .render(&self.root_box, width, height, Rgba::WHITE)
    }

    /// Rasterize the visible viewport with the content scrolled up by `scroll_y`.
    pub fn paint_scrolled(
        &self,
        fonts: &FontContext,
        width: u32,
        height: u32,
        scroll_y: f32,
    ) -> Canvas {
        let z = self.z_index_map();
        let clip = self.clip_map();
        CpuPainter::with_layers(fonts, &self.images, &z, &clip).render_scrolled(
            &self.root_box,
            width,
            height,
            Rgba::WHITE,
            scroll_y,
        )
    }

    /// All `<a href>` links, in document order, with hrefs resolved absolute.
    pub fn links(&self) -> Vec<Link> {
        let base = Url::parse(&self.final_url).ok();
        self.dom
            .descendants(self.dom.root())
            .filter(|&n| self.dom.tag_name(n) == Some("a"))
            .filter_map(|n| {
                let href = self.dom.element(n)?.attr("href")?.trim().to_string();
                if href.is_empty() || href.starts_with('#') {
                    return None;
                }
                let abs = base
                    .as_ref()
                    .and_then(|b| b.join(&href).ok())
                    .map(|u| u.to_string())
                    .unwrap_or_else(|| href.clone());
                let text = collapse_ws(&self.dom.text_content(n));
                Some(Link { text, href: abs })
            })
            .collect()
    }

    /// The page's visible text (body, whitespace-collapsed) — the agent's textual
    /// observation channel alongside the screenshot.
    /// The text a reader actually sees.
    ///
    /// Read from the **fragment tree**, not `Node.textContent`: that is what makes it
    /// respect `display:none`, `<head>` content, **shadow DOM**, and slot assignment for
    /// free. (`textContent` is a node-tree API and would miss shadow content entirely
    /// while including unrendered light-DOM children.)
    pub fn visible_text(&self) -> String {
        let mut words: Vec<String> = Vec::new();
        self.root_box.walk(&mut |b| {
            if let BoxContent::Inline(frags) = &b.content {
                for f in frags {
                    if !f.text.is_empty() {
                        words.push(f.text.clone());
                    }
                }
            }
        });
        words.join(" ")
    }
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A site hard-wall detected **honestly** (F2) — a page that blocks non-mainstream
/// browsers. Manuk never solves or bypasses these; it presents an honest interstitial.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HardWall {
    /// The documented `cf-mitigated: challenge` response header (a bot challenge).
    Challenge,
    /// `403 Forbidden` — often an access/bot wall.
    Forbidden,
    /// `429 Too Many Requests` — rate/bot limiting.
    RateLimited,
}

impl HardWall {
    fn describe(self) -> &'static str {
        match self {
            HardWall::Challenge => "the site served a bot-challenge (cf-mitigated)",
            HardWall::Forbidden => "the site refused the request (HTTP 403)",
            HardWall::RateLimited => "the site rate-limited the request (HTTP 429)",
        }
    }
}

/// Detect a hard-wall response **honestly** from its status + a header lookup. Returns
/// `None` for a normal response. (The `cf-mitigated: challenge` header is Cloudflare's
/// own documented signal; 403/429 are the coarse fallback.)
pub fn detect_hard_wall(status: u16, header: impl Fn(&str) -> Option<String>) -> Option<HardWall> {
    if header("cf-mitigated")
        .map(|v| v.trim().eq_ignore_ascii_case("challenge"))
        .unwrap_or(false)
    {
        return Some(HardWall::Challenge);
    }
    match status {
        403 => Some(HardWall::Forbidden),
        429 => Some(HardWall::RateLimited),
        _ => None,
    }
}

/// The honest graceful-degradation interstitial (F2): a calm page explaining that the
/// site blocks non-mainstream browsers and that **Manuk won't impersonate another
/// browser**, with honest options (retry / copy URL / open elsewhere). It contains
/// **no** challenge-solving or bypass — it is UX honesty, not evasion.
pub fn interstitial_html(url: &str, wall: HardWall) -> String {
    let safe_url = url
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;");
    format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Site unavailable in Manuk</title>\
         <style>body{{font-family:sans-serif;max-width:40em;margin:4em auto;padding:0 1em;color:#222;line-height:1.5}}\
         h1{{font-size:1.4em}} .u{{color:#06c;word-break:break-all}} ul{{padding-left:1.2em}}</style></head>\
         <body><h1>This site blocks non-mainstream browsers</h1>\
         <p><span class=\"u\">{safe_url}</span> did not load: {reason}.</p>\
         <p>Manuk <strong>will not impersonate another browser</strong> to get past it — \
         doing so would be dishonest and fragile. Your options:</p>\
         <ul><li><strong>Retry</strong> — the wall may be transient.</li>\
         <li><strong>Copy the URL</strong> and open it in another browser you trust.</li>\
         <li>Continue browsing sites that serve standards-based engines.</li></ul>\
         <p style=\"color:#888;font-size:.9em\">Manuk identifies itself truthfully and \
         solves no challenges.</p></body></html>",
        safe_url = safe_url,
        reason = wall.describe()
    )
}

/// **Streaming page load with a first-paint checkpoint (B-latency, end to end).**
/// For `http(s)` URLs, streams the body via [`manuk_net::fetch_streaming`] into an
/// incremental [`manuk_html::StreamParser`], laying out the partial DOM at the
/// head-complete checkpoint (the **first paint**, available before the tail arrives).
/// Returns the finished [`Page`] and that first-paint layout (if the checkpoint was
/// reached). Non-`http(s)` URLs (`data:`/`file:`/local) fall back to a buffered load.
///
/// Input is treated as UTF-8 (matching the parser); streaming transcode for legacy
/// charsets is a follow-on. External stylesheets are still applied via
/// [`Page::fetch_and_apply_stylesheets`] by the caller.
pub async fn fetch_streaming_page(
    url: &str,
    fonts: &FontContext,
    viewport_width: f32,
) -> Result<(Page, Option<LayoutBox>)> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        let (html, final_url) = fetch_html(url).await?;
        return Ok((
            Page::load_async(&html, &final_url, fonts, viewport_width).await,
            None,
        ));
    }

    let mut sp = manuk_html::StreamParser::new();
    let mut first_paint: Option<LayoutBox> = None;
    let meta = manuk_net::fetch_streaming(url, |bytes| {
        sp.feed_bytes(bytes);
        if first_paint.is_none() && sp.body_started() {
            // Lay out the DOM-so-far (inline styles only) — the first paint.
            let partial = sp.snapshot();
            let sheets = MinimalCascade::collect_style_elements(&partial);
            let styles = MinimalCascade.cascade(&partial, &sheets);
            first_paint = Some(layout_document(&partial, &styles, fonts, viewport_width));
        }
    })
    .await
    .with_context(|| format!("streaming fetch of {url}"))?;

    let page = Page::from_dom(sp.finish(), meta.final_url.as_str(), fonts, viewport_width);
    Ok((page, first_paint))
}

/// Fetch a document's HTML. Supports `http(s)://` (via `manuk-net`, with WHATWG
/// charset decoding), `data:` URLs (RFC 2397), `file://`, and bare local paths.
/// Returns `(html, final_url_after_redirects)`.
/// Fetch every external `<script src>` in `dom` (resolved against `base`) and inline its
/// content as the script node's text, dropping the `src`, so the from_dom script pass runs it.
/// External scripts fetch sequentially in document order (the classic-script model).
#[cfg(feature = "spidermonkey")]
async fn fetch_external_scripts(dom: &mut Dom, base: &str) {
    let mut targets = Vec::new();
    for n in dom.descendants(dom.root()) {
        if dom.tag_name(n) == Some("script") {
            if let Some(src) = dom.element(n).and_then(|e| e.attr("src")) {
                if let Ok(u) = Url::parse(base).and_then(|b| b.join(src)) {
                    targets.push((n, u.to_string()));
                }
            }
        }
    }
    for (node, url) in targets {
        match fetch_html(&url).await {
            Ok((js, _)) => {
                dom.remove_attr(node, "src");
                let text = dom.create_text(js);
                dom.append_child(node, text);
                tracing::debug!(%url, "fetched external script");
            }
            Err(e) => tracing::warn!(%url, "external script fetch failed: {e:#}"),
        }
    }
}

pub async fn fetch_html(url: &str) -> Result<(String, String)> {
    if url.starts_with("http://") || url.starts_with("https://") {
        let resp = manuk_net::fetch(url)
            .await
            .with_context(|| format!("fetching {url}"))?;
        if resp.status >= 400 {
            anyhow::bail!("server returned HTTP {} for {}", resp.status, url);
        }
        // WHATWG charset sniff (D4) instead of lossy UTF-8.
        Ok((resp.decoded_text(), resp.final_url.to_string()))
    } else if let Some(rest) = url.strip_prefix("data:") {
        Ok((decode_data_url(rest)?, url.to_string()))
    } else {
        let path = url.strip_prefix("file://").unwrap_or(url);
        let html =
            std::fs::read_to_string(path).with_context(|| format!("reading local file {path}"))?;
        Ok((html, url.to_string()))
    }
}

/// Decode an RFC 2397 `data:` URL body (`[<mediatype>][;base64],<data>`).
fn decode_data_url(rest: &str) -> Result<String> {
    let (meta, data) = rest
        .split_once(',')
        .context("malformed data: URL (no comma)")?;
    let bytes = if meta.trim_end().to_ascii_lowercase().ends_with(";base64") {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD
            .decode(data.trim())
            .context("bad base64 in data: URL")?
    } else {
        percent_decode(data)
    };
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Minimal percent-decoding for non-base64 `data:` payloads.
fn percent_decode(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

fn hex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §4a — element geometry must come from the **real** layout pipeline, not a
    /// synthetic map: the a11y tree's bboxes have to agree with the fragment tree,
    /// and hit-testing a button's own center must return that button.
    #[test]
    fn a11y_tree_carries_real_layout_geometry_and_hit_tests() {
        let fonts = FontContext::new();
        let html = r#"<!DOCTYPE html><title>T</title><body>
            <h1>Heading</h1>
            <button>Sign in</button>
            <p>filler</p>
            </body>"#;
        let page = Page::load(html, "http://example.test/", &fonts, 800.0);
        let tree = page.a11y_tree();

        let btn = tree
            .find(&manuk_a11y::Role::Button, "Sign in")
            .expect("button is in the a11y tree");
        let bbox = btn.bbox.expect("button was laid out, so it has geometry");
        assert!(bbox.width > 0.0 && bbox.height > 0.0, "degenerate bbox: {bbox:?}");

        // The heading is laid out above the button (normal block flow).
        let h1 = tree.find(&manuk_a11y::Role::Heading { level: 1 }, "Heading").unwrap();
        assert!(h1.bbox.unwrap().y < bbox.y, "h1 should precede the button");

        // Hit-testing the button's own center resolves to the button.
        let (cx, cy) = bbox.center();
        assert_eq!(tree.hit_test(cx, cy).map(|n| n.node), Some(btn.node));

        // The viewport rendering carries a click point for the button.
        let vp = manuk_a11y::Rect { x: 0.0, y: 0.0, width: 800.0, height: 600.0 };
        assert!(tree
            .to_viewport_lines(vp)
            .iter()
            .any(|l| l.starts_with("button \"Sign in\" @(")));
    }

    /// E1 acceptance: Ctrl+/− **reflows** rather than magnifying a bitmap. Zooming in
    /// must scale `font_size` (so glyphs rasterize larger — crisp) and therefore grow
    /// the content, while zooming out shrinks it. Repeated calls must not compound.
    #[test]
    fn full_page_zoom_reflows_crisply_and_does_not_compound() {
        let fonts = FontContext::new();
        let html = "<body><p>Some text that wraps a little at narrow widths.</p></body>";
        let mut page = Page::load(html, "http://x.test/", &fonts, 400.0);
        assert_eq!(page.zoom(), 1.0);
        let base_h = page.content_height;

        let font_size_at = |p: &Page| -> f32 {
            let mut fs = 0.0f32;
            p.root_box.walk(&mut |b| {
                if let manuk_layout::BoxContent::Inline(frags) = &b.content {
                    for f in frags {
                        fs = fs.max(f.style.font_size);
                    }
                }
            });
            fs
        };
        let base_fs = font_size_at(&page);

        // Zoom in: text is laid out larger, so the document gets taller.
        page.relayout_zoomed(&fonts, 400.0, 2.0);
        assert_eq!(page.zoom(), 2.0);
        let big_h = page.content_height;
        assert!(big_h > base_h, "zoom-in must grow content: {base_h} -> {big_h}");

        // The *font size* really changed — that is what makes it crisp rather than a
        // scaled bitmap.
        let big_fs = font_size_at(&page);
        assert!((big_fs - base_fs * 2.0).abs() < 0.01, "font_size must scale with zoom");

        // Returning to 100% restores the original layout exactly (no compounding).
        page.relayout_zoomed(&fonts, 400.0, 1.0);
        assert_eq!(page.zoom(), 1.0);
        assert!((page.content_height - base_h).abs() < 0.01);
        assert!((font_size_at(&page) - base_fs).abs() < 0.01);

        // Zooming twice in a row derives from the base each time, never compounding.
        page.relayout_zoomed(&fonts, 400.0, 2.0);
        let once = page.content_height;
        page.relayout_zoomed(&fonts, 400.0, 2.0);
        assert!((page.content_height - once).abs() < 0.01, "zoom compounded");

        // Zoom out shrinks it.
        page.relayout_zoomed(&fonts, 400.0, 0.5);
        assert!(page.content_height < base_h);

        // Out-of-range factors clamp rather than producing a degenerate layout.
        page.relayout_zoomed(&fonts, 400.0, 1000.0);
        assert_eq!(page.zoom(), MAX_ZOOM);
        page.relayout_zoomed(&fonts, 400.0, 0.0001);
        assert_eq!(page.zoom(), MIN_ZOOM);
    }

    /// N3+N4 end-to-end: declarative shadow content, and light-DOM content slotted into
    /// it, both reach LAYOUT through the flat tree — not merely the style map.
    #[test]
    fn shadow_and_slotted_content_reach_layout() {
        let fonts = FontContext::new();
        let html = r#"<body><div id="host">
              <template shadowrootmode="open"><h1>ShadowTitle</h1><slot></slot></template>
              <p>SlottedBody</p>
            </div></body>"#;
        let page = Page::load(html, "http://x.test/", &fonts, 800.0);

        // Both strings are visible: one from the shadow tree, one slotted from the light DOM.
        let text = page.visible_text();
        assert!(text.contains("ShadowTitle"), "shadow content must render: {text:?}");
        assert!(text.contains("SlottedBody"), "slotted content must render: {text:?}");

        // And both produced real geometry.
        let tree = page.a11y_tree();
        let h1 = tree
            .find(&manuk_a11y::Role::Heading { level: 1 }, "ShadowTitle")
            .expect("shadow <h1> is in the a11y tree");
        assert!(h1.bbox.expect("laid out").height > 0.0);
    }

    #[test]
    fn loads_titles_links_and_text() {
        let fonts = FontContext::new();
        let html = r#"<!DOCTYPE html><title>My Page</title>
            <body><h1>Hi</h1><p>some text <a href="/about">About us</a> and
            <a href="https://other.test/x">Other</a></p></body>"#;
        let page = Page::load(html, "http://example.test/dir/", &fonts, 800.0);
        assert_eq!(page.title, "My Page");
        assert!(page.content_height > 0.0);
        assert!(page.visible_text().contains("some text"));

        let links = page.links();
        assert_eq!(links.len(), 2);
        // Relative href resolved against the base URL.
        assert_eq!(links[0].href, "http://example.test/about");
        assert_eq!(links[0].text, "About us");
        assert_eq!(links[1].href, "https://other.test/x");
    }

    #[tokio::test]
    async fn data_url_loads() {
        let (html, _) = fetch_html("data:text/html,<title>D</title><p>hello</p>")
            .await
            .unwrap();
        assert!(html.contains("hello") && html.contains("<title>D</title>"));

        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::STANDARD.encode("<p>b64 body</p>");
        let (html2, _) = fetch_html(&format!("data:text/html;base64,{b64}"))
            .await
            .unwrap();
        assert!(html2.contains("b64 body"));
    }

    #[test]
    fn relayout_changes_wrapping_height() {
        let fonts = FontContext::new();
        if fonts.face_count() == 0 {
            return;
        }
        let html =
            "<body><p>the quick brown fox jumps over the lazy dog several times over</p></body>";
        let mut page = Page::load(html, "x", &fonts, 600.0);
        let wide = page.content_height;
        page.relayout(&fonts, 90.0);
        assert!(
            page.content_height > wide,
            "narrower viewport should wrap taller"
        );
    }

    #[test]
    fn hard_wall_detection_and_honest_interstitial() {
        // cf-mitigated:challenge → Challenge, regardless of status.
        let hdr = |name: &str| (name == "cf-mitigated").then(|| "challenge".to_string());
        assert_eq!(detect_hard_wall(200, hdr), Some(HardWall::Challenge));
        // Bare status walls.
        assert_eq!(detect_hard_wall(403, |_| None), Some(HardWall::Forbidden));
        assert_eq!(detect_hard_wall(429, |_| None), Some(HardWall::RateLimited));
        // A normal response is not a wall.
        assert_eq!(detect_hard_wall(200, |_| None), None);

        // The interstitial is honest — no bypass/challenge-solving language, and it
        // renders (the pipeline can lay it out) with the URL escaped.
        let html = interstitial_html("https://walled.example/?a=1&b=2", HardWall::Challenge);
        assert!(html.contains("will not impersonate another browser"));
        assert!(html.contains("solves no challenges"));
        assert!(html.contains("&amp;")); // URL entity-escaped
        for banned in ["bypass", "solve the challenge", "spoof"] {
            assert!(
                !html.to_lowercase().contains(banned),
                "no evasion language: {banned}"
            );
        }
        let fonts = FontContext::new();
        let page = Page::load(&html, "manuk:interstitial", &fonts, 800.0);
        assert!(page
            .visible_text()
            .contains("blocks non-mainstream browsers"));
    }

    #[test]
    fn shaped_run_cache_hits_during_layout() {
        let fonts = FontContext::new();
        if fonts.face_count() == 0 {
            return;
        }
        // A page whose words repeat heavily (a list of identical items) — real layout
        // should hit the shaped-run cache far more than it misses.
        let items = "<li>alpha beta gamma delta</li>".repeat(60);
        let html = format!("<body><ul>{items}</ul></body>");
        let _page = Page::load(&html, "x", &fonts, 800.0);

        let (hits, misses) = fonts.measure_cache_stats();
        assert!(
            hits > misses * 4,
            "repeated text should hit the shaped-run cache far more than it misses \
             (hits={hits}, misses={misses})"
        );
    }

    #[test]
    fn streaming_first_paint_precedes_full_content() {
        let fonts = FontContext::new();
        // Head + above-the-fold arrives first; the long tail arrives after.
        let head_and_top = "<html><head><title>T</title></head>\
                            <body><div style='height:40px'>top</div>";
        let tail = "<div style='height:400px'>tail</div></body></html>";

        let load = Page::load_streaming([head_and_top, tail], "x", &fonts, 800.0);

        // A first paint was produced at the head-complete checkpoint...
        let fp = load.first_paint.expect("first paint at checkpoint");
        let fp_height = fp.content_bottom();
        // ...and it is strictly shorter than the full page (the tail was not yet in).
        assert!(
            load.page.content_height > fp_height + 300.0,
            "full page ({}) should be much taller than the first paint ({fp_height})",
            load.page.content_height
        );
        // The full page has the tail content; the DOM is complete.
        assert!(load.page.visible_text().contains("tail"));
        assert!(load.page.visible_text().contains("top"));
    }

    #[test]
    fn collects_subresources_with_semantics() {
        let fonts = FontContext::new();
        let html = r#"<head>
            <link rel="stylesheet" href="/css/site.css">
            <link rel="icon" href="/favicon.ico">
            <style>p{color:red}</style>
            <script src="/js/app.js" defer></script>
            <script src="analytics.js" async></script>
          </head><body><img src="/img/logo.png"><p>hi</p></body>"#;
        let page = Page::load(html, "http://ex.test/dir/", &fonts, 800.0);
        let subs = page.subresources();

        // stylesheet link, two scripts, one image — the `icon` link is not a sheet.
        assert_eq!(
            subs.iter()
                .filter(|s| s.kind == SubresourceKind::Stylesheet)
                .count(),
            1
        );
        let sheet = subs
            .iter()
            .find(|s| s.kind == SubresourceKind::Stylesheet)
            .unwrap();
        assert_eq!(
            sheet.url, "http://ex.test/css/site.css",
            "href resolved absolute"
        );

        let scripts: Vec<_> = subs
            .iter()
            .filter(|s| matches!(s.kind, SubresourceKind::Script { .. }))
            .collect();
        assert_eq!(scripts.len(), 2);
        assert_eq!(
            scripts[0].kind,
            SubresourceKind::Script {
                defer: true,
                is_async: false
            }
        );
        assert_eq!(
            scripts[1].kind,
            SubresourceKind::Script {
                defer: false,
                is_async: true
            }
        );
        assert_eq!(
            scripts[1].url, "http://ex.test/dir/analytics.js",
            "relative resolved"
        );

        assert_eq!(
            subs.iter()
                .filter(|s| s.kind == SubresourceKind::Image)
                .count(),
            1
        );
    }

    #[test]
    fn external_stylesheet_applies_before_layout() {
        let fonts = FontContext::new();
        // The page has NO inline sizing; the external sheet sets the div's height.
        let html = r#"<head><link rel="stylesheet" href="/s.css"></head>
            <body><div id=box></div></body>"#;
        let mut page = Page::load(html, "http://ex.test/", &fonts, 800.0);
        let before = page.content_height;

        // Inject the fetched CSS (as fetch_and_apply_stylesheets would) and apply it.
        let mut external = HashMap::new();
        external.insert(
            "http://ex.test/s.css".to_string(),
            "#box { height: 250px }".to_string(),
        );
        let damage = page.apply_stylesheets(&external, &fonts, 800.0);

        assert!(
            damage >= RestyleDamage::Reflow,
            "external sizing forces reflow"
        );
        assert!(
            page.content_height >= before + 240.0,
            "external stylesheet's height must apply: {} -> {}",
            before,
            page.content_height
        );
    }

    #[test]
    fn incremental_relayout_classifies_and_skips() {
        let fonts = FontContext::new();
        let html = "<body><div id=a style='width:100px;height:20px'></div>\
                    <div id=b style='height:20px'></div></body>";
        let mut page = Page::load(html, "x", &fonts, 800.0);

        // Nothing changed → None (zero-work reuse).
        assert_eq!(
            page.relayout_incremental(&fonts, 800.0),
            RestyleDamage::None,
            "an unmutated page relayouts to None"
        );

        // Find node `a`.
        let a = page
            .dom
            .descendants(page.dom.root())
            .find(|&n| page.dom.element(n).and_then(|e| e.id()) == Some("a"))
            .unwrap();

        // A color-only change → Repaint.
        page.dom_mut()
            .set_attr(a, "style", "width:100px;height:20px;background:red");
        assert_eq!(
            page.relayout_incremental(&fonts, 800.0),
            RestyleDamage::Repaint,
            "background-only change is Repaint"
        );

        // A geometry change → Reflow, and the height actually changes.
        let before_h = page.content_height;
        page.dom_mut()
            .set_attr(a, "style", "width:100px;height:200px;background:red");
        assert_eq!(
            page.relayout_incremental(&fonts, 800.0),
            RestyleDamage::Reflow,
            "height change is Reflow"
        );
        assert!(page.content_height > before_h, "taller box grew the page");

        // A structural change (append a new block) → at least Reflow.
        let new_div = page.dom_mut().create_element("div");
        page.dom_mut().set_attr(new_div, "style", "height:50px");
        let body = page.dom.find_first("body").unwrap();
        page.dom_mut().append_child(body, new_div);
        assert!(
            page.relayout_incremental(&fonts, 800.0) >= RestyleDamage::Reflow,
            "appending a box forces reflow"
        );

        // Settles back to None once clean.
        assert_eq!(
            page.relayout_incremental(&fonts, 800.0),
            RestyleDamage::None
        );
    }
}
