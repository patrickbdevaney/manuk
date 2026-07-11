//! manuk-html — HTML parsing.
//!
//! Per CLAUDE.md we *reuse* `html5ever` (Servo's spec-compliant, streaming HTML
//! tokenizer/tree-builder) rather than hand-rolling a parser. This crate drives
//! html5ever directly into our arena-based
//! [`manuk_dom::Dom`], which is the representation the rest of the engine consumes.
//!
//! Streaming (CLAUDE.md § click-to-navigate latency): [`parse`] handles a fully-
//! buffered document, while [`StreamParser`] drives html5ever incrementally — feeding
//! chunks off the socket and snapshotting the parsed-so-far tree, so the shell can
//! first-paint `<head>` + above-the-fold before the tail arrives (B-latency).

use std::cell::RefCell;
use std::rc::Rc;

use html5ever::tendril::stream::Utf8LossyDecoder;
use html5ever::tendril::{ByteTendril, TendrilSink};
use html5ever::{parse_document, parse_fragment, ParseOpts, Parser};
use manuk_dom::{Dom, NodeData, NodeId};
/// N3 — our `TreeSink` directly over the arena DOM (enables Declarative Shadow DOM).
pub mod sink;


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
    // N3: parse straight into the arena. html5ever's tree builder drives our `ArenaSink`,
    // so `<template shadowrootmode>` reaches `attach_declarative_shadow` and a real shadow
    // root is attached. (The previous `RcDom` intermediate could not: that hook defaults to
    // `false` and `markup5ever_rcdom` never overrides it.)
    parse_document(sink::ArenaSink::new(), ParseOpts::default())
        .from_utf8()
        .read_from(&mut std::io::Cursor::new(bytes))
        .expect("parsing is infallible for in-memory input")
}

/// B-latency — an **incremental** parse driven by bytes as they arrive off the socket.
///
/// `feed`/`feed_bytes` push chunks (UTF-8 sequences split across a boundary are handled by
/// the decoder); [`snapshot`](StreamParser::snapshot) reads the parsed-so-far tree so a
/// first paint can happen before the tail arrives.
///
/// N3: this now streams into the arena directly, sharing the sink's `Rc<RefCell<Dom>>`
/// rather than snapshotting an `RcDom` and re-walking it on every call.
pub struct StreamParser {
    sink: Utf8LossyDecoder<Parser<sink::ArenaSink>>,
    dom: Rc<RefCell<Dom>>,
}

impl Default for StreamParser {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamParser {
    pub fn new() -> Self {
        let arena = sink::ArenaSink::new();
        let dom = arena.dom_handle();
        let sink = parse_document(arena, ParseOpts::default()).from_utf8();
        StreamParser { sink, dom }
    }

    /// Feed the next chunk of document **bytes** (as they arrive off the socket).
    pub fn feed_bytes(&mut self, bytes: &[u8]) {
        self.sink.process(ByteTendril::from_slice(bytes));
    }

    pub fn feed(&mut self, chunk: &str) {
        self.feed_bytes(chunk.as_bytes());
    }

    /// The **parsed-so-far** tree (a partial document).
    pub fn snapshot(&self) -> Dom {
        self.dom.borrow().clone()
    }

    /// Finish parsing and return the complete [`Dom`].
    pub fn finish(self) -> Dom {
        self.sink.finish()
    }

    /// Whether `<body>` has been opened yet — the head is complete, so a first paint of
    /// the partial document is meaningful.
    pub fn body_started(&self) -> bool {
        self.dom.borrow().find_first("body").is_some()
    }
}

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
        // A shadow root / template fragment is a separate tree: `innerHTML` of the host
        // never includes it (that is `getHTML({serializableShadowRoots})`, out of scope).
        NodeData::ShadowRoot { .. } | NodeData::Fragment => {}
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
    // Context-aware fragment parse: parse `html` as if inside `node`'s element, so
    // table-scoped content (`<tr>`, `<td>`, `<option>`, `<li>`, …) survives instead of
    // being dropped as it would at document level. The parsed nodes are children of the
    // fragment's synthetic root element.
    let context = dom.tag_name(node).unwrap_or("div").to_string();
    let fragment = parse_fragment_in(html, &context);
    let root = fragment.find_first("html").unwrap_or_else(|| fragment.root());
    let roots: Vec<NodeId> = fragment.children(root).collect();
    for r in roots {
        clone_into(&fragment, r, dom, node);
    }
}

