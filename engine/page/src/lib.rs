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
/// One step of a streaming response — see [`Page::deliver_fetch_stream`]. Re-exported so the shell
/// can drive the streaming path without depending on `manuk-js` directly.
pub use manuk_js::FetchStreamEvent;

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

/// Cascade styles for `dom` with author `sheets`, resolving `@media` and viewport units
/// (`vw`/`vh`/…) against a `viewport_width`-wide viewport. Under `--features stylo` this
/// drives the real Stylo cascade (full selector matching, `@media`, `var()`, `@layer`,
/// correct specificity); otherwise the from-scratch [`MinimalCascade`]. Both consume the
/// same `Stylesheet`s (collected via [`MinimalCascade::collect_style_elements`]), so only
/// the cascade step swaps — the rest of the pipeline is engine-agnostic.
fn cascade_styles(dom: &Dom, sheets: &[Stylesheet], viewport_width: f32) -> StyleMap {
    manuk_css::values::set_viewport_width(viewport_width);
    #[cfg(feature = "stylo")]
    {
        let (_, vh) = manuk_css::values::viewport_size();
        // Stylo's DOM trait wall has provably-unreachable `unimplemented!()` paths; if a real
        // page trips one, don't crash the browser — fall back to MinimalCascade for that page.
        let cascaded = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            manuk_css::stylo_engine::cascade_via_stylo(dom, sheets, viewport_width, vh)
        }));
        match cascaded {
            Ok(styles) => styles,
            Err(_) => {
                tracing::warn!(
                    "Stylo cascade panicked; falling back to MinimalCascade for this page"
                );
                MinimalCascade.cascade(dom, sheets)
            }
        }
    }
    #[cfg(not(feature = "stylo"))]
    {
        MinimalCascade.cascade(dom, sheets)
    }
}

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
    /// `(css, media)` — the `media` attribute of the `<style>`/`<link>` that carried it.
    ///
    /// A `<link media="(prefers-color-scheme: dark)">` is a **conditional** stylesheet: applying it
    /// unconditionally is not "slightly wrong", it is the wrong theme. docs.python.org ships its
    /// dark theme exactly this way, and we rendered the entire site dark on a light-scheme device.
    /// Every print sheet and every mobile sheet on the web is gated the same way.
    Inline(String, Option<String>),
    External(String, Option<String>),
}

/// Wrap a conditional stylesheet in `@media <query> { … }` so the cascade's existing media
/// evaluation decides whether it applies — rather than reimplementing that decision here, in a
/// second place, differently.
fn wrap_media(css: &str, media: &Option<String>) -> String {
    match media.as_deref().map(str::trim) {
        Some(m) if !m.is_empty() && !m.eq_ignore_ascii_case("all") => {
            format!("@media {m} {{\n{css}\n}}")
        }
        _ => css.to_string(),
    }
}

/// Scan raw HTML for render-blocking subresource URLs to prefetch early (the preload
/// scanner): `<link rel="stylesheet">` and `<link rel="preload">` hrefs, resolved absolute.
/// A lightweight string scan (runs before the tree is built) — only well-formed `http(s)`
/// URLs are returned.
fn scan_preloads(html: &str, base: &str) -> Vec<String> {
    let lower = html.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut i = 0;
    while let Some(p) = lower[i..].find("<link") {
        let start = i + p;
        let end = lower[start..]
            .find('>')
            .map(|e| start + e + 1)
            .unwrap_or(lower.len());
        let tag_low = &lower[start..end];
        i = end;
        let is_target = tag_low.contains("stylesheet") || tag_low.contains("preload");
        if !is_target {
            continue;
        }
        if let Some(href) = extract_tag_attr(&html[start..end], "href") {
            let abs = resolve_url(base, &href);
            if abs.starts_with("http") && seen.insert(abs.clone()) {
                out.push(abs);
            }
        }
    }
    out
}

/// Extract `name="value"` (or `name='value'`) from a single HTML start-tag string.
fn extract_tag_attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let key = format!("{name}=");
    let at = lower.find(&key)? + key.len();
    let rest = &tag[at..];
    let quote = rest.chars().next()?;
    if quote == '"' || quote == '\'' {
        let v = &rest[1..];
        v.find(quote).map(|e| v[..e].to_string())
    } else {
        Some(rest.split([' ', '>', '\t', '\n']).next()?.to_string())
    }
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
/// UI-thread convenience: [`fetch_images_owned`] with the results wrapped in `Rc`.
async fn fetch_images(
    dom: &Dom,
    base: &str,
) -> std::collections::HashMap<manuk_dom::NodeId, std::rc::Rc<manuk_paint::DecodedImage>> {
    fetch_images_except(
        dom,
        base,
        &std::collections::HashSet::new(),
        &mut std::collections::HashMap::new(),
    )
    .await
    .0
}

/// The same, minus the nodes whose image we have ALREADY fetched and decoded for this navigation.
/// See the call site: this method runs once per dynamic-script round, and without this it re-fetched
/// and re-decoded every `<img>` on the page every single time.
/// Fetch + decode a list of image URLs off the UI thread. **Owned data only** — not one `Rc` appears
/// here, because an `Rc` held across an `.await` makes the whole future `!Send` and pins it to the UI
/// thread, which is the exact mistake that made image loading block the window.
pub async fn fetch_image_urls(
    urls: Vec<String>,
) -> std::collections::HashMap<String, manuk_paint::DecodedImage> {
    let fetched = futures_util::future::join_all(
        urls.into_iter()
            .map(|url| async move { (url.clone(), fetch_image_bytes(&url).await, url) }),
    )
    .await;
    let mut out = std::collections::HashMap::new();
    for (key, bytes, url) in fetched {
        let Some(bytes) = bytes else { continue };
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
                out.insert(key, img);
            }
        }
    }
    out
}

async fn fetch_images_except(
    dom: &Dom,
    base: &str,
    attempted: &std::collections::HashSet<(manuk_dom::NodeId, String)>,
    cache: &mut std::collections::HashMap<String, Option<std::rc::Rc<manuk_paint::DecodedImage>>>,
) -> (
    std::collections::HashMap<manuk_dom::NodeId, std::rc::Rc<manuk_paint::DecodedImage>>,
    Vec<(manuk_dom::NodeId, String)>,
) {
    let known: std::collections::HashSet<String> = cache.keys().cloned().collect();
    let (by_url, tried) = fetch_images_owned(dom, base, attempted, &known).await;

    // Fold the answers into the page's URL cache. **A URL we asked for and did not get back is a
    // FAILURE, and it is recorded as `None`.** Remembering the "no" is the difference between one
    // fetch and one fetch per render round forever — that was the nytimes storm (507 duplicates
    // of 813 fetches), and it is what G_DEDUP exists to keep dead.
    for (url, img) in by_url {
        cache.insert(url, Some(std::rc::Rc::new(img)));
    }
    for (_, url) in &tried {
        cache.entry(url.clone()).or_insert(None);
    }

    // Every node that wanted an image is resolved from the ONE decoded copy for its URL. Nine elements
    // pointing at the same sprite share one `Rc`, and cost one fetch and one decode between them.
    let mut out = std::collections::HashMap::new();
    for (node, url) in &tried {
        if let Some(Some(img)) = cache.get(url) {
            out.insert(*node, img.clone());
        }
    }
    (out, tried)
}

/// DEBT-1 — the **owned, `Send`** image fetch. Not one `Rc` appears inside this async body: an
/// `Rc` held across an `.await` makes the whole future `!Send`, and that single detail is what
/// pinned image fetching to the UI thread (and blocked it). The `Rc` wrapper is applied afterwards,
/// on the UI thread, by [`fetch_images`].
async fn fetch_images_owned(
    dom: &Dom,
    base: &str,
    attempted: &std::collections::HashSet<(manuk_dom::NodeId, String)>,
    known_urls: &std::collections::HashSet<String>,
) -> (
    std::collections::HashMap<String, manuk_paint::DecodedImage>,
    Vec<(manuk_dom::NodeId, String)>,
) {
    let mut out: std::collections::HashMap<String, manuk_paint::DecodedImage> =
        std::collections::HashMap::new();
    // **Skip anything already ATTEMPTED — not merely anything already SUCCEEDED.**
    //
    // The old filter was `!self.images.contains_key(n)`, i.e. "have we got a decoded image for this
    // node". An image that FAILS — a blocked tracker, a 404, a timeout — never lands in that map, so
    // it was re-fetched on every script round, forever. A news front page is *made* of images that
    // fail, and it runs six rounds: measured, nytimes.com issued **813 fetches of which 507 were
    // duplicates**, theguardian.com 431 of 576 (75%).
    //
    // Keyed by `(node, resolved url)` and not by node alone, so a script that legitimately swaps an
    // `img.src` still triggers a real fetch. Remembering the failure is not the same as refusing to
    // retry a *different* request.
    let targets: Vec<(manuk_dom::NodeId, String)> = dom
        .flat_descendants(dom.root())
        .into_iter()
        .filter_map(|n| {
            // **`<video poster>` is a still image, and we can already decode still images.**
            //
            // We cannot decode the video. We can decode the poster — so a `<video>` renders its poster
            // frame at the right size, with an honest "cannot play" behind it (see the HTMLMediaElement
            // surface in `dom_bindings`). That is what graceful degradation *is*: not a blank rectangle,
            // and not a thrown exception. It is the frame the author chose to represent the video,
            // which is exactly what a real browser shows before you press play.
            let el = dom.element(n)?;
            let src = match dom.tag_name(n)? {
                "img" => el.attr("src")?,
                "video" => el.attr("poster")?,
                _ => return None,
            }
            .trim()
            .to_string();
            if src.is_empty() {
                return None;
            }
            let url = resolve_url(base, &src);
            if attempted.contains(&(n, url.clone())) {
                return None;
            }
            Some((n, url))
        })
        .collect();
    let tried: Vec<(manuk_dom::NodeId, String)> = targets.clone();

    // **Fetch by URL, not by node.** A browser does not fetch the same sprite nine times because nine
    // elements point at it — but that is exactly what this did, keyed by `(node, url)`. G_DEDUP caught
    // it the moment it was written: a page naming one image from nine elements issued **14 duplicate
    // fetches of 17**. The per-round storm was fixed in tick 25; the same-URL storm was not, and news
    // sites mention their placeholders and sprites constantly.
    //
    // So: reduce to the DISTINCT urls we do not already have an answer for, and fetch each exactly once.
    let mut wanted: Vec<String> = Vec::new();
    for (_, url) in &targets {
        if !known_urls.contains(url) && !wanted.contains(url) {
            wanted.push(url.clone());
        }
    }

    // Concurrent (I/O-bound; the shared client pools + multiplexes), then decode sequentially (CPU).
    let fetched = futures_util::future::join_all(
        wanted
            .into_iter()
            .map(|url| async move { (url.clone(), fetch_image_bytes(&url).await, url) }),
    )
    .await;
    for (node, bytes, url) in fetched {
        let Some(bytes) = bytes else {
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
                out.insert(node, img); // `node` is the URL key here — see the signature
            }
        }
    }
    (out, tried)
}

/// Every raw `url(...)` argument of a `mask-image` / `-webkit-mask-image` declaration in a
/// stylesheet. A cheap text scan, deliberately: the prefetch thread has no cascade, and a superset
/// of the URLs actually used is harmless (a page has a few dozen distinct icons at most).
fn scan_mask_urls(css: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = css.as_bytes();
    let mut i = 0usize;
    while let Some(hit) = css[i..].find("mask-image") {
        let start = i + hit;
        i = start + 10;
        // The declaration's value runs to the next `;` or `}`.
        let end = bytes[i..]
            .iter()
            .position(|&c| c == b';' || c == b'}')
            .map(|p| i + p)
            .unwrap_or(bytes.len());
        let val = &css[i..end];
        if let Some(u0) = val.find("url(") {
            let rest = &val[u0 + 4..];
            if let Some(u1) = rest.find(')') {
                let raw = rest[..u1].trim().trim_matches('"').trim_matches('\'');
                if !raw.is_empty() {
                    out.push(raw.to_string());
                }
            }
        }
        i = end;
    }
    out
}

/// Decode fetched bytes as a raster image, falling back to SVG (icons are overwhelmingly SVG).
fn decode_bitmap(bytes: &[u8], url: &str) -> Option<manuk_paint::DecodedImage> {
    match image::load_from_memory(bytes) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            (w > 0 && h > 0).then(|| manuk_paint::DecodedImage {
                width: w,
                height: h,
                rgba: rgba.into_raw(),
            })
        }
        Err(_) => decode_svg(bytes, url).filter(|i| i.width > 0 && i.height > 0),
    }
}

