//! manuk-html — HTML parsing.
//!
//! Per CLAUDE.md we *reuse* `html5ever` (Servo's spec-compliant, streaming HTML
//! tokenizer/tree-builder) rather than hand-rolling a parser. This crate drives
//! html5ever into an `RcDom` and then walks that into our arena-based
//! [`manuk_dom::Dom`], which is the representation the rest of the engine consumes.
//!
//! Streaming note (CLAUDE.md § click-to-navigate latency): html5ever is itself an
//! incremental `TendrilSink`, so the eventual zero-copy socket→tokenizer path feeds
//! bytes in as they arrive. This first pass parses a fully-buffered document; the
//! incremental driver is a drop-in on the same sink.

use html5ever::tendril::TendrilSink;
use html5ever::{parse_document, ParseOpts};
use manuk_dom::{Dom, NodeId};
use markup5ever_rcdom::{Handle, NodeData as RcNodeData, RcDom};

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
}