/// Parse `html` as a fragment inside a `context_tag` element (HTML fragment parsing
/// algorithm), so context-sensitive content is retained. Returns a [`Dom`] whose synthetic
/// root element holds the parsed nodes.
pub fn parse_fragment_in(html: &str, context_tag: &str) -> Dom {
    let context = sink::html_name(context_tag);
    parse_fragment(sink::ArenaSink::new(), ParseOpts::default(), context, vec![], false)
        .from_utf8()
        .read_from(&mut std::io::Cursor::new(html.as_bytes()))
        .expect("parsing is infallible for in-memory input")
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

    #[test]
    fn set_inner_html_is_context_aware_for_table_rows() {
        // A `<tr>` set as innerHTML of a <tbody> must survive (document-level parsing
        // would drop it). Context-aware fragment parsing keeps it.
        let mut dom = parse("<body><table><tbody id=tb></tbody></table></body>");
        let tb = dom.find_first("tbody").unwrap();
        set_inner_html(&mut dom, tb, "<tr><td>cell</td></tr>");
        let rows: Vec<_> = dom.children(tb).collect();
        assert_eq!(rows.len(), 1, "the <tr> survived context-aware parsing");
        assert_eq!(dom.tag_name(rows[0]), Some("tr"));
        assert_eq!(dom.text_content(tb), "cell");
    }
}

#[cfg(test)]
mod shadow_tests {
    use super::*;
    use manuk_dom::ShadowRootMode;

    /// N3's headline acceptance. `<template shadowrootmode="open">` must produce a real
    /// shadow root; the `<p>` must remain a **light-DOM child of the host** in the node
    /// tree while being **slotted into the shadow tree** in the flat tree. Those are two
    /// different trees, and conflating them is the classic shadow-DOM bug.
    #[test]
    fn declarative_shadow_root_attaches_and_the_slot_fills_in_the_flat_tree() {
        let dom = parse(
            r#"<body><div id="host">
                 <template shadowrootmode="open"><span>before</span><slot></slot></template>
                 <p>light</p>
               </div></body>"#,
        );

        let host = dom.find_first("div").expect("host exists");

        // 1. A real shadow root is attached (this is what RcDom silently dropped).
        let shadow = dom.shadow_root(host).expect("shadow root attached");
        assert_eq!(dom.shadow_root_mode(shadow), Some(ShadowRootMode::Open));
        assert_eq!(dom.shadow_host(shadow), Some(host));

        // 2. The shadow root is NOT a child of the host in the node tree.
        assert!(
            !dom.children(host).any(|c| c == shadow),
            "the shadow root must not appear among the host's children"
        );

        // 3. The <p> IS still a light-DOM child of the host in the node tree.
        let p = dom.find_first("p").unwrap();
        assert_eq!(dom.parent(p), Some(host));

        // 4. The template's contents moved into the shadow root...
        let shadow_kids: Vec<&str> = dom
            .children(shadow)
            .filter_map(|c| dom.tag_name(c))
            .collect();
        assert_eq!(shadow_kids, vec!["span", "slot"]);

        // 5. ...and the FLAT tree of the host yields the shadow content, with the <slot>
        //    filled by the light-DOM <p>.
        let flat = dom.flat_children(host);
        let flat_tags: Vec<&str> = flat.iter().filter_map(|&c| dom.tag_name(c)).collect();
        assert_eq!(flat_tags, vec!["span", "slot"]);

        let slot = flat[1];
        assert_eq!(dom.tag_name(slot), Some("slot"));
        // The slot is filled by the host's light-DOM children. Whitespace text nodes are
        // slottable too (per spec), so compare the *element* view.
        let slotted = dom.flat_children(slot);
        assert!(slotted.contains(&p), "the slot must be filled by the light-DOM <p>");
        let slotted_elems: Vec<NodeId> = slotted.iter().copied().filter(|&n| dom.is_element(n)).collect();
        assert_eq!(slotted_elems, vec![p]);
    }

