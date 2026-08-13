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

/// The result of an XML parse: the tree, and **whether the source was well-formed**.
///
/// XML has no error recovery. A document that is not well-formed does not get "fixed up" the way
/// HTML does — per the DOM spec it must be replaced wholesale by a `parsererror` document, so the
/// verdict is not diagnostics, it is the return value.
pub struct XmlParse {
    pub dom: Dom,
    /// Empty iff the source was well-formed.
    pub errors: Vec<String>,
}

impl XmlParse {
    /// ⚠ **MEASURED, NOT ASSUMED — and it is not the full spec set.** xml5ever reports mismatched
    /// end tags, EOF inside a tag, a stray end tag, a bad character reference, two document
    /// elements, an empty document and duplicate attributes. It does **NOT** report two cases a
    /// strict XML parser must reject, because its tree builder silently recovers from both:
    ///
    /// | input              | strict XML | here            |
    /// |--------------------|------------|-----------------|
    /// | `<foo>` (unclosed) | fatal      | **accepted**    |
    /// | `<f a=1/>` (unquoted attr value) | fatal | **accepted** |
    ///
    /// The unclosed-tag case is not reachable from the sink: xml5ever's `end()` drains its open
    /// element stack and pops each entry *before* `TreeSink::finish` runs, and `open_elems` is
    /// private, so by the time we can look, a well-formed and an unclosed parse are identical. It
    /// is written down here rather than papered over — `parseFromString("<foo>", "text/xml")`
    /// therefore yields the parsed tree where Chrome yields a `parsererror`. Closing it needs a
    /// change upstream (or driving the tokenizer directly), not a guess in this crate.
    pub fn well_formed(&self) -> bool {
        self.errors.is_empty()
    }
}

/// **Parse an XML string into a [`Dom`] — case-sensitively, with real namespaces.**
///
/// This is the same arena, the same `ArenaSink` and the same `markup5ever::TreeSink` trait the
/// HTML path already drives; only the tree builder differs. That is why this is a port rather than
/// a second parser: xml5ever and html5ever share a version train and an interface, so every
/// behaviour the sink already implements (shadow roots, template contents, the id index) applies
/// unchanged.
///
/// The difference that matters to callers is that **XML is not HTML with different tags**:
/// `<Foo/>` and `<foo/>` are distinct elements, an unclosed tag is a fatal error rather than
/// something to recover from, and elements carry the namespace their `xmlns` declared instead of
/// being forced into the HTML namespace. Running XML through the HTML parser — which is what
/// `DOMParser.parseFromString(s, 'text/xml')` did before this existed — silently lowercases every
/// tag name and never reports the error.
pub fn parse_xml(src: &str) -> XmlParse {
    let sink = sink::ArenaSink::new();
    let errors = sink.errors_handle();
    let dom = xml5ever::driver::parse_document(sink, xml5ever::driver::XmlParseOpts::default())
        .from_utf8()
        .read_from(&mut std::io::Cursor::new(src.as_bytes()))
        .expect("parsing is infallible for in-memory input");
    let errors = errors.borrow().clone();
    XmlParse { dom, errors }
}

/// The namespace a `parsererror` element lives in. Gecko minted it, and every other engine
/// adopted it verbatim — WPT asserts this exact string, so it is not ours to choose.
pub const PARSERERROR_NS: &str = "http://www.mozilla.org/newlayout/xml/parsererror.xml";

