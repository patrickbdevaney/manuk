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

/// N1 — the shared session-history (navigation stack) model. Lives in this leaf crate so
/// both `manuk-page` and `manuk-js` (the History API host) can use it without a dependency
/// cycle. It has no DOM dependency itself.
pub mod history;

/// A handle to a node in a [`Dom`] arena. It packs a **generation** (high 32 bits) and a **slot
/// index** (low 32 bits), so a handle to a removed node whose slot was later reused is detected as
/// stale (its generation no longer matches) instead of silently aliasing the new occupant. For a
/// never-reused (generation-0) node the packed value equals the bare index, so old code and
/// serialized handles stay compatible.
///
/// **The backing type is `u64`, NOT `usize`, and that is load-bearing for `wasm32`.** On a 32-bit
/// target `usize` is 32 bits, so `generation << 32` overflows and the crate does not even compile —
/// which is exactly what the in-browser demo's `wasm32-unknown-unknown` build surfaced. `u64` is
/// identical to `usize` on 64-bit platforms and correct on 32-bit ones, so the arena is now
/// pointer-width-independent (this also matters for the ARM/cross-platform target).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NodeId(pub u64);

impl NodeId {
    const INDEX_MASK: u64 = 0xFFFF_FFFF;

    /// Pack a slot index + generation into a handle.
    #[inline]
    pub(crate) fn pack(index: usize, generation: u32) -> NodeId {
        NodeId((generation as u64) << 32 | (index as u64 & Self::INDEX_MASK))
    }

    /// The arena slot this handle points at (its low 32 bits).
    #[inline]
    pub fn index(self) -> usize {
        (self.0 & Self::INDEX_MASK) as usize
    }