    #[test]
    fn a_closed_shadow_root_is_recorded_as_closed() {
        let dom = parse(r#"<div><template shadowrootmode="closed"><b>x</b></template></div>"#);
        let host = dom.find_first("div").unwrap();
        let sr = dom.shadow_root(host).unwrap();
        assert_eq!(dom.shadow_root_mode(sr), Some(ShadowRootMode::Closed));
    }

    /// A `<template>` WITHOUT `shadowrootmode` must stay an ordinary template — its
    /// contents live in a fragment, not in the light DOM, and no shadow root appears.
    #[test]
    fn a_plain_template_is_not_a_shadow_root_and_its_contents_are_not_rendered() {
        let dom = parse(r#"<div><template><b>hidden</b></template><i>shown</i></div>"#);
        let host = dom.find_first("div").unwrap();
        assert!(dom.shadow_root(host).is_none(), "no shadowrootmode => no shadow root");

        let tpl = dom.find_first("template").unwrap();
        let frag = dom.get_template_contents(tpl).expect("template has contents");
        let inner: Vec<&str> = dom.children(frag).filter_map(|c| dom.tag_name(c)).collect();
        assert_eq!(inner, vec!["b"], "contents live in the fragment, not the light DOM");

        // The template's contents are NOT children of the template in the node tree.
        assert_eq!(dom.children(tpl).count(), 0);
        // ...so the visible text of the div is only the <i>.
        assert!(!dom.text_content(host).contains("hidden"));
        assert!(dom.text_content(host).contains("shown"));
    }

    /// Named slots: a light child's `slot` attribute picks its slot; unnamed children go
    /// to the default slot. A slot with nothing assigned renders its fallback children.
    #[test]
    fn named_slots_and_fallback_content() {
        let dom = parse(
            r#"<div id="h">
                 <template shadowrootmode="open">
                   <slot name="title"></slot>
                   <slot></slot>
                   <slot name="empty">fallback</slot>
                 </template>
                 <h1 slot="title">T</h1>
                 <p>body</p>
               </div>"#,
        );
        let host = dom.find_first("div").unwrap();
        let flat = dom.flat_children(host);
        let slots: Vec<NodeId> = flat.iter().copied().filter(|&n| dom.tag_name(n) == Some("slot")).collect();
        assert_eq!(slots.len(), 3);

        let h1 = dom.find_first("h1").unwrap();
        let p = dom.find_first("p").unwrap();

        // named slot gets the h1; default slot gets the p (plus the source's whitespace
        // text nodes, which are slottable per spec — hence the element-only view).
        let elems = |n: NodeId| -> Vec<NodeId> {
            dom.flat_children(n).into_iter().filter(|&c| dom.is_element(c)).collect()
        };
        assert_eq!(elems(slots[0]), vec![h1]);
        assert_eq!(elems(slots[1]), vec![p]);

        // The unassigned named slot renders its fallback content instead.
        let fallback = dom.flat_children(slots[2]);
        assert_eq!(fallback.len(), 1);
        assert_eq!(dom.text_content(fallback[0]).trim(), "fallback");
    }

    /// Text nodes are slottables too — a bare string child of the host renders through
    /// the default slot. Asserting this keeps the behavior deliberate rather than
    /// incidental.
    #[test]
    fn text_children_of_the_host_are_slotted() {
        let dom = parse(r#"<div><template shadowrootmode="open"><slot></slot></template>hello</div>"#);
        let host = dom.find_first("div").unwrap();
        let slot = dom.flat_children(host)[0];
        assert_eq!(dom.tag_name(slot), Some("slot"));
        let slotted = dom.flat_children(slot);
        assert_eq!(slotted.len(), 1);
        assert_eq!(dom.text_content(slotted[0]), "hello");
    }

    /// The parser must merge adjacent text runs; two text nodes for one string would
    /// produce two inline runs in layout.
    #[test]
    fn adjacent_text_is_merged_into_one_node() {
        let dom = parse("<p>a&amp;b</p>");
        let p = dom.find_first("p").unwrap();
        assert_eq!(dom.children(p).count(), 1, "one text node, not three");
        assert_eq!(dom.text_content(p), "a&b");
    }
}