/// **Parse `src` as XML directly under `dst_doc`, an existing document node in `dst`.**
///
/// `NodeId`s are arena-local, so the freshly parsed tree cannot be moved across — it is cloned in,
/// exactly as `set_inner_html` already does for HTML fragments.
///
/// Returns the well-formedness errors, empty iff the parse was clean. **On a malformed document
/// the tree grafted in is a `parsererror` document, not the partial parse** — that substitution
/// lives here, in one place, rather than at each call site, because it IS the parse result as far
/// as the DOM spec is concerned.
pub fn parse_xml_into(src: &str, dst: &mut Dom, dst_doc: NodeId) -> Vec<String> {
    let parsed = parse_xml(src);
    if parsed.well_formed() {
        let root = parsed.dom.root();
        let kids: Vec<NodeId> = parsed.dom.children(root).collect();
        for k in kids {
            clone_into(&parsed.dom, k, dst, dst_doc);
        }
    } else {
        // Per DOM §DOMParser: the document element becomes `parsererror`, carrying a
        // human-readable description of what went wrong.
        let err = dst.create_element_ns(Some(PARSERERROR_NS.to_string()), "parsererror");
        let msg = dst.create_text(parsed.errors.join("\n"));
        dst.append_child(err, msg);
        dst.append_child(dst_doc, err);
    }
    parsed.errors
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
    // The other half of the template redirect (see `set_inner_html`): a `<template>`'s markup lives
    // in its template contents, so serializing its child list returns `""` for every template that
    // has any content at all — and a round-trip `t.innerHTML = t.innerHTML` would ERASE it.
    // Read-only, so it takes the fragment if one exists and never materialises one.
    let node = match dom.get_template_contents(node) {
        Some(frag) if dom.tag_name(node) == Some("template") => frag,
        _ => node,
    };
    let mut out = String::new();
    for child in dom.children(node) {
        serialize_node(dom, child, &mut out);
    }
    out
}

