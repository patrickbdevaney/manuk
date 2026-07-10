//! manuk-page — the shared page pipeline.
//!
//! `bytes → DOM → style → layout → paint`, wired end to end. This is the common
//! engine core CLAUDE.md calls for: the **headful shell** and the **headless
//! agent** both drive these functions and diverge only at how they consume the
//! output — the shell presents to a window, the agent screenshots + reads it.

use anyhow::{Context, Result};
use manuk_css::{MinimalCascade, Rgba, StyleEngine, StyleMap, Stylesheet};
use manuk_dom::Dom;
use manuk_layout::{layout_document, LayoutBox};
use manuk_paint::{Canvas, CpuPainter, Painter};
use manuk_text::FontContext;
use url::Url;

/// A hyperlink discovered in the page, with its href resolved to an absolute URL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub text: String,
    pub href: String,
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
    styles: StyleMap,
}

impl Page {
    /// Parse + style + lay out `html` for a viewport of `viewport_width` px.
    pub fn load(html: &str, final_url: &str, fonts: &FontContext, viewport_width: f32) -> Page {
        let dom = manuk_html::parse(html);

        let sheets: Vec<Stylesheet> = MinimalCascade::collect_style_elements(&dom);
        let styles = MinimalCascade.cascade(&dom, &sheets);

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

        let root_box = layout_document(&dom, &styles, fonts, viewport_width);
        let content_height = root_box.content_bottom();

        Page {
            final_url: final_url.to_string(),
            title,
            content_height,
            root_box,
            dom,
            styles,
        }
    }

    /// Re-run layout at a new viewport width (reuses the DOM + computed styles).
    pub fn relayout(&mut self, fonts: &FontContext, viewport_width: f32) {
        self.root_box = layout_document(&self.dom, &self.styles, fonts, viewport_width);
        self.content_height = self.root_box.content_bottom();
    }

    /// Rasterize the whole page to a canvas of the given pixel size (CPU tier).
    pub fn paint(&self, fonts: &FontContext, width: u32, height: u32) -> Canvas {
        CpuPainter::new(fonts).render(&self.root_box, width, height, Rgba::WHITE)
    }

    /// Rasterize the visible viewport with the content scrolled up by `scroll_y`.
    pub fn paint_scrolled(
        &self,
        fonts: &FontContext,
        width: u32,
        height: u32,
        scroll_y: f32,
    ) -> Canvas {
        CpuPainter::new(fonts).render_scrolled(&self.root_box, width, height, Rgba::WHITE, scroll_y)
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
    pub fn visible_text(&self) -> String {
        let node = self
            .dom
            .find_first("body")
            .unwrap_or_else(|| self.dom.root());
        collapse_ws(&self.dom.text_content(node))
    }
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Fetch a document's HTML. Supports `http(s)://` (via `manuk-net`), `file://`, and
/// bare local paths. Returns `(html, final_url_after_redirects)`.
pub async fn fetch_html(url: &str) -> Result<(String, String)> {
    if url.starts_with("http://") || url.starts_with("https://") {
        let resp = manuk_net::fetch(url)
            .await
            .with_context(|| format!("fetching {url}"))?;
        if resp.status >= 400 {
            anyhow::bail!("server returned HTTP {} for {}", resp.status, url);
        }
        Ok((resp.text(), resp.final_url.to_string()))
    } else {
        let path = url.strip_prefix("file://").unwrap_or(url);
        let html =
            std::fs::read_to_string(path).with_context(|| format!("reading local file {path}"))?;
        Ok((html, url.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
