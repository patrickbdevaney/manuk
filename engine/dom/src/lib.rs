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
    /// N3 — a shadow root. It is **not** a child of its host: it is the root of a separate
    /// tree, reachable via [`Dom::shadow_root`]. Its `parent` link points at the host so
    /// upward walks work, but the host's `children()` never yields it.
    ShadowRoot { mode: ShadowRootMode },
    /// A `DocumentFragment` — a `<template>`'s contents. Also not a child of the template.
    Fragment,
}

/// `<template shadowrootmode>` / `attachShadow({mode})`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShadowRootMode {
    Open,
    Closed,
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
    /// N3 — the shadow root attached to this element, if any. Deliberately *not* a child
    /// link: shadow content lives in its own tree and only appears in the **flat tree**.
    shadow_root: Option<NodeId>,
    /// N3 — a `<template>`'s contents fragment, if any. Also not a child link.
    template_contents: Option<NodeId>,
    /// Incremental-layout (A2) double dirty-bit. `dirty` = this node changed and needs
    /// restyle/relayout; `dirty_descendants` = a descendant is dirty (the summary bit
    /// that lets a traversal skip any subtree whose summary bit is clear).
    dirty: bool,
    dirty_descendants: bool,
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
            shadow_root: None,
            template_contents: None,
            // Freshly-created nodes start dirty: they have never been laid out.
            dirty: true,
            dirty_descendants: false,
        }
    }
}

/// A whole document tree. `nodes[0]` is always the [`NodeData::Document`] root.
#[derive(Clone, Debug)]
pub struct Dom {
    nodes: Vec<Node>,
    root: NodeId,
    /// Set by structural mutations (`append_child`/`detach`) since the last clean
    /// pass — a box was added or removed, so incremental relayout must reflow (an
    /// attribute-only change, by contrast, is classified by the style diff).
    structure_changed: bool,
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
            structure_changed: false,
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

    // ---- N3: shadow DOM ----

    /// Attach a shadow root to `host`, returning it. Idempotent: a host that already has
    /// one keeps it (the spec throws; we return the existing root, which is what a parser
    /// wants when a second `<template shadowrootmode>` appears).
    ///
    /// The shadow root is **not** a child of the host — `children(host)` never yields it.
    /// It only appears in the [flat tree](Self::flat_children).
    pub fn attach_shadow(&mut self, host: NodeId, mode: ShadowRootMode) -> NodeId {
        if let Some(existing) = self.nodes[host.0].shadow_root {
            return existing;
        }
        let sr = self.alloc(NodeData::ShadowRoot { mode });
        self.nodes[sr.0].parent = Some(host);
        self.nodes[host.0].shadow_root = Some(sr);
        self.structure_changed = true;
        self.mark_dirty(host);
        sr
    }

    pub fn shadow_root(&self, host: NodeId) -> Option<NodeId> {
        self.nodes[host.0].shadow_root
    }

    pub fn is_shadow_root(&self, id: NodeId) -> bool {
        matches!(self.nodes[id.0].data, NodeData::ShadowRoot { .. })
    }

    pub fn shadow_root_mode(&self, id: NodeId) -> Option<ShadowRootMode> {
        match self.nodes[id.0].data {
            NodeData::ShadowRoot { mode } => Some(mode),
            _ => None,
        }
    }

    /// The element hosting this shadow root.
    pub fn shadow_host(&self, shadow_root: NodeId) -> Option<NodeId> {
        if self.is_shadow_root(shadow_root) {
            self.nodes[shadow_root.0].parent
        } else {
            None
        }
    }

    /// Create (or fetch) a `<template>`'s contents fragment. Also not a child link.
    pub fn template_contents(&mut self, template: NodeId) -> NodeId {
        if let Some(f) = self.nodes[template.0].template_contents {
            return f;
        }
        let frag = self.alloc(NodeData::Fragment);
        self.nodes[frag.0].parent = Some(template);
        self.nodes[template.0].template_contents = Some(frag);
        frag
    }