/// `element.outerHTML` — the element's own serialization, its tag included.
///
/// `serialize_node` was already exactly this function and had been since the parser was written; it
/// was simply never reachable from JavaScript. Another instance of the recurring shape: the mechanism
/// existed, and nobody had drawn a line from it to the thing that needed it.
pub fn serialize_outer(dom: &Dom, node: NodeId) -> String {
    let mut out = String::new();
    serialize_node(dom, node, &mut out);
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
        // HTML fragment serialization of a PI: `<?` target ` ` data `>` (a single `>`, per spec — NOT
        // the `?>` of XML; the escaping rules also differ, hence this is not the XML `debug_node` form).
        NodeData::ProcessingInstruction { target, data } => {
            out.push_str("<?");
            out.push_str(target);
            out.push(' ');
            out.push_str(data);
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
    // ⚠⚠⚠ **A `<template>`'s `innerHTML` REPLACES ITS TEMPLATE CONTENTS, NEVER ITS CHILD LIST**
    // (DOM Parsing: *"if context is a template element, then set context to the template element's
    // template contents"*). A `<template>` element's own child list is **always empty** in a real
    // browser, and `.content` is the only place its markup lives.
    //
    // Writing to the child list instead was survivable ONLY for a template whose `.content` had
    // never been read: `Dom::template_content` materialises the fragment lazily and MOVES the
    // direct children in on first access, so `t.innerHTML = …; t.content` happened to work. The
    // instant the order reverses — or the template is written twice — the cached fragment goes
    // stale and every later write lands somewhere nothing reads:
    //
    // ```text
    //                                                    Chrome   manuk (before)
    //   innerHTML, then read .content                        1        1
    //   read .content, then innerHTML                        1        0   <- and .childNodes = 1
    //   innerHTML twice (2nd writes two nodes)               2        1   <- the FIRST write's node
    // ```
    //
    // Measured on `pt88.app` (Vue 3): `insertStaticContent` keeps ONE module-level template and
    // writes it on every static block — `Pw.innerHTML = '<svg>…</svg>'; const a = Pw.content;
    // const l = a.firstChild; while (l.firstChild) …` — so the second write onward reads a stale
    // fragment and the page dies on `can't access property "firstChild", l is null`, inside an
    // async render where nothing is listening. That one throw is the whole app.
    //
    // The context tag is read from the ORIGINAL element, before the redirect: a fragment has no tag
    // name, and `template` is exactly the context the tree builder needs for the "in template"
    // insertion mode.
    let context = dom.tag_name(node).unwrap_or("div").to_string();
    let node = if context == "template" {
        dom.template_content(node)
    } else {
        node
    };
    // Detach existing children.
    let existing: Vec<NodeId> = dom.children(node).collect();
    for c in existing {
        dom.detach(c);
    }
    // Context-aware fragment parse: parse `html` as if inside `node`'s element, so
    // table-scoped content (`<tr>`, `<td>`, `<option>`, `<li>`, …) survives instead of
    // being dropped as it would at document level. The parsed nodes are children of the
    // fragment's synthetic root element.
    let fragment = parse_fragment_in(html, &context);
    let root = fragment
        .find_first("html")
        .unwrap_or_else(|| fragment.root());
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
    parse_fragment(
        sink::ArenaSink::new(),
        ParseOpts::default(),
        context,
        vec![],
        false,
    )
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
            // ⚠⚠⚠ **A COPY MUST CARRY THE NAMESPACE, AND THIS ONE DROPPED IT** — so
            // `sink.innerHTML = '<svg>…</svg>'` produced an element whose `namespaceURI` was
            // **xhtml** and whose `nodeName` was **`SVG`** (foreign names are not uppercased), while
            // the DOCUMENT parser had this right all along.
            //
            // ⚠ **WHAT THIS DOES AND DOES NOT BUY, MEASURED RATHER THAN ASSUMED.** The obvious claim
            // — *"an `<svg>` in the HTML namespace has no intrinsic ratio and does not paint"* — is
            // FALSE here and was checked before being written: our layout keys on the TAG, so an
            // injected `<svg viewBox="0 0 200 100">` in a 400px block measured `400x200` and a bare
            // one `300x150`, byte-identical to Chrome, both before and after this line. What it buys
            // is the property `parsedEqMade` in `G_FOREIGN_CONTENT_NS` already names one layer up:
            // **the same markup reached two ways produced two different DOMs.** Every library that
            // branches on `namespaceURI`, matches an `svg|rect` selector, or asks `instanceof
            // SVGElement` — D3, Chart.js, Snap.svg, every icon set that injects markup — was right
            // about parsed SVG and wrong about injected SVG, with nothing reporting a disagreement.
            let ns = el.namespace.clone();
            let attrs: Vec<(String, String)> = el
                .attrs
                .iter()
                .map(|a| (a.name.clone(), a.value.clone()))
                .collect();
            let new = dst.create_element_ns(ns, name);
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

    /// **XML is not HTML with different tags.** Every assertion here is something the HTML parser
    /// gets deliberately, correctly WRONG for XML input — which is what `DOMParser.parseFromString`
    /// was doing to every `text/xml` string it was handed.
    #[test]
    fn xml_is_case_sensitive_and_namespaced() {
        let p = parse_xml(r#"<Foo xmlns="urn:x"><Bar baz="Q"/></Foo>"#);
        assert!(p.well_formed(), "well-formed input: {:?}", p.errors);
        let root = p
            .dom
            .children(p.dom.root())
            .next()
            .expect("document element");
        // The HTML parser lowercases every tag name. XML must not.
        assert_eq!(p.dom.tag_name(root), Some("Foo"));
        let el = p.dom.element(root).expect("element");
        assert_eq!(el.namespace.as_deref(), Some("urn:x"), "xmlns is honoured");
        let bar = p.dom.children(root).next().expect("child");
        assert_eq!(p.dom.tag_name(bar), Some("Bar"), "child keeps its case");
    }

    /// A malformed XML document is NOT recovered from — it is replaced. HTML's error recovery
    /// means `parse("<foo>")` always yields a tree; for XML that same input is fatal.
    #[test]
    fn malformed_xml_becomes_a_parsererror_document() {
        // A mismatched end tag — the error class xml5ever DOES report. See `XmlParse::well_formed`
        // for the two it does not.
        let p = parse_xml("<foo></bar>");
        assert!(!p.well_formed(), "mismatched end tag is not well-formed");

        let mut dst = Dom::new();
        let doc = dst.create_document();
        let errs = parse_xml_into("<foo></bar>", &mut dst, doc);
        assert!(!errs.is_empty());
        let de = dst.children(doc).next().expect("document element");
        assert_eq!(dst.tag_name(de), Some("parsererror"));
        assert_eq!(
            dst.element(de).unwrap().namespace.as_deref(),
            Some(PARSERERROR_NS)
        );
    }

    /// **Pins the known well-formedness gap so it cannot drift unnoticed.** These two inputs are
    /// fatal errors in strict XML and are currently accepted; the table in `XmlParse::well_formed`
    /// says why. If a future xml5ever starts reporting them this test FAILS — which is the point:
    /// the day the gap closes, the loop is told, rather than the limitation quietly outliving its
    /// own documentation.
    #[test]
    fn known_wellformedness_gap_is_pinned() {
        assert!(
            parse_xml("<foo>").well_formed(),
            "unclosed tag still accepted — if this now FAILS the gap has CLOSED: \
             delete this test and assert the parsererror instead"
        );
        assert!(
            parse_xml("<f a=1/>").well_formed(),
            "unquoted attribute value still accepted — same instruction as above"
        );
    }

    /// The clean path must graft the real tree into the CALLER's arena, not a parsererror.
    #[test]
    fn well_formed_xml_grafts_into_the_target_arena() {
        let mut dst = Dom::new();
        let doc = dst.create_document();
        let errs = parse_xml_into("<foo><bar/></foo>", &mut dst, doc);
        assert!(errs.is_empty(), "{errs:?}");
        let de = dst.children(doc).next().expect("document element");
        assert_eq!(dst.tag_name(de), Some("foo"));
        assert_eq!(dst.children(de).count(), 1, "child survives the graft");
    }

    /// A document reports the type it was PARSED as, per document — not one hardcoded answer for
    /// the whole arena. One arena holds many documents, which is why this is keyed by node.
    #[test]
    fn content_type_is_per_document() {
        let mut dom = Dom::new();
        let html_doc = dom.create_document();
        let xml_doc = dom.create_document();
        assert_eq!(dom.content_type(html_doc), "text/html", "the default");
        dom.set_content_type(xml_doc, "image/svg+xml");
        assert_eq!(dom.content_type(xml_doc), "image/svg+xml");
        assert_eq!(
            dom.content_type(html_doc),
            "text/html",
            "the sibling document is untouched"
        );
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
        assert!(
            slotted.contains(&p),
            "the slot must be filled by the light-DOM <p>"
        );
        let slotted_elems: Vec<NodeId> = slotted
            .iter()
            .copied()
            .filter(|&n| dom.is_element(n))
            .collect();
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
        assert!(
            dom.shadow_root(host).is_none(),
            "no shadowrootmode => no shadow root"
        );

        let tpl = dom.find_first("template").unwrap();
        let frag = dom
            .get_template_contents(tpl)
            .expect("template has contents");
        let inner: Vec<&str> = dom.children(frag).filter_map(|c| dom.tag_name(c)).collect();
        assert_eq!(
            inner,
            vec!["b"],
            "contents live in the fragment, not the light DOM"
        );

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
        let slots: Vec<NodeId> = flat
            .iter()
            .copied()
            .filter(|&n| dom.tag_name(n) == Some("slot"))
            .collect();
        assert_eq!(slots.len(), 3);

        let h1 = dom.find_first("h1").unwrap();
        let p = dom.find_first("p").unwrap();

        // named slot gets the h1; default slot gets the p (plus the source's whitespace
        // text nodes, which are slottable per spec — hence the element-only view).
        let elems = |n: NodeId| -> Vec<NodeId> {
            dom.flat_children(n)
                .into_iter()
                .filter(|&c| dom.is_element(c))
                .collect()
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
        let dom =
            parse(r#"<div><template shadowrootmode="open"><slot></slot></template>hello</div>"#);
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
