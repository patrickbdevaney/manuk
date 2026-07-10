//! INFERENCE.MD §4 — **page-triage fast path**.
//!
//! Running JavaScript is the expensive part of loading a page: parse + style + layout are
//! cheap and CPU-deterministic, but spinning up a SpiderMonkey realm and executing a page's
//! scripts is where the cost and the concurrency ceiling live. Most Tier-1 content —
//! articles, docs, product pages, search results — is **already in the server-rendered
//! HTML**; only genuine client-rendered SPAs need the JS pass to have any content at all.
//!
//! So before paying for JS, [`triage`] classifies the raw HTML: it parses to a DOM (no
//! layout, no JS), measures the *visible* text already present, and looks for the tell-tale
//! signals of an empty SPA shell (a lone `<div id="root">`, a `<noscript>` telling the user
//! to enable JavaScript, a body that is mostly `<script>`). A page with substantial
//! server-rendered text is served straight through; only the sparse-shell minority is
//! routed to the JS engine.
//!
//! This is deliberately *conservative about forcing JS*: a page with plenty of static text
//! never needs it, and a genuinely thin page with no SPA signal is reported as thin rather
//! than optimistically sent through a JS pass that would not help. The report carries its
//! reasons so the decision is auditable, never a silent gate.

use manuk_dom::{Dom, NodeData, NodeId};

/// How much collapsed visible text counts as "this page already has its content". Below
/// this, we look at SPA signals; at or above it, JS is unnecessary regardless of scripts.
pub const MEANINGFUL_TEXT_CHARS: usize = 200;

/// Element `id`s conventionally used as the single mount point of a client-rendered app. An
/// *empty* one of these in an otherwise text-poor page is a strong "needs JS" signal.
const SPA_MOUNT_IDS: &[&str] = &["root", "app", "__next", "__nuxt", "__layout", "___gatsby", "q-app"];

/// A single reason the triage decision went the way it did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    /// Enough server-rendered visible text to answer from; JS unnecessary.
    ServerRenderedText,
    /// An empty conventional SPA mount point (`<div id="root">` with no text) was found.
    EmptySpaMount,
    /// A `<noscript>` block asks the user to enable JavaScript.
    NoscriptEnableJs,
    /// The body is dominated by `<script>` with little else.
    ScriptHeavyShell,
    /// Little static text and no SPA signal — a genuinely thin page; JS would likely not
    /// add content, so we do not force it.
    SparseNoSpaSignal,
}

/// The outcome of triaging a page's raw HTML.
#[derive(Clone, Debug)]
pub struct TriageReport {
    /// Whether the meaningful content requires a JS pass to appear. `false` means the
    /// server-rendered HTML already carries it (the fast path).
    pub needs_js: bool,
    /// Collapsed visible-text length found in the static HTML (script/style excluded).
    pub static_text_len: usize,
    /// Count of `<script>` elements (inline or `src`).
    pub script_count: usize,
    /// The signals that drove the decision, most salient first.
    pub signals: Vec<Signal>,
    /// The collapsed visible text triage extracted while classifying — reused by the
    /// traversal cache as the content-address key, so no second parse/layout is needed just
    /// to hash "what actually matters" on this page.
    pub extracted_text: String,
}

impl TriageReport {
    /// The fast path: server-rendered content is sufficient, skip JS.
    pub fn is_fast_path(&self) -> bool {
        !self.needs_js
    }
}

/// Triage raw HTML: decide whether a JS pass is needed for its content to be present.
///
/// Cheap by construction — one parse and one DOM walk, no style or layout.
pub fn triage(html: &str) -> TriageReport {
    let dom = manuk_html::parse(html);
    let root = dom.root();

    let mut visible = String::new();
    let mut script_count = 0usize;
    let mut noscript_enable_js = false;
    let mut mount_candidates: Vec<NodeId> = Vec::new();

    for id in dom.descendants(root) {
        match dom.data(id) {
            NodeData::Element(_) => {
                let tag = dom.tag_name(id).unwrap_or("").to_ascii_lowercase();
                match tag.as_str() {
                    "script" => script_count += 1,
                    "noscript" => {
                        let t = dom.text_content(id).to_ascii_lowercase();
                        if t.contains("enable") && t.contains("javascript")
                            || t.contains("enable") && t.contains("script")
                        {
                            noscript_enable_js = true;
                        }
                    }
                    _ => {}
                }
                // A conventional SPA mount id?
                if let Some(el) = dom.element(id) {
                    if let Some(idv) = el.attr("id") {
                        if SPA_MOUNT_IDS.contains(&idv.to_ascii_lowercase().as_str()) {
                            mount_candidates.push(id);
                        }
                    }
                }
            }
            NodeData::Text(t) => {
                // Only count text that is not inside <script>/<style>/<head>/<noscript>.
                if is_visible_text_context(&dom, id) {
                    visible.push_str(t);
                    visible.push(' ');
                }
            }
            _ => {}
        }
    }

    let extracted_text = collapse_ws(&visible);
    let static_text_len = extracted_text.len();

    // An SPA mount is only a signal if it is *empty* (no meaningful text under it).
    let empty_mount = mount_candidates.iter().any(|&m| {
        collapse_ws(&dom.text_content(m)).len() < 20
    });

    let mut signals = Vec::new();
    let needs_js;

    if static_text_len >= MEANINGFUL_TEXT_CHARS {
        // Plenty of server-rendered text: fast path, no matter how many scripts there are.
        signals.push(Signal::ServerRenderedText);
        needs_js = false;
    } else if empty_mount {
        signals.push(Signal::EmptySpaMount);
        if noscript_enable_js {
            signals.push(Signal::NoscriptEnableJs);
        }
        needs_js = true;
    } else if noscript_enable_js {
        signals.push(Signal::NoscriptEnableJs);
        needs_js = true;
    } else if script_count >= 3 && static_text_len < MEANINGFUL_TEXT_CHARS / 4 {
        // Script-dominated with almost no text: a client-rendered shell.
        signals.push(Signal::ScriptHeavyShell);
        needs_js = true;
    } else {
        // Thin, but nothing says JS would help. Do not force it.
        signals.push(Signal::SparseNoSpaSignal);
        needs_js = false;
    }

    TriageReport {
        needs_js,
        static_text_len,
        script_count,
        signals,
        extracted_text,
    }
}

