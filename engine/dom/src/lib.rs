//! manuk-dom — the Document Object Model tree.
//!
//! An arena-backed node tree (indices instead of `Rc`/pointers) so it is cheap to
//! share across the parse, style, and layout passes and trivially `Send`. This
//! crate is deliberately free of any JS-engine dependency: the JS bindings live in
//! `manuk-js` and project *this* tree into the runtime. See the deviation note in
//! `Cargo.toml`.
//!
//! Not yet a spec-complete DOM — it models the node kinds the layout/paint slice
//! needs (Document, Doctype, Element, Text, Comment). The Web API surface
//! (`Node.appendChild`, `Element.classList`, ranges, mutation observers, …) is the
//! large-volume follow-on work called out in CLAUDE.md, and hangs off these types.

use std::fmt::Write as _;

/// Index of a node within a [`Dom`] arena. Stable for the life of the tree.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NodeId(pub usize);

/// An element attribute. Namespaced attributes are folded to their local name for
/// now; the `namespace` slot is reserved so XML/SVG/MathML can populate it later.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attr {
    pub name: String,
    pub value: String,
}

/// Element payload: a lowercased local name plus ordered attributes.
#[derive(Clone, Debug, Default)]
pub struct ElementData {
    /// Lowercased local name, e.g. `div`, `p`, `span`.
    pub name: String,
    pub attrs: Vec<Attr>,
}

impl ElementData {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|a| a.name == name)
            .map(|a| a.value.as_str())
    }

    pub fn id(&self) -> Option<&str> {
        self.attr("id")
    }

    /// Whitespace-split `class` attribute.
    pub fn classes(&self) -> impl Iterator<Item = &str> {
        self.attr("class")
            .unwrap_or_default()
            .split_ascii_whitespace()
    }

    pub fn has_class(&self, class: &str) -> bool {
        self.classes().any(|c| c == class)
    }
}

/// The kind + payload of a node.
#[derive(Clone, Debug)]
pub enum NodeData {
    Document,
    Doctype { name: String },
    Element(ElementData),
    Text(String),
    Comment(String),
}

/// A node and its links. Links are `Option<NodeId>` into the owning arena.
#[derive(Clone, Debug)]
pub struct Node {
    pub parent: Option<NodeId>,
    pub first_child: Option<NodeId>,
    pub last_child: Option<NodeId>,
    pub prev_sibling: Option<NodeId>,
    pub next_sibling: Option<NodeId>,
    pub data: NodeData,
}

impl Node {
    fn new(data: NodeData) -> Self {
        Node {
            parent: None,
            first_child: None,
            last_child: None,
            prev_sibling: None,
            next_sibling: None,
            data,
        }
    }
}

/// A whole document tree. `nodes[0]` is always the [`NodeData::Document`] root.
#[derive(Clone, Debug)]
pub struct Dom {
    nodes: Vec<Node>,
    root: NodeId,
}

impl Default for Dom {
    fn default() -> Self {
        Self::new()
    }
}

impl Dom {
    /// Create an empty document with just the `Document` root node.
    pub fn new() -> Self {
        let mut nodes = Vec::with_capacity(64);
        nodes.push(Node::new(NodeData::Document));
        Dom {
            nodes,
            root: NodeId(0),
        }
    }

