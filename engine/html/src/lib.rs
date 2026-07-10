//! manuk-html — HTML parsing.
//!
//! Per CLAUDE.md we *reuse* `html5ever` (Servo's spec-compliant, streaming HTML
//! tokenizer/tree-builder) rather than hand-rolling a parser. This crate drives
//! html5ever into an `RcDom` and then walks that into our arena-based
//! [`manuk_dom::Dom`], which is the representation the rest of the engine consumes.
//!
//! Streaming (CLAUDE.md § click-to-navigate latency): [`parse`] handles a fully-
//! buffered document, while [`StreamParser`] drives html5ever incrementally — feeding
//! chunks off the socket and snapshotting the parsed-so-far tree, so the shell can
//! first-paint `<head>` + above-the-fold before the tail arrives (B-latency).

use std::rc::Rc;

use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::{parse_document, ParseOpts, Parser};
use manuk_dom::{Dom, NodeData, NodeId};
use markup5ever_rcdom::{Handle, NodeData as RcNodeData, RcDom};

/// HTML **void elements** (no closing tag, no children) — used by serialization.
const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// Parse a UTF-8 HTML string into a [`Dom`].
pub fn parse(html: &str) -> Dom {
    parse_bytes(html.as_bytes())
}

/// Parse HTML bytes (assumed UTF-8) into a [`Dom`].
///
/// Encoding sniffing (`<meta charset>` / BOM / HTTP `Content-Type`) is a follow-on;
/// for now input is treated as UTF-8, matching the common case for the target site
/// set.
pub fn parse_bytes(bytes: &[u8]) -> Dom {
    let rc: RcDom = parse_document(RcDom::default(), ParseOpts::default())
        .from_utf8()
        .read_from(&mut std::io::Cursor::new(bytes))
        .expect("RcDom parsing is infallible for in-memory input");

    let mut dom = Dom::new();
    let root = dom.root();
    // The RcDom `Document` handle mirrors our `Document` root: walk its children.
    for child in rc.document.children.borrow().iter() {
        walk(child, &mut dom, root);
    }
    dom
}

/// An **incremental** HTML parser (B-latency first-paint checkpoint). html5ever is
/// itself a streaming [`TendrilSink`]: [`feed`](StreamParser::feed) pushes bytes as
/// they arrive off the socket, and [`snapshot`](StreamParser::snapshot) walks the
/// parsed-so-far tree into an arena [`Dom`] at any point — so the shell can lay out
/// and paint `<head>` + early `<body>` before the rest of the document streams in.
///
/// The trick: we clone the `RcDom`'s document `Handle` (an `Rc<Node>`) before handing
/// the `RcDom` to the parser; the tree builder mutates that same node in place, so the
/// clone always reflects current progress.
pub struct StreamParser {
    parser: Parser<RcDom>,
    document: Handle,
}

impl Default for StreamParser {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamParser {
    /// Start a streaming parse.
    pub fn new() -> Self {
        let rcdom = RcDom::default();
        let document = Rc::clone(&rcdom.document);
        let parser = parse_document(rcdom, ParseOpts::default());
        StreamParser { parser, document }
    }

    /// Feed the next chunk of document text (as it arrives off the network).
    pub fn feed(&mut self, chunk: &str) {
        self.parser.process(StrTendril::from(chunk));
    }

    /// Walk the **parsed-so-far** tree into an arena [`Dom`] (a partial document).
    pub fn snapshot(&self) -> Dom {
        rcdom_to_dom(&self.document)
    }

    /// Finish parsing and return the complete [`Dom`].
    pub fn finish(self) -> Dom {
        let rc = self.parser.finish();
        // `rc.document` is the same node the running `self.document` aliased.
        rcdom_to_dom(&rc.document)
    }

