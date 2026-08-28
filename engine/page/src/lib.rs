//! manuk-page — the shared page pipeline.
//!
//! `bytes → DOM → style → layout → paint`, wired end to end. This is the common
//! engine core CLAUDE.md calls for: the **headful shell** and the **headless
//! agent** both drive these functions and diverge only at how they consume the
//! output — the shell presents to a window, the agent screenshots + reads it.

/// Inline-`<svg>` interior geometry — the tick-393 spec's other half. See the module docs.
mod svg_geometry;

/// N1 — the one session-history model, shared by the shell, the agent, and BiDi.
/// Re-export the shared session-history model (moved to `manuk-dom` to break the
/// page↔js dependency cycle). `manuk_page::history::SessionHistory` still resolves.
pub use manuk_dom::history;
/// One step of a streaming response — see [`Page::deliver_fetch_stream`]. Re-exported so the shell
/// can drive the streaming path without depending on `manuk-js` directly.
pub use manuk_js::FetchStreamEvent;
pub use manuk_js::KeyModifiers;
/// What a page's WebSocket asked for, and what happened to it — see [`Page::take_ws_ops`] and
/// [`Page::deliver_ws_event`]. Re-exported so the shell can drive sockets without depending on
/// `manuk-js` directly.
pub use manuk_js::{WsEvent, WsOp};

/// The element a scroll is anchored to, and where it sat relative to the viewport top when it was
/// captured. Produced by [`Page::capture_scroll_anchor`], consumed by [`Page::scroll_anchor_delta`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollAnchor {
    pub node: manuk_dom::NodeId,
    pub offset_from_top: f32,
}

use std::collections::HashMap;

use anyhow::{Context, Result};
use manuk_css::{
    diff_style, MinimalCascade, RestyleDamage, Rgba, StyleEngine, StyleMap, Stylesheet,
};
use manuk_dom::{Dom, NodeId};
use manuk_layout::{layout_document, BoxContent, LayoutBox};
use manuk_net::csp::Csp;
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
        // Stylo's `Device` is what resolves `vw`/`vh` and `@media` on that path, so it must be
        // built from the ICB — the same reference the minimal path's `values::viewport()` uses.
        // `set_viewport_width` above has just reset the ICB width to the window's; a narrower ICB
        // arrives on the SECOND cascade, from `root_scrollbar_icb` below.
        let (icb_w, vh) = manuk_css::values::icb_size();
        let viewport_width = icb_w;
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

/// Re-cascade **without** resetting the ICB — the second pass of the root-scrollbar decision.
///
/// `cascade_styles` calls `set_viewport_width`, which deliberately resets the ICB to the window (a
/// new width is a new page whose root `overflow` has not been read yet). Calling it again for the
/// second pass would therefore throw away the narrowing that pass 1 just computed, and the two
/// cascades would produce identical styles — a re-cascade that costs a cascade and changes nothing
/// is the worst of both, and it would have been invisible: the numbers would simply have stayed
/// wrong.
fn cascade_styles_keeping_icb(dom: &Dom, sheets: &[Stylesheet]) -> StyleMap {
    let (icb_w, _) = manuk_css::values::icb_size();
    let saved = manuk_css::values::icb_size();
    let styles = cascade_styles(dom, sheets, icb_w);
    manuk_css::values::set_icb(saved.0, saved.1);
    styles
}

/// **Does the ROOT ELEMENT reserve a scrollbar gutter, and if so what is the ICB?**
///
/// `Some((w, h))` only when the answer differs from the window — the caller uses that to decide
/// whether a second cascade is owed at all.
///
/// Only the **deterministic** case (`overflow: scroll`, a scrollbar that is always there) is
/// answered. CSS Values 4 is explicit about the other one: *"when the value of `overflow` on the
/// root element is `auto`, any scroll bars are assumed not to exist"* — so `auto` is not residue
/// here, it is the specified behaviour, and treating it as residue would be inventing a gap.
///
/// ⚠ **Only the ROOT ELEMENT, deliberately, and `overflow` PROPAGATION is not implemented here.**
/// CSS Overflow §3.3 propagates the root's `overflow` to the viewport and, when the root is
/// `visible`, propagates `body`'s instead — and the element it came from then uses `visible` for
/// itself. We do neither: a `body { overflow-y: scroll }` page (`ticket.jfa.jp` is one) still
/// reserves the gutter on `body`'s own box, which lands the 15px in almost the same place by a
/// different route and is why the two have never visibly disagreed. Implementing propagation means
/// implementing BOTH halves at once — narrow the ICB *and* stop `body` reserving for itself — or
/// the gutter is counted twice and every such page loses 15px more. That is a tick of its own.
fn root_scrollbar_icb(dom: &Dom, styles: &StyleMap, viewport_width: f32) -> Option<(f32, f32)> {
    let de = dom.children(dom.root()).find(|&c| dom.is_element(c))?;
    let st = styles.get(&de)?;
    // ⭐ `is_viewport: true` — the root's own `overflow` is the initial `visible`, which is not a
    // scroll container, but CSS Overflow §3.3 propagates it to the VIEWPORT, which always is. So
    // `html { scrollbar-gutter: stable }` reserves here even though the element itself scrolls
    // nothing. That single declaration is what the corpus sites write.
    let gy = manuk_layout::gutter_reservation(
        st.overflow_y,
        st.scrollbar_width,
        st.scrollbar_gutter,
        true,
    );
    let gx = manuk_layout::gutter_reservation(
        st.overflow_x,
        st.scrollbar_width,
        st.scrollbar_gutter,
        true,
    );
    if gx == 0.0 && gy == 0.0 {
        return None;
    }
    let (_, vh) = manuk_css::values::viewport_size();
    Some(((viewport_width - gy).max(0.0), (vh - gx).max(0.0)))
}

/// The `@container` re-pass: if any sheet uses container queries, re-cascade against the
/// content-box sizes the pass-1 layout just measured (which is the spec's own model — a
/// container's descendants are styled from the container's laid-out size). Returns whether
/// `styles` was replaced, in which case the caller re-runs layout (and whatever else it
/// re-derives from styles) so the container-gated rules take geometric effect.
///
/// One re-pass per frame, not a fixpoint loop: a container-gated rule can change the
/// container's own size, and browsers converge on exactly this one-re-pass behaviour rather
/// than iterate. Pages without `@container` return immediately — they pay one substring scan.
#[cfg(feature = "stylo")]
fn container_query_recascade(
    dom: &Dom,
    sheets: &[Stylesheet],
    viewport_width: f32,
    styles: &mut StyleMap,
    root_box: &LayoutBox,
    images: &std::collections::HashMap<manuk_dom::NodeId, std::rc::Rc<manuk_paint::DecodedImage>>,
) -> bool {
    if !sheets.iter().any(|s| s.source().contains("@container")) {
        return false;
    }
    let mut sizes: std::collections::HashMap<NodeId, (f32, f32)> = std::collections::HashMap::new();
    collect_content_sizes(root_box, styles, viewport_width, &mut sizes);
    let (_, vh) = manuk_css::values::viewport_size();
    let recascaded = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        manuk_css::stylo_engine::cascade_via_stylo_sized(
            dom,
            sheets,
            viewport_width,
            vh,
            Some(sizes),
        )
    }));
    match recascaded {
        Ok(new_styles) => {
            *styles = new_styles;
            // ⚠⚠⚠ **THE RE-PASS REPLACES THE STYLE MAP WHOLESALE, WHICH ERASES EVERY DECODED
            // IMAGE'S INTRINSIC SIZE — and this is the same rule `restyle_and_layout` states eight
            // lines above its own call: *"BETWEEN the cascade and the layout, every time."*
            //
            // A decoded image's natural size is in no stylesheet, so any rebuilt map has lost it and
            // the picture lays out as a full-width strip of zero height. Every one of the six callers
            // applied `apply_natural_sizes` BEFORE this function and none re-applied it after, so on
            // any page whose CSS contains `@container` the sizes were restated and then immediately
            // thrown away. The single exception is `load_document`, which spotted the hazard and
            // re-ran its own inline-image decode afterwards — *being the only site that was right is
            // how the other five stayed wrong*, which is the shape this file has already named twice
            // (`sheets_of`, `set_root_box`).
            //
            // So it lives INSIDE the re-pass now: the map this function hands back carries the
            // natural sizes by construction, and a seventh caller cannot forget.
            apply_natural_sizes(dom, styles, images);
            true
        }
        Err(_) => {
            tracing::warn!("@container re-pass panicked; keeping the pass-1 styles");
            false
        }
    }
}

#[cfg(not(feature = "stylo"))]
fn container_query_recascade(
    _dom: &Dom,
    _sheets: &[Stylesheet],
    _viewport_width: f32,
    _styles: &mut StyleMap,
    _root_box: &LayoutBox,
    _images: &std::collections::HashMap<manuk_dom::NodeId, std::rc::Rc<manuk_paint::DecodedImage>>,
) -> bool {
    false
}

/// Per-node **content-box** sizes from a laid-out fragment tree — what `@container` conditions
/// are answered against. `LayoutBox.rect` is the border box; the style's border widths and
/// padding (resolved against the containing block's content width, carried down the walk) are
/// subtracted per CSS 2.1 §8.
#[cfg(feature = "stylo")]
fn collect_content_sizes(
    b: &LayoutBox,
    styles: &StyleMap,
    containing_width: f32,
    out: &mut std::collections::HashMap<NodeId, (f32, f32)>,
) {
    let mut content_w = b.rect.width;
    if let Some(n) = b.node {
        if let Some(s) = styles.get(&n) {
            let pl = s.padding.left.resolve(containing_width, 0.0);
            let pr = s.padding.right.resolve(containing_width, 0.0);
            // Vertical padding percentages also resolve against the containing block's WIDTH.
            let pt = s.padding.top.resolve(containing_width, 0.0);
            let pb = s.padding.bottom.resolve(containing_width, 0.0);
            content_w =
                (b.rect.width - s.border_width.left - s.border_width.right - pl - pr).max(0.0);
            let content_h =
                (b.rect.height - s.border_width.top - s.border_width.bottom - pt - pb).max(0.0);
            out.insert(n, (content_w, content_h));
        }
    }
    if let BoxContent::Block(kids) = &b.content {
        for k in kids {
            collect_content_sizes(k, styles, content_w, out);
        }
    }
}

/// Cascade + layout with the `@container` re-pass folded in — the one join every restyle path
/// shares, so a page using container queries gets them on EVERY route to a new box tree, not
/// just first load.
fn restyle_and_layout(
    dom: &Dom,
    sheets: &[Stylesheet],
    fonts: &FontContext,
    viewport_width: f32,
    images: &std::collections::HashMap<manuk_dom::NodeId, std::rc::Rc<manuk_paint::DecodedImage>>,
) -> (StyleMap, LayoutBox) {
    // ⚠ Phase-split for the same reason `forced_reflow` is: t1237 measured ONE forced reflow at
    // 21,169 ms on `bhramarah.in` and this function is all of it. Reported only when it is slow
    // enough for a human to see, so a normal pass costs two `Instant`s and no log line.
    let t_cascade = std::time::Instant::now();
    let mut styles = cascade_styles(dom, sheets, viewport_width);
    // ⚠⚠⚠ **THE ROOT ELEMENT'S SCROLLBAR COMES OUT OF THE INITIAL CONTAINING BLOCK, AND THE ONLY
    // WAY TO KNOW IT IS THERE IS TO HAVE CASCADED FIRST.**
    //
    // `html { overflow: scroll }` reserves a classic scrollbar unconditionally, and CSS Values 4
    // resolves every viewport-percentage unit against the ICB — which the scrollbar is *outside*
    // of. Chrome, 200x100 frame, `html { overflow: scroll }`: `100vw` is **185px** and
    // `window.innerWidth` is still **200**. We answered 185 nowhere and 200 everywhere.
    //
    // It cannot be decided before the cascade — `overflow` is a cascaded property — so this is a
    // second cascade, and it is bought only by the pages that ask for it: a page whose root element
    // does not unconditionally scroll takes the `is_none()` early-out and pays one comparison. The
    // `@container` re-pass below is the same shape and the same justification.
    if let Some((icb_w, icb_h)) = root_scrollbar_icb(dom, &styles, viewport_width) {
        manuk_css::values::set_icb(icb_w, icb_h);
        styles = cascade_styles_keeping_icb(dom, sheets);
    }
    let us_cascade = t_cascade.elapsed().as_micros() as u64;
    // **BETWEEN the cascade and the layout, every time.** See `apply_natural_sizes`: a decoded
    // image's intrinsic size is not in any stylesheet, so a cascade that rebuilds the style map
    // erases it, and the picture becomes a full-width strip of zero height.
    apply_natural_sizes(dom, &mut styles, images);
    let t_layout = std::time::Instant::now();
    let mut root_box = layout_document(dom, &styles, fonts, viewport_width);
    let us_layout = t_layout.elapsed().as_micros() as u64;
    let t_cq = std::time::Instant::now();
    let mut cq_relaid = false;
    if container_query_recascade(dom, sheets, viewport_width, &mut styles, &root_box, images) {
        root_box = layout_document(dom, &styles, fonts, viewport_width);
        cq_relaid = true;
    }
    let us_cq = t_cq.elapsed().as_micros() as u64;
    if (us_cascade + us_layout + us_cq) / 1000 >= 100 {
        tracing::info!(
            cascade_ms = us_cascade / 1000,
            layout_ms = us_layout / 1000,
            container_query_ms = us_cq / 1000,
            cq_relaid,
            n_sheets = sheets.len(),
            nodes = dom.len(),
            "SLOW RESTYLE+LAYOUT"
        );
    }
    (styles, root_box)
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

/// **A `<source type>` this UA can say NO to with confidence** — used by [`Page::pending_media_urls`]
/// to skip past a stream it cannot decode and reach one it can.
///
/// The asymmetry here is deliberate and is the whole design. The question is *not* "do we support
/// this type" — it is "are we **certain** we do not", and only a certain no is acted on. Everything
/// unrecognised is attempted, because the container sniffer and the decoder downstream are the real
/// authorities and a string table that guesses `no` silently breaks streams that would have played.
/// A wrong `no` here is invisible (the video just never loads); a wrong `yes` merely costs a fetch
/// that fails honestly. So the table lists what is genuinely absent and nothing else.
///
/// Kept as string policy in `manuk-page` on purpose: naming `manuk-media`'s types would drag the
/// decoder features — and `openh264`'s C toolchain — into all ~25 gate binaries that link this
/// crate, the isolation tick 236 established and [`Page::set_video_frame`] documents.
fn media_type_rejected(mime: &str) -> bool {
    let t = mime.to_ascii_lowercase();
    // Containers with no demuxer: `re_mp4` reads ISO-BMFF; the symphonia stream seam reads
    // MPEG-audio/FLAC/Ogg (t362-364), so Ogg left this list — only a CERTAIN no is acted on,
    // and an Ogg naming opus is one (no decoder).
    if t.contains("webm") || t.contains("matroska") || t.contains("x-flv") {
        return true;
    }
    // Codecs with no decoder behind the `VideoDecoder`/audio traits. `openh264` is Constrained
    // Baseline H.264, `symphonia` is AAC, `re_rav1d` is AV1 (tick 354 — `av01` left OFF this list
    // the same tick the shell lane gained the decoder) — anything else in a `codecs=` parameter
    // is a certain no.
    ["vp8", "vp9", "vp09", "theora", "opus", "hev1", "hvc1"]
        .iter()
        .any(|c| t.contains(c))
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
        None,
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
    fetch_image_urls_with_raw(urls).await.0
}

/// [`fetch_image_urls`], plus the bytes this crate could NOT decode, returned raw instead of
/// dropped (tick 355). The decoder-isolation rule is why this seam exists: formats whose decoder
/// must not link into the gate binaries (AVIF rides rav1d) are decoded by the SHELL, which merges
/// its results into the same map before [`Page::apply_images_by_url`]. To this crate an AVIF is
/// honestly undecodable; to the browser it is not — the raw channel is how both stay true.
pub async fn fetch_image_urls_with_raw(
    urls: Vec<String>,
) -> (
    std::collections::HashMap<String, manuk_paint::DecodedImage>,
    Vec<(String, Vec<u8>)>,
) {
    let fetched = futures_util::future::join_all(
        urls.into_iter()
            .map(|url| async move { (url.clone(), fetch_image_bytes(&url).await, url) }),
    )
    .await;
    let mut out = std::collections::HashMap::new();
    let mut raw = Vec::new();
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
        match decoded {
            Some(img) if img.width > 0 && img.height > 0 => {
                out.insert(key, img);
            }
            _ => raw.push((key, bytes.to_vec())),
        }
    }
    (out, raw)
}

/// **Drive a fan-out of fetches against a DEADLINE and return whatever completed** — the partial
/// result, not the empty one.
///
/// `join_all` is a single future that yields a single vector when the *last* member settles. Under a
/// cancelling deadline that makes a fan-out all-or-nothing: one host that accepts the connection and
/// never answers discards every response that had already arrived, decoded, in memory, paid for. The
/// phase does not lose the work it had not done yet — it loses the work it had.
///
/// That is not what the budget promises. `finish_loading` says *"whatever has arrived is what the page
/// gets"*, and this is the primitive that makes the sentence true: a `FuturesUnordered` polled item by
/// item, so each answer is **banked as it lands**, and the deadline ends the waiting rather than the
/// results.
///
/// `None` is the un-budgeted case and behaves exactly as `join_all` did: run to completion.
async fn collect_before_deadline<F, T>(
    futs: impl IntoIterator<Item = F>,
    deadline: Option<std::time::Instant>,
) -> Vec<T>
where
    F: std::future::Future<Output = T>,
{
    use futures_util::stream::StreamExt;
    let mut pending: futures_util::stream::FuturesUnordered<F> = futs.into_iter().collect();
    let mut out = Vec::with_capacity(pending.len());
    let Some(deadline) = deadline else {
        while let Some(v) = pending.next().await {
            out.push(v);
        }
        return out;
    };
    let deadline = tokio::time::Instant::from_std(deadline);
    let wanted = pending.len();
    loop {
        match tokio::time::timeout_at(deadline, pending.next()).await {
            Ok(Some(v)) => out.push(v),
            // Every member settled with time to spare — the ordinary case.
            Ok(None) => break,
            Err(_) => {
                tracing::debug!(
                    got = out.len(),
                    of = wanted,
                    "load budget reached mid fan-out — keeping what arrived"
                );
                break;
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
    deadline: Option<std::time::Instant>,
) -> (
    std::collections::HashMap<manuk_dom::NodeId, std::rc::Rc<manuk_paint::DecodedImage>>,
    Vec<(manuk_dom::NodeId, String)>,
) {
    let known: std::collections::HashSet<String> = cache.keys().cloned().collect();
    let (by_url, tried) = fetch_images_owned(dom, base, attempted, &known, deadline).await;

    // Fold the answers into the page's URL cache. **A URL we asked for and did not get back is a
    // FAILURE, and it is recorded as `None`.** Remembering the "no" is the difference between one
    // fetch and one fetch per render round forever — that was the nytimes storm (507 duplicates
    // of 813 fetches), and it is what G_DEDUP exists to keep dead.
    //
    // ⚠ `tried` now contains only the URLs that actually SETTLED. A request the deadline interrupted
    // did not answer "no"; nobody asked it to finish. Recording it here as a remembered failure would
    // turn a slow host into a permanently blank image for the rest of the navigation — the deadline
    // deciding a capability question it was never asked. Those URLs stay absent from the cache, so
    // the next round asks again.
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
    deadline: Option<std::time::Instant>,
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
    //
    // **Against the load deadline, not to completion.** This was `join_all`, and `join_all` yields its
    // vector only when the LAST fetch settles — so under `finish_loading`'s cancelling deadline a
    // single stalled image host discarded every image that had already arrived. On a page whose
    // pictures are the content that is not a lost enhancement; it is the page. See
    // [`collect_before_deadline`].
    let fetched = collect_before_deadline(
        wanted
            .into_iter()
            .map(|url| async move { (url.clone(), fetch_image_bytes(&url).await, url) }),
        deadline,
    )
    .await;
    // Only the URLs that actually answered — see `fetch_images_except`, which turns this into the
    // "we already asked" record. A fetch the deadline interrupted must not be remembered as a failure.
    let settled: std::collections::HashSet<String> =
        fetched.iter().map(|(k, _, _)| k.clone()).collect();
    let tried: Vec<(manuk_dom::NodeId, String)> = targets
        .into_iter()
        .filter(|(_, url)| known_urls.contains(url) || settled.contains(url))
        .collect();
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
    deadline: Option<std::time::Instant>,
) -> (
    HashMap<String, manuk_paint::DecodedImage>,
    std::collections::HashSet<String>,
) {
    let fetched = collect_before_deadline(
        targets
            .into_iter()
            .map(|(raw, abs)| async move { (raw, fetch_image_bytes(&abs).await, abs) }),
        deadline,
    )
    .await;
    // The second half is the URLs that ACTUALLY ANSWERED — success or honest failure. The caller
    // records those as asked-and-answered; a request the deadline interrupted is not one of them, and
    // marking it would retire an icon nobody ever declined to send.
    let settled = fetched.iter().map(|(raw, _, _)| raw.clone()).collect();
    let decoded = fetched
        .into_iter()
        .filter_map(|(raw, bytes, abs)| Some((raw, decode_bitmap(&bytes?, &abs)?)))
        .collect();
    (decoded, settled)
}

/// **A SPATIAL MEDIA FRAGMENT ON AN SVG URL — `#xywh=` (Media Fragments URI 1.0 §4.2, wired to
/// SVG2 Linking §"Linking into SVG content").**
///
/// `url("sprite.svg#xywh=pixel:100,100,100,100")` does not merely scroll the image: the selected
/// region **becomes** the image, natural size and all. That is what makes an SVG sprite sheet work —
/// one file, one cell drawn 1:1 in a box the size of the cell — and it is the shape 19 of the 26
/// `svg/linking` WPT reftests take.
///
/// Returns the region in the SVG's own pixel space, or `None` to mean *"no usable fragment — use
/// the whole image"*. Grammar, measured against the WPT battery rather than inferred:
///
/// ```text
///   #xywh=100,100,100,100                 pixel is the DEFAULT unit
///   #xywh=pixel:100,100,100,100           …and may be named
///   #xywh=percent:50,50,50,50             …or given as a % of the natural size
///   #xywh=0,0,200,200   on a 100x100      CLAMPED to the image, not rejected
///   #xywh=100,100,-100,100                INVALID (w<=0) -> ignored, whole image
///   #xywh=0,0,100,100&xywh=100,100,100,100    `&`-separated; the LAST VALID one wins
/// ```
///
/// ⚠⚠ **"Invalid" and "out of range" are different answers, and the battery separates them.** A
/// non-positive width is a *syntax* failure and the fragment is discarded (the whole image is used);
/// a region that merely extends past the edge is *clamped*. Treating the second as the first renders
/// the whole sheet where Chrome renders one clamped cell, and both cases are one line apart.
///
/// ⚠ Only the **spatial** dimension is handled. `#t=` (temporal), `#track=` and `#id=` are not
/// spatial selectors and are deliberately ignored here rather than guessed at.
fn svg_media_fragment_rect(url: &str, nat_w: f32, nat_h: f32) -> Option<(f32, f32, f32, f32)> {
    let frag = url.split_once('#')?.1;
    if nat_w <= 0.0 || nat_h <= 0.0 {
        return None;
    }
    // `&`-separated name/value pairs; the LAST syntactically valid `xywh` wins, and an invalid one
    // does not invalidate an earlier valid one — it is simply not a candidate.
    let mut best = None;
    for pair in frag.split('&') {
        let Some(v) = pair.strip_prefix("xywh=") else {
            continue;
        };
        let (percent, nums) = match v.split_once(':') {
            Some(("percent", rest)) => (true, rest),
            Some(("pixel", rest)) => (false, rest),
            // An unknown unit prefix is not a pixel value with a colon in it — it is invalid.
            Some(_) => continue,
            None => (false, v),
        };
        let parts: Vec<&str> = nums.split(',').collect();
        if parts.len() != 4 {
            continue;
        }
        let Ok(vals) = parts
            .iter()
            .map(|p| p.trim().parse::<f32>())
            .collect::<Result<Vec<f32>, _>>()
        else {
            continue;
        };
        if vals.iter().any(|v| !v.is_finite()) {
            continue;
        }
        let (mut x, mut y, mut w, mut h) = (vals[0], vals[1], vals[2], vals[3]);
        if percent {
            x = x / 100.0 * nat_w;
            y = y / 100.0 * nat_h;
            w = w / 100.0 * nat_w;
            h = h / 100.0 * nat_h;
        }
        // A non-positive extent is invalid — discarded, NOT clamped.
        if w <= 0.0 || h <= 0.0 {
            continue;
        }
        best = Some((x, y, w, h));
    }
    let (x, y, w, h) = best?;
    // …whereas overflowing the image IS clamped.
    let x0 = x.max(0.0).min(nat_w);
    let y0 = y.max(0.0).min(nat_h);
    let x1 = (x + w).max(0.0).min(nat_w);
    let y1 = (y + h).max(0.0).min(nat_h);
    if x1 - x0 <= 0.0 || y1 - y0 <= 0.0 {
        return None;
    }
    Some((x0, y0, x1 - x0, y1 - y0))
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
    let (nat_w, nat_h) = (size.width(), size.height());
    // ⚠⚠⚠ **THE URL'S FRAGMENT SELECTS A REGION OF THE IMAGE, AND IT ARRIVES HERE ALREADY** —
    // `decode_svg` has taken the URL since it was written, to sniff `.svg`. A spatial media fragment
    // (`#xywh=`) makes the SELECTED REGION the image: its natural size becomes the region's, which
    // is why `sprite.svg#xywh=pixel:100,100,100,100` in a 100x100 box is a 1:1 draw of one sprite
    // cell rather than a corner of the whole sheet. See [`svg_media_fragment_rect`].
    let (crop_x, crop_y, w, h) = match svg_media_fragment_rect(url, nat_w, nat_h) {
        Some((x, y, cw, ch)) => (x, y, cw.ceil() as u32, ch.ceil() as u32),
        None => (0.0, 0.0, nat_w.ceil() as u32, nat_h.ceil() as u32),
    };
    if w == 0 || h == 0 || w > 8192 || h > 8192 {
        return None;
    }
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_translate(-crop_x, -crop_y),
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

/// The bytes of a `data:` image URL. **Synchronous, because there is nothing to wait for** — the
/// payload is already in the string. That is the whole point of the split: an inline image needs no
/// network, so it can be decoded during load rather than in the async subresource pass, and a page
/// that never reaches that pass (a gate, the WPT runner, `Page::load`) still gets its size.
fn data_url_image_bytes(url: &str) -> Option<Vec<u8>> {
    let rest = url.strip_prefix("data:")?;
    let comma = rest.find(',')?;
    let data = &rest[comma + 1..];
    if rest[..comma].contains("base64") {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.decode(data).ok()
    } else {
        // Non-base64 data URLs are percent-encoded (e.g. `%23` for `#` in inline SVG).
        Some(percent_decode(data))
    }
}

/// Apply a decoded image's **natural size** to the element's computed style.
///
/// The *aspect ratio* is the load-bearing part: an `auto` dimension of a replaced element is derived
/// from the USED value of the other one, not from the image's natural pixels. Pinning `height` to the
/// natural height means a `max-width: 100%` clamp narrows the box and leaves the height alone, and
/// the image renders stretched — a reset that is on essentially every site on the web.
///
/// So: record the ratio, give `width` its natural value only when BOTH axes are auto (the
/// unconstrained case), and otherwise leave the auto axis auto for layout to derive.
///
/// Shared by the async subresource pass and the inline-`data:` pass so the two cannot drift into
/// sizing the same image two different ways depending on how its bytes arrived.
/// **Re-state every decoded image's intrinsic size into a freshly cascaded style map.**
///
/// An image's natural size is the one geometry input that is **not in any stylesheet** — it arrives
/// from the network, long after the cascade that will be asked to lay it out. So it was written
/// straight into the cascade's *output*, once, by `apply_images` — and every later cascade rebuilt
/// that map from the stylesheets and erased it.
///
/// Measured, on a page with no CSS at all and one 41×23 image:
///
/// ```text
///   after load_async (image applied)     width=Px(41)  ar=Some(1.78)  rect 41×23
///   after finish_loading (re-cascaded)   width=Auto    ar=None        rect 784×0
/// ```
///
/// 784×0 is the full content width and **no height** — the picture occupies no space at all and
/// everything below it slides up. It never recovered, either: the image phase's own dedup means the
/// second pass has nothing to re-apply, so the size was applied exactly once and any cascade after it
/// was final. A page whose images are its content laid out as a stack of zero-height strips is the
/// picture-shaped-hole signature the fidelity sweep records.
///
/// Which is why this runs *inside* [`restyle_and_layout`] — the one join every restyle path shares —
/// rather than at the call sites. The intrinsic size is a **standing** input to layout, not an event
/// that happened once.
/// ⚠⚠ **An inline `<svg>` is EXCLUDED, and the exclusion is the whole point of taking `dom` here.**
///
/// The inline-svg raster cache is merged into `self.images` so the painter can find it — and this
/// pass reads that same map, so a `<svg viewBox="0 0 100 25">` with no width/height silently
/// acquired an **intrinsic size of 100×25**: usvg's `Tree::size()` falls back to the viewBox when
/// the dimension attributes are absent. That is exactly the sizing input CSS says an outermost svg
/// does **not** have. `viewBox` is an intrinsic **RATIO**, never a size (SVG2 §8.2 + CSS-Images
/// §5.3.2 default sizing), so `width:auto` must fill the containing block and the height follow the
/// ratio — Chrome-measured 400×100 in a 400px block, 250×63 in a 250px one. We rendered 100×25 in
/// both, i.e. the icon-sized-by-its-own-coordinates bug, and every child `<path>`/`<g>`/`<rect>`
/// inherited the wrong scale while the page below it slid up by the missing height.
///
/// The size half of `apply_natural_size` is the harm; the ratio half is redundant here (the cascade
/// already derives the same ratio from `viewBox`), so the honest exclusion is the whole call.
fn apply_natural_sizes(
    dom: &Dom,
    styles: &mut StyleMap,
    images: &std::collections::HashMap<manuk_dom::NodeId, std::rc::Rc<manuk_paint::DecodedImage>>,
) {
    for (node, img) in images {
        if dom.tag_name(*node) == Some("svg") {
            continue;
        }
        if let Some(style) = styles.get_mut(node) {
            apply_natural_size(style, img);
        }
    }
}

/// A decoded bitmap's own pixel size, through the one producer that also marks the axes it filled —
/// `manuk_css::fill_natural_size`. A `<canvas>`'s dimension attributes reach the same function from
/// the cascade; see there for why the marks exist and why a ratio leaves the height `auto`.
fn apply_natural_size(style: &mut manuk_css::ComputedStyle, img: &manuk_paint::DecodedImage) {
    manuk_css::fill_natural_size(style, img.width as f32, img.height as f32);
}

/// Decode every `<img src="data:...">` in the tree and give it its natural size, **before the first
/// layout**. Inline images carry their own bytes, so unlike a network image there is nothing to wait
/// for and no reason to make the page reflow later.
///
/// Without this an inline image laid out as `0x0` on every path that does not run the async
/// subresource pass — which is every gate, the WPT runner, and `Page::load` itself.
fn decode_inline_images(
    dom: &Dom,
    styles: &mut StyleMap,
) -> std::collections::HashMap<manuk_dom::NodeId, std::rc::Rc<manuk_paint::DecodedImage>> {
    let mut out = std::collections::HashMap::new();
    // One decode per distinct URL: a sprite or icon repeated across the page costs one.
    let mut by_url: std::collections::HashMap<
        String,
        Option<std::rc::Rc<manuk_paint::DecodedImage>>,
    > = std::collections::HashMap::new();
    for node in dom.descendants(dom.root()) {
        // **The source attribute is chosen exactly as the async pass chooses it** (`<img src>` /
        // `<video poster>`), because the two passes decode the same elements for the same reason and
        // had already drifted: this one matched `img` only, so a NETWORK poster rendered and an
        // INLINE `data:` poster silently did not — on `Page::load`, every gate and the WPT runner.
        // Found by G_VIDEO_FRAME's baseline assertion (tick 240), which is the whole argument for
        // asserting the thing you are about to build on top of rather than assuming it.
        let Some(src) = dom.element(node).and_then(|e| match dom.tag_name(node) {
            Some("img") => e.attr("src"),
            Some("video") => e.attr("poster"),
            _ => None,
        }) else {
            continue;
        };
        if !src.starts_with("data:") {
            continue;
        }
        let src = src.to_string();
        let decoded = by_url
            .entry(src.clone())
            .or_insert_with(|| {
                data_url_image_bytes(&src)
                    .and_then(|bytes| decode_bitmap(&bytes, &src))
                    .map(std::rc::Rc::new)
            })
            .clone();
        if let Some(img) = decoded {
            if let Some(style) = styles.get_mut(&node) {
                apply_natural_size(style, &img);
            }
            out.insert(node, img);
        }
    }
    out
}

/// Fetch the raw bytes of an image URL: `data:` (base64 or literal), `http(s)://`, or a
/// local `file://`/path (for the render CLI on local pages).
async fn fetch_image_bytes(url: &str) -> Option<Vec<u8>> {
    if url.starts_with("data:") {
        return data_url_image_bytes(url);
    }
    if url.starts_with("http://") || url.starts_with("https://") {
        let resp = manuk_net::fetch(url).await.ok()?;
        if resp.status >= 400 {
            return None;
        }
        return Some(resp.body.to_vec());
    }
    // ⚠⚠⚠ **A FRAGMENT IS NOT PART OF THE PATH, AND LEAVING IT ON MEANT THE IMAGE DID NOT LOAD AT
    // ALL.** `url("icons.svg#icon-home")` — the SVG sprite idiom, a `<view>` reference, and every
    // `#xywh=` media fragment — reached `std::fs::read` as the literal filename
    // `icons.svg#icon-home`, which does not exist. Not a wrong crop: **no image**. The fragment
    // identifies a part of the resource *after* it is retrieved (RFC 3986 §3.5) and is never sent
    // to the transport; `decode_svg` reads it back off the original URL, which is why it is stripped
    // here and not earlier.
    let path = url.split('#').next().unwrap_or(url);
    let path = path.strip_prefix("file://").unwrap_or(path);
    std::fs::read(path).ok()
}

/// **Fetch one media resource whole**, for a `<video>`/`<audio>` named by
/// [`Page::pending_media_urls`]. Same transport handling as every other subresource
/// (`data:`/`http(s):`/`file:`), which is why it delegates rather than reimplementing.
///
/// **Whole, not ranged, and that is a stated limitation rather than an oversight.** Real video
/// delivery is `Range` requests against a progressively-buffered file — that is what makes a
/// two-hour film start in a second and is the machinery MSE's `SourceBuffer` exists to feed. This
/// buffers the entire resource before a single frame decodes, which is correct for the short files
/// the pipeline can currently play and would be an OOM on a feature-length one. The demuxer already
/// reports `DemuxError::Incomplete` for a partial buffer, so the seam for ranged fetching is open;
/// nothing here needs to change shape when it lands.
pub async fn fetch_media_bytes(url: &str) -> Option<Vec<u8>> {
    fetch_image_bytes(url).await
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
/// The viewport width `collect_subresources` resolves `sizes`/`media` against.
///
/// It runs before layout, so there is no real viewport yet — and picking one candidate step wrong is
/// far cheaper than fetching the `src` thumbnail, which is what happened before. `pending_image_urls`
/// uses the page's actual width once it is known.
const DEFAULT_SELECTION_VIEWPORT: f32 = 800.0;

/// The image MIME types this build can actually decode.
///
/// `image` is built with `["png","jpeg","gif","webp","bmp","ico"]` and **AVIF is deliberately off** —
/// its decoder is C dav1d, declined on purpose. SVG is here because a raster-decode failure falls
/// through to `decode_svg` (usvg/resvg).
///
/// This list is what makes `<picture>`'s `type` attribute worth honouring rather than a decoration:
/// a site that ships `<source type="image/avif">` with a JPEG `<img>` fallback is telling us, in
/// advance, which candidate we can use. Choosing the AVIF would render **nothing** — strictly worse
/// than ignoring `srcset` altogether, which is why the skip is asserted before the pick.
fn can_decode_image_type(mime: &str) -> bool {
    matches!(
        mime.trim().to_ascii_lowercase().as_str(),
        "image/png"
            | "image/jpeg"
            | "image/jpg"
            | "image/gif"
            | "image/webp"
            | "image/bmp"
            | "image/x-icon"
            | "image/vnd.microsoft.icon"
            | "image/svg+xml"
    )
}

/// One `srcset` candidate: a URL and its descriptor.
struct Candidate<'a> {
    url: &'a str,
    /// `Nw` — the candidate's intrinsic width in px.
    width: Option<f32>,
    /// `Nx` — the candidate's pixel density. Defaults to 1 when no descriptor is given.
    density: f32,
}

/// Parse a `srcset` attribute into candidates — **HTML's own two-phase scan, because a comma is a
/// legal URL character and the naive split shreds 10% of the corpus's `srcset` sites** (t1046).
///
/// The comment this replaces read: *"a comma inside a URL is vanishingly rare compared to a missing
/// space after one, so candidates are split on commas"*. **Measured against the burndown corpus, that
/// premise is false**: 40 of 170 pages ship a `srcset`, and **4 of them carry a comma inside a
/// candidate URL** — one site in ten that uses the attribute at all, and `www.kuechenmomente.de` is
/// one the loop already tracks. Two shapes produce it and both are ordinary:
///
/// ```text
///   an image CDN's transform segment   /upload/w_400,h_300,c_fill/hero.jpg 400w
///   any `data:` URI at all             data:image/png;base64,iVBORw0K… 1x
/// ```
///
/// A shredded candidate is not a smaller image — the fragments are not URLs, so the list yields
/// nothing and **the element renders a broken-image placeholder**.
///
/// The spec's rule is a two-phase scan, and it is what makes a comma unambiguous: **a URL runs to
/// the next whitespace** (so an interior comma is simply part of it), and only a *trailing* comma
/// ends the candidate; the **descriptor then runs to the next comma**. Leniency is unchanged where
/// it was right: a malformed descriptor drops that candidate rather than the whole list, because
/// dropping the list is how this attribute silently costs a page its images.
fn parse_srcset(srcset: &str) -> Vec<Candidate<'_>> {
    let mut out = Vec::new();
    let b = srcset.as_bytes();
    let n = b.len();
    let ws = |c: u8| c.is_ascii_whitespace();
    let mut i = 0usize;
    while i < n {
        // Between candidates: any run of whitespace and commas.
        while i < n && (ws(b[i]) || b[i] == b',') {
            i += 1;
        }
        if i >= n {
            break;
        }
        // ── PHASE 1: the URL runs to the next WHITESPACE. An interior comma belongs to it.
        let start = i;
        while i < n && !ws(b[i]) {
            i += 1;
        }
        let raw = &srcset[start..i];
        // …and only a TRAILING comma ends the candidate, which is the spec's signal for "no
        // descriptor". `a.png,b.png` stays one URL, exactly as Chrome treats it.
        // ⚠ `trim_end_matches` and `trim_matches` are provably equivalent here — a LEADING comma
        // can never reach this line, because the inter-candidate skip above consumes commas before
        // the URL scan starts. Stated rather than left as a mutation that cannot go red: the
        // end-only form is the one that says what the spec means.
        let url = raw.trim_end_matches(',');
        let ended_by_comma = url.len() != raw.len();
        if url.is_empty() {
            continue;
        }
        let mut width = None;
        let mut density = 1.0f32;
        if !ended_by_comma {
            // ── PHASE 2: the DESCRIPTOR LIST, which runs to the next comma **outside parentheses**
            //    and is then split on whitespace.
            //
            // ⚠⚠ **The parenthesis rule is not decoration — it decides where the next CANDIDATE
            // starts.** `data:,a ( , data:,b 1x, ), data:,c` has exactly two candidates, because
            // every comma between `(` and `)` is part of one descriptor token; splitting at the
            // first comma instead reads `data:,b` as a candidate and picks it. An unclosed `(`
            // swallows the rest of the attribute, which is why `data:,a (, data:,b` yields NOTHING.
            let dstart = i;
            let mut in_parens = false;
            while i < n {
                match b[i] {
                    b'(' => in_parens = true,
                    b')' => in_parens = false,
                    b',' if !in_parens => break,
                    _ => {}
                }
                i += 1;
            }
            let desc = &srcset[dstart..i];

            // ⚠⚠⚠ **EVERY DESCRIPTOR IS CHECKED, NOT JUST THE FIRST — AND THE OLD LENIENCY WAS
            // SCORING ON AN ACCIDENT.** This used to read `desc.split_ascii_whitespace().next()`
            // and ignore the rest, so `1x 1x`, `1w 1w`, `1w 1x`, `0w` and `-1w` were all accepted
            // as valid candidates. WPT expects every one of them to produce NO image at all. Those
            // rows *passed* before this tick only because `currentSrc` returned the empty string
            // for everything, which is the same empty string an invalid list is supposed to
            // produce — a wrong mechanism agreeing with the right answer. Publishing the real
            // selection removed the accident and exposed the parser underneath it.
            //
            // The rules are HTML's "parse a srcset attribute" verbatim: at most one of `w`/`x`,
            // never both, `h` only alongside a `w`, values strictly positive, and ANY unrecognised
            // descriptor is an error. An error drops THIS candidate, never the whole list.
            let mut height: Option<f32> = None;
            let mut density_seen = false;
            let mut bad = false;
            for tok in desc.split_ascii_whitespace() {
                let Some((num, unit)) = tok.split_at_checked(tok.len().saturating_sub(1)) else {
                    bad = true;
                    break;
                };
                match unit {
                    "w" => {
                        // A width is an integer; `1.5w` is not one, and neither is `0w` or `-1w`.
                        match num.parse::<u32>() {
                            Ok(v) if v > 0 && width.is_none() && !density_seen => {
                                width = Some(v as f32)
                            }
                            _ => bad = true,
                        }
                    }
                    "x" => match num.parse::<f32>() {
                        Ok(v)
                            if v > 0.0 && !density_seen && width.is_none() && height.is_none() =>
                        {
                            density = v;
                            density_seen = true;
                        }
                        _ => bad = true,
                    },
                    "h" => match num.parse::<u32>() {
                        Ok(v) if v > 0 && height.is_none() && !density_seen => {
                            height = Some(v as f32)
                        }
                        _ => bad = true,
                    },
                    _ => bad = true,
                }
                if bad {
                    break;
                }
            }
            // A `h` descriptor is meaningless on its own — it exists to pair with a `w`.
            if height.is_some() && width.is_none() {
                bad = true;
            }
            if bad {
                continue; // an invalid descriptor list drops the candidate, not the list
            }
        }
        out.push(Candidate {
            url,
            width,
            density,
        });
    }
    out
}

/// The slot width a `w`-descriptor list is measured against — HTML's *"parse a sizes attribute"*.
///
/// A `sizes` attribute is a comma-separated list of `<source-size>`, each an optional
/// `<media-condition>` followed by a `<source-size-value>`, and **the FIRST entry whose condition
/// matches wins**. The last entry may omit its condition, which is how the unconditional fallback is
/// written.
///
/// ⚠⚠⚠ **THIS USED TO READ THE LAST ENTRY, WHICH IS THE OPPOSITE END OF THE LIST.** The old version
/// was eleven lines and said so — *"we take the LAST entry's length, which is that unconditional
/// fallback… one candidate step off at worst"* — a bounded, honestly-labelled approximation that was
/// right to ship at the time. It is wrong in the common case, and the cost is not one candidate
/// step: `sizes="(min-width:0) 1px, 100vw"` is a **1px** slot and we answered with the **whole
/// viewport**, which is the difference between the smallest file and the largest. `sizes` is how a
/// page tells the browser how big the image will BE, so the wrong answer here is a wrong intrinsic
/// size, and a wrong intrinsic size re-wraps everything below it.
///
/// The fallback when nothing parses is `100vw`, which is also what the spec's *"parse error"* path
/// leaves in place — and it is what an author who wrote no `sizes` at all gets.
fn sizes_slot_width(sizes: Option<&str>, viewport_width: f32) -> f32 {
    let Some(sizes) = sizes else {
        return viewport_width;
    };
    for entry in split_top_level_commas(sizes) {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        // The VALUE is the last balanced component value; everything before it is the media
        // condition. Scanning from the right is what keeps `calc(1px + 2px)` — which contains
        // whitespace *inside* parentheses — from being read as two tokens.
        let Some((cond, value)) = split_trailing_component(entry) else {
            continue;
        };
        // ⚠ `media_condition_matches`, NOT `media_matches`: a `<source-size>` takes a
        // `<media-condition>`, which **cannot contain a media type**. `sizes="not print 100vw, 1px"`
        // is therefore a 1px slot — the first entry is a grammar error and is discarded — where the
        // same text after `@media` is a query that matches on screen. Asking the query production
        // answered `100vw` and fetched a different bitmap.
        if !cond.trim().is_empty() && !manuk_css::media_condition_matches(cond.trim()) {
            continue;
        }
        // A negative slot is not a parse error the spec forgives — it is an invalid
        // `<source-size-value>`, and the entry is skipped rather than clamped to zero.
        if let Some(px) = resolve_size_value(value, viewport_width) {
            if px >= 0.0 {
                return px;
            }
        }
    }
    viewport_width
}

/// Split on commas that are **outside** any `()`/`[]`/`{}` nesting.
///
/// Every list in this file needs the same rule and for the same reason: a comma inside a
/// `min(1px, 200vw)` or a `(min-width: 0)` belongs to that construct, not to the list. Splitting
/// naively turns one entry into two, and the second half is usually a syntactically valid-looking
/// fragment that resolves to something plausible — which is the failure mode that hides.
fn split_top_level_commas(s: &str) -> Vec<&str> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let (mut depth, mut start) = (0i32, 0usize);
    for i in 0..b.len() {
        match b[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b',' if depth <= 0 => {
                out.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out
}

/// Split `<media-condition>? <source-size-value>` into its two halves.
///
/// Returns `None` when the entry has unbalanced brackets — an unclosed `(` makes the whole
/// `<source-size>` a parse error, and guessing at the author's intent is how `x(x(),1px` would
/// otherwise resolve to a real width.
fn split_trailing_component(entry: &str) -> Option<(&str, &str)> {
    let b = entry.as_bytes();
    let mut depth = 0i32;
    // Walk right-to-left; the value begins at the first top-level whitespace we cross.
    let mut split_at = 0usize;
    for i in (0..b.len()).rev() {
        match b[i] {
            b')' | b']' | b'}' => depth += 1,
            b'(' | b'[' | b'{' => {
                depth -= 1;
                if depth < 0 {
                    return None; // an unclosed opener — the entry is a parse error
                }
            }
            c if c.is_ascii_whitespace() && depth == 0 => {
                split_at = i;
                break;
            }
            _ => {}
        }
    }
    if depth != 0 {
        return None;
    }
    Some((&entry[..split_at], entry[split_at..].trim()))
}

/// A `<source-size-value>` in CSS pixels, or `None` if it is not one.
///
/// The unit table is the whole absolute/relative set rather than the `px`/`vw` pair the old code
/// knew, because WPT tests every one of them and a page may legitimately write `sizes: 20em`. The
/// font-relative units resolve against the initial 16px font — this runs before layout and has no
/// element to ask, which is a real approximation and is why it is stated here rather than implied.
fn resolve_size_value(v: &str, vw: f32) -> Option<f32> {
    let v = v.trim();
    if v.is_empty() {
        return None;
    }
    // Math functions, which are the reason this is recursive at all.
    let lower = v.to_ascii_lowercase();
    for (name, n) in [("calc(", 4usize), ("min(", 3), ("max(", 3), ("clamp(", 5)] {
        if let Some(rest) = lower.strip_prefix(name) {
            if !rest.ends_with(')') {
                return None; // `min(1px, 200vw` — unclosed, and a parse error rather than a guess
            }
            let inner = &v[n + 1..v.len() - 1];
            let args: Vec<f32> = split_top_level_commas(inner)
                .iter()
                .map(|a| resolve_size_value(a, vw))
                .collect::<Option<Vec<f32>>>()?;
            return match (name, args.len()) {
                ("calc(", 1) => Some(args[0]),
                ("min(", n) if n > 0 => args.iter().copied().reduce(f32::min),
                ("max(", n) if n > 0 => args.iter().copied().reduce(f32::max),
                // `clamp(min, val, max)` — and note the spec's order of operations makes
                // `clamp(1px, -50px, 100px)` resolve to **1px**, not to the negative middle.
                ("clamp(", 3) => Some(args[1].clamp(args[0], args[2])),
                _ => None,
            };
        }
    }
    // A bare `0` is a length; every other unitless number is not.
    if let Ok(n) = v.parse::<f32>() {
        return if n == 0.0 { Some(0.0) } else { None };
    }
    // Longest suffix first, or `vmin` matches `in` and `1vmin` becomes 96 pixels.
    const UNITS: &[(&str, f32)] = &[
        ("vmin", 0.0), // viewport-relative: filled in below
        ("vmax", 0.0),
        ("rem", 16.0),
        ("px", 1.0),
        ("em", 16.0),
        ("ex", 8.0),
        ("ch", 8.0),
        ("cm", 96.0 / 2.54),
        ("mm", 9.6 / 2.54),
        ("in", 96.0),
        ("pt", 96.0 / 72.0),
        ("pc", 16.0),
        ("vw", 0.0),
        ("vh", 0.0),
        ("q", 96.0 / 2.54 / 40.0),
    ];
    for (unit, factor) in UNITS {
        let Some(num) = lower.strip_suffix(unit) else {
            continue;
        };
        let n: f32 = num.trim().parse().ok()?;
        // ⚠ There is no second viewport axis here: `sizes` is resolved against the viewport WIDTH
        // for every viewport unit, because that is the only dimension the selection is about and
        // the height is not known at this point in the load.
        return Some(match *unit {
            "vw" | "vh" | "vmin" | "vmax" => vw * n / 100.0,
            _ => n * factor,
        });
    }
    None
}

/// Choose one candidate from a `srcset`, or `None` if the list yields nothing usable.
///
/// `w` descriptors win over `x` when both appear (the spec ignores `x` once any `w` is present).
/// Either way the rule is *the smallest candidate that still covers the requirement*, falling back to
/// the largest when none does — never the first listed, which is the cheap wrong answer.
fn pick_candidate<'a>(cands: &[Candidate<'a>], slot_px: f32, dpr: f32) -> Option<&'a str> {
    if cands.is_empty() {
        return None;
    }
    let any_w = cands.iter().any(|c| c.width.is_some());
    if any_w {
        let need = slot_px * dpr;
        let mut best: Option<&Candidate> = None;
        for c in cands.iter().filter(|c| c.width.is_some()) {
            let w = c.width.unwrap_or(0.0);
            let better = match best {
                None => true,
                Some(b) => {
                    let bw = b.width.unwrap_or(0.0);
                    if bw < need {
                        w > bw
                    } else {
                        w >= need && w < bw
                    }
                }
            };
            if better {
                best = Some(c);
            }
        }
        return best.map(|c| c.url);
    }
    let mut best: Option<&Candidate> = None;
    for c in cands {
        let better = match best {
            None => true,
            Some(b) => {
                if b.density < dpr {
                    c.density > b.density
                } else {
                    c.density >= dpr && c.density < b.density
                }
            }
        };
        if better {
            best = Some(c);
        }
    }
    best.map(|c| c.url)
}

/// **The image an `<img>` will actually load** — `<picture>` sources, then its own `srcset`, then `src`.
///
/// One function because there are two callers (`collect_subresources`, the fetch worklist, and
/// `pending_image_urls`, the decode worklist) and they must not disagree about which URL the element
/// wants. Two independent selections is the two-cascades trap in a different organ.
///
/// Measured before it was written (tick 582): every candidate list was ignored and the `src` fetched
/// every time. On a `w`-descriptor list — which is what WordPress, every CMS and every image CDN emit
/// — the `src` is frequently the *smallest* candidate, so this was not "2x displays get 1x images",
/// it was a thumbnail scaled across a hero. And `<img srcset>` with no `src` at all is legal, where we
/// requested nothing and rendered an empty box.
fn select_image_url(dom: &Dom, node: NodeId, viewport_width: f32) -> Option<String> {
    let dpr = 1.0f32;
    let el = dom.element(node)?;
    // `<picture>`: the first `<source>` before this `<img>` whose `media` matches and whose `type`
    // we can decode. Order is document order and the first match wins — later sources are not
    // considered, which is what lets an author put their best format first.
    if let Some(parent) = dom.parent(node) {
        if dom.tag_name(parent) == Some("picture") {
            for child in dom.flat_children(parent) {
                if child == node {
                    break; // sources after the <img> do not participate
                }
                if dom.tag_name(child) != Some("source") {
                    continue;
                }
                let Some(se) = dom.element(child) else {
                    continue;
                };
                if let Some(t) = se.attr("type") {
                    if !can_decode_image_type(t) {
                        continue;
                    }
                }
                if let Some(m) = se.attr("media") {
                    if !manuk_css::media_matches(m) {
                        continue;
                    }
                }
                let Some(ss) = se.attr("srcset") else {
                    continue;
                };
                let cands = parse_srcset(ss);
                let slot = sizes_slot_width(se.attr("sizes"), viewport_width);
                if let Some(u) = pick_candidate(&cands, slot, dpr) {
                    return Some(u.to_string());
                }
            }
        }
    }
    // The element's own `srcset`, then `src`.
    if let Some(ss) = el.attr("srcset") {
        let cands = parse_srcset(ss);
        let slot = sizes_slot_width(el.attr("sizes"), viewport_width);
        if let Some(u) = pick_candidate(&cands, slot, dpr) {
            return Some(u.to_string());
        }
    }
    el.attr("src")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// **What each `<img>` chose**, resolved against the document's base — the table behind
/// `<img>.currentSrc`.
///
/// Only elements that made a real source-set CHOICE are recorded. An `<img src=…>` with no
/// `srcset` and no `<picture>` parent is deliberately absent: its answer is the resolved `src`, the
/// getter already produces that, and putting it here would be a second copy of a fact that has an
/// owner. The empty map is therefore the normal case, not a failure case.
///
/// Same `select_image_url`, same width as the fetch and decode worklists — see that function's note
/// on why three callers must not each make their own choice.
fn selected_image_urls(
    dom: &Dom,
    viewport_width: f32,
    base: &str,
    out: &mut HashMap<(usize, NodeId), String>,
) {
    let arena = dom as *const Dom as usize;
    for n in dom.flat_descendants(dom.root()) {
        if dom.tag_name(n) != Some("img") {
            continue;
        }
        let Some(el) = dom.element(n) else { continue };
        // A plain `src` is not a selection. `<picture>` is, even when the `<img>` carries no
        // `srcset` of its own, because a matching `<source>` overrides the `src` entirely.
        let in_picture = dom
            .parent(n)
            .map(|p| dom.tag_name(p) == Some("picture"))
            .unwrap_or(false);
        if el.attr("srcset").is_none() && !in_picture {
            continue;
        }
        let Some(chosen) = select_image_url(dom, n, viewport_width) else {
            continue;
        };
        // ⚠ **NOT trimmed.** A `srcset` URL was already delimited by ASCII whitespace in
        // `parse_srcset`, so anything still attached to it is part of the URL — and Rust's `trim`
        // is Unicode-aware, so it eats U+00A0. WPT checks exactly that: `<img srcset="\u{a0}data:,a">`
        // must resolve to `…/%C2%A0data:,a`, and trimming here silently produced `data:,a` instead.
        if chosen.is_empty() {
            continue;
        }
        out.insert((arena, n), resolve_url(base, &chosen));
    }
}

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
                // `srcset`/`<picture>` decide which URL this element wants; `src` is the fallback.
                // Same selector as `pending_image_urls`, deliberately — see `select_image_url`.
                if let Some(src) = select_image_url(dom, n, DEFAULT_SELECTION_VIEWPORT) {
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

/// The buffers a **forced synchronous reflow** lays out into, plus the DOM state its current
/// layout was computed against.
///
/// Owned separately from the caller's snapshot on purpose: a script reading geometry holds the
/// host's `rects`/`styles` maps through a shared reference for the whole round, so a reflow cannot
/// write into them. It builds its own and re-points the bindings at those instead.
struct ReflowCtx {
    /// Valid for the script round only — `ReflowScope` ties the lifetime.
    fonts: *const FontContext,
    viewport_width: f32,
    /// `Dom::mutation_seq` as of the layout currently published. Equal means nothing has changed
    /// and the read is already answerable — the case that must stay free, since this is consulted
    /// on *every* geometry read.
    laid_out_at: u64,
    rects: HashMap<NodeId, [f32; 4]>,
    styles: HashMap<NodeId, manuk_css::ComputedStyle>,
    /// The decoded images as of this script round — a standing layout input the cascade cannot
    /// supply. See `apply_natural_sizes`.
    images: HashMap<NodeId, std::rc::Rc<manuk_paint::DecodedImage>>,
    /// ⚠⚠⚠ **THE EXTERNAL HALF OF THE CASCADE, WITHOUT WHICH THIS RE-LAYOUT IS A DIFFERENT
    /// ENGINE.** `forced_reflow` used to rebuild its sheet list with
    /// `MinimalCascade::collect_style_elements`, which reads inline `<style>` and nothing else —
    /// the exact hazard `recascade_all_sources` was extracted to name (*"it would quietly drop
    /// every external stylesheet"*), sitting unfixed in the reflow path. It was invisible while
    /// the biggest script round had no reflow hook at all; the moment t1184 armed one for the
    /// lifecycle, `css/css-grid/abspos/empty-grid-001.html` went 6 → 0 with every row reading
    /// `width expected 0 but got 784` — `.min-content` lives in `/css/support/width-keyword-
    /// classes.css`, so the forced reflow re-cascaded the page without it and every grid took the
    /// full viewport width.
    ///
    /// Pointers rather than clones, on the same contract as `fonts`: a script round's CSS text can
    /// be hundreds of KB and this is armed on every re-entry into script.
    final_url: *const String,
    external_css: *const HashMap<String, String>,
    /// ⚠⚠⚠ **THE SECOND STALENESS TERM, AND THE TREE INPUTS IT NEEDS.**
    ///
    /// `manuk_js::scroll_seq()` as of the published layout. Without it a script's
    /// `scroller.scrollTop = 100; el.getBoundingClientRect()` is answered from the **pre-scroll**
    /// layout, because a scroll assignment bumps no `Dom::mutation_seq` — the shape of every
    /// virtualised list on the web.
    scrolled_at: u64,
    /// The committed per-container scroll offsets. **A reflow that rebuilds the tree from
    /// `restyle_and_layout` gets an UNSCROLLED one**, because layout starts at zero every time and
    /// only `Page` knows what has been scrolled — so a forced reflow used to answer a mid-script
    /// geometry read with the geometry of a page nobody had scrolled. A pointer on the same contract
    /// as `fonts`: valid for the script round `ReflowScope` owns.
    scroll_offsets: *const std::collections::HashMap<NodeId, (f32, f32)>,
    /// The document scroll the sticky shifts must be resolved against, and whether the page has any
    /// sticky box at all. The reflow runs the SAME scrollport-aware pass `Page::restick` runs, so the
    /// tree a geometry read answers from and the tree that gets painted cannot disagree.
    sticky_scroll_y: f32,
    has_sticky: bool,
}

/// **Every stylesheet the document has, inline and external, in cascade order.**
///
/// A free function rather than a `Page` method because its OTHER caller is `forced_reflow`, which
/// has only a `&Dom` and a context — and when that caller grew its own copy of this logic it grew a
/// WRONG one (`collect_style_elements`, inline `<style>` only). *One rule, one implementation*: the
/// list a geometry read re-cascades against and the list the page paints from are now the same
/// list, by construction.
fn sheets_of(
    dom: &Dom,
    final_url: &String,
    external_css: &HashMap<String, String>,
) -> Vec<Stylesheet> {
    collect_style_sources(dom, final_url)
        .iter()
        .filter_map(|src| match src {
            StyleSource::Inline(css, m) => Some(Stylesheet::parse(&wrap_media(css, m))),
            // A sheet that has not arrived (or never will) contributes nothing — the same outcome
            // as before for a genuinely missing sheet, and `failed_css` is what records that it was
            // asked for.
            StyleSource::External(url, m) => external_css
                .get(url)
                .map(|css| Stylesheet::parse(&wrap_media(css, m))),
        })
        .collect()
}

/// The reflow itself, called up from the JS bindings when a geometry read finds a dirtied DOM.
///
/// # Safety
/// `ctx` is a `*mut ReflowCtx` from a live [`ReflowScope`]; `dom` is the re-entry's live arena.
/// **Hand the just-completed layout's grid track sizes to the CSSOM.**
///
/// Read from `manuk_layout`'s thread-local rather than threaded through a return value because
/// `layout_document` answers with a `LayoutBox` and nothing else; the read is only valid IMMEDIATELY
/// after a layout, which is why every caller sits directly on one ([`Page::set_root_box`], the
/// pre-`Page` load path, and [`forced_reflow`]) and none of them further away.
///
/// The tuple is the transport type: `manuk-js` has no `manuk-layout` dependency and must not grow
/// one, so `GridTracks` cannot cross that edge — the same reason the snap candidates are a bare
/// `(Vec<f32>, Vec<f32>)`.
fn publish_grid_tracks() {
    manuk_js::set_grid_tracks(
        manuk_layout::grid_tracks()
            .into_iter()
            .map(|(n, t)| (n, (t.columns, t.rows)))
            .collect(),
    );
}

unsafe fn forced_reflow(ctx: *mut std::ffi::c_void, dom: *mut Dom) {
    let c = unsafe { &mut *(ctx as *mut ReflowCtx) };
    let dom = unsafe { &*dom };
    // Idempotent: a run of reads with no mutation AND no scroll between them lays out once, not once
    // each. ⚠ The scroll term is not decoration — `el.scrollTop = n` moves no mutation counter, so
    // with the DOM term alone a same-task measurement after a scroll read the pre-scroll layout.
    let scroll_seq = manuk_js::scroll_seq();
    if dom.mutation_seq() == c.laid_out_at && scroll_seq == c.scrolled_at {
        return;
    }
    let fonts = unsafe { &*c.fonts };
    // ⚠⚠⚠ **PHASE-SPLIT, because t1236 could see that a forced reflow costs 21 SECONDS on
    // `bhramarah.in` and not what inside it does.** `REFLOW_COST` measures this function from the
    // outside and cannot see into itself; these four spans are the level down, and they are logged
    // (not merely counted) because the interesting case is ONE pass, so an aggregate would hide it.
    let t_sheets = std::time::Instant::now();
    // The same cascade the surrounding batch relayout uses, so a forced reflow and the post-script
    // pass can never disagree about the same tree.
    let sheets: Vec<Stylesheet> =
        sheets_of(dom, unsafe { &*c.final_url }, unsafe { &*c.external_css });
    let us_sheets = t_sheets.elapsed().as_micros() as u64;

    let t_layout = std::time::Instant::now();
    let mut root_box;
    (c.styles, root_box) = restyle_and_layout(dom, &sheets, fonts, c.viewport_width, &c.images);

    // ⚠⚠⚠ **LAYOUT STARTS AT ZERO EVERY TIME, SO THIS TREE IS UNSCROLLED AND UNSTUCK.** Only `Page`
    // knows what has been scrolled, so a forced reflow used to answer a mid-script geometry read
    // with the geometry of a page nobody had scrolled — a confident wrong answer of the right type,
    // which is worse than not answering (t733).
    //
    // The **pending** writes are merged over the committed ones and win: a script that just assigned
    // `scrollTop` and then measures must read back *its own write*, and the queue is peeked rather
    // than drained because `Page::drain_element_scrolls` still has to commit it or the painted tree
    // stays unscrolled.
    let mut offsets: std::collections::HashMap<NodeId, (f32, f32)> =
        unsafe { (*c.scroll_offsets).clone() };
    for (node, left, top) in manuk_js::peek_element_scrolls() {
        offsets.insert(node, (left, top));
    }
    for (node, (sx, sy)) in &offsets {
        if let Some(b) = root_box.find_mut(*node) {
            if let manuk_layout::BoxContent::Block(kids) = &mut b.content {
                for k in kids.iter_mut() {
                    k.translate(-sx, -sy);
                }
            }
        }
    }
    // The SAME scrollport-aware sticky pass `Page::restick` runs, so the tree a geometry read
    // answers from and the tree that gets painted cannot disagree about where a stuck box is.
    // ⚠⚠⚠ **ASK THE FRESH CASCADE, NOT THE FLAG CAPTURED AT INSTALL TIME.** `c.has_sticky` was read
    // off the `Page` when this scope was armed — i.e. before the script ran — so a
    // `position: sticky` element the script CREATED is invisible to it, and the sticky pass is
    // skipped for exactly the element the script is about to measure. `c.styles` is the cascade this
    // reflow just produced and does know about it. Same defect as `from_dom` passing
    // `has_sticky: false` (t1283), one layer along: **a flag derived from a snapshot is stale in
    // precisely the case a forced reflow exists to serve.**
    let has_sticky = c.has_sticky
        || c.styles
            .values()
            .any(|s| s.position == manuk_css::Position::Sticky);
    if has_sticky {
        let (_, vh) = manuk_css::values::viewport_size();
        // ⚠ **THE LIVE DOCUMENT SCROLL, NOT THE `Page`'s COMMITTED ONE.** `window.scrollTo` is a
        // request the host performs later; all it does synchronously is move `SCROLL` (optimistically,
        // t378) so `window.scrollY` reads back on the next line. So a script that scrolls and then
        // measures has a scroll the `Page` has not seen yet, and `c.sticky_scroll_y` is by definition
        // the scroll BEFORE it. Falls back to the committed value when there is no JS.
        let live_y = match manuk_js::view_scroll() {
            (_, y) if y > 0.0 => y,
            _ => c.sticky_scroll_y,
        };
        let mut applied = std::collections::HashMap::new();
        apply_sticky(&mut root_box, &c.styles, live_y, vh, &mut applied);
    }
    let us_layout = t_layout.elapsed().as_micros() as u64;

    let t_rects = std::time::Instant::now();
    c.rects = root_box
        .node_rects(dom)
        .into_iter()
        .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
        .collect();
    let us_rects = t_rects.elapsed().as_micros() as u64;

    // ⚠⚠⚠ **THE SCROLL GEOMETRY MUST BE REPUBLISHED HERE, OR A SCROLL CONTAINER THE SCRIPT JUST
    // CREATED CANNOT BE SCROLLED.** `el.scrollTop = n` is clamped in the bindings against
    // `SCROLL_GEOM` — *"max = content extent − visible window"* — so a script that writes `1e9` to
    // reach the bottom reads back the real maximum. That map is published by the host BEFORE the
    // script runs, so for an element created THIS round there is no entry, `max` is 0, and the write
    // is clamped to **zero**: the clamp is correct, its input is stale, and the scroll silently does
    // nothing. Every SPA that builds a pane and then scrolls it in the same task hits this — a chat
    // view jumping to the newest message, a virtualised list restoring position, a carousel moving
    // to the active slide.
    //
    // ⚠ Computed from the tree WITH the offsets applied above, because `scroll_geometry_of` adds the
    // offset back when it measures the extent; handing it an unscrolled tree would measure a
    // different page from the one the clamp is about.
    manuk_js::set_scroll_geometry(scroll_geometry_of(dom, &root_box, &c.styles, &offsets));
    manuk_js::set_snap_candidates(snap_candidates_of(dom, &root_box, &c.styles, &offsets));

    c.laid_out_at = dom.mutation_seq();
    c.scrolled_at = scroll_seq;
    // The box tree is dropped: the read wants rects, and the host's own post-script relayout still
    // produces the tree that gets painted. A forced reflow answers a question; it does not commit.
    let t_pub = std::time::Instant::now();
    unsafe { manuk_js::republish_view_maps(&c.rects, &c.styles) };
    // A forced reflow is what `getComputedStyle` triggers when the script has dirtied the DOM, so the
    // grid tracks have to be republished HERE too or `el.style.gridTemplateColumns = …` followed by a
    // read-back would answer with the tracks from before the assignment — the staleness the rects
    // beside them are already protected from.
    publish_grid_tracks();
    let us_pub = t_pub.elapsed().as_micros() as u64;

    // 100ms is a frame and a half — below it this is a normal read and the log would be noise;
    // above it a single geometry read is visible to a human, and this line says which phase did it.
    let total_ms = (us_sheets + us_layout + us_rects + us_pub) / 1000;
    if total_ms >= 100 {
        tracing::info!(
            total_ms,
            sheets_ms = us_sheets / 1000,
            n_sheets = sheets.len(),
            layout_ms = us_layout / 1000,
            rects_ms = us_rects / 1000,
            n_rects = c.rects.len(),
            publish_ms = us_pub / 1000,
            nodes = dom.len(),
            "SLOW FORCED REFLOW — one geometry read laid out the whole document"
        );
    }
}

/// Installs [`forced_reflow`] for one script round and guarantees its teardown.
///
/// The hook holds a raw pointer to the context; if it outlived the context that pointer is a
/// use-after-free on the next geometry read. `Drop` is what makes that impossible on every path
/// out, including a panic unwinding from script.
struct ReflowScope {
    // Boxed so the address is stable across the move into this struct.
    _ctx: Box<ReflowCtx>,
    /// What was published before this scope armed itself — see the `Drop` impl.
    prev_maps: manuk_js::ViewMaps,
}

impl ReflowScope {
    /// `dom`/`fonts`/`viewport_width` must describe the layout currently published to JS.
    fn install(
        dom: &Dom,
        fonts: &FontContext,
        viewport_width: f32,
        images: &HashMap<NodeId, std::rc::Rc<manuk_paint::DecodedImage>>,
        final_url: &String,
        external_css: &HashMap<String, String>,
        scroll_offsets: &std::collections::HashMap<NodeId, (f32, f32)>,
        sticky_scroll_y: f32,
        has_sticky: bool,
    ) -> ReflowScope {
        let mut ctx = Box::new(ReflowCtx {
            fonts: fonts as *const FontContext,
            viewport_width,
            laid_out_at: dom.mutation_seq(),
            scrolled_at: manuk_js::scroll_seq(),
            scroll_offsets: scroll_offsets as *const std::collections::HashMap<NodeId, (f32, f32)>,
            sticky_scroll_y,
            has_sticky,
            rects: HashMap::new(),
            styles: HashMap::new(),
            // Cheap: an `Rc` clone per decoded image, no pixels copied. Without it the reflow a JS
            // geometry read forces mid-script re-cascades the document and erases every image's
            // intrinsic size — the ninth re-cascade site, and it needs the same standing input the
            // other eight get.
            images: images.clone(),
            final_url: final_url as *const String,
            external_css: external_css as *const HashMap<String, String>,
        });
        let p = &mut *ctx as *mut ReflowCtx as *mut std::ffi::c_void;
        let prev_maps = manuk_js::view_maps();
        unsafe { manuk_js::set_reflow_hook(forced_reflow, p) };
        ReflowScope {
            _ctx: ctx,
            prev_maps,
        }
    }
}

impl Drop for ReflowScope {
    fn drop(&mut self) {
        manuk_js::clear_reflow_hook();
        // **The context's maps die here, and the bindings may be pointing at them.** If a forced
        // reflow ran, it re-pointed the view maps at buffers owned by `_ctx`; letting those
        // pointers outlive this drop is a use-after-free whose symptom is not a crash but the
        // *next* document silently measuring freed memory. Put back what was published before.
        unsafe { manuk_js::restore_view_maps(self.prev_maps) };
    }
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
    /// Forms whose **submit button was clicked**, awaiting the host's next
    /// [`take_form_submits`](Page::take_form_submits). `RefCell` because that getter takes `&self`
    /// (it is a drain, and every other `take_*` on `Page` has the same shape).
    pending_submits: std::cell::RefCell<Vec<(manuk_dom::NodeId, Option<manuk_dom::NodeId>)>>,
    /// Whether any element uses `position:sticky` — gates the per-frame sticky paint pass so
    /// non-sticky pages pay nothing.
    has_sticky: bool,
    /// ⚠⚠⚠ **THE STICKY SHIFT CURRENTLY BAKED INTO `root_box`, PER NODE — THE LEDGER THAT LETS A
    /// PAINT EFFECT LIVE IN THE LAYOUT TREE.**
    ///
    /// Until tick 1282 `position:sticky` was applied inside `paint_scrolled`, to a **throwaway
    /// clone**. The pixels moved and nothing else did: `getBoundingClientRect`, `offsetTop`,
    /// hit-testing, the a11y tree and the fidelity oracle's SHAPE term all read `root_box`, and
    /// `root_box` had never contained the shift. So a stuck header reported its *in-flow* position
    /// — the same number scrolled and unscrolled — which is every one of the 63 failing assertions
    /// in `css/css-position/sticky`, and every scroll-spy, "is it stuck?" class toggle and
    /// sticky-aware measurement library on the real web.
    ///
    /// The shift now lands in `root_box`. That is only safe because it is **exactly invertible**:
    /// this map holds the `dy` translated into each sticky node's subtree, so
    /// [`Page::restick`] can undo it and recompute from the true in-flow layout instead of
    /// compounding scroll after scroll. Empty on a page with no sticky box, which is nearly all of
    /// them.
    sticky_applied: std::collections::HashMap<manuk_dom::NodeId, f32>,
    /// The **document** scroll offset the baked sticky shifts were computed for. Element scroll
    /// containers carry their own offset in [`Page::scroll_offsets`] and need no mirror here — their
    /// children are already translated in the tree, so their scrollport is a plain box rect.
    sticky_scroll_y: f32,
    /// ⚠⚠⚠ **THE "ALREADY RAN" MARKER FOR RUNTIME-FETCHED SCRIPTS, WHICH USED TO BE "DELETE ITS
    /// `src`" — AND A SCRIPT CANNOT READ ITS OWN URL OFF AN ATTRIBUTE THAT IS GONE.**
    ///
    /// `fetch_and_run_dynamic_scripts` removed `src` *before* evaluating, so throughout its own
    /// execution a script's `document.currentScript.src` was the **empty string** where Chrome
    /// gives the URL. That is a wrong answer of the right type, which is worse than a missing one:
    /// `new URL(document.currentScript.src)` does not skip, it **throws** —
    /// `TypeError: Invalid URL: ` on 4 of 200 CrUX corpus sites (marktplaats.nl, mangaraw.ac,
    /// sports.yahoo.com, nautica.com).
    ///
    /// The marker moves in here so `src` can survive the call. The attribute is still removed
    /// **after** the script runs, so the post-run document is byte-identical to before — the only
    /// thing that changed is what the script itself can see while it is running.
    dyn_scripts_ran: std::collections::HashSet<manuk_dom::NodeId>,
    /// **The pre-fetched ES-module import graph (B3b)** — resolved-url → source for every module the
    /// page's `<script type=module>` roots reach. Filled by the async pre-fetch pass (`load_async` /
    /// `prepare_prefetched`) and seeded into the JS layer by [`run_deferred_scripts`] at the instant the
    /// deferred (module) pass runs — kept ON the page, not in a thread-local, so it survives the shell's
    /// blocking→paint→deferred gap and is seeded on whichever thread runs the scripts. Empty ⇒ modules
    /// link self-contained, exactly as before the loader existed.
    module_graph_sources: std::collections::HashMap<String, String>,
    /// `<script type=module>` node index → the URL its source was fetched from. Empty for a document
    /// whose module roots are all inline (their base IS the document url). A module's imports resolve
    /// against the MODULE's url, and after `fetch_external_scripts` inlines an external one the DOM can
    /// no longer say where it came from — this is that record.
    module_node_bases: std::collections::HashMap<usize, String>,
    /// **The document's import map (tick 520)** — bare specifier → raw target, parsed from a
    /// `<script type=importmap>`. Seeded into the JS layer alongside [`Self::module_graph_sources`] by
    /// `run_deferred_scripts` so `import 'react'` resolves through it. Empty ⇒ bare specifiers fail
    /// loud-but-safe, exactly as before.
    import_map: std::collections::HashMap<String, String>,
    zoom: f32,
    /// Decoded raster images keyed by their `<img>` node, painted into each element's box.
    images: std::collections::HashMap<manuk_dom::NodeId, std::rc::Rc<manuk_paint::DecodedImage>>,
    /// Inline `<svg>` elements rasterized via the SAME usvg/resvg path that decodes `<img
    /// src="*.svg">` (tick 394 — the paint half of the SVG-internals spec). Keyed by the svg
    /// element's node; decoded once per node and carried across `apply_images` calls, which
    /// REPLACE `self.images` wholesale every round. A script that mutates an svg's subtree after
    /// first decode keeps the stale raster (named residue; re-serialize-on-mutation is the fix).
    inline_svg_cache:
        std::collections::HashMap<manuk_dom::NodeId, std::rc::Rc<manuk_paint::DecodedImage>>,
    /// The GEOMETRY half of the same spec: each inline `<svg>`'s interior bounding boxes, in the
    /// user space usvg resolved. Filled from the SAME parse as the raster above, so the two halves
    /// describe one document; consumed after every layout by [`svg_geometry::apply`], which needs
    /// the svg's used box and therefore cannot run any earlier. Absent for every svg the pairing
    /// refuses — see the module docs; those keep their CSS-derived boxes.
    svg_geometry: svg_geometry::SvgGeometry,
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
    /// Render-blocking external stylesheets that were requested and NEVER arrived. A page styled by
    /// UA defaults because its author sheet timed out is not this engine's layout, and anything
    /// measuring the engine (the differential oracle above all) must be able to ask. A URL is
    /// removed if a later fetch round does deliver it — this set holds what is failed NOW.
    failed_css: std::collections::HashSet<String>,
    /// Render-blocking stylesheet URLs the origin answered `404`/`410` for. **A settled answer, so
    /// this navigation must not ask again** — Part 22.3's "no URL on the wire twice" applies to a
    /// definitive absence exactly as it does to a hit.
    ///
    /// ⚠⚠ **WITHOUT THIS THE 404 EXEMPTION LEAKS BACK IN THROUGH THE DEADLINE (t860).** A 404 sheet
    /// never enters `external_css`, so the `!external_css.contains_key` filter re-requested it on
    /// every re-entry after a round of dynamic scripts — and on a *late* re-entry the load deadline
    /// cuts it before it can settle, at which point the `cut` block books it as "never settled" and
    /// the page is UA-fallback again. `cuneocronaca.it` is the measured case: both its dead sheets
    /// were correctly exempted on the first pass and re-blamed on the second, so the site stayed
    /// unscorable with the exemption working perfectly. **An exemption keyed on the OUTCOME of a
    /// fetch is undone by anything that stops the fetch from having an outcome.**
    absent_css: std::collections::HashSet<String>,
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
    /// The document's Content-Security-Policy, retained for the loads that happen **after**
    /// construction — a script injected by a later script, fetched by `fetch_and_run_dynamic_scripts`.
    /// Those are exactly the loads an XSS payload makes, so a policy that only covered the initial
    /// parse would be enforcing on the half that matters least.
    csp: manuk_net::csp::Csp,
    /// **When the load budget runs out — the instant the subresource fan-outs must stop WAITING and
    /// commit whatever arrived.** `None` outside a budgeted load (a script round, the shell's own
    /// re-fetch), where the caller owns the clock and a fan-out runs to completion.
    ///
    /// This exists because the budget's enforcement and its bookkeeping were the same future. See
    /// [`collect_before_deadline`].
    load_deadline: Option<std::time::Instant>,
}

/// **Is this an XML MIME type?** The MIME Sniffing standard's rule, and the `+xml` suffix is the
/// load-bearing half: it is what makes `application/xhtml+xml`, `image/svg+xml`,
/// `application/rss+xml` and `application/atom+xml` all XML without enumerating them — an
/// enumeration would be wrong for the next one.
///
/// Parameters are stripped (`text/xml; charset=utf-8`) and the comparison is ASCII-case-insensitive,
/// because a real server sends both. An absent or unparseable type is NOT XML: HTML is the safe
/// default on a path where guessing wrong means a blank frame.
fn is_xml_mime(content_type: Option<&str>) -> bool {
    let Some(ct) = content_type else { return false };
    let essence = ct
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    essence == "text/xml" || essence == "application/xml" || essence.ends_with("+xml")
}

/// **The slice of the load budget reserved for COMMITTING what the network already delivered.**
///
/// The budget is enforced by a `tokio::time::timeout` around the whole phase sequence, so the instant
/// it expires the future is dropped — including the decode-and-apply that would have banked the
/// bytes. A fan-out that stops fetching at exactly the deadline therefore still lands nothing. So the
/// fan-outs stop this much *earlier* and spend the remainder committing.
///
/// It is deliberately taken out of the budget rather than added to it: the page's outer deadline is a
/// Bar 0 promise about how long a tab can be busy, and buying correctness by extending it would be
/// trading the floor for the capability. Nothing about the total load time changes.
const COMMIT_RESERVE: std::time::Duration = std::time::Duration::from_millis(400);

/// How many times the event loop has given up on a non-converging drain — `0` in the headless build,
/// where there is no event loop to give up. The navigation ledger reports this per phase and must
/// compile in BOTH lanes: the JS-less `--no-default-features` build is CI's gating lane, and reaching
/// straight for `manuk_js::event_loop` broke it on the first draft of this ledger.
fn drain_ceiling_hits_or_zero() -> usize {
    #[cfg(feature = "spidermonkey")]
    {
        manuk_js::event_loop::drain_ceiling_hits()
    }
    #[cfg(not(feature = "spidermonkey"))]
    {
        0
    }
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
    dom: &Dom,
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
        // ⚠⚠⚠ **THE SCROLLBAR COMES OUT OF `clientWidth`/`clientHeight` TOO, AND THIS IS THE SECOND
        // CONSUMER OF ONE RULE.** CSSOM-View defines `clientWidth` as the padding box *"excluding
        // the vertical scrollbar"*, and the layout gutter below has been reserving that space for
        // children since it landed — but this table was still handing out the full padding box.
        // Measured against headless Chrome, one `overflow: scroll` box 200×100: Chrome reports
        // `clientWidth` **185** where we reported **200**.
        //
        // ⭐ It is not a cosmetic 15px. Every virtualised list on the web — react-window,
        // TanStack Virtual, every data grid — divides `clientHeight` by the row height to decide how
        // many rows to render, and a `clientHeight` that is one scrollbar too large renders a row
        // too many and then measures the overflow it just caused. This is the FUNCTION axis of the
        // same defect whose SHAPE axis is `manuk_layout::scrollbar_gutter`.
        //
        // ⚠ Only the deterministic `scroll` case, exactly as the layout gutter does — an
        // `overflow: auto` box that happens to overflow needs a second layout pass and is residue in
        // both places. The two must agree, so they call the same function.
        // A reserved gutter is inside the padding box exactly as a shown one is, so `clientWidth`
        // must subtract it too — the virtualised-list arithmetic above reads this number, and a
        // `scrollbar-gutter: stable` pane would otherwise report one scrollbar too many pixels.
        let gy = manuk_layout::gutter_reservation(
            st.overflow_y,
            st.scrollbar_width,
            st.scrollbar_gutter,
            false,
        );
        let gx = manuk_layout::gutter_reservation(
            st.overflow_x,
            st.scrollbar_width,
            st.scrollbar_gutter,
            false,
        );
        let client_w = (b.rect.width - bw.left - bw.right - gy).max(0.0);
        let client_h = (b.rect.height - bw.top - bw.bottom - gx).max(0.0);
        // ⚠⚠⚠ **THE SCROLL CONTAINER'S OWN END PADDING IS PART OF THE SCROLLABLE OVERFLOW REGION,
        // AND LEAVING IT OUT WAS INVISIBLE BEHIND THE CLIENT-BOX FLOOR.** CSS Overflow 3 §3.1 puts
        // the box's padding box into the region and inflates descendants at the END edges by the
        // container's own padding — so a 100px-tall filler in a `padding: 10px` scroller has a
        // scrollable overflow of 10 + 100 + 10 = 120, not 110.
        //
        // Measured, headless Chrome, `width:100px;height:100px;padding:10px;overflow:scroll` around
        // a 1×100 filler: `clientHeight` **105**, `scrollHeight` **120**, `clientWidth` **105**,
        // `scrollWidth` **105**. The 120 is content 110 + padding-bottom 10; the 105s are the client
        // floor. Both rules are needed to produce that pair of numbers and neither alone does.
        //
        // ⭐ This was hidden by the very bug fixed above: `client_h` used to be the FULL padding box,
        // which floored the under-computed extent right back up to the expected number. Four
        // `css/css-overflow` files were passing on that floor rather than on the geometry, and
        // subtracting the scrollbar exposed all four at once — a correction arriving in the shape of
        // a regression.
        //
        // ⚠ `resolve` against the box's own border-box width is exact for `px`/`calc()` padding and
        // an approximation for PERCENTAGE padding, whose correct reference is the containing block's
        // width — not available here. Named residue: percentage padding on a scroll container.
        let (cw, ch) = b.content_extent();
        // `content_extent` measures from the BORDER-box origin; the scrollable overflow region is
        // measured in the PADDING box, so drop the start border and add the end padding.
        let cw = (cw - bw.left + st.padding.right.resolve(b.rect.width, 0.0)).max(0.0);
        let ch = (ch - bw.top + st.padding.bottom.resolve(b.rect.width, 0.0)).max(0.0);
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

    // ⚠⚠⚠ **THE ROOT ELEMENT'S `clientWidth`/`clientHeight` ARE THE VIEWPORT, NOT ITS OWN BOX — AND
    // ANSWERING WITH ITS OWN BOX MEANT `documentElement.clientHeight` WAS THE HEIGHT OF THE WHOLE
    // DOCUMENT.**
    //
    // CSSOM-View is explicit: *"if the element is the root element and the document is not in quirks
    // mode … return the viewport width/height excluding the size of a rendered scroll bar"* (and in
    // quirks mode the same clause moves to the HTML `body` element). Every other element reports its
    // padding box, which is what the fallback above did for everybody — so the ONE element with a
    // different rule got the general one.
    //
    // Measured, headless Chrome vs here, viewport 800x800 over a 5,000px-tall document:
    //
    // ```text
    //                              Chrome        before
    //     documentElement.clientWidth    800        784
    //     documentElement.clientHeight   800       5800   <- the DOCUMENT height
    //     100vw / 100vh              800 / 800  800 / 800  <- already right, which is what hid it
    // ```
    //
    // ⭐ **`clientHeight` off by 7x is not a geometry rounding error, it is a load-bearing lie.**
    // `document.documentElement.clientHeight` is *the* "how tall is the viewport?" idiom on the web:
    // `scrollTop + clientHeight >= scrollHeight` is the infinite-scroll test, lazy-image loaders use
    // it to decide what is within a screen of the fold, sticky/scroll-spy code compares against it,
    // and virtualised lists divide by it to choose a row count. With `clientHeight` equal to the
    // document height, *everything* is inside the viewport: infinite scroll fires at once, every
    // lazy image loads immediately, and the row count is the whole list.
    //
    // `vw`/`vh` were already correct — they resolve from `values::viewport_size()`, the same source
    // used here — which is exactly why this survived: the CSS half of the viewport was right in
    // every test that looked, and only the CSSOM half was wrong.
    //
    // `scrollWidth`/`scrollHeight` come along because they are the same element's *scrolling area*,
    // floored at the viewport (a short document still scrolls zero, not negative). `scrollTop`/
    // `scrollLeft` stay 0 here, which is what the fallback already answered — the document scroll
    // offset is not part of this table, and pretending otherwise would be a new claim, not a fix.
    // ⚠ The ICB, not the window — and the subtraction is NOT repeated here. `values::icb_size()`
    // has already had the root element's scrollbar taken out of it by `root_scrollbar_icb`, which
    // is also what `vw`/`vh` and `@media` now read; doing the arithmetic a second time in this
    // table would take 30px out of a page that reserves 15. **One narrowing, one owner.**
    let (vpw, vph) = manuk_css::values::icb_size();
    // The document element is POSITIONAL — the first element child of the document node — not
    // `<html>` by name, so an XML/SVG document root gets the rule too. Same definition as
    // `document.documentElement`, deliberately: two definitions of the root element is how the two
    // drift apart.
    let de = dom.children(dom.root()).find(|&c| dom.is_element(c));
    // Quirks mode moves the clause to `body`. ⚠ Only the element the clause names is given the
    // viewport: in quirks the root element keeps reporting its own box, which is what Chrome does.
    let host = if dom.quirks() {
        de.and_then(|d| dom.find_first_in(d, "body"))
    } else {
        de
    };
    if let Some(h) = host {
        // ⚠ **THE ROOT ELEMENT HAS NO BOX IN OUR TREE.** `layout_document` roots the box tree at
        // `<body>` (`find_first("body")`, then `html`), so `root_box.find(<html>)` is `None` on every
        // ordinary page — and a `None` here silently made the document's scrolling area equal to the
        // viewport, i.e. *"this page does not scroll"*, which is the same lie as the one above wearing
        // different clothes. The document's scrolling area is the root box's far edge, which is what
        // the fallback in `scroll_getter!` was already reporting before this arm existed. Measured as
        // the far EDGE, not the size: the body's own top margin is part of the document's height.
        let b = root_box.find(h).unwrap_or(root_box);
        let (content_w, content_h) = (b.rect.x + b.rect.width, b.rect.y + b.rect.height);
        m.insert(
            h,
            [
                0.0,
                0.0,
                content_h.max(vph),
                content_w.max(vpw),
                vph.max(0.0),
                vpw.max(0.0),
            ],
        );
    }

    m
}

/// Per-container snap-point candidates, in content-space, clamped to the scrollable range —
/// `(xs, ys)` per snap container, with an axis's list left EMPTY when the container does not
/// snap that axis (`scroll-snap-type: x` says nothing about y).
///
/// This is the ONE collection of snap candidates. `Page::snap_scroll` (the engine chokepoint)
/// and the JS `scrollLeft`/`scrollTop` setters (via `manuk_js::set_snap_candidates`) both
/// consume it — measured Chrome snaps SYNCHRONOUSLY (`el.scrollLeft = 130; el.scrollLeft`
/// reads `100` on the same line, tick 408), so the JS-side mirror must know the snap points at
/// assignment time; recomputing them in the bindings would be the two-sources-of-truth trap.
fn snap_candidates_of(
    dom: &Dom,
    root_box: &manuk_layout::LayoutBox,
    styles: &StyleMap,
    offsets: &std::collections::HashMap<manuk_dom::NodeId, (f32, f32)>,
) -> std::collections::HashMap<manuk_dom::NodeId, (Vec<f32>, Vec<f32>)> {
    use manuk_css::Overflow;
    let geom = scroll_geometry_of(dom, root_box, styles, offsets);
    let mut m = std::collections::HashMap::new();
    for (node, st) in styles.iter() {
        let axis = st.scroll_snap_type;
        if axis == manuk_css::ScrollSnapAxis::None {
            continue;
        }
        if !matches!(
            st.overflow,
            Overflow::Auto | Overflow::Scroll | Overflow::Hidden
        ) {
            continue;
        }
        let Some(container) = root_box.find(*node) else {
            continue;
        };
        let Some(g) = geom.get(node) else { continue };
        let max = ((g[3] - g[5]).max(0.0), (g[2] - g[4]).max(0.0));
        let cur = offsets.get(node).copied().unwrap_or((0.0, 0.0));
        let (xs, ys) = snap_candidates_for(container, *node, styles, cur, max);
        m.insert(
            *node,
            (
                if axis.snaps_x() { xs } else { Vec::new() },
                if axis.snaps_y() { ys } else { Vec::new() },
            ),
        );
    }
    m
}

/// The snap offsets `container`'s aligned descendants ask for, on both axes, clamped to `max`.
/// Walks the CONTAINER's own subtree, not the whole page: a snap point is a descendant of the
/// scroller, and collecting them document-wide would let one carousel snap to another's slide.
fn snap_candidates_for(
    container: &manuk_layout::LayoutBox,
    node: manuk_dom::NodeId,
    styles: &StyleMap,
    cur: (f32, f32),
    max: (f32, f32),
) -> (Vec<f32>, Vec<f32>) {
    let (cw, ch) = (container.rect.width, container.rect.height);
    let origin = (container.rect.x, container.rect.y);
    let mut xs: Vec<f32> = Vec::new();
    let mut ys: Vec<f32> = Vec::new();
    let mut candidates: Vec<(manuk_dom::NodeId, manuk_layout::Rect)> = Vec::new();
    container.walk(&mut |b: &manuk_layout::LayoutBox| {
        if let Some(n) = b.node {
            if n != node {
                candidates.push((n, b.rect));
            }
        }
    });
    for (kid, krect) in candidates {
        let Some(ks) = styles.get(&kid) else {
            continue;
        };
        let align = ks.scroll_snap_align;
        if align == manuk_css::ScrollSnapAlign::None {
            continue;
        }
        // The children's rects are in the CURRENT (already-scrolled) tree; adding the live offset
        // recovers the content-space position each snap point actually sits at.
        let (kx, ky) = (krect.x - origin.0 + cur.0, krect.y - origin.1 + cur.1);
        let (kw, kh) = (krect.width, krect.height);
        // The offset that puts this child where the alignment asks inside the snapport.
        let (sx, sy) = match align {
            manuk_css::ScrollSnapAlign::Start => (kx, ky),
            manuk_css::ScrollSnapAlign::Center => (kx - (cw - kw) / 2.0, ky - (ch - kh) / 2.0),
            manuk_css::ScrollSnapAlign::End => (kx - (cw - kw), ky - (ch - kh)),
            manuk_css::ScrollSnapAlign::None => continue,
        };
        xs.push(sx.clamp(0.0, max.0));
        ys.push(sy.clamp(0.0, max.1));
    }
    (xs, ys)
}

/// Point `CSS.supports()` at the real feature-query evaluator.
///
/// Without the `stylo` feature there is no evaluator, and the answer is a flat `false`. That is the
/// conservative direction on purpose: a build that cannot honour a property must not claim it, or
/// every page throws away the fallback it already shipped in favour of a branch this configuration
/// cannot render.
fn install_supports_hook() {
    #[cfg(feature = "stylo")]
    manuk_js::set_supports_hook(manuk_css::stylo_engine::supports_condition);
    #[cfg(not(feature = "stylo"))]
    manuk_js::set_supports_hook(|_| false);
    // The SERIALIZING half of the same seam (t1224): `el.style` must report CSSOM's normal form,
    // not the author's text. ⚠ Its no-engine answer is `None`, not a value — the opposite polarity
    // from `supports`, because echoing an un-normalised string costs conformance while inventing an
    // empty one would DELETE a declaration the page set.
    #[cfg(feature = "stylo")]
    manuk_js::set_serialize_decl_hook(manuk_css::stylo_engine::serialize_declaration);
    #[cfg(not(feature = "stylo"))]
    manuk_js::set_serialize_decl_hook(|_, _| None);
    // The EXPANDING half: a shorthand sets its longhands, and `el.style` had no way to say so. Same
    // polarity as the serializer — with no engine the answer is "nothing", which leaves a longhand
    // read exactly where it was rather than inventing values no parser produced.
    #[cfg(feature = "stylo")]
    manuk_js::set_expand_decl_hook(manuk_css::stylo_engine::expand_declaration);
    #[cfg(not(feature = "stylo"))]
    manuk_js::set_expand_decl_hook(|_, _| Vec::new());
    // The ENUMERABLE half of the same question, from the same oracle — see
    // `stylo_engine::supported_property_names`. Installed here rather than at three call sites for
    // the reason the comment above gives: three callers is what produced one.
    #[cfg(feature = "stylo")]
    manuk_js::set_supported_props_hook(manuk_css::stylo_engine::supported_property_names);
    #[cfg(not(feature = "stylo"))]
    manuk_js::set_supported_props_hook(|| &[]);
}

thread_local! {
    /// The Content-Security-Policy the **next** page built on this thread is born under.
    ///
    /// `Page::from_dom` takes no headers — it is handed an already-parsed DOM — but CSP arrives in
    /// the response headers and must be in force *before* the document's first script runs, which
    /// happens inside `from_dom`. That is the same shape as `window.opener` (a popup's login script
    /// reads it at load time, so it cannot be assigned afterwards), and it is solved the same way
    /// here: seed it, then construct. See [`set_pending_identity`].
    static PENDING_CSP: std::cell::RefCell<Option<(manuk_net::csp::Csp, Vec<NodeId>)>> =
        const { std::cell::RefCell::new(None) };

    /// The `<script>` nodes the **next** page built on this thread inlined from the network.
    ///
    /// Same shape and same reason as `PENDING_CSP` above: the fact is known at FETCH time, it is
    /// needed while `from_dom` is running the document's scripts, and the fetch destroys the
    /// evidence on its way past (`src` is removed once the source is inlined). Seed, then construct.
    /// The JS layer takes ownership of the set at `load_document`, so it is document-scoped from
    /// there on and this cell holds nothing between navigations.
    static PENDING_EXTERNAL_SCRIPTS: std::cell::RefCell<std::collections::HashSet<NodeId>> =
        std::cell::RefCell::new(std::collections::HashSet::new());

    /// The author's **external** stylesheets (`<link rel=stylesheet>` URL → CSS text) that had
    /// already arrived when the next page on this thread is constructed.
    ///
    /// ⚠⚠⚠ **A `<link rel=stylesheet>` IS SCRIPT-BLOCKING, AND EVERY BLOCKING `<script>` HERE RAN
    /// BEFORE IT (t1296).** `from_dom` cascades, lays out, and then runs the document's blocking
    /// scripts — and both async callers (`load_async`, `from_prefetched_inner`) applied the fetched
    /// external CSS only *after* `from_dom` returned. So a page's own inline script measured a
    /// document styled by nothing but the UA sheet and its own `<style>` blocks:
    ///
    /// ```text
    ///   at parse time   getComputedStyle(el).display = "block"   ← the <link> said `grid`
    ///   at `load`       getComputedStyle(el).display = "grid"
    /// ```
    ///
    /// That is not a WPT artefact. HTML §"a style sheet that is blocking scripts" exists precisely
    /// because pages measure at parse time: a carousel that reads `offsetWidth` from an inline
    /// script, a framework that snapshots `getBoundingClientRect` before hydrating, and every
    /// `document.body.clientWidth` breakpoint check get UA-default geometry and act on it.
    ///
    /// Same shape and same reason as the two seeds above — the bytes are known before construction
    /// and needed *during* it — so it is solved the same way: seed, then construct. `from_dom` takes
    /// it exactly once, so a sheet seeded here can never leak into the next navigation or into a
    /// subframe's own construction.
    static PENDING_EXTERNAL_CSS: std::cell::RefCell<Option<HashMap<String, String>>> =
        const { std::cell::RefCell::new(None) };
}

/// Seed the already-fetched external stylesheets for the next page built on this thread, so the
/// **initial** cascade — the one the document's blocking scripts measure — contains them.
///
/// ⚠ Only sheets whose bytes are already in hand belong here. This must never introduce a *wait*:
/// the two designs that did were measured and reverted (see [`Page::load_async`]). A sheet still in
/// flight is left to `apply_stylesheets`, exactly as before.
fn set_pending_external_css(css: HashMap<String, String>) {
    PENDING_EXTERNAL_CSS.with(|p| *p.borrow_mut() = Some(css));
}

/// Does this document contain a `<script>` that `from_dom` will execute **before** anything else
/// gets a turn — i.e. one that can observe an unstyled layout?
///
/// The predicate mirrors `manuk_js`'s own `blocks_paint`: a module is deferred by default, and
/// `defer`/`async` both mean *"do not block me"*. By the time this is asked, `fetch_external_scripts`
/// has already inlined each external script's text into its element, so a `<script src>` looks like
/// an inline one here — which is exactly the set that matters.
///
/// ⚠⚠ **This predicate is what makes waiting for stylesheets affordable, and it is the answer to the
/// counter-example that killed the two earlier designs (t715/t716).** The fixture that made a
/// concurrent CSS wait cost 2s *"has dead sheets and no scripts"* — under this predicate it waits
/// zero, because with no blocking script there is nothing that could tell the difference and the
/// existing post-construction apply is already early enough.
fn document_has_blocking_script(dom: &Dom) -> bool {
    dom.descendants(dom.root()).any(|n| {
        if dom.tag_name(n) != Some("script") {
            return false;
        }
        let Some(el) = dom.element(n) else {
            return false;
        };
        let is_module = el
            .attr("type")
            .map(|t| t.trim().eq_ignore_ascii_case("module"))
            .unwrap_or(false);
        !is_module
            && el.attr("defer").is_none()
            && el.attr("async").is_none()
            && !dom.text_content(n).trim().is_empty()
    })
}

/// Move every stylesheet fetch that has **already finished** out of `inflight` and into `out`,
/// leaving the ones still in flight running.
///
/// ⚠ `is_finished()` then `now_or_never()`, never `await` — this must be callable at any point in a
/// navigation without lengthening it. Extracted because it is now called twice (before the blocking
/// scripts and after them), and two hand-written copies of a "does this block?" loop is exactly the
/// kind of duplication that ends with one of them awaiting.
fn harvest_finished_css(
    inflight: &mut Vec<(String, tokio::task::JoinHandle<Option<String>>)>,
    out: &mut HashMap<String, String>,
) {
    use futures_util::FutureExt;
    let mut still_running = Vec::new();
    for (url, handle) in std::mem::take(inflight) {
        if !handle.is_finished() {
            still_running.push((url, handle));
            continue;
        }
        if let Some(Ok(Some(text))) = handle.now_or_never() {
            out.insert(url, text);
        }
    }
    *inflight = still_running;
}

/// **A frame's viewport is its CONTENT box, not its border box** — and the difference is 4px on a
/// default `<iframe>`, because the UA sheet gives it a 2px border.
///
/// ⚠⚠⚠ `node_rects` returns the **border** box. Handing that to the child makes `100vw` inside
/// `iframe { width: 200px }` resolve to **204px**, which is `css/css-values/viewport-units-compute`'s
/// first assertion, off by exactly the two borders. Every frame-sizing call site must go through
/// here, because a second copy of this rule is how two of them end up disagreeing.
///
/// ⚠ **This is also the trap a too-kind probe walks straight past.** The fixture that first proved
/// the frame reflow worked wrote `border:0` on the iframe — and so read `200px` and declared
/// victory, because it had removed the one thing that breaks it. A probe must not style away the
/// default it is measuring.
fn frame_content_box(
    style: Option<&manuk_css::ComputedStyle>,
    border_box_w: f32,
    border_box_h: f32,
) -> (f32, f32) {
    let Some(s) = style else {
        return (border_box_w.max(1.0), border_box_h.max(1.0));
    };
    // Percentage padding resolves against the containing block's WIDTH on both axes (CSS 2.1
    // §8.4 — yes, vertical padding too); the border box is the closest reference in hand and is
    // exact for the `px` padding that is all an `<iframe>` ever carries in practice.
    let pad_x =
        s.padding.left.resolve(border_box_w, 0.0) + s.padding.right.resolve(border_box_w, 0.0);
    let pad_y =
        s.padding.top.resolve(border_box_w, 0.0) + s.padding.bottom.resolve(border_box_w, 0.0);
    let bord_x = s.border_width.left + s.border_width.right;
    let bord_y = s.border_width.top + s.border_width.bottom;
    (
        content_from(border_box_w, pad_x, bord_x),
        content_from(border_box_h, pad_y, bord_y),
    )
}

/// The one-axis arithmetic behind [`frame_content_box`], split out so the forced-reflow path can
/// apply the SAME subtraction to terms it measured itself. It is the *rule* that must not be
/// duplicated, not this line — a caller that has already gathered `padding` and `border-width`
/// sums for an axis subtracts them here rather than growing a second opinion about which terms
/// count.
fn content_from(border_box: f32, pad: f32, bord: f32) -> f32 {
    (border_box - pad - bord).max(1.0)
}

/// **Resolve viewport units against THIS frame's viewport for the duration of a child cascade.**
///
/// ⚠⚠⚠ `manuk_css::values` keeps the viewport in a **global**, and its own comment says why the
/// height drifts: *"cascade sites thread an authoritative width but not always a height, so they
/// update width alone."* A child therefore cascaded with the frame's width and the **window's
/// height**, so in a 200x100 frame `100vw` was right and `100vh` answered **800px** — and `vmin`
/// and `vmax`, being derived, were wrong in opposite directions, which is why the failures did not
/// look like one bug.
///
/// Restores on drop, on every path including a panic, because a child cascade that leaked its
/// viewport would silently mis-resolve the PARENT's next one.
struct ViewportScope((f32, f32));

impl ViewportScope {
    fn set(w: f32, h: f32) -> ViewportScope {
        let prev = manuk_css::values::viewport_size();
        manuk_css::values::set_viewport(w, h);
        ViewportScope(prev)
    }
}

impl Drop for ViewportScope {
    fn drop(&mut self) {
        manuk_css::values::set_viewport(self.0 .0, self.0 .1);
    }
}

#[cfg(feature = "spidermonkey")]
thread_local! {
    /// **Child pages built ON DEMAND, mid-script, awaiting adoption** — see
    /// [`frame_create_on_demand`]. They are already registered with JS and already readable; this
    /// only holds ownership until the parent `Page` takes them.
    /// ⚠ **`Box<Page>`, and that is load-bearing.** The reflow registry publishes a `*mut Page` and
    /// `&Page::styles`, both inline in the value — so a bare `Vec<Page>` would move every child it
    /// already holds the moment it grew, dangling every pointer published for the earlier ones. The
    /// `Box` makes each child's address independent of the vector's.
    static DYNAMIC_FRAMES: std::cell::RefCell<Vec<(manuk_dom::NodeId, Box<Page>, f32, f32)>> =
        std::cell::RefCell::new(Vec::new());
    /// What the on-demand builder needs and cannot get from the DOM: the fonts, and the base URL a
    /// frame's `about:blank`/`srcdoc` document inherits from its embedder (HTML §4.8.5).
    static DYNAMIC_FRAME_HOST: std::cell::RefCell<Option<(*const FontContext, String)>> =
        std::cell::RefCell::new(None);
}

/// Arm the on-demand frame builder for one script round. Cheap; called wherever scripts are about
/// to run and a `FontContext` + base URL are in hand.
#[cfg(feature = "spidermonkey")]
fn arm_dynamic_frames(fonts: &FontContext, base_url: &str) {
    DYNAMIC_FRAME_HOST.with(|h| {
        *h.borrow_mut() = Some((fonts as *const FontContext, base_url.to_string()));
    });
}

/// **Build the document for a frame the SCRIPT just created, at the moment it is first read.**
///
/// ⚠⚠⚠ HTML §4.8.5 navigates a srcless `<iframe>` to `about:blank` **when it is inserted**, so
///
/// ```js
///   var f = document.createElement('iframe');
///   document.body.appendChild(f);
///   f.contentDocument.body            // <- already a document, per spec
/// ```
///
/// is a synchronous read of something that already exists. Ours arrived on the host's *next round*,
/// so it read `null` — and `typeof null === 'object'`, so every feature detect passed and the next
/// line threw. `G_INLINE_FRAME_DOCUMENT` assertion (4) has pinned this as an honest known gap since
/// t512; t1297 closed the parser-inserted half, and this is the other one.
///
/// **Why it is not a niche.** This exact sequence is the **ad slot** (build a frame, write the
/// creative into it), the OAuth / 3-D-Secure bridge, the sandboxed preview, and the pristine-`window`
/// lift every library uses to recover unpatched natives from a frame nobody has touched.
///
/// **Lazy, deliberately.** Nothing hooks `appendChild`: a frame nobody reads costs nothing, and the
/// read is the only place the gap is observable, so that is where the work happens.
///
/// Returns `true` if a document now exists for `node`.
///
/// # Safety
/// Called from the JS bindings with the arena that owns `node`.
#[cfg(feature = "spidermonkey")]
unsafe extern "C" fn frame_create_on_demand(
    dom: *mut manuk_dom::Dom,
    node: manuk_dom::NodeId,
) -> bool {
    let Some((fonts_ptr, base)) = DYNAMIC_FRAME_HOST.with(|h| h.borrow().clone()) else {
        return false;
    };
    let parent = unsafe { &*dom };
    if parent.tag_name(node) != Some("iframe") {
        return false;
    }
    let Some(el) = parent.element(node) else {
        return false;
    };
    // **Only the network-free frames** — the same three-way classification `load_inline_frames` and
    // `build_inline_frame_docs` make, and it must stay the same three. A frame with a real `src` is
    // the fetch path's business and is correctly declined here; `FRAME_CREATE_TRIED` makes that
    // decline cost one set lookup for the rest of the page's life.
    let html = if let Some(doc) = el.attr("srcdoc") {
        doc.to_string()
    } else {
        let src = el.attr("src").unwrap_or("").trim().to_string();
        if !(src.is_empty() || src.eq_ignore_ascii_case("about:blank")) {
            return false;
        }
        String::new()
    };
    if INLINE_FRAME_DEPTH.with(|d| d.get()) >= MAX_IFRAME_DEPTH {
        return false;
    }
    let fonts = unsafe { &*fonts_ptr };
    // ⚠ **300x150, the HTML default object size** — and it is the honest answer, not a fallback. A
    // frame created this instant has not been laid out, so it HAS no box yet; the next layout gives
    // it one and `publish_iframe_docs` republishes the real width with it.
    let (w, h) = (300.0f32, 150.0f32);
    INLINE_FRAME_DEPTH.with(|d| d.set(d.get() + 1));
    let mut child = {
        let _vp = ViewportScope::set(w, h);
        Page::load(&html, &base, fonts, w)
    };
    INLINE_FRAME_DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
    {
        let croot = child.dom.root();
        child.dom.set_referrer(croot, &base);
    }
    manuk_js::register_dom(&mut *child.dom as *mut manuk_dom::Dom);
    let mut boxed = Box::new(child);
    let croot = boxed.dom.root();
    let addr = &mut *boxed.dom as *mut manuk_dom::Dom as usize;
    // Registered ONE at a time: the rest of the child set is untouched, because this runs in the
    // MIDDLE of a script round and the parent's own map must not be rebuilt under it.
    manuk_js::add_iframe_doc(node, addr, croot);
    manuk_js::set_frame_styles_one(addr, &boxed.styles as *const _ as usize);
    // The freshly-built child must be MEASURABLE too, or `getComputedStyle` inside it answers from
    // nothing — the t1298 mechanism, extended to a child that did not exist when the registry was
    // last published. Safe because the child is boxed: its address does not depend on the vector's.
    add_frame_reflow_page(addr, &mut boxed, w, h, fonts);
    DYNAMIC_FRAMES.with(|v| v.borrow_mut().push((node, boxed, w, h)));
    true
}

/// Adopt every frame the on-demand builder created during the last script round.
///
/// ⚠ **Called from `publish_iframe_docs` and NOWHERE ELSE.** That function's contract is already
/// *"every mutation of `child_pages` republishes"*, and this is a mutation of `child_pages` — so
/// hanging it anywhere else is the *"three call sites feeding one post-step"* failure the codebase
/// has already paid for twice.
#[cfg(feature = "spidermonkey")]
fn adopt_dynamic_frames(
    child_pages: &mut std::collections::HashMap<manuk_dom::NodeId, Page>,
    iframes: &mut std::collections::HashMap<manuk_dom::NodeId, String>,
    base: &str,
) -> bool {
    let taken: Vec<_> = DYNAMIC_FRAMES.with(|v| std::mem::take(&mut *v.borrow_mut()));
    if taken.is_empty() {
        return false;
    }
    for (node, page, _, _) in taken {
        iframes.entry(node).or_insert_with(|| base.to_string());
        child_pages.insert(node, *page);
    }
    true
}

/// One network-free `<iframe>` built ahead of the page's own scripts — see
/// [`build_inline_frame_docs`]. A struct rather than a tuple because `width` is the field the whole
/// frame-reflow mechanism turns on, and a bare `f32` in fifth position is exactly the kind of thing
/// that gets re-derived at the far call site instead of carried.
#[cfg(feature = "spidermonkey")]
struct InlineFrame {
    node: manuk_dom::NodeId,
    page: Page,
    url: String,
    image: Option<std::rc::Rc<manuk_paint::DecodedImage>>,
    /// The frame's own viewport — what makes `100vw`/`100vh` inside it resolve to the frame.
    width: f32,
    height: f32,
    /// **This is the INITIAL `about:blank` of a frame that has not navigated yet**, built so a
    /// parse-time script can read `contentDocument` and so the frame counts as a child browsing
    /// context. It must NOT be entered in `Page::iframes` (that map is `pending_iframes`' "already
    /// rendered" marker — claiming it would stop the `src` from ever being fetched) and must NOT
    /// fire `load` (that event belongs to the document the `src` names). See
    /// `render_iframe_inner`'s `provisional` for the same two withheld steps on the other path.
    provisional: bool,
}

/// What the host needs to lay out ONE child document on demand — see [`frame_forced_reflow`].
#[cfg(feature = "spidermonkey")]
struct FrameReflowEntry {
    /// The child `Page`. Raw because the registry outlives no borrow: it is republished at the same
    /// two sites that republish the arena addresses, and is valid for exactly the same window.
    page: *mut Page,
    /// **The FRAME's viewport, not the window's** — the entire reason this is a separate
    /// mechanism from `ReflowScope`. Both axes: the height is what `100vh` inside the frame reads.
    width: f32,
    height: f32,
    fonts: *const FontContext,
    /// The child's `mutation_seq` when its current `styles` were computed.
    laid_out_at: u64,
    /// ⚠⚠⚠ **AND THE VIEWPORT THEY WERE COMPUTED AT — because the guard is a CONJUNCTION.**
    /// `mutation_seq` alone says *"the child's DOM has not changed"*, which is true and irrelevant
    /// when the thing that changed is the FRAME. A script that resizes an `<iframe>` and then reads
    /// a `vw` inside it mutates nothing in the child, so a seq-only guard skips the one re-cascade
    /// that was needed and the child keeps answering at its old width (t1243's lesson, and the
    /// exact shape of `css/css-values/viewport-units-invalidation`).
    laid_out_w: f32,
    laid_out_h: f32,
}

#[cfg(feature = "spidermonkey")]
thread_local! {
    /// **Which child `Page` sits behind each frame arena**, keyed the same way `set_frame_styles`
    /// keys its map — by the arena's address. Published beside it, at the same two call sites.
    static FRAME_REFLOW_PAGES: std::cell::RefCell<HashMap<usize, FrameReflowEntry>> =
        std::cell::RefCell::new(HashMap::new());
}

/// **Lay out a child document before a style read against it is answered.**
///
/// ⚠⚠⚠ **A child's cascade ran once, when the child was built, and never again** — so every node a
/// script CREATES inside a frame is missing from the child's style map and `getComputedStyle`
/// answers `undefined` for every property. Measured, with its own control row:
///
/// ```text
///   gcs=function  div=DIV  cs=object  h=undefined d=undefined   <- a div the script just created
///                                     bodyh=auto                <- CONTROL: a node that predates it
/// ```
///
/// The control is the diagnosis: the read path works and the *snapshot* is stale, which is the
/// t1282-1295 class one arena over. It is the single sentence 135 WPT viewport-unit assertions land
/// on — each of those fixtures writes `doc.body.innerHTML = …` and then measures what it wrote.
///
/// **Why this is not `forced_reflow` pointed at a child.** `ReflowCtx` carries the parent's viewport
/// width, URL and external stylesheets; borrowing it would resolve the frame's `vw` against the
/// **window**, which is the exact answer those tests exist to check. The child is laid out at its own
/// frame width instead.
///
/// **And why it is SMALLER than the main reflow, not larger.** `FRAME_STYLES` publishes the *address*
/// of `Page::styles`, so assigning into that field in place leaves the published pointer valid —
/// nothing is republished. Rects, the scroll merge, the sticky pass and grid tracks are separate
/// observables with their own call sites and are deliberately not done here.
///
/// # Safety
/// Called only from the JS bindings, with an arena address that is a key of [`FRAME_REFLOW_PAGES`];
/// entries are replaced whenever the child set changes.
#[cfg(feature = "spidermonkey")]
#[allow(clippy::too_many_arguments)]
unsafe extern "C" fn frame_forced_reflow(
    dom: *mut manuk_dom::Dom,
    border_w: f32,
    border_h: f32,
    pad_x: f32,
    pad_y: f32,
    bord_x: f32,
    bord_y: f32,
) {
    FRAME_REFLOW_PAGES.with(|r| {
        let mut reg = r.borrow_mut();
        let Some(e) = reg.get_mut(&(dom as usize)) else {
            return;
        };
        let child = unsafe { &mut *e.page };
        // ⚠ **THE FRAME'S BOX, AS OF THE PARENT LAYOUT THAT JUST RAN.** The bindings force the
        // parent's reflow before calling this and hand over the frame element's fresh BORDER box;
        // `0.0` means the parent has no box for it (not yet laid out), in which case the entry's
        // published size stands. This is what makes an `<iframe>` the script just appended — or one
        // whose class the script just changed — measure at its real width instead of the default.
        // ⚠ The padding/border sums arrive LIVE from the bindings rather than being captured when
        // the entry was published, because a frame the script created in THIS round was published
        // before the parent had ever laid it out — a captured inset would be `0` and the frame
        // would measure at its BORDER box (204px for a 200px frame). `content_from` stays the one
        // place that decides which terms come off; the bindings only supply them.
        if border_w > 0.0 && border_h > 0.0 {
            e.width = content_from(border_w, pad_x, bord_x);
            e.height = content_from(border_h, pad_y, bord_y);
        }
        // Idempotent: a run of reads with no mutation AND no resize between them lays out once, not
        // once each. Both terms — see `laid_out_w`.
        if child.dom.mutation_seq() == e.laid_out_at
            && e.width == e.laid_out_w
            && e.height == e.laid_out_h
        {
            return;
        }
        let fonts = unsafe { &*e.fonts };
        // ⚠ **THE FRAME'S OWN VIEWPORT, BOTH AXES, FOR THE DURATION OF THE CASCADE.** Viewport units
        // are resolved eagerly from a global at parse time, so without this the child re-cascades
        // with the frame's width and the WINDOW's height.
        let _vp = ViewportScope::set(e.width, e.height);
        // The child's OWN sheets and OWN base URL — the same `sheets_of` the main reflow uses, so a
        // frame's forced reflow and its next full relayout cannot disagree about the same tree.
        let sheets = sheets_of(&child.dom, &child.final_url, &child.external_css);
        let (styles, root_box) =
            restyle_and_layout(&child.dom, &sheets, fonts, e.width, &child.images);
        // ⚠ Assigned IN PLACE. `&Page::styles` is what `FRAME_STYLES` published; replacing the map's
        // contents keeps that address valid, and is what makes this reflow need no republish.
        child.styles = styles;
        child.root_box = root_box;
        child.content_height = child.root_box.content_bottom();
        e.laid_out_at = child.dom.mutation_seq();
        e.laid_out_w = e.width;
        e.laid_out_h = e.height;
    });
}

thread_local! {
    /// How deep the *pre-script* inline-frame pass currently is on this thread.
    ///
    /// `render_iframe_with_type` carries an explicit `depth` parameter, but the pre-script pass runs
    /// inside `from_dom`, which has no such parameter and is re-entered by `Page::load` on the child.
    /// An `<iframe srcdoc="<iframe srcdoc=…>">` would therefore recurse without a bound. This is that
    /// bound, and it uses the same `MAX_IFRAME_DEPTH` the fetched path does so the two agree.
    static INLINE_FRAME_DEPTH: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

/// **Build the documents of the network-free `<iframe>`s BEFORE the page's own scripts run.**
///
/// ⚠⚠⚠ **The frames were being loaded one whole phase after the only moment the page looks.**
/// `load_inline_frames` — `srcdoc`, `src="about:blank"`, and no `src` at all — is reachable only
/// from `fetch_and_load_iframes`, which `load_async` calls **after `DOMContentLoaded`**. But
/// `from_dom` runs the document's *blocking* scripts, and HTML §4.8.5 says a srcless frame is
/// navigated to `about:blank` **when the element is inserted**, i.e. during parsing. So the four
/// lines every WPT viewport-unit test opens with —
///
/// ```html
///   <iframe id=iframe></iframe>
///   <script> const doc = iframe.contentDocument; …​ </script>
/// ```
///
/// — read `null`, throw, and score **0 out of N** (t1266's shape). `G_INLINE_FRAME_DOCUMENT` banks
/// all three frame kinds as working and is not wrong: they *are* readable at `load`, at
/// `DOMContentLoaded`, from any later task. The one moment nobody had measured is the page's own
/// parse-time script, and it is the only moment those tests ever look.
///
/// This is t1296's finding in a second subsystem: the work is done, correctly, one phase too late
/// for the page's own code to see it — and, exactly as with the stylesheet, the final paint is
/// right, so no screenshot or box dump could ever show it.
///
/// Returns one [`InlineFrame`] per frame; the caller owns installation, because the `Page` that will
/// hold them does not exist yet.
#[cfg(feature = "spidermonkey")]
fn build_inline_frame_docs(
    dom: &Dom,
    rects: &std::collections::HashMap<manuk_dom::NodeId, [f32; 4]>,
    styles: &StyleMap,
    final_url: &str,
    fonts: &FontContext,
) -> Vec<InlineFrame> {
    if INLINE_FRAME_DEPTH.with(|d| d.get()) >= MAX_IFRAME_DEPTH {
        return Vec::new();
    }
    // The same three-way classification `load_inline_frames` makes, and it must stay the same three:
    // a frame this pass claims but does not build would be skipped by BOTH passes.
    // `srcdoc` beats `src` (HTML §4.8.5), including a `src` that points somewhere real.
    let mut work: Vec<(manuk_dom::NodeId, String, bool)> = Vec::new();
    for n in dom.flat_descendants(dom.root()) {
        if dom.tag_name(n) != Some("iframe") {
            continue;
        }
        let Some(el) = dom.element(n) else { continue };
        if let Some(doc) = el.attr("srcdoc") {
            work.push((n, doc.to_string(), false));
            continue;
        }
        let src = el.attr("src").unwrap_or("").trim();
        if src.is_empty() || src.eq_ignore_ascii_case("about:blank") {
            work.push((n, String::new(), false));
        } else if !src.starts_with("javascript:") {
            // ⚠⚠⚠ **AND THE FOURTH KIND, WHICH WAS NOT A KIND: A FRAME THAT HAS NOT NAVIGATED YET.**
            // HTML gives an `<iframe>` a child browsing context at INSERTION, holding `about:blank`,
            // and swaps it when `src` lands. This pass built the three network-free kinds and left a
            // `src`-bearing frame with NO DOCUMENT until the fetch returned — so a parse-time script
            // read `contentDocument === null`, and `typeof null` is `"object"`, so the check every
            // embed writes saw a document that was not there. It also under-counted the frame tree:
            // no document means no child browsing context, so `window.length` and `window.frames[i]`
            // skipped it (t1349). Chrome-measured, one `src` frame that never resolves plus one bare:
            //
            // ```text
            //                                  Chrome    before
            //   s.contentDocument === null       false      true
            //   window.length                        2         1
            //   typeof window[1]                object  undefined
            // ```
            //
            // ⚠ Built EMPTY and marked provisional. The comment above — *"a frame this pass claims
            // but does not build would be skipped by BOTH passes"* — is exactly why this is a build
            // and not a `continue`, and why `provisional` is a flag rather than an omission.
            work.push((n, String::new(), true));
        }
    }
    if work.is_empty() {
        return Vec::new();
    }
    INLINE_FRAME_DEPTH.with(|d| d.set(d.get() + 1));
    let mut out = Vec::new();
    for (node, html, provisional) in work {
        // The frame's own viewport width, so its media queries and layout see the frame — the
        // `300x150` fallback is the HTML default an unsized `<iframe>` gets, and it is what
        // `render_iframe_with_type` uses for a frame with no box.
        // ⚠ The child's viewport is the frame's CONTENT box — see `frame_content_width`. `rects`
        // is the BORDER box, and a default `<iframe>` carries a 2px UA border on each side.
        let (w, h, visible) = match rects.get(&node) {
            Some([_, _, rw, rh]) if *rw >= 1.0 && *rh >= 1.0 => {
                let (cw, ch) = frame_content_box(styles.get(&node), *rw, *rh);
                (cw.round().max(1.0) as u32, ch.round().max(1.0) as u32, true)
            }
            // The HTML default for an `<iframe>` with no box of its own.
            _ => (300, 150, false),
        };
        // ⚠ The viewport unit scope wraps the child's WHOLE construction: `Page::load` cascades,
        // and `100vh` inside the frame is resolved at parse time from the global.
        let mut child = {
            let _vp = ViewportScope::set(w as f32, h as f32);
            Page::load(&html, final_url, fonts, w as f32)
        };
        let img = if visible {
            let canvas = child.paint(fonts, w, h);
            Some(std::rc::Rc::new(manuk_paint::DecodedImage {
                width: canvas.width(),
                height: canvas.height(),
                rgba: canvas.rgba_bytes().to_vec(),
            }))
        } else {
            None
        };
        // **A framed document's referrer is its EMBEDDER** — set here for the same reason
        // `render_iframe_with_type` sets it before `fire_frame_load`: after is after the only
        // moment anyone looks.
        {
            let doc = child.dom.root();
            child.dom.set_referrer(doc, final_url);
        }
        manuk_js::register_dom(&mut *child.dom as *mut manuk_dom::Dom);
        out.push(InlineFrame {
            node,
            page: child,
            url: final_url.to_string(),
            image: img,
            height: h as f32,
            // ⚠ Carried, not re-derived. The frame's own width is what makes `100vw` inside it
            // resolve to the frame; a second copy of the `300x150`-fallback rule elsewhere is how
            // two call sites end up disagreeing about the same frame (the "one rule, N
            // implementations" class).
            width: w as f32,
            provisional,
        });
    }
    INLINE_FRAME_DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
    out
}

/// Publish the child arenas + their style maps to JS, from a set the `Page` does not own yet.
///
/// ⚠ The addresses published here are `Box<Dom>` (stable across the map move into `Page`) and
/// `&Page::styles` **inside a `HashMap` value** (stable only until that map is next mutated).
/// `Page::publish_iframe_docs` is called again the moment the `Page` exists for exactly that reason
/// — this call covers the window *before* it, which is the window the blocking scripts run in.
#[cfg(feature = "spidermonkey")]
fn publish_pre_script_frame_docs(frames: &mut [InlineFrame], fonts: &FontContext) {
    let docs: std::collections::HashMap<_, _> = frames
        .iter_mut()
        .map(|f| {
            let root = f.page.dom.root();
            (
                f.node,
                (&mut *f.page.dom as *mut manuk_dom::Dom as usize, root),
            )
        })
        .collect();
    manuk_js::set_iframe_docs(docs);
    let styles: std::collections::HashMap<usize, usize> = frames
        .iter_mut()
        .map(|f| {
            (
                &mut *f.page.dom as *mut manuk_dom::Dom as usize,
                &f.page.styles as *const _ as usize,
            )
        })
        .collect();
    manuk_js::set_frame_styles(styles);
    // …and the registry that lets a style read LAY THE CHILD OUT — published here for the same
    // reason and with the same lifetime as the two maps above. Without it the frames are readable
    // and frozen at their construction cascade.
    publish_frame_reflow_pages(
        frames.iter_mut().map(|f| {
            let addr = &mut *f.page.dom as *mut manuk_dom::Dom as usize;
            (addr, &mut f.page, f.width, f.height)
        }),
        fonts,
    );
}

/// Add ONE entry to [`FRAME_REFLOW_PAGES`] without disturbing the rest — the on-demand path's
/// counterpart to [`publish_frame_reflow_pages`], which replaces wholesale. Wholesale is right when
/// the host republishes the whole child set and wrong mid-script, when one frame is being born.
#[cfg(feature = "spidermonkey")]
fn add_frame_reflow_page(
    addr: usize,
    page: &mut Page,
    width: f32,
    height: f32,
    fonts: &FontContext,
) {
    let e = FrameReflowEntry {
        width: width.max(1.0),
        height: height.max(1.0),
        page: page as *mut Page,
        fonts: fonts as *const FontContext,
        laid_out_at: page.dom.mutation_seq(),
        laid_out_w: width.max(1.0),
        laid_out_h: height.max(1.0),
    };
    FRAME_REFLOW_PAGES.with(|r| {
        r.borrow_mut().insert(addr, e);
    });
}

/// Publish [`FRAME_REFLOW_PAGES`] from whatever currently owns the child pages — a local `Vec`
/// before the `Page` exists, `Page::child_pages` after it. Both call sites hand over the same three
/// facts: which arena, which `Page`, and **the frame's own width**.
#[cfg(feature = "spidermonkey")]
fn publish_frame_reflow_pages<'a>(
    entries: impl Iterator<Item = (usize, &'a mut Page, f32, f32)>,
    fonts: &FontContext,
) {
    // ⚠ The previous entries are read, not discarded: `width`/`height` below are what the frame
    // IS now, while `laid_out_*` is what the child was last cascaded at. Recomputing `laid_out_*`
    // from the new width here would assert the re-cascade had already happened and skip it forever
    // — the guard would be self-satisfying.
    let prev: HashMap<usize, (u64, f32, f32)> = FRAME_REFLOW_PAGES.with(|r| {
        r.borrow()
            .iter()
            .map(|(k, e)| (*k, (e.laid_out_at, e.laid_out_w, e.laid_out_h)))
            .collect()
    });
    let m: HashMap<usize, FrameReflowEntry> = entries
        .map(|(addr, c, width, height)| {
            let (lat, lw, lh) = prev.get(&addr).copied().unwrap_or((
                c.dom.mutation_seq(),
                width.max(1.0),
                height.max(1.0),
            ));
            let e = FrameReflowEntry {
                // ⚠ The width is passed in, never re-derived from the child, because it is the one
                // fact this mechanism exists for: the child laid out at its FRAME's width when it
                // was built and must keep doing so, which is what makes `100vw` inside a 200px
                // frame resolve to 200px instead of the window's width.
                width: width.max(1.0),
                height: height.max(1.0),
                page: c as *mut Page,
                fonts: fonts as *const FontContext,
                laid_out_at: lat,
                laid_out_w: lw,
                laid_out_h: lh,
            };
            (addr, e)
        })
        .collect();
    FRAME_REFLOW_PAGES.with(|r| *r.borrow_mut() = m);
}

/// The document's author stylesheets, in **document order**, with the already-fetched externals
/// woven in at the position their `<link>` occupies.
///
/// ⚠⚠ **The walk is the LIGHT tree, deliberately** — matching
/// [`MinimalCascade::collect_style_elements`], the function this replaces. `cascade_styles` adds
/// shadow-root sheets separately *and scoped* (`collect_shadow_stylesheets`); walking the flat tree
/// here would cascade every shadow `<style>` a second time as an unscoped document sheet.
fn initial_sheets_with_external(
    dom: &Dom,
    base: &str,
    external: &HashMap<String, String>,
) -> Vec<Stylesheet> {
    let mut out = Vec::new();
    for n in dom.descendants(dom.root()) {
        match dom.tag_name(n) {
            Some("style") => {
                let media = dom
                    .element(n)
                    .and_then(|e| e.attr("media"))
                    .map(str::to_string);
                out.push(Stylesheet::parse(&wrap_media(&dom.text_content(n), &media)));
            }
            Some("link") => {
                let Some(el) = dom.element(n) else { continue };
                let is_sheet = el
                    .attr("rel")
                    .map(|r| {
                        r.split_ascii_whitespace()
                            .any(|t| t.eq_ignore_ascii_case("stylesheet"))
                    })
                    .unwrap_or(false);
                if !is_sheet {
                    continue;
                }
                let Some(href) = el.attr("href").map(str::trim).filter(|h| !h.is_empty()) else {
                    continue;
                };
                // A sheet that has NOT arrived is simply absent from the map and contributes
                // nothing — the same fall-through as before, now expressed per-sheet instead of
                // per-document.
                if let Some(css) = external.get(&resolve_url(base, href)) {
                    let media = el.attr("media").map(str::to_string);
                    out.push(Stylesheet::parse(&wrap_media(css, &media)));
                }
            }
            _ => {}
        }
    }
    out
}

/// Seed the `<script>` nodes whose source came from the network, for the next page built on this
/// thread. They are the ones that owe a `load` event once they run — see
/// `PageContext::fire_external_script_load`.
///
/// Call this **before** constructing the page; `from_dom` takes it exactly once, so a set seeded
/// here can never fire a `load` in the navigation after it.
pub fn set_pending_external_scripts(nodes: std::collections::HashSet<NodeId>) {
    PENDING_EXTERNAL_SCRIPTS.with(|p| *p.borrow_mut() = nodes);
}

/// Seed the Content-Security-Policy the next page built on this thread will enforce.
///
/// Call this **before** constructing the page. Every construction path consumes it exactly once, so
/// a policy seeded here can never leak into the navigation after it.
pub fn set_pending_csp(csp: manuk_net::csp::Csp) {
    set_pending_csp_with_authorized(csp, Vec::new());
}

/// As [`set_pending_csp`], plus the `<script>` nodes whose source was **already authorized by URL**
/// when it was fetched.
///
/// Those nodes need the exemption because of how external scripts are run here: the fetched text is
/// inlined into the element and the `src` dropped, so at execution time an external script looks
/// exactly like an author-written inline one. Without this it would be judged a second time, by the
/// *inline* rules — and a script served from an allowed origin would be blocked for the crime of not
/// carrying a nonce it was never required to have. One load, one decision, made where the URL is
/// still known.
pub fn set_pending_csp_with_authorized(csp: manuk_net::csp::Csp, authorized: Vec<NodeId>) {
    PENDING_CSP.with(|p| *p.borrow_mut() = Some((csp, authorized)));
}

/// Take the seeded policy and install its inline-script check into the JS host, **unconditionally**
/// — including installing "no policy" when nothing was seeded. Clearing is the load-bearing half:
/// the hook is a thread-local, so a policy left installed by the previous navigation would be
/// enforced against a document that never sent one, blocking scripts on an innocent page.
fn install_csp_for_next_page(dom: &Dom, final_url: &str) -> manuk_net::csp::Csp {
    let (mut csp, authorized) = PENDING_CSP
        .with(|p| p.borrow_mut().take())
        .unwrap_or_else(|| (manuk_net::csp::Csp::none(), Vec::new()));
    // The `<meta>` fold happens here as well as at the fetch sites, and that redundancy is
    // deliberate: the fetch sites need the policy BEFORE construction (to not issue a request), and
    // this is the one place EVERY construction path passes through — including the synchronous
    // `Page::load`, which has no fetch site at all. Re-adding an identical policy is a no-op,
    // because policies compose by conjunction and a policy ANDed with itself decides the same way.
    csp.set_document_url(Url::parse(final_url).ok().as_ref());
    for content in collect_meta_csp(dom) {
        csp.add_meta(&content);
    }
    if csp.restricts_scripts() {
        let policy = csp.clone();
        let exempt: std::collections::HashSet<NodeId> = authorized.into_iter().collect();
        manuk_js::set_csp_inline_hook(Some(Box::new(move |node, nonce| {
            exempt.contains(&node) || policy.allows_inline_script(nonce)
        })));
    } else {
        manuk_js::set_csp_inline_hook(None);
    }
    csp
}

impl Page {
    /// Parse + style + lay out `html` for a viewport of `viewport_width` px.
    /// As [`load`](Self::load), but the document is born knowing its window id and opener, so its
    /// **load-time** scripts can read `window.opener`.
    ///
    /// `set_identity` cannot serve that case — it only exists once the page's render-blocking
    /// scripts have already run. A popup's login script reads `window.opener` at load time to post
    /// its token back, so with the late seeding it read `null`, posted nothing, and the opener
    /// waited on its callback forever with nothing thrown.
    pub fn load_with_identity(
        html: &str,
        final_url: &str,
        fonts: &FontContext,
        viewport_width: f32,
        win_id: u64,
        opener_win: u64,
    ) -> Page {
        manuk_js::set_pending_identity(win_id, opener_win);
        Page::load(html, final_url, fonts, viewport_width)
    }

    pub fn load(html: &str, final_url: &str, fonts: &FontContext, viewport_width: f32) -> Page {
        Page::load_dom(manuk_html::parse(html), final_url, fonts, viewport_width)
    }

    /// **The same load, from an XML parse.** `XML is not HTML with different tags`
    /// (`manuk_html::parse_xml`): `<Foo/>` and `<foo/>` are distinct elements, `xmlns` gives real
    /// namespaces, and an unclosed tag is a fatal error rather than something to recover from.
    ///
    /// This exists because an `<iframe src="…​.xml">` was handed to the HTML parser, which lowercases
    /// every tag and never reports the error — so a framed XML document's `documentElement` was the
    /// wrong element with the wrong name. RSS/Atom, SVG documents and XHTML pages all arrive through
    /// exactly that path.
    ///
    /// ⚠ Deliberately a sibling of `load`, both delegating to [`Page::load_dom`], rather than a second
    /// copy of the load sequence — the shape t1207 named: *one rule, two implementations* is how the
    /// two silently diverge, and everything after the parse (deferred scripts, DOMContentLoaded,
    /// inline-SVG rasterization, `load`) is identical for both.
    pub fn load_xml(src: &str, final_url: &str, fonts: &FontContext, viewport_width: f32) -> Page {
        Page::load_dom(
            manuk_html::parse_xml(src).dom,
            final_url,
            fonts,
            viewport_width,
        )
    }

    /// The load sequence, from an already-parsed document. Shared by [`Page::load`] and
    /// [`Page::load_xml`]; the ONLY difference between them is which parser produced the `Dom`.
    pub fn load_dom(
        dom: manuk_dom::Dom,
        final_url: &str,
        fonts: &FontContext,
        viewport_width: f32,
    ) -> Page {
        let mut page = Page::from_dom(dom, final_url, fonts, viewport_width);
        // **Both passes, back to back.** `from_dom` runs only the scripts that block first paint; the
        // deferred ones (`defer`, `async`, `type=module`) run here. So this function behaves exactly as
        // it always has — every gate, and the whole SPA suite, sees all scripts run before it asserts
        // anything.
        //
        // The SHELL is the only caller that separates them: blocking → paint → deferred → repaint. That
        // is the only place a human is waiting, and it is the only place the difference is visible.
        page.run_deferred_scripts(fonts, viewport_width);
        // Parsing is done and the deferred scripts have executed — that IS DOMContentLoaded.
        page.fire_lifecycle("DOMContentLoaded", fonts, viewport_width);
        // On the SYNC path there is no async runtime to fetch subframes with; a caller that needs a
        // frame's document (a gate) drives `render_iframe` directly. The async path
        // (`load_async`/`finish_loading`) is where real frames load.
        //
        // Inline `<svg>` needs no network either — rasterize on this path too (tick 394), so
        // gates and shell navigations paint vectors, not only the fetch path via `apply_images`.
        page.rasterize_inline_svgs_into_images();
        page.fire_lifecycle("load", fonts, viewport_width);
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
        // ── **THE PHASE LEDGER DID NOT SUM TO THE LOAD, AND SAID IT DID** (tick 678). ────────────
        //
        // `finish_loading`'s ledger (below) was built at t670 on the stated principle that *"the
        // per-phase times sum to the total; a set of parts that does not sum to its whole is the
        // accounting reconciliation this project rates as its highest-yield instrument."* Measured on
        // `playhop.com` it accounts for **12.2s of a 27.6s load** — 56% of the time is outside the
        // ledger, because the ledger only ever covered `finish_loading` while the navigation also
        // spends `load_async`: the external-script fetch, the module-graph prefetch, parse + cascade
        // + layout + blocking scripts, the deferred pass, and the subframes.
        //
        // So the ledger now spans the navigation, and it **closes with the subtraction printed** —
        // `total_ms` vs `accounted_ms` vs `unaccounted_ms` — rather than leaving a reader to notice
        // that two numbers on different lines do not agree. That is the difference between an
        // instrument whose parts sum and one that merely asserts they do; t669 spent a tick on a
        // cluster that named the phase where time was spent rather than the one that spent it, and
        // this is the same failure in the other direction.
        //
        // Same event shape as the budgeted phases (`load phase done`, `phase`/`ms`/`gave_up`), so
        // `RUST_LOG=manuk_page=info` prints ONE continuous ledger for the whole navigation and every
        // existing grep keeps working. `info`, so asking the question again needs no rebuild.
        // **A NAVIGATION FORGETS WHAT THE LAST ONE COULD NOT FETCH** (tick 683). The per-navigation
        // negative cache in `manuk-net` was cleared by exactly two callers, neither of them on the
        // navigation path, so it was per-PROCESS: the second load of a site inherited every subresource
        // failure of the first. Measured on `www.agoda.com`, three consecutive loads in one process —
        // `external scripts` 1072ms → 9ms → 4ms, and 65 shared paths → 10 → 10. Silently, because from
        // the fetch layer's side that is the feature working. See `manuk_net::begin_navigation`.
        //
        // Here and not in `Page::load`, deliberately: `render_iframe` calls `load` for a SUBFRAME, and
        // resetting there would clear the parent navigation's state in the middle of it.
        manuk_net::begin_navigation();
        // ⚠⚠⚠ **AND THE CONVERGENCE FLAG, WHICH HAD NO PRODUCTION CALLER AT ALL.**
        //
        // `clear_convergence_state()` resets *"has a drain given up"*, and its own doc says the reset
        // lives with the round loop **on purpose** — *"a flag whose reset lives somewhere else is a
        // flag that eventually leaks across navigations and silently stops a healthy page from
        // running its scripts."* The hazard was named exactly right and then landed anyway: outside
        // the round loop the flag was never cleared, so a single non-converging page poisoned every
        // LATER navigation in the process — the round loop stopped asking, permanently.
        //
        // It was latent because the flag had exactly one consumer. t1330 gave it a second (the drain
        // ceiling itself), which would have turned a silent staleness into every subsequent page
        // getting a 250-task grace it never earned. **A new consumer is what makes a stale flag
        // visible**, and the answer is the reset the comment predicted, at the one place that means
        // "a new page": beside `begin_navigation`.
        #[cfg(feature = "spidermonkey")]
        manuk_js::event_loop::clear_convergence_state();
        let nav_started = std::time::Instant::now();
        let mut nav_at = nav_started;
        let mut nav_accounted_ms = 0u64;
        let mut nav_hits = drain_ceiling_hits_or_zero();
        let mut nav_phase = |name: &'static str,
                             at: &mut std::time::Instant,
                             accounted: &mut u64,
                             hits_before: &mut usize| {
            let hits = drain_ceiling_hits_or_zero();
            let ms = at.elapsed().as_millis() as u64;
            *accounted += ms;
            tracing::info!(
                phase = name,
                ms,
                gave_up = hits.saturating_sub(*hits_before),
                elapsed_ms = nav_started.elapsed().as_millis() as u64,
                "load phase done"
            );
            *hits_before = hits;
            *at = std::time::Instant::now();
        };
        // Preload scanner: kick off render-blocking subresource fetches (external CSS,
        // <link rel=preload>) concurrently *before* parse + layout + scripts run, so they
        // land in the HTTP cache by the time apply_stylesheets needs them — overlapping the
        // network with the CPU-bound pipeline.
        //
        // ── **THE AUTHOR'S STYLESHEETS: FETCHED HERE, WAITED FOR NOWHERE.**
        //
        // `from_prefetched` — the SHELL's path — has always applied the CSS between `from_dom` and
        // the deferred pass, so its lifecycle events and its timers see a styled document.
        // `load_async` applied it in `finish_loading`, *after* `DOMContentLoaded` and *after*
        // `load`, so every script on this path measured a document with no author CSS in it. That is
        // the t714 finding, and its blast radius is this path — the AGENT and every fidelity
        // measurement — not the shell.
        //
        // ⚠⚠ **Two designs were measured and both were reverted, and the arithmetic is why.**
        // `G_LOAD` bounds the WHOLE page at 2x the load budget; one budget is already spent in
        // `load_async` and one in `finish_loading`, so a third *waiting* phase has no room: give it
        // its own budget and the page spends 3x (G_LOAD red at 5.4s against 2s, t715); make it share
        // one and the phase it shares with starves (`keirin.jp` SHAPE 60.2% -> 53.0%, t716). Even
        // running it *concurrently* with the script fetch is not free, because the fixture that
        // measures this has dead sheets and **no scripts** — there was no existing wait to hide
        // behind, and the phase went 0s -> 2s.
        //
        // So the answer is not a better slice, it is **no wait at all**: start the fetches here,
        // before the parser has run, and at the apply point take only the ones that have ALREADY
        // FINISHED. A page's stylesheets get parse + cascade + layout + every blocking script of head
        // start — seconds, on a real site — and a sheet that has not arrived by then simply falls
        // through to `finish_loading` exactly as it does today. Strictly more capability, zero added
        // latency, and nothing to starve.
        let mut css_inflight: Vec<(String, tokio::task::JoinHandle<Option<String>>)> = Vec::new();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            for url in scan_preloads(html, final_url) {
                handle.spawn(async move {
                    let _ = manuk_net::fetch(&url).await;
                });
            }
        }
        #[allow(unused_mut)]
        let mut dom = manuk_html::parse(html);
        // Off the real tree rather than a regex over the bytes: `collect_style_sources` already knows
        // about `media`, shadow roots and inline `<style>`, and getting the URL list wrong here would
        // fetch the wrong thing early and still look like it worked. The head start is everything
        // between here and the apply — the external-script fetch, the module-graph prefetch, the
        // cascade, layout, and every blocking script — which on a real site is seconds.
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let mut seen = std::collections::HashSet::new();
            for url in collect_style_sources(&dom, final_url)
                .iter()
                .filter_map(|src| match src {
                    StyleSource::External(u, _) => Some(u.clone()),
                    _ => None,
                })
                .filter(|u| seen.insert(u.clone()))
            {
                let u = url.clone();
                css_inflight.push((
                    url,
                    handle.spawn(async move {
                        manuk_net::fetch(&u)
                            .await
                            .ok()
                            .and_then(|r| subresource_text(&r))
                    }),
                ));
            }
        }
        nav_phase(
            "html parse",
            &mut nav_at,
            &mut nav_accounted_ms,
            &mut nav_hits,
        );
        // Where each inlined external script CAME FROM, produced by the fetch inside the block below
        // and read by the module-graph walk after it. Declared out here because the two are in
        // different scopes and a module's imports resolve against the MODULE's url, not the document's.
        #[allow(unused_mut)]
        let mut script_origins: HashMap<NodeId, String> = HashMap::new();
        // The external stylesheets, fetched beside the scripts below and applied right after
        // construction — the same shape `Prefetched.css` has on the shell path.
        #[allow(unused_mut)]
        let mut early_css: HashMap<String, String> = HashMap::new();
        // Fold this document's `<meta>` policy into whatever the caller seeded from the response
        // headers, and leave the result seeded for `from_dom` — the script FETCH below needs it
        // now, and the inline-script check needs it a few lines later.
        {
            // Peek, do not take: `from_dom` below consumes the seed. Only the POLICY is needed
            // here — the authorized set is what this block is about to produce.
            let mut csp = PENDING_CSP
                .with(|p| p.borrow().as_ref().map(|(c, _)| c.clone()))
                .unwrap_or_else(Csp::none);
            csp.set_document_url(Url::parse(final_url).ok().as_ref());
            for content in collect_meta_csp(&dom) {
                csp.add_meta(&content);
            }
            #[cfg(feature = "spidermonkey")]
            let authorized = {
                let (a, origins) = fetch_external_scripts(&mut dom, final_url, &csp).await;
                script_origins = origins;
                a
            };
            #[cfg(not(feature = "spidermonkey"))]
            let authorized: Vec<NodeId> = Vec::new();
            set_pending_csp_with_authorized(csp, authorized);
        }
        nav_phase(
            "external scripts",
            &mut nav_at,
            &mut nav_accounted_ms,
            &mut nav_hits,
        );
        // **Pre-fetch the ES-module import graph off the UI thread, before any module runs (B3b).** An
        // inline `<script type=module>` may `import` a relative graph of sibling modules; `ModuleLink`
        // (below, inside `run_deferred_scripts`) is synchronous with no network, so the whole reachable
        // graph must be in hand first — exactly the reason `fetch_external_scripts` above pre-fetches
        // classic `<script src>`. The resolved-url → source map is handed to the JS layer just before the
        // deferred pass; `run_module` drives the population walk over it so every `import` resolves. A
        // page with no module imports yields an empty map ⇒ modules link self-contained, as before.
        #[cfg(feature = "spidermonkey")]
        let import_map = extract_import_map(&dom);
        #[cfg(feature = "spidermonkey")]
        let module_graph_sources =
            prefetch_module_graph(&dom, final_url, &import_map, &script_origins).await;
        #[cfg(feature = "spidermonkey")]
        nav_phase(
            "module graph prefetch",
            &mut nav_at,
            &mut nav_accounted_ms,
            &mut nav_hits,
        );
        // The external `<script src>` set, seeded before construction because `from_dom` runs the
        // blocking scripts and each one owes its element a `load` the instant it has executed.
        set_pending_external_scripts(script_origins.keys().copied().collect());
        // **HARVEST ONCE HERE TOO — because `from_dom` runs the blocking scripts.** The harvest
        // below is unchanged and still runs; this one is strictly additional, takes only handles
        // that are ALREADY FINISHED, and therefore adds no latency by exactly the argument the
        // comment below makes for itself. What it buys is order: a sheet that landed during parse,
        // the external-script fetch and the module-graph prefetch — which on this path is nearly
        // all of them, since every one of those phases is network — is in the FIRST cascade, so the
        // document's own scripts measure the author's page instead of the UA default.
        //
        // ⚠⚠⚠ **AND ON A FAST ORIGIN THE FREE HARVEST ALONE IS NOT ENOUGH — MEASURED, NOT ASSUMED.**
        // The probe that found this runs against a loopback server: `html parse` 0ms, `external
        // scripts` 2ms, `module graph prefetch` 0ms — so the head start the free harvest relies on
        // is **2ms**, the sheet lands at ~5ms, and the harvest banks nothing. The page still read
        // `display: block` from a `<link>` that said `grid`.
        //
        // So this waits, and the North Star is why. *"A speed advantage is only real if it comes
        // from doing the same work better — and not from not doing the work at all"*; `<link
        // rel=stylesheet>` is script-blocking in every engine, and **"fast because we never waited
        // for the stylesheet" is the third member of a list this project has already caught twice**
        // (never loaded the images, never ran the script). Skipping it does not merely paint early:
        // it hands the page's own code UA-default geometry to compute with.
        //
        // ⚠ **BOUNDED THREE WAYS, so it cannot become the phase that t715/t716 reverted:** it is
        // skipped entirely unless a blocking script exists to observe it; it never runs past the
        // navigation's own `load_budget()` (so `G_LOAD`'s 2x page bound is untouched); and it never
        // spends more than a quarter of that budget in one go. Sheets that miss the deadline are
        // not lost — `finish_loading` picks them up exactly as it does today.
        if !css_inflight.is_empty() && document_has_blocking_script(&dom) {
            let budget = load_budget();
            let deadline =
                std::cmp::min(nav_started + budget, std::time::Instant::now() + budget / 4);
            let futs = std::mem::take(&mut css_inflight)
                .into_iter()
                .map(|(url, h)| async move { (url, h.await.ok().flatten()) });
            // `collect_before_deadline`, not `join_all`: a sheet that landed is banked even when a
            // sibling never answers — the same reason `finish_loading` uses it.
            for (url, text) in collect_before_deadline(futs, Some(deadline)).await {
                if let Some(t) = text {
                    early_css.insert(url, t);
                }
            }
        }
        harvest_finished_css(&mut css_inflight, &mut early_css);
        nav_phase(
            "blocking stylesheets",
            &mut nav_at,
            &mut nav_accounted_ms,
            &mut nav_hits,
        );
        if !early_css.is_empty() {
            set_pending_external_css(early_css.clone());
        }
        let mut page = Page::from_dom(dom, final_url, fonts, viewport_width);
        // `from_dom` is parse-to-first-layout AND the scripts that block first paint: cascade,
        // layout, and every blocking `<script>` with its drain. On an app-web page this is usually
        // the largest single phase and it was entirely outside the ledger.
        nav_phase(
            "cascade+layout+blocking scripts",
            &mut nav_at,
            &mut nav_accounted_ms,
            &mut nav_hits,
        );
        // **Applied HERE — after construction, before the deferred pass and before either lifecycle
        // event — which is exactly where `from_prefetched` has always applied it.** Pure CPU: the
        // bytes are already in hand from the phase above, so this spends no budget and cannot starve
        // anything. `finish_loading` still calls `fetch_and_apply_stylesheets`, and it is now a
        // near-no-op: the URLs are deduped for the navigation and `apply_stylesheets` fingerprints
        // its inputs, so an unchanged sheet set does not re-cascade.
        // **Take only what has already landed — `is_finished()`, never `await`.** A handle still in
        // flight is left alone: it keeps running, warms the HTTP/negative cache, and
        // `finish_loading` picks the sheet up on its own phase. This loop cannot block.
        harvest_finished_css(&mut css_inflight, &mut early_css);
        if !early_css.is_empty() {
            tracing::debug!(
                sheets = early_css.len(),
                "author CSS applied before the lifecycle events"
            );
            page.apply_stylesheets(&early_css, fonts, viewport_width);
        }
        // Carry the pre-fetched graph + import map onto the page; `run_deferred_scripts` seeds them into
        // the JS layer and clears them after the module pass (the same seam the shell path uses).
        #[cfg(feature = "spidermonkey")]
        {
            page.module_graph_sources = module_graph_sources;
            page.module_node_bases = script_origins
                .iter()
                .map(|(n, u)| (n.index(), u.clone()))
                .collect();
            page.import_map = import_map;
        }
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
        nav_phase(
            "deferred scripts",
            &mut nav_at,
            &mut nav_accounted_ms,
            &mut nav_hits,
        );
        // Parsing is done and the deferred scripts have executed — that IS DOMContentLoaded.
        page.fire_lifecycle("DOMContentLoaded", fonts, viewport_width);
        nav_phase(
            "DOMContentLoaded",
            &mut nav_at,
            &mut nav_accounted_ms,
            &mut nav_hits,
        );
        // **Subframes load BEFORE `load` fires** — `load` waits for subframes (HTML spec), and a page's
        // `<body onload>` is precisely where it reaches into them. Firing `load` first made the entire
        // `encoding` suite (767k subtests) read a not-yet-loaded frame and throw. Idempotent with the
        // pass in `finish_loading`: `pending_iframes` skips any frame already rendered.
        #[cfg(feature = "spidermonkey")]
        page.fetch_and_load_iframes(fonts, viewport_width).await;
        #[cfg(feature = "spidermonkey")]
        nav_phase(
            "subframes (pre-load)",
            &mut nav_at,
            &mut nav_accounted_ms,
            &mut nav_hits,
        );
        // The subresource phases have not run yet, but the document and its frames are ready, which is
        // what `load` waits for. The call is idempotent, so `finish_loading` firing it again is harmless.
        page.fire_lifecycle("load", fonts, viewport_width);
        nav_phase(
            "load event",
            &mut nav_at,
            &mut nav_accounted_ms,
            &mut nav_hits,
        );

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
        // The fan-outs inside stop WAITING here and spend what is left committing — see
        // `COMMIT_RESERVE`. Without it the `timeout` below drops the decode-and-apply along with the
        // fetch, and the images this phase already downloaded never reach the page.
        page.load_deadline =
            Some(std::time::Instant::now() + budget.saturating_sub(COMMIT_RESERVE));
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
        nav_phase(
            "initial images+masks",
            &mut nav_at,
            &mut nav_accounted_ms,
            &mut nav_hits,
        );
        page.load_deadline = None;
        // **THE SUBTRACTION, PRINTED.** The parts must sum to the whole, and the only way that claim
        // stays true is if the instrument computes the difference itself and says so on every load.
        // `unaccounted_ms` above a few milliseconds means a phase exists that this ledger does not
        // name — which is exactly the state `load_async` was in for eight ticks while
        // `finish_loading`'s ledger was trusted as the whole picture.
        let nav_total_ms = nav_started.elapsed().as_millis() as u64;
        tracing::info!(
            total_ms = nav_total_ms,
            accounted_ms = nav_accounted_ms,
            unaccounted_ms = nav_total_ms.saturating_sub(nav_accounted_ms),
            "navigation phases reconciled (load_async; finish_loading keeps its own ledger)"
        );
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
        self.fire_lifecycle("load", fonts, viewport_width);
    }

    #[cfg(feature = "spidermonkey")]
    async fn finish_loading_inner(&mut self, fonts: &FontContext, viewport_width: f32) {
        let budget = load_budget();
        let started = std::time::Instant::now();
        // The instant the fan-outs must stop waiting, held back by `COMMIT_RESERVE` so what already
        // arrived is decoded and applied INSIDE the `timeout` that wraps this whole sequence.
        self.load_deadline = Some(started + budget.saturating_sub(COMMIT_RESERVE));
        // **PER-PHASE ACCOUNTING, because three ticks in a row guessed and two of them guessed wrong.**
        //
        // `finish_loading` has one budget and seven phases, and until now the only thing it reported
        // was *which phase noticed the budget was gone* — which is the phase that ran FIRST AFTER it
        // was spent, not the one that spent it. Sorting a warning cluster by count named
        // `pump_page_fetches` as `agoda`'s culprit (t668); the timestamps then showed it never got a
        // turn at all (t669), because sixteen drain give-ups had already burned 37 seconds ahead of
        // it. **A cluster cannot answer a question about order.**
        //
        // So each boundary reports what the PREVIOUS phase actually cost, in the two units that
        // matter: wall time, and how many times the event loop gave up inside it. Emitted at `info`,
        // so `RUST_LOG=manuk_page=info` is the whole instrument — no rebuild to ask the question
        // again, which is the property the last three ticks kept wishing for.
        /// Not a phase — the sentinel that makes the LAST phase report itself, so the per-phase times
        /// sum to the total. A set of parts that does not sum to its whole is the accounting
        /// reconciliation this project rates as its highest-yield instrument.
        const LEDGER_CLOSE: &str = "<end of phases>";
        let mut last_at = std::time::Instant::now();
        let mut last_name: Option<&'static str> = None;
        let mut last_hits = manuk_js::event_loop::drain_ceiling_hits();
        let mut phase = |name: &'static str| -> bool {
            if let Some(prev) = last_name.take() {
                let hits = manuk_js::event_loop::drain_ceiling_hits();
                tracing::info!(
                    phase = prev,
                    ms = last_at.elapsed().as_millis() as u64,
                    gave_up = hits.saturating_sub(last_hits),
                    elapsed_ms = started.elapsed().as_millis() as u64,
                    "load phase done"
                );
                last_hits = hits;
            }
            // The sentinel closes the ledger; it is not a phase that could be "painted without".
            if name == LEDGER_CLOSE {
                return false;
            }
            let left = budget.saturating_sub(started.elapsed());
            if left.is_zero() {
                tracing::warn!(
                    "load budget of {:.1}s exhausted — painting without {name}. The page renders; \
                     the subresource was an enhancement, not a hostage.",
                    budget.as_secs_f32()
                );
                return false;
            }
            last_at = std::time::Instant::now();
            last_name = Some(name);
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
        // The last phase has no successor to report it, so close the ledger by hand. Without this the
        // per-phase times would not sum to the total — and a set of parts that does not sum to its
        // whole is the accounting reconciliation this project rates as its highest-yield instrument.
        phase(LEDGER_CLOSE);
        self.load_deadline = None;
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

            // **THE BUDGET IS CHECKED BETWEEN SETTLES, NOT ONLY BETWEEN ROUNDS.**
            //
            // Settling a result runs the page's own promise continuation, which drains the event
            // loop — so a page that is not converging pays a full drain ceiling **per settled
            // request**, and this loop settles up to `MAX_PER_ROUND` of them with no clock between.
            // Measured on `www.agoda.com` with the per-phase ledger (t670): this phase alone is
            // `ms=36061 gave_up=15` of a 39-second load, against a 12-second budget — **fifteen
            // give-ups inside ONE round.** The outer loop's per-round check cannot see any of it,
            // which is why three earlier ticks looked for the cost in three different phases.
            //
            // This takes nothing the budget was not already discarding: past the deadline
            // `finish_loading` skips images, masks and backgrounds outright, so continuing to settle
            // here buys a page nothing and costs it those phases. Stopping is the same promise
            // `finish_loading` already makes — *whatever has arrived is what the page gets* — applied
            // where it is actually spent.
            for (id, status, body, headers) in results {
                if budget.saturating_sub(started.elapsed()).is_zero() {
                    tracing::warn!(
                        round,
                        "load budget exhausted mid-round — the remaining fetch results are not \
                         settled. Each settle runs the page's own JS, and this page is not converging."
                    );
                    return;
                }
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
            self.load_deadline,
        )
        .await;
        self.image_attempts.extend(tried);
        self.apply_images(images, fonts, viewport_width)
    }

    /// **Every stylesheet that styles this document** — inline `<style>` blocks *and* the
    /// `<link>`ed sheets already fetched into [`external_css`](Self::external_css), each wrapped in
    /// its own `media` condition.
    ///
    /// ⚠ **This is the ONLY correct way to rebuild a sheet list for a re-cascade, and it exists
    /// because eight call sites did it wrong.** They rebuilt from
    /// `MinimalCascade::collect_style_elements`, which sees inline `<style>` and **nothing else** —
    /// so a resolved `fetch`, a click, a WebSocket frame, a streamed chunk, `postMessage`,
    /// `popstate`, the deferred-script pass and the incremental relayout each silently deleted every
    /// external stylesheet on the page and re-styled the document against UA defaults. On a page
    /// whose CSS lives in a `<link>` — essentially every page on the web — the first interaction
    /// threw the design away: every element a full-width block in the default serif, stacked, the
    /// document several times too tall.
    ///
    /// `keirin.jp` measured exactly that: nine author sheets (375KB) logged as *applied*, then a
    /// resolved `fetch()` re-cascaded them away. Coverage 97.8% — we render every element Chromium
    /// does — against SHAPE **2.2%**, because we place almost none of them where Chromium does.
    ///
    /// The comment at the tree-grew restyle already stated the rule (*"rebuilding from it would strip
    /// every `<link>`ed stylesheet from the page, which is a far worse bug than the one being
    /// fixed"*). One rule with N implementations is one rule that is wrong N−1 times; this is the
    /// implementation, and there is now only one.
    fn all_sheets(&self) -> Vec<Stylesheet> {
        sheets_of(&self.dom, &self.final_url, &self.external_css)
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
        manuk_js::set_snap_candidates(self.snap_candidates_map());
        // **Seed the pre-fetched ES-module import graph for THIS pass (B3b).** Modules are deferred, so
        // this is where `run_module` runs them and where the graph source map must be in place — set
        // here (a thread-local read synchronously by the module runner), from the map the async
        // pre-fetch pass carried onto the page, exactly like the scroll/snap geometry above. Empty ⇒
        // no-op. Cleared right after the pass so one document's graph can never resolve the next's.
        manuk_js::set_module_graph_sources(self.module_graph_sources.clone());
        // …and WHERE each module root came from, which is what its relative imports resolve against.
        manuk_js::set_module_node_bases(self.module_node_bases.clone());
        // The import map rides alongside the graph sources (tick 520) — the resolve hook consults it for
        // bare `import 'react'` specifiers during the same module pass.
        manuk_js::set_import_map(self.import_map.clone());
        // Before the scripts run, not after: a script that draws an image on its first tick must find
        // the pixels already there, or the draw silently no-ops and the canvas stays blank.
        self.publish_image_sources();
        // ⚠⚠⚠ **THE MODULE ROUND NEEDS THE FORCED-REFLOW HOOK, AND THE RELAYOUT BELOW IS NOT A
        // SUBSTITUTE FOR IT.** `if ran > 0` re-lays-out *after* this pass, so a module's nodes do
        // eventually get boxes and do eventually paint — but **every microtask the module queues
        // drains inside `run_deferred_scripts`**, before that line. `document.fonts.ready.then(…)`,
        // `Promise.resolve().then(…)`, a dynamic `import()`'s continuation and the whole
        // `queueMicrotask` family therefore measured the PRE-MODULE snapshot: a node the module had
        // just appended read `offsetWidth === 0`, and `css/css-grid/abspos/positioned-grid-
        // descendants-*` (32 files, 3,200 subtests) scored a flat zero on exactly that.
        //
        // This is the same defect t1184 fixed for `fire_lifecycle` and the same shape as
        // `set_root_box`'s warning: a pass wired into all but one of its call sites is a pass that
        // silently does not run on that one. ES modules are how the modern web ships JavaScript, so
        // this round is not a corner — it is most of them.
        let _reflow = ReflowScope::install(
            &self.dom,
            fonts,
            viewport_width,
            &self.images,
            &self.final_url,
            &self.external_css,
            &self.scroll_offsets,
            self.sticky_scroll_y,
            self.has_sticky,
        );
        let ran = match manuk_js::run_deferred_scripts(ctx, &mut self.dom, &rects, &self.styles) {
            Ok(n) => n,
            Err(e) => {
                // Surfaced, never swallowed — G_SILENT_FAIL. A deferred script that dies silently is
                // how "the app renders nothing and throws nothing" happens, and that cost several ticks.
                tracing::warn!("deferred scripts: {e}");
                0
            }
        };
        manuk_js::clear_module_graph_sources();
        manuk_js::clear_module_node_bases();
        // …and the compiled-module registry, which `run_module` used to drop itself. Too early: a
        // dynamic `import()` settles its promise in a later microtask and needs the module to still
        // be resolvable. Here is after the deferred pass and its microtasks, and still inside the
        // navigation — which is the lifetime the GC contract actually cares about.
        manuk_js::clear_module_registry();
        manuk_js::clear_import_map();
        self.drain_canvases();
        self.drain_element_scrolls();
        if ran > 0 {
            tracing::debug!(scripts = ran, "executed deferred page scripts");
            let sheets: Vec<Stylesheet> = self.all_sheets();
            (self.styles, self.root_box) =
                restyle_and_layout(&self.dom, &sheets, fonts, viewport_width, &self.images);
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
    /// The URLs of the subframes that actually **rendered**, in no particular order.
    ///
    /// This is the observable an anti-framing check needs: a frame refused by `X-Frame-Options` or
    /// CSP `frame-ancestors` never reaches `render_iframe`, so it is absent here. A frame's own
    /// document is otherwise reachable only through `contentDocument`, i.e. from JS — which a Rust
    /// gate would have to stand up a script context to read.
    pub fn rendered_iframe_urls(&self) -> Vec<String> {
        self.iframes.values().cloned().collect()
    }

    pub fn render_iframe(
        &mut self,
        node: manuk_dom::NodeId,
        html: &str,
        url: &str,
        fonts: &FontContext,
        depth: u8,
    ) {
        self.render_iframe_with_type(node, html, url, fonts, depth, None, None)
    }

    /// **The same, but told what the server said it was sending.**
    ///
    /// An `<iframe src="…​.xml">` was parsed as HTML, because nothing on this path ever looked at the
    /// response's `Content-Type`. The HTML parser lowercases every tag name and recovers silently from
    /// errors, so a framed XML document came back with the wrong `documentElement` and no complaint —
    /// and *98 `dom` subtests* fail at their first line for that reason, before testing anything.
    ///
    /// The routing rule is the MIME sniffing standard's: an XML MIME type is `text/xml`,
    /// `application/xml`, or **anything ending in `+xml`** — which is what makes `application/
    /// xhtml+xml`, `image/svg+xml`, `application/rss+xml` and `application/atom+xml` all XML without
    /// enumerating them. Everything else stays HTML, including an absent or unparseable type.
    pub fn render_iframe_with_type(
        &mut self,
        node: manuk_dom::NodeId,
        html: &str,
        url: &str,
        fonts: &FontContext,
        depth: u8,
        content_type: Option<&str>,
        charset: Option<&str>,
    ) {
        self.render_iframe_inner(node, html, url, fonts, depth, content_type, charset, false)
    }

    /// **The same, with two steps withheld — the INITIAL `about:blank` of a frame that has not
    /// navigated yet.**
    ///
    /// ⚠⚠⚠ HTML gives an `<iframe>` a child browsing context the moment it is INSERTED, holding an
    /// `about:blank` document, and replaces it when `src` lands. This engine only ever built the
    /// document at load, so `<iframe src="…">` read `contentDocument === null` until the fetch
    /// returned — and `typeof null` is `"object"`, so every `typeof`-shaped feature check saw a
    /// document that was not there. Chrome-measured on a `src` that never resolves:
    ///
    /// ```text
    ///                                      Chrome    before
    ///   s.contentDocument === null          false      true
    ///   window.length (1 src + 1 bare)          2         1
    ///   typeof window[1]                   object  undefined
    /// ```
    ///
    /// `provisional` withholds exactly two steps, and each is load-bearing:
    ///
    ///  * **`self.iframes.insert`** — that map IS the "already rendered" marker `pending_iframes`
    ///    reads. Marking a provisional frame would make the real fetch skip it, i.e. every
    ///    `<iframe src>` on the web would stop loading. This is the trap in the change and it is why
    ///    the flag exists rather than a second call to the public method.
    ///  * **`fire_frame_load`** — Chrome does NOT fire `load` for the initial `about:blank` of a
    ///    `src`-bearing frame; the event belongs to the document the `src` names. (A frame with NO
    ///    `src` is different: its about:blank IS its document, it fires once, and that path is
    ///    unchanged because it does not come through here.)
    #[allow(clippy::too_many_arguments)]
    fn render_iframe_inner(
        &mut self,
        node: manuk_dom::NodeId,
        html: &str,
        url: &str,
        fonts: &FontContext,
        depth: u8,
        content_type: Option<&str>,
        charset: Option<&str>,
        provisional: bool,
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
        // ⚠ CONTENT box, not border box — see `frame_content_width`. The fetched path had the same
        // 4px error as the inline one; it had simply never been measured.
        let (w, h) = match rects.get(&node) {
            Some(r) if r.width >= 1.0 && r.height >= 1.0 => {
                let (cw, ch) = frame_content_box(self.styles.get(&node), r.width, r.height);
                (cw.round().max(1.0) as u32, ch.round().max(1.0) as u32)
            }
            _ => (300, 150),
        };
        let _vp = ViewportScope::set(w as f32, h as f32);
        let visible = rects
            .get(&node)
            .is_some_and(|r| r.width >= 1.0 && r.height >= 1.0);

        // A whole page, at the iframe's own viewport width — so its media queries and its layout see the
        // size of the frame, not the size of the window. That is what makes a responsive embed responsive.
        let mut child = if is_xml_mime(content_type) {
            Page::load_xml(html, url, fonts, w as f32)
        } else {
            Page::load(html, url, fonts, w as f32)
        };
        if visible {
            let canvas = child.paint(fonts, w, h);
            let img = manuk_paint::DecodedImage {
                width: canvas.width(),
                height: canvas.height(),
                rgba: canvas.rgba_bytes().to_vec(),
            };
            self.images.insert(node, std::rc::Rc::new(img));
        }
        if !provisional {
            self.iframes.insert(node, url.to_string());
        }

        // **Keep the document.** Everything above this line already existed; the child was built in full
        // and then thrown away, which is why `contentDocument` was `undefined` for eighty-three ticks.
        // Registering the arena is what makes a reflector into it *safe* — see `manuk_js::register_dom`.
        // ⚠ **BEFORE `fire_frame_load` BELOW, and that ordering is the whole point.** Every test —
        // and every embed — reads the child document inside its `load` handler, so a value written
        // after `render_iframe_with_type` returns is written after the only moment anyone looks.
        // The first version of this set it at the call site and measured +0 for exactly that reason.
        if let Some(cs) = charset {
            let doc = child.dom.root();
            child.dom.set_character_set(doc, cs);
        }
        // **A framed document's referrer is its EMBEDDER**, and it is read first thing inside every
        // analytics, attribution and paywall script an embed runs. Set here for the same reason the
        // charset is: `fire_frame_load` is below, and a value written after it is written after the
        // only moment anyone looks (t1211).
        {
            let doc = child.dom.root();
            let embedder = self.final_url.clone();
            child.dom.set_referrer(doc, &embedder);
        }
        manuk_js::register_dom(&mut *child.dom as *mut manuk_dom::Dom);
        self.child_pages.insert(node, child);
        self.publish_iframe_docs(fonts);
        // **`load` fires HERE, at the one place a child document is installed**, and not at the two
        // callers — `set_root_box` above states the rule this follows: three call sites feeding one
        // post-step is how a pass silently does not run on the third. Both the fetched path
        // (`fetch_and_load_iframes`) and the network-free path (`load_inline_frames`: `srcdoc`,
        // `about:blank`, bare `<iframe>`) arrive here, and so does every direct caller.
        if !provisional {
            self.fire_frame_load(node);
        }
    }

    /// Tell the JS world which arena sits behind each `<iframe>`. Cheap, and it must run before any
    /// script that might reach into a frame — which, once frames are loaded, is any script at all.
    fn publish_iframe_docs(&mut self, fonts: &FontContext) {
        // **Adopt anything the on-demand builder made during the last script round, FIRST.** This
        // is the single post-step for a `child_pages` mutation, so the frames a script created are
        // taken here and the publish below sees them like any other child (t1299).
        #[cfg(feature = "spidermonkey")]
        {
            let base = self.final_url.clone();
            adopt_dynamic_frames(&mut self.child_pages, &mut self.iframes, &base);
            // Re-arm for the round that is about to run: `fire_lifecycle` and every dispatch path
            // reach here before they execute a line of script.
            arm_dynamic_frames(fonts, &self.final_url);
        }
        let m: std::collections::HashMap<_, _> = self
            .child_pages
            .iter_mut()
            .map(|(&n, c)| {
                let root = c.dom.root();
                (n, (&mut *c.dom as *mut manuk_dom::Dom as usize, root))
            })
            .collect();
        manuk_js::set_iframe_docs(m);

        // **And each child's COMPUTED-STYLE map, keyed by its arena.** `getComputedStyle` had one
        // style map to read, so a frame element resolved against the PARENT's — the same
        // one-arena assumption `node_and_dom` closed for the DOM, one pass later in the pipeline.
        //
        // ⚠ Published HERE and nowhere else, for the reason the field above documents: a child
        // `Page` is a value in a `HashMap`, so its `styles` field MOVES when that map rehashes.
        // `Page::dom` is a `Box` and survives that; `Page::styles` is not, so its address is only
        // meaningful until the next `child_pages` mutation — which is exactly the window this
        // single call site covers, because every such mutation calls it.
        let styles: std::collections::HashMap<usize, usize> = self
            .child_pages
            .iter_mut()
            .map(|(_, c)| {
                (
                    &mut *c.dom as *mut manuk_dom::Dom as usize,
                    &c.styles as *const _ as usize,
                )
            })
            .collect();
        manuk_js::set_frame_styles(styles);

        // **And the registry that lets a style read LAY THAT CHILD OUT** (t1298) — same key, same
        // lifetime, same call site, for the reason the block above states: a child `Page` is a value
        // in a `HashMap`, so a raw pointer to it is meaningful only until the next `child_pages`
        // mutation, and every such mutation comes through here.
        //
        // ⚠ **The width is each frame's OWN box**, taken from the parent's layout, with the same
        // `300x150` fallback `render_iframe_with_type` uses for a frame with no box. Handing the
        // window's width here would resolve every `vw` inside every frame against the window, which
        // is the precise bug this mechanism exists to prevent.
        #[cfg(feature = "spidermonkey")]
        {
            let frame_rects = self.root_box.node_rects(&self.dom);
            let boxes: HashMap<manuk_dom::NodeId, (f32, f32)> = self
                .child_pages
                .keys()
                .map(|&n| {
                    let wh = match frame_rects.get(&n) {
                        Some(r) if r.width >= 1.0 && r.height >= 1.0 => {
                            let (cw, ch) =
                                frame_content_box(self.styles.get(&n), r.width, r.height);
                            (cw.round(), ch.round())
                        }
                        _ => (300.0, 150.0),
                    };
                    (n, wh)
                })
                .collect();
            publish_frame_reflow_pages(
                self.child_pages.iter_mut().map(|(n, c)| {
                    let addr = &mut *c.dom as *mut manuk_dom::Dom as usize;
                    let (w, h) = boxes.get(n).copied().unwrap_or((300.0, 150.0));
                    (addr, c, w, h)
                }),
                fonts,
            );
        }

        // **And the source-set selection behind `<img>.currentSrc`, for this document AND every
        // child.** It rides here rather than in `ReflowScope::install` for one reason: `install`
        // receives a single `&Dom` and has no way to reach `child_pages`, and an iframed `<img>` is
        // the case that matters most — WPT's whole `the-img-element/sizes` fixture lives inside an
        // `<iframe>`, and a parent-only table answers `null` for every one of its images.
        //
        // `DEFAULT_SELECTION_VIEWPORT`, not the live width, deliberately: this must agree with the
        // fetch worklist and the decode worklist to the byte, and both select at that width. A
        // third width here would fetch one candidate and report another.
        let mut selected = HashMap::new();
        selected_image_urls(
            &self.dom,
            DEFAULT_SELECTION_VIEWPORT,
            &self.final_url,
            &mut selected,
        );
        for c in self.child_pages.values() {
            selected_image_urls(
                &c.dom,
                DEFAULT_SELECTION_VIEWPORT,
                &c.final_url,
                &mut selected,
            );
        }
        manuk_js::set_selected_srcs(selected);
    }

    /// **The frames whose document does not come off the network** — `srcdoc`, `src="about:blank"`,
    /// and an `<iframe>` with no `src` at all.
    ///
    /// `pending_iframes` skips all three, correctly, because it is a *fetch* work-list and there is
    /// nothing to fetch. Nothing then loaded them either, and that is the bug: HTML §4.8.5 says an
    /// `<iframe>` with no `src` is **immediately navigated to `about:blank`** and gets a fully-formed,
    /// same-origin document. Ours got none, so `contentDocument` was `null`.
    ///
    /// ⚠ **And `null` is why nothing caught it: `typeof null === 'object'`.** Every feature detect of
    /// the form `typeof f.contentDocument === 'object'` passed — measured against headless Chrome on
    /// one fixture, where we answered `object` for all three cases and then threw
    /// `can't access property "body", f1.contentDocument is null` on the very next line. A `null` that
    /// types as present is the false-presence class this project already has a gate family for.
    ///
    /// **What a page does with a document it makes itself**, which is why this is not a niche:
    /// a hidden `about:blank` frame is the standard way to get a *pristine* `window` (libraries lift
    /// unpatched natives out of it), to sandbox untrusted markup, to relay `postMessage`, to host an
    /// OAuth or payment bridge, and — measured on `www.welt.de` this session — to run an **ad-bait
    /// test**: create a frame, write ad-shaped markup into `contentDocument`, and measure whether it
    /// survives. A frame with no document fails that test the same way an ad blocker does.
    ///
    /// `srcdoc` is the same mechanism with the markup supplied inline, and it is what sandboxed
    /// previews, documentation embeds and mail clients ship. It beats `src` per spec; the code had
    /// said so in a comment for as long as it had ignored it.
    ///
    /// ⚠ Named residual: these load on the host's next round, not synchronously inside the
    /// `appendChild` that created the frame. A script that appends a frame and reads
    /// `contentDocument` **on the very next line** still sees `null`; one that reads it at
    /// `DOMContentLoaded`, `load`, or from any later task sees a real document. Closing that means
    /// building a child document from inside a JS binding, which is a different change.
    /// **Fire `load` on an `<iframe>` ELEMENT once its document is installed.**
    ///
    /// ⚠⚠⚠ **THE FRAME LOADED AND NOTHING EVER SAID SO.** `contentDocument` was populated and
    /// readable — t512's work — but no `load` event was ever dispatched on the element, so
    /// `<iframe onload=…>`, `frame.addEventListener('load', …)` and the `onload` property were all
    /// dead. Probed one property per file so the pass count names the failure:
    ///
    /// ```text
    ///   contentDocument non-null              PASS   <- the frame really is loaded
    ///   contentDocument has the child's text  PASS   <- ...and really is the right document
    ///   INLINE onload= fired                  FAIL
    ///   addEventListener('load') fired        FAIL
    ///   the onload PROPERTY is a function     FAIL
    /// ```
    ///
    /// The first two rows are what make the last three a *missing event* rather than a missing
    /// frame. `<iframe onload>` is how an embed, an ad slot, a payment frame, an OAuth popup-frame
    /// and every lazy widget on the web announce readiness — and in WPT it is what gates
    /// `domparsing`'s four `DOMParser-parseFromString-url*` files (45 subtests each, all
    /// `harness=TIMEOUT` at ~120ms because their shared `loadPromise` never resolves).
    ///
    /// Same shape as the `<script>` completion event: dispatch on the element, through the ordinary
    /// event path, so inline handlers, listeners and the reflected property all see it.
    #[cfg(feature = "spidermonkey")]
    fn fire_frame_load(&mut self, node: manuk_dom::NodeId) {
        let Some(ctx) = &self.js else { return };
        let rects: HashMap<manuk_dom::NodeId, [f32; 4]> = self
            .root_box
            .node_rects(&self.dom)
            .into_iter()
            .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
            .collect();
        let _ = manuk_js::dispatch_event(ctx, &mut self.dom, node, "load", &rects, &self.styles);
    }

    /// Without SpiderMonkey there is no handler to notify — a no-op, not a lie.
    #[cfg(not(feature = "spidermonkey"))]
    fn fire_frame_load(&mut self, _node: manuk_dom::NodeId) {}

    #[cfg(feature = "spidermonkey")]
    fn load_inline_frames(&mut self, fonts: &FontContext) {
        let mut work: Vec<(manuk_dom::NodeId, String)> = Vec::new();
        let mut provisional: Vec<manuk_dom::NodeId> = Vec::new();
        for n in self.dom.flat_descendants(self.dom.root()) {
            if self.dom.tag_name(n) != Some("iframe") || self.iframes.contains_key(&n) {
                continue;
            }
            let Some(el) = self.dom.element(n) else {
                continue;
            };
            // `srcdoc` beats `src` (HTML §4.8.5), including a `src` that points somewhere real.
            if let Some(doc) = el.attr("srcdoc") {
                work.push((n, doc.to_string()));
                continue;
            }
            let src = el.attr("src").unwrap_or("").trim();
            if src.is_empty() || src.eq_ignore_ascii_case("about:blank") {
                work.push((n, String::new()));
            } else if !src.starts_with("javascript:") {
                // ── ⚠⚠⚠ **A FRAME THAT HAS NOT NAVIGATED YET STILL HAS A DOCUMENT.** HTML gives an
                //    `<iframe>` a child browsing context at INSERTION, holding `about:blank`, and
                //    swaps it when `src` lands. We built nothing until the fetch returned, so
                //    `contentDocument` was `null` for every `<iframe src>` on the page — and
                //    `typeof null` is `"object"`, so the check every embed writes
                //    (`f.contentWindow ? … : f.contentDocument`) saw a document that was not there.
                //
                //    It is not only a reflector gap: a frame with no document is not a child
                //    browsing context, so `window.length` under-counted and `window.frames[i]` skipped
                //    it (t1349). Chrome-measured, one `src` frame that never resolves plus one bare:
                //
                //    ```text
                //                                      Chrome    before
                //      s.contentDocument === null       false      true
                //      window.length                        2         1
                //      typeof window[1]                object  undefined
                //    ```
                //
                //    ⚠ PROVISIONAL — see `render_iframe_inner`. The two withheld steps are the whole
                //    safety argument: this must not mark the frame rendered (the real fetch would
                //    skip it) and must not fire `load` (that event belongs to the `src` document).
                if !self.child_pages.contains_key(&n) {
                    provisional.push(n);
                }
            }
        }
        for node in provisional {
            let base = self.final_url.clone();
            self.render_iframe_inner(node, "", &base, fonts, 0, None, None, true);
        }
        for (node, html) in work {
            // **The parent's URL, not `about:blank`.** A `srcdoc` document's base URL is the
            // embedder's (HTML §4.8.5), which is what makes a relative `<img src="logo.png">` inside
            // one resolve at all; an `about:blank` frame inherits it for the same reason. Passing the
            // literal `about:` scheme here would make every relative URL in the child unresolvable —
            // and `render_iframe` is also where the child's own subresource fetches are based.
            let base = self.final_url.clone();
            self.render_iframe(node, &html, &base, fonts, 0);
        }
    }

    /// Without SpiderMonkey there is no `contentDocument` to hand anyone, so there is nothing to load.
    #[cfg(not(feature = "spidermonkey"))]
    fn load_inline_frames(&mut self, _fonts: &FontContext) {}

    /// Fetch every `<iframe src>` and load it as a real document.
    ///
    /// **Before `load` fires**, because that is the spec (`load` waits for subframes) and because it is
    /// the whole point: `<body onload="...">` is where a page reaches into its frames. WPT's entire
    /// `encoding` suite — 767,003 subtests, 91% of the measured universe — is exactly that shape.
    async fn fetch_and_load_iframes(&mut self, fonts: &FontContext, _viewport_width: f32) {
        // **The frames with nothing to fetch, which are not the frames with nothing to load.**
        // Done first and synchronously — they need no network, and a page that creates a frame in
        // order to *read* it should not be made to wait on a fetch round that has nothing to do.
        self.load_inline_frames(fonts);
        let pending = self.pending_iframes();
        if pending.is_empty() {
            return;
        }
        // One round of fetches, concurrently. A frame inside a frame is handled by the child's own
        // load — `MAX_IFRAME_DEPTH` is the fork-bomb guard.
        let fetches = pending.into_iter().map(|(node, url, _w, _h)| async move {
            let (html, final_url, headers, charset) =
                fetch_html_with_headers_named(&url).await.ok()?;
            Some((node, html, final_url, headers, charset))
        });
        let got: Vec<_> = futures_util::future::join_all(fetches)
            .await
            .into_iter()
            .flatten()
            .collect();
        // The embedder is THIS document — the origin the framed page is being asked to trust.
        let embedder = Url::parse(&self.final_url).ok();
        for (node, html, final_url, headers, charset) in got {
            // **A site that says "do not frame me" must not be framed.** `X-Frame-Options` and CSP
            // `frame-ancestors` are the only clickjacking defence a page has, and ignoring them is
            // silent: the frame renders, the user interacts with it believing it is the outer site,
            // and nothing anywhere reports that a stated policy was overruled. It matters more here
            // than in an ordinary browser, because an agentic browser composes pages it did not
            // author and acts inside them.
            if let Ok(framed) = Url::parse(&final_url) {
                if !framing_allowed(&headers, embedder.as_ref(), &framed) {
                    tracing::warn!(url = %final_url, "framing refused by X-Frame-Options / frame-ancestors");
                    // The frame element stays, with no document — which is what a refusal looks
                    // like. Rendering the body anyway "so the user sees something" would be
                    // precisely the failure the header exists to prevent.
                    continue;
                }
            }
            let ct = headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
                .map(|(_, v)| v.clone());
            // **What the bytes were decoded FROM** travels with them: the sniffer computed it and
            // every caller threw it away, which is why `document.characterSet` was a constant.
            self.render_iframe_with_type(
                node,
                &html,
                &final_url,
                fonts,
                0,
                ct.as_deref(),
                Some(charset.as_str()),
            );
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
        scroll_geometry_of(
            &self.dom,
            &self.root_box,
            &self.styles,
            &self.scroll_offsets,
        )
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
        let clamped = (left.clamp(0.0, max_x), top.clamp(0.0, max_y));
        // **Snap AFTER clamping, never before.** A snap point past the scrollable range is not
        // reachable, and snapping first would pick it and then clamp back to an unaligned position —
        // the container would refuse to reach its own last slide, which is the classic carousel bug.
        let new = self.snap_scroll(node, clamped, (max_x, max_y));
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
        // ⭐ **The scroll just moved a sticky box's own scrollport past it.** The translation above
        // carried every child of the container, including any sticky one, so without this the
        // sticky header inside an `overflow:auto` pane scrolls away with the pane — which is the
        // whole of `css/css-position/sticky`'s scroll-container family reading the in-flow position.
        self.restick(self.sticky_scroll_y);
        new
    }

    /// **Land a scroll on the nearest snap point** — `scroll-snap-type` on the container,
    /// `scroll-snap-align` on the children.
    ///
    /// This is the whole of "the carousel stops on a slide". Every paged feed, image gallery, story
    /// tray and mobile card row on the web is a scroll container plus these two properties, and
    /// without them a flick lands wherever momentum happened to stop — two half-slides on screen
    /// and neither readable. The scroll path is a single chokepoint, so this is one transformation
    /// inserted at it rather than a scrolling subsystem.
    ///
    /// **Only children carrying `scroll-snap-align` are candidates**, and a container with none
    /// keeps the raw offset. That matters: `scroll-snap-type` alone must not pin the container to
    /// 0, which is what "snap to the nearest of an empty candidate set" degrades to if the empty
    /// case is not handled — a scroll container that cannot be scrolled at all.
    fn snap_scroll(&self, node: manuk_dom::NodeId, at: (f32, f32), max: (f32, f32)) -> (f32, f32) {
        let Some(cs) = self.styles.get(&node) else {
            return at;
        };
        let axis = cs.scroll_snap_type;
        if axis == manuk_css::ScrollSnapAxis::None {
            return at;
        }
        let Some(container) = self.root_box.find(node) else {
            return at;
        };
        // The current (already-scrolled) offset, so the shared collector can recover each snap
        // point's content-space position. ONE collector — `snap_candidates_for` — feeds both this
        // engine chokepoint and the JS-side mirror (via `snap_candidates_of` →
        // `manuk_js::set_snap_candidates`); building the candidate list a second time here is exactly
        // the two-sources-of-truth trap that drift comes from.
        let cur = self
            .scroll_offsets
            .get(&node)
            .copied()
            .unwrap_or((0.0, 0.0));
        let (xs, ys) = snap_candidates_for(container, node, &self.styles, cur, max);

        let nearest = |cands: &[f32], v: f32| -> f32 {
            cands
                .iter()
                .copied()
                .min_by(|a, b| {
                    (a - v)
                        .abs()
                        .partial_cmp(&(b - v).abs())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap_or(v)
        };
        // `nearest` returns the INPUT when there are no candidates — which is the whole of "a
        // container that declares snapping but has no aligned children still scrolls freely". An
        // `unwrap_or(0.0)` here instead would pin such a container at the top forever, and an
        // explicit `!is_empty()` guard in front would be dead code hiding which line is load-bearing.
        (
            if axis.snaps_x() {
                nearest(&xs, at.0)
            } else {
                at.0
            },
            if axis.snaps_y() {
                nearest(&ys, at.1)
            } else {
                at.1
            },
        )
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
        // ⚠⚠⚠ **RE-ASK WHETHER THE PAGE HAS A STICKY BOX AT ALL, because a SCRIPT can add one.**
        // `has_sticky` is computed once in the constructor from the initial cascade, and it gates the
        // whole sticky pass — so a `position: sticky` element created by `createElement` (or by a
        // class toggle) was invisible to it forever, and the box never stuck. One more instance of
        // this session's theme: a value derived from a snapshot, and something created after the
        // snapshot was taken. There IS a second update site on the re-cascade path (`self.styles =
        // new_styles`), but not every relayout goes through it; this one runs after all of them.
        //
        // `any` short-circuits on the first sticky box, so a page with one pays almost nothing and a
        // page with none pays one pass over a map the caller has just rebuilt anyway.
        self.has_sticky = self
            .styles
            .values()
            .any(|s| s.position == manuk_css::Position::Sticky);
        // The tree is FRESH out of layout, so the old ledger describes boxes that no longer exist —
        // drop it rather than "undo" a shift that was never applied to these rects.
        self.sticky_applied.clear();
        self.restick(self.sticky_scroll_y);
    }

    /// **Re-bake the `position:sticky` shifts into `root_box` for document scroll `scroll_y`.**
    ///
    /// Undo what [`Page::sticky_applied`] says is currently baked in, then walk the tree and apply
    /// the shifts the current view state calls for, recording them again. Undo-then-reapply rather
    /// than applying a delta, because the constraint is not linear in the scroll: a box is pinned,
    /// then released at the end of its containing block, and a delta cannot express the release.
    ///
    /// ⚠ **Undo order does not matter and that is not luck.** A nested sticky box has its ancestor's
    /// shift in its rect as well as its own, but both are translations of overlapping subtrees, and
    /// translations commute — the total removed from any box is the sum of the entries covering it
    /// whichever order they are subtracted in. Re-application *is* ordered (outer first, by the
    /// walk), because the inner box's constraint is evaluated against its already-shifted ancestor,
    /// which is what makes nested sticky offsets accumulate the way §6.3 says they do.
    ///
    /// Costs nothing on a page with no sticky box: `has_sticky` is the first thing it asks.
    fn restick(&mut self, scroll_y: f32) {
        if !self.has_sticky {
            return;
        }
        let previous: Vec<(manuk_dom::NodeId, f32)> = self.sticky_applied.drain().collect();
        for (node, dy) in previous {
            if let Some(b) = self.root_box.find_mut(node) {
                b.shift_y(-dy);
            }
        }
        // The document scrollport. `viewport_size()` is the same viewport the cascade resolved
        // `vh`/`dvh` against, so a sticky box and the units around it cannot disagree about how tall
        // the screen is.
        let (_, vh) = manuk_css::values::viewport_size();
        let mut applied = std::collections::HashMap::new();
        apply_sticky(&mut self.root_box, &self.styles, scroll_y, vh, &mut applied);
        self.sticky_applied = applied;
        self.sticky_scroll_y = scroll_y;
    }

    /// Every `overflow: auto|scroll|hidden` container's scroll geometry — what the DOM must report.
    fn scroll_geometry_map(&self) -> std::collections::HashMap<manuk_dom::NodeId, [f32; 6]> {
        scroll_geometry_of(
            &self.dom,
            &self.root_box,
            &self.styles,
            &self.scroll_offsets,
        )
    }

    fn snap_candidates_map(
        &self,
    ) -> std::collections::HashMap<manuk_dom::NodeId, (Vec<f32>, Vec<f32>)> {
        snap_candidates_of(
            &self.dom,
            &self.root_box,
            &self.styles,
            &self.scroll_offsets,
        )
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
    /// **The mirror of [`drain_canvases`](Self::drain_canvases): decoded images go IN, so a script can
    /// `ctx.drawImage(img, …)` them.**
    ///
    /// Canvas has only ever pushed pixels outward. `drawImage` is the first operation that needs the
    /// host's pixels going the other way, because the thing it draws is an `<img>` the network fetched
    /// and the image decoder decoded — data a script cannot produce for itself.
    ///
    /// **Canvases are deliberately excluded even though they live in the same map.** `drain_canvases`
    /// drops finished canvases into `self.images` alongside `<img>`, which is the trick that lets the
    /// painter treat them identically. Publishing them back would hand `drawImage` a snapshot of a
    /// canvas as it was at the *end of the last script round* — so the standard double-buffer idiom,
    /// `dst.drawImage(scratch, 0, 0)`, would composite a stale frame. The canvas registry already holds
    /// the live surfaces and is looked up first; this is only for pixels it does not have.
    ///
    /// Publishing is idempotent, so this can run every round without re-uploading a megabyte per image.
    fn publish_image_sources(&self) {
        for (node, img) in &self.images {
            if self.dom.tag_name(*node) == Some("canvas") {
                continue;
            }
            manuk_js::publish_image_source(node.0, img.width, img.height, &img.rgba);
        }
    }

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

    /// **Hand a decoded video frame to the page** — the frame renders exactly where the poster was.
    ///
    /// This is the whole of "video paints", and it is deliberately three lines, because the structural
    /// work was already done years of ticks ago and nobody had connected the two ends. A `<video>` is
    /// **already** a replaced element in layout (`manuk_layout::is_replaced_element`), and a
    /// `<video poster>` **already** fetches, decodes and paints through the identical route as `<img>` —
    /// `self.images` keyed by the `<video>`'s own `NodeId`, blitted into the content box by
    /// `manuk_paint::blit_image`. So a decoded frame does not need a video path in the painter, a new
    /// display item, or a relayout. **It needs to overwrite one map entry.** MEDIA.md called this out as
    /// the insight that makes the whole track small: *a video frame IS a `DecodedImage`, and playing a
    /// video is swapping the `Rc` in the map the poster already occupies.*
    ///
    /// **It takes raw RGBA, not a `manuk_media::video::Frame`, and that is the important decision.**
    /// Naming the media type here would drag `manuk-media`'s decoder features into `manuk-page`, and
    /// `openh264` compiles C — which would put a C toolchain into all ~25 gate binaries that link
    /// `manuk-page`, the exact isolation tick 236 went to lengths to establish. Taking bytes keeps the
    /// page **decoder-agnostic**: openh264 today, `re_rav1d` or a VA-API backend later, and this
    /// signature does not move. Same reasoning as tick 236's `trait VideoDecoder` — the boundary is the
    /// deliverable, not the backend behind it.
    ///
    /// **No relayout, on purpose.** Unlike `<img>`, a `<video>`'s box is sized by its `width`/`height`
    /// attributes or CSS, never by the frame that happens to be on screen — otherwise the page would
    /// reflow on the first frame and again on any stream that changes resolution mid-playback, which is
    /// what adaptive streaming does by design. The frame is scaled into the box that already exists.
    pub fn set_video_frame(
        &mut self,
        node: manuk_dom::NodeId,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    ) {
        debug_assert_eq!(
            rgba.len(),
            width as usize * height as usize * 4,
            "a video frame must be tightly-packed RGBA"
        );
        self.images.insert(
            node,
            std::rc::Rc::new(manuk_paint::DecodedImage {
                width,
                height,
                rgba,
            }),
        );
    }

    /// **The MSE byte-streams the page's players have appended since the last drain** — the
    /// playback JOIN (tick 349), coalesced to the newest stream per element.
    ///
    /// An MSE-attached `<video>` is invisible to the fetch-side media path: its `src` is a `blob:`
    /// URL naming a `MediaSource`, so [`Page::pending_media_urls`]' bytes can never arrive — the
    /// only copy of the media is what `appendBuffer` accumulated inside the page. The JS side
    /// publishes that stream (full, not a delta — an fMP4 decoder needs the init segment plus
    /// every fragment as one buffer) on each settled append that carries a video track; this
    /// drains it for the host to decode and drive exactly like a fetched progressive movie.
    ///
    /// Coalescing here is what keeps a burst of appends cheap: ten appends between two host
    /// visits become ONE decode of the newest stream, not ten decodes of ten prefixes.
    #[cfg(feature = "spidermonkey")]
    pub fn take_mse_media(&mut self) -> Vec<(manuk_dom::NodeId, Vec<u8>)> {
        if self.js.is_none() {
            return Vec::new();
        }
        let mut newest: std::collections::HashMap<u64, Vec<u8>> = std::collections::HashMap::new();
        for (node, bytes) in manuk_js::take_mse_streams() {
            newest.insert(node, bytes); // oldest-first drain, so the last write per node wins
        }
        newest
            .into_iter()
            .map(|(n, b)| (manuk_dom::NodeId(n), b))
            .collect()
    }

    #[cfg(not(feature = "spidermonkey"))]
    pub fn take_mse_media(&mut self) -> Vec<(manuk_dom::NodeId, Vec<u8>)> {
        Vec::new()
    }

    /// Live media-IDL property writes since the last drain (tick 360): `(node, prop, value)`,
    /// coalesced to the LAST write per (node, prop) — a slider dragged across ten frames is one
    /// gain change, not ten. "muted" is 0/1, "volume" 0..1, "playbackRate" a rate.
    #[cfg(feature = "spidermonkey")]
    pub fn take_media_props(&mut self) -> Vec<(manuk_dom::NodeId, String, f64)> {
        if self.js.is_none() {
            return Vec::new();
        }
        let mut last: std::collections::HashMap<(u64, String), f64> =
            std::collections::HashMap::new();
        for (node, prop, value) in manuk_js::take_media_props() {
            last.insert((node, prop), value); // oldest-first drain: the final write wins
        }
        last.into_iter()
            .map(|((n, p), v)| (manuk_dom::NodeId(n), p, v))
            .collect()
    }

    #[cfg(not(feature = "spidermonkey"))]
    pub fn take_media_props(&mut self) -> Vec<(manuk_dom::NodeId, String, f64)> {
        Vec::new()
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
                // The SAME selector `collect_subresources` uses, against the same width — two
                // independent choices here would be the two-cascades trap in a different organ: we
                // would fetch one URL and then decode another.
                Some("img") => select_image_url(&self.dom, n, DEFAULT_SELECTION_VIEWPORT),
                Some("video") => el.attr("poster").map(str::to_string), // a poster is a still
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

    /// **Report the host's real verdict on one element's media** — decoded, or did not.
    ///
    /// The other half of [`Page::pending_media_urls`], and the piece that makes `video.error` tell
    /// the truth. The JS surface used to answer `MediaError(4)` eagerly on every media element,
    /// which was honest while nothing could decode and became a lie contradicting `canPlayType` the
    /// moment playback landed. **Neither fixed value is honest** — an eager error gives up on video
    /// that works, and a permanent `null` hangs on video that does not, showing a dead player where
    /// a fallback belonged. Only the actual outcome is honest, and the host is the one layer that
    /// has it: the shell fetched the bytes and knows whether they decoded.
    ///
    /// A no-op without a JS context, which is the correct shape rather than a guard bolted on: the
    /// media state is a property of the *script-visible* element, and a page with no scripts has
    /// nobody to tell.
    ///
    /// Feature-gated on `spidermonkey` for the same reason it is a no-op without a JS context, one
    /// step further out: with no engine linked there is no `error` property for anyone to read, so
    /// there is nothing to report to. Building the body unconditionally is what broke `manuk-agent`,
    /// which links `manuk-page` WITHOUT the JS feature — the JS-less build is a supported
    /// configuration, not an afterthought.
    #[cfg(feature = "spidermonkey")]
    pub fn set_media_outcome(&mut self, node: manuk_dom::NodeId, ok: bool) {
        if self.js.is_none() {
            return;
        }
        // `__nodes[id]` resolves only for an element that has been reflected; `__manukMedia` is what
        // installs `__setOutcome`, and it runs at reflection time. Guarding on both means a report
        // for an element no script has ever touched is dropped rather than throwing.
        let src = format!(
            "(function(){{var e=globalThis.__nodes&&__nodes[{}];\
               if(e&&e.__setOutcome){{e.__setOutcome({});}}}})()",
            node.0, ok
        );
        self.eval_for_test(&src);
    }

    /// **The media this page wants, and the element each byte-stream belongs to.**
    ///
    /// The counterpart to [`Page::pending_image_urls`] for `<video>`/`<audio>`, and the missing
    /// first link of the media chain. Until this existed a `<video src="movie.mp4">` was **never
    /// fetched at all**: `pending_image_urls` reads `<video>`'s `poster` and nothing else, so the
    /// still frame loaded and the movie behind it was not so much undecodable as *unrequested*.
    /// Every step downstream — demux, decode, the presentation clock, [`Page::set_video_frame`] —
    /// was already built and gated, waiting on a URL that no one ever produced.
    ///
    /// **It returns `(NodeId, url)` pairs, not bare URLs, and the pair is the point.** Images can be
    /// answered by URL alone (`apply_images_by_url` re-walks the DOM and binds one decoded bitmap to
    /// every node naming it) because a bitmap is immutable and shareable. A playing video is not: it
    /// carries a position, and two `<video>` elements pointing at one file are two independent
    /// playbacks that may sit at different times. `set_video_frame` is keyed by `NodeId` for exactly
    /// that reason, so the host must be told which element it is fetching *for*, at request time.
    ///
    /// **Source selection follows the spec's shape, not "the first `<source>`".** HTML's resource
    /// selection walks the `<source>` children in order and takes the first whose `type` the UA does
    /// not reject — which is why sites list WebM before MP4 and expect an MP4-only UA to skip past
    /// the WebM. Taking the first child unconditionally would fetch a file that cannot decode while
    /// a playable one sat two lines below it, and the failure would look like a broken decoder.
    /// A `type` we affirmatively reject is skipped; an absent or unrecognised `type` is *attempted*,
    /// because the container sniffer downstream is the honest authority and refusing to try is how a
    /// UA breaks a stream it could actually have played.
    ///
    /// Cheap and side-effect-free: a DOM walk, no network, no decode. Callers dedupe by `NodeId`.
    pub fn pending_media_urls(&self) -> Vec<(manuk_dom::NodeId, String)> {
        let mut out: Vec<(manuk_dom::NodeId, String)> = Vec::new();
        for n in self.dom.flat_descendants(self.dom.root()) {
            let Some(el) = self.dom.element(n) else {
                continue;
            };
            match self.dom.tag_name(n) {
                Some("video") | Some("audio") => {}
                _ => continue,
            }
            // A `src` on the element itself wins outright — the spec never consults `<source>`
            // children when the attribute is present.
            let chosen = match el.attr("src").map(str::trim).filter(|s| !s.is_empty()) {
                Some(src) => Some(src.to_string()),
                None => self
                    .dom
                    .flat_descendants(n)
                    .into_iter()
                    .filter(|&c| self.dom.tag_name(c) == Some("source"))
                    .find_map(|c| {
                        let se = self.dom.element(c)?;
                        let src = se.attr("src").map(str::trim).filter(|s| !s.is_empty())?;
                        if se.attr("type").is_some_and(|t| media_type_rejected(t)) {
                            return None;
                        }
                        Some(src.to_string())
                    }),
            };
            let Some(src) = chosen else { continue };
            out.push((n, resolve_url(&self.final_url, &src)));
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
        // Inline `<svg>` rasterization (tick 394) — BEFORE the empty-early-return, because a page
        // whose only imagery is inline svg (an icon-heavy app shell) fetches zero network images
        // and still must paint its vectors.
        let new_svgs = self.decode_inline_svgs();
        if images.is_empty() && new_svgs == 0 {
            return 0;
        }
        // Natural sizing — see `apply_natural_size`, shared with the inline-`data:` pass so an image
        // is sized the same way regardless of whether its bytes came off the network or out of its
        // own URL. Inline svgs are deliberately NOT natural-sized: the measured replaced-sizing
        // model (t389/391 — default object size + viewBox ratio) owns their geometry; the raster
        // only paints into whatever box layout gave them.
        for (&node, img) in &images {
            if let Some(style) = self.styles.get_mut(&node) {
                apply_natural_size(style, img);
            }
        }
        let count = images.len();
        self.images = images;
        for (n, img) in &self.inline_svg_cache {
            self.images.entry(*n).or_insert_with(|| img.clone());
        }
        self.relayout(fonts, viewport_width);
        count + new_svgs
    }

    /// Rasterize every inline `<svg>` element not yet in the cache (the paint half of the
    /// tick-393 SVG-internals spec — geometry mapping is the other half). The subtree is
    /// serialized back to markup and decoded through the SAME usvg/resvg path as `<img
    /// src="*.svg">`; HTML-parsed svgs usually lack the `xmlns` declaration usvg requires, so
    /// one is injected when absent. Returns how many NEW svgs were decoded.
    /// Decode-and-merge for the SYNC construction paths (`load`, `from_prefetched`), which never
    /// pass through `apply_images`: inline svg needs no network, so those pages must paint their
    /// vectors too.
    fn rasterize_inline_svgs_into_images(&mut self) {
        self.decode_inline_svgs();
        for (n, img) in &self.inline_svg_cache {
            self.images.entry(*n).or_insert_with(|| img.clone());
        }
    }

    fn decode_inline_svgs(&mut self) -> usize {
        let svgs: Vec<manuk_dom::NodeId> = self
            .dom
            .descendants(self.dom.root())
            .filter(|&n| self.dom.tag_name(n) == Some("svg"))
            .filter(|n| !self.inline_svg_cache.contains_key(n))
            .collect();
        if svgs.is_empty() {
            return 0;
        }
        // Built once for the whole pass, not once per `<use>`: see `IdIndex`. This is also why the
        // early return above is not cosmetic — this function is called again on every dynamic-script
        // round, and most of those rounds have no new `<svg>` to decode.
        let ids = svg_geometry::IdIndex::build(&self.dom);
        let mut new = 0usize;
        for node in svgs {
            let mut markup = manuk_html::serialize_outer(&self.dom, node);
            if !markup.contains("xmlns") {
                if let Some(rest) = markup.strip_prefix("<svg") {
                    markup = format!("<svg xmlns=\"http://www.w3.org/2000/svg\"{rest}");
                }
            }
            // ⚠⚠ **THE SPRITE IDIOM WAS DANGLING BY CONSTRUCTION.** This serialises ONE `<svg>`
            // element, and the way icons ship is a `<symbol>` sheet in one `<svg>` and
            // `<use href="#icon">` in another — so every such reference named an id that was not in
            // the string usvg was handed. usvg drops an unresolvable `<use>`, so the icon did not
            // paint at all and the geometry pass was measuring a blank. Inject the definitions the
            // subtree reaches outside itself, inside a `<defs>` so they stay invisible.
            if let Some(defs) = svg_geometry::external_use_defs(&self.dom, &ids, node) {
                if let Some(cut) = markup.rfind("</svg>") {
                    markup.insert_str(cut, &defs);
                }
            }
            // ── THE GEOMETRY HALF, from the SAME markup string as the raster.
            //
            // Deriving both from one serialization is the point: a `<path>` measured against a
            // document the painter never saw is a number with nothing behind it. `map_inline_svg`
            // refuses (returns `None`) far more readily than it answers — see its module docs — and
            // a refusal is recorded as "no entry", which leaves that svg's CSS-derived boxes alone.
            if let Some(boxes) = svg_geometry::map_inline_svg(&self.dom, &ids, node, &markup) {
                self.svg_geometry.insert(node, boxes);
            }
            if let Some(img) = decode_svg(markup.as_bytes(), "inline.svg") {
                self.inline_svg_cache.insert(node, std::rc::Rc::new(img));
                new += 1;
            }
        }
        new
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
        // Scoped to this run of rounds, not to the navigation: a flag whose reset lives somewhere
        // else eventually leaks and silently stops a healthy page from running its scripts.
        manuk_js::event_loop::clear_convergence_state();
        for _ in 0..max_rounds {
            let csp = &self.csp;
            let pending: Vec<(manuk_dom::NodeId, String)> = self
                .dom
                .descendants(self.dom.root())
                .filter(|&n| self.dom.tag_name(n) == Some("script"))
                // A node in `dyn_scripts_ran` has already been evaluated. This used to be read off
                // the DOM ("`src` was deleted, so it ran"), which is why a running script could not
                // see its own URL — see the field's own comment.
                .filter(|n| !self.dyn_scripts_ran.contains(n))
                .filter_map(|n| {
                    let src = self.dom.element(n)?.attr("src")?.trim().to_string();
                    (!src.is_empty()).then(|| (n, resolve_url(&self.final_url, &src)))
                })
                // **This is the path an injected `<script src>` takes**, so it is the one CSP was
                // written for: the initial parse is authored markup, but a script appended at
                // runtime is what an XSS payload does. Enforcing on the first pass only would have
                // been enforcement on the half that matters least.
                .filter(|(_, url)| match Url::parse(url) {
                    Ok(u) => {
                        let ok = csp.allows_script_url(&u);
                        if !ok {
                            tracing::info!(%url, "CSP blocked a dynamically added <script src>");
                        }
                        ok
                    }
                    Err(_) => true,
                })
                .collect();
            if pending.is_empty() {
                break;
            }
            let fetched =
                futures_util::future::join_all(pending.into_iter().map(|(node, url)| async move {
                    let text = manuk_net::fetch(&url)
                        .await
                        .ok()
                        .and_then(|r| subresource_text(&r));
                    (node, text)
                }))
                .await;

            let mut any = false;
            for (node, text) in fetched {
                // Mark it run *first*: a script that throws must not be retried forever. The mark is
                // the SET, not the deletion of `src` — the attribute now outlives the evaluation so
                // that `document.currentScript.src` is the URL while the script runs, and is removed
                // below once it has.
                self.dyn_scripts_ran.insert(node);
                let Some(ctx) = &self.js else { continue };
                let rects: HashMap<manuk_dom::NodeId, [f32; 4]> = self
                    .root_box
                    .node_rects(&self.dom)
                    .into_iter()
                    .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
                    .collect();
                let fetched_ok = text.is_some();
                if let Some(src) = &text {
                    if !src.trim().is_empty() {
                        if let Err(e) = manuk_js::eval_in_page(
                            ctx,
                            &mut self.dom,
                            src,
                            &rects,
                            &self.styles,
                            Some(node),
                        ) {
                            tracing::warn!("dynamic script: {e}");
                        }
                        ran += 1;
                    }
                }
                // ── **AND THEN TELL THE PAGE, which is the half that was missing.**
                //
                // We fetched injected `<script src>` and ran it correctly — and never fired `load`
                // or `error` on the element, so no loader on the page could ever learn that its
                // script had arrived. **Every chunked bundler build depends on exactly this event.**
                // webpack's `__webpack_require__.l` does `script.onerror = script.onload =
                // onScriptComplete` and arms a timer; with neither event it falls through to the
                // timer and rejects with `ChunkLoadError`.
                //
                // Measured on `www.agoda.com` (top-100k): chunk 5035 executes fine, webpack is never
                // told, its timeout fires, the app never converges — **the event loop hit its 20,000
                // task ceiling ten times (~22s), which exhausted the 12s load budget, and the page
                // painted BLANK.** One undelivered event, a completely white page.
                //
                // The class is much wider than bundlers: every `loadScript()` helper on the web
                // resolves its promise here — analytics, tag managers, ad tags, reCAPTCHA, payment
                // and map SDKs. A script loader with no completion signal does not fail loudly; it
                // waits, and the page it was bootstrapping never starts.
                //
                // **`load` fires even if the script THREW.** The event reports that fetching and
                // running happened, not that the code succeeded — a throw goes to `window.onerror`.
                // Making `load` conditional on success would strand every loader whose chunk has a
                // benign error, which is the same bug with a smaller blast radius.
                let ty = if fetched_ok { "load" } else { "error" };
                let _ =
                    manuk_js::dispatch_event(ctx, &mut self.dom, node, ty, &rects, &self.styles);
                // `src` is dropped HERE rather than before the evaluation, so the post-run document
                // is exactly what it has always been (`collect_inline_scripts` reads a surviving
                // `src` as "the fetch failed, nothing to run") while the script itself still saw its
                // own URL. The `load`/`error` handler above is a script-visible callback too, so the
                // removal comes after it for the same reason.
                self.dom.remove_attr(node, "src");
                // A handler almost always injects the NEXT script (that is what a chunk loader is
                // for), so this round counts as progress even when the body itself was empty.
                any = true;
            }
            if !any {
                break;
            }
            // **STOP ASKING A PAGE THAT HAS ALREADY ANSWERED.**
            //
            // The drain's bounds bound A DRAIN. The promise beside them is about A PAGE, and this loop
            // is where the difference lived: a page that both spins and injects scripts paid the bound
            // once per round. Measured at the page level on `www.agoda.com` (t666), three runs within
            // a second of each other: `finish_loading` 39.9s against its own 12s budget, 17 drains at
            // ~2.3s each.
            //
            // **The `tokio::time::timeout` around the phases cannot enforce that budget**, because a
            // timeout fires only at an await point and these drains are synchronous JavaScript. So the
            // bound has to be a decision made BETWEEN rounds — here.
            //
            // A page that burned 20,000 tasks without converging will not converge in the next 20,000.
            // This can only REDUCE work, and only for a page the engine has already declared
            // non-converging out loud, so it cannot cost a working page a round.
            if manuk_js::event_loop::page_stopped_converging() {
                tracing::warn!(
                    rounds_run = ran,
                    "a script round ended with the page not converging — not starting another. The \
                     drain's ceiling bounds one drain; this bounds the NAVIGATION."
                );
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
        let _reflow = ReflowScope::install(
            &self.dom,
            fonts,
            viewport_width,
            &self.images,
            &self.final_url,
            &self.external_css,
            &self.scroll_offsets,
            self.sticky_scroll_y,
            self.has_sticky,
        );
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
            let _reflow = ReflowScope::install(
                &self.dom,
                fonts,
                viewport_width,
                &self.images,
                &self.final_url,
                &self.external_css,
                &self.scroll_offsets,
                self.sticky_scroll_y,
                self.has_sticky,
            );
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

    /// **Drop files onto `node` — the actuation a drag-and-drop upload performs.** Returns `false`
    /// iff a handler called `preventDefault()` on the `drop`.
    ///
    /// The other half of [`set_input_files`](Self::set_input_files), and not a niche one: Gmail
    /// attachments, GitHub issue images, Slack, Drive and every modern uploader put a dashed
    /// rectangle on the screen and read `e.dataTransfer.files`. With `DataTransfer` inert that read
    /// was `undefined.files` — **a TypeError inside the `drop` handler**, so the page did not merely
    /// ignore the drop, its handler threw.
    ///
    /// Fires `dragenter`, `dragover`, `drop` in order, sharing one `DataTransfer`. See
    /// [`manuk_js::PageContext::dispatch_drop`] for why all three are required rather than just the
    /// last: the page opts *in* to being a drop target by cancelling `dragover`, and a dropzone that
    /// never receives one never receives a drop either.
    pub fn dispatch_drop(
        &mut self,
        node: manuk_dom::NodeId,
        files: &[(String, String, String)],
        fonts: &FontContext,
        viewport_width: f32,
    ) -> bool {
        let _json = files_to_json(files);
        #[cfg(feature = "spidermonkey")]
        {
            let Some(ctx) = &self.js else {
                self.relayout(fonts, viewport_width);
                return true;
            };
            let rects: HashMap<manuk_dom::NodeId, [f32; 4]> = self
                .root_box
                .node_rects(&self.dom)
                .into_iter()
                .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
                .collect();
            let _reflow = ReflowScope::install(
                &self.dom,
                fonts,
                viewport_width,
                &self.images,
                &self.final_url,
                &self.external_css,
                &self.scroll_offsets,
                self.sticky_scroll_y,
                self.has_sticky,
            );
            let proceed = match manuk_js::dispatch_drop(
                ctx,
                &mut self.dom,
                node,
                &_json,
                &rects,
                &self.styles,
            ) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!("drop dispatch: {e}");
                    true
                }
            };
            self.relayout(fonts, viewport_width);
            return proceed;
        }
        #[cfg(not(feature = "spidermonkey"))]
        {
            self.relayout(fonts, viewport_width);
            true
        }
    }

    /// **Drag `source` onto `target` within the page — the reorder gesture.**
    ///
    /// The source side of drag-and-drop, the half a sortable list or kanban board originates itself
    /// (dragging a row to a new position, a card to another column). Fires `dragstart` on the source,
    /// then `dragenter`/`dragover`/`drop` on the target, then `dragend` on the source, all sharing one
    /// `DataTransfer` — so the id the source's `dragstart` writes with `setData` is the id the
    /// target's `drop` reads with `getData`. See [`manuk_js::PageContext::dispatch_drag`]. Returns
    /// `false` iff a handler `preventDefault()`-ed the `drop`.
    pub fn dispatch_drag(
        &mut self,
        source: manuk_dom::NodeId,
        target: manuk_dom::NodeId,
        fonts: &FontContext,
        viewport_width: f32,
    ) -> bool {
        #[cfg(feature = "spidermonkey")]
        {
            let Some(ctx) = &self.js else {
                self.relayout(fonts, viewport_width);
                return true;
            };
            let rects: HashMap<manuk_dom::NodeId, [f32; 4]> = self
                .root_box
                .node_rects(&self.dom)
                .into_iter()
                .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
                .collect();
            let _reflow = ReflowScope::install(
                &self.dom,
                fonts,
                viewport_width,
                &self.images,
                &self.final_url,
                &self.external_css,
                &self.scroll_offsets,
                self.sticky_scroll_y,
                self.has_sticky,
            );
            let proceed = match manuk_js::dispatch_drag(
                ctx,
                &mut self.dom,
                source,
                target,
                &rects,
                &self.styles,
            ) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!("drag dispatch: {e}");
                    true
                }
            };
            self.relayout(fonts, viewport_width);
            return proceed;
        }
        #[cfg(not(feature = "spidermonkey"))]
        {
            let _ = (source, target);
            self.relayout(fonts, viewport_width);
            true
        }
    }

    /// **Double-click `node` — the full sequence, not a bare `dblclick`.**
    ///
    /// Fires `click` (detail 1), `click` (detail 2), then `dblclick` (detail 2), which is the order
    /// and the numbering a real browser produces. Returns `false` iff a handler called
    /// `preventDefault()` on the `dblclick`.
    ///
    /// **Firing `dblclick` alone would be the shape of a fix that reads as complete and is not.**
    /// `event.detail` is the click count, and `if (e.detail === 2)` on an ordinary `click` listener
    /// is the idiomatic way to handle double-click — used precisely because it does not require a
    /// separate listener. A dispatcher that emits only `dblclick` leaves that branch permanently
    /// unreachable, and also skips the two `click` handlers a real double-click always runs, so a
    /// page that (say) selects on first click and opens on second would open something it never
    /// selected. The two clicks are the interaction; `dblclick` is the notification that it happened.
    ///
    /// The `click`s route through [`Page::dispatch_click`] rather than being synthesised here, so
    /// label-forwarding, checkbox activation and the disabled check all behave exactly as they do
    /// for a single click — a double-click on a `<label>` must still reach its control twice.
    pub fn dispatch_dblclick(
        &mut self,
        node: manuk_dom::NodeId,
        fonts: &FontContext,
        viewport_width: f32,
    ) -> bool {
        // The two constituent clicks, with their real activation behaviour and their real click
        // COUNTS — the second click of a double-click reports `detail: 2`, which is what a page
        // handling double-click on a plain `click` listener branches on.
        self.dispatch_click_detail(node, 1, fonts, viewport_width);
        self.dispatch_click_detail(node, 2, fonts, viewport_width);

        #[cfg(feature = "spidermonkey")]
        {
            let Some(ctx) = &self.js else {
                self.relayout(fonts, viewport_width);
                return true;
            };
            let rects: HashMap<manuk_dom::NodeId, [f32; 4]> = self
                .root_box
                .node_rects(&self.dom)
                .into_iter()
                .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
                .collect();
            let _reflow = ReflowScope::install(
                &self.dom,
                fonts,
                viewport_width,
                &self.images,
                &self.final_url,
                &self.external_css,
                &self.scroll_offsets,
                self.sticky_scroll_y,
                self.has_sticky,
            );
            let proceed = match manuk_js::dispatch_mouse(
                ctx,
                &mut self.dom,
                node,
                "dblclick",
                2,
                0,
                &rects,
                &self.styles,
            ) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!("dblclick dispatch: {e}");
                    true
                }
            };
            self.relayout(fonts, viewport_width);
            return proceed;
        }
        #[cfg(not(feature = "spidermonkey"))]
        {
            self.relayout(fonts, viewport_width);
            true
        }
    }

    /// **Right-click `node`.** Returns `false` iff a handler called `preventDefault()` — which is
    /// the whole point of the call rather than a detail of it.
    ///
    /// `contextmenu` is cancelable, and cancelling it is how *every* page with a custom right-click
    /// menu works: the handler calls `preventDefault()` and draws its own menu. So the return value
    /// is the browser's question — *"did the page take over?"* — and a browser that ignored a
    /// `false` would render its native menu on top of the page's own. Same shape as the drop verdict
    /// in tick 248, and the same reason it is returned rather than discarded.
    ///
    /// `button: 2` is passed rather than defaulted: handlers guarding on `e.button === 2` are common
    /// enough that dispatching with the left-button default would silently skip them.
    pub fn dispatch_contextmenu(
        &mut self,
        node: manuk_dom::NodeId,
        fonts: &FontContext,
        viewport_width: f32,
    ) -> bool {
        #[cfg(feature = "spidermonkey")]
        {
            let Some(ctx) = &self.js else {
                self.relayout(fonts, viewport_width);
                return true;
            };
            let rects: HashMap<manuk_dom::NodeId, [f32; 4]> = self
                .root_box
                .node_rects(&self.dom)
                .into_iter()
                .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
                .collect();
            let _reflow = ReflowScope::install(
                &self.dom,
                fonts,
                viewport_width,
                &self.images,
                &self.final_url,
                &self.external_css,
                &self.scroll_offsets,
                self.sticky_scroll_y,
                self.has_sticky,
            );
            let proceed = match manuk_js::dispatch_mouse(
                ctx,
                &mut self.dom,
                node,
                "contextmenu",
                0,
                2,
                &rects,
                &self.styles,
            ) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!("contextmenu dispatch: {e}");
                    true
                }
            };
            self.relayout(fonts, viewport_width);
            return proceed;
        }
        #[cfg(not(feature = "spidermonkey"))]
        {
            self.relayout(fonts, viewport_width);
            true
        }
    }

    /// **Choose an option in a `<select>` — the actuation the native dropdown performs.** Returns
    /// `false` if `node` is not a `<select>` or `index` is out of range.
    ///
    /// **This is the last common form control an agent could not drive.** Country pickers, currency,
    /// quantity, sort order, shipping method, every settings page — all of them read `select.value`
    /// and branch, and the native dropdown is an OS-drawn popup with no scriptable surface. Nothing
    /// was broken; there was no door, exactly as with the file picker in tick 247.
    ///
    /// **`input` THEN `change`, and both are required.** React's `onChange` is really the `input`
    /// event, so a host that fires only `change` leaves every React select unchanged — while a
    /// vanilla page listening for `change` would work, which is the kind of split that reads as "it
    /// works on some sites". Firing only `input` fails the mirror image. The order is the spec's.
    pub fn select_option(
        &mut self,
        node: manuk_dom::NodeId,
        index: usize,
        fonts: &FontContext,
        viewport_width: f32,
    ) -> bool {
        let is_select = self
            .dom
            .element(node)
            .map(|e| e.name == "select")
            .unwrap_or(false);
        if !is_select {
            return false;
        }
        let options: Vec<manuk_dom::NodeId> = self
            .dom
            .flat_descendants(node)
            .into_iter()
            .filter(|n| {
                self.dom
                    .element(*n)
                    .map(|e| e.name == "option")
                    .unwrap_or(false)
            })
            .collect();
        let Some(&chosen) = options.get(index) else {
            return false;
        };
        // Exactly one selected — a control with two marked options has no defined value.
        for &o in &options {
            if o == chosen {
                self.dom.set_attr(o, "selected", "");
            } else {
                self.dom.remove_attr(o, "selected");
            }
        }

        #[cfg(feature = "spidermonkey")]
        {
            if let Some(ctx) = &self.js {
                let rects: HashMap<manuk_dom::NodeId, [f32; 4]> = self
                    .root_box
                    .node_rects(&self.dom)
                    .into_iter()
                    .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
                    .collect();
                let _reflow = ReflowScope::install(
                    &self.dom,
                    fonts,
                    viewport_width,
                    &self.images,
                    &self.final_url,
                    &self.external_css,
                    &self.scroll_offsets,
                    self.sticky_scroll_y,
                    self.has_sticky,
                );
                for ty in ["input", "change"] {
                    if let Err(e) =
                        manuk_js::dispatch_event(ctx, &mut self.dom, node, ty, &rects, &self.styles)
                    {
                        tracing::warn!("select {ty} dispatch: {e}");
                    }
                }
            }
        }
        self.relayout(fonts, viewport_width);
        true
    }

    /// **Choose files on an `<input type=file>` — the actuation a file picker performs.** Returns
    /// `false` if `node` is not a file input.
    ///
    /// **This is the entry point that makes every upload flow on the web drivable.** Uploading is
    /// the one common interaction an agent could not reach: the bytes normally arrive through a
    /// native OS picker dialog, which has no scriptable surface, so avatar/attachment/document/photo
    /// flows were all dead ends. Nothing was broken — there was simply no door.
    ///
    /// `files` is `(name, mime_type, contents)`. The selection is stored on the element as
    /// `data-manuk-files` (JSON), which is what the `input.files` getter in the JS prelude reads;
    /// see the `FileList` block in `dom_bindings` for why the data lives in an attribute and what
    /// that costs.
    ///
    /// **`value` is set to `C:\fakepath\<name>`, and the fake path is not a joke.** It is what every
    /// browser reports, deliberately: the real path is withheld from the page (it leaks the user's
    /// username and directory layout), and the `C:\fakepath\` prefix specifically is in the spec
    /// because sites had already been written to parse a Windows path out of `value`, so returning a
    /// bare filename broke them. A page reading `value` to show "a.txt" splits on `\` and still works.
    ///
    /// Fires **`input` then `change`**, in that order, exactly as a real picker does — `change` is
    /// the event upload widgets actually listen on, and firing only `input` leaves the file chosen
    /// and the page unaware.
    pub fn set_input_files(
        &mut self,
        node: manuk_dom::NodeId,
        files: &[(String, String, String)],
        fonts: &FontContext,
        viewport_width: f32,
    ) -> bool {
        let is_file_input = self
            .dom
            .element(node)
            .map(|e| {
                e.name.eq_ignore_ascii_case("input")
                    && e.attr("type")
                        .is_some_and(|t| t.eq_ignore_ascii_case("file"))
            })
            .unwrap_or(false);
        if !is_file_input {
            return false;
        }
        let json = files_to_json(files);
        self.dom.set_attr(node, "data-manuk-files", json);
        let shown = files
            .first()
            .map(|(name, _, _)| format!("C:\\fakepath\\{name}"))
            .unwrap_or_default();
        self.dom.set_attr(node, "value", shown);
        #[cfg(feature = "spidermonkey")]
        {
            let Some(ctx) = &self.js else {
                self.relayout(fonts, viewport_width);
                return true;
            };
            let rects: HashMap<manuk_dom::NodeId, [f32; 4]> = self
                .root_box
                .node_rects(&self.dom)
                .into_iter()
                .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
                .collect();
            let _reflow = ReflowScope::install(
                &self.dom,
                fonts,
                viewport_width,
                &self.images,
                &self.final_url,
                &self.external_css,
                &self.scroll_offsets,
                self.sticky_scroll_y,
                self.has_sticky,
            );
            for ty in ["input", "change"] {
                if let Err(e) =
                    manuk_js::dispatch_event(ctx, &mut self.dom, node, ty, &rects, &self.styles)
                {
                    tracing::warn!("{ty} dispatch: {e}");
                }
            }
        }
        self.relayout(fonts, viewport_width);
        true
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
            let _reflow = ReflowScope::install(
                &self.dom,
                fonts,
                viewport_width,
                &self.images,
                &self.final_url,
                &self.external_css,
                &self.scroll_offsets,
                self.sticky_scroll_y,
                self.has_sticky,
            );
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
    /// derived from `key`. No JS context → always `true` (perform the default). This is the
    /// no-modifier form; use [`Page::dispatch_key_mods`] to carry Ctrl/Shift/Alt/Meta.
    pub fn dispatch_key(
        &mut self,
        node: manuk_dom::NodeId,
        ty: &str,
        key: &str,
        fonts: &FontContext,
        viewport_width: f32,
    ) -> bool {
        self.dispatch_key_mods(
            node,
            ty,
            key,
            KeyModifiers::default(),
            fonts,
            viewport_width,
        )
    }

    /// Like [`Page::dispatch_key`] but carrying the four `KeyboardEvent` modifier flags. The
    /// dispatched event exposes `ctrlKey`/`shiftKey`/`altKey`/`metaKey` — so a page's keyboard-shortcut
    /// handler (`if (e.metaKey && e.key === 'k')`, the Cmd/Ctrl+K command palette in Slack/Notion/
    /// Linear/GitHub; a composer inserting a newline only on `Shift+Enter`) fires — and a shortcut
    /// CHORD (`ctrl`/`meta` held) does NOT insert a stray character into a contenteditable.
    pub fn dispatch_key_mods(
        &mut self,
        node: manuk_dom::NodeId,
        ty: &str,
        key: &str,
        mods: KeyModifiers,
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
                mods,
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
            let _ = (node, ty, key, mods, fonts, viewport_width);
            true
        }
    }

    /// Dispatch the **IME composition-commit sequence** that commits `data` into `node` —
    /// `compositionstart`, `compositionupdate`, `beforeinput` (`inputType: insertCompositionText`),
    /// `input`, `compositionend`. This is how CJK and accented text actually arrives: the user
    /// composes phonetic/romanised input in an IME buffer and commits a character; there is no
    /// per-character `keydown` for the committed glyph. Returns `true` unless a handler
    /// `preventDefault()`-ed the `beforeinput` (an editor vetoing the insert). See
    /// [`manuk_js::PageContext::dispatch_composition`] for why the whole ordered sequence and the
    /// `isComposing` flag matter to a rich editor.
    pub fn dispatch_composition(
        &mut self,
        node: manuk_dom::NodeId,
        data: &str,
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
            let proceed = match manuk_js::dispatch_composition(
                ctx,
                &mut self.dom,
                node,
                data,
                &rects,
                &self.styles,
            ) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!("composition dispatch: {e}");
                    return true;
                }
            };
            self.relayout(fonts, viewport_width);
            proceed
        }
        #[cfg(not(feature = "spidermonkey"))]
        {
            let _ = (node, data, fonts, viewport_width);
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
            let _reflow = ReflowScope::install(
                &self.dom,
                fonts,
                viewport_width,
                &self.images,
                &self.final_url,
                &self.external_css,
                &self.scroll_offsets,
                self.sticky_scroll_y,
                self.has_sticky,
            );
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
        // ⚠⚠ **BEFORE the `wants_view_events` early-out, and that placement is the point.** A page
        // with no scroll listener and no observer still has a sticky header, and the DOM must still
        // be able to say where it is. Gating the re-bake on "is anything listening?" would make
        // `getBoundingClientRect` correct on exactly the pages that were already asking and wrong on
        // the quiet ones — a bug visible only to the careful caller (t1267).
        self.restick(scroll_y);
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

    /// Without SpiderMonkey there is nothing to *notify* — but sticky is layout, not script, so the
    /// re-bake still has to happen or the JS-less build reports a different geometry from the
    /// shipping one.
    #[cfg(not(feature = "spidermonkey"))]
    pub fn view_changed(&mut self, scroll_y: f32, _vw: f32, _vh: f32, _scrolled: bool) {
        self.restick(scroll_y);
    }

    /// Evaluate a script in the page's context. Used by the conformance suite to read state back
    /// out of the JS world through the DOM, which is the only channel a test has into a page.
    /// The painted bitmap the parent holds for `node` — the pixels an `<iframe>` (or `<img>`, or
    /// `<canvas>`) actually shows. Exposed so a gate can assert what is ON SCREEN rather than what the
    /// DOM says, which for frames are two different questions.
    ///
    /// Deliberately NOT feature-gated: pixels exist in the JS-less build too.
    pub fn image_for(
        &self,
        node: manuk_dom::NodeId,
    ) -> Option<&std::rc::Rc<manuk_paint::DecodedImage>> {
        self.images.get(&node)
    }

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
            &self.dom,
            &self.root_box,
            &self.styles,
            &self.scroll_offsets,
        ));
        manuk_js::set_snap_candidates(self.snap_candidates_map());
        self.publish_image_sources();
        let _ = manuk_js::eval_in_page(ctx, &mut self.dom, src, &rects, &self.styles, None);
        self.drain_canvases();
        self.drain_element_scrolls();
    }

    /// Fire `pageswap` on the OUTGOING document — the MPA companion to `pagereveal` (t372/373).
    /// Called by the host at the one moment it is true: navigation is committed and this page is
    /// about to be replaced. `.viewTransition` is null (the spec's no-transition value — no
    /// cross-document transition animation exists here, a named non-claim).
    #[cfg(feature = "spidermonkey")]
    pub fn fire_pageswap(&mut self) {
        let Some(ctx) = &self.js else { return };
        let rects: HashMap<manuk_dom::NodeId, [f32; 4]> = self
            .root_box
            .node_rects(&self.dom)
            .into_iter()
            .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
            .collect();
        let js = "(function(){ try { var e; try { e = new Event('pageswap'); } \
                  catch(_) { e = { type: 'pageswap' }; } e.viewTransition = null; \
                  dispatchEvent(e); } catch(_) {} })()";
        if let Err(e) = manuk_js::eval_in_page(ctx, &mut self.dom, js, &rects, &self.styles, None) {
            tracing::debug!("pageswap: {e}");
        }
    }

    #[cfg(not(feature = "spidermonkey"))]
    pub fn fire_pageswap(&mut self) {}

    /// **Fire the document lifecycle: `DOMContentLoaded`, then `load`.**
    ///
    /// Neither event was EVER dispatched by this engine — grep found zero occurrences. A site whose
    /// init lives in `window.addEventListener('load', …)` — a very large fraction of the web —
    /// simply **never initialised**, silently, with nothing in any log to say so.
    ///
    /// The host must fire these because **only the host knows when they are true**: *"the document
    /// finished parsing"* and *"the subresources finished"* are facts about the loader, not about JS.
    /// ⚠⚠⚠ **AND IT IS THE ONE SCRIPT ROUND OF EIGHTEEN THAT HAD NO `ReflowScope`.** Everything a
    /// `load` or `DOMContentLoaded` handler appended had **no geometry, and never got any** — not
    /// on a later read, not after writing the node's own style, not on the next task:
    ///
    /// ```text
    ///   append during parse, read during parse            550   CONTROL
    ///   append in the load handler, read there              0   <- the defect
    ///   ...re-read after writing the node's OWN style       0
    ///   ...re-read one task later (setTimeout)              0   <- it never recovers
    ///   a node that has existed since parse                550   CONTROL
    /// ```
    ///
    /// A geometry read is supposed to force a synchronous reflow: the binding calls the host, the
    /// host re-cascades and re-lays-out, the read answers fresh. `ReflowScope::install` arms that
    /// hook, and `eval_for_test` — the function this one delegates to — takes neither `fonts` nor a
    /// viewport width, so it could not arm it and silently did not. Every other re-entry into
    /// script (events, timers, rAF, scroll) installs one. **`window.addEventListener('load', …)` is
    /// where a very large fraction of the web does its DOM building**, and every box it built
    /// measured zero.
    ///
    /// So the lifecycle takes the same two arguments every other script re-entry does. They were
    /// already in scope at all four call sites; nothing outside this file calls it.
    #[cfg(feature = "spidermonkey")]
    pub fn fire_lifecycle(&mut self, which: &str, fonts: &FontContext, viewport_width: f32) {
        // The thread-local is shared by every `Page` on this thread, so re-publish rather than trust it.
        self.publish_iframe_docs(fonts);
        let src = match which {
            "DOMContentLoaded" => "globalThis.__fireDOMContentLoaded && __fireDOMContentLoaded()",
            _ => "globalThis.__fireLoad && __fireLoad()",
        };
        let _reflow = ReflowScope::install(
            &self.dom,
            fonts,
            viewport_width,
            &self.images,
            &self.final_url,
            &self.external_css,
            &self.scroll_offsets,
            self.sticky_scroll_y,
            self.has_sticky,
        );
        self.eval_for_test(src);
    }

    /// Without SpiderMonkey there is no JS to notify, so the lifecycle is a no-op — not a lie.
    #[cfg(not(feature = "spidermonkey"))]
    pub fn fire_lifecycle(&mut self, _which: &str, _fonts: &FontContext, _viewport_width: f32) {}

    /// **Tell the page whether its tab is in front** — `document.visibilityState` / `document.hidden`,
    /// and the `visibilitychange` event when it flips.
    ///
    /// Like the lifecycle above, this is a fact the host owns and JS cannot observe for itself:
    /// *"this tab was backgrounded"* is a statement about the shell's window, not about the
    /// document. Until this existed the property was `undefined`, which reads FALSY — so every
    /// `if (document.hidden) return;` guard on the page failed open and a hidden tab kept
    /// animating, polling and decoding.
    ///
    /// Idempotent by value on the JS side: setting the state we are already in fires no event, so a
    /// shell that re-publishes on every frame does not deliver a storm of change events.
    #[cfg(feature = "spidermonkey")]
    pub fn set_visibility(&mut self, hidden: bool) {
        let src = if hidden {
            "globalThis.__setVisibility && __setVisibility('hidden')"
        } else {
            "globalThis.__setVisibility && __setVisibility('visible')"
        };
        self.eval_for_test(src);
    }

    /// Without SpiderMonkey nothing is listening, so there is no one to tell.
    #[cfg(not(feature = "spidermonkey"))]
    pub fn set_visibility(&mut self, _hidden: bool) {}

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
        // **Marked AFTER the fetch, and only for what answered.** This ran before it, over every
        // target, which was harmless while the whole phase died together — the mutation was dropped
        // with the future. Now the phase survives its deadline, so marking a mask nobody ever
        // declined to send would retire that icon for the rest of the navigation.
        let (owned, settled) = fetch_masks_owned(targets, self.load_deadline).await;
        self.fetched_urls.extend(settled);
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
        let fetched = collect_before_deadline(
            targets
                .into_iter()
                .map(|(n, url)| async move { (n, fetch_image_bytes(&url).await, url) }),
            self.load_deadline,
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
        page.fire_lifecycle("DOMContentLoaded", fonts, viewport_width);
        // On the SYNC path there is no async runtime to fetch subframes with; a caller that needs a
        // frame's document (a gate) drives `render_iframe` directly. The async path
        // (`load_async`/`finish_loading`) is where real frames load.
        //
        // Inline `<svg>` needs no network either — rasterize on this path too (tick 394), so
        // gates and shell navigations paint vectors, not only the fetch path via `apply_images`.
        page.rasterize_inline_svgs_into_images();
        page.fire_lifecycle("load", fonts, viewport_width);
        page
    }

    fn from_prefetched_inner(pre: Prefetched, fonts: &FontContext, viewport_width: f32) -> Page {
        let Prefetched {
            dom,
            final_url,
            csp,
            csp_authorized_scripts,
            external_scripts,
            css,
            images,
            masks,
            module_graph_sources,
            module_node_bases,
            import_map,
        } = pre;
        // Seeded before construction, because `from_dom` runs the document's blocking scripts and
        // the policy has to be in force by then — not after. Same for the external-script set: each
        // one owes its element a `load` the instant it has executed, which is inside `from_dom`.
        set_pending_csp_with_authorized(csp, csp_authorized_scripts);
        set_pending_external_scripts(external_scripts.into_iter().collect());
        // **AND THE SHELL HAD THE SAME GAP, one copy along.** `load_async`'s comment says this path
        // "has always applied the CSS between `from_dom` and the deferred pass, so its lifecycle
        // events and its timers see a styled document" — true, and it stops one phase short: the
        // BLOCKING scripts run inside `from_dom`, before that apply. On the shell the bytes are
        // already fully fetched (that is what `Prefetched` means), so there is not even a
        // has-it-landed question here — only an ordering one.
        if !css.is_empty() {
            set_pending_external_css(css.clone());
        }
        let mut page = Page::from_dom(dom, &final_url, fonts, viewport_width);
        // Carry the pre-fetched module graph onto the page so `run_deferred_scripts` — which the shell
        // may call much later, after paint — seeds it for the module (deferred) pass. This is the shell
        // path's half of B3b; `load_async` sets the same field on the streaming/agent path.
        page.module_graph_sources = module_graph_sources;
        page.module_node_bases = module_node_bases;
        page.import_map = import_map;
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
        // ── **`CSS.supports()` must answer from the CSS engine, and it was answering from nowhere.**
        //
        // The hook is installed here rather than at binding-registration time because this is the
        // layer that HAS a CSS engine — `manuk-js` deliberately has no CSS dependency, exactly as
        // with the forced-reflow hook. ⚠⚠ It used to be installed in **`Page::load` only**, the
        // synchronous path — so on `load_async` (the agent and every fidelity measurement) and on
        // `from_prefetched` (**the SHELL**, i.e. the shipping browser) the hook was never set and
        // `CSS.supports()` answered **false for everything**:
        //
        // ```text
        //   CHROME  width:5px=true  display:flex=true  color:red=true  width:5lh=true
        //   MANUK   width:5px=FALSE display:flex=FALSE color:red=FALSE width:5lh=FALSE
        // ```
        //
        // `display:flex` is the tell. The Rust-level `supports_condition` has asserted that exact
        // string true since it was written, and its unit test passes — the engine knew the answer and
        // no page could reach it. **A false NEGATIVE on feature detection is not a missing feature,
        // it is every progressive-enhancement guard on the web taking its fallback**: `@supports`-in-JS
        // is how a site decides whether to ship the grid layout or the float one, the modern
        // scroll-snap carousel or the manual, `display:flex` or a table.
        //
        // `from_dom` is the one function every construction path goes through (`load`, `load_async`,
        // `from_prefetched_inner`), which is why the call belongs here and not in three callers —
        // three callers is what produced one.
        install_supports_hook();
        // **The child-document reflow, installed for the same reason and in the same place.** It
        // carries no borrowed state (the host's registry is keyed by arena address), so unlike
        // `ReflowScope` it is installed once and never torn down — and it belongs on the one
        // function every construction path goes through, because three callers is what produced one.
        #[cfg(feature = "spidermonkey")]
        unsafe {
            manuk_js::set_frame_reflow_hook(frame_forced_reflow);
            manuk_js::set_frame_create_hook(frame_create_on_demand);
        };
        // ⚠ A NEW DOCUMENT, so the "already asked" set must go: it is keyed `(arena, NodeId)` and a
        // reused arena address would otherwise inherit the previous document's answers — the
        // `(arena, NodeId)` rule from t1273, which is only safe if somebody clears it.
        #[cfg(feature = "spidermonkey")]
        manuk_js::clear_frame_create_tried();
        // Box the DOM up front so its address is stable for the persistent JS context's raw
        // reflector pointers, then style + lay out once and run the document's inline scripts
        // against that layout snapshot (so `getBoundingClientRect` works), letting them mutate
        // the DOM and register event listeners. If they mutated it, re-style + re-lay-out so
        // script-built content renders. All a no-op unless the `spidermonkey` feature is on.
        // The document's CSP must be in force before its first script runs, and any policy from the
        // PREVIOUS navigation must be gone. Both happen here, on every construction path.
        let csp = install_csp_for_next_page(&dom, final_url);
        let mut dom = Box::new(dom);
        // **The author's external sheets belong in THIS cascade, not the one after the scripts.**
        // See [`PENDING_EXTERNAL_CSS`]: everything below — the first layout and every blocking
        // `<script>` that reads it — used to run against inline `<style>` alone.
        //
        // ⚠ Kept in `seeded_css` rather than consumed on the spot, because the FORCED-REFLOW scope
        // installed for those same scripts rebuilds the cascade from its own sheet list — hand it an
        // empty external map and the first `getComputedStyle` in the document un-styles the page it
        // was called to measure. That is the t1283 family again (a value derived from a snapshot
        // that predates what it describes), and it is why the seed is a variable here.
        let seeded_css: HashMap<String, String> = PENDING_EXTERNAL_CSS
            .with(|p| p.borrow_mut().take())
            .unwrap_or_default();
        let sheets: Vec<Stylesheet> = if seeded_css.is_empty() {
            // Unchanged for every path that seeds nothing (`Page::load`, subframes, gates): the
            // list is byte-for-byte what it was, so a page with no external CSS cannot move.
            MinimalCascade::collect_style_elements(&dom)
        } else {
            let s = initial_sheets_with_external(&dom, final_url, &seeded_css);
            tracing::debug!(
                seeded = seeded_css.len(),
                bytes = seeded_css.values().map(|v| v.len()).sum::<usize>(),
                sheets = s.len(),
                "initial cascade built WITH the author's external stylesheets"
            );
            s
        };
        let mut styles = cascade_styles(&dom, &sheets, viewport_width);
        // **Inline images are sized BEFORE the first layout.** A `data:` image carries its own bytes,
        // so there is nothing to wait for — decoding it here means it has its natural size in the
        // very first box tree, instead of laying out `0x0` and never being corrected on any path that
        // does not run the async subresource pass.
        let mut inline_images = decode_inline_images(&dom, &mut styles);
        let mut root_box = layout_document(&dom, &styles, fonts, viewport_width);
        // The `@container` re-pass, BEFORE `rects`/JS see any styles: the probe scripts below run
        // against the map handed to `load_document`, so a re-pass after them would leave
        // `getComputedStyle` answering from the unsized pass. The re-cascade replaces the style
        // map wholesale, so the inline-image natural sizes annotated above are re-applied.
        if container_query_recascade(
            &dom,
            &sheets,
            viewport_width,
            &mut styles,
            &root_box,
            &inline_images,
        ) {
            let _ = decode_inline_images(&dom, &mut styles);
            root_box = layout_document(&dom, &styles, fonts, viewport_width);
        }

        let rects: std::collections::HashMap<manuk_dom::NodeId, [f32; 4]> = root_box
            .node_rects(&dom)
            .into_iter()
            .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
            .collect();
        // The `Page` does not exist yet, so `set_root_box` cannot be the seam here — but the probe
        // scripts below DO run `getComputedStyle`, and this is the layout they must read.
        publish_grid_tracks();
        // **The network-free frames get their documents HERE, before the first script.** See
        // [`build_inline_frame_docs`]: a parser-inserted `<iframe>` is navigated to `about:blank`
        // at insertion, so the very next `<script>` must find a real `contentDocument`. `rects` is
        // the layout above, which is what gives each frame its own viewport width.
        // The blocking scripts below can create frames of their own, and the `Page` that will adopt
        // them does not exist yet — so the builder is armed here and drains at the first
        // `publish_iframe_docs`, which `from_dom` runs the moment the `Page` is built.
        #[cfg(feature = "spidermonkey")]
        arm_dynamic_frames(fonts, final_url);
        #[cfg(feature = "spidermonkey")]
        let mut inline_frames = build_inline_frame_docs(&dom, &rects, &styles, final_url, fonts);
        #[cfg(feature = "spidermonkey")]
        if !inline_frames.is_empty() {
            publish_pre_script_frame_docs(&mut inline_frames, fonts);
        }
        // Only stand up a JS context for documents that actually have a script — a static page
        // needs no engine spin-up (faster load, and no persistent global to keep alive). With
        // no initial script, no listener can ever be registered, so there is nothing to lose.
        let js = if dom.find_first("script").is_none() {
            None
        } else {
            manuk_js::set_scroll_geometry(scroll_geometry_of(
                &dom,
                &root_box,
                &styles,
                &std::collections::HashMap::new(),
            ));
            manuk_js::set_snap_candidates(snap_candidates_of(
                &dom,
                &root_box,
                &styles,
                &std::collections::HashMap::new(),
            ));
            // A geometry read during these scripts must see the DOM they have built so far, not
            // the snapshot above — `measure -> mutate -> measure` in one round is how every
            // virtualized list sizes its rows.
            //
            // ⚠⚠⚠ **THIS USED TO BE AN EMPTY MAP, AND THE COMMENT EXPLAINING WHY WAS TRUE UNTIL
            // t1296.** It read: *"nothing has been fetched and `external_css` would be empty
            // whatever we passed"* — correct while `from_dom` was only ever handed a bare tree, and
            // false the moment the caller started seeding the sheets it had already fetched.
            //
            // The consequence is worth stating because it is the whole reason the fix looked inert
            // for a build: the initial cascade DID contain the author's sheet, the probe still read
            // `display: block`, and the reason is that `getComputedStyle` **forces a reflow**, the
            // reflow rebuilds the cascade from the list it is given, and an empty external map
            // silently un-styles the document the call was made to measure. A correct value and a
            // correct fetch, defeated by a third copy of the sheet list.
            let reflow_url = final_url.to_string();
            let reflow_css: HashMap<String, String> = seeded_css.clone();
            // ⚠ Nothing has been scrolled yet — `from_dom` is building the page — so an empty
            // offset map is the TRUTH here, not a placeholder. `has_sticky` is likewise not yet
            // computed (it is derived below, from these same styles); the pre-`Page` scripts run
            // against scroll 0, where the constructor's own `restick(0.0)` has not happened either,
            // so both halves agree that this tree is the unshifted one.
            let no_scroll: std::collections::HashMap<NodeId, (f32, f32)> =
                std::collections::HashMap::new();
            // ⚠ `has_sticky` IS derived here, from these same styles, rather than passed as `false`.
            // It is computed again below for the `Page` — but a blocking script that scrolls a pane
            // and measures a sticky header inside it runs HERE, before the `Page` exists, and a
            // hardcoded `false` made that read answer with the header carried down by the scroll.
            let sticky_here = styles
                .values()
                .any(|s| s.position == manuk_css::Position::Sticky);
            let _reflow = ReflowScope::install(
                &dom,
                fonts,
                viewport_width,
                &inline_images,
                &reflow_url,
                &reflow_css,
                &no_scroll,
                0.0,
                sticky_here,
            );
            // **The inline images decoded above are publishable RIGHT NOW, before the first script.**
            // `Page` does not exist yet at this point — it is constructed below — so the ordinary
            // `publish_image_sources` hook cannot have run, and a BLOCKING script that draws a `data:`
            // image would find nothing. Those bytes came with the document; there is nothing to wait
            // for, which is the same reasoning that decodes them ahead of the first layout.
            for (node, img) in &inline_images {
                manuk_js::publish_image_source(node.0, img.width, img.height, &img.rgba);
            }
            // Taken, not read: the seed belongs to THIS document, and leaving it in place would let
            // a node index from this page fire a `load` on the next one built in this thread.
            let external = PENDING_EXTERNAL_SCRIPTS.with(|p| std::mem::take(&mut *p.borrow_mut()));
            match manuk_js::load_document(&mut dom, final_url, &rects, &styles, external) {
                Ok((ctx, n)) => {
                    if n > 0 {
                        tracing::debug!(scripts = n, "executed page scripts");
                        let sheets2: Vec<Stylesheet> = MinimalCascade::collect_style_elements(&dom);
                        (styles, root_box) = restyle_and_layout(
                            &dom,
                            &sheets2,
                            fonts,
                            viewport_width,
                            &inline_images,
                        );
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
        // **Take ownership of the frames built above.** They go into the SAME two maps
        // `render_iframe_with_type` fills, which is what makes `load_inline_frames`' later pass skip
        // them (`self.iframes.contains_key`) instead of building a second document and orphaning the
        // one a parse-time script is already holding a reference to.
        #[cfg(feature = "spidermonkey")]
        let (frame_pages, frame_urls) = {
            let mut pages = std::collections::HashMap::new();
            let mut urls = std::collections::HashMap::new();
            for f in inline_frames {
                if let Some(img) = f.image {
                    inline_images.insert(f.node, img);
                }
                let provisional = f.provisional;
                pages.insert(f.node, f.page);
                // ⚠⚠⚠ **A PROVISIONAL FRAME GOES INTO `pages` AND NOT INTO `urls`, AND THAT
                // ASYMMETRY IS THE WHOLE SAFETY ARGUMENT.** `urls` becomes `Page::iframes`, which is
                // the "already rendered" marker `pending_iframes` reads — entering a frame whose
                // `src` has not been fetched would make the fetch skip it, i.e. every `<iframe src>`
                // on the web would silently stop loading. Being in `child_pages` but NOT in
                // `iframes` is precisely the definition of "has its initial about:blank, has not
                // navigated", and the `load`-firing loop below reads it that way.
                if !provisional {
                    urls.insert(f.node, f.url);
                }
            }
            (pages, urls)
        };
        #[cfg(not(feature = "spidermonkey"))]
        let (frame_pages, frame_urls) = (
            std::collections::HashMap::new(),
            std::collections::HashMap::new(),
        );
        let mut page = Page {
            pending_submits: std::cell::RefCell::new(Vec::new()),
            final_url: final_url.to_string(),
            title,
            content_height,
            root_box,
            dom,
            styles,
            js,
            has_sticky,
            sticky_applied: std::collections::HashMap::new(),
            sticky_scroll_y: 0.0,
            dyn_scripts_ran: std::collections::HashSet::new(),
            // Empty by default; the async pre-fetch pass on the caller's path sets this before the
            // deferred (module) scripts run (`load_async`, `from_prefetched_inner`).
            module_graph_sources: std::collections::HashMap::new(),
            module_node_bases: std::collections::HashMap::new(),
            import_map: std::collections::HashMap::new(),
            zoom: 1.0,
            // The inline images decoded above already have their natural size in `styles`; carrying
            // them here is what lets them PAINT as well as lay out.
            images: inline_images,
            inline_svg_cache: std::collections::HashMap::new(),
            svg_geometry: svg_geometry::SvgGeometry::new(),
            scroll_offsets: std::collections::HashMap::new(),
            external_css: HashMap::new(),
            failed_css: std::collections::HashSet::new(),
            absent_css: std::collections::HashSet::new(),
            fetched_urls: std::collections::HashSet::new(),
            image_attempts: std::collections::HashSet::new(),
            image_by_url: std::collections::HashMap::new(),
            iframes: frame_urls,
            child_pages: frame_pages,
            last_cascade: None,
            csp,
            load_deadline: None,
        };
        // **Re-publish, then announce.** The maps moved into the `Page` above, and
        // `publish_pre_script_frame_docs` handed JS `&Page::styles` addresses taken from a map that
        // no longer exists — `publish_iframe_docs` is the single call site whose whole contract is
        // "every mutation of `child_pages` republishes", and this is one. `load` fires only now,
        // because it needs a JS context and `from_dom` did not have one when the frames were built.
        // ⚠ `child_pages` is EMPTY when a blocking script created the only frame on the page — the
        // frames it built are still sitting in `DYNAMIC_FRAMES` waiting to be adopted, and adoption
        // happens inside `publish_iframe_docs`. Gating on `child_pages` alone would strand exactly
        // the case this tick exists for.
        #[cfg(feature = "spidermonkey")]
        if !page.child_pages.is_empty() || DYNAMIC_FRAMES.with(|v| !v.borrow().is_empty()) {
            page.publish_iframe_docs(fonts);
            let nodes: Vec<manuk_dom::NodeId> = page.child_pages.keys().copied().collect();
            for n in nodes {
                // ⚠ **NOT for a frame that has only its initial `about:blank`.** Chrome fires no
                // `load` for that document; the event belongs to the one the `src` names, and
                // firing here would announce readiness before a single byte had arrived — which is
                // worse than the silence it replaces, because `<iframe onload>` is how every embed,
                // ad slot and payment frame decides it may start talking. "In `child_pages` but not
                // in `iframes`" IS that state; see the installation loop above.
                if page.iframes.contains_key(&n) {
                    page.fire_frame_load(n);
                }
            }
        }
        // ⚠ **A STICKY BOX CAN BE STUCK AT SCROLL 0.** `top: 0` on a box whose containing block
        // starts above the scrollport, and every `bottom`-stuck footer, are already displaced before
        // anything scrolls — `css/css-position/sticky` opens with exactly that assertion
        // (*"initially the sticky box should be pushed to the top of the container"*). Baking the
        // shift only on the first scroll would leave the DOM reporting the in-flow position for the
        // whole of a page's life if the user never scrolls, which is most sticky footers.
        page.restick(0.0);
        page
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
        self.dispatch_click_detail(node, 1, fonts, viewport_width)
    }

    /// [`Page::dispatch_click`], carrying the **click count** in `detail`.
    ///
    /// Split out for [`Page::dispatch_dblclick`], which needs the second click of a pair to say so.
    /// `detail` is not decoration: `if (e.detail === 2)` on an ordinary `click` listener is the
    /// idiomatic double-click handler, and a click dispatched without it leaves that branch
    /// unreachable while every listener still runs.
    pub(crate) fn dispatch_click_detail(
        &mut self,
        node: manuk_dom::NodeId,
        detail: u32,
        fonts: &FontContext,
        viewport_width: f32,
    ) -> bool {
        // ── THE POINTER SEQUENCE, which a large class of real UI is the only listener for. ──────
        //
        // Dropdown menus, comboboxes, drag handles, sliders and press-and-hold controls listen on
        // **`mousedown`**, not `click` — deliberately, so the menu is up before the button comes
        // back up. Firing only `click` leaves every one of them inert, with nothing thrown to
        // notice. `mousedown` -> `mouseup` -> `click` is the order a real browser produces.
        //
        // **`buttons` differs between the two, and that is not a detail.** It is a mask of the
        // buttons *currently held*: 1 while the primary button is down, and **0 on `mouseup`,
        // because by then it has been released.** Passing the same mask to both is the shape of
        // wrong that reads as right.
        //
        // A `preventDefault()` on `mousedown` does **not** cancel the click — it suppresses focus
        // and text selection. Pages depend on precisely that: a toolbar button that prevents
        // `mousedown` to keep the editor's selection alive still expects its `click`. So the
        // verdicts here are deliberately discarded.
        self.dispatch_pointer_pair(node, detail, fonts, viewport_width);
        self.dispatch_click_inner(node, detail, fonts, viewport_width)
    }

    /// `mousedown` then `mouseup` on `node`. Verdicts discarded — see the caller's note.
    fn dispatch_pointer_pair(
        &mut self,
        node: manuk_dom::NodeId,
        detail: u32,
        fonts: &FontContext,
        viewport_width: f32,
    ) {
        #[cfg(feature = "spidermonkey")]
        {
            let Some(ctx) = &self.js else { return };
            let rects: HashMap<manuk_dom::NodeId, [f32; 4]> = self
                .root_box
                .node_rects(&self.dom)
                .into_iter()
                .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
                .collect();
            let _reflow = ReflowScope::install(
                &self.dom,
                fonts,
                viewport_width,
                &self.images,
                &self.final_url,
                &self.external_css,
                &self.scroll_offsets,
                self.sticky_scroll_y,
                self.has_sticky,
            );
            // (type, buttons-held-during-this-event)
            for (ty, buttons) in [("mousedown", 1u32), ("mouseup", 0u32)] {
                if let Err(e) = manuk_js::dispatch_mouse_buttons(
                    ctx,
                    &mut self.dom,
                    node,
                    ty,
                    detail,
                    0,
                    buttons,
                    &rects,
                    &self.styles,
                ) {
                    tracing::warn!("{ty} dispatch: {e}");
                }
            }
        }
        #[cfg(not(feature = "spidermonkey"))]
        {
            let _ = (node, detail, fonts, viewport_width);
        }
    }

    /// The activation + `click` half, without the pointer sequence. The `<label>` forwarding path
    /// re-enters HERE rather than at [`Page::dispatch_click_detail`]: a real browser fires
    /// `mousedown`/`mouseup` **once, on the element actually under the pointer** (the label), and
    /// forwards only the *click* to the labelled control. Re-entering at the outer function would
    /// press the mouse down a second time, on a node the pointer never touched.
    fn dispatch_click_inner(
        &mut self,
        node: manuk_dom::NodeId,
        detail: u32,
        fonts: &FontContext,
        viewport_width: f32,
    ) -> bool {
        // **Activation does NOT require a JS context.** A static form with no `<script>` still has
        // working checkboxes in every browser; gating the whole path on `self.js` meant a
        // script-free page's controls were inert. Event dispatch is what needs JS — the toggle is
        // not, so the two are separated below.
        let rects: std::collections::HashMap<manuk_dom::NodeId, [f32; 4]> = self
            .root_box
            .node_rects(&self.dom)
            .into_iter()
            .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
            .collect();
        // Covers every dispatch below, including the nested `dispatch_click` a <label> forwards
        // into — a handler that mutates and then measures must see what it just built.
        let _reflow = ReflowScope::install(
            &self.dom,
            fonts,
            viewport_width,
            &self.images,
            &self.final_url,
            &self.external_css,
            &self.scroll_offsets,
            self.sticky_scroll_y,
            self.has_sticky,
        );
        // ── A <label> forwards its click to the control it labels. ─────────────────────────
        // This is how most checkboxes on the web are actually clicked: the visible target is the
        // text, not the 12px box. Without forwarding, clicking "Remember me" does nothing at all.
        // A label whose control is disabled activates nothing — the control is inert, and routing
        // through the label must not be a way around that.
        if let Some(control) = self.labeled_control(node).filter(|c| !self.is_disabled(*c)) {
            // The label's own click still fires and can still be cancelled — a handler on the
            // label that calls preventDefault() stops the control being activated. With no JS
            // there is nothing to cancel it, so it always proceeds.
            let proceed = match self.js.as_ref() {
                None => true,
                Some(ctx) => match manuk_js::dispatch_mouse(
                    ctx,
                    &mut self.dom,
                    node,
                    "click",
                    detail,
                    0,
                    &rects,
                    &self.styles,
                ) {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::warn!("label click dispatch: {e}");
                        return true;
                    }
                },
            };
            if proceed {
                return self.dispatch_click_inner(control, detail, fonts, viewport_width);
            }
            return proceed;
        }

        // ── ACTIVATION BEHAVIOUR, part 1: the PRE-click steps. ──────────────────────────────
        // A click on a checkbox toggles it, and it does so **before** the event is dispatched —
        // which is why a real handler reading `this.checked` sees the NEW state. Firing the event
        // and toggling afterwards would hand every handler on the web the stale value. If the
        // handler then cancels the event, the toggle is undone (the "canceled activation steps").
        let activation = self.pre_click_activation(node);

        let proceed = match self.js.as_ref() {
            None => true,
            Some(ctx) => match manuk_js::dispatch_mouse(
                ctx,
                &mut self.dom,
                node,
                "click",
                detail,
                0,
                &rects,
                &self.styles,
            ) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!("click dispatch: {e}");
                    return true;
                }
            },
        };

        // ── ACTIVATION BEHAVIOUR, part 2: commit or undo. ───────────────────────────────────
        if let Some(prev) = activation {
            if proceed {
                // `input` then `change`, in that order — a framework listening for either must see
                // the committed state, and every controlled-component binding is written for it.
                let Some(ctx) = self.js.as_ref() else {
                    return proceed;
                };
                for kind in ["input", "change"] {
                    if let Err(e) = manuk_js::dispatch_event(
                        ctx,
                        &mut self.dom,
                        node,
                        kind,
                        &rects,
                        &self.styles,
                    ) {
                        tracing::warn!("{kind} dispatch: {e}");
                    }
                }
            } else {
                self.undo_click_activation(node, prev);
            }
        }

        // ── A SUBMIT BUTTON submits its form. ───────────────────────────────────────────────
        // "Click Sign in" is the single most common thing an agent is asked to do, and until now
        // `element.click()` on a submit button fired an event and stopped. Queued as a *requested*
        // submit so the `submit` handler runs first and the page can validate or cancel.
        if proceed && !self.is_disabled(node) {
            if let Some(form) = self.submit_target(node) {
                // Record WHICH button submitted: `<button name="action" value="delete">` next to
                // `value="save"` is how many forms say what the user asked for.
                self.pending_submits.borrow_mut().push((form, Some(node)));
            }
        }
        // ── A <summary> CLICK TOGGLES ITS <details>. ────────────────────────────────────────
        // The disclosure widget is the web's standard "show more", and it is pure UA behaviour:
        // GitHub's folded diffs and review threads, MDN's collapsible sections and every docs
        // site's FAQ carry NO script for it — the browser is the entire implementation. Without
        // this, clicking a summary does nothing at all and the section can never be opened.
        //
        // It is **activation behaviour**, so it runs AFTER the event and only if nothing cancelled
        // it: a handler calling preventDefault() on the summary keeps the section shut, which is
        // how a page implements its own animated disclosure.
        if proceed {
            if let Some(details) = self.summary_details_target(node) {
                let open = self
                    .dom
                    .element(details)
                    .is_some_and(|e| e.attr("open").is_some());
                if open {
                    self.dom.remove_attr(details, "open");
                } else {
                    self.dom.set_attr(details, "open", "");
                    // **EXCLUSIVE ACCORDION (`<details name>`).** A named group holds at most one
                    // open panel: opening this one closes every OTHER open `<details>` that shares
                    // its non-empty `name`. This is the platform FAQ/docs accordion (Baseline 2024)
                    // and, like the summary toggle itself, ships with no page script — without it a
                    // named group expands into a wall of every section at once. Scoped strictly BY
                    // NAME: an empty name is not a group, and a different name is a different group.
                    let group = self
                        .dom
                        .element(details)
                        .and_then(|e| e.attr("name"))
                        .filter(|n| !n.is_empty())
                        .map(str::to_string);
                    if let Some(name) = group {
                        let peers: Vec<manuk_dom::NodeId> = self
                            .dom
                            .descendants(self.dom.root())
                            .filter(|&o| o != details)
                            .filter(|&o| {
                                self.dom.element(o).is_some_and(|e| {
                                    e.name == "details"
                                        && e.attr("name") == Some(name.as_str())
                                        && e.attr("open").is_some()
                                })
                            })
                            .collect();
                        for peer in peers {
                            self.dom.remove_attr(peer, "open");
                            // The spec fires `beforetoggle` then `toggle` on each panel exclusivity
                            // auto-closes, so a lazy-loaded section learns it was collapsed. Both are
                            // stateless here (matching this engine's `toggle`) and `beforetoggle` is
                            // non-cancelable for `<details>` — its use is the fire-before-render hook.
                            if let Some(ctx) = self.js.as_ref() {
                                for ev in ["beforetoggle", "toggle"] {
                                    if let Err(e) = manuk_js::dispatch_event(
                                        ctx,
                                        &mut self.dom,
                                        peer,
                                        ev,
                                        &rects,
                                        &self.styles,
                                    ) {
                                        tracing::warn!("accordion {ev} dispatch: {e}");
                                    }
                                }
                            }
                        }
                    }
                }
                // `beforetoggle` then `toggle` — the two events the spec fires when the disclosure
                // state changes. A page listens for `beforetoggle` to prepare (lazy-load) BEFORE the
                // panel renders (relayout happens after this dispatch) and `toggle` to react after.
                // Dispatched after the attribute changes, so a handler reading `details.open` sees the
                // new state; `beforetoggle` is non-cancelable for `<details>` (unlike popover's).
                if let Some(ctx) = self.js.as_ref() {
                    for ev in ["beforetoggle", "toggle"] {
                        if let Err(e) = manuk_js::dispatch_event(
                            ctx,
                            &mut self.dom,
                            details,
                            ev,
                            &rects,
                            &self.styles,
                        ) {
                            tracing::warn!("{ev} dispatch: {e}");
                        }
                    }
                }
            }
        }

        // If a handler mutated the DOM, re-style + re-lay-out so it renders (at base zoom;
        // the caller re-applies zoom on its next relayout).
        let root = self.dom.root();
        if self.dom.is_dirty(root) || self.dom.has_dirty_descendants(root) {
            let sheets: Vec<Stylesheet> = self.all_sheets();
            (self.styles, self.root_box) =
                restyle_and_layout(&self.dom, &sheets, fonts, viewport_width, &self.images);
            self.reapply_scroll_offsets();
            self.content_height = self.root_box.content_bottom();
            self.dom.clear_all_dirty();
        }
        // A script round can mutate ONLY a child frame, leaving the parent clean — so this sits
        // OUTSIDE the parent dirty guard above. It is a flag check per frame when nothing changed.
        self.repaint_child_frames(fonts);
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
        // A caller that already has text is telling us the body IS text, and for a text body the
        // correct bytes are its UTF-8 encoding — so this stays exactly what it always was.
        self.resolve_fetch_bytes_inner(
            id,
            status,
            body,
            body.as_bytes(),
            headers,
            fonts,
            viewport_width,
        )
    }

    /// Settle a request from the **raw response bytes**, decoding the text channel here.
    ///
    /// This is the entry point a host with real wire bytes should use. `resolve_fetch` above cannot
    /// serve that case: a media segment reaching it as a `&str` has already been through a charset
    /// decode, and re-encoding it to UTF-8 for `arrayBuffer()` inflates every byte above `0x7F` into
    /// two — a 260-byte segment came back as 407, which no demuxer can parse and which surfaces as a
    /// codec bug rather than a transport one.
    pub fn resolve_fetch_bytes(
        &mut self,
        id: u32,
        status: u16,
        bytes: &[u8],
        headers: &[(String, String)],
        fonts: &FontContext,
        viewport_width: f32,
    ) {
        let ct = headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
            .map(|(_, v)| v.as_str());
        let text = manuk_net::charset::decode_html(bytes, ct);
        self.resolve_fetch_bytes_inner(id, status, &text, bytes, headers, fonts, viewport_width)
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_fetch_bytes_inner(
        &mut self,
        id: u32,
        status: u16,
        body: &str,
        raw: &[u8],
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
        if let Err(e) = manuk_js::resolve_fetch_bytes(
            ctx,
            &mut self.dom,
            id,
            status,
            body,
            raw,
            headers,
            &rects,
            &self.styles,
        ) {
            tracing::warn!("fetch resolve: {e}");
            return;
        }
        let root = self.dom.root();
        if self.dom.is_dirty(root) || self.dom.has_dirty_descendants(root) {
            let sheets: Vec<Stylesheet> = self.all_sheets();
            (self.styles, self.root_box) =
                restyle_and_layout(&self.dom, &sheets, fonts, viewport_width, &self.images);
            self.reapply_scroll_offsets();
            self.content_height = self.root_box.content_bottom();
            self.dom.clear_all_dirty();
        }
        // A script round can mutate ONLY a child frame, leaving the parent clean — so this sits
        // OUTSIDE the parent dirty guard above. It is a flag check per frame when nothing changed.
        self.repaint_child_frames(fonts);
    }

    /// The form control a `<label>` labels — `for="id"`, else the first labelable descendant.
    ///
    /// `None` when `node` is not a label, when the label labels nothing, or when the click already
    /// landed **on the control itself** (a control nested inside its own label). That last case is
    /// what stops the forwarding recursing: without it, clicking the checkbox inside
    /// `<label><input>text</label>` would forward to itself forever.
    /// The `<details>` a click should toggle: the nearest `<summary>` at or above `node`, if it is
    /// a child of a `<details>`.
    ///
    /// The walk upward is the load-bearing part. A click lands on whatever is under the cursor —
    /// the text node's element, a `<span>`, an `<svg>` chevron, a `<b>` — essentially never on the
    /// `<summary>` box itself. Matching only an exact hit would make the widget work in a test and
    /// fail on every real page, because real summaries have markup inside them.
    ///
    /// Only a summary that is a **child of** a details toggles it (the spec's requirement), and
    /// only the FIRST such summary is the disclosure's label — a second `<summary>` is ordinary
    /// content and must not toggle anything.
    fn summary_details_target(&self, node: manuk_dom::NodeId) -> Option<manuk_dom::NodeId> {
        let mut cur = Some(node);
        while let Some(n) = cur {
            if self.dom.element(n).is_some_and(|e| e.name == "summary") {
                let parent = self.dom.parent(n)?;
                if !self
                    .dom
                    .element(parent)
                    .is_some_and(|e| e.name == "details")
                {
                    return None;
                }
                let first = self
                    .dom
                    .children(parent)
                    .into_iter()
                    .find(|c| self.dom.element(*c).is_some_and(|e| e.name == "summary"));
                return (first == Some(n)).then_some(parent);
            }
            cur = self.dom.parent(n);
        }
        None
    }

    fn labeled_control(&self, node: manuk_dom::NodeId) -> Option<manuk_dom::NodeId> {
        let el = self.dom.element(node)?;
        if el.name != "label" {
            return None;
        }
        const LABELABLE: [&str; 5] = ["input", "select", "textarea", "button", "meter"];
        if let Some(id) = el.attr("for") {
            let want = id.to_string();
            for n in self.dom.descendants(self.dom.root()) {
                if let Some(e) = self.dom.element(n) {
                    if e.attr("id") == Some(want.as_str()) && LABELABLE.contains(&e.name.as_str()) {
                        return Some(n);
                    }
                }
            }
            // `for` naming nothing labelable labels nothing — it does not fall back to a
            // descendant, because the author said which control they meant.
            return None;
        }
        self.dom.descendants(node).find(|n| {
            self.dom
                .element(*n)
                .is_some_and(|e| LABELABLE.contains(&e.name.as_str()))
        })
    }

    /// The form a click on `node` submits, if `node` is a submit button.
    ///
    /// `<button>`'s default type IS submit — a bare `<button>` inside a form submits it, which is a
    /// classic source of "why did my page reload". `type=button` and `type=reset` do not submit.
    /// Association is the ancestor `<form>`, or an explicit `form="id"` (which lets a button live
    /// outside the form it submits).
    fn submit_target(&self, node: manuk_dom::NodeId) -> Option<manuk_dom::NodeId> {
        let el = self.dom.element(node)?;
        let ty = el.attr("type").unwrap_or("").to_ascii_lowercase();
        let submits = match el.name.as_str() {
            // A bare <button> defaults to type=submit.
            "button" => ty.is_empty() || ty == "submit",
            "input" => ty == "submit" || ty == "image",
            _ => false,
        };
        if !submits {
            return None;
        }
        if let Some(id) = el.attr("form") {
            let want = id.to_string();
            return self.dom.descendants(self.dom.root()).find(|n| {
                self.dom
                    .element(*n)
                    .is_some_and(|e| e.name == "form" && e.attr("id") == Some(want.as_str()))
            });
        }
        let mut cur = self.dom.parent(node);
        while let Some(n) = cur {
            if self.dom.element(n).is_some_and(|e| e.name == "form") {
                return Some(n);
            }
            cur = self.dom.parent(n);
        }
        None
    }

    /// Whether a form control is disabled — by its own `disabled` attribute, or by inheriting it
    /// from an ancestor `<fieldset disabled>`.
    ///
    /// **The inherited case is not an edge case.** Disabling a whole step of a multi-step form by
    /// wrapping it in one `<fieldset disabled>` is the idiomatic way to do it, and checking only
    /// the control's own attribute leaves every control in that fieldset live.
    fn is_disabled(&self, node: manuk_dom::NodeId) -> bool {
        let mut cur = Some(node);
        while let Some(n) = cur {
            if let Some(e) = self.dom.element(n) {
                // Only a `<fieldset>` propagates disabledness; a disabled `<div>` means nothing.
                if e.attr("disabled").is_some() && (n == node || e.name == "fieldset") {
                    return true;
                }
            }
            cur = self.dom.parent(n);
        }
        false
    }

    /// The pre-click activation steps for a form control, returning the state to restore if the
    /// click is cancelled. `None` for anything with no activation behaviour.
    ///
    /// The returned `Vec` is every node whose `checked` was changed, with its prior value — a radio
    /// activation touches the whole group, not just the one clicked.
    fn pre_click_activation(
        &mut self,
        node: manuk_dom::NodeId,
    ) -> Option<Vec<(manuk_dom::NodeId, bool)>> {
        // **A disabled control is inert.** Not "styled grey" — it does not activate, and clicking
        // it must leave the page exactly as it was. Getting this wrong is worse than cosmetic for
        // an agent: it ticks a disabled consent box, reads the state back, sees it ticked, and
        // reports success on a form the server will reject.
        if self.is_disabled(node) {
            return None;
        }
        let el = self.dom.element(node)?;
        if el.name != "input" {
            return None;
        }
        let ty = el.attr("type").unwrap_or("").to_ascii_lowercase();
        let was = el.attr("checked").is_some();
        match ty.as_str() {
            "checkbox" => {
                let mut prev = vec![(node, was)];
                let _ = &mut prev;
                if was {
                    self.dom.remove_attr(node, "checked");
                } else {
                    self.dom.set_attr(node, "checked", "");
                }
                Some(prev)
            }
            "radio" => {
                // **A radio is not a toggle — it is a group.** Clicking one must uncheck its
                // siblings, or the page ends up with two "selected" options and a form that
                // submits the wrong one. Grouping is by `name`, which is how the form serialises.
                let name = self
                    .dom
                    .element(node)?
                    .attr("name")
                    .unwrap_or("")
                    .to_string();
                let mut prev = Vec::new();
                if !name.is_empty() {
                    let all: Vec<manuk_dom::NodeId> =
                        self.dom.descendants(self.dom.root()).collect();
                    for other in all {
                        if other == node || !self.dom.is_element(other) {
                            continue;
                        }
                        let Some(oe) = self.dom.element(other) else {
                            continue;
                        };
                        let is_peer = oe.name == "input"
                            && oe.attr("type").unwrap_or("").eq_ignore_ascii_case("radio")
                            && oe.attr("name").unwrap_or("") == name;
                        if is_peer && oe.attr("checked").is_some() {
                            prev.push((other, true));
                            self.dom.remove_attr(other, "checked");
                        }
                    }
                }
                prev.push((node, was));
                // Clicking a radio always CHECKS it; it never unchecks (unlike a checkbox).
                self.dom.set_attr(node, "checked", "");
                Some(prev)
            }
            _ => None,
        }
    }

    /// Restore what [`pre_click_activation`](Self::pre_click_activation) changed, because the click
    /// was cancelled. `preventDefault()` on a checkbox means the box does not tick — a page that
    /// validates before allowing a toggle depends on exactly this.
    fn undo_click_activation(
        &mut self,
        _node: manuk_dom::NodeId,
        prev: Vec<(manuk_dom::NodeId, bool)>,
    ) {
        for (n, was_checked) in prev {
            if was_checked {
                self.dom.set_attr(n, "checked", "");
            } else {
                self.dom.remove_attr(n, "checked");
            }
        }
    }

    /// An element remembered as the visual fixed point of a scroll, with where it sat relative to
    /// the viewport's top edge. See [`Page::capture_scroll_anchor`].
    ///
    /// **Why this exists.** A feed loads an image, an ad or the next page of posts *above* what the
    /// user is reading, the document gets taller above them, and the line they were on jumps down
    /// the screen. Scroll anchoring is what makes the browser keep that line still: pick the
    /// element at the top of the viewport, remember where it was, and after the relayout move the
    /// scroll offset by however far it moved. The user's content stays put and the growth happens
    /// off-screen, where it belongs.
    pub fn capture_scroll_anchor(&self, scroll_y: f32) -> Option<ScrollAnchor> {
        let rects = self.root_box.node_rects(&self.dom);
        // The anchor is the FIRST element in document order that the viewport's top edge cuts
        // through or that begins just below it — that is the thing the user is looking at. Taking
        // the first box overall would anchor to <body>, which never moves and so never corrects
        // anything; taking the deepest would anchor to a text run that a reflow may destroy.
        let mut best: Option<(NodeId, f32)> = None;
        for node in self.dom.descendants(self.dom.root()) {
            if !self.dom.is_element(node) {
                continue;
            }
            let Some(r) = rects.get(&node) else { continue };
            // Zero-height boxes cannot be a fixed point, and a box that STARTS ABOVE the top edge
            // must not be one either. That second rule is the whole correctness of this: <body> and
            // every ancestor container straddle the viewport top, they begin at y=0, and they do
            // not move when content is inserted inside them — so anchoring to one yields a
            // correction of zero and the page jumps exactly as if there were no anchoring at all.
            // The fixed point has to be the first box that begins at or below the fold.
            if r.height <= 0.0 || r.y < scroll_y {
                continue;
            }
            let offset = r.y - scroll_y;
            match best {
                // Closest to the top edge wins; ties go to document order (first seen).
                Some((_, bo)) if bo <= offset => {}
                _ => best = Some((node, offset)),
            }
        }
        best.map(|(node, offset_from_top)| ScrollAnchor {
            node,
            offset_from_top,
        })
    }

    /// How far the scroll offset must move so `anchor` stays visually where it was.
    ///
    /// Add this to the host's `scroll_y`. `0.0` when the anchor did not move — the overwhelmingly
    /// common case, and it costs one map lookup — or when it no longer exists, because guessing at
    /// a correction for an element that is gone would move the page for no reason.
    pub fn scroll_anchor_delta(&self, anchor: &ScrollAnchor, scroll_y: f32) -> f32 {
        let rects = self.root_box.node_rects(&self.dom);
        let Some(r) = rects.get(&anchor.node) else {
            return 0.0;
        };
        // Where the anchor sits now, versus where it must sit for nothing to appear to move.
        let want = scroll_y + anchor.offset_from_top;
        r.y - want
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
        let _reflow = ReflowScope::install(
            &self.dom,
            fonts,
            viewport_width,
            &self.images,
            &self.final_url,
            &self.external_css,
            &self.scroll_offsets,
            self.sticky_scroll_y,
            self.has_sticky,
        );
        if let Err(e) =
            manuk_js::deliver_ws_event(ctx, &mut self.dom, id, event, &rects, &self.styles)
        {
            tracing::warn!("websocket deliver: {e}");
            return;
        }
        let root = self.dom.root();
        if self.dom.is_dirty(root) || self.dom.has_dirty_descendants(root) {
            let sheets: Vec<Stylesheet> = self.all_sheets();
            (self.styles, self.root_box) =
                restyle_and_layout(&self.dom, &sheets, fonts, viewport_width, &self.images);
            self.reapply_scroll_offsets();
            self.content_height = self.root_box.content_bottom();
            self.dom.clear_all_dirty();
        }
        // A script round can mutate ONLY a child frame, leaving the parent clean — so this sits
        // OUTSIDE the parent dirty guard above. It is a flag check per frame when nothing changed.
        self.repaint_child_frames(fonts);
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
        let _reflow = ReflowScope::install(
            &self.dom,
            fonts,
            viewport_width,
            &self.images,
            &self.final_url,
            &self.external_css,
            &self.scroll_offsets,
            self.sticky_scroll_y,
            self.has_sticky,
        );
        if let Err(e) =
            manuk_js::deliver_fetch_stream(ctx, &mut self.dom, id, event, &rects, &self.styles)
        {
            tracing::warn!("fetch stream deliver: {e}");
            return;
        }
        let root = self.dom.root();
        if self.dom.is_dirty(root) || self.dom.has_dirty_descendants(root) {
            let sheets: Vec<Stylesheet> = self.all_sheets();
            (self.styles, self.root_box) =
                restyle_and_layout(&self.dom, &sheets, fonts, viewport_width, &self.images);
            self.reapply_scroll_offsets();
            self.content_height = self.root_box.content_bottom();
            self.dom.clear_all_dirty();
        }
        // A script round can mutate ONLY a child frame, leaving the parent clean — so this sits
        // OUTSIDE the parent dirty guard above. It is a flag check per frame when nothing changed.
        self.repaint_child_frames(fonts);
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
    ///
    /// A form submitted by **clicking its submit button** joins the `requested` list, not `direct`:
    /// `requested` fires the `submit` event first, and a click-to-submit is exactly the case a page
    /// validates in that handler. Putting it in `direct` would skip every client-side validator on
    /// the web.
    pub fn take_form_submits(
        &self,
    ) -> (
        Vec<manuk_dom::NodeId>,
        Vec<(manuk_dom::NodeId, Option<manuk_dom::NodeId>)>,
    ) {
        let from_click = std::mem::take(&mut *self.pending_submits.borrow_mut());
        match &self.js {
            Some(ctx) => {
                let (d, mut r) = manuk_js::take_form_submits(ctx);
                // A script's `requestSubmit()` has no submitter (unless it passes one, which we do
                // not model yet) — `None` is the honest answer, not a guessed button.
                let mut requested: Vec<(manuk_dom::NodeId, Option<manuk_dom::NodeId>)> = r
                    .drain(..)
                    .map(|x| (manuk_dom::NodeId(x as u64), None))
                    .collect();
                requested.extend(from_click);
                (
                    d.into_iter().map(|x| manuk_dom::NodeId(x as u64)).collect(),
                    requested,
                )
            }
            None => (Vec::new(), from_click),
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
            let sheets: Vec<Stylesheet> = self.all_sheets();
            (self.styles, self.root_box) =
                restyle_and_layout(&self.dom, &sheets, fonts, viewport_width, &self.images);
            self.reapply_scroll_offsets();
            self.content_height = self.root_box.content_bottom();
            self.dom.clear_all_dirty();
        }
        // A script round can mutate ONLY a child frame, leaving the parent clean — so this sits
        // OUTSIDE the parent dirty guard above. It is a flag check per frame when nothing changed.
        self.repaint_child_frames(fonts);
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
        let _reflow = ReflowScope::install(
            &self.dom,
            fonts,
            viewport_width,
            &self.images,
            &self.final_url,
            &self.external_css,
            &self.scroll_offsets,
            self.sticky_scroll_y,
            self.has_sticky,
        );
        if let Err(e) =
            manuk_js::fire_popstate(ctx, &mut self.dom, state_json, url, &rects, &self.styles)
        {
            tracing::warn!("popstate: {e}");
            return;
        }
        let root = self.dom.root();
        if self.dom.is_dirty(root) || self.dom.has_dirty_descendants(root) {
            let sheets: Vec<Stylesheet> = self.all_sheets();
            (self.styles, self.root_box) =
                restyle_and_layout(&self.dom, &sheets, fonts, viewport_width, &self.images);
            self.reapply_scroll_offsets();
            self.content_height = self.root_box.content_bottom();
            self.dom.clear_all_dirty();
        }
        // A script round can mutate ONLY a child frame, leaving the parent clean — so this sits
        // OUTSIDE the parent dirty guard above. It is a flag check per frame when nothing changed.
        self.repaint_child_frames(fonts);
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
    /// Re-lay-out and re-paint every child frame whose document has changed, refreshing the bitmap
    /// the parent shows for it.
    ///
    /// **Why frames need this and ordinary elements do not.** A frame is composited as a *bitmap*
    /// into the parent's image map — the same map an `<img>` lands in — because it is a whole
    /// separate document with its own viewport and its own cascade. `render_iframe` painted that
    /// bitmap once. The child `Page` stayed alive (which is what made `contentDocument` work), so
    /// scripts could reach in and mutate it, and nothing ever repainted: the DOM changed and the
    /// screen did not.
    ///
    /// That is the worst shape of bug, because **every read comes back correct** — the parent can
    /// query the frame's DOM and see the new state while showing the old pixels. It lands on exactly
    /// the content the web puts in frames precisely because it is interactive: a **3-D Secure
    /// challenge**, an embedded **OAuth consent screen**, a payment form, a CAPTCHA. Each shows its
    /// first state forever, so the payment or the login cannot be completed and the frame reads to a
    /// user as frozen.
    ///
    /// Guarded on the child's dirty bits, so an untouched frame costs one flag check rather than a
    /// full paint on every script round.
    pub fn repaint_child_frames(&mut self, fonts: &FontContext) {
        if self.child_pages.is_empty() {
            return;
        }
        let rects = self.root_box.node_rects(&self.dom);
        let nodes: Vec<manuk_dom::NodeId> = self.child_pages.keys().copied().collect();
        for node in nodes {
            // A frame with no box is not painted at all — same rule as the first paint.
            let Some(r) = rects.get(&node) else { continue };
            if r.width < 1.0 || r.height < 1.0 {
                continue;
            }
            // ⚠ CONTENT box — the fifth and last frame-sizing site, through the same rule. A
            // repaint that laid the child out at the BORDER width would disagree with the pass that
            // built it, and the frame would reflow by 4px the first time anything touched it.
            let (cw, ch) = frame_content_box(self.styles.get(&node), r.width, r.height);
            let (w, h) = (cw.round().max(1.0) as u32, ch.round().max(1.0) as u32);
            self.repaint_frame(node, w, h, fonts, false);
        }
    }

    /// Re-paint one frame's bitmap. `force` skips the dirty check.
    ///
    /// **`force` is not an optimisation escape hatch — it is a correctness requirement.** When a
    /// click is routed INTO a frame, the child runs its own script round, which re-cascades,
    /// re-lays-out and then clears its own dirty bits. By the time the parent looks, the child is
    /// already clean, so a dirty-guarded repaint skips exactly the frame that just changed. The
    /// click itself is the signal; there is nothing left to detect.
    fn repaint_frame(
        &mut self,
        node: manuk_dom::NodeId,
        w: u32,
        h: u32,
        fonts: &FontContext,
        force: bool,
    ) {
        let Some(child) = self.child_pages.get_mut(&node) else {
            return;
        };
        if !force {
            let croot = child.dom.root();
            if !(child.dom.is_dirty(croot) || child.dom.has_dirty_descendants(croot)) {
                return;
            }
        }
        // The child lays out at the FRAME's viewport, not the window's — that is what makes a
        // responsive embed responsive, and it must stay true on every repaint. BOTH axes: a
        // `100vh` hero inside an embed is as common as a `100vw` one.
        let _vp = ViewportScope::set(w as f32, h as f32);
        child.relayout(fonts, w as f32);
        child.dom.clear_all_dirty();
        let canvas = child.paint(fonts, w, h);
        let img = manuk_paint::DecodedImage {
            width: canvas.width(),
            height: canvas.height(),
            rgba: canvas.rgba_bytes().to_vec(),
        };
        self.images.insert(node, std::rc::Rc::new(img));
    }

    pub fn relayout(&mut self, fonts: &FontContext, viewport_width: f32) {
        self.relayout_zoomed(fonts, viewport_width, self.zoom);
    }

    /// **Install a freshly computed fragment tree**, and run the passes that can only see a
    /// FINISHED one.
    ///
    /// There is exactly one of these because there were three `self.root_box = layout_document(…)`
    /// sites and every one of them feeds `node_rects`. A post-layout pass wired into two of the
    /// three is a pass that silently does not run on the third — the shape of bug this codebase has
    /// paid for repeatedly ("one rule, N implementations"). Adding the next such pass here makes it
    /// run everywhere by construction.
    fn set_root_box(&mut self, root: LayoutBox) {
        self.root_box = root;
        // The inside of an `<svg>` is not laid out by CSS; its boxes come from path data and the
        // `viewBox` transform, and both need the svg's USED box, which exists only now.
        svg_geometry::apply(&mut self.root_box, &self.svg_geometry);
        // **The USED grid track sizes, which exist ONLY as a by-product of the layout that just
        // finished** — `grid-template-columns`/`-rows` resolve to the used value (CSSOM §5.1), and
        // taffy hands them over through a trait method whose default body is a no-op (t1270). This is
        // exactly the "adding the next post-layout pass here makes it run everywhere" case this
        // function's own doc comment was written for: wired into two of the three layout sites, a
        // grid would report last layout's tracks on whichever path was missed.
        publish_grid_tracks();
    }

    /// **Re-run the cascade over EVERY stylesheet the document has** — inline `<style>` and
    /// `<link>`ed alike — without requiring the tree to have grown.
    ///
    /// This exists because the two relayout paths each answer a different question and neither
    /// answers *"a cascade INPUT changed while the tree stayed the same"*, which is what a `:hover`
    /// transition is. `relayout` recascades only when the node count outgrew the style map;
    /// `relayout_incremental` recascades on dirty bits but rebuilds its sheet list from inline
    /// `<style>` elements only, so it would quietly drop every external stylesheet.
    ///
    /// Extracted rather than inlined because `:active` and `:focus` are the same shape and are the
    /// obvious next fills — they should not each rediscover this.
    fn recascade_all_sources(&mut self, viewport_width: f32) {
        // The THIRD hand-rolled copy of [`Page::all_sheets`]'s body when it was written — this one
        // correct, like the tree-grew site, and both of them being correct in private is how the
        // other eight stayed wrong. There is one implementation now.
        let sheets: Vec<Stylesheet> = self.all_sheets();
        self.styles = cascade_styles(&self.dom, &sheets, viewport_width);
        // The intrinsic size of a decoded image is not in any stylesheet, so a rebuilt style
        // map has lost it. Restate it here, before the layout that consumes it.
        apply_natural_sizes(&self.dom, &mut self.styles, &self.images);
        // `@container` conditions answer from the previous pass's geometry — same one-frame
        // model as `relayout_incremental`; the caller's next layout uses the sized styles.
        container_query_recascade(
            &self.dom,
            &sheets,
            viewport_width,
            &mut self.styles,
            &self.root_box,
            &self.images,
        );
        self.last_cascade = None; // the fingerprint no longer describes this tree
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
            // With the EXTERNAL sheets, not just the inline ones — see [`Page::all_sheets`], which is
            // this rule's ONE implementation. This site had its own copy of it, and being the only
            // site that was right is how the other eight stayed wrong.
            let sheets: Vec<Stylesheet> = self.all_sheets();
            self.styles = cascade_styles(&self.dom, &sheets, viewport_width);
            // The intrinsic size of a decoded image is not in any stylesheet, so a rebuilt style
            // map has lost it. Restate it here, before the layout that consumes it.
            apply_natural_sizes(&self.dom, &mut self.styles, &self.images);
            container_query_recascade(
                &self.dom,
                &sheets,
                viewport_width,
                &mut self.styles,
                &self.root_box,
                &self.images,
            );
            self.last_cascade = None; // the fingerprint no longer describes this tree
        }
        let scaled;
        let styles = if (self.zoom - 1.0).abs() < 1e-6 {
            &self.styles
        } else {
            scaled = manuk_css::zoom_styles(&self.styles, self.zoom);
            &scaled
        };
        let root = layout_document(&self.dom, styles, fonts, viewport_width);
        self.set_root_box(root);
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

        let sheets: Vec<Stylesheet> = self.all_sheets();
        let mut new_styles = cascade_styles(&self.dom, &sheets, viewport_width);
        // The intrinsic size of a decoded image is not in any stylesheet, so a rebuilt style
        // map has lost it. Restate it here, before the layout that consumes it.
        apply_natural_sizes(&self.dom, &mut new_styles, &self.images);
        // `@container` conditions answer from the PREVIOUS pass's geometry (`self.root_box`) —
        // the spec's own one-frame model; the damage classification below then decides whether
        // the sized styles warrant a relayout exactly as for any other style delta.
        container_query_recascade(
            &self.dom,
            &sheets,
            viewport_width,
            &mut new_styles,
            &self.root_box,
            &self.images,
        );

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
            let root = layout_document(&self.dom, &self.styles, fonts, viewport_width);
            self.set_root_box(root);
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
                        let c = s.border_color;
                        border.colors = [c.top, c.right, c.bottom, c.left];
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
    /// The live document behind an `<iframe>`, if it has loaded. Lets a gate assert what the
    /// FRAME's own document believes, which is a different question from what the parent shows.
    pub fn child_page(&self, node: manuk_dom::NodeId) -> Option<&Page> {
        self.child_pages.get(&node)
    }

    /// Click at a **document point**, routing INTO a frame when the point lands inside one.
    ///
    /// Returns `true` if the engine should still perform the hit element's default action (no
    /// handler called `preventDefault`), matching [`Self::dispatch_click`].
    ///
    /// **Why coordinates and not a node.** The host hit-tests the parent and gets the `<iframe>`
    /// ELEMENT — clicking that dispatches a click on the frame box itself, which is not what the
    /// user did. The frame is a separate document painted into a bitmap, so the click has to be
    /// translated into the child's own coordinate space and hit-tested there. Without this, tick
    /// 232 left the frame able to change only when a SCRIPT reached into it: a 3-D Secure challenge
    /// or an embedded OAuth consent screen would re-render correctly and still could not be
    /// operated, because the user's press never reached the bank's button.
    ///
    /// Nested frames recurse, so a frame inside a frame is clickable at the depth the box tree says.
    pub fn dispatch_click_at(
        &mut self,
        doc_x: f32,
        doc_y: f32,
        fonts: &FontContext,
        viewport_width: f32,
    ) -> bool {
        let Some(hit) = self.a11y_tree().hit_test(doc_x, doc_y).map(|n| n.node) else {
            return true;
        };
        // Does the hit land on a frame we hold a document for?
        if self.child_pages.contains_key(&hit) {
            let rect = self.root_box.node_rects(&self.dom).get(&hit).copied();
            if let Some(r) = rect {
                let (lx, ly) = (doc_x - r.x, doc_y - r.y);
                let w = r.width.max(1.0);
                if let Some(child) = self.child_pages.get_mut(&hit) {
                    // The child lays out at the FRAME's width, so it must be clicked at it too.
                    let proceed = child.dispatch_click_at(lx, ly, fonts, w);
                    // FORCED: the child's own script round already cleared its dirty bits, so a
                    // dirty-guarded repaint would skip the one frame that just changed.
                    let (pw, ph) = (w.round().max(1.0) as u32, r.height.round().max(1.0) as u32);
                    self.repaint_frame(hit, pw, ph, fonts, true);
                    return proceed;
                }
            }
        }
        self.dispatch_click(hit, fonts, viewport_width)
    }

    /// **Move the pointer to a document point** — updates the `:hover` chain, restyles, and fires
    /// the mouse events a page listens for. Returns `true` if the hover target changed.
    ///
    /// Until this existed `:hover` was hard-coded `false` everywhere in the cascade, which is the
    /// correct answer for a static render and the wrong one for a browser. What that cost is not
    /// "buttons don't light up": **the hover-reveal navigation menu is a desktop-web primitive**,
    /// and `nav li:hover > ul { display: block }` is how a large share of sites build their top
    /// navigation with no JavaScript at all. With `:hover` never matching, every one of those menus
    /// is permanently closed — the links inside them are unreachable to a user and invisible to an
    /// agent, and nothing anywhere reports a problem, because the page is rendering exactly what it
    /// was told to render.
    ///
    /// **The out-then-in ordering is the spec's and it is load-bearing.** `mouseout`/`mouseleave`
    /// fire on what the pointer left *before* `mouseover`/`mouseenter` fire on what it entered.
    /// Menu code is written against that order — the leave handler starts a close timer that the
    /// enter handler cancels — so firing enter first makes a menu close behind a pointer that is
    /// still inside it.
    pub fn dispatch_hover_at(
        &mut self,
        doc_x: f32,
        doc_y: f32,
        fonts: &FontContext,
        viewport_width: f32,
    ) -> bool {
        let hit = self.a11y_tree().hit_test(doc_x, doc_y).map(|n| n.node);
        let previous = self.dom.hovered();
        // `set_hovered` is the one that decides whether anything changed, and it marks the tree
        // dirty when it did. Pointer-move events arrive at motion rates and almost all of them land
        // on the element already hovered; recascading the document for those would be a per-frame
        // cost for zero visual change.
        if !self.dom.set_hovered(hit) {
            return false;
        }

        // The cascade is what `:hover` feeds, so the restyle must happen BEFORE the page's handlers
        // run: a handler that measures on `mouseover` (menu code positioning a submenu is exactly
        // this) must see the geometry the hover itself produced, not the previous frame's.
        //
        // **NEITHER EXISTING RELAYOUT IS CORRECT HERE, and both fail silently — in opposite
        // directions.** This cost the tick's second half, and both traps are worth naming.
        //
        // `relayout` only re-runs the cascade when the tree GREW (a node count against
        // `styles.len()`), because its job is catching nodes a script injected after the last
        // cascade. A hover change adds no nodes, so it re-lays-out the OLD styles: `:hover`
        // matches, `Dom::hovered` is set, every piece of the wiring is correct, and **not one pixel
        // moves.** That is the tick-243 half-fix shape exactly — a fix that compiles, reads as
        // complete, and does nothing.
        //
        // `relayout_incremental` does recascade on the dirty bits `set_hovered` just marked, and it
        // is what I reached for next — but it rebuilds its sheet list from
        // `MinimalCascade::collect_style_elements`, which sees inline `<style>` blocks and **not
        // `<link>`ed stylesheets**. It has no production callers today (tests only), so nothing had
        // ever paid for that. Shipping it on the hover path would mean: hover any link on any site
        // with external CSS, and every external stylesheet silently drops out of the cascade.
        // A gate whose fixture used inline `<style>` — as mine did — passes that with no complaint.
        //
        // So the hover path recascades with the FULL source set, the way `relayout_zoomed`'s
        // tree-grew branch does, and then lays out.
        self.recascade_all_sources(viewport_width);
        self.relayout(fonts, viewport_width);

        let Some(ctx) = self.js.as_ref() else {
            return true;
        };
        let rects: std::collections::HashMap<manuk_dom::NodeId, [f32; 4]> = self
            .root_box
            .node_rects(&self.dom)
            .into_iter()
            .map(|(n, r)| (n, [r.x, r.y, r.width, r.height]))
            .collect();
        let _reflow = ReflowScope::install(
            &self.dom,
            fonts,
            viewport_width,
            &self.images,
            &self.final_url,
            &self.external_css,
            &self.scroll_offsets,
            self.sticky_scroll_y,
            self.has_sticky,
        );

        // Leaving, then entering — see the note above on why the order is not cosmetic.
        if let Some(old) = previous.filter(|n| self.dom.is_alive(*n)) {
            for kind in ["mouseout", "mouseleave"] {
                if let Err(e) =
                    manuk_js::dispatch_event(ctx, &mut self.dom, old, kind, &rects, &self.styles)
                {
                    tracing::warn!("{kind} dispatch: {e}");
                }
            }
        }
        if let Some(new) = hit {
            for kind in ["mouseover", "mousemove", "mouseenter"] {
                if let Err(e) =
                    manuk_js::dispatch_event(ctx, &mut self.dom, new, kind, &rects, &self.styles)
                {
                    tracing::warn!("{kind} dispatch: {e}");
                }
            }
        }
        true
    }

    /// **Route the shell's focus into the cascade** — `:focus`, `:focus-within`, `:focus-visible`.
    /// Returns `true` if anything changed.
    ///
    /// This was a **dead-end wire**, the same shape as the parser's quirks verdict at tick 242. The
    /// shell has tracked focus for many ticks and publishes it into the JS world through
    /// [`publish_view_state`](Self::publish_view_state) — that is what backs
    /// `document.activeElement` — but it never reached the style system, so `:focus` answered
    /// `false` for the life of every page. The engine had the answer and threw it away, which no
    /// capability probe surfaces: the feature *appears* present at every layer you inspect.
    ///
    /// **What it costs is accessibility, not decoration.** The focus ring is the only thing telling
    /// a keyboard user where they are on the page — and because authors have spent twenty years
    /// writing `:focus { outline: none }` to remove the ring mouse users did not want, on a great
    /// many sites the *only* remaining cue is an author's own `:focus`/`:focus-visible` rule. With
    /// the pseudo-class never matching, tabbing through those pages moves an invisible cursor.
    /// `:focus-within` is separately load-bearing: the expanding search box and the open combobox
    /// panel are both written as `.box:focus-within { … }`.
    ///
    /// `from_keyboard` decides `:focus-visible`. Only the caller knows how focus arrived, and it is
    /// the entire distinction the pseudo-class was added to draw — see [`Dom::set_focused`].
    pub fn set_focus(
        &mut self,
        node: Option<manuk_dom::NodeId>,
        from_keyboard: bool,
        fonts: &FontContext,
        viewport_width: f32,
    ) -> bool {
        // A control that cannot be interacted with cannot receive focus either — Tab skips it and
        // `el.focus()` is a no-op — so no `:focus` styling ever lands on it. Two sources, both refused
        // here (before touching DOM state; moving focus AWAY, i.e. `None`, is always allowed):
        //   * `inert` (and its subtree) — the page behind an open `<dialog>.showModal()` stays
        //     untabbable, so keyboard focus cannot escape the modal into neutralised UI.
        //   * a `disabled` form control (its own attribute, or inherited from `<fieldset disabled>`) —
        //     a greyed-out button is not a tab stop.
        if let Some(n) = node {
            if self.is_inert(n) || self.is_disabled(n) {
                return false;
            }
        }
        if !self.dom.set_focused(node, from_keyboard) {
            return false;
        }
        // Same pair as the hover path, and for the same reason — see `dispatch_hover_at`, where the
        // two relayout traps are written out in full. `relayout` alone recascades only a GROWN tree
        // and would move nothing; `relayout_incremental` would drop every external stylesheet.
        self.recascade_all_sources(viewport_width);
        self.relayout(fonts, viewport_width);
        true
    }

    /// **Route the shell's pointer press into the cascade** — `:active`. `Some(node)` on
    /// `mousedown`, `None` on `mouseup`. Returns `true` if anything changed.
    ///
    /// `:active` was the last unfed dynamic pseudo-class: the Stylo matcher answered a hard `false`,
    /// so every button/link/nav press-feedback rule (`button:active { … }`) was dead — the same
    /// dead-end-wire shape `:focus` had before tick 246. The state lives on `Dom` (reached by the
    /// cascade with no signature change), the shell writes it on pointer down/up, and this recascades
    /// so the pressed styling appears BEFORE any handler that measures on `mousedown` runs.
    ///
    /// Recascade with the FULL source set + relayout, the exact pair the hover and focus paths use —
    /// `relayout` alone recascades only a grown tree (a press adds no nodes, so nothing would move),
    /// and `relayout_incremental` drops external stylesheets. See [`Page::dispatch_hover_at`].
    pub fn set_active(
        &mut self,
        node: Option<manuk_dom::NodeId>,
        fonts: &FontContext,
        viewport_width: f32,
    ) -> bool {
        if !self.dom.set_active(node) {
            return false;
        }
        self.recascade_all_sources(viewport_width);
        self.relayout(fonts, viewport_width);
        true
    }

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
        // ⚠⚠⚠ **I3 (t1097/t1098): generated content reaches the semantic model ONLY here.** A
        // `::before` is not in the DOM by construction, so the AX tree — built from the DOM — could
        // not see it by any path, and every pseudo was missing from `accessible_name`. Every other
        // renderer subsystem gets its semantic exposure free from the shared `node_rects` producer,
        // but that producer carries GEOMETRY: a fix that adds TEXT has to be threaded deliberately.
        manuk_a11y::build_tree_generated(
            &self.dom,
            &rects,
            &self.z_index_map(),
            &self.invisible_nodes(),
            &self.non_hittable_nodes(),
            &manuk_layout::generated_text(&self.dom, &self.styles),
        )
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
        manuk_a11y::build_tree_generated_with_focus(
            &self.dom,
            &rects,
            &self.z_index_map(),
            focused,
            &self.invisible_nodes(),
            &self.non_hittable_nodes(),
            &manuk_layout::generated_text(&self.dom, &self.styles),
        )
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
        let mut sources = collect_style_sources(&self.dom, &self.final_url);
        // ⚠⚠⚠ **AN `@import`ED SHEET WAS DELIVERING ITS `@font-face`s AND NOT ONE OF ITS RULES.**
        //
        // t564 fetches imports and its own comment says *"the imported sheets must reach the CASCADE
        // too"* — but it pushed them into `fetch_and_apply_stylesheets`'s LOCAL `sources` vec, and
        // this function re-derives its sources FROM THE DOM. An imported sheet has no `<link>` node,
        // so the cascade never saw it. Only the `@font-face` scan, which reads that local vec, did —
        // which is exactly why the defect survived: a page whose import carries fonts LOOKED fixed.
        //
        // Measured on a local fixture whose imported sheet carries both halves: the `@font-face`
        // arrived (Ahem measured its exact 100px) while `#r { width: 137px }` from the same file
        // came out at the layout's own 8px.
        //
        // **PREPENDED, not appended.** An `@import` must precede every rule in its own sheet, so its
        // rules lose ties to the importing sheet — front-insertion keeps an imported base/reset from
        // overriding the page's own CSS, which is the direction that cannot break a working page.
        // The precise position (immediately before the IMPORTING sheet) needs provenance `external`
        // does not carry, and is named here as residue rather than approximated silently.
        // ⚠ SORTED: `external` is a `HashMap`, so its key order varies run to run. Two imported
        // sheets that set the same property would then cascade in a DIFFERENT ORDER on different
        // runs — a nondeterministic render, which is worse than a wrong one because no gate can
        // catch it reliably.
        let mut imported_urls: Vec<&String> = external.keys().collect();
        imported_urls.sort();
        let imported: Vec<StyleSource> = imported_urls
            .into_iter()
            .filter(|url| {
                !sources
                    .iter()
                    .any(|s| matches!(s, StyleSource::External(u, _) if u == *url))
            })
            .map(|url| StyleSource::External(url.clone(), None))
            .collect();
        if !imported.is_empty() {
            let mut all = imported;
            all.append(&mut sources);
            sources = all;
        }

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
        let mut new_styles = cascade_styles(&self.dom, &sheets, viewport_width);
        // The intrinsic size of a decoded image is not in any stylesheet, so a rebuilt style
        // map has lost it. Restate it here, before the layout that consumes it.
        apply_natural_sizes(&self.dom, &mut new_styles, &self.images);
        // External sheets are the main `@container` carrier. Conditions answer from the
        // pre-external geometry here (the only layout that exists yet) — the previous-pass
        // model, one cascade generation behind until the next restyle re-evaluates them.
        container_query_recascade(
            &self.dom,
            &sheets,
            viewport_width,
            &mut new_styles,
            &self.root_box,
            &self.images,
        );
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
        let root = layout_document(&self.dom, &self.styles, fonts, viewport_width);
        self.set_root_box(root);
        self.content_height = self.root_box.content_bottom();
        self.dom.clear_all_dirty();
        damage
    }

    /// How many render-blocking external stylesheets were requested and never arrived. Non-zero
    /// means the current layout is (partly) UA-default fallback, NOT this engine's rendering of the
    /// author's page — a measurement that diffs it against a fully-styled reference is charging
    /// network weather to the engine's account. The differential oracle discards such runs.
    ///
    /// ⚠ **A `404`/`410` IS NOT COUNTED HERE (t860).** The question this number answers is "am I
    /// less-styled than the reference browser?", and a sheet the origin does not have is a sheet the
    /// reference browser does not get either — a dead `<link>` in the author's own HTML, rendered
    /// identically by both engines and therefore perfectly scorable. Every other failure (a refused
    /// connection, a `403`, a `5xx`, a deadline cut) still counts. See the `absent` arm of
    /// [`Self::fetch_and_apply_stylesheets`] for the measurement that forced the distinction.
    pub fn failed_stylesheet_fetches(&self) -> usize {
        self.failed_css.len()
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
        let mut sources = collect_style_sources(&self.dom, &self.final_url);
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
            // A 404/410 is as settled an answer as a body — asking again cannot change it, and
            // asking again LATE is how the exemption below gets undone (see `absent_css`).
            .filter(|url| !self.absent_css.contains(url))
            .collect();
        // Against the load deadline (tick 655). Tick 654 fixed this phase's ORDER — apply the sheets
        // where they are complete, before the phase goes back to the network for `@import`/`@font-face`
        // — but left the fan-out itself all-or-nothing: one slow CSS host and `join_all` still yields
        // nothing, so the sheets that DID arrive are discarded before they can be applied at all. Same
        // rule, one implementation lower down.
        // ⚠⚠ **A SHEET THAT NEVER SETTLES MUST STILL BE COUNTED AND SAID OUT LOUD.**
        //
        // `collect_before_deadline` returns the futures that FINISHED; the ones the deadline cut off
        // simply are not in the result. So the `for (url, text) in fetched` loop below — which owns
        // BOTH the logging and the `failed_css` bookkeeping — never sees them, and a page whose CSS
        // was cancelled renders in UA-default fallback **in total silence**: no `STYLESHEET FAILED`
        // line, no `failed_css` entry, and `failed_stylesheet_fetches()` answering ZERO.
        //
        // Measured on `serverfault.com` (t707): the page comes out as the UA stylesheet's idea of the
        // document — `html [8 8 1184x7328]`, `body display=Block` where Chrome has a 720px
        // `display=Flex` app shell, the 8px being `body{margin:8px}` — and the ENTIRE stderr for that
        // load is one line. The fidelity sweep books it `render-failed`, *"the only reason on this
        // list that is our own bug"*, which is true and useless: we painted faithfully, we painted
        // the wrong document, and nothing in the engine said which. 21 of 265 corpus sites land here.
        //
        // `failed_stylesheet_fetches()` exists precisely so a measurement can refuse to score such a
        // page, and its doc comment says so. It had **zero callers and a structural undercount**: the
        // cancelled case — the common one on a slow CSS host — was the one it could not see.
        let requested: Vec<String> = ext_urls.clone();
        let fetched = collect_before_deadline(
            ext_urls.into_iter().map(|url| async move {
                // ⚠⚠⚠ **THE STATUS TRAVELS WITH THE MISS, BECAUSE "NO TEXT" HAS TWO CAUSES AND ONLY
                // ONE OF THEM IS OURS.** `subresource_text` already reads `status >= 400` and then
                // throws the status away, so a sheet the origin does not HAVE arrived at the `None`
                // arm below indistinguishable from a sheet that died on the wire — and the `None`
                // arm books both into `failed_css`, which is the number every measurement uses to
                // decide whether it styled the page at all. See the `absent` arm below.
                let r = manuk_net::fetch(&url).await.ok();
                let absent = matches!(r.as_ref().map(|r| r.status), Some(404 | 410));
                let text = r.as_ref().and_then(subresource_text);
                (url, text, absent)
            }),
            self.load_deadline,
        )
        .await;
        {
            let settled: std::collections::HashSet<&str> =
                fetched.iter().map(|(u, ..)| u.as_str()).collect();
            let cut: Vec<&String> = requested
                .iter()
                .filter(|u| !settled.contains(u.as_str()))
                .collect();
            if !cut.is_empty() {
                // ONE line naming the count and the URLs, not one per sheet: the failure is the
                // DEADLINE, which is a single event, and N lines would read as N independent faults.
                tracing::warn!(
                    cut = cut.len(),
                    requested = requested.len(),
                    urls = %cut.iter().map(|u| u.as_str()).collect::<Vec<_>>().join(" "),
                    "RENDER-BLOCKING STYLESHEETS NEVER SETTLED before the load deadline — this page \
                     is rendering in UA-DEFAULT FALLBACK, not in the author's design"
                );
                for u in cut {
                    self.failed_css.insert(u.clone());
                }
            }
        }
        // Start from what we already have — a re-entry must ADD sheets, never rebuild the set from
        // scratch (which would drop the ones it just decided not to re-fetch).
        let mut external: HashMap<String, String> = self.external_css.clone();
        for (url, text, absent) in fetched {
            match text {
                Some(t) => {
                    tracing::info!(bytes = t.len(), %url, "stylesheet applied");
                    self.failed_css.remove(&url);
                    external.insert(url, t);
                }
                // ⚠⚠⚠ **A SHEET THE ORIGIN DOES NOT HAVE IS NOT A PAGE WE FAILED TO STYLE — AND
                // BOOKING IT AS ONE COST THREE IN-SCOPE SITES THEIR SCORABILITY.**
                //
                // `failed_css` answers exactly one question: *"is this layout (partly) UA-default
                // fallback rather than our rendering of the author's design?"* A 404 answers it
                // **no**. The author's `<link>` is a dead link in their own HTML; Chrome requests
                // the same URL, gets the same 404, and renders the same page without it. We are not
                // less-styled than the reference — we are identically-styled, which is precisely
                // the state a differential measurement is allowed to score.
                //
                // Measured (t860, `curl` against the three `css-starved` rows of the t857 sweep —
                // ALL THREE, not a sample):
                //
                // ```text
                //   cuneocronaca.it   css/normalize.css        404   (4 of its 6 sheets are 200)
                //                     css/simple_slider.css    404
                //   m.youm7.com       landing/landingstyle.css 404
                //   nortenoticia      cdnjs …/tailwind.min.css 404
                // ```
                //
                // The reason string those rows carried asserted the opposite in so many words —
                // *"the sheets were cut by our own load deadline, NOT refused by the origin"* — and
                // it was wrong on 3 of 3. This is the second time in five ticks that the
                // scorability ceiling turned out to be overstated by a reason nobody had `curl`ed
                // (t856: 10 of 12 `shell-only` rows were an oracle bound, not engine work).
                //
                // The line is drawn at **404/410 only**, and narrowly on purpose: a `403` on a
                // stylesheet is very often a bot-wall answering US differently than it answers
                // Chrome, which IS a divergence we own, and a `5xx` is weather that may well have
                // served Chrome fine. Those keep counting. "The resource does not exist" is the one
                // status that is the same answer for every client.
                None if absent => {
                    tracing::info!(%url, "stylesheet 404/410 — the author's link is dead at the \
                                          origin, so the reference browser renders without it too; \
                                          NOT counted as a failure to style this page");
                    self.failed_css.remove(&url);
                    self.absent_css.insert(url);
                }
                // A stylesheet that fails to arrive is not a cosmetic loss — it is the difference
                // between a site's desktop layout and its mobile one. Say so — and COUNT it, so a
                // measurement can refuse to score a page we never actually styled (tick 383: the
                // oracle crawl booked exactly this as 100s of phantom engine divergences per site).
                None => {
                    tracing::warn!(%url, "STYLESHEET FAILED — the page will render unstyled or \
                                              in its fallback layout");
                    self.failed_css.insert(url);
                }
            }
        }
        // ── **BANK THE SHEETS THAT ARE ALREADY IN HAND, BEFORE CHASING THE ENHANCEMENTS.**
        //
        // Everything below this line — the `@import` walk (up to three network rounds) and the
        // `@font-face` `src` fetches (a SEQUENTIAL loop over every source of every face) — is more
        // network, and `finish_loading` wraps this whole phase in the load budget as a **hard
        // deadline, enforced wherever it runs out, including in the middle of a fetch.** The budget's
        // safety story is that "a dropped future loses that phase's *enhancement* and never a
        // half-mutated document". For this phase that story was FALSE: the apply lived at the very
        // bottom, so a cancellation threw away every top-level stylesheet we had already fully
        // downloaded — and a page with no author CSS is not a lost enhancement, it is the entire
        // design of the site gone.
        //
        // `keirin.jp` is the measured case, and the log reads like the bug's own confession: nine
        // sheets, 375KB, all nine logged `stylesheet applied` at +0.2s — then eleven and a half
        // seconds inside font-awesome's per-face `src` ladder, then `load budget of 12.0s exhausted
        // mid-phase`, and the future died two stages above the cascade. Coverage **98%** (we render
        // every element Chromium does) against SHAPE **2.1%** — every box a full-width UA block in
        // `serif/16` where Chromium had `{Meiryo UI/…}`, the document three times too tall. The
        // stylesheets were on this machine the whole time.
        //
        // So the primary artifact is committed here, where it is complete: the sheets go into
        // `external_css` (so any later re-cascade can find them — see `all_sheets`) and the cascade
        // runs on them. The imports and the fonts then arrive as what they actually are —
        // enhancements — and the apply at the bottom re-cascades **only if they changed the
        // fingerprint**, so a page with no `@import` and no new face pays one hash, not one cascade.
        // A page that does get a late face pays a second cascade, which is exactly what a real
        // browser does when a webfont lands (FOUT is the web's own model), and it is a far better
        // trade than rendering the site naked.
        //
        // `apply_stylesheets` is what banks `external_css` (its first statement), so this one call
        // does both halves — the map and the cascade — and there is no second copy of either.
        self.apply_stylesheets(&external, fonts, viewport_width);

        // ── `@import`ED SHEETS (tick 564). An unfetched import drops a WHOLE STYLESHEET.
        //
        // `@import url(https://fonts.googleapis.com/css?family=Lora:400,700)` inside an external sheet
        // is how a very large share of the web delivers its fonts, and how CSS architectures put a
        // design system behind one entry point. We never fetched them, so every rule and every
        // `@font-face` in the imported sheet was silently absent. Measured at t563 on martinfowler.com:
        // its `home.css` imports Open Sans, Inconsolata and Lora, Chromium resolved `{Lora/13}` where we
        // fell back to `{serif/13}`, and that one substitution made a `<p>` 293px wide in Chromium and
        // 619px in ours — a different wrap width cascading through everything below it.
        //
        // Resolved against the IMPORTING sheet's own URL (that is what `@import` is relative to, not the
        // document), deduped through the same `external_css` map so a re-entry cannot re-fetch, and
        // bounded to a small depth: import chains are legitimate (tokens → components → page) but an
        // unbounded walk is a cycle waiting to hang a tab, and Bar 0 outranks the last sheet in a chain.
        const IMPORT_DEPTH: usize = 3;
        for _ in 0..IMPORT_DEPTH {
            let mut want: Vec<(String, String)> = Vec::new(); // (import url, resolved)
                                                              // ⚠⚠⚠ **AN `@import` INSIDE AN INLINE `<style>` WAS NEVER FETCHED, AND IT IS THE
                                                              // SPELLING GOOGLE FONTS' OWN "@import" TAB TELLS YOU TO PASTE.** t564 wired this walk
                                                              // over `external` — the `<link rel=stylesheet>` sheets — and stopped there, so an
                                                              // import declared in a `<style>` block dropped the whole imported sheet: its rules, its
                                                              // `@font-face`s, everything. The engine even said so and nothing read it — stylo's own
                                                              // parser logs `Saw @import rule, but no way to trigger the load` once per occurrence.
                                                              //
                                                              // The A/B is one line of markup apart, `font-family:'M PLUS 1p'` at 14px, and the
                                                              // external row is the CONTROL that proves the walk itself works:
                                                              //
                                                              // ```text
                                                              //   where the @import lives          Chrome            before
                                                              //   in an EXTERNAL sheet (<link>)    119.313 x 20      119 x 20   ✓ (t564)
                                                              //   in an INLINE <style>             119.313 x 20      111 x 16   ✗
                                                              // ```
                                                              //
                                                              // PRICED before building, on 73 CrUX corpus pages actually fetched: **2 carry one**
                                                              // (2.7%), both Google Fonts, and one of them — `pasarbokep.com` — is in the near-bar
                                                              // band the burndown ranks. A small share, and a TOTAL loss on the pages that have it.
                                                              //
                                                              // The base is the DOCUMENT's URL, which is what an inline sheet's relative URLs resolve
                                                              // against (CSS Values §4.2) — the same rule the `@font-face` `src` scan below applies,
                                                              // and for the same reason.
            for src in &sources {
                if let StyleSource::Inline(css, _) = src {
                    for spec in Stylesheet::parse(css).imports() {
                        let resolved = resolve_url(&self.final_url, &spec);
                        if !external.contains_key(&resolved)
                            && !self.failed_css.contains(&resolved)
                            && !want.iter().any(|(_, r)| r == &resolved)
                        {
                            want.push((spec, resolved));
                        }
                    }
                }
            }
            for (sheet_url, css) in &external {
                for spec in Stylesheet::parse(css).imports() {
                    let resolved = resolve_url(sheet_url, &spec);
                    if !external.contains_key(&resolved)
                        && !self.failed_css.contains(&resolved)
                        && !want.iter().any(|(_, r)| r == &resolved)
                    {
                        want.push((spec, resolved));
                    }
                }
            }
            if want.is_empty() {
                break;
            }
            let got = collect_before_deadline(
                want.into_iter().map(|(_, url)| async move {
                    let text = manuk_net::fetch(&url)
                        .await
                        .ok()
                        .and_then(|r| subresource_text(&r));
                    (url, text)
                }),
                self.load_deadline,
            )
            .await;
            for (url, text) in got {
                match text {
                    Some(t) => {
                        tracing::info!(bytes = t.len(), %url, "@import sheet applied");
                        external.insert(url, t);
                    }
                    None => {
                        tracing::warn!(%url, "@IMPORT SHEET FAILED — its rules and @font-faces are absent");
                        self.failed_css.insert(url);
                    }
                }
            }
        }
        // The imported sheets must reach the CASCADE too, not only the @font-face scan below — an
        // import that delivered `@font-face` almost always delivers rules as well.
        for (url, css) in &external {
            if !sources
                .iter()
                .any(|s| matches!(s, StyleSource::External(u, _) if u == url))
            {
                // No media condition: an imported sheet's own `@media` (and any media list on the
                // `@import` itself, which the enclosing sheet already carries) is decided by the
                // cascade, not here.
                let _ = css;
                sources.push(StyleSource::External(url.clone(), None));
            }
        }

        // Web fonts: fetch @font-face sources (from inline + external CSS) and register
        // them BEFORE the relayout, so the cascade's font-family resolves to them.
        //
        // `registered_webfont` records whether a face arrived that the document has NOT been laid
        // out with yet — see the relayout guard below.
        let mut registered_webfont = false;
        // ⚠⚠⚠ **THE DOCUMENT'S OWN CODEPOINTS, COLLECTED ONCE — THE OTHER HALF OF `unicode-range`.**
        //
        // The inlined Google-Fonts block declares one `@font-face` PER SUBSET: `kuechenmomente.de`
        // ships **100 of them, all named `Raleway`**, Cyrillic and Vietnamese first in source order.
        // Registered blind they all land in one family list where `FontContext::face_id` selects on
        // weight and style alone, so a Cyrillic subset and the Latin subset are indistinguishable —
        // and our `Raleway/18` advance measured **240 against Chrome's 166** (t1153), every text box
        // on the page 45% too wide, re-wrapping prose and arriving downstream as `dy`.
        //
        // Skipping a block the document cannot use is the SELECTION fix and the REQUEST-COUNT fix in
        // one move: ninety-nine of those hundred are never fetched, on a page Chrome serves with one.
        //
        // Lazily built, because a page with no `@font-face` must not pay for it, and a `HashSet` of
        // distinct scalars rather than of characters — a page has hundreds of distinct codepoints,
        // not thousands.
        let mut doc_codepoints: Option<std::collections::HashSet<u32>> = None;
        for s in &sources {
            // ⚠ **A RELATIVE `src` RESOLVES AGAINST THE STYLESHEET, NOT THE DOCUMENT** (CSS Values
            // §4.2). This loop is iterating `sources` and holds the sheet's own URL, and it used to
            // throw it away one line later and resolve against `self.final_url`. That is right for
            // an inline `<style>` and wrong for every external sheet not sitting at the site root:
            // `/assets/css/main.css` + `url(../fonts/x.woff2)` is `/assets/fonts/x.woff2`, but
            // against the document it is `/fonts/x.woff2` — a 404, and a webfont that 404s does not
            // announce itself, it just looks like a page in a different font.
            let (css, base) = match s {
                StyleSource::Inline(c, _) => (c.clone(), self.final_url.clone()),
                StyleSource::External(url, _) => {
                    (external.get(url).cloned().unwrap_or_default(), url.clone())
                }
            };
            for ff in Stylesheet::parse(&css).font_faces() {
                // ── DECLARE FIRST, FETCH SECOND (tick 561). CSS Fonts' shadowing rule is about the
                // DECLARATION: once this document says `@font-face { font-family: "Open Sans" }`, a
                // locally-installed `Open Sans` is shadowed for this document, and if every `src` fails
                // the family yields no usable face and matching continues to the NEXT `font-family`
                // entry. Declaring only on success would let a failed download be masked by a
                // same-named local face — measured at t559/t560 as a 19-point SHAPE loss on
                // martinfowler.com, where a failed webfont looked like a different font rather than
                // like a failure.
                fonts.declare_webfont_family(&ff.family);
                // ⚠ **AFTER `declare_webfont_family`, DELIBERATELY.** CSS Fonts' shadowing rule is
                // about the DECLARATION (t561, above): the family is claimed by this document
                // whether or not this particular subset is one we need, and skipping the declaration
                // would let a locally-installed same-named face mask the family. What is skipped is
                // the FETCH.
                if let Some(ranges) = &ff.unicode_range {
                    let cps = doc_codepoints.get_or_insert_with(|| {
                        let mut set = std::collections::HashSet::new();
                        for n in self.dom.descendants(self.dom.root()) {
                            if let manuk_dom::NodeData::Text(t) = self.dom.data(n) {
                                set.extend(t.chars().map(|c| c as u32));
                            }
                        }
                        set
                    });
                    if !cps
                        .iter()
                        .any(|c| ranges.iter().any(|&(lo, hi)| *c >= lo && *c <= hi))
                    {
                        continue;
                    }
                }
                // **Idempotence is keyed on the SRC URL, not on the family.** This function runs
                // again after EVERY round of dynamic scripts, so without a key the same font is
                // re-fetched and re-registered each round — and because a new face forces a
                // relayout, that would be a full-document relayout per round: exactly the waste the
                // guard below exists to prevent.
                //
                // ⚠ The key used to be `has_webfont_face(&ff.family)`, and a family is the WRONG
                // grain: a real site declares one `@font-face` block per weight and style, all under
                // one name (regular / italic / 700 / 700-italic — what every self-hosted Google font
                // ships). The first block registered and the other three hit `continue`, so **every
                // bold and italic run on the page was measured in the regular face**, with no
                // synthetic bold to soften it — while `FontContext::face_id`'s weight/style search
                // over the family's face list sat there unable to ever fire. The src URL is stable
                // across re-runs (same idempotence) and distinct per face (all four now load).
                //
                // The claim is on `(family, the block's FIRST src)`, which identifies the BLOCK — not
                // on each src in turn. A block lists the same face in several formats
                // (`url(x.woff2), url(x.woff)`) and stops at the first that works, so claiming
                // per-src would leave the later formats unclaimed and the next script round would
                // fetch the `.woff` fallback of a face that already loaded as `.woff2`. The family is
                // in the key because two families may legitimately point at the same file.
                let urls: Vec<String> = ff.srcs.iter().map(|s| resolve_url(&base, s)).collect();
                let Some(first) = urls.first() else { continue };
                if !fonts.claim_webfont_src(&ff.family, first) {
                    continue; // this block was already tried this load — arrived or 404'd
                }
                for url in &urls {
                    if let Some(data) = fetch_font_bytes(url).await {
                        fonts.register_named_font(&ff.family, data);
                        // **A FONT THAT ARRIVES IS A REASON TO RE-LAY-OUT, and it was not one.**
                        // Every text box was measured with the fallback face; the new face has
                        // different advances, so every line box, wrap point and content height in
                        // the document is now stale.
                        registered_webfont = true;
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
        // …**and a web font that just arrived is the THIRD reason**, which this condition did not
        // know about. Measured: a page whose `@font-face` is in an INLINE `<style>` — what every
        // "inline your critical CSS" build produces — has `count == 0` and a clean tree, so it took
        // the `else` branch and kept the layout computed with the FALLBACK face. The font
        // downloaded, decoded, registered and resolved correctly (`manuk-text` measures Ahem at
        // exactly 100.0px for 5 chars at 20px) and then nothing asked the document to use it: the
        // box stayed 66.7px. **An optimisation guard is only as correct as its list of inputs.**
        if count > 0 || self.dom.has_dirty() || registered_webfont {
            // **Part 22.3: no duplicate tree renders.** `apply_stylesheets` now returns
            // `RestyleDamage::None` when the cascade's inputs have not moved (same sheets, same
            // tree). Laying out anyway threw that away: a full-document layout ran after EVERY round
            // of dynamic scripts whether or not the round changed anything. bbc.co.uk performed
            // **nine** full-document layouts for one navigation at 257ms each — over two seconds of
            // relaying-out a tree that had not changed.
            let damage = self.apply_stylesheets(&external, fonts, viewport_width);
            // ⚠⚠ **…and `registered_webfont` belongs HERE TOO, which is where it was missing.**
            //
            // The condition above learned that an arriving face is a third reason to re-cascade. This
            // early return — inside the branch that condition opens — did not, and it is the one that
            // decides whether the RELAYOUT happens. For the case the gate is named for (an
            // `@font-face` in an INLINE `<style>`, which every "inline your critical CSS" build
            // produces) the numbers line up exactly wrong: `count == 0`, the tree is clean, so the
            // branch is entered *only* because of `registered_webfont` — and then `apply_stylesheets`
            // fingerprints its inputs, finds the style SOURCES unchanged (the same inline `<style>`
            // as before; a font is not a source), returns `RestyleDamage::None`, and this line
            // returns before the relayout it was entered for.
            //
            // The font downloads, decodes, registers and resolves correctly — `manuk-text` measures
            // Ahem at exactly 100.0px for 5 chars at 20px — and the document keeps the layout it
            // computed with the FALLBACK: 66.7px. `g_webfont_relayout` has been RED on this since
            // before t718 and nothing said so, because it is one of the ~85 gates the wall does not
            // run (t730/t731).
            //
            // **One rule, two places.** The entry condition and the early return are the same
            // question asked twice — *"is there a reason to redo the layout?"* — and only one of them
            // was taught the third reason.
            if damage == RestyleDamage::None && !self.dom.has_dirty() && !registered_webfont {
                return count;
            }
            // ⚠⚠ **A FONT IS NOT A STYLE INPUT, AND THAT IS THE WHOLE BUG.**
            //
            // The relayout lives inside `apply_stylesheets`, behind a fingerprint of *the cascade's
            // inputs* — the style sources and the shape of the tree. A newly-arrived face changes
            // neither: the same inline `<style>` that declared `@font-face` was there before the
            // bytes came back. So the fingerprint matches, `apply_stylesheets` returns
            // `RestyleDamage::None`, and **it skips the layout as well as the cascade** — which is
            // correct for every input it knows about and wrong for the one it does not.
            //
            // The guard above had already learned that an arriving face is a third reason to re-do
            // the work; it just handed that reason to a function that answers a different question.
            // **A trigger is only as good as what it triggers**, and the two were one call apart.
            //
            // Measured: `g_webfont_relayout` — an `@font-face` in an INLINE `<style>`, which is what
            // every "inline your critical CSS" build produces — measured **66.7px** where Ahem
            // guarantees exactly **100px** for 5 characters at 20px. The font downloaded, decoded,
            // registered and resolved correctly the whole time; nothing ever asked the document to
            // measure itself again. RED since before t718 and unnoticed because this gate is one of
            // the ~85 the wall does not run (t730/t731).
            if registered_webfont && damage == RestyleDamage::None {
                let sheets: Vec<Stylesheet> = self.all_sheets();
                (self.styles, self.root_box) =
                    restyle_and_layout(&self.dom, &sheets, fonts, viewport_width, &self.images);
                self.reapply_scroll_offsets();
                self.content_height = self.root_box.rect.height;
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
    /// Nodes whose computed `visibility` is `hidden`/`collapse` — laid out, occupying space, but
    /// not perceivable and therefore neither exposed to accessibility nor hit-testable.
    ///
    /// `visibility` INHERITS, and the computed style already carries the inherited value, so a
    /// per-node read is sufficient and a subtree walk would be redundant. A descendant may set
    /// `visibility: visible` back on inside a hidden ancestor — which is exactly why this is a
    /// per-node test rather than a subtree prune; the tree builder skips the hidden node and keeps
    /// walking, so the re-shown descendant survives.
    fn invisible_nodes(&self) -> std::collections::HashSet<manuk_dom::NodeId> {
        self.styles
            .iter()
            .filter(|(_, s)| s.visibility != manuk_css::Visibility::Visible)
            .map(|(n, _)| *n)
            .collect()
    }

    /// Nodes dropped from coordinate hit-testing — an agent grounding a click does not land on them —
    /// while they stay in the a11y tree (a screen reader still announces them). Two sources:
    ///
    ///   * computed `pointer-events: none` (a decorative overlay/scrim). This inherits through the
    ///     *cascade*, so the computed value on each node already reflects the overlay→subtree
    ///     inheritance and a per-node read is sufficient.
    ///   * the HTML `inert` content attribute (what `<dialog>.showModal()` sets on the rest of the
    ///     page to neutralise it). `inert` inherits down the *DOM subtree*, not the cascade — an
    ///     element with `inert` makes itself AND every descendant non-interactive — so it needs a
    ///     subtree walk, not a per-node style read.
    ///
    /// Mirrors [`Self::invisible_nodes`].
    fn non_hittable_nodes(&self) -> std::collections::HashSet<manuk_dom::NodeId> {
        let mut set: std::collections::HashSet<manuk_dom::NodeId> = self
            .styles
            .iter()
            .filter(|(_, s)| s.pointer_events == manuk_css::PointerEvents::None)
            .map(|(n, _)| *n)
            .collect();
        // Walk the DOM; once inside an `inert` element, every descendant is inert too (the attribute
        // does not need to repeat on children). A descendant cannot escape inertness here — the
        // top-layer/modal-dialog escape is a niche the common modal-backdrop case does not need.
        let mut stack = vec![(self.dom.root(), false)];
        while let Some((node, inherited)) = stack.pop() {
            let is_inert = inherited
                || self
                    .dom
                    .element(node)
                    .is_some_and(|e| e.attr("inert").is_some());
            if is_inert {
                set.insert(node);
            }
            for c in self.dom.children(node) {
                stack.push((c, is_inert));
            }
        }
        set
    }

    /// Whether `node` is inside (or itself carries) an `inert` subtree — an ancestor walk, the single-
    /// node counterpart to [`Self::non_hittable_nodes`]'s top-down set. Used to refuse focus: an inert
    /// element cannot receive focus (the page behind an open modal is not tabbable and `el.focus()` on
    /// it is a no-op).
    fn is_inert(&self, node: manuk_dom::NodeId) -> bool {
        let mut cur = Some(node);
        while let Some(n) = cur {
            if self
                .dom
                .element(n)
                .is_some_and(|e| e.attr("inert").is_some())
            {
                return true;
            }
            cur = self.dom.parent(n);
        }
        false
    }

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
        manuk_paint::DisplayList::build_captioned(
            &self.root_box,
            &self.images,
            &z,
            &self.caption_map(),
        )
    }

    /// The cues the UA must paint over each media box right now.
    ///
    /// Read fresh on every paint rather than cached, because the thing that changes it is the page
    /// moving its own `currentTime` — there is no host-side event to invalidate a cache on, and a
    /// stale caption is the failure this whole arc is about.
    ///
    /// A cue whose element is no longer in the tree simply never matches a layout box, so a removed
    /// `<video>` cannot leave a caption floating over the page.
    pub fn caption_map(&self) -> manuk_paint::CaptionMap {
        manuk_js::active_cues()
            .into_iter()
            .map(|(node, cues)| {
                let cues = cues
                    .into_iter()
                    .map(|c| manuk_paint::CaptionCue {
                        text: c.text,
                        line: c.line,
                        line_is_percent: c.line_is_percent,
                        position: c.position,
                        size: c.size,
                        align: c.align,
                        vertical: c.vertical,
                    })
                    .collect();
                (manuk_dom::NodeId(node), cues)
            })
            .collect()
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
        let mut dark_scheme = false;
        for sel in ["html", "body"] {
            let Some(n) = self.dom.find_first(sel) else {
                continue;
            };
            let _ = root;
            if let Some(st) = self.styles.get(&n) {
                // `color-scheme: dark` on the root opts the whole page into a dark UA appearance —
                // its most visible effect is the canvas default: a dark-only page with no explicit
                // background must paint dark below its content, not on the white void the old
                // hard-coded WHITE produced. An explicit background still wins (checked first).
                if st.color_scheme.is_dark() {
                    dark_scheme = true;
                }
                if let Some(bg) = st.background_color {
                    if bg.a > 0 {
                        return bg;
                    }
                }
            }
        }
        // The UA dark canvas surface (Chrome renders the dark `Canvas` system color, ~#121212). We
        // do not pin the exact value — Bar 2 is deferred — only that a dark-scheme page is dark.
        if dark_scheme {
            return Rgba::new(18, 18, 18, 255);
        }
        Rgba::WHITE
    }

    pub fn paint(&self, fonts: &FontContext, width: u32, height: u32) -> Canvas {
        let z = self.z_index_map();
        let clip = self.clip_map();
        let caps = self.caption_map();
        CpuPainter::with_layers(fonts, &self.images, &z, &clip)
            .with_captions(&caps)
            .render(&self.root_box, width, height, self.canvas_background())
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
        // ⭐ **THE TREE ALREADY CARRIES THE SHIFT** (tick 1282) — `restick` bakes it in for
        // `sticky_scroll_y`, so the common case (the host paints the scroll it just reported) is a
        // plain borrow and the per-frame deep clone this used to pay on every sticky page is gone.
        //
        // A caller asking for a DIFFERENT scroll than the page was last told about still gets a
        // correct picture: un-bake what the ledger says is in the tree, then re-apply for the scroll
        // requested. Un-baking FIRST is what stops the two shifts compounding — the failure that
        // baking a paint effect into the layout tree invites, and the reason the ledger exists.
        let sticky_boxes;
        let boxes: &LayoutBox = if self.has_sticky && (scroll_y - self.sticky_scroll_y).abs() > 0.01
        {
            let mut b = self.root_box.clone();
            for (node, dy) in &self.sticky_applied {
                if let Some(x) = b.find_mut(*node) {
                    x.shift_y(-dy);
                }
            }
            let mut applied = std::collections::HashMap::new();
            apply_sticky(&mut b, &self.styles, scroll_y, height as f32, &mut applied);
            sticky_boxes = b;
            &sticky_boxes
        } else {
            &self.root_box
        };
        let caps = self.caption_map();
        CpuPainter::with_layers(fonts, &self.images, &z, &clip)
            .with_captions(&caps)
            .render_scrolled(boxes, width, height, self.canvas_background(), scroll_y)
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
        // ── A LINE-BREAK OPPORTUNITY IS NOT A SPACE.
        //
        // The line breaker emits one fragment per break *opportunity*, not per line — and CSS puts
        // one after a hyphen, after `//`, and after `?` in a query string. This used to be
        // `words.join(" ")`, so a word the layout merely COULD have broken came back broken, on the
        // same line, with a space wedged into it: `non- mainstream`, `https:// walled.example/? a=1`.
        //
        // Nothing about the rendering was wrong — the pixels and the DOM `textContent` are both
        // right. Only this string was, and this string is `Observation.text` (what `manuk-agent`
        // hands a model) and the body `store::history_index` embeds. So a model asked to find
        // "non-mainstream" found nothing, and so did a user searching their history for a URL.
        //
        // The geometry needed to tell the two cases apart was already on the fragment, and the rule
        // now lives on `TextFragment::continues` — beside the data, because find-in-page and
        // selection-copy had made the identical mistake independently (tick 578).
        let mut out = String::new();
        let mut prev: Option<manuk_layout::TextFragment> = None;
        self.root_box.walk(&mut |b| {
            if let BoxContent::Inline(frags) = &b.content {
                for f in frags {
                    if f.text.is_empty() {
                        continue;
                    }
                    // Nothing emitted yet ⇒ no separator before the first run.
                    if prev.as_ref().is_some_and(|p| !f.continues(p)) {
                        out.push(' ');
                    }
                    out.push_str(&f.text);
                    prev = Some(f.clone());
                }
            }
        });
        out
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
/// Every `<meta http-equiv="Content-Security-Policy" content="…">` in the document, in order.
///
/// A DOM walk rather than the charset prescan's raw-byte scan, because a CSP meta is not bounded to
/// the first 1024 bytes the way `<meta charset>` is, and because by this point the document is
/// already parsed — reading it out of the tree cannot disagree with what the tree says.
fn collect_meta_csp(dom: &Dom) -> Vec<String> {
    let mut out = Vec::new();
    for n in dom.descendants(dom.root()) {
        if dom.tag_name(n) != Some("meta") {
            continue;
        }
        let Some(el) = dom.element(n) else { continue };
        if !el
            .attr("http-equiv")
            .is_some_and(|v| v.trim().eq_ignore_ascii_case("content-security-policy"))
        {
            continue;
        }
        if let Some(c) = el.attr("content") {
            out.push(c.to_string());
        }
    }
    out
}

/// **Does `bytes` satisfy an `integrity` attribute?**
///
/// The attribute is a whitespace-separated list of `<algo>-<base64digest>` metadata. Per SRI §3.3.4
/// the strongest algorithm present wins and the content is valid if it matches **any** entry of that
/// strength — which is what lets a page list a fallback for older agents without weakening itself.
/// Implementing "any entry at all matches" instead would let a page's own weak `sha256` fallback
/// downgrade its `sha512`, so the strength selection is not a detail.
///
/// Unknown algorithms are ignored (spec: they are not valid metadata). An `integrity` attribute that
/// contains **no** recognised algorithm is treated as no integrity at all, per §3.3.3 — `integrity=""`
/// must not block the script, and neither must `integrity="md5-…"`.
fn sri_matches(spec: &str, bytes: &[u8]) -> bool {
    use base64::Engine as _;
    use sha2::Digest as _;

    // Strength order matters; the strongest algorithm the attribute names is the only one checked.
    let mut best: Option<&'static str> = None;
    for tok in spec.split_whitespace() {
        let algo = match tok.split('-').next() {
            Some("sha512") => "sha512",
            Some("sha384") => "sha384",
            Some("sha256") => "sha256",
            _ => continue,
        };
        let rank = |a: &str| match a {
            "sha512" => 3,
            "sha384" => 2,
            _ => 1,
        };
        if best.is_none_or(|b| rank(algo) > rank(b)) {
            best = Some(algo);
        }
    }
    let Some(algo) = best else {
        return true; // no recognised metadata == no integrity requirement (SRI §3.3.3)
    };

    let digest: Vec<u8> = match algo {
        "sha512" => sha2::Sha512::digest(bytes).to_vec(),
        "sha384" => sha2::Sha384::digest(bytes).to_vec(),
        _ => sha2::Sha256::digest(bytes).to_vec(),
    };
    // Both alphabets: the spec says base64, and real pages carry standard-alphabet digests, but a
    // url-safe one is a copy-paste away and rejecting it would fail closed on a *correct* hash.
    let std_b64 = base64::engine::general_purpose::STANDARD.encode(&digest);
    let url_b64 = base64::engine::general_purpose::URL_SAFE.encode(&digest);

    spec.split_whitespace().any(|tok| {
        let Some((a, want)) = tok.split_once('-') else {
            return false;
        };
        if !a.eq_ignore_ascii_case(algo) {
            return false;
        }
        // Trailing `?options` is allowed by the grammar and carries no meaning today.
        let want = want.split('?').next().unwrap_or(want);
        want == std_b64 || want == url_b64
    })
}

/// Fetch every external `<script src>` in `dom` (resolved against `base`) and inline its
/// content as the script node's text, dropping the `src`, so the from_dom script pass runs it.
/// External scripts fetch sequentially in document order (the classic-script model).
#[cfg(feature = "spidermonkey")]

/// Returns `(csp-authorized nodes, node -> the URL each inlined script CAME FROM)`.
///
/// **The second half exists because a module's imports resolve against the MODULE's url, not the
/// document's.** This function inlines an external `<script src>` into the node and drops `src`, so by
/// the time `prefetch_module_graph` walks the DOM an external module is indistinguishable from an
/// inline one — and it was resolving `./chunks/react.js` against the document. On `www.welt.de` that
/// turned `/assets/bff-section/scripts/chunks/react.BPdhuoKc.js` (200, 8KB of real JavaScript) into
/// `/chunks/react.BPdhuoKc.js` (404, 414KB of HTML), which then compiled as a module:
/// `SyntaxError: expected expression, got '<'`. Carrying the origin URL forward is what lets the
/// graph walk ask the right question.
async fn fetch_external_scripts(
    dom: &mut Dom,
    base: &str,
    csp: &manuk_net::csp::Csp,
) -> (Vec<NodeId>, HashMap<NodeId, String>) {
    let mut targets = Vec::new();
    let mut origins: HashMap<NodeId, String> = HashMap::new();
    for n in dom.descendants(dom.root()) {
        if dom.tag_name(n) == Some("script") {
            // **`nomodule` is checked BEFORE the request, exactly as CSP is below.** This browser
            // runs `type="module"`, so the legacy half of a differential build must not be fetched
            // at all — downloading it spends the load budget on a bundle we are forbidden to run,
            // and leaving `src` in place is precisely the "there is nothing to run" path a blocked
            // or failed fetch already takes. One rule, one implementation:
            // `manuk_js::script_is_nomodule_classic`, shared with `collect_inline_scripts`.
            if let Some(el) = dom.element(n) {
                if manuk_js::script_is_nomodule_classic(
                    el.attr("type"),
                    el.attr("nomodule").is_some(),
                ) {
                    tracing::debug!("nomodule <script src> skipped — this browser runs modules");
                    continue;
                }
            }
            if let Some(src) = dom.element(n).and_then(|e| e.attr("src")) {
                if let Ok(u) = Url::parse(base).and_then(|b| b.join(src)) {
                    // **CSP is checked before the request is issued, not after it lands.** A blocked
                    // script that is fetched anyway still tells the attacker's server that this user
                    // visited this page with this session — half of what the policy was written to
                    // prevent. The `src` is left in place, so `collect_inline_scripts` skips the node
                    // for the same reason it skips a failed fetch: there is nothing to run.
                    if !csp.allows_script_url(&u) {
                        tracing::info!(url = %u, "CSP blocked a <script src> — not fetched");
                        continue;
                    }
                    // `integrity` is read HERE, with the node, because after the fetch the
                    // element has been rewritten (`src` dropped, text inlined) and the attribute
                    // would have to be re-found on a node that no longer looks external.
                    let integrity = dom
                        .element(n)
                        .and_then(|e| e.attr("integrity"))
                        .map(str::to_string);
                    targets.push((n, u.to_string(), integrity));
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
        futures_util::future::join_all(targets.iter().map(|(n, url, integ)| async move {
            (*n, integ.clone(), manuk_net::fetch(url).await.ok())
        })),
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
    let mut authorized = Vec::new();
    for (node, integrity, resp) in fetched {
        match resp {
            Some(r) => {
                // **Subresource Integrity, checked on the RAW bytes before anything is executed.**
                //
                // `integrity="sha384-…"` is the page telling us, in advance, exactly which bytes it
                // expects — the one control a page has against a compromised or swapped CDN. Running
                // the script anyway is not a partial implementation of that promise, it is the
                // absence of it, and the page cannot tell the difference: nothing throws and nothing
                // is logged, so a supply-chain substitution executes silently. That is the same
                // shape as every silent-failure defect this engine has been caught by, with the
                // consequence turned up.
                //
                // The digest is over `body`, the bytes as received — NOT `decoded_text()`. A
                // transcode to UTF-8 changes the bytes and would make every hash on a non-UTF-8 or
                // BOM-prefixed script fail for the wrong reason.
                if let Some(spec) = integrity.as_deref() {
                    if !sri_matches(spec, &r.body) {
                        tracing::warn!(
                            "SRI mismatch — <script src> NOT executed (integrity=\"{spec}\")"
                        );
                        // Leave `src` in place: `collect_inline_scripts` skips a node that still
                        // looks external, which is exactly the "there is nothing to run" path a
                        // failed fetch already takes.
                        continue;
                    }
                }
                // **An error page is not a script.** `continue` takes the exact "there is nothing to
                // run" path a failed fetch and an SRI mismatch already take (both leave `src` in
                // place, and `collect_inline_scripts` skips a node that still looks external) —
                // instead of injecting an HTML error document as inline JavaScript, whose SyntaxError
                // would then kill whatever frame ran it.
                let Some(js) = subresource_text(&r) else {
                    continue;
                };
                // Remember where this script CAME FROM before the evidence is destroyed. `src` is
                // removed on the next line and the node then looks inline forever after.
                origins.insert(node, r.final_url.to_string());
                dom.remove_attr(node, "src");
                let text = dom.create_text(js);
                dom.append_child(node, text);
                // CSP already said yes to this URL, above. Record the node so the inline check does
                // not ask a second, different question about the same script — see
                // `set_pending_csp_with_authorized`.
                authorized.push(node);
            }
            None => tracing::warn!("external script fetch failed"),
        }
    }
    (authorized, origins)
}

/// **Scan JS source for the specifiers of its STATIC `import` / `export … from` statements (B3b).**
///
/// The page-path module-graph pre-fetch needs to know *what a module imports* before any JS runs, so it
/// can fetch the whole transitive graph up front (there is no synchronous network on the JS thread — the
/// authoritative walk in `esm_load_graph` reads a *pre-fetched* map). SpiderMonkey gives the exact
/// specifiers only *after* a module is compiled, and compiling needs the JS engine on the UI thread —
/// so this is a lightweight textual pre-scan whose only job is to seed the fetch. It is deliberately a
/// SUPERSET-or-miss heuristic, not a parser: an over-match fetches a URL nothing imports (harmless), and
/// a miss means that dependency is absent from the map and its importer fails loud-but-safe at
/// `ModuleLink` — the same failure policy `esm_load_graph` already documents. `esm_load_graph` remains
/// the source of truth for which modules are actually linked; this only decides what gets fetched.
///
/// Matches the specifier string of `import x from 'm'`, `import {a} from 'm'`, `import * as n from 'm'`,
/// `import 'm'` (side-effect), and `export … from 'm'` / `export * from 'm'`. Skips comments and string
/// bodies so a `from`/`import` inside them is not mistaken for a keyword, and skips `import(` (a dynamic
/// import — resolved lazily at runtime, not part of the static graph) and `import.meta`.
fn scan_static_import_specifiers(src: &str) -> Vec<String> {
    let b = src.as_bytes();
    let n = b.len();
    let mut i = 0usize;
    let mut out = Vec::new();
    // The last significant token in code context: an identifier word, or a single punctuation byte. A
    // specifier is captured when this is exactly `from` (any import/export … from) or `import` (a bare
    // side-effect import). Tracking it — rather than the string alone — is what keeps `{`, `}`, `,`, `*`
    // between `import` and `from` from masquerading as the keyword, and what makes `import(` and
    // `import.meta` not fire the bare-import rule (the `(` / `.` becomes the previous token instead).
    let mut prev = String::new();
    while i < n {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        // Comments — skipped so a `//`/`/* */` `from` is never read as a keyword.
        if c == b'/' && i + 1 < n && b[i + 1] == b'/' {
            i += 2;
            while i < n && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if c == b'/' && i + 1 < n && b[i + 1] == b'*' {
            i += 2;
            while i + 1 < n && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(n);
            continue;
        }
        // String literal — capture its body; a `'`/`"` body that directly follows `from` or `import`
        // is a specifier. Backtick strings are consumed but never captured (a static specifier is a
        // plain string literal, never a template).
        if c == b'\'' || c == b'"' || c == b'`' {
            let quote = c;
            i += 1;
            let mut content: Vec<u8> = Vec::new();
            while i < n {
                let d = b[i];
                if d == b'\\' && i + 1 < n {
                    content.push(b[i + 1]);
                    i += 2;
                    continue;
                }
                if d == quote {
                    i += 1;
                    break;
                }
                content.push(d);
                i += 1;
            }
            if (quote == b'\'' || quote == b'"')
                && (prev == "from" || prev == "import" || prev == "import(")
            {
                let s = String::from_utf8_lossy(&content).trim().to_string();
                if !s.is_empty() {
                    out.push(s);
                }
            }
            prev.clear();
            continue;
        }
        // Identifier / keyword.
        if c == b'_' || c == b'$' || c.is_ascii_alphabetic() {
            let s = i;
            i += 1;
            while i < n && (b[i] == b'_' || b[i] == b'$' || b[i].is_ascii_alphanumeric()) {
                i += 1;
            }
            prev = String::from_utf8_lossy(&b[s..i]).to_string();
            continue;
        }
        // Punctuation. `import(` is a dynamic import — clear so the bare-import rule cannot fire on the
        // string inside. Any other single byte becomes the previous token.
        if c == b'(' && prev == "import" {
            // **A LITERAL `import("m")` specifier IS collected (t624).** The dynamic-import host hook
            // resolves from this same pre-fetched map, so a specifier missing from it is one
            // `import()` cannot satisfy. `import(` becomes its own marker so the string branch can
            // tell it from a bare side-effect `import 'm'`. A COMPUTED specifier still cannot be seen
            // by a textual scan and rejects at runtime — over-collecting is the safe direction, since
            // a fetched module nothing imports costs one request and a missing one costs the feature.
            prev.clear();
            prev.push_str("import(");
        } else {
            prev = (c as char).to_string();
        }
        i += 1;
    }
    out
}

/// **Parse the document's import map from its `<script type=importmap>` (tick 520).** Returns the flat
/// `imports` object (bare specifier → raw target) — the standard form CDN-pinned no-bundler apps use
/// (`{"imports":{"react":"https://esm.sh/react"}}`). Per spec the FIRST import map wins and must precede
/// module loads, so only the first is read; malformed JSON yields an empty map (loud-but-safe). `scopes`
/// (per-path overrides) are not yet honoured — a bounded follow-up; the flat `imports` covers the common
/// case.
#[cfg(feature = "spidermonkey")]
fn extract_import_map(dom: &Dom) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for node in dom.descendants(dom.root()) {
        if dom.tag_name(node) != Some("script") {
            continue;
        }
        let is_importmap = dom
            .element(node)
            .and_then(|e| e.attr("type"))
            .map(|t| t.eq_ignore_ascii_case("importmap"))
            .unwrap_or(false);
        if !is_importmap {
            continue;
        }
        let text = dom.text_content(node);
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(imports) = v.get("imports").and_then(|i| i.as_object()) {
                for (k, val) in imports {
                    if let Some(s) = val.as_str() {
                        out.insert(k.clone(), s.to_string());
                    }
                }
            }
        } else {
            tracing::info!("<script type=importmap> JSON did not parse — bare specifiers unmapped");
        }
        break; // the first import map wins
    }
    out
}

/// Look a bare specifier up in an import map — exact key, then the longest trailing-slash PREFIX key.
/// Mirrors the JS-side `import_map_lookup` so the page pre-fetch and the link-time resolve hook agree.
#[cfg(feature = "spidermonkey")]
fn import_map_lookup_page(map: &HashMap<String, String>, spec: &str) -> Option<String> {
    if let Some(t) = map.get(spec) {
        return Some(t.clone());
    }
    let mut best: Option<(&String, &String)> = None;
    for (k, v) in map {
        if k.ends_with('/') && spec.starts_with(k.as_str()) {
            if best.map_or(true, |(bk, _)| k.len() > bk.len()) {
                best = Some((k, v));
            }
        }
    }
    best.map(|(k, v)| format!("{v}{}", &spec[k.len()..]))
}

/// Resolve a module specifier the way the JS `resolve_module_specifier` does, so the url pre-fetched is
/// the url the resolve hook later looks up: a relative specifier against its `importer`, a BARE specifier
/// through the import map against `doc_base`. `None` for a bare specifier with no mapping (skip — the
/// importer fails at `ModuleLink`, loud-but-safe).
#[cfg(feature = "spidermonkey")]
fn resolve_page_specifier(
    importer: &Url,
    doc_base: &Url,
    spec: &str,
    import_map: &HashMap<String, String>,
) -> Option<Url> {
    let bare = !(spec.starts_with('/') || spec.starts_with("./") || spec.starts_with("../"))
        && Url::parse(spec).is_err();
    if bare {
        let target = import_map_lookup_page(import_map, spec)?;
        return doc_base
            .join(&target)
            .ok()
            .or_else(|| Url::parse(&target).ok());
    }
    importer.join(spec).ok()
}

/// **A 404 page is a DOCUMENT to render — it is not JavaScript to execute, and not CSS to apply.**
///
/// t607 established the first half and was right: an HTTP error status arrives with a real body and a
/// browser renders it, so `manuk_net::fetch` stops failing on `status >= 400`. Its own comment said
/// *"the status is not swallowed: it rides on `Response::status` for every caller that cares"* —
/// **and none of the six subresource callers cared.** Every one of them was
/// `fetch(&url).await.ok().map(|r| r.decoded_text())`, which reads *"the request completed"* as
/// *"the request succeeded"*.
///
/// What that costs, found on `www.welt.de`:
///
/// ```text
///   a page module failed — SyntaxError: expected expression, got '<'
/// ```
///
/// A module URL answered `404`, the error page's `<!doctype html>` was inserted as the module's
/// SOURCE, and SpiderMonkey compiled HTML as JavaScript. The same hazard sits on `<script src>`,
/// where the error page becomes inline script text and its `SyntaxError` kills whatever frame runs
/// it — the throw class t612 and t615 spent two ticks on.
///
/// **The distinction is the interesting part, and it is the same fact answered three ways.** A 403
/// challenge page is: a *document* to the navigation path (render it — t607), *not evidence* to the
/// certificate (refuse to score it — t611), and *not code* here. One response, three consumers, three
/// correct and different answers. Getting one right does not settle the others.
///
/// `< 400` rather than `2xx` on purpose: redirects are already followed by the time a caller sees a
/// response, and a revalidated `304` is returned as the STORED entry (status 200), so this rejects
/// exactly the error statuses and cannot regress a path that works today.
fn subresource_text(r: &manuk_net::Response) -> Option<String> {
    if r.status >= 400 {
        tracing::info!(
            url = %r.final_url, status = r.status,
            "subresource answered with an error status — NOT executed or applied as content \
             (its body is an error page, not the resource)"
        );
        return None;
    }
    Some(r.decoded_text())
}

/// **Pre-fetch the whole static-import graph of a document's inline `<script type=module>` roots (B3b).**
///
/// Walks each module root's static `import`s (via [`scan_static_import_specifiers`]), resolving every
/// specifier the way the loader does (a relative specifier against its importer, a BARE specifier through
/// the `import_map` against the document url — tick 520), fetching each not-yet-seen module off the UI
/// thread with `manuk_net::fetch`, and recursing into *its* imports — a breadth-first walk with a visited
/// set (the diamond/cycle guard) and a hard node cap. Returns the resolved-url → source map
/// `manuk_js::set_module_graph_sources` seeds so the synchronous `ModuleLink` finds every dependency.
///
/// Failure is loud-but-safe, matching the loader: a fetch miss (404, dead host, bare specifier that
/// resolves to nothing real) is logged and skipped — the module never enters the map and its importer
/// fails at `ModuleLink`, never a crash. External `<script type=module src>` roots are already inlined
/// by [`fetch_external_scripts`] before this runs, so their source is visible here as text content.
#[cfg(feature = "spidermonkey")]
async fn prefetch_module_graph(
    dom: &Dom,
    base: &str,
    import_map: &HashMap<String, String>,
    script_origins: &HashMap<NodeId, String>,
) -> HashMap<String, String> {
    let mut sources: HashMap<String, String> = HashMap::new();
    let base_url = match Url::parse(base) {
        Ok(u) => u,
        Err(_) => return sources,
    };
    // The roots' import specifiers, each paired with **the importer URL to resolve against — which is
    // the MODULE's own url, not the document's.**
    //
    // For a genuinely inline `<script type=module>` those are the same thing, which is why the bug
    // hid: the code said "the document URL, since an inline module resolves against the document",
    // which is true, and then applied it to external modules too. `fetch_external_scripts` inlines a
    // `<script type=module src>` and drops `src`, so by the time this walk runs an external module is
    // indistinguishable from an inline one — and every relative import in it resolved one directory
    // tree too high. `script_origins` is the record of where each inlined script came from.
    let mut queue: std::collections::VecDeque<(Url, String)> = std::collections::VecDeque::new();
    for node in dom.descendants(dom.root()) {
        if dom.tag_name(node) != Some("script") {
            continue;
        }
        let is_module = dom
            .element(node)
            .and_then(|e| e.attr("type"))
            .map(|t| t.eq_ignore_ascii_case("module"))
            .unwrap_or(false);
        if !is_module {
            continue;
        }
        let src = dom.text_content(node);
        if src.trim().is_empty() {
            continue;
        }
        // An external module's imports are relative to IT; an inline one's are relative to the
        // document. Same rule, and `script_origins` is what distinguishes the two cases now that the
        // DOM no longer can.
        let importer = script_origins
            .get(&node)
            .and_then(|u| Url::parse(u).ok())
            .unwrap_or_else(|| base_url.clone());
        for spec in scan_static_import_specifiers(&src) {
            queue.push_back((importer.clone(), spec));
        }
    }
    if queue.is_empty() {
        return sources;
    }
    // BFS. The visited set is `sources`' keys: a URL is inserted the moment its source lands, so a
    // diamond (two importers, one dep) fetches once and a cycle terminates. The cap backstops a
    // pathological or adversarial graph — the loader has its own depth-256 guard behind this.
    let mut fetched_count = 0usize;
    while let Some((importer, specifier)) = queue.pop_front() {
        // Resolve like the loader: `./b.js` against the importer, a bare `react` through the import map
        // against the document url — so the pre-fetched key matches the resolve-hook lookup exactly.
        let resolved = match resolve_page_specifier(&importer, &base_url, &specifier, import_map) {
            Some(u) => u,
            None => continue,
        };
        let key = resolved.as_str().to_string();
        if sources.contains_key(&key) {
            continue;
        }
        if fetched_count >= 512 {
            tracing::warn!(
                "module graph pre-fetch hit the 512-module cap — linking with what arrived"
            );
            break;
        }
        fetched_count += 1;
        match manuk_net::fetch(&key).await {
            Ok(r) => {
                let text = r.decoded_text();
                for spec in scan_static_import_specifiers(&text) {
                    queue.push_back((resolved.clone(), spec));
                }
                sources.insert(key, text);
            }
            Err(e) => tracing::info!(url = %key, "module graph pre-fetch miss: {e}"),
        }
    }
    sources
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
    /// The document's Content-Security-Policy — response headers plus any `<meta http-equiv>`,
    /// already combined. It rides along with the DOM because it was needed to *produce* that DOM
    /// (a blocked `<script src>` was never fetched), and is needed again on the UI thread to decide
    /// which inline scripts may run.
    pub csp: Csp,
    /// The `<script>` nodes whose external source was authorized by URL during the prefetch above.
    /// See [`set_pending_csp_with_authorized`] for why the decision has to travel rather than be
    /// re-made.
    pub csp_authorized_scripts: Vec<NodeId>,
    /// The `<script>` nodes whose source was fetched from the network and inlined into the element
    /// above. They owe their element a `load` event once they execute on the UI thread, and by then
    /// the DOM can no longer tell them from an author-written inline script — so, like the CSP
    /// decision beside them, the fact has to travel. See [`set_pending_external_scripts`].
    ///
    /// ⚠ **Not a synonym for `csp_authorized_scripts`, even though today the two sets are equal.**
    /// One says *"the policy said yes to this URL"* and the other says *"this element was external"*;
    /// they coincide only because a script that is not fetched is also not inlined. Reusing one for
    /// the other would make a future CSP change silently delete a load event.
    pub external_scripts: Vec<NodeId>,
    /// External stylesheet URL → CSS text.
    pub css: HashMap<String, String>,
    /// resolved image URL → decoded bitmap. **Keyed by URL, not node** — for the same reason `masks`
    /// is: nine elements naming one sprite are one fetch and one decode, not nine. The binding to
    /// nodes happens on the UI thread in `from_prefetched`, where the DOM is available.
    pub images: HashMap<String, manuk_paint::DecodedImage>,
    /// raw `mask-image` url → decoded bitmap (icons). Keyed by URL, not node, because the
    /// off-thread prefetch has no cascade — the nodes are bound to it once styles exist.
    pub masks: HashMap<String, manuk_paint::DecodedImage>,
    /// **The pre-fetched ES-module import graph (B3b-iii)** — resolved-url → source, for every module a
    /// `<script type=module>` reaches through its static `import`s. Pre-fetched off-thread here so the
    /// synchronous `ModuleLink` (which cannot touch the network) finds the whole graph already in hand.
    /// Carried on `Prefetched` — not seeded into a thread-local off-thread — because the shell builds the
    /// page and runs its deferred scripts on the UI thread, possibly on a different worker than fetched
    /// this; the map rides on the page (`Page::module_graph_sources`) and is seeded into the JS layer at
    /// the moment the deferred pass runs. Empty ⇒ no module imports ⇒ modules link self-contained.
    pub module_graph_sources: HashMap<String, String>,
    /// `<script type=module>` node index → the URL its source came from, carried the same way and for
    /// the same reason as the graph above: a module's imports resolve against the MODULE's url.
    pub module_node_bases: HashMap<usize, String>,
    /// **The document's import map (tick 520)** — bare specifier → raw target from `<script
    /// type=importmap>`. Parsed off-thread with the graph and carried onto the page the same way, so the
    /// resolve hook resolves `import 'react'` at link. Empty ⇒ bare specifiers fail loud-but-safe.
    pub import_map: HashMap<String, String>,
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
        Loaded::Document {
            html,
            final_url,
            csp,
        } => prepare_prefetched(html, final_url, csp).await,
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
    let csp = Csp::from_headers(&resp.headers, Some(&resp.final_url));
    prepare_prefetched(resp.decoded_text(), resp.final_url.to_string(), csp).await
}

/// Turn a fetched document (`html` at `final_url`) into a [`Loaded::Prefetched`] with its external
/// scripts inlined and its stylesheets + mask icons fetched — all off the UI thread. Shared by
/// [`prefetch_document`] (GET) and [`prefetch_document_post`] (form POST) so the two navigation
/// kinds build identical pages.
async fn prepare_prefetched(html: String, final_url: String, mut csp: Csp) -> Result<Loaded> {
    #[allow(unused_mut)]
    let mut dom = manuk_html::parse(&html);
    // `<meta http-equiv="Content-Security-Policy">` — read AFTER the parse and BEFORE any script
    // fetch, which is the only ordering that works: the policy is in the markup, so it cannot be
    // known before parsing, and it must be in force before the first subresource decision.
    // A meta policy can only ever TIGHTEN the header one (policies are conjunctive), which is what
    // makes it safe to honour a policy that arrived in content the page itself authored.
    csp.set_document_url(Url::parse(&final_url).ok().as_ref());
    for content in collect_meta_csp(&dom) {
        csp.add_meta(&content);
    }
    // External <script src> — fetched and inlined here, off-thread. (Execution still
    // happens on the UI thread inside `from_dom`; only the *fetch* moves.)
    #[cfg(feature = "spidermonkey")]
    let (csp_authorized_scripts, script_origins) =
        fetch_external_scripts(&mut dom, &final_url, &csp).await;
    #[cfg(not(feature = "spidermonkey"))]
    let csp_authorized_scripts: Vec<NodeId> = Vec::new();
    // Headless has no module runner, so there is nothing to resolve imports FOR — but the field is
    // unconditional on `Prefetched`, so it needs a value in both configurations. (This is the split
    // that `--features stylo,spidermonkey` hides: the shipping build compiled clean while the headless
    // one did not.)
    #[cfg(not(feature = "spidermonkey"))]
    let script_origins: HashMap<NodeId, String> = HashMap::new();

    // **The ES-module import graph (B3b-iii) + import map (tick 520)** — pre-fetched off-thread here, on
    // the DEBT-1 path the shell actually navigates with, exactly as `load_async` does for the
    // streaming/agent path. The external `type=module src` roots were just inlined above, so every root's
    // source is visible; the import map (bare-specifier resolution) is parsed from `<script type=importmap>`
    // and feeds the graph pre-fetch so mapped urls are fetched too.
    #[cfg(feature = "spidermonkey")]
    let import_map = extract_import_map(&dom);
    #[cfg(not(feature = "spidermonkey"))]
    let import_map: HashMap<String, String> = HashMap::new();
    #[cfg(feature = "spidermonkey")]
    let module_graph_sources =
        prefetch_module_graph(&dom, &final_url, &import_map, &script_origins).await;
    #[cfg(not(feature = "spidermonkey"))]
    let module_graph_sources: HashMap<String, String> = HashMap::new();

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
        let text = manuk_net::fetch(&u)
            .await
            .ok()
            .and_then(|r| subresource_text(&r));
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
    // The prefetch path has no page-level budget of its own (the caller owns the clock), so no deadline.
    let (masks, _settled) = fetch_masks_owned(mask_targets, None).await;

    Ok(Loaded::Prefetched(Box::new(Prefetched {
        dom,
        final_url,
        csp,
        csp_authorized_scripts,
        external_scripts: script_origins.keys().copied().collect(),
        css,
        images,
        masks,
        module_graph_sources,
        module_node_bases: script_origins
            .iter()
            .map(|(n, u)| (n.index(), u.clone()))
            .collect(),
        import_map,
    })))
}

/// The outcome of navigating to a URL: a document to render, or a file to save (the server
/// marked the response `Content-Disposition: attachment` or served a non-renderable binary).
pub enum Loaded {
    Document {
        html: String,
        final_url: String,
        /// The policy from the response's `Content-Security-Policy` headers. A caller that builds a
        /// page from this must seed it with [`set_pending_csp`] first, or the document's scripts
        /// run unrestricted. (The [`Prefetched`] path — the one the shell actually navigates with —
        /// carries the policy through automatically and cannot forget.)
        csp: Csp,
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
/// Seed the window identity (own id + opener) the NEXT page built on this thread is born with.
///
/// Call this BEFORE constructing the page. `Page::set_identity` can only run once the page's
/// render-blocking scripts have already executed, which is too late for the case that matters: a
/// popup's login script reads `window.opener` at load time to post its token back to the opener. With
/// late seeding it reads `null`, posts nothing, and the opener waits on its callback forever.
pub fn set_pending_identity(win_id: u64, opener_win: u64) {
    manuk_js::set_pending_identity(win_id, opener_win);
}

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
                csp: Csp::from_headers(&resp.headers, Some(&resp.final_url)),
                html: resp.decoded_text(),
                final_url: resp.final_url.to_string(),
            }),
        }
    } else {
        // `data:`/`file:`/local paths carry no headers, so no policy — and `'self'` would name
        // nothing anyway, since those origins are opaque.
        let (html, final_url) = fetch_html(url).await?;
        Ok(Loaded::Document {
            html,
            final_url,
            csp: Csp::none(),
        })
    }
}

pub async fn fetch_html(url: &str) -> Result<(String, String)> {
    fetch_html_with_headers(url).await.map(|(t, u, _)| (t, u))
}

/// As [`fetch_html`], but also hands back the response headers — which the iframe path needs and the
/// document path does not. Split rather than widened so the ~dozen `fetch_html` call sites stay
/// unchanged: a framing decision is the only caller that cares.
pub async fn fetch_html_with_headers(url: &str) -> Result<(String, String, Vec<(String, String)>)> {
    fetch_html_with_headers_named(url)
        .await
        .map(|(t, u, h, _)| (t, u, h))
}

/// The same, **and the canonical name of the encoding the body was decoded from** — what
/// `document.characterSet` is specified to report. Split rather than widened so the existing callers
/// stay unchanged; only the frame loader, which owns a child document, has anywhere to put it.
pub async fn fetch_html_with_headers_named(
    url: &str,
) -> Result<(String, String, Vec<(String, String)>, String)> {
    if url.starts_with("http://") || url.starts_with("https://") {
        // The document gets the long deadline; its subresources get the short one. See
        // `manuk_net::fetch_document`.
        let resp = manuk_net::fetch_document(url)
            .await
            .with_context(|| format!("fetching {url}"))?;
        // The second half of the same rule enforced in `manuk_net::fetch_document_or_download`: an
        // error status is a document. This bail was the one on the path the **iframe** loader takes
        // (`fetch_and_load_iframes` → `fetch_html_with_headers`), so a framed OAuth consent screen
        // that answered `403`, or a 3DS challenge that answered `404`, rendered as *nothing at all*
        // inside an otherwise-working page — the failure shape this project calls silent.
        if resp.status >= 400 {
            tracing::info!(status = resp.status, %url, "error status — rendering its body, as a browser does");
        }
        // WHATWG charset sniff (D4) instead of lossy UTF-8 — and KEEP the name it sniffed.
        let (text, enc) = resp.decoded_text_named();
        Ok((
            text,
            resp.final_url.to_string(),
            resp.headers.clone(),
            enc.to_string(),
        ))
    } else if let Some(rest) = url.strip_prefix("data:") {
        Ok((
            decode_data_url(rest)?,
            url.to_string(),
            Vec::new(),
            "UTF-8".to_string(),
        ))
    } else {
        let path = url.strip_prefix("file://").unwrap_or(url);
        let html =
            std::fs::read_to_string(path).with_context(|| format!("reading local file {path}"))?;
        Ok((html, url.to_string(), Vec::new(), "UTF-8".to_string()))
    }
}

/// **May `embedder` frame a document served with these headers?**
///
/// Two mechanisms answer one question, and the spec is explicit that they are not equals: when CSP
/// `frame-ancestors` is present it **overrides `X-Frame-Options` entirely** (CSP3 §7.4.1), including
/// overriding a `DENY`. Checking both and taking the stricter answer is the intuitive
/// implementation and it is wrong — a site migrating from the legacy header to the modern directive
/// deliberately leaves a stale `X-Frame-Options: DENY` behind, and honouring it would break the
/// framing the site has just decided to allow.
///
/// Returns `true` when framing is permitted (which is the answer for the overwhelming majority of
/// responses, since neither header is present).
fn framing_allowed(headers: &[(String, String)], embedder: Option<&Url>, framed: &Url) -> bool {
    let get = |name: &str| {
        headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    };
    let same_origin = |a: &Url, b: &Url| {
        a.scheme() == b.scheme()
            && a.host_str() == b.host_str()
            && a.port_or_known_default() == b.port_or_known_default()
    };

    // ── CSP `frame-ancestors` first, and if it is present nothing else is consulted.
    let csp_directive = get("content-security-policy").and_then(|csp| {
        csp.split(';')
            .map(str::trim)
            .find(|d| d.to_ascii_lowercase().starts_with("frame-ancestors"))
            .map(|d| d["frame-ancestors".len()..].trim().to_ascii_lowercase())
    });
    if let Some(sources) = csp_directive {
        let Some(embedder) = embedder else {
            return true; // a top-level document is not framed at all
        };
        return sources.split_whitespace().any(|src| match src {
            "'none'" => false,
            "*" => true,
            "'self'" => same_origin(embedder, framed),
            // A host source: `https://example.com`, `example.com`, `https://*.example.com`. Anything
            // this cannot parse is treated as NOT matching — for `frame-ancestors` that is the
            // direction the page asked for ("only these may frame me"), so an unrecognised source
            // must not become a wildcard.
            other => {
                let other = other.trim_matches('\'');
                let (scheme, host) = match other.split_once("://") {
                    Some((s, h)) => (Some(s), h),
                    None => (None, other),
                };
                if let Some(s) = scheme {
                    if s != embedder.scheme() {
                        return false;
                    }
                }
                let host = host.split('/').next().unwrap_or(host);
                let host = host.split(':').next().unwrap_or(host);
                let Some(eh) = embedder.host_str() else {
                    return false;
                };
                match host.strip_prefix("*.") {
                    Some(suffix) => eh.ends_with(suffix) && eh.len() > suffix.len(),
                    None => eh.eq_ignore_ascii_case(host),
                }
            }
        });
    }

    // ── Legacy `X-Frame-Options`, consulted only when there is no `frame-ancestors`.
    match get("x-frame-options").map(|v| v.trim().to_ascii_lowercase()) {
        Some(v) if v == "deny" => false,
        Some(v) if v == "sameorigin" => match embedder {
            None => true,
            Some(e) => same_origin(e, framed),
        },
        // `ALLOW-FROM` was removed from every engine (it could not express the origin the
        // navigation actually came from) and an unrecognised value is ignored, per the HTML spec's
        // handling of a header it does not understand — NOT treated as `DENY`, which would block
        // frames on the strength of a typo.
        _ => true,
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

/// Recursively pin `position:sticky` boxes against **their own scrollport**, recording every shift
/// it makes into `applied` so the exact same translation can later be UNDONE.
///
/// Each sticky child is shifted (with its subtree) so it stays at its `top`/`bottom` threshold
/// inside the scrollport, bounded by its containing block (its parent box). Non-sticky boxes are
/// untouched.
///
/// ## The scrollport is the NEAREST one, not always the viewport (tick 1282)
///
/// `sport_top`/`sport_h` describe the sticky view rectangle in **the box tree's coordinate space**,
/// and the walk REPLACES them with the padding box of any `overflow: auto|scroll|hidden` box it
/// descends into. CSS Position §6.3 resolves a sticky box against its nearest scrollport, and a
/// sticky table header inside an `overflow:auto` pane — the single most common sticky construct
/// after the page header — has nothing to do with the document viewport. This used to resolve
/// *every* sticky box against the document, so such a header was pinned to a rectangle it is not
/// inside: `css/css-position/sticky` reported `start of scroll container: expected 20 but got 200`,
/// which is the in-flow position, unshifted, because the document-relative constraint never fired.
///
/// ⚠ **The container's own box is NOT translated by its scroll offset — its children are**
/// (`Page::set_element_scroll`). So the container's padding box and the already-scrolled children
/// are in one space, and no scroll term appears here: `sport_top` IS the padding-box top.
///
/// ## Why `applied` is an output rather than a side effect
///
/// The shift is baked into the live `root_box`, which every DOM reader (`getBoundingClientRect`,
/// `offsetTop`, hit-testing, the a11y tree, the fidelity oracle) sees. Recomputing on the next
/// scroll must therefore start from the *unshifted* tree — and a translation is exactly invertible,
/// so recording `dy` per node is a ledger that cannot drift, where a re-derivation could.
fn apply_sticky(
    b: &mut LayoutBox,
    styles: &StyleMap,
    sport_top: f32,
    sport_h: f32,
    applied: &mut std::collections::HashMap<manuk_dom::NodeId, f32>,
) {
    use manuk_layout::BoxContent;
    let (cb_top, cb_bottom) = (b.rect.y, b.rect.y + b.rect.height);
    let cb_width = b.rect.width;
    if let BoxContent::Block(children) = &mut b.content {
        for child in children.iter_mut() {
            if let Some(node) = child.node {
                if let Some(s) = styles.get(&node) {
                    // ⚠ `Option`, not a `0.0` default. `bottom: auto` means *do not pin to the
                    // bottom*; a zero there would pin every top-sticky box to the viewport bottom
                    // too. The old code asked only `!s.inset.top.is_auto()`, so a
                    // `position:sticky; bottom:0` footer never entered this branch at all and was
                    // indistinguishable from `position:static`.
                    if s.position == manuk_css::Position::Sticky {
                        let top =
                            (!s.inset.top.is_auto()).then(|| s.inset.top.resolve(cb_width, 0.0));
                        let bottom = (!s.inset.bottom.is_auto())
                            .then(|| s.inset.bottom.resolve(cb_width, 0.0));
                        if top.is_some() || bottom.is_some() {
                            let shift = manuk_layout::sticky_shift(
                                child.rect.y,
                                child.rect.height,
                                top,
                                bottom,
                                cb_top,
                                cb_bottom,
                                sport_top,
                                sport_h,
                            );
                            if shift != 0.0 {
                                child.shift_y(shift);
                                *applied.entry(node).or_insert(0.0) += shift;
                            }
                        }
                    }
                }
            }
            // A scroll container becomes the scrollport for everything beneath it.
            let (ct, ch) = child
                .node
                .and_then(|n| styles.get(&n))
                .filter(|s| {
                    matches!(
                        s.overflow,
                        manuk_css::Overflow::Auto
                            | manuk_css::Overflow::Scroll
                            | manuk_css::Overflow::Hidden
                    )
                })
                .map(|s| {
                    let bw = &s.border_width;
                    (
                        child.rect.y + bw.top,
                        (child.rect.height - bw.top - bw.bottom).max(0.0),
                    )
                })
                .unwrap_or((sport_top, sport_h));
            apply_sticky(child, styles, ct, ch, applied);
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

        // (4) Boot-time window/screen metrics exist (SPAs read these or throw at load) — and
        //     report the width the page is ACTUALLY laid out at. This asserted a hardcoded
        //     `1280x720` while the page was loaded at 800px, i.e. it asserted the disagreement:
        //     an SPA sizing a canvas, a virtualised list or a chart off `innerWidth` drew it for a
        //     viewport the user does not have. The width is threaded through the assertion now, so
        //     re-hardcoding the prelude fails it.
        let html4 = r#"<!doctype html><html><body id="b"><script>
            document.getElementById('b').setAttribute('data-m',
                window.innerWidth + 'x' + screen.height + 'x' + devicePixelRatio +
                ':' + (typeof matchMedia) + ':' + (typeof requestAnimationFrame));
            </script></body></html>"#;
        let vw4 = 900.0_f32;
        let page4 = Page::load(html4, "https://app.test/", &fonts, vw4);
        let b = manuk_css::query_selector_all(page4.dom(), page4.dom().root(), "#b")[0];
        assert_eq!(
            page4.dom().element(b).and_then(|e| e.attr("data-m")),
            Some(format!("{vw4}x720x1:function:function").as_str()),
            "window/screen/devicePixelRatio/matchMedia/rAF present at load, and innerWidth is the \
             width the page was laid out at — not a constant"
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
        // **This assertion used to read `origin == "https://auth.test"` — "targetOrigin preserved" —
        // and it was pinning a security bug** (corrected tick 231).
        //
        // The slot is not a scratch field: `gui.rs::pump_messages` passes it straight into
        // `deliver_message`, where it becomes the receiver's `e.origin`. So carrying the sender's own
        // `targetOrigin` ARGUMENT there let any page forge its identity — every popup-login SDK
        // guards with `if (e.origin !== PROVIDER) return;`, and `postMessage(payload, PROVIDER)`
        // walked straight through it, because the receiver has no other way to learn who sent a
        // message.
        //
        // `e.origin` is the SENDER's origin, per spec. This page is `https://app.test`, so that is
        // what the receiver must see — regardless of which target the sender addressed.
        assert_eq!(
            origin, "https://app.test",
            "e.origin must be the SENDER's origin, not the caller-supplied targetOrigin"
        );
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
        // Loaded at 1280 — this said "at 1280px wide" in its own message while loading at 800,
        // which is only consistent if matchMedia ignores the real viewport. It must not.
        let page12 = Page::load(html12, "https://app.test/", &fonts, 1280.0);
        let mm = manuk_css::query_selector_all(page12.dom(), page12.dom().root(), "#mm")[0];
        assert_eq!(
            page12.dom().element(mm).and_then(|e| e.attr("data-r")),
            Some("false,true,true"),
            "matchMedia: not-narrow, is-wide, and in-range at 1280px wide"
        );

        // (15) **Media: the honest answer, and it CHANGED when playback landed.**
        //
        // This block used to assert that `canPlayType` returns `''` for everything and `play()`
        // REJECTS. Both were exactly right while nothing could decode. Tick 263 wired the shell's
        // media drive, so Constrained-Baseline H.264 in MP4 now genuinely fetches, decodes and
        // plays — and at that moment the old assertions started pinning a LIE in place. A site that
        // politely feature-detects was being told no about something that works, and would hide its
        // `<video>` behind a "your browser cannot play this" fallback.
        //
        // So the vocabulary is the same and the answers moved: `'probably'` when the codecs are
        // NAMED and we have them, `'maybe'` for a container we read with no codec string (it cannot
        // be promised — mp4 carries HEVC and High-profile H.264 too, and neither decodes here),
        // `''` for everything we can name and cannot play. `play()` resolves and flips `paused`.
        let html15 = r#"<!doctype html><html><body>
            <video id="v" width="640" height="360" poster="p.png" controls>
              <source src="m.mp4" type="video/mp4">
            </video>
            <div id="out">-</div>
            <script>
              var v = document.getElementById('v'), r = [];
              // The three answers, and the distinction between them is the whole point.
              r.push('named:' + (v.canPlayType('video/mp4; codecs="avc1.42E01E, mp4a.40.2"') === 'probably'));
              r.push('bare:' + (v.canPlayType('video/mp4') === 'maybe'));
              r.push('nowebm:' + (v.canPlayType('video/webm; codecs="vp9"') === ''));
              r.push('nohigh:' + (v.canPlayType('video/mp4; codecs="avc1.640028"') === ''));
              r.push('state:' + (v.paused === true && v.readyState === 0 && v.networkState === 3));
              // `error` is spec-initial NULL: no load has been ATTEMPTED here, so there is nothing
              // to report. It used to be an eager MediaError(4), which contradicted canPlayType
              // saying 'probably' and made every player give up on video that works.
              r.push('noerr:' + (v.error === null));
              // ...and the HOST's verdict is what fills it in, in BOTH directions.
              v.__setOutcome(false);
              r.push('failed:' + (v.error !== null && v.error.code === 4 && v.readyState === 0));
              v.__setOutcome(true);
              r.push('ok:' + (v.error === null && v.readyState === 4 && v.networkState === 1));
              r.push('iface:' + (v instanceof HTMLMediaElement));
              // Setters must not throw. Scripts assign these unconditionally.
              v.pause(); v.currentTime = 5; v.volume = 0.5; v.load();
              r.push('setters:' + (v.currentTime === 5));
              // play() must RESOLVE and flip `paused` — a rejection now sends every player into
              // its own catch branch while the video plays behind it.
              var p = v.play();
              r.push('promise:' + (p && typeof p.then === 'function'));
              p.then(function(){
                 r.push('resolved:true');
                 r.push('playing:' + (v.paused === false));
                 v.pause();
                 r.push('repaused:' + (v.paused === true));
                 document.getElementById('out').textContent = r.join(' ');
               })
               .catch(function(e){ document.getElementById('out').textContent = 'PLAY REJECTED (a lie now)'; });
            </script></body></html>"#;
        let page15 = Page::load(html15, "https://app.test/", &fonts, 800.0);
        let root15 = page15.dom().root();
        let out15 = manuk_css::query_selector_all(page15.dom(), root15, "#out")[0];
        let got15 = page15.dom().text_content(out15);
        for claim in [
            "named:true",    // named Baseline codecs => 'probably' — what we genuinely decode
            "bare:true",     // bare container => 'maybe' — readable, but cannot be promised
            "nowebm:true",   // a codec we do not have => '' , even in a container we read
            "nohigh:true",   // High-profile H.264 => '' — openh264 is Constrained Baseline only
            "state:true",    // paused / HAVE_NOTHING / NETWORK_NO_SOURCE
            "noerr:true",    // spec-initial null — no load attempted, nothing to report
            "failed:true",   // the host reports a failed decode => MediaError code 4
            "ok:true",       // ...and a successful one clears it => HAVE_ENOUGH_DATA / IDLE
            "iface:true",    // instanceof HTMLMediaElement
            "setters:true",  // currentTime/volume/pause/load do not throw
            "promise:true",  // play() returns a thenable
            "resolved:true", // ...and it RESOLVES, because playback now happens
            "playing:true",  // ...and flips `paused` (a getter-only prop would silently not)
            "repaused:true", // pause() flips it back
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

    /// # G_CONTAINER_REPASS_KEEPS_NATURAL_SIZE — `@container` must not erase an image's intrinsic size
    ///
    /// `restyle_and_layout` states the rule eight lines above its own call site: *"BETWEEN the cascade
    /// and the layout, EVERY TIME. A cascade that rebuilds the style map erases it, and the picture
    /// becomes a full-width strip of zero height."* `container_query_recascade` rebuilds the style map
    /// **wholesale** (`*styles = new_styles`) — and every one of its six callers applied
    /// `apply_natural_sizes` BEFORE it and none re-applied it after. So on any page whose CSS merely
    /// contains the string `@container`, every decoded image's natural size was restated and then
    /// immediately thrown away.
    ///
    /// Found by the `LAYOUT PHASES` ledger (t1258): profiling `bhramarah.in` — a Shopify storefront in
    /// the timeout cohort, whose `base.css` and `styles.css` both carry real `@container` rules —
    /// showed `container_query_ms=7,329` against a `cascade_ms=4,866`, which is what put the re-pass
    /// under a microscope in the first place. The defect is a correctness one and was sitting in the
    /// path the whole time.
    ///
    /// ⚠ **THE `@container`-FREE ROW IS THE CONTROL, AND IT IS WHAT MAKES THIS A STATEMENT ABOUT THE
    /// RE-PASS.** Without it a fix that simply applied natural sizes twice everywhere would pass, and
    /// so would one that never ran the re-pass at all. The control proves the sizes were already
    /// correct on the ordinary path — the defect is specifically what the second cascade destroys.
    ///
    /// **How it goes RED:** delete the `apply_natural_sizes(dom, styles, images)` call inside
    /// `container_query_recascade`. The `@container` row reports 0x0 (or the CSS-declared size) and
    /// the control row keeps passing.
    #[test]
    fn container_repass_keeps_a_decoded_images_natural_size() {
        // 40x20 is deliberately not square and not a round CSS default, so nothing else can produce it.
        let img = std::rc::Rc::new(manuk_paint::DecodedImage {
            width: 40,
            height: 20,
            rgba: vec![0; 40 * 20 * 4],
        });
        let fonts = manuk_text::FontContext::new();

        // (label, css) — the same document, one sheet carrying `@container` and one not.
        let cases: [(&str, &str); 2] = [
            (
                "@container",
                "#box{container-type:inline-size}@container (min-width:1px){#i{opacity:0.5}}",
            ),
            ("CONTROL (no @container)", "#box{width:300px}"),
        ];

        for (label, css) in cases {
            let dom = manuk_html::parse("<div id=box><img id=i src='x.png'></div>");
            let node = dom
                .descendants(dom.root())
                .find(|&n| dom.tag_name(n) == Some("img"))
                .expect("the fixture has an <img>");
            let mut images = std::collections::HashMap::new();
            images.insert(node, img.clone());

            let sheets = vec![Stylesheet::parse(css)];
            let (_styles, root) = restyle_and_layout(&dom, &sheets, &fonts, 800.0, &images);

            // ⚠ Asserted on the LAID-OUT BOX, not on the style. `fill_natural_size` sets `width` plus
            // an `aspect_ratio` and deliberately leaves `height: auto` for layout to derive, so a
            // style-level assertion on `height` would fail against correct code — it did, on the first
            // cut of this gate. The rendered rect is also the thing the defect is actually about.
            let rect = root
                .node_rects(&dom)
                .into_iter()
                .find(|(n, _)| *n == node)
                .map(|(_, r)| r)
                .expect("the <img> produced a box");
            assert!(
                (rect.width - 40.0).abs() < 0.5 && (rect.height - 20.0).abs() < 0.5,
                "{label}: the decoded image must lay out at its 40x20 natural size — got {}x{}. \
                 A rebuilt cascade carries no intrinsic size, so the picture becomes a strip of \
                 zero height.",
                rect.width,
                rect.height
            );
        }
    }

    /// **A COMMA IS A LEGAL URL CHARACTER, AND SPLITTING `srcset` ON COMMAS SHREDS ONE SITE IN TEN
    /// THAT USES THE ATTRIBUTE** (t1046).
    ///
    /// The parser this replaces split on `,` and justified it in its own comment: *"a comma inside a
    /// URL is vanishingly rare compared to a missing space after one."* **Measured against the
    /// burndown corpus that premise is false** — 40 of 170 pages ship a `srcset` and **4 of them
    /// carry a comma inside a candidate URL** (`www.kuechenmomente.de`, `www.kroftools.com`,
    /// `xhdesign.today`, `www.timeline.com`). Two ordinary shapes produce it: an image CDN's
    /// transform segment (`/upload/w_400,h_300,c_fill/hero.jpg`) and **every `data:` URI in
    /// existence**. A shredded candidate is not a smaller image: the fragments are not URLs, the
    /// list yields nothing, and the element renders a broken-image placeholder.
    ///
    /// HTML's scan is two-phase, and that is exactly what disambiguates the comma: **the URL runs to
    /// the next WHITESPACE** (an interior comma is part of it) and only a *trailing* comma ends the
    /// candidate; **the descriptor then runs to the next COMMA**.
    ///
    /// **How it goes RED:** restore `for part in srcset.split(',')` — rows 3, 4 and 5 collapse (the
    /// CDN and `data:` URLs split into non-URL fragments, and the `data:` row yields two bogus
    /// candidates instead of one real one). Rows 1, 2, 6 and 7 are the CONTROLS: they are what the
    /// old parser got right, and a fix that broke ordinary lists to rescue the comma case would be
    /// a trade, not a fix.
    #[test]
    fn srcset_candidates_survive_a_comma_inside_a_url() {
        let cases: &[(&str, &[(&str, Option<f32>, f32)])] = &[
            // ── CONTROLS: what the old parser already got right and must keep getting right.
            ("a.png", &[("a.png", None, 1.0)]),
            (
                "a.png 1x, b.png 2x",
                &[("a.png", None, 1.0), ("b.png", None, 2.0)],
            ),
            // ── THE DEFECT: a comma inside the URL.
            (
                "/up/w_400,h_300,c_fill/h.jpg 400w, /up/w_800,h_600,c_fill/h.jpg 800w",
                &[
                    ("/up/w_400,h_300,c_fill/h.jpg", Some(400.0), 1.0),
                    ("/up/w_800,h_600,c_fill/h.jpg", Some(800.0), 1.0),
                ],
            ),
            (
                "/up/w_400,h_300/h.jpg",
                &[("/up/w_400,h_300/h.jpg", None, 1.0)],
            ),
            (
                "data:image/png;base64,iVBORw0K 1x, b.png 2x",
                &[
                    ("data:image/png;base64,iVBORw0K", None, 1.0),
                    ("b.png", None, 2.0),
                ],
            ),
            // ── CONTROL: a trailing comma still ends a candidate with no descriptor, and a URL
            //    with an INTERIOR comma and no whitespace stays ONE url (Chrome agrees).
            (
                "a.png, b.png",
                &[("a.png", None, 1.0), ("b.png", None, 1.0)],
            ),
            ("a.png,b.png", &[("a.png,b.png", None, 1.0)]),
        ];
        for (input, want) in cases {
            let got = parse_srcset(input);
            let got: Vec<(&str, Option<f32>, f32)> =
                got.iter().map(|c| (c.url, c.width, c.density)).collect();
            let want: Vec<(&str, Option<f32>, f32)> = want.to_vec();
            assert_eq!(
                got, want,
                "srcset {input:?}\n  A comma is a legal URL character. HTML scans the URL to the \
                 next WHITESPACE and the descriptor to the next COMMA; splitting on commas turns \
                 one CDN or `data:` candidate into fragments that are not URLs, and the element \
                 renders a broken-image placeholder rather than a wrong-sized one."
            );
        }
    }

    #[test]
    fn static_import_scanner_finds_specifiers_and_skips_the_rest() {
        // Every static import/export-from form yields its specifier…
        let src = r#"
            import a from './a.js';
            import { b } from "./b.js";
            import * as c from './c.js';
            import d, { e } from './d.js';
            import './side-effect.js';
            export { x } from './x.js';
            export * from './y.js';
            export * as ns from './z.js';
        "#;
        let got = scan_static_import_specifiers(src);
        for want in [
            "./a.js",
            "./b.js",
            "./c.js",
            "./d.js",
            "./side-effect.js",
            "./x.js",
            "./y.js",
            "./z.js",
        ] {
            assert!(got.contains(&want.to_string()), "missing {want} in {got:?}");
        }

        // …minified (no spaces) still resolves via the `from`/`import` adjacency…
        assert_eq!(
            scan_static_import_specifiers(r#"import{a}from"./m.js";import"./s.js""#),
            vec!["./m.js".to_string(), "./s.js".to_string()]
        );

        // ── …and the non-import forms are NOT mistaken for specifiers.
        //
        // ⚠⚠⚠ **THIS WAS ONE `is_empty()` OVER FIVE SEPARATE RULES, AND IT WENT PERMANENTLY RED
        // WHEN EXACTLY ONE OF THEM WAS DELIBERATELY CHANGED.** t624 made a *literal* `import("m")`
        // specifier collectable on purpose — `module_dynamic_import_hook` resolves from
        // `MODULE_GRAPH_SOURCES`, the same pre-fetched map (`js/src/dom_bindings.rs:12374`), so a
        // specifier missing from it is one `import()` cannot satisfy. The change was right and its
        // reasoning is written next to it; this assertion was never updated, and from then on the
        // test said nothing true about the other four rules either. **A lumped assertion does not
        // fail loudly when one clause is superseded — it fails permanently, and a permanent red is
        // read as background noise.**
        //
        // So the five rules are now five assertions, each independently falsifiable, and the
        // dynamic case asserts the behaviour we actually want rather than the one we used to.
        let scanned = scan_static_import_specifiers(
            r#"
                // import x from './comment.js';
                /* import y from './block.js'; */
                const s = "import z from './string.js'";
                const p = import('./dynamic.js');
                const u = import.meta.url;
                "#,
        );
        for (bad, why) in [
            (
                "./comment.js",
                "a `from` inside a LINE comment is not a keyword",
            ),
            (
                "./block.js",
                "a `from` inside a BLOCK comment is not a keyword",
            ),
            (
                "./string.js",
                "an `import … from` inside a STRING LITERAL is not code",
            ),
        ] {
            assert!(
                !scanned.contains(&bad.to_string()),
                "static import scanner: {why} — got {scanned:?}"
            );
        }
        // `import.meta` must not fire the bare side-effect-import rule: the `.` becomes the previous
        // token, so the scanner never sees `import` adjacent to a string. Nothing there is a string
        // at all, so the check is that the scan produced EXACTLY one entry and it is the dynamic one.
        assert_eq!(
            scanned,
            vec!["./dynamic.js".to_string()],
            "static import scanner: a LITERAL `import('./dynamic.js')` IS collected on purpose \
             (t624) — the dynamic-import host hook resolves from the same pre-fetched module map, \
             so a specifier missing from it is one `import()` cannot satisfy, and over-collecting \
             costs one request while under-collecting costs the feature. A COMPUTED specifier is \
             invisible to any textual scan and still rejects at runtime. Everything else in the \
             fixture — two comments, a string, and `import.meta` — must contribute nothing."
        );
    }

    /// ⚠⚠⚠ **EVERY ASSERTION HERE READS `Page::node_rects` — THE MAP `getBoundingClientRect`
    /// ANSWERS FROM — AND NOT A PAINT-TIME CLONE.** That is the whole of tick 1282. The shift used
    /// to be computed inside `paint_scrolled` on a throwaway copy of the box tree, so the pixels
    /// moved and the DOM never learned: a stuck header reported its in-flow position, scrolled or
    /// not, to `getBoundingClientRect`, `offsetTop`, hit-testing, the a11y tree, the fidelity
    /// oracle, and to every scroll-spy on the web.
    ///
    /// **To watch it go RED:** delete the `self.restick(scroll_y)` from `view_changed`, or the
    /// `page.restick(0.0)` at the end of the constructor. Either leaves these rects at the in-flow
    /// position, which is what the pinned/stuck assertions reject. Dropping the `bottom` arm from
    /// `manuk_layout::sticky_shift` (t1281) still fails the footer rows.
    #[test]
    fn sticky_pins_in_the_box_tree_the_dom_reads() {
        // The scrollport height the constructor and `view_changed` resolve against — the SAME
        // viewport the cascade resolves `vh` against, read rather than assumed so this test cannot
        // disagree with the engine about how tall the screen is.
        let (_, vh) = manuk_css::values::viewport_size();
        let html = r#"<body style="margin:0">
            <div id="h" style="position:sticky;top:0;height:40px">Header</div>
            <div style="height:2000px">tall content</div>
        </body>"#;
        let fonts = FontContext::new();
        let mut page = Page::load(html, "https://x.test/", &fonts, 400.0);
        assert!(page.has_sticky, "sticky is detected");

        let hid = manuk_css::query_selector_all(page.dom(), page.dom().root(), "#h")[0];
        let natural_y = page.node_rects()[&hid].y;

        // Scrolled 500px past the top: the header pins so it stays at the viewport top (top:0),
        // i.e. its document y rises to ~scroll_y — AND THE DOM SAYS SO.
        page.view_changed(500.0, 400.0, vh, true);
        let pinned = page.node_rects()[&hid].y;
        assert!(
            (pinned - (natural_y + 500.0)).abs() < 1.5,
            "sticky header pinned to the scroll offset IN node_rects (natural {natural_y}, got \
             {pinned}) — this is the number getBoundingClientRect returns"
        );
        // CONTROL: scrolled back to the top it must return to its in-flow position exactly. A
        // one-way shift that never releases would pass the row above and is not sticky; it is also
        // what an un-invertible bake would produce, so this row guards the ledger.
        page.view_changed(0.0, 400.0, vh, true);
        let back = page.node_rects()[&hid].y;
        assert!(
            (back - natural_y).abs() < 0.01,
            "scrolling back releases the header to its EXACT in-flow y (natural {natural_y}, got \
             {back}) — a compounding bake drifts here"
        );

        // ⚠⚠⚠ **THE BOTTOM EDGE** (t1281): `position: sticky; bottom: 0` is the sticky footer, the
        // bottom action bar and the "continue" button every checkout flow pins to the bottom.
        //
        // The footer sits below 2000px of filler, so at scroll 0 it is far below the fold and must
        // already be pulled UP onto the viewport's bottom edge: `vh − 0 − 40`. **A sticky box can be
        // stuck at scroll 0**, which is why the constructor re-sticks rather than waiting for a
        // scroll that may never come.
        let html2 = r#"<body style="margin:0">
            <div style="height:2000px">tall content</div>
            <div id="f" style="position:sticky;bottom:0;height:40px">Footer</div>
        </body>"#;
        let mut page2 = Page::load(html2, "https://x.test/", &fonts, 400.0);
        assert!(
            page2.has_sticky,
            "sticky is detected for a bottom-only inset"
        );
        let fid = manuk_css::query_selector_all(page2.dom(), page2.dom().root(), "#f")[0];
        let stuck = page2.node_rects()[&fid].y;
        assert!(
            (stuck - (vh - 40.0)).abs() < 1.5,
            "a `bottom:0` sticky footer rides the viewport's bottom edge WITHOUT ANY SCROLL (want \
             {}, got {stuck})",
            vh - 40.0
        );
        // CONTROL: scrolled far enough that the footer's own in-flow position is on screen, it must
        // sit exactly there and not be dragged anywhere. A box that always moves is not sticky, it
        // is fixed — and this row is what tells the two apart.
        page2.view_changed(2000.0, 400.0, vh, true);
        let released = page2.node_rects()[&fid].y;
        assert!(
            (released - 2000.0).abs() < 1.5,
            "once its in-flow position is inside the viewport the footer stops sticking (want 2000, \
             got {released})"
        );
    }

    /// ⭐ **A SCROLL CONTAINER IS ITS OWN SCROLLPORT** — the sticky table header inside an
    /// `overflow:auto` pane, which is the second-most-common sticky construct on the web and the
    /// one `css/css-position/sticky` is mostly made of.
    ///
    /// Before tick 1282 every sticky box was resolved against the **document** viewport, so a header
    /// inside a 200px-tall scrolling pane was measured against a constraint it is nowhere near and
    /// simply scrolled away with its pane: the suite read `start of scroll container: expected 20
    /// but got 200` — the in-flow position, unmoved.
    ///
    /// **To watch it go RED:** pass `sport_top, sport_h` down `apply_sticky`'s recursion instead of
    /// the container's padding box, and the header lands at 300 instead of 400.
    ///
    /// ⚠⚠⚠ **THE FIRST VERSION OF THIS FIXTURE WAS VACUOUS AND THE FALSIFICATION CAUGHT IT.** It
    /// put the pane at document y 0, where the pane's padding-box top and the document scrollport
    /// top are BOTH 0 — so the right answer and the wrong one coincide and the mutation above stayed
    /// green. **The configuration that already works is not evidence about the ones that do not.**
    /// The 400px spacer below is the whole difference between a test and a decoration, and the
    /// `<section>` wrapper is the second half of it: with the pane itself as the containing block,
    /// the §6.3 containing-block clamp pins the header on its own and the sticky constraint is never
    /// consulted — the assertion would have been measuring the clamp.
    #[test]
    fn sticky_resolves_against_its_own_scroll_container() {
        let html = r#"<body style="margin:0">
            <div style="height:400px">spacer — the pane is NOT at the document origin</div>
            <div id="pane" style="overflow:auto;height:200px">
              <div id="sec" style="height:600px">
                <div id="hdr" style="position:sticky;top:0;height:30px">Header</div>
                <div id="rows" style="height:500px">rows</div>
              </div>
            </div>
        </body>"#;
        let fonts = FontContext::new();
        let mut page = Page::load(html, "https://x.test/", &fonts, 400.0);
        let pane = manuk_css::query_selector_all(page.dom(), page.dom().root(), "#pane")[0];
        let hdr = manuk_css::query_selector_all(page.dom(), page.dom().root(), "#hdr")[0];
        let rows = manuk_css::query_selector_all(page.dom(), page.dom().root(), "#rows")[0];

        let pane_top = page.node_rects()[&pane].y;
        assert!(
            (pane_top - 400.0).abs() < 1.5,
            "the pane starts 400px down the document, so its scrollport and the document's cannot \
             be confused (got {pane_top})"
        );
        assert!(
            (page.node_rects()[&hdr].y - pane_top).abs() < 1.5,
            "unscrolled, the header is at the top of its pane"
        );

        // Scroll the PANE, not the document. `set_element_scroll` translates the pane's children in
        // the live box tree — so without the sticky re-bake the header rides down with them.
        page.set_element_scroll(pane, 0.0, 100.0);
        let after = page.node_rects()[&hdr].y;
        assert!(
            (after - pane_top).abs() < 1.5,
            "the header stays pinned to the PANE's top edge after the pane scrolls 100px (pane \
             {pane_top}, got {after}) — resolved against the DOCUMENT scrollport instead it lands \
             at 300, riding down with the content"
        );
        // CONTROL: the non-sticky sibling in the same pane must have moved the full 100px. Without
        // this the row above passes for a pane that never scrolled at all.
        let rows_y = page.node_rects()[&rows].y;
        assert!(
            (rows_y - (pane_top + 30.0 - 100.0)).abs() < 1.5,
            "the pane's non-sticky content really did scroll 100px up (want {}, got {rows_y})",
            pane_top + 30.0 - 100.0
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

    /// **I3 — THE AGENT'S CLICK POINT IS LAYOUT GEOMETRY, SO A CONTAINING-BLOCK BUG IS AN
    /// ACTUATION BUG.** Every row of a drawer must expose a DISTINCT click point for its own
    /// control, and each control's point must fall inside the row it belongs to.
    ///
    /// Written because constitution check #71 asked whether t841/t843's `reading_order` wins carried
    /// their semantic-model exposure, and answering it corrected the question. The agent does **not**
    /// order by geometry — `A11yNode::iter()` is DOM pre-order, which is already the correct reading
    /// order for RTL — so neither tick changed the agent's ORDER. What they changed is the
    /// **coordinates**: `to_viewport_lines` emits `role "name" @(cx,cy)` straight from the layout
    /// rect, and before t843 all fourteen carets of an AdminLTE sidebar reported the *same* centre,
    /// because `top:50%` resolved against the drawer instead of the row. An agent told to expand the
    /// seventh menu item would have clicked the same pixel as the first — a *silent mis-actuation*,
    /// not a visual blemish, and nothing in the agent or a11y crates could have caught it.
    ///
    /// The layout half is gated in `manuk-layout`
    /// (`a_relative_ancestor_inside_an_out_of_flow_subtree_is_a_containing_block`). This is the
    /// half I3 actually asks for: the assertion runs on the SEMANTIC surface, through the real
    /// pipeline, in the units an agent acts in.
    ///
    /// To watch it go RED, revert the `rects.extend(...)` in `manuk_layout::position_absolutes`:
    /// all three carets collapse onto one centre and both assertions fail.
    #[test]
    fn every_drawer_row_exposes_its_own_click_point_to_the_agent() {
        let fonts = FontContext::new();
        let html = r##"<!DOCTYPE html><title>T</title><style>
            body{margin:0}
            .drawer{position:absolute;top:0;left:0;width:230px}
            ul{list-style:none;margin:0;padding:0}
            .drawer li>div.row{position:relative;height:44px}
            a.caret{position:absolute;right:10px;top:50%;margin-top:-7px;width:10px;height:14px}
            </style><body>
            <div class="drawer"><ul>
              <li><div class="row">One<a class="caret" href="#">Expand one</a></div></li>
              <li><div class="row">Two<a class="caret" href="#">Expand two</a></div></li>
              <li><div class="row">Three<a class="caret" href="#">Expand three</a></div></li>
            </ul></div></body>"##;
        let page = Page::load(html, "http://example.test/", &fonts, 1200.0);
        let tree = page.a11y_tree();
        let point = |name: &str| -> (f32, f32) {
            tree.find(&manuk_a11y::Role::Link, name)
                .unwrap_or_else(|| panic!("{name} is in the a11y tree"))
                .bbox
                .unwrap_or_else(|| panic!("{name} was laid out, so it has geometry"))
                .center()
        };
        let carets = [
            point("Expand one"),
            point("Expand two"),
            point("Expand three"),
        ];
        // (1) THREE ROWS, THREE CLICK POINTS. A shared centre is the signature of every caret
        //     resolving against the drawer instead of its own row.
        for (i, a) in carets.iter().enumerate() {
            for (j, b) in carets.iter().enumerate().skip(i + 1) {
                assert!(
                    (a.1 - b.1).abs() > 1.0,
                    "carets {i} and {j} share a click point ({:?} vs {:?}) — an agent told to \
                     expand one row would actuate another",
                    a,
                    b
                );
            }
        }
        // (2) AND EACH POINT IS INSIDE ITS OWN ROW. Distinctness alone would pass if every caret
        //     were merely displaced by a different wrong amount.
        // The rows are 44px tall and stacked from the drawer's origin, so row N spans
        // `44N .. 44N+44`. Asserted from the CSS rather than from a lookup, because the rows are
        // generic `<div>`s and the a11y tree does not surface those by name.
        for (i, caret) in carets.iter().enumerate() {
            let (top, bottom) = (44.0 * i as f32, 44.0 * i as f32 + 44.0);
            let caret = *caret;
            let rb = manuk_a11y::Rect {
                x: 0.0,
                y: top,
                width: 230.0,
                height: bottom - top,
            };
            let row = i;
            assert!(
                caret.1 >= rb.y && caret.1 <= rb.y + rb.height,
                "the caret for row {row} clicks at y={} and the row spans {}..{} — the agent's \
                 click point must land in the row it names",
                caret.1,
                rb.y,
                rb.y + rb.height
            );
        }
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
    /// **A render-blocking stylesheet the load deadline CUT OFF must be COUNTED, not vanish.**
    ///
    /// `collect_before_deadline` returns only the futures that finished, so a cancelled sheet never
    /// reaches the `for (url, text) in fetched` loop that owns both the logging and the `failed_css`
    /// bookkeeping. The page then renders in UA-default fallback and the engine reports nothing —
    /// measured on `serverfault.com` (t707), whose entire stderr for such a load is one line, and
    /// which the fidelity sweep books as `render-failed` ("our own bug") with no way to learn that
    /// the real fault was an unstyled document. 21 of 265 corpus sites land in that bucket.
    ///
    /// The deadline is set in the PAST so the cut is deterministic and needs no network: every
    /// requested sheet is unsettled by construction, which is exactly the cancelled case.
    ///
    /// RED-proof: delete the `cut` block and `failed_stylesheet_fetches()` answers 0 — the state the
    /// bug is made of, and the state `failed_stylesheet_fetches()` shipped in with zero callers.
    #[test]
    fn a_stylesheet_the_deadline_cut_off_is_counted_as_failed() {
        let fonts = FontContext::new();
        let html = r#"<head><link rel="stylesheet" href="/slow.css"></head><body><p>x</p></body>"#;
        let mut page = Page::load(html, "http://ex.test/", &fonts, 800.0);
        assert_eq!(
            page.failed_stylesheet_fetches(),
            0,
            "nothing has been attempted yet"
        );
        // Already expired: `collect_before_deadline` yields an empty set immediately.
        page.load_deadline = Some(std::time::Instant::now() - std::time::Duration::from_secs(1));
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("rt");
        let applied = rt.block_on(page.fetch_and_apply_stylesheets(&fonts, 800.0));
        assert_eq!(
            applied, 0,
            "the deadline had already passed — nothing applied"
        );
        assert_eq!(
            page.failed_stylesheet_fetches(),
            1,
            "the sheet the deadline cut off must be RECORDED as failed — otherwise the page renders \
             in UA-default fallback and every downstream measurement reads that as our paint bug"
        );
    }

    /// **A STYLESHEET THE ORIGIN 404s IS NOT A PAGE WE FAILED TO STYLE.**
    ///
    /// `failed_stylesheet_fetches()` answers one question — *"is this layout UA-default fallback
    /// rather than our rendering of the author's design?"* — and every downstream measurement uses
    /// it to decide whether the site can be scored at all. A `404` answers it **no**: the author's
    /// `<link>` is dead in their own HTML, the reference browser gets the same `404`, and both
    /// engines render the same page without it.
    ///
    /// Measured (t860): all three `css-starved` rows of the t857 sweep were 404s at the origin —
    /// `cuneocronaca.it` (2 of 6 sheets, the other 4 serve 200), `m.youm7.com`, `nortenoticia`'s
    /// cdnjs tailwind — while the reason string those rows carried said *"cut by our own load
    /// deadline, NOT refused by the origin"*.
    ///
    /// The two halves are asserted together on purpose: the guard is what keeps the fix from
    /// becoming "stop counting failures". RED-proof: drop the `absent` arm and the first assertion
    /// reads 1; widen it past 404/410 and the second reads 0.
    #[test]
    fn a_stylesheet_the_origin_does_not_have_is_not_counted_as_unstyled() {
        let fonts = FontContext::new();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("rt");

        // A local one-shot origin, so the STATUS is the only variable. Two ports, two answers.
        let serve = |status_line: &'static str| {
            let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
            let port = l.local_addr().unwrap().port();
            std::thread::spawn(move || {
                for s in l.incoming().take(1).flatten() {
                    use std::io::{Read, Write};
                    let mut b = [0u8; 2048];
                    let _ = (&s).read(&mut b);
                    let _ = (&s).write_all(
                        format!(
                            "{status_line}\r\nContent-Type: text/css\r\nContent-Length: 0\r\n\r\n"
                        )
                        .as_bytes(),
                    );
                }
            });
            port
        };

        let missing = serve("HTTP/1.1 404 Not Found");
        let mut page = Page::load(
            &format!(
                r#"<head><link rel="stylesheet" href="http://127.0.0.1:{missing}/gone.css"></head><body><p>x</p></body>"#
            ),
            "http://ex.test/",
            &fonts,
            800.0,
        );
        rt.block_on(page.fetch_and_apply_stylesheets(&fonts, 800.0));
        assert_eq!(
            page.failed_stylesheet_fetches(),
            0,
            "a 404 sheet is a dead link in the AUTHOR's html — Chrome gets the same 404 and renders \
             the same page, so this layout is not UA-fallback relative to the reference and the \
             site stays scorable"
        );

        // THE GUARD — a 5xx is weather that may well have served the reference browser fine, so it
        // still counts. Same fixture, same code path, one status apart.
        let broken = serve("HTTP/1.1 503 Service Unavailable");
        let mut page = Page::load(
            &format!(
                r#"<head><link rel="stylesheet" href="http://127.0.0.1:{broken}/down.css"></head><body><p>x</p></body>"#
            ),
            "http://ex.test/",
            &fonts,
            800.0,
        );
        rt.block_on(page.fetch_and_apply_stylesheets(&fonts, 800.0));
        assert_eq!(
            page.failed_stylesheet_fetches(),
            1,
            "only 404/410 is 'the resource does not exist for anybody' — a 5xx must still be counted"
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

/// Serialise `(name, mime_type, contents)` triples into the `[{name,type,text}]` document that both
/// [`Page::set_input_files`] and [`Page::dispatch_drop`] hand to the JS side.
///
/// Hand-rolled rather than pulled from a serialiser so the escaping is visible at the point it
/// matters: a filename or a file body containing `"` or `\` would otherwise produce a document the
/// prelude's `JSON.parse` silently rejects — and the failure mode of that is not an error, it is
/// `files.length === 0`, i.e. **"no file chosen" reported for a file that IS chosen.**
fn files_to_json(files: &[(String, String, String)]) -> String {
    fn esc(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
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
    format!(
        "[{}]",
        files
            .iter()
            .map(|(name, ty, text)| format!(
                r#"{{"name":"{}","type":"{}","text":"{}"}}"#,
                esc(name),
                esc(ty),
                esc(text)
            ))
            .collect::<Vec<_>>()
            .join(",")
    )
}