    pub fn root(&self) -> NodeId {
        self.root
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        // A fresh Dom always has the Document root, so it is never truly empty;
        // this reports whether the document has any children.
        self.first_child(self.root).is_none()
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.0]
    }

    pub fn node_mut(&mut self, id: NodeId) -> &mut Node {
        &mut self.nodes[id.0]
    }

    pub fn data(&self, id: NodeId) -> &NodeData {
        &self.nodes[id.0].data
    }

    fn alloc(&mut self, data: NodeData) -> NodeId {
        let id = NodeId(self.nodes.len());
        self.nodes.push(Node::new(data));
        id
    }

    pub fn create_element(&mut self, name: impl Into<String>) -> NodeId {
        self.alloc(NodeData::Element(ElementData {
            name: name.into(),
            attrs: Vec::new(),
        }))
    }

    pub fn create_text(&mut self, text: impl Into<String>) -> NodeId {
        self.alloc(NodeData::Text(text.into()))
    }

    pub fn create_comment(&mut self, text: impl Into<String>) -> NodeId {
        self.alloc(NodeData::Comment(text.into()))
    }

    pub fn create_doctype(&mut self, name: impl Into<String>) -> NodeId {
        self.alloc(NodeData::Doctype { name: name.into() })
    }

    pub fn set_attr(&mut self, id: NodeId, name: impl Into<String>, value: impl Into<String>) {
        if let NodeData::Element(el) = &mut self.nodes[id.0].data {
            let name = name.into();
            if let Some(a) = el.attrs.iter_mut().find(|a| a.name == name) {
                a.value = value.into();
            } else {
                el.attrs.push(Attr {
                    name,
                    value: value.into(),
                });
            }
        }
    }

    /// Append `child` as the last child of `parent`, unlinking it from any old
    /// position first.
    pub fn append_child(&mut self, parent: NodeId, child: NodeId) {
        self.detach(child);
        self.nodes[child.0].parent = Some(parent);
        match self.nodes[parent.0].last_child {
            Some(last) => {
                self.nodes[last.0].next_sibling = Some(child);
                self.nodes[child.0].prev_sibling = Some(last);
                self.nodes[parent.0].last_child = Some(child);
            }
            None => {
                self.nodes[parent.0].first_child = Some(child);
                self.nodes[parent.0].last_child = Some(child);
            }
        }
    }

    /// Remove `child` from its parent, leaving it a detached root of its subtree.
    pub fn detach(&mut self, child: NodeId) {
        let (parent, prev, next) = {
            let n = &self.nodes[child.0];
            (n.parent, n.prev_sibling, n.next_sibling)
        };
        if let Some(p) = prev {
            self.nodes[p.0].next_sibling = next;
        }
        if let Some(n) = next {
            self.nodes[n.0].prev_sibling = prev;
        }
        if let Some(par) = parent {
            if self.nodes[par.0].first_child == Some(child) {
                self.nodes[par.0].first_child = next;
            }
            if self.nodes[par.0].last_child == Some(child) {
                self.nodes[par.0].last_child = prev;
            }
        }
        let n = &mut self.nodes[child.0];
        n.parent = None;
        n.prev_sibling = None;
        n.next_sibling = None;
    }

    pub fn first_child(&self, id: NodeId) -> Option<NodeId> {
        self.nodes[id.0].first_child
    }

    pub fn next_sibling(&self, id: NodeId) -> Option<NodeId> {
        self.nodes[id.0].next_sibling
    }

    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.nodes[id.0].parent
    }

    /// Iterator over the direct children of `id`.
    pub fn children(&self, id: NodeId) -> Children<'_> {
        Children {
            dom: self,
            next: self.nodes[id.0].first_child,
        }
    }

    pub fn is_element(&self, id: NodeId) -> bool {
        matches!(self.nodes[id.0].data, NodeData::Element(_))
    }

    pub fn element(&self, id: NodeId) -> Option<&ElementData> {
        match &self.nodes[id.0].data {
            NodeData::Element(e) => Some(e),
            _ => None,
        }
    }

    pub fn tag_name(&self, id: NodeId) -> Option<&str> {
        self.element(id).map(|e| e.name.as_str())
    }

    /// The first element with the given lowercased tag name, searched depth-first
    /// from the document root. Handy for `<html>`/`<body>`/`<title>` lookups.
    pub fn find_first(&self, name: &str) -> Option<NodeId> {
        self.descendants(self.root)
            .find(|&id| self.tag_name(id) == Some(name))
    }

    /// Depth-first (pre-order) descendants of `id`, not including `id` itself.
    pub fn descendants(&self, id: NodeId) -> Descendants<'_> {
        // Seed with all direct children (reversed) so we pop them left-to-right.
        // Seeding with only `first_child` would drop its siblings' subtrees.
        let mut stack: Vec<NodeId> = self.children(id).collect();
        stack.reverse();
        Descendants { dom: self, stack }
    }

    /// Concatenated text content of the subtree rooted at `id`.
    pub fn text_content(&self, id: NodeId) -> String {
        let mut out = String::new();
        if let NodeData::Text(t) = &self.nodes[id.0].data {
            out.push_str(t);
        }
        for d in self.descendants(id) {
            if let NodeData::Text(t) = &self.nodes[d.0].data {
                out.push_str(t);
            }
        }
        out
    }

    /// A compact indented dump of the tree, for tests and debugging.
    pub fn to_debug_string(&self) -> String {
        let mut out = String::new();
        self.debug_node(self.root, 0, &mut out);
        out
    }

    fn debug_node(&self, id: NodeId, depth: usize, out: &mut String) {
        for _ in 0..depth {
            out.push_str("  ");
        }
        match &self.nodes[id.0].data {
            NodeData::Document => out.push_str("#document"),
            NodeData::Doctype { name } => {
                let _ = write!(out, "<!DOCTYPE {name}>");
            }
            NodeData::Element(e) => {
                let _ = write!(out, "<{}", e.name);
                for a in &e.attrs {
                    let _ = write!(out, " {}=\"{}\"", a.name, a.value);
                }
                out.push('>');
            }
            NodeData::Text(t) => {
                let trimmed = t.trim();
                let _ = write!(out, "#text {:?}", trimmed);
            }
            NodeData::Comment(c) => {
                let _ = write!(out, "<!-- {} -->", c.trim());
            }
        }
        out.push('\n');
        for child in self.children(id) {
            self.debug_node(child, depth + 1, out);
        }
    }
}