    /// The generation this handle was minted at (its high 32 bits).
    #[inline]
    pub fn generation(self) -> u32 {
        (self.0 >> 32) as u32
    }
}

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
    /// The element's **namespace**, or `None` for the HTML namespace (the overwhelmingly common case, so
    /// it costs a null and not a string).
    ///
    /// This is not decoration. `document.createElementNS(SVG_NS, 'linearGradient')` must keep its **case**
    /// (`localName` is `linearGradient`, not `lineargradient`) and must report its namespace — and until
    /// now `createElementNS` threw the namespace away entirely and returned an HTML element. Everything
    /// downstream then lied: `namespaceURI` said XHTML, `localName` was `undefined`, and `tagName` was
    /// uppercased when it must not be. WPT's `Document-createElementNS.html` is **596 subtests** of
    /// exactly this, and it scored zero.
    pub namespace: Option<String>,
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
    Doctype {
        name: String,
    },
    Element(ElementData),
    Text(String),
    Comment(String),
    /// A **ProcessingInstruction** (`<?target data?>`) — nodeType 7. It is a `CharacterData` node
    /// (its `data` is the instruction body), distinguished from `Comment` by carrying a `target`
    /// (`nodeName`). `document.createProcessingInstruction(target, data)` mints one; ~88 WPT `dom/nodes`
    /// subtests fail only because the factory did not exist and every later operation on the node threw.
    ProcessingInstruction {
        target: String,
        data: String,
    },
    /// N3 — a shadow root. It is **not** a child of its host: it is the root of a separate
    /// tree, reachable via [`Dom::shadow_root`]. Its `parent` link points at the host so
    /// upward walks work, but the host's `children()` never yields it.
    ShadowRoot {
        mode: ShadowRootMode,
    },
    /// A **DocumentFragment** — a `<template>`'s contents, and what every framework builds a subtree
    /// in before committing it.
    ///
    /// Its defining property is not that it holds children; it is what happens when you INSERT it:
    /// **the fragment's children move, and the fragment itself does not.** Get that wrong and you
    /// insert an inert wrapper where the content should be — which is exactly what lit-html got, and
    /// why it rendered an empty component with no error at all.
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
    /// Per-slot generation, parallel to `nodes`. Bumped when a slot is freed, so a stale
    /// [`NodeId`] into a reused slot fails the generation check in [`Dom::is_alive`].
    generations: Vec<u32>,
    /// Whether each slot currently holds a live node (vs. a freed tombstone awaiting reuse).
    alive: Vec<bool>,
    /// Freed slot indices available for reuse by [`Dom::alloc`] (LIFO).
    free: Vec<usize>,
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
            generations: vec![0],
            alive: vec![true],
            free: Vec::new(),
            root: NodeId(0),
            structure_changed: false,
        }
    }

    /// Whether `id` still points at a live node of the generation it was minted at. A
    /// handle to a removed (and possibly reused) node returns `false`. Public accessors
    /// that return `Option` gate on this so a stale handle reads as "no such node" rather
    /// than aliasing whatever now occupies the slot.
    #[inline]
    pub fn is_alive(&self, id: NodeId) -> bool {
        let i = id.index();
        i < self.nodes.len() && self.alive[i] && self.generations[i] == id.generation()
    }

    /// The live handle for arena slot `index`, if it currently holds a node. Lets an
    /// index-keyed external reference (e.g. a JS reflector) recover the current generation.
    #[inline]
    pub fn id_at_index(&self, index: usize) -> Option<NodeId> {
        (index < self.nodes.len() && self.alive[index])
            .then(|| NodeId::pack(index, self.generations[index]))
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
        &self.nodes[id.index()]
    }

    pub fn node_mut(&mut self, id: NodeId) -> &mut Node {
        &mut self.nodes[id.index()]
    }

    /// The character data of a `Text` or `Comment` node — `CharacterData.data` / `Node.nodeValue`.
    ///
    /// Lit reads `comment.data` on every node a TreeWalker hands it, looking for its binding markers.
    /// Without it: `can't access property "indexOf", i.data is undefined`, thrown inside an async
    /// render, which is why it surfaced as an unhandled promise rejection and not as anything legible.
    pub fn character_data(&self, id: NodeId) -> Option<&str> {
        // `.get()`, not `data(id)`: reachable from JS with a handle from another arena, where indexing
        // panics — and a panic inside an `extern "C"` native ABORTS THE PROCESS.
        match self.nodes.get(id.index()).map(|n| &n.data) {
            Some(NodeData::Text(t)) | Some(NodeData::Comment(t)) => Some(t.as_str()),
            // A PI is CharacterData: `.data`/`nodeValue`/`textContent` read its instruction body.
            Some(NodeData::ProcessingInstruction { data, .. }) => Some(data.as_str()),
            _ => None,
        }
    }

    /// Set the character data of a `Text`, `Comment` or `ProcessingInstruction` node. Returns whether
    /// it applied.
    pub fn set_character_data(&mut self, id: NodeId, value: impl Into<String>) -> bool {
        match &mut self.nodes[id.index()].data {
            NodeData::Text(t) | NodeData::Comment(t) => {
                *t = value.into();
                self.mark_dirty(id);
                true
            }
            NodeData::ProcessingInstruction { data, .. } => {
                *data = value.into();
                self.mark_dirty(id);
                true
            }
            _ => false,
        }
    }

    pub fn data(&self, id: NodeId) -> &NodeData {
        &self.nodes[id.index()].data
    }

    fn alloc(&mut self, data: NodeData) -> NodeId {
        if let Some(index) = self.free.pop() {
            // Reuse a freed slot: its generation was already bumped at free time, so the
            // handle we mint here differs from any stale handle to the old occupant.
            self.nodes[index] = Node::new(data);
            self.alive[index] = true;
            NodeId::pack(index, self.generations[index])
        } else {
            let index = self.nodes.len();
            self.nodes.push(Node::new(data));
            self.generations.push(0);
            self.alive.push(true);
            NodeId::pack(index, 0)
        }
    }

    /// Free a node's slot for reuse, bumping its generation so outstanding handles to it
    /// become stale. The node's links are left as-is (callers unlink first); only the slot
    /// is reclaimed. Not freed if already dead (idempotent).
    fn free_slot(&mut self, id: NodeId) {
        let i = id.index();
        if i < self.nodes.len() && self.alive[i] && self.generations[i] == id.generation() {
            self.alive[i] = false;
            self.generations[i] = self.generations[i].wrapping_add(1);
            self.free.push(i);
        }
    }

    /// Recursively free a node and its entire subtree (child links + shadow root +
    /// template contents), reclaiming every slot. Used when a subtree is detached and
    /// discarded so long-lived pages don't leak arena slots.
    fn free_subtree(&mut self, id: NodeId) {
        if !self.is_alive(id) {
            return;
        }
        let mut child = self.nodes[id.index()].first_child;
        while let Some(c) = child {
            let next = self.nodes[c.index()].next_sibling;
            self.free_subtree(c);
            child = next;
        }
        if let Some(sr) = self.nodes[id.index()].shadow_root {
            self.free_subtree(sr);
        }
        if let Some(tc) = self.nodes[id.index()].template_contents {
            self.free_subtree(tc);
        }
        self.free_slot(id);
    }

    /// Permanently discard a node and its whole subtree: detach it from its parent, then
    /// reclaim every slot, bumping each generation so any outstanding handle into the
    /// subtree becomes stale (fails [`Dom::is_alive`]). Use only when the subtree is
    /// **known** to be thrown away — not moved or re-inserted (the parser's reparenting and
    /// JS `removeChild`-then-append both re-insert, so those must keep using `remove_child`).
    /// This is the safe seam for reclaiming arena slots on long-lived pages.
    pub fn discard_subtree(&mut self, node: NodeId) {
        if !self.is_alive(node) {
            return;
        }
        self.detach(node);
        self.free_subtree(node);
    }

    pub fn create_element(&mut self, name: impl Into<String>) -> NodeId {
        self.alloc(NodeData::Element(ElementData {
            name: name.into(),
            attrs: Vec::new(),
            namespace: None,
        }))
    }

    /// `document.createElementNS(ns, qualifiedName)`. `ns` of `None` (or the HTML namespace) behaves
    /// exactly like [`create_element`]; anything else is a foreign element, and its **case is preserved**.
    pub fn create_element_ns(&mut self, ns: Option<String>, name: impl Into<String>) -> NodeId {
        let ns = ns.filter(|n| n != "http://www.w3.org/1999/xhtml" && !n.is_empty());
        self.alloc(NodeData::Element(ElementData {
            name: name.into(),
            attrs: Vec::new(),
            namespace: ns,
        }))
    }

    /// The element's namespace, or `None` for HTML.
    pub fn namespace(&self, node: NodeId) -> Option<&str> {
        match self.nodes.get(node.index()).map(|n| &n.data) {
            Some(NodeData::Element(e)) => e.namespace.as_deref(),
            _ => None,
        }
    }

    pub fn create_text(&mut self, text: impl Into<String>) -> NodeId {
        self.alloc(NodeData::Text(text.into()))
    }

    /// A DocumentFragment. See [`NodeData::Fragment`] — inserting it moves its CHILDREN.
    pub fn create_fragment(&mut self) -> NodeId {
        self.alloc(NodeData::Fragment)
    }

    /// **A second, detached DOCUMENT in the same arena** — what `createHTMLDocument()` returns.
    ///
    /// One arena, several roots. A document is not special storage; it is a node whose *type* is
    /// `Document`, and everything that already walks the tree works on it unchanged.
    pub fn create_document(&mut self) -> NodeId {
        self.alloc(NodeData::Document)
    }

    /// **Is `maybe_ancestor` an INCLUSIVE ancestor of `node`?** (i.e. itself, or any ancestor.)
    ///
    /// This is the check that stands between the DOM and an **infinite loop**. The spec's pre-insertion
    /// validity step *"if node is a host-including inclusive ancestor of parent, throw
    /// HierarchyRequestError"* exists because inserting a node into its own descendant makes the tree a
    /// **cycle** — and every subsequent `children()` walk spins forever.
    pub fn is_inclusive_ancestor(&self, maybe_ancestor: NodeId, node: NodeId) -> bool {
        let mut cur = Some(node);
        while let Some(n) = cur {
            if n == maybe_ancestor {
                return true;
            }
            cur = self.parent(n);
        }
        false
    }

    /// Is this node a Document? Decides whether its JS reflector gets the *document* method set
    /// (`createElement`, `getElementById`, `documentElement`, `body`, …) rather than the element one.
    pub fn is_document(&self, id: NodeId) -> bool {
        matches!(
            self.nodes.get(id.index()).map(|n| &n.data),
            Some(NodeData::Document)
        )
    }

    pub fn is_fragment(&self, id: NodeId) -> bool {
        // Bounds-safe on purpose: this is reachable from JS via `appendChild`/`insertBefore`, and a raw
        // index from a stale reflector used to panic here — inside an `extern "C"` native, where a panic
        // cannot unwind and therefore ABORTS THE PROCESS.
        match self.nodes.get(id.index()) {
            Some(n) => matches!(n.data, NodeData::Fragment),
            None => false,
        }
    }

    /// **`<template>.content`** — a real fragment holding the template's children.
    ///
    /// Created lazily, once, and the template's children are MOVED into it. That is safe precisely
    /// because a `<template>`'s children never render anyway (it is `display:none` by definition), and
    /// it is what makes `importNode(tpl.content, true)` followed by `insertBefore(fragment, ...)` —
    /// the exact sequence lit-html commits every template through — insert the CONTENT rather than an
    /// inert `<template>` wrapper.
    pub fn template_content(&mut self, template: NodeId) -> NodeId {
        if let Some(existing) = self.nodes[template.index()].shadow_root {
            if self.is_fragment(existing) {
                return existing;
            }
        }
        let frag = self.create_fragment();
        let kids: Vec<NodeId> = self.children(template).collect();
        for k in kids {
            self.append_child(frag, k);
        }
        self.nodes[frag.index()].parent = Some(template);
        self.nodes[template.index()].shadow_root = Some(frag);
        frag
    }

    pub fn create_comment(&mut self, text: impl Into<String>) -> NodeId {
        self.alloc(NodeData::Comment(text.into()))
    }

    /// `document.createProcessingInstruction(target, data)` — a detached PI node. Target/data validity
    /// is enforced at the binding (it throws `InvalidCharacterError`); the arena only stores.
    pub fn create_processing_instruction(
        &mut self,
        target: impl Into<String>,
        data: impl Into<String>,
    ) -> NodeId {
        self.alloc(NodeData::ProcessingInstruction {
            target: target.into(),
            data: data.into(),
        })
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
        if let Some(existing) = self.nodes[host.index()].shadow_root {
            return existing;
        }
        let sr = self.alloc(NodeData::ShadowRoot { mode });
        self.nodes[sr.index()].parent = Some(host);
        self.nodes[host.index()].shadow_root = Some(sr);
        self.structure_changed = true;
        self.mark_dirty(host);
        sr
    }

    pub fn shadow_root(&self, host: NodeId) -> Option<NodeId> {
        self.nodes[host.index()].shadow_root
    }

    pub fn is_shadow_root(&self, id: NodeId) -> bool {
        matches!(
            self.nodes.get(id.index()).map(|n| &n.data),
            Some(NodeData::ShadowRoot { .. })
        )
    }

    pub fn shadow_root_mode(&self, id: NodeId) -> Option<ShadowRootMode> {
        match self.nodes[id.index()].data {
            NodeData::ShadowRoot { mode } => Some(mode),
            _ => None,
        }
    }

    /// The element hosting this shadow root.
    pub fn shadow_host(&self, shadow_root: NodeId) -> Option<NodeId> {
        if self.is_shadow_root(shadow_root) {
            self.nodes[shadow_root.index()].parent
        } else {
            None
        }
    }

    /// Create (or fetch) a `<template>`'s contents fragment. Also not a child link.
    pub fn template_contents(&mut self, template: NodeId) -> NodeId {
        if let Some(f) = self.nodes[template.index()].template_contents {
            return f;
        }
        let frag = self.alloc(NodeData::Fragment);
        self.nodes[frag.index()].parent = Some(template);
        self.nodes[template.index()].template_contents = Some(frag);
        frag
    }

    pub fn get_template_contents(&self, template: NodeId) -> Option<NodeId> {
        self.nodes[template.index()].template_contents
    }

    /// Point a `<template>`'s contents at `node`.
    ///
    /// The declarative-shadow-DOM parse aims a template's contents **at the shadow root**,
    /// so everything the parser inserts into the template lands directly in the shadow
    /// tree. (The tree builder attaches the shadow root at the *start* tag, then keeps
    /// inserting into `get_template_contents`; without this the shadow root would stay
    /// empty.)
    pub fn set_template_contents(&mut self, template: NodeId, node: NodeId) {
        self.nodes[template.index()].template_contents = Some(node);
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
        let want = self
            .element(node)
            .and_then(|e| e.attr("slot"))
            .unwrap_or("");
        self.descendants(shadow).find(|&s| {
            self.tag_name(s) == Some("slot")
                && self.element(s).and_then(|e| e.attr("name")).unwrap_or("") == want
        })
    }

    /// Every shadow root in the arena, in creation order.
    pub fn all_shadow_roots(&self) -> Vec<NodeId> {
        (0..self.nodes.len())
            .map(|i| NodeId(i as u64))
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
            cur = self.nodes[n.index()].parent;
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
        if let NodeData::Element(el) = &mut self.nodes[id.index()].data {
            if let Some(i) = el.attrs.iter().position(|a| a.name == name) {
                el.attrs.remove(i);
                return true;
            }
        }
        false
    }

    pub fn set_attr(&mut self, id: NodeId, name: impl Into<String>, value: impl Into<String>) {
        if let NodeData::Element(el) = &mut self.nodes[id.index()].data {
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
        if node.index() >= self.nodes.len() {
            return;
        }
        self.nodes[node.index()].dirty = true;
        let mut cur = self.nodes[node.index()].parent;
        while let Some(p) = cur {
            if self.nodes[p.index()].dirty_descendants {
                break;
            }
            self.nodes[p.index()].dirty_descendants = true;
            cur = self.nodes[p.index()].parent;
        }
    }

    /// Has `node` itself changed since the last clean pass?
    pub fn is_dirty(&self, node: NodeId) -> bool {
        self.nodes.get(node.index()).is_some_and(|n| n.dirty)
    }

    /// Does `node`'s subtree contain a dirty node (the skip-this-subtree summary bit)?
    pub fn has_dirty_descendants(&self, node: NodeId) -> bool {
        self.nodes
            .get(node.index())
            .is_some_and(|n| n.dirty_descendants)
    }

    /// Is `node` clean *and* free of dirty descendants — i.e. a traversal may skip its
    /// whole subtree and reuse cached layout/paint?
    pub fn subtree_clean(&self, node: NodeId) -> bool {
        self.nodes
            .get(node.index())
            .is_some_and(|n| !n.dirty && !n.dirty_descendants)
    }

    /// Clear both dirty bits on a single node (call after processing it).
    pub fn clear_dirty(&mut self, node: NodeId) {
        if let Some(n) = self.nodes.get_mut(node.index()) {
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
    /// Is anything in the tree dirty? The cheap question the load path asks before deciding whether
    /// a full re-cascade is warranted — the cascade is the most expensive stage in the pipeline, and
    /// running it when nothing changed is pure latency.
    pub fn has_dirty(&self) -> bool {
        self.is_dirty(self.root()) || self.has_dirty_descendants(self.root())
    }

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
        if self.nodes[child.index()].parent != Some(parent) {
            return false;
        }
        self.detach(child);
        self.structure_changed = true;
        self.mark_dirty(parent);
        true
    }

    pub fn append_child(&mut self, parent: NodeId, child: NodeId) {
        // **The arena's own backstop: never build a cycle.** A node inserted into its own descendant makes
        // `children()` walk forever — a HANG (Bar 0), not a wrong answer. The JS layer throws
        // `HierarchyRequestError` for this; the arena simply refuses, because it is reachable from the
        // parser and from Rust callers too.
        if self.is_inclusive_ancestor(child, parent) {
            return;
        }

        // **Inserting a DocumentFragment moves its CHILDREN, not itself.** That single rule is the
        // whole reason fragments exist, and it is what every framework relies on to commit a built
        // subtree in one insertion. Insert the fragment itself and you have inserted an inert wrapper
        // where the content should be — which is exactly what lit-html got, and why it rendered an
        // empty component with no error.
        if self.is_fragment(child) {
            let kids: Vec<NodeId> = self.children(child).collect();
            for k in kids {
                self.append_child(parent, k);
            }
            return;
        }
        self.detach(child);
        self.nodes[child.index()].parent = Some(parent);
        match self.nodes[parent.index()].last_child {
            Some(last) => {
                self.nodes[last.index()].next_sibling = Some(child);
                self.nodes[child.index()].prev_sibling = Some(last);
                self.nodes[parent.index()].last_child = Some(child);
            }
            None => {
                self.nodes[parent.index()].first_child = Some(child);
                self.nodes[parent.index()].last_child = Some(child);
            }
        }
        // Structural change: the child (and thus the parent's subtree) is dirty.
        self.structure_changed = true;
        self.mark_dirty(child);
    }

    /// Remove `child` from its parent, leaving it a detached root of its subtree.
    pub fn detach(&mut self, child: NodeId) {
        let (parent, prev, next) = {
            let n = &self.nodes[child.index()];
            (n.parent, n.prev_sibling, n.next_sibling)
        };
        if let Some(p) = prev {
            self.nodes[p.index()].next_sibling = next;
        }
        if let Some(n) = next {
            self.nodes[n.index()].prev_sibling = prev;
        }
        if let Some(par) = parent {
            if self.nodes[par.index()].first_child == Some(child) {
                self.nodes[par.index()].first_child = next;
            }
            if self.nodes[par.index()].last_child == Some(child) {
                self.nodes[par.index()].last_child = prev;
            }
        }
        let n = &mut self.nodes[child.index()];
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
        self.nodes.get(id.index())?.first_child
    }

    pub fn next_sibling(&self, id: NodeId) -> Option<NodeId> {
        self.nodes.get(id.index())?.next_sibling
    }

    pub fn last_child(&self, id: NodeId) -> Option<NodeId> {
        self.nodes.get(id.index())?.last_child
    }

    pub fn prev_sibling(&self, id: NodeId) -> Option<NodeId> {
        self.nodes.get(id.index())?.prev_sibling
    }

    /// If `id` is a text node, append `text` to it and report `true`. Used by the parser
    /// to merge adjacent text runs — two sibling text nodes would otherwise produce two
    /// inline runs for what is one string.
    pub fn append_text_to(&mut self, id: NodeId, text: &str) -> bool {
        if let NodeData::Text(t) = &mut self.nodes[id.index()].data {
            t.push_str(text);
            self.mark_dirty(id);
            return true;
        }
        false
    }

    /// Insert `new_node` into `parent`'s child list immediately before `sibling`.
    pub fn insert_before(&mut self, parent: NodeId, new_node: NodeId, sibling: NodeId) {
        debug_assert_eq!(self.nodes[sibling.index()].parent, Some(parent));
        // Same rule as `append_child`: a fragment contributes its CHILDREN, in order, and does not
        // itself enter the tree. lit-html commits every template through exactly this call.
        if self.is_fragment(new_node) {
            let kids: Vec<NodeId> = self.children(new_node).collect();
            for k in kids {
                self.insert_before(parent, k, sibling);
            }
            return;
        }
        // **THE ARENA'S OWN BACKSTOP: never build a cycle.** The JS layer checks pre-insertion validity
        // and throws `HierarchyRequestError` — but the arena is reachable from the parser, from layout and
        // from Rust callers too, and a cycle here is not a wrong answer, it is an **infinite `children()`
        // walk**: a hang, which is Bar 0. So the invariant is enforced where the tree actually lives.
        if self.is_inclusive_ancestor(new_node, parent) {
            return;
        }

        // **DOM spec, "pre-insert", step 2:** *"If referenceChild is node, then set referenceChild to
        // node's next sibling."*
        //
        // Inserting a node **before itself** is not an error and not a no-op — it is a *move*, and
        // the reference point has to slide past the node before we unlink it. Without this step the
        // sequence below builds a **self-cycle**:
        //
        //   detach(X)                      → X's sibling links are cleared
        //   X.next_sibling = Some(sibling) → sibling IS X, so **X.next_sibling = X**
        //
        // …and every subsequent `children()` walk spins forever. That is a **hang**, not a wrong
        // answer, so it takes the whole page down with it — Bar 0.
        //
        // No real site inserts a node before itself, which is exactly why the 265-site Chromium diff
        // could never have found this. **WPT found it in the first 25 tests** (`ChildNode-after`,
        // which calls `child.after(child)` on purpose). This is the argument for the instrument, in
        // one bug.
        let sibling = if new_node == sibling {
            match self.nodes[sibling.index()].next_sibling {
                Some(next) => next,
                // It was already last: "before its own next sibling" means "at the end".
                None => {
                    self.append_child(parent, new_node);
                    return;
                }
            }
        } else {
            sibling
        };

        self.detach(new_node);
        let prev = self.nodes[sibling.index()].prev_sibling;
        self.nodes[new_node.index()].parent = Some(parent);
        self.nodes[new_node.index()].prev_sibling = prev;
        self.nodes[new_node.index()].next_sibling = Some(sibling);
        self.nodes[sibling.index()].prev_sibling = Some(new_node);
        match prev {
            Some(p) => self.nodes[p.index()].next_sibling = Some(new_node),
            None => self.nodes[parent.index()].first_child = Some(new_node),
        }
        self.structure_changed = true;
        self.mark_dirty(new_node);
    }

    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.nodes.get(id.index())?.parent
    }

    /// **The flat tree was BUILT and then never used by the thing that matters.**
    ///
    /// `flat_children` (above) has been correct all along. Layout and the cascade walked `children()`
    /// instead — which does not contain the shadow root, because a shadow root hangs off its host in
    /// its own field rather than among its children. So **every web component on the web rendered
    /// nothing**, and the mechanism that would have rendered them was sitting right here, tested, in
    /// use by the HTML crate, and wired to nothing that draws pixels.
    ///
    /// Custom elements are not a niche. They are how design systems ship — Material, Fluent, Shoelace,
    /// Spectrum, every `<x-y>` on a bank or a government site — and Lit is merely the framework that
    /// made the gap visible.
    /// Every node in the flattened tree under `id`, in render order — shadow trees included. This is
    /// what the CASCADE must walk: a node the cascade never sees is a node layout cannot style, and
    /// before this the entire shadow tree was exactly that.
    pub fn flat_descendants(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut stack = vec![id];
        while let Some(n) = stack.pop() {
            out.push(n);
            let kids = self.flat_children(n);
            for &k in kids.iter().rev() {
                stack.push(k);
            }
        }
        out
    }

    /// Iterator over the direct children of `id`.
    pub fn children(&self, id: NodeId) -> Children<'_> {
        Children {
            dom: self,
            next: self.nodes[id.index()].first_child,
        }
    }

    /// Is this a text node? (`nodeType === 3`, and the difference between `nodeValue` returning the
    /// text and returning null — both of which frameworks read directly.)
    pub fn is_text(&self, id: NodeId) -> bool {
        matches!(self.nodes[id.index()].data, NodeData::Text(_))
    }

    pub fn is_element(&self, id: NodeId) -> bool {
        matches!(
            self.nodes.get(id.index()).map(|n| &n.data),
            Some(NodeData::Element(_))
        )
    }

    /// Is this a ProcessingInstruction node? (`nodeType === 7`.) `.get()`, not indexing: reachable
    /// from JS with a handle whose id may be out of range for this arena.
    pub fn is_processing_instruction(&self, id: NodeId) -> bool {
        matches!(
            self.nodes.get(id.index()).map(|n| &n.data),
            Some(NodeData::ProcessingInstruction { .. })
        )
    }

    pub fn element(&self, id: NodeId) -> Option<&ElementData> {
        // Gate on the generation so a stale handle (to a removed/reused slot) reads as "no
        // element" rather than aliasing the slot's current or freed-but-uncleared contents.
        if !self.is_alive(id) {
            return None;
        }
        match &self.nodes[id.index()].data {
            NodeData::Element(e) => Some(e),
            _ => None,
        }
    }

    pub fn tag_name(&self, id: NodeId) -> Option<&str> {
        self.element(id).map(|e| e.name.as_str())
    }

    /// The XML, XMLNS and XHTML namespace URIs — the fixed points of the DOM's namespace algorithms.
    /// `XML_NS` and `XMLNS_NS` are always bound (they cannot be re-declared or overridden); `XHTML_NS`
    /// is the effective namespace of an HTML-namespaced element, which this arena stores as `None`.
    pub const XML_NS: &'static str = "http://www.w3.org/XML/1998/namespace";
    pub const XMLNS_NS: &'static str = "http://www.w3.org/2000/xmlns/";
    pub const XHTML_NS: &'static str = "http://www.w3.org/1999/xhtml";

    /// The DOM "locate a namespace" algorithm (DOM Living Standard §Node). Given a `prefix` (or `None`
    /// for the default namespace), walk from `node` toward the root and return the namespace URI it
    /// resolves to, or `None`.
    ///
    /// This backs both `node.lookupNamespaceURI(prefix)` (which normalises `""` → `None` first) and
    /// `node.isDefaultNamespace(ns)` (which calls it with `None` and compares). The subtleties that make
    /// it more than a field read:
    ///   * The `xml` and `xmlns` prefixes are **always** bound on an element and cannot be overridden by
    ///     an `xmlns:*` declaration — so `el.lookupNamespaceURI('xmlns')` is `XMLNS_NS` even after
    ///     `setAttributeNS(XMLNS_NS, 'xmlns', ...)`.
    ///   * An HTML element stores `namespace: None` but **is** in the XHTML namespace with a null prefix,
    ///     so `document.lookupNamespaceURI(null)` is XHTML, not whatever `xmlns` the `<html>` carries —
    ///     the element's own namespace wins over its attributes.
    ///   * "Parent element" means the parent *iff it is an element* — a comment whose parent is the
    ///     document resolves to `None`, it does not climb to the document element.
    pub fn locate_namespace(&self, node: NodeId, prefix: Option<&str>) -> Option<String> {
        match &self.nodes.get(node.index())?.data {
            NodeData::Element(el) => {
                match prefix {
                    Some("xml") => return Some(Self::XML_NS.to_string()),
                    Some("xmlns") => return Some(Self::XMLNS_NS.to_string()),
                    _ => {}
                }
                // 1. The element's own namespace, if its prefix matches. HTML (`None`) → XHTML, null prefix.
                let (el_ns, el_prefix) = match &el.namespace {
                    Some(ns) => (ns.as_str(), el.name.split_once(':').map(|(p, _)| p)),
                    None => (Self::XHTML_NS, None),
                };
                if el_prefix == prefix {
                    return Some(el_ns.to_string());
                }
                // 2. An `xmlns:<prefix>` (or bare `xmlns` for the default) declaration. Stored as an
                //    ordinary attribute by qualified name; an empty value un-declares (→ `None`).
                let attr_name = match prefix {
                    Some(p) => format!("xmlns:{p}"),
                    None => "xmlns".to_string(),
                };
                if let Some(v) = el.attr(&attr_name) {
                    return (!v.is_empty()).then(|| v.to_string());
                }
                // 3. Recurse to the parent ELEMENT.
                self.parent_element(node)
                    .and_then(|pe| self.locate_namespace(pe, prefix))
            }
            NodeData::Document => self
                .document_element(node)
                .and_then(|de| self.locate_namespace(de, prefix)),
            // A DocumentType, DocumentFragment or ShadowRoot has no namespace and no element to climb to.
            NodeData::Doctype { .. } | NodeData::Fragment | NodeData::ShadowRoot { .. } => None,
            // Text / Comment / etc.: resolve on the parent element, if any.
            _ => self
                .parent_element(node)
                .and_then(|pe| self.locate_namespace(pe, prefix)),
        }
    }

    /// `node.isDefaultNamespace(namespace)` — is `namespace` (with `""` normalised to `None`) the
    /// default namespace in scope at `node`? (DOM §Node.)
    pub fn is_default_namespace(&self, node: NodeId, namespace: Option<&str>) -> bool {
        let ns = namespace.filter(|s| !s.is_empty());
        self.locate_namespace(node, None).as_deref() == ns
    }

    /// `node.lookupPrefix(namespace)` — DOM §Node "locate a namespace prefix". Given a namespace URI,
    /// find the prefix that maps to it in scope at `node`. `namespace` is nullable and an empty string is
    /// treated as `null` (which always returns `None`, per spec). The return is a prefix string or `None`.
    ///
    /// This is the inverse of [`Self::locate_namespace`], and the two share the same walk: check the
    /// element's own `(namespace, prefix)`, then its `xmlns:<prefix>` declarations, then recurse to the
    /// parent element. A DocumentType/DocumentFragment has none, a Document defers to its documentElement,
    /// and a Text/Comment/PI defers to its parent element.
    pub fn lookup_prefix(&self, node: NodeId, namespace: Option<&str>) -> Option<String> {
        let ns = namespace.filter(|s| !s.is_empty())?;
        self.locate_prefix(node, ns)
    }

    fn locate_prefix(&self, node: NodeId, namespace: &str) -> Option<String> {
        match &self.nodes.get(node.index())?.data {
            NodeData::Element(el) => {
                // 1. The element's own namespace, if it has a (non-null) prefix that maps to it.
                let (el_ns, el_prefix) = match &el.namespace {
                    Some(ns) => (ns.as_str(), el.name.split_once(':').map(|(p, _)| p)),
                    None => (Self::XHTML_NS, None),
                };
                if el_ns == namespace {
                    if let Some(p) = el_prefix {
                        return Some(p.to_string());
                    }
                }
                // 2. An `xmlns:<prefix>` declaration whose value is this namespace → its local name.
                for a in &el.attrs {
                    if a.value == namespace {
                        if let Some(("xmlns", prefix)) = a.name.split_once(':') {
                            return Some(prefix.to_string());
                        }
                    }
                }
                // 3. Recurse to the parent ELEMENT.
                self.parent_element(node)
                    .and_then(|pe| self.locate_prefix(pe, namespace))
            }
            NodeData::Document => self
                .document_element(node)
                .and_then(|de| self.locate_prefix(de, namespace)),
            NodeData::Doctype { .. } | NodeData::Fragment | NodeData::ShadowRoot { .. } => None,
            _ => self
                .parent_element(node)
                .and_then(|pe| self.locate_prefix(pe, namespace)),
        }
    }

    /// The parent node **iff it is an element** (i.e. `node.parentElement`), else `None`.
    fn parent_element(&self, node: NodeId) -> Option<NodeId> {
        let p = self.parent(node)?;
        self.is_element(p).then_some(p)
    }

    /// A document's document element — its first element child — or `None` (e.g. `new Document()`).
    fn document_element(&self, doc: NodeId) -> Option<NodeId> {
        self.children(doc).find(|&c| self.is_element(c))
    }

    /// `node.nodeName` (DOM §Node). Per the spec, this is **per node type**, and — the bug this fixes —
    /// an element's nodeName is its `tagName`, which is ASCII-uppercased **only in the HTML namespace**.
    /// A non-HTML element (`createElementNS('http://example.com/', 'foo')`) keeps its case (`"foo"`, not
    /// `"FOO"`); the old getter uppercased unconditionally and returned `"#text"` for every non-element.
    pub fn node_name(&self, id: NodeId) -> String {
        match self.nodes.get(id.index()).map(|n| &n.data) {
            Some(NodeData::Element(el)) => match &el.namespace {
                Some(_) => el.name.clone(),
                None => el.name.to_ascii_uppercase(),
            },
            Some(NodeData::Text(_)) => "#text".to_string(),
            Some(NodeData::Comment(_)) => "#comment".to_string(),
            // A PI's nodeName is its target (`<?xml-stylesheet ...?>` → `"xml-stylesheet"`).
            Some(NodeData::ProcessingInstruction { target, .. }) => target.clone(),
            Some(NodeData::Document) => "#document".to_string(),
            Some(NodeData::Fragment) | Some(NodeData::ShadowRoot { .. }) => {
                "#document-fragment".to_string()
            }
            Some(NodeData::Doctype { name }) => name.clone(),
            None => "#text".to_string(),
        }
    }

    /// The first element with the given lowercased tag name, searched depth-first
    /// from the document root. Handy for `<html>`/`<body>`/`<title>` lookups.
    pub fn find_first(&self, name: &str) -> Option<NodeId> {
        self.find_first_in(self.root, name)
    }

    /// Like [`find_first`], but scoped to the subtree of `root` (pre-order, `root` excluded).
    ///
    /// **This is what makes a SECOND document in one arena resolve its own structure.** `find_first`
    /// searches from `self.root` — the main document — so `createHTMLDocument().body` used to alias the
    /// PAGE's `<body>`, and a test that appended to it corrupted the real document (and the WPT harness).
    /// A document's `documentElement`/`body`/`head` must search *its own* subtree, which is exactly this.
    pub fn find_first_in(&self, root: NodeId, name: &str) -> Option<NodeId> {
        self.descendants(root)
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
        if let NodeData::Text(t) = &self.nodes[id.index()].data {
            out.push_str(t);
        }
        for d in self.descendants(id) {
            if let NodeData::Text(t) = &self.nodes[d.index()].data {
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
        match &self.nodes[id.index()].data {
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
            NodeData::ProcessingInstruction { target, data } => {
                let _ = write!(out, "<?{target} {}?>", data.trim());
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
        self.next = self.dom.nodes[cur.index()].next_sibling;
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
    fn generational_nodeid_reclaims_slots_and_detects_stale() {
        let mut dom = Dom::new();
        let body = dom.create_element("body");
        dom.append_child(dom.root(), body);
        let a = dom.create_element("div");
        dom.append_child(body, a);
        assert!(dom.is_alive(a));
        let a_index = a.index();
        assert_eq!(a.generation(), 0, "first-alloc node is generation 0");

        // Discard the subtree: `a` is now stale, its slot freed for reuse.
        dom.discard_subtree(a);
        assert!(!dom.is_alive(a), "handle to a discarded node is stale");
        assert!(
            dom.element(a).is_none(),
            "stale handle resolves to no element"
        );

        // Next allocation reuses the freed slot with a bumped generation, so the new
        // handle differs from the stale one even though it shares the slot index.
        let b = dom.create_element("span");
        assert_eq!(b.index(), a_index, "freed slot was reused");
        assert_ne!(
            b, a,
            "reused slot yields a distinct (newer-generation) handle"
        );
        assert!(dom.is_alive(b));
        assert!(!dom.is_alive(a), "the old handle stays stale after reuse");
        // The current live handle for that slot is recoverable by index.
        assert_eq!(dom.id_at_index(a_index), Some(b));
    }

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

#[cfg(test)]
mod stale_handle_tests {
    use super::*;

    /// **G_STALE_NODE — a handle that does not name a node in THIS arena must be INERT, never fatal.**
    ///
    /// A JS reflector stores its node as a bare integer, and the arena it indexes is **not necessarily
    /// the arena it came from**: one process loads many documents and the current-DOM pointer is swapped
    /// on every re-entry into script. So a handle held from an earlier document indexes into a
    /// *different, smaller* arena.
    ///
    /// **And the consequence is not a wrong answer — it is a DEAD BROWSER.** These accessors are reached
    /// from `extern "C"` JS natives, which are `nounwind`, so a Rust panic inside one is *"panic in a
    /// function that cannot unwind"* → **SIGSEGV, core dumped.** Every tab the user had open dies because
    /// one page held a stale node.
    ///
    /// WPT found exactly this (`Node-appendChild-three-scripts-from-fragment`) — **and only when the file
    /// ran AFTER other documents.** It is perfectly clean in isolation, which is why no single-page test
    /// could ever have caught it, and why it survived every gate on the wall.
    ///
    /// The spec-shaped answer is also the safe one: **an operation on a node that is not there does
    /// nothing.**
    #[test]
    fn a_handle_from_another_arena_is_inert_not_fatal() {
        let mut small = Dom::new();
        let root = small.create_element("div");

        // A handle from a *bigger* document — exactly what a reflector from a previous page looks like.
        let foreign = NodeId(9_999);

        assert!(
            !small.is_alive(foreign),
            "a handle past the end of this arena must not be alive"
        );
        // Each of these is reachable from JS. None may panic. (`is_fragment` is the one that actually
        // aborted the process: `appendChild` calls it on its argument.)
        assert!(
            !small.is_fragment(foreign),
            "is_fragment must not index blindly"
        );
        assert!(small.parent(foreign).is_none());
        assert!(small.first_child(foreign).is_none());
        assert!(small.next_sibling(foreign).is_none());
        assert!(small.tag_name(foreign).is_none());
        assert!(small.character_data(foreign).is_none());
        assert!(!small.is_element(foreign));

        // And the real document is untouched by any of it.
        assert!(small.is_alive(root));
        assert_eq!(small.tag_name(root), Some("div"));
    }
}

#[cfg(test)]
mod pointer_width_tests {
    use super::*;

    /// **G_ARENA_U64 — the arena handle must be pointer-width-INDEPENDENT.**
    ///
    /// `NodeId` packs `generation << 32 | index`. If it is backed by `usize`, then on a 32-bit target
    /// (`wasm32-unknown-unknown` — the in-browser demo's build) `usize` is 32 bits and the shift
    /// **overflows: the crate does not even compile.** That is exactly what the demo's wasm build
    /// surfaced. The fix is a `u64` backing, identical to `usize` on 64-bit and correct on 32-bit.
    ///
    /// This test would not have caught the *compile* failure (it is 64-bit here), so its real job is to
    /// pin the packing semantics so a future "simplify NodeId back to usize" cannot silently reintroduce
    /// the 32-bit overflow. It also matters for the ARM/cross-platform target, not only wasm.
    #[test]
    fn nodeid_packs_generation_above_the_32_bit_boundary() {
        // A generation in the high 32 bits and an index in the low 32 — the exact pattern that overflows
        // a 32-bit usize.
        let id = NodeId::pack(0x1234_5678, 0x9abc);
        assert_eq!(
            id.index(),
            0x1234_5678,
            "the low 32 bits are the slot index"
        );
        assert_eq!(
            id.generation(),
            0x9abc,
            "the high 32 bits are the generation"
        );
        // The packed value genuinely uses bits above 32 — proving it is not a 32-bit type.
        assert!(
            id.0 > u32::MAX as u64,
            "the packed handle exceeds 32 bits, so usize would overflow it"
        );
        // Generation-0 compatibility: the packed value equals the bare index (serialized-handle contract).
        assert_eq!(NodeId::pack(42, 0).0, 42);
    }
}