/// Whether a text node sits in a context whose text is visible page content (i.e. not inside
/// `<script>`, `<style>`, `<head>`, `<noscript>`, or `<template>`).
fn is_visible_text_context(dom: &Dom, text: NodeId) -> bool {
    let mut cur = dom.parent(text);
    while let Some(p) = cur {
        if let Some(tag) = dom.tag_name(p) {
            match tag.to_ascii_lowercase().as_str() {
                "script" | "style" | "head" | "noscript" | "template" | "title" => return false,
                _ => {}
            }
        }
        cur = dom.parent(p);
    }
    true
}

/// Collapse runs of ASCII whitespace to single spaces and trim — so text length measures
/// actual content, not indentation.
fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_server_rendered_article_takes_the_fast_path() {
        let html = format!(
            "<html><head><title>News</title></head><body><article><h1>Headline</h1><p>{}</p></article><script src='/analytics.js'></script></body></html>",
            "The article body has plenty of real, server-rendered prose that the agent can read directly without ever running a line of JavaScript. ".repeat(3)
        );
        let r = triage(&html);
        assert!(r.is_fast_path(), "SSR text present: {:?}", r.signals);
        assert!(!r.needs_js);
        assert!(r.signals.contains(&Signal::ServerRenderedText));
        assert!(r.static_text_len >= MEANINGFUL_TEXT_CHARS);
        // Script presence does not force JS when the content is already there.
        assert_eq!(r.script_count, 1);
    }

    #[test]
    fn an_empty_spa_shell_needs_js() {
        let html = "<html><head><title>App</title></head><body>\
            <div id=\"root\"></div>\
            <script src=\"/bundle.js\"></script></body></html>";
        let r = triage(html);
        assert!(r.needs_js, "empty #root shell must need JS: {:?}", r.signals);
        assert!(r.signals.contains(&Signal::EmptySpaMount));
    }

    #[test]
    fn a_noscript_enable_js_notice_needs_js() {
        let html = "<html><body><noscript>You need to enable JavaScript to run this app.</noscript>\
            <div id=\"main\"></div><script src=\"/a.js\"></script></body></html>";
        let r = triage(html);
        assert!(r.needs_js);
        assert!(r.signals.contains(&Signal::NoscriptEnableJs));
    }

    #[test]
    fn a_script_heavy_shell_with_no_text_needs_js() {
        let html = "<html><body><script>a()</script><script>b()</script><script src=\"c.js\"></script></body></html>";
        let r = triage(html);
        assert!(r.needs_js);
        assert!(r.signals.contains(&Signal::ScriptHeavyShell));
    }

    /// A populated SPA mount (server-rendered into #root, as Next.js/Nuxt SSR do) is NOT an
    /// empty shell — the fast path still applies.
    #[test]
    fn a_prerendered_spa_mount_is_the_fast_path() {
        let html = format!(
            "<html><body><div id=\"__next\"><main><h1>Product</h1><p>{}</p></main></div><script src=\"/_next.js\"></script></body></html>",
            "Full server-rendered product description text that is already present in the HTML payload. ".repeat(3)
        );
        let r = triage(&html);
        assert!(r.is_fast_path(), "prerendered #__next is content, not a shell: {:?}", r.signals);
        assert!(r.signals.contains(&Signal::ServerRenderedText));
    }

    /// A thin page with no SPA signal is reported as sparse, not optimistically forced
    /// through a JS pass that would not help.
    #[test]
    fn a_thin_page_with_no_spa_signal_is_not_forced_through_js() {
        let html = "<html><body><p>Hi.</p></body></html>";
        let r = triage(html);
        assert!(!r.needs_js);
        assert!(r.signals.contains(&Signal::SparseNoSpaSignal));
    }

    /// Script text and style text never count as visible content.
    #[test]
    fn script_and_style_text_is_not_counted_as_content() {
        let html = "<html><head><style>.a{color:red}</style></head><body>\
            <script>var x = 'this is a long string inside a script that must not count as page text at all whatsoever';</script>\
            <p>Short.</p></body></html>";
        let r = triage(html);
        // Only "Short." (plus collapse) counts — well under the meaningful threshold.
        assert!(r.static_text_len < 20, "got {}", r.static_text_len);
    }
}