    /// Has the `<body>` been reached — i.e. `<head>` (and its render-blocking `<link>`/
    /// `<style>`) is parsed, the point at which a first paint is worthwhile?
    pub fn body_started(&self) -> bool {
        fn find_body(handle: &Handle) -> bool {
            for child in handle.children.borrow().iter() {
                if let RcNodeData::Element { name, .. } = &child.data {
                    if name.local.as_ref() == "body" {
                        return true;
                    }
                }
                if find_body(child) {
                    return true;
                }
            }
            false
        }
        find_body(&self.document)
    }
}

/// Walk an RcDom `document` handle's children into a fresh arena [`Dom`].
fn rcdom_to_dom(document: &Handle) -> Dom {
    let mut dom = Dom::new();
    let root = dom.root();
    for child in document.children.borrow().iter() {
        walk(child, &mut dom, root);
    }
    dom
}

/// Recursively convert an RcDom node and its subtree, appending under `parent`.
fn walk(handle: &Handle, dom: &mut Dom, parent: NodeId) {
    let new_id = match &handle.data {
        RcNodeData::Document => {
            // Nested documents don't occur here; treat as transparent.
            for child in handle.children.borrow().iter() {
                walk(child, dom, parent);
            }
            return;
        }
        RcNodeData::Doctype { name, .. } => Some(dom.create_doctype(name.to_string())),
        RcNodeData::Text { contents } => Some(dom.create_text(contents.borrow().to_string())),
        RcNodeData::Comment { contents } => Some(dom.create_comment(contents.to_string())),
        RcNodeData::Element { name, attrs, .. } => {
            let id = dom.create_element(name.local.to_string());
            for attr in attrs.borrow().iter() {
                dom.set_attr(id, attr.name.local.to_string(), attr.value.to_string());
            }
            Some(id)
        }
        // Processing instructions have no rendered representation.
        RcNodeData::ProcessingInstruction { .. } => None,
    };

    if let Some(id) = new_id {
        dom.append_child(parent, id);
        for child in handle.children.borrow().iter() {
            walk(child, dom, id);
        }
    }
}

// ---------------------------------------------------------------------------
// Serialization + fragment grafting (backs `Element.innerHTML`)
// ---------------------------------------------------------------------------

/// Serialize `node`'s **children** back to an HTML string (the `innerHTML` getter).
/// Void elements are emitted without a closing tag; text and attribute values are
/// entity-escaped. Not a full HTML-serialization conformance target (no
/// `<template>`/CDATA/foreign-content special cases) — the documented common case.
pub fn serialize_inner(dom: &Dom, node: NodeId) -> String {
    let mut out = String::new();
    for child in dom.children(node) {
        serialize_node(dom, child, &mut out);
    }
    out
}

/// Serialize a single node (including itself) into `out`.
fn serialize_node(dom: &Dom, node: NodeId, out: &mut String) {
    match dom.data(node) {
        NodeData::Element(el) => {
            out.push('<');
            out.push_str(&el.name);
            for attr in &el.attrs {
                out.push(' ');
                out.push_str(&attr.name);
                out.push_str("=\"");
                push_escaped_attr(&attr.value, out);
                out.push('"');
            }
            out.push('>');
            if VOID_ELEMENTS.contains(&el.name.as_str()) {
                return;
            }
            for child in dom.children(node) {
                serialize_node(dom, child, out);
            }
            out.push_str("</");
            out.push_str(&el.name);
            out.push('>');
        }
        NodeData::Text(t) => push_escaped_text(t, out),
        NodeData::Comment(c) => {
            out.push_str("<!--");
            out.push_str(c);
            out.push_str("-->");
        }
        NodeData::Doctype { name } => {
            out.push_str("<!DOCTYPE ");
            out.push_str(name);
            out.push('>');
        }
        NodeData::Document => {}
    }
}

fn push_escaped_text(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
}

fn push_escaped_attr(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
}

/// Replace `node`'s children with the parse of `html` (the `innerHTML` setter).
/// The fragment is parsed as a document and its `<body>` children are deep-cloned
/// into `node` (a pragmatic fragment parse; true context-aware fragment parsing —
/// e.g. `<tr>` inside a table — is a follow-on).
pub fn set_inner_html(dom: &mut Dom, node: NodeId, html: &str) {
    // Detach existing children.
    let existing: Vec<NodeId> = dom.children(node).collect();
    for c in existing {
        dom.detach(c);
    }
    // Parse the fragment and graft the body's children in.
    let fragment = parse(html);
    if let Some(body) = fragment.find_first("body") {
        let roots: Vec<NodeId> = fragment.children(body).collect();
        for r in roots {
            clone_into(&fragment, r, dom, node);
        }
    }
}

/// Deep-copy `src_node`'s subtree from `src` into `dst` under `dst_parent`
/// (NodeIds are arena-local, so cross-`Dom` grafting must clone, not move).
fn clone_into(src: &Dom, src_node: NodeId, dst: &mut Dom, dst_parent: NodeId) {
    match src.data(src_node) {
        NodeData::Element(el) => {
            let name = el.name.clone();
            let attrs: Vec<(String, String)> = el
                .attrs
                .iter()
                .map(|a| (a.name.clone(), a.value.clone()))
                .collect();
            let new = dst.create_element(name);
            for (n, v) in attrs {
                dst.set_attr(new, n, v);
            }
            dst.append_child(dst_parent, new);
            let kids: Vec<NodeId> = src.children(src_node).collect();
            for k in kids {
                clone_into(src, k, dst, new);
            }
        }
        NodeData::Text(t) => {
            let n = dst.create_text(t.clone());
            dst.append_child(dst_parent, n);
        }
        NodeData::Comment(c) => {
            let n = dst.create_comment(c.clone());
            dst.append_child(dst_parent, n);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_document() {
        let dom = parse(
            r#"<!DOCTYPE html><html><head><title>Hi</title></head>
               <body><p class="lead">Hello <b>world</b></p></body></html>"#,
        );
        // html5ever inserts html/head/body even where implied.
        assert!(dom.find_first("html").is_some());
        assert!(dom.find_first("head").is_some());
        assert!(dom.find_first("body").is_some());
        let p = dom.find_first("p").expect("p element");
        assert!(dom.element(p).unwrap().has_class("lead"));
        assert_eq!(dom.text_content(p), "Hello world");
        assert_eq!(dom.text_content(dom.find_first("title").unwrap()), "Hi");
    }

    #[test]
    fn recovers_from_missing_tags() {
        // No <html>/<body>; html5ever's tree-builder must synthesize them.
        let dom = parse("<p>one<p>two");
        let ps: Vec<_> = dom
            .descendants(dom.root())
            .filter(|&n| dom.tag_name(n) == Some("p"))
            .collect();
        assert_eq!(ps.len(), 2, "two paragraphs via auto-closing");
    }

    #[test]
    fn stream_parser_first_paint_checkpoint() {
        // Chunk 1 delivers <head> + the start of <body>; chunk 2 the rest.
        let mut sp = StreamParser::new();
        sp.feed(
            "<!DOCTYPE html><html><head><title>T</title>\
                 <link rel='stylesheet' href='/s.css'></head><body><h1>Above the fold</h1>",
        );
        // The head is parsed and body has started → a first paint is worthwhile.
        assert!(sp.body_started(), "body reached after the head");
        let early = sp.snapshot();
        assert!(early.find_first("h1").is_some(), "early content is present");
        assert!(early.find_first("title").is_some());
        let early_h1_text = early
            .find_first("h1")
            .map(|n| early.text_content(n))
            .unwrap_or_default();
        assert_eq!(early_h1_text, "Above the fold");
        // The later paragraph has NOT arrived yet.
        assert!(
            early.find_first("p").is_none(),
            "below-the-fold content not yet parsed at the first-paint checkpoint"
        );

        // Chunk 2 streams the rest.
        sp.feed("<p>below the fold</p></body></html>");
        let full = sp.finish();
        assert!(full.find_first("h1").is_some());
        assert!(
            full.find_first("p").is_some(),
            "full document has the late content"
        );
    }

    #[test]
    fn serialize_inner_round_trips() {
        let dom = parse("<body><p class=\"lead\">Hi <b>there</b><br>x &amp; y</p></body>");
        let p = dom.find_first("p").unwrap();
        let html = serialize_inner(&dom, p);
        // Text escaped, void <br> not closed, nested element serialized.
        assert_eq!(html, "Hi <b>there</b><br>x &amp; y");
    }

    #[test]
    fn set_inner_html_replaces_children() {
        let mut dom = parse("<body><div id=host>old</div></body>");
        let host = dom.find_first("div").unwrap();
        set_inner_html(&mut dom, host, "<span>new</span><b>bold</b>");
        assert_eq!(dom.text_content(host), "newbold");
        // The old text node is gone; two element children remain.
        let kids: Vec<_> = dom.children(host).collect();
        assert_eq!(kids.len(), 2);
        assert_eq!(dom.tag_name(kids[0]), Some("span"));
        assert_eq!(dom.tag_name(kids[1]), Some("b"));
        // Round-trips through serialization.
        assert_eq!(serialize_inner(&dom, host), "<span>new</span><b>bold</b>");
    }
}