    pub fn get_template_contents(&self, template: NodeId) -> Option<NodeId> {
        self.nodes[template.0].template_contents
    }

    /// Point a `<template>`'s contents at `node`.
    ///
    /// The declarative-shadow-DOM parse aims a template's contents **at the shadow root**,
    /// so everything the parser inserts into the template lands directly in the shadow
    /// tree. (The tree builder attaches the shadow root at the *start* tag, then keeps
    /// inserting into `get_template_contents`; without this the shadow root would stay
    /// empty.)
    pub fn set_template_contents(&mut self, template: NodeId, node: NodeId) {
        self.nodes[template.0].template_contents = Some(node);
    }

    fn is_slot(&self, id: NodeId) -> bool {
        self.tag_name(id) == Some("slot")
    }

    /// The light-DOM children of `host` assigned to `slot`, per the slot-assignment
    /// algorithm: a child's `slot` attribute names its slot; the unnamed `<slot>` takes
    /// the children with no `slot` attribute.
    pub fn assigned_nodes(&self, slot: NodeId) -> Vec<NodeId> {
        let Some(shadow) = self.enclosing_shadow_root(slot) else {
            return Vec::new();
        };
        let Some(host) = self.shadow_host(shadow) else {
            return Vec::new();
        };
        let slot_name = self
            .element(slot)
            .and_then(|e| e.attr("name"))
            .unwrap_or("");
        self.children(host)
            .filter(|&c| {
                let child_slot = self.element(c).and_then(|e| e.attr("slot")).unwrap_or("");
                child_slot == slot_name
            })
            .collect()
    }

    /// The `<slot>` a light-DOM child of a shadow host is assigned to, if any. The
    /// inverse of [`Self::assigned_nodes`], and what `::slotted()` matches on.
    pub fn assigned_slot(&self, node: NodeId) -> Option<NodeId> {
        let host = self.parent(node)?;
        let shadow = self.shadow_root(host)?;
        let want = self.element(node).and_then(|e| e.attr("slot")).unwrap_or("");
        self.descendants(shadow).find(|&s| {
            self.tag_name(s) == Some("slot")
                && self.element(s).and_then(|e| e.attr("name")).unwrap_or("") == want
        })
    }

    /// Every shadow root in the arena, in creation order.
    pub fn all_shadow_roots(&self) -> Vec<NodeId> {
        (0..self.nodes.len())
            .map(NodeId)
            .filter(|&n| self.is_shadow_root(n))
            .collect()
    }

    /// The shadow root that `node` lives inside, if any (walks up the node tree).
    pub fn enclosing_shadow_root(&self, node: NodeId) -> Option<NodeId> {
        let mut cur = Some(node);
        while let Some(n) = cur {
            if self.is_shadow_root(n) {
                return Some(n);
            }
            cur = self.nodes[n.0].parent;
        }
        None
    }

    /// **The flat tree** — what layout, style, and the a11y tree actually walk.
    ///
    /// * A host with a shadow root yields the shadow root's children, not its own.
    /// * A `<slot>` yields its **assigned** light-DOM nodes, or its fallback children when
    ///   nothing is assigned.
    /// * Everything else yields its ordinary children.
    ///
    /// The light-DOM children of a host remain its children in the *node* tree; they are
    /// merely rendered where the slot is. Those are two different trees, and conflating
    /// them is the classic shadow-DOM bug.
    pub fn flat_children(&self, node: NodeId) -> Vec<NodeId> {
        if let Some(sr) = self.shadow_root(node) {
            return self.children(sr).collect();
        }
        if self.is_slot(node) {
            let assigned = self.assigned_nodes(node);
            if !assigned.is_empty() {
                return assigned;
            }
            // Fallback content: the slot's own children.
            return self.children(node).collect();
        }
        self.children(node).collect()
    }