/// Iterator over direct children — see [`Dom::children`].
pub struct Children<'a> {
    dom: &'a Dom,
    next: Option<NodeId>,
}

impl Iterator for Children<'_> {
    type Item = NodeId;
    fn next(&mut self) -> Option<NodeId> {
        let cur = self.next?;
        self.next = self.dom.nodes[cur.0].next_sibling;
        Some(cur)
    }
}

/// Pre-order descendant iterator — see [`Dom::descendants`].
pub struct Descendants<'a> {
    dom: &'a Dom,
    stack: Vec<NodeId>,
}

impl Iterator for Descendants<'_> {
    type Item = NodeId;
    fn next(&mut self) -> Option<NodeId> {
        let cur = self.stack.pop()?;
        // Push children in reverse so we pop them left-to-right (document order).
        let mut kids: Vec<NodeId> = self.dom.children(cur).collect();
        kids.reverse();
        self.stack.extend(kids);
        Some(cur)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_and_traverse() {
        let mut dom = Dom::new();
        let html = dom.create_element("html");
        let body = dom.create_element("body");
        let p = dom.create_element("p");
        dom.set_attr(p, "class", "lead intro");
        let text = dom.create_text("Hello");
        dom.append_child(dom.root(), html);
        dom.append_child(html, body);
        dom.append_child(body, p);
        dom.append_child(p, text);

        assert_eq!(dom.tag_name(html), Some("html"));
        assert_eq!(dom.find_first("p"), Some(p));
        assert_eq!(dom.text_content(dom.root()), "Hello");
        assert!(dom.element(p).unwrap().has_class("intro"));
        assert_eq!(dom.children(body).count(), 1);
        // html, body, p, text
        assert_eq!(dom.descendants(dom.root()).count(), 4);
    }

    #[test]
    fn detach_relinks_siblings() {
        let mut dom = Dom::new();
        let a = dom.create_element("a");
        let b = dom.create_element("b");
        let c = dom.create_element("c");
        for n in [a, b, c] {
            dom.append_child(dom.root(), n);
        }
        dom.detach(b);
        let remaining: Vec<_> = dom.children(dom.root()).collect();
        assert_eq!(remaining, vec![a, c]);
        assert_eq!(dom.next_sibling(a), Some(c));
    }
}