/// Fetch + decode a set of **mask images**, keyed by the raw `url(...)` string exactly as it
/// appears in the CSS (so a computed `mask-image` can be looked up directly).
///
/// The modern web draws icons as an empty element with a `background-color` shaped by a
/// `mask-image`. Without the mask, that background paints as a solid block — a black square where
/// every icon should be. Owned data only, no `Rc`: this runs off the UI thread.
async fn fetch_masks_owned(
    targets: Vec<(String, String)>,
) -> HashMap<String, manuk_paint::DecodedImage> {
    let fetched = futures_util::future::join_all(
        targets
            .into_iter()
            .map(|(raw, abs)| async move { (raw, fetch_image_bytes(&abs).await, abs) }),
    )
    .await;
    fetched
        .into_iter()
        .filter_map(|(raw, bytes, abs)| Some((raw, decode_bitmap(&bytes?, &abs)?)))
        .collect()
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
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
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
/// (and TrueType collections) pass through; WOFF2 (`wOF2`) and legacy WOFF1 (`wOFF`) are
/// decoded to sfnt via the pure-Rust reconstructor in `manuk_text::woff2` (A3) — most real
/// web fonts ship as WOFF2, so this is the difference between the page font loading or not.
async fn fetch_font_bytes(url: &str) -> Option<Vec<u8>> {
    let bytes = fetch_image_bytes(url).await?; // reuses data:/http(s)/file handling
    let sig = bytes.get(..4)?;
    match sig {
        // WOFF2 / WOFF1: decompress + reconstruct to sfnt (None if malformed/unsupported).
        b"wOF2" | b"wOFF" => manuk_text::decode_webfont(&bytes),
        // sfnt magics: TrueType (0x00010000 / "true"), OpenType ("OTTO"), collection ("ttcf").
        b"\x00\x01\x00\x00" | b"true" | b"OTTO" | b"ttcf" => Some(bytes),
        _ => None,
    }
}

/// Collect the page's stylesheet sources (inline + external) in document order.
///
/// **Walks the FLAT tree, not the light tree.** A `<style>` inside a shadow root is not a descendant
/// in the light DOM — so this walk, which is the only thing that collects stylesheets, could not see
/// it, and every web component on every page rendered completely unstyled. Not subtly wrong: *no rule
/// from inside a shadow root reached the cascade at all.*
///
/// This is the fifth instance of one shape, and it is worth naming as a shape rather than as five
/// bugs: `flat_children`, `NodeData::Comment`, `NodeData::Fragment`, the flat tree in the cascade —
/// and now this. **The mechanism existed and was correct. Nobody had drawn a line from it to the thing
/// that renders, and no gate asked whether such a line existed.** The feature being present in the
/// codebase is not the same as the feature being reachable from the pixels.
fn collect_style_sources(dom: &Dom, base: &str) -> Vec<StyleSource> {
    let mut out = Vec::new();
    for n in dom.flat_descendants(dom.root()) {
        match dom.tag_name(n) {
            Some("style") => {
                let media = dom
                    .element(n)
                    .and_then(|e| e.attr("media"))
                    .map(str::to_string);
                out.push(StyleSource::Inline(dom.text_content(n), media));
            }
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
                            let media = el.attr("media").map(str::to_string);
                            out.push(StyleSource::External(resolve_url(base, href), media));
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
    // Flat tree, for the same reason as `collect_style_sources`: a `<link rel=stylesheet>` inside a
    // shadow root is invisible to a light-DOM walk.
    for n in dom.flat_descendants(dom.root()) {
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
    /// Boxed so its heap address is stable across `Page` moves: the persistent JS context
    /// (`js`) caches raw `*mut Dom` pointers in its reflectors, which must not dangle when the
    /// `Page` is moved into the shell's tab slot.
    dom: Box<Dom>,
    /// The **base** cascade at 100% zoom. Zoomed layouts always derive from this, so
    /// repeated zooming never compounds.
    styles: StyleMap,
    /// The document's persistent JS context, kept alive so listeners registered by the page's
    /// scripts survive to fire on user input (a click, an input). `None` if scripts failed to
    /// load or (always) without the `spidermonkey` feature.
    js: Option<manuk_js::PageContext>,
    /// Whether any element uses `position:sticky` — gates the per-frame sticky paint pass so
    /// non-sticky pages pay nothing.
    has_sticky: bool,
    zoom: f32,
    /// Decoded raster images keyed by their `<img>` node, painted into each element's box.
    images: std::collections::HashMap<manuk_dom::NodeId, std::rc::Rc<manuk_paint::DecodedImage>>,
    /// **Per-element scroll offsets** — `overflow: auto|scroll` containers, in CSS px.
    ///
    /// Before tick 67, `element.scrollTop` was not a property at all: reading it gave `undefined`, and
    /// writing it quietly created a plain JS own-property that scrolled nothing. A virtualised list
    /// would set it, read it back, get its own value, and believe it had worked — **the failure was
    /// silent on both sides of the API.**
    scroll_offsets: std::collections::HashMap<manuk_dom::NodeId, (f32, f32)>,
    /// **The external stylesheets, kept.** They used to live only in a local map inside
    /// `fetch_and_apply_stylesheets` and were dropped on the floor afterwards — so every LATER
    /// cascade rebuilt its sheet list from `collect_style_elements`, which sees only inline
    /// `<style>`, and silently lost every `<link>`ed stylesheet on the page. A re-cascade that
    /// quietly strips the site's CSS is worse than no re-cascade.
    external_css: HashMap<String, String>,
    /// Mask/background URLs already fetched for this navigation. Same discipline as `external_css`
    /// and `images`: `fetch_and_apply_masks` and `fetch_and_apply_background_images` run once for
    /// `finish_loading` and AGAIN after every round of dynamic scripts, and each call was re-fetching
    /// every mask and every background image on the page from scratch. Part 22.3.
    fetched_urls: std::collections::HashSet<String>,
    /// Every `(img node, resolved url)` we have TRIED to fetch — successes and failures alike.
    /// A failure that is not remembered is a fetch that repeats on every round; see
    /// `fetch_images_owned`.
    image_attempts: std::collections::HashSet<(manuk_dom::NodeId, String)>,
    /// `<iframe>`s already rendered, by node → the URL they show. Prevents re-fetching an embed on
    /// every round, which is the same storm `image_by_url` exists to stop.
    iframes: std::collections::HashMap<manuk_dom::NodeId, String>,
    /// **The documents inside the frames — kept, not thrown away.**
    ///
    /// They were always built (a real arena, real styles, real scripts, laid out at the frame's own
    /// width), painted to a bitmap, and dropped. The pixels survived and the document did not, so
    /// `iframe.contentDocument` was `undefined` — and the platform web *is* other people's documents
    /// inside yours: embeds, OAuth frames, payment fields, players, comment widgets.
    ///
    /// The JS side holds each arena's **address** (in a reflector's `SLOT_DOM`), and a `HashMap` moves
    /// its values around as it grows — but `Page::dom` is already a `Box<Dom>`, so what moves is the
    /// pointer, never the arena it points at. No second box is needed, and adding one would just be a
    /// second indirection on every child access.
    child_pages: std::collections::HashMap<manuk_dom::NodeId, Page>,
    /// Decoded images keyed by **resolved URL**. `None` means "we asked and did not get it" — a
    /// remembered failure, which is what stops a blocked tracker being re-fetched on every round.
    /// Nine elements naming one sprite share one entry, one fetch and one decode.
    image_by_url: std::collections::HashMap<String, Option<std::rc::Rc<manuk_paint::DecodedImage>>>,
    /// Fingerprint of the inputs to the last full cascade — the style sources and the shape of the
    /// tree. If neither has changed, re-cascading produces byte-identical output, and on a large
    /// document that is not a small waste: see `apply_stylesheets`.
    last_cascade: Option<u64>,
}

/// E1 full-page zoom bounds (matching what mainstream browsers offer).
pub const MIN_ZOOM: f32 = 0.25;
pub const MAX_ZOOM: f32 = 5.0;

/// The **top layer** — the stacking level a modal `<dialog>` is promoted to, above every author
/// `z-index`. Deliberately far above any number a stylesheet would plausibly write (the web's
/// "just make it win" idiom tops out around `z-index: 2147483647`, but real sheets live in the
/// hundreds), and below `i32::MAX` so nothing that adds to it overflows.
pub const TOP_LAYER_Z: i32 = 1_000_000_000;

/// **Bar 0 containment (METHODOLOGY Part 23.2): a panic kills the PAGE, not the process.**
///
/// You will not prevent every crash-class bug before Bar 1. That is not pessimism, it is the premise
/// of the whole 99%-pattern-coverage strategy (Part 24): the tail of patterns we do not yet cover is
/// where the panics live, and the tail is infinite. So the requirement is not "never panic" — it is
/// that a failure on ONE page is contained to that page.
///
/// apple.com core-dumped this browser. The specific cause (a node the cascade never saw) is fixed,
/// and fixing it was right — but a fix for one instance is not containment of a class, and the next
/// uncovered pattern will find the next panic. The failure mode for an uncovered pattern must be
/// "this tab shows an error and the browser carries on", never "everything the user had open is
/// gone".
///
/// Returns `None` if the page's own code brought it down. The caller shows an error page; the
/// browser lives. `panic = "unwind"` in the release profile is what makes this possible at all — with
/// `abort` it could not exist, which is precisely why it was removed.
///
/// **What this does NOT cover, stated honestly:** a fault raised inside SpiderMonkey's own C++ frames
/// cannot be caught here, because unwinding across that FFI boundary is undefined behaviour rather
/// than a catchable panic. Containing *that* needs the panic caught inside the Rust callback before
/// it returns to C++ (done at the six binding catch sites) or, ultimately, a per-tab process. This
/// boundary covers the Rust render path — parse, cascade, layout, paint — which is where every crash
/// this project has actually seen has come from.
pub fn contained<T>(what: &str, f: impl FnOnce() -> T) -> Option<T> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(v) => Some(v),
        Err(e) => {
            let msg = e
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| e.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "(non-string panic payload)".into());
            // Part 22.1: no silent failure. A contained panic is still a BUG, and it must reach the
            // discovery pipeline rather than being quietly absorbed by the thing that makes it
            // survivable.
            tracing::error!(
                %what,
                panic = %msg,
                "CONTAINED PANIC — this page died, the browser did not. This is a real bug and it \
                 belongs in the oracle's crash signal (Part 24.3), which outranks every visual \
                 divergence in the ledger."
            );
            None
        }
    }
}

/// How long a page's *enhancements* get, in total, before the document paints regardless.
/// How deep an `<iframe>` tree may go. An iframe containing an iframe is normal; a page that frames
/// itself is a fork bomb. Each level is rasterized to a bitmap here, so the cost compounds and the
/// returns do not.
pub const MAX_IFRAME_DEPTH: u8 = 2;

pub fn load_budget() -> std::time::Duration {
    static B: std::sync::OnceLock<std::time::Duration> = std::sync::OnceLock::new();
    *B.get_or_init(|| {
        let ms = std::env::var("MANUK_LOAD_BUDGET_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(12_000);
        std::time::Duration::from_millis(ms)
    })
}

/// The scroll geometry of every `overflow: auto|scroll|hidden` container:
/// `[scrollTop, scrollLeft, scrollHeight, scrollWidth, clientHeight, clientWidth]`.
///
/// A free function, not a method, because the **inline** scripts run inside `from_dom` — before a `Page`
/// exists — and a virtualised list that reads `clientHeight` at boot (they all do) must not be handed a
/// zero. A capability that only works after the deferred pass is a capability that works on half the web.
fn scroll_geometry_of(
    root_box: &manuk_layout::LayoutBox,
    styles: &StyleMap,
    offsets: &std::collections::HashMap<manuk_dom::NodeId, (f32, f32)>,
) -> std::collections::HashMap<manuk_dom::NodeId, [f32; 6]> {
    use manuk_css::Overflow;
    let mut m = std::collections::HashMap::new();
    for (node, st) in styles.iter() {
        if !matches!(
            st.overflow,
            Overflow::Auto | Overflow::Scroll | Overflow::Hidden
        ) {
            continue;
        }
        let Some(b) = root_box.find(*node) else {
            continue;
        };
        let bw = &st.border_width;
        let client_w = (b.rect.width - bw.left - bw.right).max(0.0);
        let client_h = (b.rect.height - bw.top - bw.bottom).max(0.0);
        let (cw, ch) = b.content_extent();
        let (sx, sy) = offsets.get(node).copied().unwrap_or((0.0, 0.0));
        // The extent is measured on the ALREADY-SCROLLED tree, so add the offset back: the content did
        // not get shorter because the user scrolled down it.
        m.insert(
            *node,
            [
                sy,
                sx,
                (ch + sy).max(client_h),
                (cw + sx).max(client_w),
                client_h,
                client_w,
            ],
        );
    }
    m
}

impl Page {
    /// Parse + style + lay out `html` for a viewport of `viewport_width` px.
    pub fn load(html: &str, final_url: &str, fonts: &FontContext, viewport_width: f32) -> Page {
        let mut page = Page::from_dom(manuk_html::parse(html), final_url, fonts, viewport_width);
        // **Both passes, back to back.** `from_dom` runs only the scripts that block first paint; the
        // deferred ones (`defer`, `async`, `type=module`) run here. So this function behaves exactly as
        // it always has — every gate, and the whole SPA suite, sees all scripts run before it asserts
        // anything.
        //
        // The SHELL is the only caller that separates them: blocking → paint → deferred → repaint. That
        // is the only place a human is waiting, and it is the only place the difference is visible.
        page.run_deferred_scripts(fonts, viewport_width);
        // Parsing is done and the deferred scripts have executed — that IS DOMContentLoaded.
        page.fire_lifecycle("DOMContentLoaded");
        // On the SYNC path there is no async runtime to fetch subframes with; a caller that needs a
        // frame's document (a gate) drives `render_iframe` directly. The async path
        // (`load_async`/`finish_loading`) is where real frames load.
        page.fire_lifecycle("load");
        page
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
        // Preload scanner: kick off render-blocking subresource fetches (external CSS,
        // <link rel=preload>) concurrently *before* parse + layout + scripts run, so they
        // land in the HTTP cache by the time apply_stylesheets needs them — overlapping the
        // network with the CPU-bound pipeline.
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            for url in scan_preloads(html, final_url) {
                handle.spawn(async move {
                    let _ = manuk_net::fetch(&url).await;
                });
            }
        }
        #[allow(unused_mut)]
        let mut dom = manuk_html::parse(html);
        #[cfg(feature = "spidermonkey")]
        fetch_external_scripts(&mut dom, final_url).await;
        let mut page = Page::from_dom(dom, final_url, fonts, viewport_width);
        // **Both passes, back to back** — see `load`. `from_dom` runs only the scripts that block first
        // paint; the deferred ones (`defer`, `async`, `type=module`) run here.
        //
        // Missing this call broke every SPA in the suite the moment the split landed, and did it
        // *silently*: a Vite bundle is `type="module"`, which is deferred by default, so with nothing
        // running the deferred pass the app simply never mounted. The root element was still there, the
        // right size, empty. That is the exact shape of failure this project keeps re-learning — so the
        // rule is worth stating rather than remembering: **every path that used to run all the scripts
        // must still run all the scripts.** There is exactly one caller allowed to split them, and it is
        // the shell, because it is the only one with a human waiting.
        page.run_deferred_scripts(fonts, viewport_width);
        // Parsing is done and the deferred scripts have executed — that IS DOMContentLoaded.
        page.fire_lifecycle("DOMContentLoaded");
        // **Subframes load BEFORE `load` fires** — `load` waits for subframes (HTML spec), and a page's
        // `<body onload>` is precisely where it reaches into them. Firing `load` first made the entire
        // `encoding` suite (767k subtests) read a not-yet-loaded frame and throw. Idempotent with the
        // pass in `finish_loading`: `pending_iframes` skips any frame already rendered.
        #[cfg(feature = "spidermonkey")]
        page.fetch_and_load_iframes(fonts, viewport_width).await;
        // The subresource phases have not run yet, but the document and its frames are ready, which is
        // what `load` waits for. The call is idempotent, so `finish_loading` firing it again is harmless.
        page.fire_lifecycle("load");

        // **The enhancement phases run under the load budget, exactly as they do in `finish_loading`.**
        //
        // They did not, and the omission is a Bar 0 hole rather than a slow path: `finish_loading` was
        // budgeted and `load_async` — which runs the *same two subresource phases* — was not. A page
        // whose images and masks are slow (or numerous, which for a news front page they always are)
        // could therefore sit here without any deadline at all, and the tab is frozen for as long as it
        // takes. Whatever bounds one of these phases must bound both, or the bound is decorative.
        //
        // The document is already parsed and `page` already exists at this point, so expiry costs the
        // enhancements and never the article. That is the same promise `finish_loading` makes, and it
        // is what a browser actually promises: Chromium does not wait for the last tracking pixel
        // before showing you the story.
        let budget = load_budget();
        let enhancements = async {
            page.fetch_and_apply_images(fonts, viewport_width).await;
            page.fetch_and_apply_masks().await;
        };
        if tokio::time::timeout(budget, enhancements).await.is_err() {
            tracing::warn!(
                "load budget of {:.1}s exhausted during initial subresource load — painting now. \
                 The article is never the thing we drop.",
                budget.as_secs_f32()
            );
        }
        page
    }

    /// **The page-load budget: the document renders, full stop.**
    ///
    /// A per-request deadline (`manuk_net::request_timeout`) bounds any single fetch, but it does not
    /// bound the *page*: these phases are serial by necessity (a stylesheet can add an image, a
    /// script can add a stylesheet), so a site with dead subresources in several phases still stacks
    /// timeout upon timeout. Worst case across the six phases is ~64s, which is a frozen tab with
    /// extra steps.
    ///
    /// So the phases run under one overall deadline. When it expires, whatever has arrived is what
    /// the page gets, and it paints. **This is what a browser actually promises.** Chromium does not
    /// wait for a tracker to answer before showing you the article, and neither does this.
    ///
    /// The budget starts after parse, so the document — the thing the user came for — is never the
    /// thing that gets dropped. Only the enhancements are abandonable, and abandoning them is the
    /// correct behaviour, not a degradation of it.
    ///
    /// `MANUK_LOAD_BUDGET_MS` overrides.
    #[cfg(feature = "spidermonkey")]
    pub async fn finish_loading(&mut self, fonts: &FontContext, viewport_width: f32) {
        // **A hard deadline, not a between-phases courtesy.** Checking the clock only *between*
        // phases lets a phase start with a millisecond left and then run for its full per-request
        // timeout, so a 12s budget delivered 21.6s. Wrapping the whole sequence means the clock is
        // enforced wherever it runs out, including in the middle of a fetch.
        //
        // Cancelling mid-phase is safe by construction: each phase fetches everything it needs and
        // only then applies it to the DOM, so a dropped future loses that phase's *enhancement* and
        // never a half-mutated document. Earlier phases keep what they already applied.
        let budget = load_budget();
        if tokio::time::timeout(budget, self.finish_loading_inner(fonts, viewport_width))
            .await
            .is_err()
        {
            tracing::warn!(
                "load budget of {:.1}s exhausted mid-phase — painting now. The document is what the \
                 user came for; the rest was an enhancement.",
                budget.as_secs_f32()
            );
        }
        // **`load` fires either way.** A real browser fires it when loading settles; it does not
        // withhold the event forever because one subresource was slow. Withholding it would leave
        // every `window.onload` handler on the page unrun — which is the bug this exists to fix.
        self.fire_lifecycle("load");
    }

    #[cfg(feature = "spidermonkey")]
    async fn finish_loading_inner(&mut self, fonts: &FontContext, viewport_width: f32) {
        let budget = load_budget();
        let started = std::time::Instant::now();
        let mut phase = |name: &'static str| -> bool {
            let left = budget.saturating_sub(started.elapsed());
            if left.is_zero() {
                tracing::warn!(
                    "load budget of {:.1}s exhausted — painting without {name}. The page renders; \
                     the subresource was an enhancement, not a hostage.",
                    budget.as_secs_f32()
                );
                return false;
            }
            true
        };
        if phase("external CSS") {
            self.fetch_and_apply_stylesheets(fonts, viewport_width)
                .await;
        }
        if phase("dynamic scripts") {
            self.fetch_and_run_dynamic_scripts(fonts, viewport_width, 4)
                .await;
        }
        // **Subframes — BEFORE `load` fires, because `load` waits for them and because that is where
        // pages reach into their frames.** `<body onload="showNodes(...)">` reading
        // `iframe.contentWindow.document` is not an exotic pattern; it is WPT's entire `encoding` suite
        // and it is every embed script in the wild.
        //
        // This ran only in the GUI before, so the test runner, the oracle and every headless render
        // loaded pages whose frames were simply never fetched — the same class of defect as the
        // never-drained fetch queue two paragraphs down. A harness that cannot load a subframe is not
        // measuring the browser; it is measuring itself.
        if phase("subframes") {
            self.fetch_and_load_iframes(fonts, viewport_width).await;
        }
        // **The page's own `fetch()`/XHR calls — PERFORMED, not just queued.**
        //
        // They never were. `take_fetches()` handed them to the SHELL, and the shell alone performed
        // them. So every consumer that is not the shell — the oracle, `boxes`, the agent, any headless
        // render — queued a data-driven SPA's API calls and **never made them.** The app sat in its
        // loading state forever and rendered a skeleton.
        //
        // Found through aljazeera.com: 2,131 server-rendered elements were replaced by a 19-element
        // shell. React had cleared its container (normal for `createRoot`) and re-rendered — correctly —
        // with no data, because the data request was sitting in a queue nobody was draining.
        //
        // **This is why the oracle reported 13,741 "missing" nodes.** A measurement harness that cannot
        // load a modern site's content is not measuring the browser; it is measuring itself. Every
        // data-driven SPA in the corpus was being scored against a skeleton.
        if phase("page fetches") {
            self.pump_page_fetches(fonts, viewport_width, &started, budget)
                .await;
        }
        if phase("images") {
            self.fetch_and_apply_images(fonts, viewport_width).await;
        }
        if phase("icon masks") {
            self.fetch_and_apply_masks().await;
        }
        if phase("background images") {
            self.fetch_and_apply_background_images().await;
        }
    }

    /// Perform the `fetch()`/XHR requests the page's scripts issued, settle them, and repeat — because
    /// settling one request routinely issues the next (a page fetches its config, then its content).
    ///
    /// Bounded three ways, because an app that fetches in a loop must not own the tab:
    ///   * the **load budget**, shared with every other phase;
    ///   * a **round ceiling** — a page that keeps issuing new requests after this many rounds is not
    ///     converging, and waiting longer will not help;
    ///   * `manuk_net`'s per-request deadline, and its single-flight + negative caches, so a dead
    ///     endpoint is asked once and remembered.
    #[cfg(feature = "spidermonkey")]
    async fn pump_page_fetches(
        &mut self,
        fonts: &FontContext,
        viewport_width: f32,
        started: &std::time::Instant,
        budget: std::time::Duration,
    ) {
        const MAX_ROUNDS: usize = 6;
        for round in 0..MAX_ROUNDS {
            if budget.saturating_sub(started.elapsed()).is_zero() {
                tracing::warn!(
                    round,
                    "load budget exhausted with page fetches still in flight"
                );
                return;
            }
            let mut reqs = self.take_fetches();
            if reqs.is_empty() {
                return;
            }
            // **A ceiling per round.** An analytics-heavy page can queue hundreds of beacons in one
            // tick; performing all of them is work the user did not ask for and will not see. The
            // document's own data requests come first (they are issued first), so a prefix is the right
            // truncation — and it is LOGGED, because a silent cap reads as "we did everything".
            const MAX_PER_ROUND: usize = 40;
            if reqs.len() > MAX_PER_ROUND {
                tracing::warn!(
                    queued = reqs.len(),
                    performed = MAX_PER_ROUND,
                    "page queued more fetches than one round performs — the rest are dropped"
                );
                reqs.truncate(MAX_PER_ROUND);
            }
            tracing::debug!(round, n = reqs.len(), "performing page fetches");

            // Concurrent — a page's requests are independent of one another, and serialising them is
            // how a five-request page becomes a five-deadline page.
            let base = self.final_url.clone();
            let left = budget.saturating_sub(started.elapsed());
            let all = futures_util::future::join_all(reqs.into_iter().map(
                |(id, raw, method, headers, body)| {
                    let url = resolve_url(&base, &raw);
                    // The page's own document URL is the initiator of this `fetch`/XHR, so the net
                    // layer applies `SameSite`: a cross-site request withholds this page's `Lax`/
                    // `Strict` cookies (the CSRF/credential-leak fix). Cloned per request because the
                    // futures are moved into `join_all`.
                    let initiator = base.clone();
                    async move {
                        let is_get = method.eq_ignore_ascii_case("GET") || method.is_empty();
                        let out = if is_get && headers.is_empty() {
                            // A header-less GET goes through `fetch`, which carries the HTTP cache, the
                            // single-flight coalescer and the per-navigation negative cache. A POST must
                            // not: it is not idempotent, and de-duplicating one would drop a real request.
                            // A GET WITH request headers (an `Authorization: Bearer …` API read) also
                            // bypasses that path — it is not safely shareable across auth contexts, and
                            // dropping its headers was the bug this fixes.
                            manuk_net::fetch_from(&url, Some(&initiator)).await
                        } else {
                            let mut hdrs: Vec<(&str, &str)> = headers
                                .iter()
                                .map(|(k, v)| (k.as_str(), v.as_str()))
                                .collect();
                            // Default `Content-Type` for a bodied request only when the page did not set
                            // one — overriding an explicit `application/x-www-form-urlencoded` would break
                            // every classic form-POST and every OAuth token exchange.
                            let has_ct = headers
                                .iter()
                                .any(|(k, _)| k.eq_ignore_ascii_case("content-type"));
                            if !is_get && !has_ct {
                                hdrs.push(("content-type", "application/json"));
                            }
                            let m = if method.is_empty() {
                                "GET"
                            } else {
                                method.as_str()
                            };
                            manuk_net::request_from(
                                m,
                                &url,
                                &hdrs,
                                body.clone().into_bytes().into(),
                                Some(&initiator),
                            )
                            .await
                        };
                        match out {
                            Ok(r) => (id, r.status, r.decoded_text(), r.headers),
                            // status 0 is the fetch API's "network failure" — the page's `.catch` runs, which
                            // is a path it was written for. Silence is not.
                            Err(e) => {
                                tracing::warn!(%url, "page fetch failed: {e}");
                                (id, 0u16, String::new(), Vec::new())
                            }
                        }
                    }
                },
            ));

            // **The ROUND lives inside the budget, not merely between rounds.** Checking the clock only
            // at the top of the loop let a single round run unbounded — a 20s budget produced a 200s+
            // load, which is a Bar 0 regression and not a slow path. Whatever arrived by the deadline is
            // what the page gets, which is the same promise every other phase makes.
            let Ok(results) = tokio::time::timeout(left, all).await else {
                tracing::warn!(
                    round,
                    "page fetches exceeded the load budget — settling with what arrived"
                );
                return;
            };

            for (id, status, body, headers) in results {
                self.resolve_fetch(id, status, &body, &headers, fonts, viewport_width);
            }
        }
        tracing::debug!(
            "page fetches hit the {MAX_ROUNDS}-round ceiling — the page is still issuing requests"
        );
    }

    /// Without SpiderMonkey there are no dynamic scripts to run.
    #[cfg(not(feature = "spidermonkey"))]
    pub async fn finish_loading(&mut self, fonts: &FontContext, viewport_width: f32) {
        let budget = load_budget();
        let started = std::time::Instant::now();
        macro_rules! within {
            () => {
                !budget.saturating_sub(started.elapsed()).is_zero()
            };
        }
        if within!() {
            self.fetch_and_apply_stylesheets(fonts, viewport_width)
                .await;
        }
        if within!() {
            self.fetch_and_apply_images(fonts, viewport_width).await;
        }
        if within!() {
            self.fetch_and_apply_masks().await;
        }
        if within!() {
            self.fetch_and_apply_background_images().await;
        }
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
        // Same discipline for images: `finish_loading` and every script round both call this, and
        // re-fetching an image we have already decoded is pure waste — and, for a `<img>` whose src
        // a script has not touched, it is waste on every single round.
        let (images, tried) = fetch_images_except(
            &self.dom,
            &self.final_url,
            &self.image_attempts,
            &mut self.image_by_url,
        )
        .await;
        self.image_attempts.extend(tried);
        self.apply_images(images, fonts, viewport_width)
    }

    /// **Run the scripts that do not block first paint** — `defer`, `async`, `type="module"`.
    ///
    /// The shell calls this *after* the document is on screen; `Page::load` and `from_prefetched` call
    /// it immediately, so every gate and the whole SPA suite see exactly the behaviour they always have.
    ///
    /// Returns how many scripts ran. A non-zero count means the tree may have changed, so the cascade
    /// and layout are redone — the same thing the blocking pass already does, for the same reason.
    #[cfg(feature = "spidermonkey")]
    pub fn run_deferred_scripts(&mut self, fonts: &FontContext, viewport_width: f32) -> usize {
        let Some(ctx) = self.js.as_ref() else {
            return 0;
        };
        let rects: std::collections::HashMap<manuk_dom::NodeId, [f32; 4]> = self
            .root_box
            .node_rects(&self.dom)
            .into_iter()
            .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
            .collect();
        manuk_js::set_scroll_geometry(self.scroll_geometry_map());
        let ran = match manuk_js::run_deferred_scripts(ctx, &mut self.dom, &rects, &self.styles) {
            Ok(n) => n,
            Err(e) => {
                // Surfaced, never swallowed — G_SILENT_FAIL. A deferred script that dies silently is
                // how "the app renders nothing and throws nothing" happens, and that cost several ticks.
                tracing::warn!("deferred scripts: {e}");
                0
            }
        };
        self.drain_canvases();
        self.drain_element_scrolls();
        if ran > 0 {
            tracing::debug!(scripts = ran, "executed deferred page scripts");
            let sheets: Vec<Stylesheet> = MinimalCascade::collect_style_elements(&self.dom);
            self.styles = cascade_styles(&self.dom, &sheets, viewport_width);
            self.root_box = layout_document(&self.dom, &self.styles, fonts, viewport_width);
            self.reapply_scroll_offsets();
            self.content_height = self.root_box.rect.height;
        }
        ran
    }

    #[cfg(not(feature = "spidermonkey"))]
    pub fn run_deferred_scripts(&mut self, _fonts: &FontContext, _viewport_width: f32) -> usize {
        0
    }

    /// **The `<iframe>`s this page wants to load** — `(node, resolved url, width, height)`.
    ///
    /// Cheap (a DOM walk plus a rect lookup), and taken on the UI thread so the *fetch* can happen off
    /// it. An iframe is 23% of the corpus and it is the gateway to embeds, maps, players, payment frames
    /// and comment widgets — most of what makes a page feel like the modern web.
    ///
    /// **Fetched AFTER first paint, exactly like images.** A heavy third-party embed must not hold the
    /// parent's article hostage; that is the same lesson `G_FIRST_PAINT` exists to enforce, and an
    /// `<iframe>` is the single most likely thing on a page to be slow.
    pub fn pending_iframes(&self) -> Vec<(manuk_dom::NodeId, String, u32, u32)> {
        let rects = self.root_box.node_rects(&self.dom);
        let mut out = Vec::new();
        for n in self.dom.flat_descendants(self.dom.root()) {
            if self.dom.tag_name(n) != Some("iframe") {
                continue;
            }
            if self.iframes.contains_key(&n) {
                continue; // already rendered
            }
            let Some(el) = self.dom.element(n) else {
                continue;
            };
            // `srcdoc` beats `src`, per spec. A `src` of `about:blank` has nothing to fetch.
            let src = el.attr("src").unwrap_or("").trim();
            if src.is_empty() || src.starts_with("about:") || src.starts_with("javascript:") {
                continue;
            }
            // **A `display:none` iframe still loads.** This used to `continue` when the element had no
            // layout box, which conflates two different questions: *should this document load?* (a DOM
            // question) and *is there anywhere to draw it?* (a layout question). A hidden frame answers
            // no to the second and **yes to the first** — that is what a hidden frame is FOR: analytics
            // beacons, OAuth relays, payment bridges, `postMessage` shims, prefetch.
            //
            // It cost 767,003 subtests. WPT's entire `encoding` suite hides its frame —
            // `<style> iframe { display:none } </style>` — because the frame is a *data source*, not a
            // picture; the test then reads its `contentDocument`. We refused to fetch it, so there was
            // nothing to read, and the failure looked like a character-encoding bug for eighty-three
            // ticks. The box is a painting decision. It was never a loading decision.
            let (w, h) = match rects.get(&n) {
                Some(r) if r.width >= 1.0 && r.height >= 1.0 => {
                    (r.width.round() as u32, r.height.round() as u32)
                }
                // No box: load it at the spec's default frame size, which is only used as the child's
                // viewport width. Nothing will be painted — see `render_iframe`.
                _ => (300, 150),
            };
            out.push((n, resolve_url(&self.final_url, src), w, h));
        }
        out
    }

    /// Every arena this `Page` owns — its own, and its frames'.
    fn owned_arenas(&mut self) -> Vec<*mut manuk_dom::Dom> {
        let mut v = vec![&mut *self.dom as *mut manuk_dom::Dom];
        v.extend(
            self.child_pages
                .values_mut()
                .map(|c| &mut *c.dom as *mut manuk_dom::Dom),
        );
        v
    }

    /// Render a fetched iframe document into the iframe's box.
    ///
    /// **The child is a whole `Page`** — its own DOM, its own cascade, its own layout, and (crucially)
    /// **its own JS context**. That is the isolation, and it comes for free from the architecture rather
    /// than from a policy anyone has to remember: a `PageContext` is per-`Page`, so a child's script has
    /// no path to the parent's DOM. It cannot reach it because it does not have it.
    ///
    /// The child is then **rasterized to a bitmap and blitted through the replaced-element path** — the
    /// same one an `<img>` uses. That is a deliberate scope choice and its limits are real and should be
    /// stated rather than discovered: the embed renders, but it does not scroll, and it does not update.
    /// A rendered embed you cannot scroll is enormously better than a 300×150 hole, and it is a fraction
    /// of the work of a live nested browsing context — which is where this goes next.
    pub fn render_iframe(
        &mut self,
        node: manuk_dom::NodeId,
        html: &str,
        url: &str,
        fonts: &FontContext,
        depth: u8,
    ) {
        // **Depth limit.** An iframe whose document contains an iframe is normal; a page that frames
        // itself is a fork bomb. Chromium's limit is far higher, but ours renders each level to a
        // bitmap, so the cost compounds and the returns do not.
        if depth >= MAX_IFRAME_DEPTH {
            tracing::warn!(depth, "iframe nesting limit reached — not rendering deeper");
            return;
        }
        // **No box is not a reason not to load** — see `pending_iframes`. It is a reason not to *paint*.
        let rects = self.root_box.node_rects(&self.dom);
        let (w, h) = match rects.get(&node) {
            Some(r) if r.width >= 1.0 && r.height >= 1.0 => (
                r.width.round().max(1.0) as u32,
                r.height.round().max(1.0) as u32,
            ),
            _ => (300, 150),
        };
        let visible = rects
            .get(&node)
            .is_some_and(|r| r.width >= 1.0 && r.height >= 1.0);

        // A whole page, at the iframe's own viewport width — so its media queries and its layout see the
        // size of the frame, not the size of the window. That is what makes a responsive embed responsive.
        let mut child = Page::load(html, url, fonts, w as f32);
        if visible {
            let canvas = child.paint(fonts, w, h);
            let img = manuk_paint::DecodedImage {
                width: canvas.width(),
                height: canvas.height(),
                rgba: canvas.rgba_bytes().to_vec(),
            };
            self.images.insert(node, std::rc::Rc::new(img));
        }
        self.iframes.insert(node, url.to_string());

        // **Keep the document.** Everything above this line already existed; the child was built in full
        // and then thrown away, which is why `contentDocument` was `undefined` for eighty-three ticks.
        // Registering the arena is what makes a reflector into it *safe* — see `manuk_js::register_dom`.
        manuk_js::register_dom(&mut *child.dom as *mut manuk_dom::Dom);
        self.child_pages.insert(node, child);
        self.publish_iframe_docs();
    }

    /// Tell the JS world which arena sits behind each `<iframe>`. Cheap, and it must run before any
    /// script that might reach into a frame — which, once frames are loaded, is any script at all.
    fn publish_iframe_docs(&mut self) {
        let m: std::collections::HashMap<_, _> = self
            .child_pages
            .iter_mut()
            .map(|(&n, c)| {
                let root = c.dom.root();
                (n, (&mut *c.dom as *mut manuk_dom::Dom as usize, root))
            })
            .collect();
        manuk_js::set_iframe_docs(m);
    }

    /// Fetch every `<iframe src>` and load it as a real document.
    ///
    /// **Before `load` fires**, because that is the spec (`load` waits for subframes) and because it is
    /// the whole point: `<body onload="...">` is where a page reaches into its frames. WPT's entire
    /// `encoding` suite — 767,003 subtests, 91% of the measured universe — is exactly that shape.
    async fn fetch_and_load_iframes(&mut self, fonts: &FontContext, _viewport_width: f32) {
        let pending = self.pending_iframes();
        if pending.is_empty() {
            return;
        }
        // One round of fetches, concurrently. A frame inside a frame is handled by the child's own
        // load — `MAX_IFRAME_DEPTH` is the fork-bomb guard.
        let fetches = pending.into_iter().map(|(node, url, _w, _h)| async move {
            let (html, final_url) = fetch_html(&url).await.ok()?;
            Some((node, html, final_url))
        });
        let got: Vec<_> = futures_util::future::join_all(fetches)
            .await
            .into_iter()
            .flatten()
            .collect();
        for (node, html, final_url) in got {
            self.render_iframe(node, &html, &final_url, fonts, 0);
        }
    }

    // ── ELEMENT SCROLLING (`overflow: auto|scroll`) ────────────────────────────────────────────────
    //
    // `scrollTop` was the roadmap's #2 item and it was the worst kind of gap: **not missing, lying.**
    // Reading gave `undefined`; writing created a plain JS own-property that scrolled nothing, so a
    // virtualised list set it, read it back, got its own value, and believed it had worked.

    /// The scrollable geometry of `node`, as the DOM reports it:
    /// `(scrollTop, scrollLeft, scrollHeight, scrollWidth, clientHeight, clientWidth)`.
    ///
    /// `clientHeight/Width` are the **padding box** (the visible window), `scrollHeight/Width` the
    /// content's full extent — and `scrollHeight - clientHeight` is exactly the number every
    /// virtualised list divides by to decide which slice of the data to render. Give it a wrong number
    /// and it renders the wrong rows; give it `undefined` and it renders `NaN` of them, which is none.
    pub fn scroll_geometry(&self, node: manuk_dom::NodeId) -> Option<[f32; 6]> {
        scroll_geometry_of(&self.root_box, &self.styles, &self.scroll_offsets)
            .get(&node)
            .copied()
    }

    /// Scroll `node` to `(left, top)`, clamped to what there is to scroll. Returns the clamped offset
    /// actually applied — the DOM must read back the *clamped* value, not the value that was asked for,
    /// or a list that scrolls to `1e9` to reach the bottom believes it is a billion pixels down.
    pub fn set_element_scroll(
        &mut self,
        node: manuk_dom::NodeId,
        left: f32,
        top: f32,
    ) -> (f32, f32) {
        // Only a real scroll container scrolls. `overflow: visible` does not, and neither does an
        // element that has no box.
        let scrollable = matches!(
            self.styles.get(&node).map(|s| s.overflow),
            Some(manuk_css::Overflow::Auto)
                | Some(manuk_css::Overflow::Scroll)
                | Some(manuk_css::Overflow::Hidden)
        );
        if !scrollable {
            return (0.0, 0.0);
        }
        let Some(g) = self.scroll_geometry(node) else {
            return (0.0, 0.0);
        };
        let max_y = (g[2] - g[4]).max(0.0);
        let max_x = (g[3] - g[5]).max(0.0);
        let new = (left.clamp(0.0, max_x), top.clamp(0.0, max_y));
        let old = self
            .scroll_offsets
            .get(&node)
            .copied()
            .unwrap_or((0.0, 0.0));
        if (new.0 - old.0).abs() < 0.01 && (new.1 - old.1).abs() < 0.01 {
            return old;
        }
        // Move the subtree by the DELTA, not the absolute offset — the tree already carries the old
        // one. Translating by the absolute value every time would scroll the content cumulatively, one
        // full offset per assignment, which looks exactly like a runaway scroll bug.
        if let Some(b) = self.root_box.find_mut(node) {
            let (dx, dy) = (-(new.0 - old.0), -(new.1 - old.1));
            if let manuk_layout::BoxContent::Block(kids) = &mut b.content {
                for k in kids.iter_mut() {
                    k.translate(dx, dy);
                }
            }
        }
        self.scroll_offsets.insert(node, new);
        new
    }

    /// Apply the scroll offsets to a freshly laid-out tree.
    ///
    /// Layout starts from zero every time, so a re-layout silently un-scrolls every container on the
    /// page — the user types in a chat box, the list jumps back to the top. This is called after every
    /// re-layout for that reason.
    fn reapply_scroll_offsets(&mut self) {
        let offsets: Vec<(manuk_dom::NodeId, (f32, f32))> =
            self.scroll_offsets.iter().map(|(k, v)| (*k, *v)).collect();
        for (node, (sx, sy)) in offsets {
            if let Some(b) = self.root_box.find_mut(node) {
                if let manuk_layout::BoxContent::Block(kids) = &mut b.content {
                    for k in kids.iter_mut() {
                        k.translate(-sx, -sy);
                    }
                }
            }
        }
    }

    /// Every `overflow: auto|scroll|hidden` container's scroll geometry — what the DOM must report.
    fn scroll_geometry_map(&self) -> std::collections::HashMap<manuk_dom::NodeId, [f32; 6]> {
        scroll_geometry_of(&self.root_box, &self.styles, &self.scroll_offsets)
    }

    /// Apply the `element.scrollTop = n` assignments a script just made, and **tell the page it
    /// scrolled**.
    ///
    /// The `scroll` event is not a nicety: an infinite-scroll feed listens for it to fetch the next
    /// page, and a sticky header listens for it to pin itself. A scroll that moves the pixels and fires
    /// nothing is half a scroll.
    fn drain_element_scrolls(&mut self) {
        let reqs = manuk_js::take_element_scrolls();
        if reqs.is_empty() {
            return;
        }
        let mut moved = Vec::new();
        for (node, left, top) in reqs {
            let before = self
                .scroll_offsets
                .get(&node)
                .copied()
                .unwrap_or((0.0, 0.0));
            let after = self.set_element_scroll(node, left, top);
            if after != before {
                moved.push(node);
            }
        }
        // Tell the page it scrolled.
        #[cfg(feature = "spidermonkey")]
        for node in moved {
            let Some(ctx) = &self.js else { break };
            let rects: HashMap<manuk_dom::NodeId, [f32; 4]> = self
                .root_box
                .node_rects(&self.dom)
                .into_iter()
                .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
                .collect();
            if let Err(e) =
                manuk_js::dispatch_event(ctx, &mut self.dom, node, "scroll", &rects, &self.styles)
            {
                tracing::warn!("scroll event: {e}");
            }
        }
        #[cfg(not(feature = "spidermonkey"))]
        let _ = moved;
    }

    /// **Move anything a script painted into a `<canvas>` onto the screen.**
    ///
    /// This needs no new machinery in the painter, and that is the whole reason canvas landed in one
    /// tick. The painter already scales a `DecodedImage` into a replaced element's content box, keyed by
    /// `NodeId` — that is exactly how `<img>` works, and how an `<iframe>` is composited. **A canvas is
    /// just an image the page draws into.** So the finished pixmaps drop into the same map, and paint is
    /// none the wiser.
    ///
    /// Only *dirty* canvases cross: a chart that was drawn once must not be re-uploaded on every script
    /// round, and a megabyte-sized pixmap copied per event handler would be a performance bug wearing a
    /// feature's clothes.
    fn drain_canvases(&mut self) {
        for (id, w, h, rgba) in manuk_js::canvas_bitmaps() {
            self.images.insert(
                manuk_dom::NodeId(id),
                std::rc::Rc::new(manuk_paint::DecodedImage {
                    width: w,
                    height: h,
                    rgba,
                }),
            );
        }
    }

    /// **The images this page still wants** — resolved URLs, distinct, none already resolved.
    ///
    /// Cheap: a DOM walk, no network. The point is that it can be taken on the UI thread, handed to a
    /// background task, and the decoded result applied later — so **first paint never waits for an
    /// image**. That is the difference between a browser that feels fast and one that does not, and it
    /// is what Chromium does: the article is on screen long before the last tracking pixel arrives.
    pub fn pending_image_urls(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for n in self.dom.flat_descendants(self.dom.root()) {
            let Some(el) = self.dom.element(n) else {
                continue;
            };
            let src = match self.dom.tag_name(n) {
                Some("img") => el.attr("src"),
                Some("video") => el.attr("poster"), // a poster is a still, and we decode stills
                _ => None,
            };
            let Some(src) = src else { continue };
            let src = src.trim();
            if src.is_empty() {
                continue;
            }
            let url = resolve_url(&self.final_url, src);
            if self.image_by_url.contains_key(&url) || out.contains(&url) {
                continue;
            }
            out.push(url);
        }
        out
    }

    /// Apply images that a background task fetched, binding each URL to every node that names it.
    /// Returns how many nodes were newly filled — 0 means nothing changed and no repaint is owed.
    pub fn apply_images_by_url(
        &mut self,
        by_url: std::collections::HashMap<String, manuk_paint::DecodedImage>,
        fonts: &FontContext,
        viewport_width: f32,
    ) -> usize {
        for (url, img) in by_url {
            self.image_by_url.insert(url, Some(std::rc::Rc::new(img)));
        }
        let mut rc: std::collections::HashMap<
            manuk_dom::NodeId,
            std::rc::Rc<manuk_paint::DecodedImage>,
        > = std::collections::HashMap::new();
        for n in self.dom.flat_descendants(self.dom.root()) {
            let Some(el) = self.dom.element(n) else {
                continue;
            };
            let src = match self.dom.tag_name(n) {
                Some("img") => el.attr("src"),
                Some("video") => el.attr("poster"),
                _ => None,
            };
            let Some(src) = src else { continue };
            let url = resolve_url(&self.final_url, src.trim());
            if let Some(Some(img)) = self.image_by_url.get(&url) {
                if !self.images.contains_key(&n) {
                    rc.insert(n, img.clone());
                }
            }
            self.image_attempts.insert((n, url));
        }
        if rc.is_empty() {
            return 0;
        }
        self.apply_images(rc, fonts, viewport_width)
    }

    /// The **pure** half of image application (DEBT-1): given already-fetched+decoded images,
    /// patch natural sizes and relayout. No network — so this can run on the UI thread without
    /// blocking it, with the fetching done off-thread.
    pub fn apply_images(
        &mut self,
        images: std::collections::HashMap<
            manuk_dom::NodeId,
            std::rc::Rc<manuk_paint::DecodedImage>,
        >,
        fonts: &FontContext,
        viewport_width: f32,
    ) -> usize {
        if images.is_empty() {
            return 0;
        }
        // Natural sizing. The *aspect ratio* is the load-bearing part: an `auto` dimension of a
        // replaced element is derived from the USED value of the other one, not from the image's
        // natural pixels. Pinning `height` to the natural height here — which is what this did —
        // means a `max-width: 100%` clamp narrows the box and leaves the height alone, and the image
        // renders stretched. That reset is on essentially every site on the web.
        //
        // So: record the ratio, give `width` its natural value only when BOTH axes are auto (the
        // unconstrained case), and otherwise leave the auto axis auto for layout to derive.
        for (&node, img) in &images {
            if let Some(style) = self.styles.get_mut(&node) {
                if img.width > 0 && img.height > 0 {
                    style.aspect_ratio = Some(img.width as f32 / img.height as f32);
                }
                if style.width == manuk_css::Dim::Auto && style.height == manuk_css::Dim::Auto {
                    style.width = manuk_css::Dim::Px(img.width as f32);
                } else if style.width == manuk_css::Dim::Auto && style.aspect_ratio.is_none() {
                    style.width = manuk_css::Dim::Px(img.width as f32);
                }
                if style.height == manuk_css::Dim::Auto && style.aspect_ratio.is_none() {
                    style.height = manuk_css::Dim::Px(img.height as f32);
                }
            }
        }
        let count = images.len();
        self.images = images;
        self.relayout(fonts, viewport_width);
        count
    }

    /// **Dynamic script loading.** Fetch and run every `<script src>` the page added to its own DOM
    /// *at runtime*, then let those scripts add more, up to `max_rounds`. Returns how many ran.
    ///
    /// This is not an edge case; it is how the modern web ships code. `createElement('script')` →
    /// set `src` → `appendChild` is the shape of every code-split bundle, every lazy-loaded route,
    /// and every module loader. Wikipedia's ResourceLoader embeds each icon's CSS inside a module
    /// payload delivered exactly this way — so without it the page loads its loader and stops, and
    /// the icons that never arrive paint as bare `background-color` squares.
    ///
    /// A script's `src` attribute is **removed** once it has run, which is both how a script is
    /// marked as executed and how the load-time collector already distinguishes "inline" from
    /// "external" — so a script can never run twice.
    ///
    /// Rounds are bounded: a loader that keeps appending scripts must terminate, and a page that
    /// wants to spin is not entitled to spin the browser with it.
    #[cfg(feature = "spidermonkey")]
    pub async fn fetch_and_run_dynamic_scripts(
        &mut self,
        fonts: &FontContext,
        viewport_width: f32,
        max_rounds: usize,
    ) -> usize {
        let mut ran = 0usize;
        for _ in 0..max_rounds {
            let pending: Vec<(manuk_dom::NodeId, String)> = self
                .dom
                .descendants(self.dom.root())
                .filter(|&n| self.dom.tag_name(n) == Some("script"))
                .filter_map(|n| {
                    let src = self.dom.element(n)?.attr("src")?.trim().to_string();
                    (!src.is_empty()).then(|| (n, resolve_url(&self.final_url, &src)))
                })
                .collect();
            if pending.is_empty() {
                break;
            }
            let fetched =
                futures_util::future::join_all(pending.into_iter().map(|(node, url)| async move {
                    let text = manuk_net::fetch(&url).await.ok().map(|r| r.decoded_text());
                    (node, text)
                }))
                .await;

            let mut any = false;
            for (node, text) in fetched {
                // Mark it run *first*: a script that throws must not be retried forever.
                self.dom.remove_attr(node, "src");
                let Some(src) = text else { continue };
                if src.trim().is_empty() {
                    continue;
                }
                let Some(ctx) = &self.js else { continue };
                let rects: HashMap<manuk_dom::NodeId, [f32; 4]> = self
                    .root_box
                    .node_rects(&self.dom)
                    .into_iter()
                    .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
                    .collect();
                if let Err(e) =
                    manuk_js::eval_in_page(ctx, &mut self.dom, &src, &rects, &self.styles)
                {
                    tracing::warn!("dynamic script: {e}");
                }
                ran += 1;
                any = true;
            }
            if !any {
                break;
            }
            // Those scripts may have injected <style>/<link> and mutated the tree — restyle before
            // the next round so the next script sees the layout it actually caused.
            self.fetch_and_apply_stylesheets(fonts, viewport_width)
                .await;
            self.fetch_and_apply_masks().await;
        }
        ran
    }

    /// Type `text` into a field: set its value, then fire `input` and `change` — which is the part
    /// that matters. A framework does not read the DOM; it listens. Setting the value without
    /// dispatching the events leaves the page's own model of the form untouched, so the box shows
    /// the text and the site behaves as though the field were still empty.
    /// **Dispatch a real `submit` event to `form`.** Returns `true` if the browser should go ahead and
    /// navigate — i.e. **no listener called `preventDefault()`**.
    ///
    /// This was missing entirely, and its absence broke essentially every modern form on the web. A form
    /// on a React/Vue/Svelte page is not submitted by the browser: the page listens for `submit`, calls
    /// `preventDefault()`, and does its own `fetch()`. With no `submit` event, that handler never runs —
    /// so we performed a **full GET navigation** the author never intended, throwing away the page and
    /// its state, and the site appeared to "reload itself" whenever anyone pressed a button.
    ///
    /// Forms are **50% of the corpus** (`docs/loop/CAPABILITIES.md`), and this is the difference between
    /// a reader and a browser: search boxes, logins, checkouts.
    #[cfg(feature = "spidermonkey")]
    pub fn dispatch_submit(
        &mut self,
        form: manuk_dom::NodeId,
        fonts: &FontContext,
        viewport_width: f32,
    ) -> bool {
        let Some(ctx) = self.js.as_ref() else {
            return true;
        };
        let rects: std::collections::HashMap<manuk_dom::NodeId, [f32; 4]> = self
            .root_box
            .node_rects(&self.dom)
            .into_iter()
            .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
            .collect();
        let proceed = match manuk_js::dispatch_event(
            ctx,
            &mut self.dom,
            form,
            "submit",
            &rects,
            &self.styles,
        ) {
            Ok(default_ok) => default_ok,
            Err(e) => {
                // Surfaced, never swallowed (G_SILENT_FAIL). A submit handler that dies silently is
                // a form that mysteriously navigates away, and the user loses what they typed.
                tracing::warn!("submit dispatch: {e}");
                true
            }
        };
        // The handler may have re-rendered the page (that is the entire point of intercepting submit).
        self.relayout(fonts, viewport_width);
        proceed
    }

    #[cfg(not(feature = "spidermonkey"))]
    pub fn dispatch_submit(
        &mut self,
        _form: manuk_dom::NodeId,
        _fonts: &FontContext,
        _viewport_width: f32,
    ) -> bool {
        true
    }

    pub fn dispatch_type(
        &mut self,
        node: manuk_dom::NodeId,
        text: &str,
        fonts: &FontContext,
        viewport_width: f32,
    ) {
        self.dom.set_attr(node, "value", text.to_string());
        #[cfg(feature = "spidermonkey")]
        {
            let Some(ctx) = &self.js else {
                self.relayout(fonts, viewport_width);
                return;
            };
            let rects: HashMap<manuk_dom::NodeId, [f32; 4]> = self
                .root_box
                .node_rects(&self.dom)
                .into_iter()
                .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
                .collect();
            for ty in ["input", "change"] {
                if let Err(e) =
                    manuk_js::dispatch_event(ctx, &mut self.dom, node, ty, &rects, &self.styles)
                {
                    tracing::warn!("{ty} dispatch: {e}");
                }
            }
        }
        self.relayout(fonts, viewport_width);
    }

    /// Set the focused control's `value` to `value` and fire an **`input`** event on it — what a
    /// single keystroke does. This is the event a **controlled component** listens on: React's
    /// `onChange`, Vue's `v-model`, Svelte's `bind:value` all update their state from the `input`
    /// event and, on the next render, write the state back into the field. Without it the shell was
    /// mutating the `value` attribute directly and firing NOTHING, so a controlled input never saw
    /// the change, its state stayed stale, and the framework **reverted the user's keystroke** on the
    /// next render — every React/Vue/Svelte text field was unusable. (Fires `input` only, not
    /// `change`: `change` is a commit event — it belongs on blur/Enter, not on every keystroke.)
    pub fn dispatch_input(
        &mut self,
        node: manuk_dom::NodeId,
        value: &str,
        fonts: &FontContext,
        viewport_width: f32,
    ) {
        self.dom.set_attr(node, "value", value.to_string());
        #[cfg(feature = "spidermonkey")]
        {
            let Some(ctx) = &self.js else {
                self.relayout(fonts, viewport_width);
                return;
            };
            let rects: HashMap<manuk_dom::NodeId, [f32; 4]> = self
                .root_box
                .node_rects(&self.dom)
                .into_iter()
                .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
                .collect();
            if let Err(e) =
                manuk_js::dispatch_event(ctx, &mut self.dom, node, "input", &rects, &self.styles)
            {
                tracing::warn!("input dispatch: {e}");
            }
        }
        self.relayout(fonts, viewport_width);
    }

    /// Dispatch a `keydown`/`keyup` keyboard event carrying `key` (the `KeyboardEvent.key` value)
    /// to `node`, and return `true` iff the browser should perform its **default** action for that
    /// key (i.e. no handler called `preventDefault`). This is how a page intercepts a key — a chat
    /// composer whose `onKeyDown` calls `preventDefault()` on Enter so it sends the message instead
    /// of submitting the form; a combobox swallowing ArrowDown. `key_code` (the legacy `keyCode`) is
    /// derived from `key`. No JS context → always `true` (perform the default).
    pub fn dispatch_key(
        &mut self,
        node: manuk_dom::NodeId,
        ty: &str,
        key: &str,
        fonts: &FontContext,
        viewport_width: f32,
    ) -> bool {
        #[cfg(feature = "spidermonkey")]
        {
            let Some(ctx) = &self.js else {
                return true;
            };
            let rects: HashMap<manuk_dom::NodeId, [f32; 4]> = self
                .root_box
                .node_rects(&self.dom)
                .into_iter()
                .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
                .collect();
            let proceed = match manuk_js::dispatch_key(
                ctx,
                &mut self.dom,
                node,
                ty,
                key,
                key_code_for(key),
                &rects,
                &self.styles,
            ) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!("{ty} dispatch: {e}");
                    return true;
                }
            };
            let root = self.dom.root();
            if self.dom.is_dirty(root) || self.dom.has_dirty_descendants(root) {
                self.relayout(fonts, viewport_width);
            }
            proceed
        }
        #[cfg(not(feature = "spidermonkey"))]
        {
            let _ = (node, ty, key, fonts, viewport_width);
            true
        }
    }

    /// Fire the **commit** events when a field loses focus: `change` (only if `value_changed` — the
    /// field's value differs from when it gained focus, which is exactly when the spec fires it) then
    /// `blur`. `change` is what a form runs its validation on ("email invalid" the moment you leave
    /// the field), and it is deliberately NOT fired per keystroke (that is `input`, tick 175) — a
    /// change-validator must run once, on commit, not on every character. `blur` order is after
    /// `change`, per the HTML event model.
    pub fn dispatch_blur(
        &mut self,
        node: manuk_dom::NodeId,
        value_changed: bool,
        fonts: &FontContext,
        viewport_width: f32,
    ) {
        #[cfg(feature = "spidermonkey")]
        {
            let Some(ctx) = &self.js else {
                return;
            };
            let rects: HashMap<manuk_dom::NodeId, [f32; 4]> = self
                .root_box
                .node_rects(&self.dom)
                .into_iter()
                .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
                .collect();
            let events: &[&str] = if value_changed {
                &["change", "blur"]
            } else {
                &["blur"]
            };
            for ty in events {
                if let Err(e) =
                    manuk_js::dispatch_event(ctx, &mut self.dom, node, ty, &rects, &self.styles)
                {
                    tracing::warn!("{ty} dispatch: {e}");
                }
            }
            let root = self.dom.root();
            if self.dom.is_dirty(root) || self.dom.has_dirty_descendants(root) {
                self.relayout(fonts, viewport_width);
            }
        }
        #[cfg(not(feature = "spidermonkey"))]
        {
            let _ = (node, value_changed, fonts, viewport_width);
        }
    }

    /// Tell the page its **view changed** — it scrolled, or it was laid out again — so the
    /// observers run and `scroll` fires. Returns any scroll the callbacks then requested.
    ///
    /// Nothing else tells a page that a box came into view. Without this call, a feed built on
    /// `IntersectionObserver` loads its first screenful and stops forever.
    #[cfg(feature = "spidermonkey")]
    pub fn view_changed(&mut self, scroll_y: f32, vw: f32, vh: f32, scrolled: bool) {
        let Some(ctx) = &self.js else { return };
        // Most pages have no scroll listener and no observer. For those, every part of what follows
        // — rebuilding the rect map, re-entering JS, pumping timers — is work done to inform a page
        // that is not listening. Sixty times a second, on the UI thread. Ask first.
        if !manuk_js::wants_view_events(ctx) {
            return;
        }
        let rects: HashMap<manuk_dom::NodeId, [f32; 4]> = self
            .root_box
            .node_rects(&self.dom)
            .into_iter()
            .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
            .collect();
        if let Err(e) = manuk_js::view_changed(
            ctx,
            &mut self.dom,
            scroll_y,
            vw,
            vh,
            scrolled,
            &rects,
            &self.styles,
        ) {
            tracing::warn!("view_changed: {e}");
        }
    }

    /// Without SpiderMonkey there is nothing to notify.
    #[cfg(not(feature = "spidermonkey"))]
    pub fn view_changed(&mut self, _scroll_y: f32, _vw: f32, _vh: f32, _scrolled: bool) {}

    /// Evaluate a script in the page's context. Used by the conformance suite to read state back
    /// out of the JS world through the DOM, which is the only channel a test has into a page.
    #[cfg(feature = "spidermonkey")]
    pub fn eval_for_test(&mut self, src: &str) {
        let Some(ctx) = &self.js else { return };
        let rects: HashMap<manuk_dom::NodeId, [f32; 4]> = self
            .root_box
            .node_rects(&self.dom)
            .into_iter()
            .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
            .collect();
        manuk_js::set_scroll_geometry(scroll_geometry_of(
            &self.root_box,
            &self.styles,
            &self.scroll_offsets,
        ));
        let _ = manuk_js::eval_in_page(ctx, &mut self.dom, src, &rects, &self.styles);
        self.drain_canvases();
        self.drain_element_scrolls();
    }

    /// **Fire the document lifecycle: `DOMContentLoaded`, then `load`.**
    ///
    /// Neither event was EVER dispatched by this engine — grep found zero occurrences. A site whose
    /// init lives in `window.addEventListener('load', …)` — a very large fraction of the web —
    /// simply **never initialised**, silently, with nothing in any log to say so.
    ///
    /// The host must fire these because **only the host knows when they are true**: *"the document
    /// finished parsing"* and *"the subresources finished"* are facts about the loader, not about JS.
    #[cfg(feature = "spidermonkey")]
    pub fn fire_lifecycle(&mut self, which: &str) {
        // The thread-local is shared by every `Page` on this thread, so re-publish rather than trust it.
        self.publish_iframe_docs();
        let src = match which {
            "DOMContentLoaded" => "globalThis.__fireDOMContentLoaded && __fireDOMContentLoaded()",
            _ => "globalThis.__fireLoad && __fireLoad()",
        };
        self.eval_for_test(src);
    }

    /// Without SpiderMonkey there is no JS to notify, so the lifecycle is a no-op — not a lie.
    #[cfg(not(feature = "spidermonkey"))]
    pub fn fire_lifecycle(&mut self, _which: &str) {}

    /// Publish the viewport's scroll offset and the focused element into the JS world.
    ///
    /// A page reads `window.scrollY` to decide what to render, which header to stick, and when to
    /// load the next screenful. It must see the CURRENT offset, not the one at load — so this is
    /// called before every re-entry into script.
    pub fn publish_view_state(
        &self,
        scroll_x: f32,
        scroll_y: f32,
        active: Option<manuk_dom::NodeId>,
    ) {
        manuk_js::set_view_state(scroll_x, scroll_y, active);
    }

    /// Scroll requests the page's script made (`scrollTo`, `scrollBy`, `scrollIntoView`). The host
    /// owns the viewport, so a script asks and the shell performs — the same shape as `window.open`.
    pub fn take_scroll_requests(&self) -> Vec<(f32, f32)> {
        manuk_js::take_scrolls()
    }

    /// Focus requests the page's script made (`el.focus()`, `el.blur()`).
    pub fn take_focus_requests(&self) -> Vec<Option<manuk_dom::NodeId>> {
        manuk_js::take_focus_requests()
    }

    /// The computed styles, keyed by node — the input to layout, and the thing to look at when a
    /// box is the wrong size (a filled box vs a hugged one is a `display` question, not a layout
    /// bug). Read-only; used by the render/box-dump harness.
    /// The decoded bitmaps, keyed by node — for probes that need to ask "how big is the image the
    /// page actually got?", which is a different question from "how big is the box".
    pub fn decoded_images(
        &self,
    ) -> &std::collections::HashMap<manuk_dom::NodeId, std::rc::Rc<manuk_paint::DecodedImage>> {
        &self.images
    }

    pub fn styles_map(&self) -> &StyleMap {
        &self.styles
    }

    /// Bind fetched **mask bitmaps** (keyed by raw CSS url) to the nodes whose computed
    /// `mask-image` names them. They land in the same per-node bitmap map the painter already
    /// consults; a masked element is empty by construction, so it is never also an `<img>`.
    ///
    /// No relayout: a mask changes only what is painted inside the box, never its size.
    pub fn apply_masks(
        &mut self,
        masks: &HashMap<String, std::rc::Rc<manuk_paint::DecodedImage>>,
    ) -> usize {
        if masks.is_empty() {
            return 0;
        }
        let bound: Vec<(manuk_dom::NodeId, std::rc::Rc<manuk_paint::DecodedImage>)> = self
            .styles
            .iter()
            .filter_map(|(&n, s)| Some((n, masks.get(s.mask_image.as_ref()?)?.clone())))
            .collect();
        let count = bound.len();
        for (n, img) in bound {
            self.images.insert(n, img);
        }
        count
    }

    /// Fetch this page's mask images from the cascade (the paths that *have* styles: `load_async`,
    /// the shell's first paint, the render/fidelity harness) and bind them.
    pub async fn fetch_and_apply_masks(&mut self) -> usize {
        let mut seen = std::collections::HashSet::new();
        let targets: Vec<(String, String)> = self
            .styles
            .values()
            .filter_map(|s| s.mask_image.clone())
            .filter(|raw| seen.insert(raw.clone()))
            .filter(|raw| !self.fetched_urls.contains(raw))
            .map(|raw| {
                let abs = if raw.starts_with("data:") {
                    raw.clone()
                } else {
                    resolve_url(&self.final_url, &raw)
                };
                (raw, abs)
            })
            .collect();
        if targets.is_empty() {
            return 0;
        }
        for (raw, _) in &targets {
            self.fetched_urls.insert(raw.clone());
        }
        let owned = fetch_masks_owned(targets).await;
        let rc: HashMap<String, std::rc::Rc<manuk_paint::DecodedImage>> = owned
            .into_iter()
            .map(|(u, i)| (u, std::rc::Rc::new(i)))
            .collect();
        self.apply_masks(&rc)
    }

    /// Fetch this page's **`background-image: url(...)`** bitmaps and bind them to their nodes.
    ///
    /// The same per-node bitmap map the painter already consults for `<img>` — an element with a
    /// `url()` background is never also a replaced image, so they cannot collide. Gradients need no
    /// fetch at all; they are painted from the computed value directly.
    pub async fn fetch_and_apply_background_images(&mut self) -> usize {
        let have: std::collections::HashSet<manuk_dom::NodeId> =
            self.images.keys().copied().collect();
        let targets: Vec<(manuk_dom::NodeId, String)> = self
            .styles
            .iter()
            .filter(|(n, _)| !have.contains(n))
            .filter_map(|(&n, s)| {
                // The per-node bitmap map holds ONE image per node, so at most one url() layer per
                // element is fetchable — take the first. (Multiple gradient layers over one photo,
                // the common case, need no fetch.)
                let u = s.background_images.iter().find_map(|i| match i {
                    manuk_css::BackgroundImage::Url(u) => Some(u),
                    _ => None,
                })?;
                let abs = if u.starts_with("data:") {
                    u.clone()
                } else {
                    resolve_url(&self.final_url, u)
                };
                Some((n, abs))
            })
            .collect();
        if targets.is_empty() {
            return 0;
        }
        let fetched = futures_util::future::join_all(
            targets
                .into_iter()
                .map(|(n, url)| async move { (n, fetch_image_bytes(&url).await, url) }),
        )
        .await;
        let mut count = 0usize;
        for (node, bytes, url) in fetched {
            let Some(bytes) = bytes else { continue };
            if let Some(img) = decode_bitmap(&bytes, &url) {
                self.images.insert(node, std::rc::Rc::new(img));
                count += 1;
            }
        }
        count
    }

    /// **DEBT-1.** Build a page from [`Prefetched`] data — parse/cascade/layout/JS only, with
    /// **zero network calls**, so the UI thread never blocks. This is the path the shell uses for
    /// every navigation and reload.
    pub fn from_prefetched(pre: Prefetched, fonts: &FontContext, viewport_width: f32) -> Page {
        // Both passes, back to back — see `load`. Only the shell separates them.
        let mut page = Page::from_prefetched_inner(pre, fonts, viewport_width);
        page.run_deferred_scripts(fonts, viewport_width);
        // Parsing is done and the deferred scripts have executed — that IS DOMContentLoaded.
        page.fire_lifecycle("DOMContentLoaded");
        // On the SYNC path there is no async runtime to fetch subframes with; a caller that needs a
        // frame's document (a gate) drives `render_iframe` directly. The async path
        // (`load_async`/`finish_loading`) is where real frames load.
        page.fire_lifecycle("load");
        page
    }

    fn from_prefetched_inner(pre: Prefetched, fonts: &FontContext, viewport_width: f32) -> Page {
        let Prefetched {
            dom,
            final_url,
            css,
            images,
            masks,
        } = pre;
        let mut page = Page::from_dom(dom, &final_url, fonts, viewport_width);
        if !css.is_empty() {
            page.apply_stylesheets(&css, fonts, viewport_width);
        }
        if !masks.is_empty() {
            // After the cascade: only now does a node have a computed `mask-image` to bind to.
            let rc: HashMap<String, std::rc::Rc<manuk_paint::DecodedImage>> = masks
                .into_iter()
                .map(|(u, i)| (u, std::rc::Rc::new(i)))
                .collect();
            page.apply_masks(&rc);
        }
        if !images.is_empty() {
            // Bind the URL-keyed bitmaps to the nodes that name them. One decoded image, shared by
            // every element pointing at it — the flat tree, so a shadow-root `<img>` is not skipped.
            let by_url: HashMap<String, std::rc::Rc<manuk_paint::DecodedImage>> = images
                .into_iter()
                .map(|(u, i)| (u, std::rc::Rc::new(i)))
                .collect();
            let mut rc: HashMap<manuk_dom::NodeId, std::rc::Rc<manuk_paint::DecodedImage>> =
                HashMap::new();
            for n in page.dom.flat_descendants(page.dom.root()) {
                let Some(el) = page.dom.element(n) else {
                    continue;
                };
                let src = match page.dom.tag_name(n) {
                    Some("img") => el.attr("src"),
                    Some("video") => el.attr("poster"),
                    _ => None,
                };
                if let Some(src) = src {
                    let url = resolve_url(&final_url, src.trim());
                    if let Some(img) = by_url.get(&url) {
                        rc.insert(n, img.clone());
                        page.image_by_url.insert(url.clone(), Some(img.clone()));
                        page.image_attempts.insert((n, url));
                    }
                }
            }
            if !rc.is_empty() {
                page.apply_images(rc, fonts, viewport_width);
            }
        }
        page
    }

    /// **DEBT-1 / tick 31.** As [`from_prefetched`](Self::from_prefetched), but stops at first paint:
    /// only the scripts that BLOCK paint have run. The caller paints, then calls
    /// [`run_deferred_scripts`](Self::run_deferred_scripts).
    ///
    /// This is the shell's path, and it is the only one that separates the two. `from_prefetched` runs
    /// both back-to-back so that nothing else in the codebase has to know this distinction exists.
    pub fn from_prefetched_blocking_only(
        pre: Prefetched,
        fonts: &FontContext,
        viewport_width: f32,
    ) -> Page {
        Page::from_prefetched_inner(pre, fonts, viewport_width)
    }

    /// Build a page from an already-parsed [`Dom`] (shared by [`load`](Self::load) and
    /// [`load_streaming`](Self::load_streaming)).
    pub fn from_dom(dom: Dom, final_url: &str, fonts: &FontContext, viewport_width: f32) -> Page {
        // Box the DOM up front so its address is stable for the persistent JS context's raw
        // reflector pointers, then style + lay out once and run the document's inline scripts
        // against that layout snapshot (so `getBoundingClientRect` works), letting them mutate
        // the DOM and register event listeners. If they mutated it, re-style + re-lay-out so
        // script-built content renders. All a no-op unless the `spidermonkey` feature is on.
        let mut dom = Box::new(dom);
        let sheets: Vec<Stylesheet> = MinimalCascade::collect_style_elements(&dom);
        let mut styles = cascade_styles(&dom, &sheets, viewport_width);
        let mut root_box = layout_document(&dom, &styles, fonts, viewport_width);

        let rects: std::collections::HashMap<manuk_dom::NodeId, [f32; 4]> = root_box
            .node_rects(&dom)
            .into_iter()
            .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
            .collect();
        // Only stand up a JS context for documents that actually have a script — a static page
        // needs no engine spin-up (faster load, and no persistent global to keep alive). With
        // no initial script, no listener can ever be registered, so there is nothing to lose.
        let js = if dom.find_first("script").is_none() {
            None
        } else {
            manuk_js::set_scroll_geometry(scroll_geometry_of(
                &root_box,
                &styles,
                &std::collections::HashMap::new(),
            ));
            match manuk_js::load_document(&mut dom, final_url, &rects, &styles) {
                Ok((ctx, n)) => {
                    if n > 0 {
                        tracing::debug!(scripts = n, "executed page scripts");
                        let sheets2: Vec<Stylesheet> = MinimalCascade::collect_style_elements(&dom);
                        styles = cascade_styles(&dom, &sheets2, viewport_width);
                        root_box = layout_document(&dom, &styles, fonts, viewport_width);
                    }
                    Some(ctx)
                }
                Err(e) => {
                    tracing::warn!("page scripts: {e}");
                    None
                }
            }
        };

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

        let has_sticky = styles
            .values()
            .any(|s| s.position == manuk_css::Position::Sticky);
        Page {
            final_url: final_url.to_string(),
            title,
            content_height,
            root_box,
            dom,
            styles,
            js,
            has_sticky,
            zoom: 1.0,
            images: std::collections::HashMap::new(),
            scroll_offsets: std::collections::HashMap::new(),
            external_css: HashMap::new(),
            fetched_urls: std::collections::HashSet::new(),
            image_attempts: std::collections::HashSet::new(),
            image_by_url: std::collections::HashMap::new(),
            iframes: std::collections::HashMap::new(),
            child_pages: std::collections::HashMap::new(),
            last_cascade: None,
        }
    }

    /// Fire a trusted `click` at `node` and its ancestors (delegation), running the page's JS
    /// listeners. If the DOM changed, re-cascade + re-lay-out so the mutation renders. Returns
    /// `true` if the engine should still perform the element's **default action** (follow a
    /// link, submit a form) — i.e. no listener called `preventDefault()`. Without JS (no
    /// context / feature off) this is a no-op that returns `true`.
    pub fn dispatch_click(
        &mut self,
        node: manuk_dom::NodeId,
        fonts: &FontContext,
        viewport_width: f32,
    ) -> bool {
        let Some(ctx) = &self.js else { return true };
        let rects: std::collections::HashMap<manuk_dom::NodeId, [f32; 4]> = self
            .root_box
            .node_rects(&self.dom)
            .into_iter()
            .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
            .collect();
        let proceed =
            match manuk_js::dispatch_event(ctx, &mut self.dom, node, "click", &rects, &self.styles)
            {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!("click dispatch: {e}");
                    return true;
                }
            };
        // If a handler mutated the DOM, re-style + re-lay-out so it renders (at base zoom;
        // the caller re-applies zoom on its next relayout).
        let root = self.dom.root();
        if self.dom.is_dirty(root) || self.dom.has_dirty_descendants(root) {
            let sheets: Vec<Stylesheet> = MinimalCascade::collect_style_elements(&self.dom);
            self.styles = cascade_styles(&self.dom, &sheets, viewport_width);
            self.root_box = layout_document(&self.dom, &self.styles, fonts, viewport_width);
            self.reapply_scroll_offsets();
            self.content_height = self.root_box.content_bottom();
            self.dom.clear_all_dirty();
        }
        proceed
    }

    /// Settle a page `fetch`/`XHR` request (issued during script run or a click handler) with an
    /// HTTP `status`, response `body`, and the server's response `headers` (`status == 0` = network
    /// failure). Runs the page's `.then`/`onload` reactions; if they mutated the DOM, re-style +
    /// re-lay-out so the update renders. The `headers` reach the page as `Response.headers.get(…)`
    /// and `XMLHttpRequest.getResponseHeader(…)` — an empty slice keeps both returning `null`, as
    /// they did before headers were plumbed. No-op when the page has no JS context.
    pub fn resolve_fetch(
        &mut self,
        id: u32,
        status: u16,
        body: &str,
        headers: &[(String, String)],
        fonts: &FontContext,
        viewport_width: f32,
    ) {
        let Some(ctx) = &self.js else { return };
        let rects: std::collections::HashMap<manuk_dom::NodeId, [f32; 4]> = self
            .root_box
            .node_rects(&self.dom)
            .into_iter()
            .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
            .collect();
        if let Err(e) = manuk_js::resolve_fetch(
            ctx,
            &mut self.dom,
            id,
            status,
            body,
            headers,
            &rects,
            &self.styles,
        ) {
            tracing::warn!("fetch resolve: {e}");
            return;
        }
        let root = self.dom.root();
        if self.dom.is_dirty(root) || self.dom.has_dirty_descendants(root) {
            let sheets: Vec<Stylesheet> = MinimalCascade::collect_style_elements(&self.dom);
            self.styles = cascade_styles(&self.dom, &sheets, viewport_width);
            self.root_box = layout_document(&self.dom, &self.styles, fonts, viewport_width);
            self.reapply_scroll_offsets();
            self.content_height = self.root_box.content_bottom();
            self.dom.clear_all_dirty();
        }
    }

    /// What the page's WebSockets asked for since the last call, each `(socket_id, op)`. The host
    /// owns the socket — the page queues a connect/send/close and the host performs it, the same
    /// shape `fetch` uses.
    pub fn take_ws_ops(&self) -> Vec<(u32, manuk_js::WsOp)> {
        match &self.js {
            Some(ctx) => manuk_js::take_ws_ops(ctx),
            None => Vec::new(),
        }
    }

    /// Deliver one WebSocket event to socket `id` and re-render if the page's handler changed the
    /// document — a chat message that arrives has to appear, which is the entire point.
    pub fn deliver_ws_event(
        &mut self,
        id: u32,
        event: &manuk_js::WsEvent,
        fonts: &FontContext,
        viewport_width: f32,
    ) {
        let Some(ctx) = &self.js else { return };
        let rects: std::collections::HashMap<manuk_dom::NodeId, [f32; 4]> = self
            .root_box
            .node_rects(&self.dom)
            .into_iter()
            .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
            .collect();
        if let Err(e) =
            manuk_js::deliver_ws_event(ctx, &mut self.dom, id, event, &rects, &self.styles)
        {
            tracing::warn!("websocket deliver: {e}");
            return;
        }
        let root = self.dom.root();
        if self.dom.is_dirty(root) || self.dom.has_dirty_descendants(root) {
            let sheets: Vec<Stylesheet> = MinimalCascade::collect_style_elements(&self.dom);
            self.styles = cascade_styles(&self.dom, &sheets, viewport_width);
            self.root_box = layout_document(&self.dom, &self.styles, fonts, viewport_width);
            self.reapply_scroll_offsets();
            self.content_height = self.root_box.content_bottom();
            self.dom.clear_all_dirty();
        }
    }

    /// Deliver one step of a **streaming** response for request `id`: `Head` (where the page's
    /// `fetch()` promise resolves, body still arriving), then a `Chunk` per piece, then `End`.
    ///
    /// The relayout at the end is the point of the whole path. [`resolve_fetch`](Self::resolve_fetch)
    /// hands the page its body in one lump, so a streamed answer can only appear once the server has
    /// finished. Re-cascading and re-laying-out after EACH chunk is what makes the answer type itself
    /// out — and it is guarded on the dirty bit, so a chunk the page's handler ignores costs nothing.
    pub fn deliver_fetch_stream(
        &mut self,
        id: u32,
        event: &manuk_js::FetchStreamEvent,
        fonts: &FontContext,
        viewport_width: f32,
    ) {
        let Some(ctx) = &self.js else { return };
        let rects: std::collections::HashMap<manuk_dom::NodeId, [f32; 4]> = self
            .root_box
            .node_rects(&self.dom)
            .into_iter()
            .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
            .collect();
        if let Err(e) =
            manuk_js::deliver_fetch_stream(ctx, &mut self.dom, id, event, &rects, &self.styles)
        {
            tracing::warn!("fetch stream deliver: {e}");
            return;
        }
        let root = self.dom.root();
        if self.dom.is_dirty(root) || self.dom.has_dirty_descendants(root) {
            let sheets: Vec<Stylesheet> = MinimalCascade::collect_style_elements(&self.dom);
            self.styles = cascade_styles(&self.dom, &sheets, viewport_width);
            self.root_box = layout_document(&self.dom, &self.styles, fonts, viewport_width);
            self.reapply_scroll_offsets();
            self.content_height = self.root_box.content_bottom();
            self.dom.clear_all_dirty();
        }
    }

    /// Drain the page's queued `fetch`/XHR requests as `(id, url, method, headers, body)`, for the
    /// host to perform over the network and settle via [`resolve_fetch`](Self::resolve_fetch). Empty
    /// when the page has no JS context.
    pub fn take_fetches(&self) -> Vec<(u32, String, String, Vec<(String, String)>, String)> {
        match &self.js {
            Some(ctx) => manuk_js::take_fetches(ctx),
            None => Vec::new(),
        }
    }

    /// Forms a script asked to submit: `(direct, requested)`. See `manuk_js::take_form_submits`.
    pub fn take_form_submits(&self) -> (Vec<manuk_dom::NodeId>, Vec<manuk_dom::NodeId>) {
        match &self.js {
            Some(ctx) => {
                let (d, r) = manuk_js::take_form_submits(ctx);
                (
                    d.into_iter().map(|x| manuk_dom::NodeId(x as u64)).collect(),
                    r.into_iter().map(|x| manuk_dom::NodeId(x as u64)).collect(),
                )
            }
            None => (Vec::new(), Vec::new()),
        }
    }

    /// The base URL for resolving relative request URLs (page `fetch`/XHR targets).
    pub fn base_url(&self) -> &str {
        &self.final_url
    }

    /// Seed this page's window identity (own id + opener id) so `postMessage` `source` and
    /// `window.opener` resolve. No-op without a JS context.
    pub fn set_identity(&self, win_id: u64, opener_win: u64) {
        if let Some(ctx) = &self.js {
            if let Err(e) = manuk_js::set_identity(ctx, win_id, opener_win) {
                tracing::warn!("set_identity: {e}");
            }
        }
    }

    /// Drain this page's queued cross-window `postMessage` sends as `(target_win, json, origin,
    /// source_win)` for the host to route. Empty without a JS context.
    pub fn take_messages(&self) -> Vec<(u64, String, String, u64)> {
        match &self.js {
            Some(ctx) => manuk_js::take_messages(ctx),
            None => Vec::new(),
        }
    }

    /// Deliver a cross-window message into this page: fire a `message` MessageEvent and run the
    /// handler, re-laying-out if it mutated the DOM. No-op without a JS context.
    pub fn deliver_message(
        &mut self,
        json: &str,
        origin: &str,
        source_win: u64,
        fonts: &FontContext,
        viewport_width: f32,
    ) {
        let Some(ctx) = &self.js else { return };
        let rects: std::collections::HashMap<manuk_dom::NodeId, [f32; 4]> = self
            .root_box
            .node_rects(&self.dom)
            .into_iter()
            .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
            .collect();
        if let Err(e) = manuk_js::deliver_message(
            ctx,
            &mut self.dom,
            json,
            origin,
            source_win,
            &rects,
            &self.styles,
        ) {
            tracing::warn!("deliver_message: {e}");
            return;
        }
        let root = self.dom.root();
        if self.dom.is_dirty(root) || self.dom.has_dirty_descendants(root) {
            let sheets: Vec<Stylesheet> = MinimalCascade::collect_style_elements(&self.dom);
            self.styles = cascade_styles(&self.dom, &sheets, viewport_width);
            self.root_box = layout_document(&self.dom, &self.styles, fonts, viewport_width);
            self.reapply_scroll_offsets();
            self.content_height = self.root_box.content_bottom();
            self.dom.clear_all_dirty();
        }
    }

    /// Drain the page's queued `history` ops (`pushState`/`replaceState`/`back`/`forward`/`go`)
    /// as `(kind, state_json, url)` — the host reflects them in the omnibox + back/forward stack
    /// without a network navigation. Empty when the page has no JS context.
    pub fn take_history_ops(&self) -> Vec<(u8, String, String)> {
        match &self.js {
            Some(ctx) => manuk_js::take_history_ops(ctx),
            None => Vec::new(),
        }
    }

    /// Fire a `popstate` (a real back/forward to a same-document `pushState` entry): updates
    /// `history.state` + `location`, runs the page's `onpopstate` reactions, and re-lays-out if
    /// they mutated the DOM. No-op without a JS context.
    pub fn fire_popstate(
        &mut self,
        state_json: &str,
        url: &str,
        fonts: &FontContext,
        viewport_width: f32,
    ) {
        let Some(ctx) = &self.js else { return };
        let rects: std::collections::HashMap<manuk_dom::NodeId, [f32; 4]> = self
            .root_box
            .node_rects(&self.dom)
            .into_iter()
            .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
            .collect();
        if let Err(e) =
            manuk_js::fire_popstate(ctx, &mut self.dom, state_json, url, &rects, &self.styles)
        {
            tracing::warn!("popstate: {e}");
            return;
        }
        let root = self.dom.root();
        if self.dom.is_dirty(root) || self.dom.has_dirty_descendants(root) {
            let sheets: Vec<Stylesheet> = MinimalCascade::collect_style_elements(&self.dom);
            self.styles = cascade_styles(&self.dom, &sheets, viewport_width);
            self.root_box = layout_document(&self.dom, &self.styles, fonts, viewport_width);
            self.reapply_scroll_offsets();
            self.content_height = self.root_box.content_bottom();
            self.dom.clear_all_dirty();
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
                let styles = cascade_styles(&partial, &sheets, viewport_width);
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
        // **Never lay out a tree the cascade has not seen all of.**
        //
        // Layout indexes the style map. A node the cascade has never seen is a node layout cannot
        // lay out — and until now it did not *fail* to lay it out, it PANICKED, and because the
        // panic unwinds through SpiderMonkey's C++ frames it aborted the process outright.
        // apple.com core-dumped the browser this way: its scripts inject `<svg>` from a timer that
        // runs after the last cascade, and layout reached the new nodes first.
        //
        // `layout` now degrades on a miss rather than dying (see `style_of`), which is the stability
        // guarantee. This is the correctness half: if the tree has grown since the cascade, cascade
        // it. The check is a node count, which is O(n) and trivially cheap next to a layout — and it
        // is only ever *true* when something really did change, so the common path pays nothing.
        let nodes = self.dom.descendants(self.dom.root()).count();
        if nodes > self.styles.len() {
            tracing::debug!(
                nodes,
                styled = self.styles.len(),
                "tree grew since the last cascade — restyling before layout"
            );
            // With the EXTERNAL sheets, not just the inline ones. `collect_style_elements` sees only
            // `<style>` blocks; rebuilding from it would strip every `<link>`ed stylesheet from the
            // page, which is a far worse bug than the one being fixed.
            let sources = collect_style_sources(&self.dom, &self.final_url);
            let sheets: Vec<Stylesheet> = sources
                .iter()
                .filter_map(|src| match src {
                    StyleSource::Inline(css, m) => Some(Stylesheet::parse(&wrap_media(css, m))),
                    StyleSource::External(url, m) => self
                        .external_css
                        .get(url)
                        .map(|css| Stylesheet::parse(&wrap_media(css, m))),
                })
                .collect();
            self.styles = cascade_styles(&self.dom, &sheets, viewport_width);
            self.last_cascade = None; // the fingerprint no longer describes this tree
        }
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
        let new_styles = cascade_styles(&self.dom, &sheets, viewport_width);

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

        // Geometry-affecting change → full relayout. A repaint-only change (color /
        // background / border color) updates the fragment tree's paint attributes in place
        // and skips layout entirely — the incremental fast path.
        if damage >= RestyleDamage::Reflow {
            self.root_box = layout_document(&self.dom, &self.styles, fonts, viewport_width);
            self.reapply_scroll_offsets();
            self.content_height = self.root_box.content_bottom();
        } else if damage == RestyleDamage::Repaint {
            self.apply_paint_only();
        }
        self.dom.clear_all_dirty();
        damage
    }

    /// Update the existing fragment tree's paint attributes (backgrounds, border colours,
    /// text colours) from the current styles, without recomputing geometry. Used on a
    /// repaint-only restyle so a colour change does not force a full relayout.
    fn apply_paint_only(&mut self) {
        let styles = &self.styles;
        self.root_box.walk_mut(&mut |b| {
            if let Some(node) = b.node {
                if let Some(s) = styles.get(&node) {
                    b.background = s.background_color;
                    if let Some(border) = &mut b.border {
                        border.color = s.border_color;
                    }
                }
            }
            if let BoxContent::Inline(frags) = &mut b.content {
                for f in frags {
                    if let Some(fnode) = f.node {
                        if let Some(s) = styles.get(&fnode) {
                            f.style.color = s.color;
                        }
                    }
                }
            }
        });
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
        // Pass the effective z-index per node so hit-testing is occlusion-aware (a
        // high-`z` overlay wins a click over content beneath it).
        manuk_a11y::build_tree_with_geometry(&self.dom, &rects, &self.z_index_map())
    }

    /// The accessibility tree with `state.focused` filled in for `focused`.
    ///
    /// Focus is **host-owned** — the shell tracks it and publishes it into the JS world via
    /// [`publish_view_state`](Self::publish_view_state) — so it cannot be read back out of the DOM.
    /// A caller that knows the focused node passes it here; [`a11y_tree`](Self::a11y_tree) leaves
    /// `focused` false rather than guessing.
    pub fn a11y_tree_with_focus(&self, focused: Option<manuk_dom::NodeId>) -> manuk_a11y::A11yNode {
        let rects: std::collections::HashMap<manuk_dom::NodeId, manuk_a11y::Rect> = self
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
        manuk_a11y::build_tree_with_focus(&self.dom, &rects, &self.z_index_map(), focused)
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

        // **Do not re-cascade a document whose inputs have not changed.**
        //
        // A page load ran the FULL cascade four times: once initially, once after the inline scripts
        // mutated the tree (both justified), and then twice more with byte-identical inputs — 19
        // sheets over 18,634 nodes, then 19 sheets over 18,634 nodes again. `finish_loading` applies
        // the stylesheets, and the dynamic-script pass applies them again after each round of
        // scripts, whether or not those scripts touched anything.
        //
        // On Wikipedia that duplicate is ~44ms of cascade plus ~25ms of relayout, thrown away. It is
        // the single largest avoidable cost in a navigation, and it is invisible in every profile
        // that only looks at one stage at a time, because each individual cascade is perfectly fast.
        //
        // So fingerprint what the cascade actually depends on — the style sources and the shape of
        // the tree — and skip the work when neither moved. A script that DID mutate the DOM or inject
        // a sheet changes the fingerprint and gets its restyle, which is the whole point.
        let fp = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            for src in &sources {
                match src {
                    StyleSource::Inline(css, m) => (0u8, css, m).hash(&mut h),
                    // The URL is not enough: the same href can resolve to different bytes across a
                    // load (a 404 the first time, a hit the second). Hash the CONTENT we will
                    // actually cascade, which is the thing the result depends on.
                    StyleSource::External(url, m) => (1u8, url, m, external.get(url)).hash(&mut h),
                }
            }
            self.dom.descendants(self.dom.root()).count().hash(&mut h);
            viewport_width.to_bits().hash(&mut h);
            // A mutation that keeps the node count identical (an attribute or class change, which is
            // how every SPA toggles state) must still invalidate.
            for n in self.dom.descendants(self.dom.root()) {
                if self.dom.is_dirty(n) {
                    n.hash(&mut h);
                }
            }
            h.finish()
        };
        self.external_css = external.clone();
        if self.last_cascade == Some(fp) && !self.styles.is_empty() {
            tracing::debug!("cascade inputs unchanged — skipping a full restyle");
            return RestyleDamage::None;
        }
        self.last_cascade = Some(fp);

        let sheets: Vec<Stylesheet> = sources
            .iter()
            .filter_map(|s| match s {
                // A conditional sheet is wrapped in its own `@media`, so the cascade decides.
                StyleSource::Inline(css, m) => Some(Stylesheet::parse(&wrap_media(css, m))),
                StyleSource::External(url, m) => external
                    .get(url)
                    .map(|css| Stylesheet::parse(&wrap_media(css, m))),
            })
            .collect();
        let new_styles = cascade_styles(&self.dom, &sheets, viewport_width);
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
        self.has_sticky = self
            .styles
            .values()
            .any(|s| s.position == manuk_css::Position::Sticky);
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
        // Fetch all render-blocking external stylesheets concurrently (deduped, order-
        // independent) rather than serially — the biggest first-paint latency win here.
        let mut seen = std::collections::HashSet::new();
        let ext_urls: Vec<String> = sources
            .iter()
            .filter_map(|s| match s {
                StyleSource::External(url, _) => Some(url.clone()),
                _ => None,
            })
            .filter(|url| seen.insert(url.clone()))
            // **Part 22.3: no URL is fetched twice for one navigation.** This method is called once
            // by `finish_loading` and then AGAIN after every round of dynamic scripts — and each
            // call was re-fetching every stylesheet on the page from scratch. Across all phases,
            // apple.com issued 282 fetches for 58 distinct URLs (224 duplicates, 79%) and bbc.co.uk
            // issued 484 for 124 (360 duplicates, 74%). The HTTP cache made most of them cheap, which
            // is exactly why nobody noticed: "cheap" is not "free", and every one still costs a body
            // clone of a multi-megabyte script.
            .filter(|url| !self.external_css.contains_key(url))
            .collect();
        let fetched = futures_util::future::join_all(ext_urls.into_iter().map(|url| async move {
            let text = manuk_net::fetch(&url).await.ok().map(|r| r.decoded_text());
            (url, text)
        }))
        .await;
        // Start from what we already have — a re-entry must ADD sheets, never rebuild the set from
        // scratch (which would drop the ones it just decided not to re-fetch).
        let mut external: HashMap<String, String> = self.external_css.clone();
        for (url, text) in fetched {
            match text {
                Some(t) => {
                    tracing::info!(bytes = t.len(), %url, "stylesheet applied");
                    external.insert(url, t);
                }
                // A stylesheet that fails to arrive is not a cosmetic loss — it is the difference
                // between a site's desktop layout and its mobile one. Say so.
                None => {
                    tracing::warn!(%url, "STYLESHEET FAILED — the page will render unstyled or \
                                              in its fallback layout")
                }
            }
        }
        // Web fonts: fetch @font-face sources (from inline + external CSS) and register
        // them BEFORE the relayout, so the cascade's font-family resolves to them.
        for s in &sources {
            let css = match s {
                StyleSource::Inline(c, _) => c.clone(),
                StyleSource::External(url, _) => external.get(url).cloned().unwrap_or_default(),
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
        // Re-cascade when there is a reason to: external CSS arrived, or a script mutated the tree
        // (layout INDEXES the style map, so a node the cascade has never seen is a node layout
        // cannot lay out — that was a real crash). Otherwise relayout only.
        //
        // Doing it unconditionally costs a *full extra cascade* on every load, and on a 19,000-node
        // page the cascade is the single most expensive stage in the pipeline. Correctness did not
        // need it; only the mutated-tree case did.
        if count > 0 || self.dom.has_dirty() {
            // **Part 22.3: no duplicate tree renders.** `apply_stylesheets` now returns
            // `RestyleDamage::None` when the cascade's inputs have not moved (same sheets, same
            // tree). Laying out anyway threw that away: a full-document layout ran after EVERY round
            // of dynamic scripts whether or not the round changed anything. bbc.co.uk performed
            // **nine** full-document layouts for one navigation at 257ms each — over two seconds of
            // relaying-out a tree that had not changed.
            let damage = self.apply_stylesheets(&external, fonts, viewport_width);
            if damage == RestyleDamage::None && !self.dom.has_dirty() {
                return count;
            }
        } else {
            // **Nothing arrived and nothing is dirty — so there is nothing to do.**
            //
            // This branch used to relayout the whole document anyway. `fetch_and_apply_stylesheets`
            // runs once for `finish_loading` and then again after EVERY round of dynamic scripts, so
            // a page whose scripts touch nothing still paid a full-document layout per round. On
            // bbc.co.uk that is 257ms a go, for a tree that did not change, several times per
            // navigation. A relayout that cannot change the output is not conservatism, it is waste
            // with a safety story attached to it.
        }
        count
    }

    /// Effective z-index per node for stacking-ordered paint: a positioned element with an
    /// explicit `z-index` establishes a layer that applies to its whole subtree (an
    /// approximation of CSS stacking contexts). Non-positioned / `z-index:auto` inherit the
    /// nearest such ancestor's layer (0 at the root).
    ///
    /// The **top layer** (modal `<dialog>`) sits above every author z-index by construction —
    /// see [`TOP_LAYER_Z`].
    fn z_index_map(&self) -> HashMap<manuk_dom::NodeId, i32> {
        use manuk_css::Position;
        let mut map = HashMap::new();
        let mut stack = vec![(self.dom.root(), 0i32)];
        while let Some((node, parent_z)) = stack.pop() {
            let z = match self.styles.get(&node) {
                Some(s) if s.position != Position::Static => s.z_index.unwrap_or(parent_z),
                _ => parent_z,
            };
            // **The top layer.** A modal dialog paints above the whole document regardless of where it
            // sits in the tree or what z-index the page gave anything else — that is what makes it
            // modal. Without this, `showModal()` on a dialog declared early in the body renders BEHIND
            // the sticky header and the z-50 overlay it is supposed to cover. Modality is marked by
            // `showModal()` (see the dialog prelude in js/src/event_loop.rs); a non-modal `show()`
            // dialog is deliberately not here — it stays in flow, where the spec puts it.
            // An OPEN POPOVER joins the same top layer (spec: both are "top layer" elements). A menu
            // that renders under the sticky header it hangs off is the same bug as a modal that does.
            let z = if self.dom.element(node).is_some_and(|e| {
                (e.name == "dialog" && e.attr("data-manuk-modal").is_some())
                    || e.attr("data-manuk-popover-open").is_some()
            }) {
                TOP_LAYER_Z
            } else {
                z
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

    /// The page's paint display list (stacking-ordered, with images). A compositor caches
    /// this and calls [`manuk_paint::DisplayList::changed_since`] / `damage_since` to skip
    /// re-rasterizing / re-uploading an idle frame, or to repaint only the damaged region.
    pub fn display_list(&self) -> manuk_paint::DisplayList {
        let z = self.z_index_map();
        manuk_paint::DisplayList::build_layered(&self.root_box, &self.images, &z)
    }

    /// Rasterize the whole page to a canvas of the given pixel size (CPU tier).
    /// Every laid-out node's rect. Used by gates and by the automation surface — the same numbers the
    /// painter works from, so a test cannot pass against a different set of boxes than the user sees.
    /// The computed style of a node, if the cascade produced one. `None` means **no style at all** —
    /// itself a finding: a node with no computed style falls back to a default, so a `<div>` that should
    /// be `block` (or `flex`) renders as `inline`.
    pub fn styles_of(&self, n: manuk_dom::NodeId) -> Option<&manuk_css::ComputedStyle> {
        self.styles.get(&n)
    }

    pub fn node_rects(&self) -> std::collections::HashMap<manuk_dom::NodeId, manuk_layout::Rect> {
        self.root_box.node_rects(&self.dom)
    }

    /// **The canvas background — `<body>`'s background propagates to the whole viewport.**
    ///
    /// CSS says the root element's background paints the *canvas*, and if the root has none, `<body>`'s
    /// is propagated up to it. This is not a detail: it is why a dark-themed page is dark **all the way
    /// down**, and not just as far as its content happens to reach.
    ///
    /// We hard-coded white. So any page whose content is shorter than the viewport — and any page with
    /// `body { background: #111 }`, which is most of the dark web — painted its content on a correct
    /// dark box floating in a **white void**. It was found through an `<iframe>`, whose child document
    /// is exactly "a page shorter than its viewport", and it was never an iframe bug at all.
    fn canvas_background(&self) -> Rgba {
        let root = self.dom.root();
        for sel in ["html", "body"] {
            let Some(n) = self.dom.find_first(sel) else {
                continue;
            };
            let _ = root;
            if let Some(bg) = self.styles.get(&n).and_then(|st| st.background_color) {
                if bg.a > 0 {
                    return bg;
                }
            }
        }
        Rgba::WHITE
    }

    pub fn paint(&self, fonts: &FontContext, width: u32, height: u32) -> Canvas {
        let z = self.z_index_map();
        let clip = self.clip_map();
        CpuPainter::with_layers(fonts, &self.images, &z, &clip).render(
            &self.root_box,
            width,
            height,
            self.canvas_background(),
        )
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
        // position:sticky is a paint-time effect of the current scroll: pin sticky boxes to
        // their threshold within their containing block. Applied to a throwaway copy of the
        // box tree so the base layout is untouched; only sticky pages pay the clone.
        let sticky_boxes;
        let boxes: &LayoutBox = if self.has_sticky {
            let mut b = self.root_box.clone();
            apply_sticky(&mut b, &self.styles, scroll_y);
            sticky_boxes = b;
            &sticky_boxes
        } else {
            &self.root_box
        };
        CpuPainter::with_layers(fonts, &self.images, &z, &clip).render_scrolled(
            boxes,
            width,
            height,
            self.canvas_background(),
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

/// The legacy `KeyboardEvent.keyCode`/`which` for a `KeyboardEvent.key` value. Handlers still read
/// `keyCode` widely (the modern `key`/`code` never fully displaced it), so a synthesised keyboard
/// event that left it `0` would miss every `if (e.keyCode === 13)`. Covers the keys the shell
/// dispatches; anything else (a multi-byte key) is `0`, which is honest.
fn key_code_for(key: &str) -> u32 {
    match key {
        "Backspace" => 8,
        "Tab" => 9,
        "Enter" => 13,
        "Escape" => 27,
        " " => 32,
        "PageUp" => 33,
        "PageDown" => 34,
        "End" => 35,
        "Home" => 36,
        "ArrowLeft" => 37,
        "ArrowUp" => 38,
        "ArrowRight" => 39,
        "ArrowDown" => 40,
        "Delete" => 46,
        _ => {
            let mut ch = key.chars();
            match (ch.next(), ch.next()) {
                // A single character: its keyCode is the UPPERCASE code point for a letter, or the
                // digit/char code otherwise — the DOM's legacy convention.
                (Some(c), None) => c.to_ascii_uppercase() as u32,
                _ => 0,
            }
        }
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
            let styles = cascade_styles(&partial, &sheets, viewport_width);
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
    // **Fetch in PARALLEL; execute in ORDER.** These were fetched one at a time, in a `for` loop,
    // each awaiting a full round-trip before the next one started — and each under the *document*
    // deadline (30s), not the subresource one. bbc.co.uk has dozens of scripts, and that loop was
    // **9.3 seconds** of its load, all of it spent waiting.
    //
    // The classic-script model requires ordered EXECUTION, not ordered fetching. Every browser has
    // fetched these concurrently since the 2000s; we were the only ones queuing. Execution order is
    // preserved exactly, because the results are applied to the DOM in document order below.
    //
    // The whole phase runs under one deadline as well: `load_async` had no budget at all, so the
    // 12s budget on `finish_loading` was guarding the back half of a navigation whose front half
    // could run indefinitely. A page that cannot get its scripts in time renders without them —
    // which is what a browser does, and is strictly better than not rendering at all.
    let budget = load_budget();
    let fetched = match tokio::time::timeout(
        budget,
        futures_util::future::join_all(
            targets
                .iter()
                .map(|(n, url)| async move { (*n, manuk_net::fetch(url).await.ok()) }),
        ),
    )
    .await
    {
        Ok(f) => f,
        Err(_) => {
            tracing::warn!(
                "script fetch exceeded the {:.0}s load budget — rendering with what arrived",
                budget.as_secs_f32()
            );
            Vec::new()
        }
    };
    for (node, resp) in fetched {
        match resp {
            Some(r) => {
                let js = r.decoded_text();
                dom.remove_attr(node, "src");
                let text = dom.create_text(js);
                dom.append_child(node, text);
            }
            None => tracing::warn!("external script fetch failed"),
        }
    }
}

/// **DEBT-1 (RELIABILITY).** Everything a page needs from the network, fetched **off the UI
/// thread** and handed over as plain data.
///
/// Before this, the UI thread called `block_on` three times while building a page — external
/// scripts, external CSS, and images. That is a *hang*: the window stops responding for the whole
/// round-trip. The user saw it as "the refresh button lags". Now the network thread does all of it
/// and the UI thread builds the page with **zero** network calls.
///
/// Everything here is `Send` on purpose: `Dom` and `DecodedImage` are plain data (no `Rc`), so they
/// cross the thread boundary. `Page` itself cannot — it borrows `FontContext` — which is exactly
/// why the *fetching* has to be what moves, not the page construction.
pub struct Prefetched {
    /// Parsed, with any external `<script src>` already fetched and inlined.
    pub dom: Dom,
    pub final_url: String,
    /// External stylesheet URL → CSS text.
    pub css: HashMap<String, String>,
    /// resolved image URL → decoded bitmap. **Keyed by URL, not node** — for the same reason `masks`
    /// is: nine elements naming one sprite are one fetch and one decode, not nine. The binding to
    /// nodes happens on the UI thread in `from_prefetched`, where the DOM is available.
    pub images: HashMap<String, manuk_paint::DecodedImage>,
    /// raw `mask-image` url → decoded bitmap (icons). Keyed by URL, not node, because the
    /// off-thread prefetch has no cascade — the nodes are bound to it once styles exist.
    pub masks: HashMap<String, manuk_paint::DecodedImage>,
}

/// Fetch a document **and all of its subresources** off-thread (DEBT-1). The returned
/// [`Prefetched`] needs no further network access, so [`Page::from_prefetched`] can build the page
/// on the UI thread without ever blocking it.
pub async fn prefetch_document(url: &str) -> Result<Loaded> {
    match fetch_document(url).await? {
        Loaded::Download {
            filename,
            path,
            bytes,
        } => Ok(Loaded::Download {
            filename,
            path,
            bytes,
        }),
        Loaded::Document { html, final_url } => prepare_prefetched(html, final_url).await,
        other => Ok(other),
    }
}

/// A **top-level `POST` navigation** — the async half of a native `<form method=post>` submission.
/// POSTs `body` (with `content_type`) to `url` off the UI thread, follows the login flow's
/// POST→redirect→GET, and runs the *same* off-thread subresource prefetch (external scripts, CSS,
/// masks) as [`prefetch_document`], so the shell swaps in a fully-prepared page exactly as it does
/// for a GET navigation. A response that turns out to be a download is handled like any other.
///
/// `initiator` is the document URL of the submitting page, applying `SameSite`: a **cross-site**
/// form POST withholds the target's `Lax`/`Strict` cookies (the CSRF defence), while the ordinary
/// **same-site** login sends them so the user lands logged in.
pub async fn prefetch_document_post(
    url: &str,
    content_type: &str,
    body: Vec<u8>,
    initiator: Option<&str>,
) -> Result<Loaded> {
    let resp = manuk_net::post_document(url, content_type, body.into(), initiator)
        .await
        .with_context(|| format!("POST {url}"))?;
    if resp.status >= 400 {
        // A 4xx/5xx still has a body worth showing (the server's "invalid password" page), so it is
        // rendered rather than turned into an error — matching a real browser, which shows the page.
        tracing::info!(status = resp.status, %url, "form POST returned an error status — showing its body");
    }
    prepare_prefetched(resp.decoded_text(), resp.final_url.to_string()).await
}

/// Turn a fetched document (`html` at `final_url`) into a [`Loaded::Prefetched`] with its external
/// scripts inlined and its stylesheets + mask icons fetched — all off the UI thread. Shared by
/// [`prefetch_document`] (GET) and [`prefetch_document_post`] (form POST) so the two navigation
/// kinds build identical pages.
async fn prepare_prefetched(html: String, final_url: String) -> Result<Loaded> {
    #[allow(unused_mut)]
    let mut dom = manuk_html::parse(&html);
    // External <script src> — fetched and inlined here, off-thread. (Execution still
    // happens on the UI thread inside `from_dom`; only the *fetch* moves.)
    #[cfg(feature = "spidermonkey")]
    fetch_external_scripts(&mut dom, &final_url).await;

    // External stylesheets, concurrently.
    let mut seen = std::collections::HashSet::new();
    let ext: Vec<String> = collect_style_sources(&dom, &final_url)
        .iter()
        .filter_map(|s| match s {
            StyleSource::External(u, _) => Some(u.clone()),
            _ => None,
        })
        .filter(|u| seen.insert(u.clone()))
        .collect();
    let fetched = futures_util::future::join_all(ext.into_iter().map(|u| async move {
        let text = manuk_net::fetch(&u).await.ok().map(|r| r.decoded_text());
        (u, text)
    }))
    .await;
    let css: HashMap<String, String> = fetched
        .into_iter()
        .filter_map(|(u, t)| t.map(|t| (u, t)))
        .collect();

    // Images: fetch + decode off-thread as OWNED data. Never touch `Rc` here — an `Rc`
    // anywhere inside this async fn would make the whole future `!Send`, which is exactly
    // what pinned image fetching to the UI thread and blocked it.
    // **FIRST PAINT DOES NOT WAIT FOR IMAGES.**
    //
    // This used to fetch and decode every image on the page before the shell was handed
    // anything at all — so the window showed nothing until the last tracking pixel on a news
    // front page had either arrived or timed out. Measured on nytimes.com: the document was
    // parsed, cascaded and laid out — *everything needed to paint* — in **1.7s**, and the user
    // saw it at **14s**. Twelve of those seconds were images nobody was looking at yet.
    //
    // Chromium does not do this, and neither does any browser a person would use: the article
    // is on screen long before the last asset lands. So the images are left to the shell, which
    // fetches them on a background task and applies them with `apply_images_by_url` when they
    // arrive (`NavEvent::ImagesReady`), repainting once. The layout reflows then — which is
    // exactly what an `<img>` without intrinsic dimensions does in a real browser too.
    let images: HashMap<String, manuk_paint::DecodedImage> = HashMap::new();

    // Masks (icons). Scanned from the CSS text rather than the cascade: this thread has no
    // styles, and a URL-keyed cache is all the cascade needs later to bind them to nodes.
    // Each raw url resolves against ITS OWN stylesheet, which is what CSS specifies.
    let mask_targets: Vec<(String, String)> = {
        let mut seen = std::collections::HashSet::new();
        css.iter()
            .flat_map(|(sheet, text)| {
                scan_mask_urls(text).into_iter().map(move |raw| {
                    let abs = if raw.starts_with("data:") {
                        raw.clone()
                    } else {
                        resolve_url(sheet, &raw)
                    };
                    (raw, abs)
                })
            })
            .filter(|(raw, _)| seen.insert(raw.clone()))
            .collect()
    };
    let masks = fetch_masks_owned(mask_targets).await;

    Ok(Loaded::Prefetched(Box::new(Prefetched {
        dom,
        final_url,
        css,
        images,
        masks,
    })))
}

/// The outcome of navigating to a URL: a document to render, or a file to save (the server
/// marked the response `Content-Disposition: attachment` or served a non-renderable binary).
pub enum Loaded {
    Document {
        html: String,
        final_url: String,
    },
    /// A file that was **streamed straight to disk** (never buffered whole in RAM, never subject to
    /// the document deadline while its body transferred). `path` is where it landed; `bytes` its size.
    Download {
        filename: String,
        path: std::path::PathBuf,
        bytes: u64,
    },
    /// DEBT-1: a document whose subresources were already fetched off-thread. The UI thread can
    /// build this with **no network calls at all**.
    Prefetched(Box<Prefetched>),
}

/// Like [`fetch_html`] but distinguishes a **download** from a document: an HTTP response whose
/// headers say "attachment" (or a clearly binary content-type) becomes [`Loaded::Download`]
/// carrying the suggested filename + bytes, for the shell to write to disk instead of rendering.
/// `data:`/`file:` URLs are always documents.
pub async fn fetch_document(url: &str) -> Result<Loaded> {
    if url.starts_with("http://") || url.starts_with("https://") {
        // The document gets the long deadline; its subresources get the short one. A download,
        // however, is **streamed to disk** by the net layer — decision made from headers, before the
        // body, so a multi-GB file neither buffers in RAM nor dies at the document deadline. See
        // `manuk_net::fetch_document_or_download`.
        let dir = manuk_net::downloads::download_dir();
        match manuk_net::fetch_document_or_download(url, &dir)
            .await
            .with_context(|| format!("fetching {url}"))?
        {
            manuk_net::DocOrDownload::Download {
                path,
                filename,
                bytes,
                ..
            } => Ok(Loaded::Download {
                filename,
                path,
                bytes,
            }),
            manuk_net::DocOrDownload::Document(resp) => Ok(Loaded::Document {
                html: resp.decoded_text(),
                final_url: resp.final_url.to_string(),
            }),
        }
    } else {
        let (html, final_url) = fetch_html(url).await?;
        Ok(Loaded::Document { html, final_url })
    }
}

pub async fn fetch_html(url: &str) -> Result<(String, String)> {
    if url.starts_with("http://") || url.starts_with("https://") {
        // The document gets the long deadline; its subresources get the short one. See
        // `manuk_net::fetch_document`.
        let resp = manuk_net::fetch_document(url)
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

/// Recursively pin `position:sticky` boxes for the current `scroll_y`. Each sticky child is
/// shifted (with its subtree) so it stays at its `top` threshold from the viewport, bounded by
/// the bottom of its containing block (its parent box). Non-sticky boxes are untouched.
fn apply_sticky(b: &mut LayoutBox, styles: &StyleMap, scroll_y: f32) {
    use manuk_layout::BoxContent;
    let cb_bottom = b.rect.y + b.rect.height;
    let cb_width = b.rect.width;
    if let BoxContent::Block(children) = &mut b.content {
        for child in children.iter_mut() {
            if let Some(node) = child.node {
                if let Some(s) = styles.get(&node) {
                    if s.position == manuk_css::Position::Sticky && !s.inset.top.is_auto() {
                        let top = s.inset.top.resolve(cb_width, 0.0);
                        let shift = manuk_layout::sticky_shift(
                            child.rect.y,
                            child.rect.height,
                            top,
                            cb_bottom,
                            scroll_y,
                        );
                        child.shift_y(shift);
                    }
                }
            }
            apply_sticky(child, styles, scroll_y);
        }
    }
}

/// The keystone interactivity test: a click handler registered while the page's scripts run
/// must fire on a *later* dispatch and mutate the live DOM — proving the persistent JS context
/// survives load, real input reaches page listeners, and their DOM writes land in the arena.
/// Only meaningful with a real JS engine.
#[cfg(all(test, feature = "spidermonkey"))]
mod js_interactive_tests {
    use super::*;

    // One combined test: a leaked per-process SpiderMonkey runtime tears down messily at
    // process exit, and exercising it from *multiple* #[test] fns in one binary can crash on
    // exit (a known mozjs shutdown-ordering issue; real browsers fast-exit the same way). A
    // single test keeps one JS context per binary and covers both the fire-and-mutate path and
    // preventDefault.
    // `#[ignore]` by default: the deliberately-leaked per-process SpiderMonkey runtime crashes
    // on *process exit* (a known mozjs shutdown-ordering issue) when this JS test co-runs with
    // the rest of the crate's suite. It passes reliably in isolation — run it explicitly:
    //   cargo test -p manuk-page --features spidermonkey -- --ignored user_click_fires
    #[test]
    #[ignore = "leaked SpiderMonkey runtime crashes at process exit when co-run; run in isolation"]
    fn js_conformance_suite() {
        // G2 — JS CONFORMANCE GATE (ADR-010). The DOM/BOM surface a real modern site actually
        // uses. This runs EVERY tick (scripts/verify.sh), and **every JS tick must add a
        // scenario** — the suite only grows. It is one test on purpose: the leaked per-process
        // SpiderMonkey runtime tears down messily at process exit when several JS tests co-run.
        //
        // Covered so far:
        //   1  click listeners fire on a real dispatch, and mutate the DOM
        //   2  preventDefault suppresses the default action
        //   3  window.open queues for the host (OAuth popup path)
        //   4  boot window/screen metrics (innerWidth/screen/dpr/matchMedia/rAF)
        //   5  fetch() — real Promise, .then chain mutates the DOM with the body
        //   6  XMLHttpRequest — onload sees status + body
        //   7  history.pushState — updates location, queues the host op
        //   8  popstate — fires onpopstate with restored state
        //   9  postMessage — queues with target window id + targetOrigin
        //  10  message delivery — onmessage gets data/origin/source
        //  11  MutationObserver — batched records (attributes, subtree, childList)
        //  12  matchMedia — width features evaluate against the viewport
        //  13  Custom Elements + Shadow DOM — upgrade, attachShadow, lifecycle callbacks
        //  14  The framework primitives — each one named by the framework it actually broke (tick 26)
        //  15  HTMLMediaElement — an honest NO, not a TypeError (tick 28)
        js_conformance_body();
    }

    fn js_conformance_body() {
        let fonts = FontContext::new();

        // (1) A click handler registered at load fires on a later dispatch and mutates the DOM.
        let html = r#"<!doctype html><html><body>
            <button id="b">Go</button><p id="out">before</p>
            <script>
              document.getElementById('b').addEventListener('click', function () {
                document.getElementById('out').textContent = 'CLICKED';
              });
            </script></body></html>"#;
        let mut page = Page::load(html, "https://example.test/", &fonts, 800.0);
        let root = page.dom().root();
        let button = manuk_css::query_selector_all(page.dom(), root, "#b")[0];
        let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
        assert_eq!(page.dom().text_content(out), "before");
        let proceed = page.dispatch_click(button, &fonts, 800.0);
        assert!(
            proceed,
            "no listener called preventDefault, so the default action proceeds"
        );
        assert_eq!(
            page.dom().text_content(out),
            "CLICKED",
            "the load-time click listener fired on dispatch and mutated the arena DOM"
        );

        // (2) preventDefault() on a link's click handler suppresses the default navigation.
        let html2 = r#"<!doctype html><html><body>
            <a id="lnk" href="/next">go</a>
            <script>
              document.getElementById('lnk').addEventListener('click', function (e) {
                e.preventDefault();
              });
            </script></body></html>"#;
        let mut page2 = Page::load(html2, "https://example.test/", &fonts, 800.0);
        let root2 = page2.dom().root();
        let link = manuk_css::query_selector_all(page2.dom(), root2, "#lnk")[0];
        assert!(
            !page2.dispatch_click(link, &fonts, 800.0),
            "preventDefault means the engine must NOT follow the link"
        );

        // (3) window.open queues the URL for the host to open as a new tab (OAuth-popup path).
        let _ = manuk_js::take_window_opens(); // clear any residue
        let html3 = r#"<!doctype html><html><body>
            <script>window.open('https://accounts.example/oauth?client=x');</script>
            </body></html>"#;
        let _page3 = Page::load(html3, "https://app.test/", &fonts, 800.0);
        let opens3 = manuk_js::take_window_opens();
        assert_eq!(
            opens3.len(),
            1,
            "window.open recorded one open for the host"
        );
        assert!(opens3[0].0 > 0, "the open carries an allocated window id");
        assert_eq!(
            opens3[0].1, "https://accounts.example/oauth?client=x",
            "window.open recorded the URL for the host"
        );

        // (4) Boot-time window/screen metrics exist (SPAs read these or throw at load).
        let html4 = r#"<!doctype html><html><body id="b"><script>
            document.getElementById('b').setAttribute('data-m',
                window.innerWidth + 'x' + screen.height + 'x' + devicePixelRatio +
                ':' + (typeof matchMedia) + ':' + (typeof requestAnimationFrame));
            </script></body></html>"#;
        let page4 = Page::load(html4, "https://app.test/", &fonts, 800.0);
        let b = manuk_css::query_selector_all(page4.dom(), page4.dom().root(), "#b")[0];
        assert_eq!(
            page4.dom().element(b).and_then(|e| e.attr("data-m")),
            Some("1280x720x1:function:function"),
            "window/screen/devicePixelRatio/matchMedia/rAF present at load"
        );

        // (5) fetch(): a load-time request is queued for the host, and resolving it runs the
        // page's `.then` chain, mutating the DOM with the response body (the SPA data path). The
        // response's HEADERS are readable — `r.headers.get('content-type')` returns the server's
        // real field (case-insensitively), not `null` as it did before headers were plumbed; a
        // header the server did not send is `null`. Every SPA that branches on `Content-Type`,
        // reads pagination from `Link`, or checks a rate-limit header depends on this.
        let html5 = r#"<!doctype html><html><body>
            <div id="out">loading</div>
            <script>
              fetch('/api/data')
                .then(function (r) {
                  document.getElementById('out').setAttribute('data-ct', String(r.headers.get('Content-Type')));
                  document.getElementById('out').setAttribute('data-miss', String(r.headers.get('x-absent')));
                  document.getElementById('out').setAttribute('data-has', String(r.headers.has('etag')));
                  return r.text();
                })
                .then(function (t) { document.getElementById('out').textContent = t; });
            </script></body></html>"#;
        let mut page5 = Page::load(html5, "https://app.test/page", &fonts, 800.0);
        let out5 = manuk_css::query_selector_all(page5.dom(), page5.dom().root(), "#out")[0];
        assert_eq!(
            page5.dom().text_content(out5),
            "loading",
            "pre-resolution placeholder"
        );
        let reqs = page5.take_fetches();
        assert_eq!(reqs.len(), 1, "the page issued exactly one fetch");
        let (id, url, method, _headers, _body) = &reqs[0];
        assert_eq!(url, "/api/data", "the requested URL reached the host queue");
        assert_eq!(method, "GET");
        let resp_headers = vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("ETag".to_string(), "\"abc\"".to_string()),
        ];
        page5.resolve_fetch(*id, 200, "HELLO-FROM-HOST", &resp_headers, &fonts, 800.0);
        assert_eq!(
            page5.dom().text_content(out5),
            "HELLO-FROM-HOST",
            "resolving the fetch ran the .then chain and mutated the DOM with the body"
        );
        assert_eq!(
            page5.dom().element(out5).and_then(|e| e.attr("data-ct")),
            Some("application/json"),
            "Response.headers.get('Content-Type') returns the server's header, matched case-insensitively"
        );
        assert_eq!(
            page5.dom().element(out5).and_then(|e| e.attr("data-miss")),
            Some("null"),
            "a header the server did not send reads back as null"
        );
        assert_eq!(
            page5.dom().element(out5).and_then(|e| e.attr("data-has")),
            Some("true"),
            "Response.headers.has() sees a header present under a different case"
        );

        // (6) XMLHttpRequest: onload fires with the resolved status + body, and
        // `getResponseHeader(…)` / `getAllResponseHeaders()` return the server's fields (case-
        // insensitively) instead of the old hard-coded null/"".
        let html6 = r#"<!doctype html><html><body>
            <div id="x">idle</div>
            <script>
              var r = new XMLHttpRequest();
              r.open('GET', '/xhr');
              r.onload = function () {
                var el = document.getElementById('x');
                el.setAttribute('data-ct', String(r.getResponseHeader('content-type')));
                el.setAttribute('data-all', r.getAllResponseHeaders());
                el.textContent = 'S' + r.status + ':' + r.responseText;
              };
              r.send();
            </script></body></html>"#;
        let mut page6 = Page::load(html6, "https://app.test/", &fonts, 800.0);
        let x6 = manuk_css::query_selector_all(page6.dom(), page6.dom().root(), "#x")[0];
        let reqs6 = page6.take_fetches();
        assert_eq!(reqs6.len(), 1, "XHR issued one request");
        let xhr_headers = vec![("Content-Type".to_string(), "text/plain".to_string())];
        page6.resolve_fetch(reqs6[0].0, 201, "BODY", &xhr_headers, &fonts, 800.0);
        assert_eq!(
            page6.dom().text_content(x6),
            "S201:BODY",
            "XHR onload saw the resolved status + body"
        );
        assert_eq!(
            page6.dom().element(x6).and_then(|e| e.attr("data-ct")),
            Some("text/plain"),
            "XHR getResponseHeader() returns the server's header, matched case-insensitively"
        );
        assert_eq!(
            page6.dom().element(x6).and_then(|e| e.attr("data-all")),
            Some("content-type: text/plain\r\n"),
            "getAllResponseHeaders() serializes each field as a lower-cased CRLF-terminated line"
        );

        // (7) history.pushState: a click handler routes client-side — location updates and the
        // op is queued for the host (no navigation).
        let html7 = r#"<!doctype html><html><body>
            <button id="nav">go</button><div id="r">home</div>
            <script>
              window.onpopstate = function (e) {
                document.getElementById('r').textContent = 'pop:' + (e.state ? e.state.p : 'none');
              };
              document.getElementById('nav').addEventListener('click', function () {
                history.pushState({ p: 42 }, '', '/next');
                document.getElementById('r').textContent = 'at:' + location.pathname;
              });
            </script></body></html>"#;
        let mut page7 = Page::load(html7, "https://app.test/home", &fonts, 800.0);
        let root7 = page7.dom().root();
        let nav = manuk_css::query_selector_all(page7.dom(), root7, "#nav")[0];
        let r7 = manuk_css::query_selector_all(page7.dom(), root7, "#r")[0];
        let _ = page7.take_history_ops(); // clear residue
        page7.dispatch_click(nav, &fonts, 800.0);
        assert_eq!(
            page7.dom().text_content(r7),
            "at:/next",
            "pushState updated location.pathname without a navigation"
        );
        let ops = page7.take_history_ops();
        assert_eq!(ops.len(), 1, "one history op queued for the host");
        assert_eq!(ops[0].0, 0, "kind 0 = pushState");
        assert_eq!(
            ops[0].2, "https://app.test/next",
            "resolved absolute URL for the omnibox"
        );
        assert!(ops[0].1.contains("42"), "state serialized: {}", ops[0].1);

        // (8) popstate: a host-driven back/forward fires onpopstate with the restored state.
        page7.fire_popstate("{\"p\":7}", "https://app.test/home", &fonts, 800.0);
        assert_eq!(
            page7.dom().text_content(r7),
            "pop:7",
            "onpopstate ran with the restored state on back/forward"
        );

        // (9) window.open(...).postMessage(...): the send is queued for the host with the popup's
        // window id + this window's id as source (the OAuth popup → opener channel).
        let html9 = r#"<!doctype html><html><body>
            <script>
              var w = window.open('https://auth.test/login');
              w.postMessage({ token: 'T' }, 'https://auth.test');
            </script></body></html>"#;
        let page9 = Page::load(html9, "https://app.test/", &fonts, 800.0);
        page9.set_identity(100, 0); // this window is #100 (no opener)
        let opens = page9.take_history_ops(); // (unrelated) — ensure no crosstalk
        assert!(opens.is_empty());
        let msgs = page9.take_messages();
        assert_eq!(msgs.len(), 1, "one postMessage queued");
        let (target, json, origin, source) = &msgs[0];
        assert!(*target > 0, "routed to the opened popup's window id");
        assert_eq!(origin, "https://auth.test", "targetOrigin preserved");
        assert!(
            json.contains("token") && json.contains('T'),
            "payload serialized: {json}"
        );
        // NB: source is read at post time; set_identity ran after load, so this asserts the
        // send path carries a source slot (0 here — the post happened during load, pre-identity).
        let _ = source;

        // (10) deliver_message: a message routed to a page fires its onmessage with data+origin.
        let html10 = r#"<!doctype html><html><body>
            <div id="m">waiting</div>
            <script>
              window.addEventListener('message', function (e) {
                document.getElementById('m').textContent =
                  'from:' + e.origin + ':' + (e.data ? e.data.token : '?') +
                  ':src=' + (e.source ? e.source.__winId : 'none');
              });
            </script></body></html>"#;
        let mut page10 = Page::load(html10, "https://app.test/", &fonts, 800.0);
        page10.set_identity(200, 0);
        let m10 = manuk_css::query_selector_all(page10.dom(), page10.dom().root(), "#m")[0];
        page10.deliver_message(
            "{\"token\":\"XYZ\"}",
            "https://auth.test",
            100,
            &fonts,
            800.0,
        );
        assert_eq!(
            page10.dom().text_content(m10),
            "from:https://auth.test:XYZ:src=100",
            "onmessage fired with data, origin, and a source window ref"
        );

        // (11) MutationObserver: a click handler mutates the DOM (attribute on the target, an
        // attribute on a descendant [subtree], and a child append) — the observer's callback
        // fires as a microtask with the batched, correctly-typed records.
        let html11 = r#"<!doctype html><html><body>
            <div id="c"><span id="s">x</span></div>
            <button id="btn">go</button>
            <div id="log">none</div>
            <script>
              var target = document.getElementById('c');
              var mo = new MutationObserver(function (records) {
                var out = [];
                for (var i = 0; i < records.length; i++) {
                  var r = records[i];
                  out.push(r.type + ':' + (r.attributeName || '-') + ':+' + r.addedNodes.length +
                           ':-' + r.removedNodes.length);
                }
                document.getElementById('log').textContent = out.join('|');
              });
              mo.observe(target, { attributes: true, childList: true, subtree: true });
              document.getElementById('btn').addEventListener('click', function () {
                target.setAttribute('data-x', '1');                 // attributes on target
                document.getElementById('s').setAttribute('data-y', '2'); // attributes, subtree
                target.appendChild(document.createElement('em'));    // childList add on target
              });
            </script></body></html>"#;
        let mut page11 = Page::load(html11, "https://app.test/", &fonts, 800.0);
        let root11 = page11.dom().root();
        let btn = manuk_css::query_selector_all(page11.dom(), root11, "#btn")[0];
        let log = manuk_css::query_selector_all(page11.dom(), root11, "#log")[0];
        assert_eq!(
            page11.dom().text_content(log),
            "none",
            "no records before the mutation"
        );
        page11.dispatch_click(btn, &fonts, 800.0);
        assert_eq!(
            page11.dom().text_content(log),
            "attributes:data-x:+0:-0|attributes:data-y:+0:-0|childList:-:+1:-0",
            "the observer got batched records: two attribute changes (incl. a subtree one) + a childList add"
        );

        // (12) matchMedia evaluates width features against the boot viewport (1280×720), so JS
        // responsive branches agree with the CSS @media cascade.
        let html12 = r#"<!doctype html><html><body id="mm"><script>
            document.getElementById('mm').setAttribute('data-r',
              matchMedia('(max-width: 600px)').matches + ',' +
              matchMedia('(min-width: 1000px)').matches + ',' +
              matchMedia('(min-width: 1000px) and (max-width: 1400px)').matches);
            </script></body></html>"#;
        let page12 = Page::load(html12, "https://app.test/", &fonts, 800.0);
        let mm = manuk_css::query_selector_all(page12.dom(), page12.dom().root(), "#mm")[0];
        assert_eq!(
            page12.dom().element(mm).and_then(|e| e.attr("data-r")),
            Some("false,true,true"),
            "matchMedia: not-narrow, is-wide, and in-range at 1280px wide"
        );

        // (15) **Media: graceful degradation means answering honestly, not staying silent.**
        //
        // We cannot decode video or audio, and that is fine — a browser is allowed to lack a codec.
        // What is not fine is the shape the limit took: `video.play` was `undefined`, so a site calling
        // it threw a TypeError and took the whole page down, and a site that *politely feature-detected*
        // with `canPlayType` read `undefined` and could not even be told no.
        //
        // Every assertion below is the spec's own vocabulary for a browser that cannot play a thing.
        // `play()` rejecting is the best-tested failure path on the web — autoplay policies make
        // rejection routine in real browsers, so every player library already handles it.
        let html15 = r#"<!doctype html><html><body>
            <video id="v" width="640" height="360" poster="p.png" controls>
              <source src="m.mp4" type="video/mp4">
            </video>
            <div id="out">-</div>
            <script>
              var v = document.getElementById('v'), r = [];
              // '' is the spec's "no". 'probably'/'maybe' are the only other answers and both are lies.
              r.push('cannot:' + (v.canPlayType('video/mp4') === ''));
              r.push('state:' + (v.paused === true && v.readyState === 0 && v.networkState === 3));
              r.push('err:' + (v.error && v.error.code === 4));      // MEDIA_ERR_SRC_NOT_SUPPORTED
              r.push('iface:' + (v instanceof HTMLMediaElement));
              // Setters must not throw. Scripts assign these unconditionally.
              v.pause(); v.currentTime = 5; v.volume = 0.5; v.load();
              r.push('setters:' + (v.currentTime === 5));
              // play() must return a REJECTED promise, never throw and never resolve.
              var p = v.play();
              r.push('promise:' + (p && typeof p.then === 'function'));
              p.then(function(){ document.getElementById('out').textContent = 'PLAY RESOLVED (a lie)'; })
               .catch(function(e){
                 r.push('rejected:' + (e.name === 'NotSupportedError'));
                 document.getElementById('out').textContent = r.join(' ');
               });
            </script></body></html>"#;
        let page15 = Page::load(html15, "https://app.test/", &fonts, 800.0);
        let root15 = page15.dom().root();
        let out15 = manuk_css::query_selector_all(page15.dom(), root15, "#out")[0];
        let got15 = page15.dom().text_content(out15);
        for claim in [
            "cannot:true",   // canPlayType() === '' — an honest no
            "state:true",    // paused / HAVE_NOTHING / NETWORK_NO_SOURCE
            "err:true",      // MediaError code 4
            "iface:true",    // instanceof HTMLMediaElement
            "setters:true",  // currentTime/volume/pause/load do not throw
            "promise:true",  // play() returns a thenable
            "rejected:true", // ...and it REJECTS with NotSupportedError
        ] {
            assert!(
                got15.contains(claim),
                "G2/15 media degradation failed: expected {claim} in {got15:?}"
            );
        }

        // (14) **The framework primitives.** Every assertion here is a bug that shipped, and each is
        // labelled with the framework that found it — because none of them would have been picked out
        // of the DOM standard by reading. The browser telling us its own bug is a discovery mechanism
        // that nothing else replaces (Part 31).
        //
        // The `ownerDocument` case is the one that matters most, and it is why this scenario forces a
        // GC rather than merely calling the getter. `DOC_REFLECTOR` was an UNROOTED `*mut JSObject`:
        // it worked perfectly until the collector moved the document, after which `ownerDocument`
        // returned whatever now occupied that address. React allocates hard enough to trigger that
        // reliably, and got back one of our own MutationRecords — an object on which `createElement`
        // is genuinely not a function. **A test that does not allocate cannot see this bug at all**,
        // which is exactly why it survived so long.
        let html14 = r#"<!doctype html><html><body>
            <div id="host"></div><div id="out">-</div>
            <script>
              var r = [];

              // React — ownerDocument must survive a garbage collection. Allocate hard first.
              var host = document.getElementById('host');
              for (var i = 0; i < 60000; i++) { var junk = { a: i, b: 'x' + i, c: [i, i, i] }; }
              var od = host.ownerDocument;
              r.push('ownerDoc:' + (od === document && typeof od.createElement === 'function'));

              // Svelte 5 — lifts the raw accessor straight off Node.prototype and .call()s it.
              var d = Object.getOwnPropertyDescriptor(Node.prototype, 'firstChild');
              var getter = d && d.get;
              var probe = document.createElement('div');
              probe.appendChild(document.createElement('span'));
              r.push('protoAccessor:' + (!!getter && getter.call(probe).tagName === 'SPAN'));

              // Lit — reads `.data` off the comment markers a TreeWalker hands it.
              var cmt = document.createComment('marker');
              r.push('commentData:' + (cmt.data === 'marker' && cmt.nodeType === 8));

              // Lit — a shadow root is a DocumentFragment (11), not a comment (8).
              var sr = host.attachShadow({ mode: 'open' });
              r.push('shadowType:' + (sr.nodeType === 11 && host.getRootNode() === document));

              // lit-html — a fragment inserted before a null reference contributes its CHILDREN.
              var t = document.createElement('template');
              t.innerHTML = '<b>A</b><i>B</i>';
              var frag = t.content.cloneNode(true);
              var marker = sr.insertBefore(document.createComment(''), null);
              marker.parentNode.insertBefore(frag, null);
              r.push('fragInsert:' + (sr.childNodes.length === 3));

              // The ChildNode / ParentNode mixins — all eleven were missing.
              var m = document.createElement('div');
              m.append('x');
              m.prepend(document.createElement('em'));
              m.insertAdjacentHTML('beforeend', '<u>u</u>');
              r.push('mixins:' + (m.outerHTML === '<div><em></em>x<u>u</u></div>' &&
                                  m.hasAttributes() === false && m.hasChildNodes() === true));

              document.getElementById('out').textContent = r.join(' ');
            </script></body></html>"#;
        let page14 = Page::load(html14, "https://app.test/", &fonts, 800.0);
        let root14 = page14.dom().root();
        let out14 = manuk_css::query_selector_all(page14.dom(), root14, "#out")[0];
        let got = page14.dom().text_content(out14);
        for claim in [
            "ownerDoc:true",      // React   — a raw *mut JSObject cached across a GC is a bug
            "protoAccessor:true", // Svelte 5 — get_descriptor(Node.prototype,'firstChild').get
            "commentData:true",   // Lit      — CharacterData.data on its binding markers
            "shadowType:true",    // Lit      — a shadow root is a DocumentFragment
            "fragInsert:true",    // lit-html — every template commits through this call
            "mixins:true",        // everyone — append/prepend/insertAdjacentHTML/outerHTML
        ] {
            assert!(
                got.contains(claim),
                "G2/14 framework primitive failed: expected {claim} in {got:?}"
            );
        }

        // (13) Custom elements + Shadow DOM: a class extending HTMLElement is defined, the
        // existing element upgrades (its constructor runs with `this` === the real element),
        // attachShadow gives it a shadow root whose content is set from JS, and
        // connectedCallback + attributeChangedCallback fire.
        let html13 = r#"<!doctype html><html><body>
            <my-widget label="Hello"></my-widget>
            <div id="trace">-</div>
            <script>
              class MyWidget extends HTMLElement {
                static get observedAttributes() { return ['label']; }
                constructor() {
                  super();
                  this.attachShadow({ mode: 'open' });
                  this.shadowRoot.innerHTML = '<b>SHADOW</b>';
                }
                connectedCallback() { this.setAttribute('data-connected', '1'); }
                attributeChangedCallback(name, oldV, newV) {
                  document.getElementById('trace').textContent = 'attr:' + name + '=' + newV;
                }
              }
              customElements.define('my-widget', MyWidget);
            </script></body></html>"#;
        let page13 = Page::load(html13, "https://app.test/", &fonts, 800.0);
        let root13 = page13.dom().root();
        let widget = manuk_css::query_selector_all(page13.dom(), root13, "my-widget")[0];
        let trace = manuk_css::query_selector_all(page13.dom(), root13, "#trace")[0];

        // connectedCallback ran on upgrade.
        assert_eq!(
            page13
                .dom()
                .element(widget)
                .and_then(|e| e.attr("data-connected")),
            Some("1"),
            "connectedCallback fired on upgrade"
        );
        // attributeChangedCallback fired for the observed attribute already present.
        assert_eq!(
            page13.dom().text_content(trace),
            "attr:label=Hello",
            "attributeChangedCallback fired for an observed attribute present at upgrade"
        );
        // attachShadow gave the host a real shadow root, and its content is in the shadow tree
        // (NOT a light-DOM child of the host).
        let sr = page13
            .dom()
            .shadow_root(widget)
            .expect("shadow root attached from JS");
        assert!(page13.dom().is_shadow_root(sr));
        assert_eq!(
            page13.dom().text_content(sr),
            "SHADOW",
            "shadowRoot.innerHTML populated the shadow tree"
        );
        assert!(
            !page13.dom().text_content(widget).contains("SHADOW"),
            "shadow content is not a light-DOM child of the host"
        );

        // (14) The **browser-capability gate** every MediaWiki site in the world runs on us:
        //
        //   isCompatible() = 'querySelector' in document && 'localStorage' in window
        //                    && typeof Promise === 'function' && Promise.prototype.finally
        //                    && /./g.flags === 'g' && new Function('async (a = 0,) => a')
        //
        // Fail it and the site does not merely lose a feature — it *grades the browser down* and
        // ships its no-script fallback. That is what kept Wikipedia's table of contents expanded
        // and threw the whole page thousands of pixels out of alignment. Passing it is what makes
        // Manuk a first-class browser to the sites that ask.
        let html14 = r#"<!doctype html><html><body><p id="o">no</p>
            <script>
              var ok = !!('querySelector' in document && 'localStorage' in window
                && typeof Promise === 'function' && Promise.prototype['finally']
                && /./g.flags === 'g'
                && (function(){ try { new Function('async (a = 0,) => a'); return true; }
                                catch(e){ return false; } }()));
              document.getElementById('o').textContent = ok ? 'GRADE-A' : 'GRADE-C';
            </script></body></html>"#;
        let page14 = Page::load(html14, "https://example.test/", &fonts, 800.0);
        let r14 = page14.dom().root();
        let o14 = manuk_css::query_selector_all(page14.dom(), r14, "#o")[0];
        assert_eq!(
            page14.dom().text_content(o14),
            "GRADE-A",
            "the real MediaWiki capability gate must grade Manuk as a fully capable browser"
        );

        // (15) `document.documentElement.className = ...` re-cascades. The web bootstraps itself
        // this way (`client-nojs` → `client-js`, dark mode, feature flags): a class set by script
        // must select real styles, or the page is styled for a world it isn't in.
        let html15 = r#"<!doctype html><html class="a"><head>
            <style>.a #x{height:300px} .b #x{height:20px}</style>
            <script>document.documentElement.className='b';</script>
            </head><body><div id="x"></div></body></html>"#;
        let page15 = Page::load(html15, "https://example.test/", &fonts, 800.0);
        let r15 = page15.dom().root();
        let x15 = manuk_css::query_selector_all(page15.dom(), r15, "#x")[0];
        let h15 = page15.root_box.node_rects(page15.dom())[&x15].height;
        assert!(
            (h15 - 20.0).abs() < 1.0,
            "a script-set class on <html> must drive the cascade (got {h15}px, want 20px)"
        );

        // (16) Web Storage round-trips through the real, origin-partitioned store, and one origin
        // cannot read another's.
        let html16 = r#"<!doctype html><html><body><p id="o">?</p>
            <script>
              localStorage.setItem('k', 'v1');
              sessionStorage.setItem('s', 'v2');
              document.getElementById('o').textContent =
                localStorage.getItem('k') + '/' + sessionStorage.getItem('s')
                + '/' + localStorage.length + '/' + ('k' in localStorage);
            </script></body></html>"#;
        let page16 = Page::load(html16, "https://storage-a.test/page", &fonts, 800.0);
        let r16 = page16.dom().root();
        let o16 = manuk_css::query_selector_all(page16.dom(), r16, "#o")[0];
        assert_eq!(
            page16.dom().text_content(o16),
            "v1/v2/1/true",
            "localStorage/sessionStorage must round-trip, count, and answer `in`"
        );
        let html17 = r#"<!doctype html><html><body><p id="o">?</p>
            <script>document.getElementById('o').textContent =
                String(localStorage.getItem('k'));</script></body></html>"#;
        let page17 = Page::load(html17, "https://storage-b.test/page", &fonts, 800.0);
        let r17 = page17.dom().root();
        let o17 = manuk_css::query_selector_all(page17.dom(), r17, "#o")[0];
        assert_eq!(
            page17.dom().text_content(o17),
            "null",
            "storage is partitioned by origin — storage-b must not see storage-a's key"
        );

        // (18) **Property → attribute reflection.** `createElement` → assign → `appendChild` is how
        // the modern web builds every element it did not ship in the HTML. Without reflection,
        // `link.href = url` sets a plain JS property on the wrapper and touches nothing in the DOM,
        // so the element that reaches the tree is empty — and a page can never load its own
        // code-split CSS or JS.
        let html18 = r#"<!doctype html><html><body><img id="i"><p id="o">?</p>
            <script>
              var im = document.getElementById('i');
              im.src = 'x.png';
              var a = document.createElement('a');
              a.href = '/next';
              a.rel = 'next';
              document.body.appendChild(a);
              document.getElementById('o').textContent =
                im.getAttribute('src') + '|' + a.getAttribute('href') + '|' + a.rel + '|' + im.src;
            </script></body></html>"#;
        let page18 = Page::load(html18, "https://example.test/dir/page", &fonts, 800.0);
        let r18 = page18.dom().root();
        let o18 = manuk_css::query_selector_all(page18.dom(), r18, "#o")[0];
        assert_eq!(
            page18.dom().text_content(o18),
            "x.png|/next|next|https://example.test/dir/x.png",
            "assigning .src/.href/.rel must write the CONTENT ATTRIBUTE, and .src must read back \
             resolved against the document"
        );

        // (19) **CSSOM + the DOM ergonomics every framework is written with.** `el.style.width = …`
        // is the most common DOM write on the web and `classList.add` is not far behind. Before this
        // they threw a TypeError that aborted the rest of the page's script — one missing property
        // taking the whole page's interactivity with it. A style written by script must also reach
        // the CASCADE, not just the attribute: the assertion checks the resulting BOX.
        let html19 = r#"<!doctype html><html><body>
            <div id="a" class="one" data-user-id="42">A</div><p id="o">?</p>
            <script>
              var a = document.getElementById('a');
              a.style.width = '123px';
              a.style.backgroundColor = 'red';
              a.classList.add('two');
              a.classList.toggle('one');
              document.getElementById('o').textContent = [
                a.getAttribute('style'), a.className, a.classList.contains('two'),
                a.classList.length, a.dataset.userId, a.matches('#a.two'),
                a.closest('body').tagName, document.body.contains(a)
              ].join('|');
            </script></body></html>"#;
        let page19 = Page::load(html19, "https://example.test/", &fonts, 600.0);
        let r19 = page19.dom().root();
        let o19 = manuk_css::query_selector_all(page19.dom(), r19, "#o")[0];
        assert_eq!(
            page19.dom().text_content(o19),
            "width: 123px; background-color: red|two|true|1|42|true|BODY|true",
            "style/classList/dataset/matches/closest/contains must all be live views of the DOM"
        );
        let a19 = manuk_css::query_selector_all(page19.dom(), r19, "#a")[0];
        let w19 = page19.root_box.node_rects(page19.dom())[&a19].width;
        assert!(
            (w19 - 123.0).abs() < 1.0,
            "a style written by script must drive the CASCADE, not just the attribute \
             (got {w19}px, want 123px)"
        );

        // (20) **Events the page constructs itself.** A page does not merely *listen* — component
        // libraries signal through `CustomEvent`, and `dispatchEvent(new Event('input'))` is how a
        // framework tells a control it changed. `dispatchEvent` took only a *string*, so an Event
        // object was coerced to `"[object Object]"` and its whole payload — detail, key,
        // coordinates — was thrown away. `new CustomEvent(...)` was a ReferenceError besides.
        //
        // Also asserted: `stopPropagation` stops the WALK but not the remaining listeners on the
        // same node (that is `stopImmediatePropagation`) — conflating them silences handlers that
        // should still run.
        let html20 = r#"<!doctype html><html><body>
            <div id="host"><button id="b">go</button></div><p id="o">?</p>
            <script>
              var log = [];
              var host = document.getElementById('host');
              var b = document.getElementById('b');
              host.addEventListener('pick', function (e) { log.push('detail=' + e.detail + ',trusted=' + e.isTrusted); });
              b.addEventListener('click', function (e) { log.push('phase=' + e.eventPhase + ',x=' + e.clientX); e.stopPropagation(); });
              host.addEventListener('click', function () { log.push('LEAKED'); });
              b.dispatchEvent(new CustomEvent('pick', { bubbles: true, detail: 42 }));
              b.dispatchEvent(new MouseEvent('click', { bubbles: true, clientX: 5 }));
              b.focus();
              log.push('scrollY=' + window.scrollY);
              log.push('active=' + document.activeElement.id);
              document.getElementById('o').textContent = log.join('|');
            </script></body></html>"#;
        let page20 = Page::load(html20, "https://example.test/", &fonts, 600.0);
        let r20 = page20.dom().root();
        let o20 = manuk_css::query_selector_all(page20.dom(), r20, "#o")[0];
        assert_eq!(
            page20.dom().text_content(o20),
            "detail=42,trusted=false|phase=2,x=5|scrollY=0|active=b",
            "a page-constructed event must keep its payload, bubble, and be untrusted; \
             stopPropagation must stop the walk (no LEAKED); focus/activeElement must work"
        );
        drop(page20);

        // (21) **IntersectionObserver + ResizeObserver + the `scroll` event** — the machinery the
        // real-time web is built on: lazy images, infinite scroll, a sentinel at the bottom of a
        // feed, sticky headers that latch, components that re-layout with their container.
        //
        // A feed built on these does not merely *look* wrong without them — it loads its first
        // screenful and then stops forever, because nothing ever tells it the sentinel came into
        // view. Only the ENGINE knows when a box moved, so the engine drives them.
        let html21 = r#"<!doctype html><html><body>
            <div id="top" style="height:40px">top</div>
            <div id="sentinel" style="height:20px;margin-top:2000px">end</div>
            <p id="o">?</p>
            <script>
              var log = [];
              window.addEventListener('scroll', function () { log.push('scroll@' + window.scrollY); });
              new IntersectionObserver(function (entries) {
                entries.forEach(function (e) {
                  log.push('io:' + e.target.id + '=' + e.isIntersecting);
                });
              }).observe(document.getElementById('sentinel'));
              new ResizeObserver(function (entries) {
                entries.forEach(function (e) { log.push('ro:' + e.target.id + '=' + Math.round(e.contentRect.height)); });
              }).observe(document.getElementById('top'));
              window.__log = log;
              document.getElementById('o').textContent = 'ready';
            </script></body></html>"#;
        let mut page21 = Page::load(html21, "https://example.test/", &fonts, 600.0);
        // First pass at the top of the document: the sentinel is 2,000px down, so it is NOT in view;
        // the observed div IS its size. Then scroll to it.
        page21.view_changed(0.0, 600.0, 400.0, false);
        page21.view_changed(2000.0, 600.0, 400.0, true);
        let r21 = page21.dom().root();
        let o21 = manuk_css::query_selector_all(page21.dom(), r21, "#o")[0];
        // Read the log back out through the DOM (the only channel the test has into the page).
        page21.eval_for_test("document.getElementById('o').textContent = window.__log.join('|')");
        let got = page21.dom().text_content(o21);
        assert!(
            got.contains("ro:top=40"),
            "ResizeObserver must report the observed box's size (got {got:?})"
        );
        assert!(
            got.contains("io:sentinel=false"),
            "the sentinel is 2,000px down — it must start OUT of view (got {got:?})"
        );
        assert!(
            got.contains("scroll@2000"),
            "the `scroll` event must fire with the live offset (got {got:?})"
        );
        assert!(
            got.contains("io:sentinel=true"),
            "scrolling to the sentinel must report it INTERSECTING — this is the moment an \
             infinite feed loads its next screenful (got {got:?})"
        );
        drop(page21);

        // (21b) **`rootMargin` is a 4-side CSS shorthand, and its BOTTOM margin is what makes an
        // infinite feed prefetch.** The idiom `rootMargin: '0px 0px 300px 0px'` extends only the
        // bottom edge so the sentinel fires *before* it scrolls into view. Honouring only the first
        // token (the old bug) dropped that margin and the feed loaded late or never. Here a sentinel
        // sits 20px BELOW the 600px viewport (top=620, not visible with a 0 margin); a second
        // observer with a 200px bottom margin must report it intersecting with NO scroll at all,
        // while a plain observer must not.
        let html21b = r#"<!doctype html><html><body>
            <div style="height:620px">spacer</div>
            <div id="s" style="height:20px">sentinel</div>
            <p id="o">?</p>
            <script>
              var mlog = [];
              new IntersectionObserver(function (es) {
                es.forEach(function (e) { mlog.push('plain:' + e.isIntersecting); });
              }, { rootMargin: '0px' }).observe(document.getElementById('s'));
              new IntersectionObserver(function (es) {
                es.forEach(function (e) { mlog.push('prefetch:' + e.isIntersecting); });
              }, { rootMargin: '0px 0px 200px 0px' }).observe(document.getElementById('s'));
              window.__mlog = mlog;
            </script></body></html>"#;
        let mut page21b = Page::load(html21b, "https://example.test/", &fonts, 600.0);
        // No scroll: viewport is [0,600] in doc coords; the sentinel top is 620.
        page21b.view_changed(0.0, 600.0, 600.0, false);
        let r21b = page21b.dom().root();
        let o21b = manuk_css::query_selector_all(page21b.dom(), r21b, "#o")[0];
        page21b.eval_for_test("document.getElementById('o').textContent = window.__mlog.join('|')");
        let got21b = page21b.dom().text_content(o21b);
        assert!(
            got21b.contains("plain:false"),
            "a 0-margin observer must NOT report the 20px-below-viewport sentinel intersecting \
             (got {got21b:?})"
        );
        assert!(
            got21b.contains("prefetch:true"),
            "a '0px 0px 200px 0px' bottom-margin observer MUST report the sentinel intersecting with \
             no scroll — this is the infinite-feed prefetch that only the first rootMargin token \
             dropped (got {got21b:?})"
        );
        drop(page21b);

        // (21c) **Intersection is 2-D.** A slide in a horizontal carousel is vertically in view but
        // scrolled off to the SIDE; the old vertical-only test reported it intersecting and every
        // off-screen slide eager-loaded. An element at x=800 in a 400px-wide viewport must NOT
        // intersect; a `right` rootMargin that reaches it must (this is also the only thing that
        // exercises the parsed-but-unused left/right margins).
        let html21c = r#"<!doctype html><html><body>
            <div id="h" style="position:absolute;left:800px;top:5px;width:50px;height:20px">slide</div>
            <p id="o">?</p>
            <script>
              var hlog = [];
              new IntersectionObserver(function (es) {
                es.forEach(function (e) { hlog.push('hplain:' + e.isIntersecting); });
              }, { rootMargin: '0px' }).observe(document.getElementById('h'));
              new IntersectionObserver(function (es) {
                es.forEach(function (e) { hlog.push('hright:' + e.isIntersecting); });
              }, { rootMargin: '0px 500px 0px 0px' }).observe(document.getElementById('h'));
              window.__hlog = hlog;
            </script></body></html>"#;
        let mut page21c = Page::load(html21c, "https://example.test/", &fonts, 400.0);
        page21c.view_changed(0.0, 600.0, 400.0, false);
        let r21c = page21c.dom().root();
        let o21c = manuk_css::query_selector_all(page21c.dom(), r21c, "#o")[0];
        page21c.eval_for_test("document.getElementById('o').textContent = window.__hlog.join('|')");
        let got21c = page21c.dom().text_content(o21c);
        assert!(
            got21c.contains("hplain:false"),
            "an element at x=800 in a 400px viewport is off-screen to the RIGHT and must NOT intersect \
             — the vertical-only test wrongly reported it visible (got {got21c:?})"
        );
        assert!(
            got21c.contains("hright:true"),
            "a '0px 500px 0px 0px' right-margin observer reaches x=800 and MUST report the slide \
             intersecting — this exercises the horizontal rootMargin (got {got21c:?})"
        );
        drop(page21c);

        // (22) **State pseudo-classes, and the URL decomposition IDL.** Two gaps that each killed a
        // whole class of page.
        //
        // `:checked` is the CSS-only interactivity primitive — `#toggle:checked ~ .panel` is how a
        // large part of the web builds a menu, an accordion, a dropdown or a sidebar with NO
        // JavaScript at all. The shipping cascade answered `false` to every pseudo-class, so every
        // one of those was frozen shut.
        //
        // `a.protocol` is not obscure either: a link is the web's canonical URL object. mdbook's
        // table-of-contents script does `a.protocol.replace(...)`; with `protocol` undefined that is
        // a TypeError, the script dies, and the navigation column of every mdbook site on the
        // internet never gets built.
        let html22 = r#"<!doctype html><html><body>
            <style>#t:not(:checked) ~ #s { display: none } #s { height: 40px }</style>
            <input type="checkbox" id="t">
            <div id="s">panel</div>
            <a id="a" href="/x/y?q=1#f">link</a>
            <p id="o">?</p>
            <script>
              document.getElementById('t').checked = true;
              var a = document.getElementById('a');
              document.getElementById('o').textContent =
                [a.protocol, a.hostname, a.pathname, a.search, a.hash,
                 document.scrollingElement.tagName, document.body.clientWidth > 0].join('|');
            </script></body></html>"#;
        let page22 = Page::load(html22, "https://ex.test/base/", &fonts, 800.0);
        let r22 = page22.dom().root();
        let o22 = manuk_css::query_selector_all(page22.dom(), r22, "#o")[0];
        assert_eq!(
            page22.dom().text_content(o22),
            "https:|ex.test|/x/y|?q=1|#f|HTML|true",
            "a link is a URL object; document.scrollingElement is <html>; body has a width"
        );
        // The checkbox hack: a script set `.checked`, so `:checked` must now MATCH and the panel
        // must be revealed. This is the assertion that a whole class of JS-free UI depends on.
        let s22 = manuk_css::query_selector_all(page22.dom(), r22, "#s")[0];
        let h22 = page22
            .root_box
            .node_rects(page22.dom())
            .get(&s22)
            .map(|r| r.height);
        assert_eq!(
            h22,
            Some(40.0),
            "`#t:checked ~ #s` must reveal the panel once a script checks the box — the CSS-only \
             toggle is how much of the web builds menus and sidebars with no JS at all"
        );
        drop(page22);

        // (23) **`getComputedStyle` exposes the flexbox resolved values.** Every flex longhand —
        // `flexDirection`, `flexWrap`, `justifyContent`, `alignItems`, `flexGrow`, `flexShrink`,
        // `flexBasis`, `alignSelf`, `rowGap`/`columnGap` — used to read back `undefined`, so any
        // framework that measured a flex container (`getComputedStyle(el).alignItems`, a CSS-in-JS
        // lib re-reading resolved values, an animation lib interpolating `flex-grow`) got garbage
        // concatenated into its logic. The resolved value is the CSS keyword, exactly as Chrome
        // serializes it; `getPropertyValue('flex-direction')` must reach the same value by kebab name.
        let html23 = r#"<!doctype html><html><body>
            <div id="f" style="display:flex;flex-direction:column;flex-wrap:wrap;justify-content:space-between;align-items:center">
              <div id="c" style="flex-grow:2;flex-shrink:0;flex-basis:100px;align-self:flex-end">item</div>
            </div>
            <p id="o">?</p>
            <script>
              var f = getComputedStyle(document.getElementById('f'));
              var c = getComputedStyle(document.getElementById('c'));
              document.getElementById('o').textContent = [
                f.flexDirection, f.flexWrap, f.justifyContent, f.alignItems,
                c.flexGrow, c.flexShrink, c.flexBasis, c.alignSelf,
                f.getPropertyValue('flex-direction')
              ].join('|');
            </script></body></html>"#;
        let page23 = Page::load(html23, "https://ex.test/", &fonts, 800.0);
        let r23 = page23.dom().root();
        let o23 = manuk_css::query_selector_all(page23.dom(), r23, "#o")[0];
        assert_eq!(
            page23.dom().text_content(o23),
            "column|wrap|space-between|center|2|0|100px|flex-end|column",
            "getComputedStyle must resolve every flexbox longhand (these read back `undefined` \
             before — the exact over-read that breaks framework layout measurement); \
             getPropertyValue('flex-direction') must reach it by kebab name too"
        );
        drop(page23);

        // (24) **`getComputedStyle` exposes the box-model longhands a framework measures with.**
        // `boxSizing` (is this a border-box element? — the single most-read layout flag), and the
        // `minWidth`/`maxWidth`/`minHeight`/`maxHeight` constraints. These read `undefined` before.
        // The subtle one: `max-*` uses `Dim::Auto` to mean *unconstrained*, whose CSS resolved value
        // is **`none`**, not `auto` (only `min-*` resolves to `auto`) — a wrong default here would
        // silently mislead any code that branches on `maxWidth === 'none'`.
        let html24 = r#"<!doctype html><html><body>
            <div id="b" style="box-sizing:border-box;min-width:50px;max-width:300px;min-height:10px">box</div>
            <p id="o">?</p>
            <script>
              var s = getComputedStyle(document.getElementById('b'));
              document.getElementById('o').textContent = [
                s.boxSizing, s.minWidth, s.maxWidth, s.minHeight, s.maxHeight,
                s.getPropertyValue('box-sizing')
              ].join('|');
            </script></body></html>"#;
        let page24 = Page::load(html24, "https://ex.test/", &fonts, 800.0);
        let r24 = page24.dom().root();
        let o24 = manuk_css::query_selector_all(page24.dom(), r24, "#o")[0];
        assert_eq!(
            page24.dom().text_content(o24),
            "border-box|50px|300px|10px|none|border-box",
            "getComputedStyle must expose box-sizing + the min/max constraints (undefined before); \
             an unset `max-height` resolves to `none`, not `auto`; getPropertyValue reaches box-sizing too"
        );
        drop(page24);

        // (25) **`fetch(url, {signal})` honours AbortController** — the request-cancellation every
        // React `useEffect` cleanup (and StrictMode's double-mount) relies on. Three behaviours, all
        // observed after a single event-loop drain: (a) the default abort reason is a DOMException
        // named 'AbortError'; (b) a fetch on an *already-aborted* signal rejects and queues **no**
        // network request; (c) a fetch aborted **in flight** rejects, and its callback is dropped so a
        // late host delivery cannot resolve it (the response body must never reach the page).
        let html25 = r#"<!doctype html><html><body>
            <div id="out" data-pre="pending" data-reason="?" data-inflight="pending">loading</div>
            <script>
              var el = document.getElementById('out');
              el.setAttribute('data-reason', AbortSignal.abort().reason.name);
              var c1 = new AbortController(); c1.abort();
              fetch('/pre', {signal: c1.signal}).then(
                function(){ el.setAttribute('data-pre', 'RESOLVED'); },
                function(e){ el.setAttribute('data-pre', e && e.name); });
              var c2 = new AbortController();
              fetch('/inflight', {signal: c2.signal}).then(
                function(t){ el.setAttribute('data-inflight', 'RESOLVED:' + t); },
                function(e){ el.setAttribute('data-inflight', e && e.name); });
              c2.abort();
            </script></body></html>"#;
        let mut page25 = Page::load(html25, "https://ex.test/", &fonts, 800.0);
        let out25 = manuk_css::query_selector_all(page25.dom(), page25.dom().root(), "#out")[0];
        // (b) The pre-aborted fetch queued NO request; only the in-flight one reached the host.
        let reqs25 = page25.take_fetches();
        assert_eq!(
            reqs25.len(),
            1,
            "a pre-aborted fetch must not queue a network request"
        );
        assert_eq!(
            &reqs25[0].1, "/inflight",
            "only the non-pre-aborted request is queued"
        );
        // Deliver the in-flight request LATE — after it was aborted. Its callback was dropped on
        // abort, so this must be a no-op (the body must not reach the page). This also drains the loop,
        // running the two `.catch` handlers.
        page25.resolve_fetch(reqs25[0].0, 200, "LATEBODY", &[], &fonts, 800.0);
        assert_eq!(
            page25
                .dom()
                .element(out25)
                .and_then(|e| e.attr("data-reason")),
            Some("AbortError"),
            "the default abort reason is a DOMException named 'AbortError'"
        );
        assert_eq!(
            page25.dom().element(out25).and_then(|e| e.attr("data-pre")),
            Some("AbortError"),
            "a fetch on an already-aborted signal rejects with AbortError"
        );
        assert_eq!(
            page25.dom().element(out25).and_then(|e| e.attr("data-inflight")),
            Some("AbortError"),
            "an in-flight abort rejects, and the late delivery is dropped (body 'LATEBODY' never resolves it)"
        );
        drop(page25);

        // (26) **`fetch(url, {body: FormData})` sends `multipart/form-data`, so a File is uploaded.**
        // A FormData body used to be `String(fd)` → urlencoded, which turns a File part into the
        // literal `"[object File]"` — every uploaded avatar/attachment/document was **silently
        // dropped**. Now a FormData body is multipart with a browser-generated boundary, each field a
        // part, each File carrying its filename + type + content.
        let html26 = r#"<!doctype html><html><body>
            <script>
              var fd = new FormData();
              fd.append('field', 'hello world');
              fd.append('upload', new File(['FILE-CONTENT-BYTES'], 'a.txt', { type: 'text/plain' }));
              fetch('/upload', { method: 'POST', body: fd });
            </script></body></html>"#;
        let page26 = Page::load(html26, "https://ex.test/", &fonts, 800.0);
        let reqs26 = page26.take_fetches();
        assert_eq!(reqs26.len(), 1, "the FormData fetch was queued");
        let (_id, url, method, headers, body) = &reqs26[0];
        assert_eq!(url, "/upload");
        assert_eq!(method, "POST");
        // The Content-Type is multipart with a boundary the browser chose (not any page-set value).
        let ct = headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
            .map(|(_, v)| v.as_str())
            .unwrap_or("");
        assert!(
            ct.starts_with("multipart/form-data; boundary="),
            "FormData body must send multipart/form-data, got {ct:?}"
        );
        let boundary = ct.trim_start_matches("multipart/form-data; boundary=");
        assert!(!boundary.is_empty(), "the multipart boundary is set");
        // The body is well-formed multipart carrying BOTH the text field and the file's real content.
        assert!(
            body.contains(&format!("--{boundary}")),
            "the body is delimited by the declared boundary"
        );
        assert!(
            body.contains("Content-Disposition: form-data; name=\"field\"")
                && body.contains("hello world"),
            "the text field is a part (got {body:?})"
        );
        assert!(
            body.contains("Content-Disposition: form-data; name=\"upload\"; filename=\"a.txt\"")
                && body.contains("Content-Type: text/plain")
                && body.contains("FILE-CONTENT-BYTES"),
            "the File is uploaded with filename + type + CONTENT, not dropped as [object File] (got {body:?})"
        );
        assert!(
            body.trim_end().ends_with(&format!("--{boundary}--")),
            "the multipart body has its closing boundary"
        );
        drop(page26);

        // (27) **Typing fires an `input` event — the controlled-component contract.** A framework
        // input (`<input value={state} onChange=…>`) keeps its value in JS state and re-renders from
        // it; it learns the user typed ONLY from the `input` event. `Page::dispatch_input` (what the
        // shell calls per keystroke) must set the value AND fire `input`, so the handler sees the new
        // `event.target.value`. Firing nothing (the old bare `set_attr`) left every React/Vue/Svelte
        // field reverting the keystroke on its next render.
        let html27 = r#"<!doctype html><html><body>
            <input id="f" value="">
            <p id="mirror">?</p>
            <p id="changes">0</p>
            <script>
              var f = document.getElementById('f');
              f.addEventListener('input', function (e) {
                // Read back through event.target.value — exactly what a controlled component does.
                document.getElementById('mirror').textContent = e.target.value;
              });
              var changes = 0;
              f.addEventListener('change', function () {
                document.getElementById('changes').textContent = String(++changes);
              });
            </script></body></html>"#;
        let mut page27 = Page::load(html27, "https://ex.test/", &fonts, 800.0);
        let r27 = page27.dom().root();
        let field = manuk_css::query_selector_all(page27.dom(), r27, "#f")[0];
        let mirror = manuk_css::query_selector_all(page27.dom(), r27, "#mirror")[0];
        let changes = manuk_css::query_selector_all(page27.dom(), r27, "#changes")[0];
        page27.dispatch_input(field, "hi", &fonts, 800.0);
        assert_eq!(
            page27.dom().text_content(mirror),
            "hi",
            "the input handler fired and read the new value off event.target.value"
        );
        assert_eq!(
            page27.dom().element(field).and_then(|e| e.attr("value")),
            Some("hi"),
            "the field's value was updated before the event fired"
        );
        // A second keystroke fires input again with the fuller value.
        page27.dispatch_input(field, "hip", &fonts, 800.0);
        assert_eq!(page27.dom().text_content(mirror), "hip");
        // `change` is a COMMIT event — it must NOT fire on a keystroke (only input does).
        assert_eq!(
            page27.dom().text_content(changes),
            "0",
            "dispatch_input fires `input`, never `change` (change is for blur/commit)"
        );
        drop(page27);

        // (28) **Blur fires `change` (only if the value changed) then `blur` — the commit contract.**
        // A form's field-level validation runs on `change`/`blur` ("email invalid" the instant you
        // leave the field). `Page::dispatch_blur(node, value_changed)` fires `change` (iff changed)
        // then `blur`; `change` must NOT fire on a blur where nothing changed (else a change-validator
        // runs on a field the user only tabbed through). This is the commit half of the input/change
        // pair whose keystroke half is tick 175's `input`.
        let html28 = r#"<!doctype html><html><body>
            <input id="g" value="start">
            <p id="log">-</p>
            <script>
              var g = document.getElementById('g');
              var log = document.getElementById('log');
              var events = [];
              g.addEventListener('change', function () { events.push('change'); log.textContent = events.join(','); });
              g.addEventListener('blur', function () { events.push('blur'); log.textContent = events.join(','); });
            </script></body></html>"#;
        let mut page28 = Page::load(html28, "https://ex.test/", &fonts, 800.0);
        let r28 = page28.dom().root();
        let g28 = manuk_css::query_selector_all(page28.dom(), r28, "#g")[0];
        let log28 = manuk_css::query_selector_all(page28.dom(), r28, "#log")[0];
        // Blur with no change → only `blur` fires.
        page28.dispatch_blur(g28, false, &fonts, 800.0);
        assert_eq!(
            page28.dom().text_content(log28),
            "blur",
            "a blur with no value change fires blur ONLY, not change"
        );
        // The user edits, then blurs with a change → `change` then `blur`.
        page28.dispatch_input(g28, "edited", &fonts, 800.0);
        page28.dispatch_blur(g28, true, &fonts, 800.0);
        assert_eq!(
            page28.dom().text_content(log28),
            "blur,change,blur",
            "a changed field fires change THEN blur on commit (input in tick 175 fires neither)"
        );
        drop(page28);

        // (29) **`XMLHttpRequest.abort()` honours the cancellation** — a late response must not fire
        // `onload`. `abort()` was a no-op, so a cancelled request still applied its response when it
        // arrived (a search-as-you-type box that aborts the stale request would clobber the new
        // result with the old — the classic race). Now abort drops the pending callback and fires
        // `abort`+`loadend`; a subsequent host delivery for that id is a no-op.
        let html29 = r#"<!doctype html><html><body>
            <div id="x" data-onload="no" data-onabort="no" data-loadend="no">idle</div>
            <script>
              var el = document.getElementById('x');
              var r = new XMLHttpRequest();
              r.open('GET', '/slow');
              r.onload = function () { el.setAttribute('data-onload', 'FIRED'); };
              r.onabort = function () { el.setAttribute('data-onabort', 'fired'); };
              r.onloadend = function () { el.setAttribute('data-loadend', 'fired'); };
              r.send();
              r.abort();
            </script></body></html>"#;
        let mut page29 = Page::load(html29, "https://ex.test/", &fonts, 800.0);
        let x29 = manuk_css::query_selector_all(page29.dom(), page29.dom().root(), "#x")[0];
        // The request was still queued (abort doesn't unsend a drained request); deliver it LATE.
        let reqs29 = page29.take_fetches();
        assert_eq!(reqs29.len(), 1, "the XHR queued its request");
        page29.resolve_fetch(reqs29[0].0, 200, "STALE-BODY", &[], &fonts, 800.0);
        assert_eq!(
            page29
                .dom()
                .element(x29)
                .and_then(|e| e.attr("data-onload")),
            Some("no"),
            "an aborted XHR must NOT fire onload when the (now stale) response arrives"
        );
        assert_eq!(
            page29
                .dom()
                .element(x29)
                .and_then(|e| e.attr("data-onabort")),
            Some("fired"),
            "abort() fires the abort event"
        );
        assert_eq!(
            page29
                .dom()
                .element(x29)
                .and_then(|e| e.attr("data-loadend")),
            Some("fired"),
            "abort() fires loadend"
        );
        drop(page29);

        // (30) **`keydown` fires with the real `key`, and `preventDefault()` suppresses the default.**
        // A chat composer's `onKeyDown` calls `preventDefault()` on Enter to send the message instead
        // of submitting the form; a combobox swallows ArrowDown. `Page::dispatch_key` returns whether
        // the browser default should proceed (false = a handler prevented it), and hands the handler a
        // real `event.key`/`event.keyCode`.
        let html30 = r#"<!doctype html><html><body>
            <input id="k" value="">
            <p id="seen">-</p>
            <script>
              var seen = document.getElementById('seen');
              document.getElementById('k').addEventListener('keydown', function (e) {
                seen.textContent = e.key + ':' + e.keyCode;
                if (e.key === 'Enter') { e.preventDefault(); }   // "I'll handle Enter myself"
              });
            </script></body></html>"#;
        let mut page30 = Page::load(html30, "https://ex.test/", &fonts, 800.0);
        let k30 = manuk_css::query_selector_all(page30.dom(), page30.dom().root(), "#k")[0];
        let seen30 = manuk_css::query_selector_all(page30.dom(), page30.dom().root(), "#seen")[0];
        // A normal key: the handler saw the right key + keyCode, and the default proceeds.
        assert!(
            page30.dispatch_key(k30, "keydown", "a", &fonts, 800.0),
            "an un-prevented keydown returns true (perform the default: insert the character)"
        );
        assert_eq!(
            page30.dom().text_content(seen30),
            "a:65",
            "the handler received event.key and the legacy event.keyCode"
        );
        // Enter is preventDefault()'d → the engine must NOT perform the default (form submit).
        assert!(
            !page30.dispatch_key(k30, "keydown", "Enter", &fonts, 800.0),
            "a keydown handler's preventDefault() suppresses the browser default (Enter does not submit)"
        );
        assert_eq!(
            page30.dom().text_content(seen30),
            "Enter:13",
            "Enter reports keyCode 13"
        );
        drop(page30);

        // (31) **`navigator.clipboard.writeText(...)` queues the text for the host** — the async
        // Clipboard API every "copy" button uses. It was absent, so `navigator.clipboard.writeText`
        // threw on `undefined` and the button silently did nothing. It now queues the text (the host
        // writes it to the OS clipboard) and returns a resolved Promise; `readText` resolves with the
        // last text this page wrote.
        let _ = manuk_js::take_clipboard_writes(); // clear any residue
        let html31 = r#"<!doctype html><html><body>
            <button id="c">Copy</button>
            <script>
              document.getElementById('c').addEventListener('click', function () {
                navigator.clipboard.writeText('copied-value-42');
              });
            </script></body></html>"#;
        let mut page31 = Page::load(html31, "https://ex.test/", &fonts, 800.0);
        let btn31 = manuk_css::query_selector_all(page31.dom(), page31.dom().root(), "#c")[0];
        // Nothing copied until the button is clicked.
        assert!(
            manuk_js::take_clipboard_writes().is_empty(),
            "no clipboard write before the click"
        );
        page31.dispatch_click(btn31, &fonts, 800.0);
        assert_eq!(
            manuk_js::take_clipboard_writes(),
            vec!["copied-value-42".to_string()],
            "the copy button's navigator.clipboard.writeText queued its text for the host"
        );
        drop(page31);

        // (32) **`keyup` fires on key RELEASE** — the release half of the keyboard trio. A huge swath
        // of the (jQuery-era) web binds search-as-you-type and character-counters to `keyup`, not
        // `keydown`, because they want the field's *settled* value after the keystroke. `keydown`
        // (30) and `input` (27) fire on press; without `keyup` the release listener never runs. No
        // default action is associated with keyup, so `dispatch_key`'s return is irrelevant here.
        let html32 = r#"<!doctype html><html><body>
            <input id="s" value="">
            <p id="up">-</p>
            <script>
              var up = document.getElementById('up');
              document.getElementById('s').addEventListener('keyup', function (e) {
                up.textContent = e.key + ':' + e.keyCode;   // read the released key
              });
            </script></body></html>"#;
        let mut page32 = Page::load(html32, "https://ex.test/", &fonts, 800.0);
        let s32 = manuk_css::query_selector_all(page32.dom(), page32.dom().root(), "#s")[0];
        let up32 = manuk_css::query_selector_all(page32.dom(), page32.dom().root(), "#up")[0];
        assert_eq!(
            page32.dom().text_content(up32),
            "-",
            "no keyup handler has run before a key is released"
        );
        // A key is released over the focused field → keyup fires carrying event.key + event.keyCode.
        page32.dispatch_key(s32, "keyup", "x", &fonts, 800.0);
        assert_eq!(
            page32.dom().text_content(up32),
            "x:88",
            "keyup fires on release with the real event.key and legacy event.keyCode"
        );
        drop(page32);

        // Tear SpiderMonkey down before this process exits, exactly as the shell and the harness do.
        // Every page above is out of scope by now, so no rooted object outlives its runtime. Leaving
        // the engine up means its C++ static destructors run at exit against a live context and
        // abort — the test would pass and the process would still fail.
        drop(page18);
        drop(page19);
        manuk_js::shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sticky_header_pins_on_scroll_and_releases_at_bottom() {
        let html = r#"<body style="margin:0">
            <div id="h" style="position:sticky;top:0;height:40px">Header</div>
            <div style="height:2000px">tall content</div>
        </body>"#;
        let fonts = FontContext::new();
        let page = Page::load(html, "https://x.test/", &fonts, 400.0);
        assert!(page.has_sticky, "sticky is detected");

        let hid = manuk_css::query_selector_all(page.dom(), page.dom().root(), "#h")[0];
        let rects: std::collections::HashMap<_, _> =
            page.root_box.node_rects(page.dom()).into_iter().collect();
        let natural_y = rects[&hid].y;

        // Scrolled 500px past the top: the header pins so it stays at the viewport top (top:0),
        // i.e. its document y rises to ~scroll_y.
        let mut boxes = page.root_box.clone();
        apply_sticky(&mut boxes, &page.styles, 500.0);
        let pinned: std::collections::HashMap<_, _> =
            boxes.node_rects(page.dom()).into_iter().collect();
        assert!(
            (pinned[&hid].y - (natural_y + 500.0)).abs() < 1.5,
            "sticky header pinned to the scroll offset (natural {natural_y}, got {})",
            pinned[&hid].y
        );
    }

    #[test]
    fn preload_scanner_finds_stylesheets_and_preloads() {
        let html = r#"<head>
            <link rel="stylesheet" href="/a.css">
            <link rel='preload' as='font' href='https://cdn.test/f.woff2'>
            <link rel="icon" href="/favicon.ico">
            <link rel="stylesheet" href="/a.css">
        </head>"#;
        let urls = scan_preloads(html, "https://e.test/page");
        assert!(
            urls.contains(&"https://e.test/a.css".to_string()),
            "found stylesheet: {urls:?}"
        );
        assert!(
            urls.contains(&"https://cdn.test/f.woff2".to_string()),
            "found preload: {urls:?}"
        );
        assert!(
            !urls.iter().any(|u| u.contains("favicon")),
            "icon is not preloaded"
        );
        assert_eq!(
            urls.iter().filter(|u| u.contains("a.css")).count(),
            1,
            "deduped"
        );
    }

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
        assert!(
            bbox.width > 0.0 && bbox.height > 0.0,
            "degenerate bbox: {bbox:?}"
        );

        // The heading is laid out above the button (normal block flow).
        let h1 = tree
            .find(&manuk_a11y::Role::Heading { level: 1 }, "Heading")
            .unwrap();
        assert!(h1.bbox.unwrap().y < bbox.y, "h1 should precede the button");

        // Hit-testing the button's own center resolves to the button.
        let (cx, cy) = bbox.center();
        assert_eq!(tree.hit_test(cx, cy).map(|n| n.node), Some(btn.node));

        // The viewport rendering carries a click point for the button.
        let vp = manuk_a11y::Rect {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
        };
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
        assert!(
            big_h > base_h,
            "zoom-in must grow content: {base_h} -> {big_h}"
        );

        // The *font size* really changed — that is what makes it crisp rather than a
        // scaled bitmap.
        let big_fs = font_size_at(&page);
        assert!(
            (big_fs - base_fs * 2.0).abs() < 0.01,
            "font_size must scale with zoom"
        );

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
        assert!(
            text.contains("ShadowTitle"),
            "shadow content must render: {text:?}"
        );
        assert!(
            text.contains("SlottedBody"),
            "slotted content must render: {text:?}"
        );

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
        let h_before_repaint = page.content_height;
        assert_eq!(
            page.relayout_incremental(&fonts, 800.0),
            RestyleDamage::Repaint,
            "background-only change is Repaint"
        );
        // The paint-only fast path updated the box's background in place without relayout.
        let mut bg_a = None;
        page.root_box.walk(&mut |b| {
            if b.node == Some(a) {
                bg_a = b.background;
            }
        });
        assert_eq!(
            bg_a,
            Some(Rgba::new(255, 0, 0, 255)),
            "repaint fast path set the background in place"
        );
        assert_eq!(
            page.content_height, h_before_repaint,
            "a repaint-only change does not change geometry"
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

/// **An arena must stop being resolvable before it stops existing.**
///
/// A reflector carries the *address* of the document it belongs to. When a `Page` drops, that address
/// becomes a dangling pointer that a script may still hold — and `Dom::is_alive()` cannot save you,
/// because it validates a node id *within* an arena and the arena itself is what went away. So the
/// registry is emptied first, and `node_and_dom` then sees an unregistered pointer and answers `None`,
/// which is a `null`, which is a correct answer.
impl Drop for Page {
    fn drop(&mut self) {
        for d in self.owned_arenas() {
            manuk_js::unregister_dom(d);
        }
    }
}