    /// Remove an attribute, returning whether it was present. Needed to *unset*
    /// boolean content attributes (`checked`, `hidden`) — setting them to `""` still
    /// counts as present, per HTML.
    pub fn remove_attr(&mut self, id: NodeId, name: &str) -> bool {
        if let NodeData::Element(el) = &mut self.nodes[id.0].data {
            if let Some(i) = el.attrs.iter().position(|a| a.name == name) {
                el.attrs.remove(i);
                return true;
            }
        }
        false
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
            self.mark_dirty(id);
        }
    }

    // -- Incremental-layout dirty tracking (A2) -----------------------------

    /// Mark `node` dirty and propagate the summary bit
    /// ([`has_dirty_descendants`](Self::has_dirty_descendants)) up its ancestor chain,
    /// stopping as soon as an ancestor already carries it. This is the double-dirty-bit
    /// model: a later traversal restyles/relayouts only dirty nodes and descends only
    /// into subtrees whose summary bit is set.
    pub fn mark_dirty(&mut self, node: NodeId) {
        if node.0 >= self.nodes.len() {
            return;
        }
        self.nodes[node.0].dirty = true;
        let mut cur = self.nodes[node.0].parent;
        while let Some(p) = cur {
            if self.nodes[p.0].dirty_descendants {
                break;
            }
            self.nodes[p.0].dirty_descendants = true;
            cur = self.nodes[p.0].parent;
        }
    }

    /// Has `node` itself changed since the last clean pass?
    pub fn is_dirty(&self, node: NodeId) -> bool {
        self.nodes.get(node.0).is_some_and(|n| n.dirty)
    }

    /// Does `node`'s subtree contain a dirty node (the skip-this-subtree summary bit)?
    pub fn has_dirty_descendants(&self, node: NodeId) -> bool {
        self.nodes.get(node.0).is_some_and(|n| n.dirty_descendants)
    }

    /// Is `node` clean *and* free of dirty descendants — i.e. a traversal may skip its
    /// whole subtree and reuse cached layout/paint?
    pub fn subtree_clean(&self, node: NodeId) -> bool {
        self.nodes
            .get(node.0)
            .is_some_and(|n| !n.dirty && !n.dirty_descendants)
    }

    /// Clear both dirty bits on a single node (call after processing it).
    pub fn clear_dirty(&mut self, node: NodeId) {
        if let Some(n) = self.nodes.get_mut(node.0) {
            n.dirty = false;
            n.dirty_descendants = false;
        }
    }

    /// Did a structural mutation (`append_child`/`detach`) occur since the last clean
    /// pass? Structural changes add/remove boxes, so incremental relayout must reflow.
    pub fn structure_changed(&self) -> bool {
        self.structure_changed
    }

    /// Clear every dirty bit in the tree (call after a full clean layout pass).
    pub fn clear_all_dirty(&mut self) {
        for n in &mut self.nodes {
            n.dirty = false;
            n.dirty_descendants = false;
        }
        self.structure_changed = false;
    }

    /// Append `child` as the last child of `parent`, unlinking it from any old
    /// position first.
    /// Detach `child` from `parent`, returning whether it was actually a child.
    ///
    /// The node stays in the arena (its `NodeId` remains valid) but is unlinked from the
    /// tree, so a caller can re-attach it elsewhere — which is exactly what E3's
    /// translation re-injection does when it rebuilds a block around its original inline
    /// elements. Removing a node that is *not* a child of `parent` is a no-op returning
    /// `false`, never a silent detach from somewhere else.
    pub fn remove_child(&mut self, parent: NodeId, child: NodeId) -> bool {
        if self.nodes[child.0].parent != Some(parent) {
            return false;
        }
        self.detach(child);
        self.structure_changed = true;
        self.mark_dirty(parent);
        true
    }

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
        // Structural change: the child (and thus the parent's subtree) is dirty.
        self.structure_changed = true;
        self.mark_dirty(child);
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
        // The old parent's child set changed: its subtree needs relayout.
        if let Some(par) = parent {
            self.structure_changed = true;
            self.mark_dirty(par);
        }
    }

    pub fn first_child(&self, id: NodeId) -> Option<NodeId> {
        self.nodes[id.0].first_child
    }

    pub fn next_sibling(&self, id: NodeId) -> Option<NodeId> {
        self.nodes[id.0].next_sibling
    }

    pub fn last_child(&self, id: NodeId) -> Option<NodeId> {
        self.nodes[id.0].last_child
    }

    pub fn prev_sibling(&self, id: NodeId) -> Option<NodeId> {
        self.nodes[id.0].prev_sibling
    }

    /// If `id` is a text node, append `text` to it and report `true`. Used by the parser
    /// to merge adjacent text runs — two sibling text nodes would otherwise produce two
    /// inline runs for what is one string.
    pub fn append_text_to(&mut self, id: NodeId, text: &str) -> bool {
        if let NodeData::Text(t) = &mut self.nodes[id.0].data {
            t.push_str(text);
            self.mark_dirty(id);
            return true;
        }
        false
    }

    /// Insert `new_node` into `parent`'s child list immediately before `sibling`.
    pub fn insert_before(&mut self, parent: NodeId, new_node: NodeId, sibling: NodeId) {
        debug_assert_eq!(self.nodes[sibling.0].parent, Some(parent));
        self.detach(new_node);
        let prev = self.nodes[sibling.0].prev_sibling;
        self.nodes[new_node.0].parent = Some(parent);
        self.nodes[new_node.0].prev_sibling = prev;
        self.nodes[new_node.0].next_sibling = Some(sibling);
        self.nodes[sibling.0].prev_sibling = Some(new_node);
        match prev {
            Some(p) => self.nodes[p.0].next_sibling = Some(new_node),
            None => self.nodes[parent.0].first_child = Some(new_node),
        }
        self.structure_changed = true;
        self.mark_dirty(new_node);
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
            NodeData::ShadowRoot { mode } => {
                let _ = write!(out, "#shadow-root ({mode:?})");
            }
            NodeData::Fragment => out.push_str("#document-fragment"),
            NodeData::Text(t) => {
                let trimmed = t.trim();
                let _ = write!(out, "#text {trimmed:?}");
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
    fn double_dirty_bit_propagates_and_clears() {
        // Build html > body > p > text, then start clean.
        let mut dom = Dom::new();
        let html = dom.create_element("html");
        let body = dom.create_element("body");
        let p = dom.create_element("p");
        let sib = dom.create_element("div"); // a clean sibling of p under body
        dom.append_child(dom.root(), html);
        dom.append_child(html, body);
        dom.append_child(body, p);
        dom.append_child(body, sib);
        dom.clear_all_dirty();
        assert!(dom.subtree_clean(dom.root()), "tree clean after clear");

        // Mutate p: it goes dirty, and every ancestor gets the summary bit.
        dom.set_attr(p, "class", "highlight");
        assert!(dom.is_dirty(p));
        assert!(dom.has_dirty_descendants(body));
        assert!(dom.has_dirty_descendants(html));
        assert!(dom.has_dirty_descendants(dom.root()));
        // p itself carries no descendant dirtiness; the sibling subtree stays clean.
        assert!(!dom.has_dirty_descendants(p));
        assert!(
            dom.subtree_clean(sib),
            "unrelated sibling subtree stays clean"
        );
        // body changed style but not structure → body itself is not `dirty`.
        assert!(!dom.is_dirty(body));

        // Clearing p's bits and the summary chain returns the tree to clean.
        dom.clear_all_dirty();
        assert!(dom.subtree_clean(dom.root()));
    }

    #[test]
    fn structural_mutation_marks_parent_dirty() {
        let mut dom = Dom::new();
        let body = dom.create_element("body");
        let a = dom.create_element("a");
        dom.append_child(dom.root(), body);
        dom.append_child(body, a);
        dom.clear_all_dirty();

        // Appending a new child marks the child dirty + body's summary bit.
        let b = dom.create_element("b");
        dom.append_child(body, b);
        assert!(dom.is_dirty(b));
        assert!(dom.has_dirty_descendants(body));

        dom.clear_all_dirty();
        // Detaching marks the (old) parent dirty for relayout.
        dom.detach(a);
        assert!(dom.is_dirty(body), "detach dirties the old parent");
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
