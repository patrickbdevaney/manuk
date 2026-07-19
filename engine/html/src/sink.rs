//! N3 — a `TreeSink` **directly over the arena `Dom`**.
//!
//! Two payoffs, one change:
//!
//! 1. **Declarative Shadow DOM works.** html5ever *already* implements the DSD parsing
//!    rules — its tree builder checks `shadowrootmode` on a `<template>` start tag and
//!    calls [`TreeSink::attach_declarative_shadow`]. That hook **defaults to `false`**,
//!    and `markup5ever_rcdom` never overrides it, so with `RcDom` a
//!    `<template shadowrootmode="open">` parsed as an ordinary template and the shadow
//!    root was silently dropped. Implementing the sink ourselves is all it takes.
//! 2. **The `RcDom` → arena copy disappears.** We were building an `Rc`-based tree and
//!    then walking it into the arena. Now the parser writes the arena directly.
//!
//! `TreeSink` takes `&self`, not `&mut self`, so the arena lives behind a `RefCell`.
//! Parsing is single-threaded and the tree builder never re-enters us, so the borrows are
//! strictly non-overlapping.
//!
//! **Documented gaps (not faked):** namespaces are folded to the local name (as the arena
//! has always done — SVG/MathML foreign content is not modelled); `associate_with_form` is
//! a no-op (no form-owner tracking); `mark_script_already_started` is a no-op (no script
//! execution during parse); template contents are parsed into a real `Fragment`, but the
//! `<template>` element itself is still exposed in the node tree.

use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use html5ever::tendril::StrTendril;
use html5ever::tree_builder::{ElemName, NodeOrText, QuirksMode, TreeSink};
use html5ever::{Attribute, ExpandedName, LocalName, Namespace, QualName};
use manuk_dom::{Dom, NodeId, ShadowRootMode};

/// The element-name view the tree builder asks for. We fold namespaces to the local name,
/// so the namespace is always HTML.
#[derive(Debug)]
pub struct ArenaElemName {
    ns: Namespace,
    local: LocalName,
}

impl ElemName for ArenaElemName {
    fn ns(&self) -> &Namespace {
        &self.ns
    }
    fn local_name(&self) -> &LocalName {
        &self.local
    }
}

pub struct ArenaSink {
    /// Shared so a streaming parse can snapshot the partial tree while the parser still
    /// owns the sink (B-latency's `StreamParser`).
    dom: Rc<RefCell<Dom>>,
    /// Parse errors, kept for diagnostics rather than discarded.
    errors: RefCell<Vec<String>>,
}

impl Default for ArenaSink {
    fn default() -> Self {
        ArenaSink {
            dom: Rc::new(RefCell::new(Dom::new())),
            errors: RefCell::new(Vec::new()),
        }
    }
}

impl ArenaSink {
    pub fn new() -> Self {
        Self::default()
    }

    /// A handle onto the arena being built, so a streaming caller can read the
    /// parsed-so-far tree without owning the sink.
    pub fn dom_handle(&self) -> Rc<RefCell<Dom>> {
        Rc::clone(&self.dom)
    }

    pub fn errors(&self) -> Vec<String> {
        self.errors.borrow().clone()
    }

    /// Append `child` (a node or text) to `parent`, merging adjacent text nodes the way a
    /// DOM must — otherwise `"a" + "b"` becomes two text nodes and `textContent` still
    /// works but layout emits two runs.
    fn append_node_or_text(&self, parent: NodeId, child: NodeOrText<NodeId>) {
        let mut dom = self.dom.borrow_mut();
        match child {
            NodeOrText::AppendNode(n) => dom.append_child(parent, n),
            NodeOrText::AppendText(t) => {
                if let Some(last) = dom.last_child(parent) {
                    if dom.append_text_to(last, &t) {
                        return;
                    }
                }
                let n = dom.create_text(t.to_string());
                dom.append_child(parent, n);
            }
        }
    }
}

impl TreeSink for ArenaSink {
    type Handle = NodeId;
    type Output = Dom;
    type ElemName<'a> = ArenaElemName;

    fn finish(self) -> Dom {
        match Rc::try_unwrap(self.dom) {
            Ok(cell) => cell.into_inner(),
            // A `StreamParser` still holds a handle; hand back a clone.
            Err(rc) => rc.borrow().clone(),
        }
    }

    fn parse_error(&self, msg: Cow<'static, str>) {
        self.errors.borrow_mut().push(msg.into_owned());
    }

    fn get_document(&self) -> NodeId {
        self.dom.borrow().root()
    }

    fn elem_name<'a>(&'a self, target: &'a NodeId) -> ArenaElemName {
        let dom = self.dom.borrow();
        let local = dom
            .tag_name(*target)
            .expect("elem_name on a non-element node");
        ArenaElemName {
            ns: html5ever::ns!(html),
            local: LocalName::from(local),
        }
    }

    fn create_element(
        &self,
        name: QualName,
        attrs: Vec<Attribute>,
        _flags: html5ever::tree_builder::ElementFlags,
    ) -> NodeId {
        let mut dom = self.dom.borrow_mut();
        let el = dom.create_element(name.local.to_string());
        for a in attrs {
            dom.set_attr(el, a.name.local.to_string(), a.value.to_string());
        }
        // A `<template>` gets its contents fragment eagerly, as the trait requires.
        if name.local == html5ever::local_name!("template") {
            dom.template_contents(el);
        }
        el
    }

    fn create_comment(&self, text: StrTendril) -> NodeId {
        self.dom.borrow_mut().create_comment(text.to_string())
    }

    fn create_pi(&self, target: StrTendril, data: StrTendril) -> NodeId {
        // Processing instructions do not exist in HTML; keep them as comments so no
        // content is silently lost.
        self.dom
            .borrow_mut()
            .create_comment(format!("?{target} {data}"))
    }

    fn append(&self, parent: &NodeId, child: NodeOrText<NodeId>) {
        self.append_node_or_text(*parent, child);
    }

    fn append_based_on_parent_node(
        &self,
        element: &NodeId,
        prev_element: &NodeId,
        child: NodeOrText<NodeId>,
    ) {
        let has_parent = self.dom.borrow().parent(*element).is_some();
        if has_parent {
            self.append_before_sibling(element, child);
        } else {
            self.append_node_or_text(*prev_element, child);
        }
    }

    fn append_doctype_to_document(&self, name: StrTendril, _public: StrTendril, _sys: StrTendril) {
        let mut dom = self.dom.borrow_mut();
        let root = dom.root();
        let dt = dom.create_doctype(name.to_string());
        dom.append_child(root, dt);
    }

    fn get_template_contents(&self, target: &NodeId) -> NodeId {
        self.dom.borrow_mut().template_contents(*target)
    }

    fn same_node(&self, x: &NodeId, y: &NodeId) -> bool {
        x == y
    }

    fn set_quirks_mode(&self, mode: QuirksMode) {
        // **This used to write to a field nobody read.** `self.quirks` was set here and never
        // consulted anywhere, while every Stylo call site hard-coded `NoQuirks` — so the parser
        // detected quirks correctly and the style system was never told (measured, tick 241).
        // Writing straight into the `Dom` is what closes the loop: the `Dom` is what every consumer
        // already receives, so no signature has to change to carry the verdict.
        //
        // `LimitedQuirks` maps to `false`. It is "almost standards" mode, which differs from full
        // standards only in the inline-image baseline rule — it does NOT enable the unitless-length
        // quirk, so folding it into `false` is correct for every behaviour currently gated on this.
        self.dom.borrow_mut().set_quirks(mode == QuirksMode::Quirks);
    }

    fn append_before_sibling(&self, sibling: &NodeId, new_node: NodeOrText<NodeId>) {
        let mut dom = self.dom.borrow_mut();
        let Some(parent) = dom.parent(*sibling) else {
            return;
        };
        match new_node {
            NodeOrText::AppendNode(n) => dom.insert_before(parent, n, *sibling),
            NodeOrText::AppendText(t) => {
                // Merge into a preceding text sibling when there is one.
                if let Some(prev) = dom.prev_sibling(*sibling) {
                    if dom.append_text_to(prev, &t) {
                        return;
                    }
                }
                let n = dom.create_text(t.to_string());
                dom.insert_before(parent, n, *sibling);
            }
        }
    }

    fn add_attrs_if_missing(&self, target: &NodeId, attrs: Vec<Attribute>) {
        let mut dom = self.dom.borrow_mut();
        for a in attrs {
            let name = a.name.local.to_string();
            let missing = dom
                .element(*target)
                .map(|e| e.attr(&name).is_none())
                .unwrap_or(false);
            if missing {
                dom.set_attr(*target, name, a.value.to_string());
            }
        }
    }

    fn associate_with_form(
        &self,
        _target: &NodeId,
        _form: &NodeId,
        _nodes: (&NodeId, Option<&NodeId>),
    ) {
        // No form-owner tracking yet; `agent::forms` finds the enclosing <form> by walking up.
    }

    fn remove_from_parent(&self, target: &NodeId) {
        let mut dom = self.dom.borrow_mut();
        if let Some(parent) = dom.parent(*target) {
            dom.remove_child(parent, *target);
        }
    }

    fn reparent_children(&self, node: &NodeId, new_parent: &NodeId) {
        let mut dom = self.dom.borrow_mut();
        let kids: Vec<NodeId> = dom.children(*node).collect();
        for k in kids {
            dom.remove_child(*node, k);
            dom.append_child(*new_parent, k);
        }
    }

    /// Documents parsed by this sink always permit declarative shadow roots.
    fn allow_declarative_shadow_roots(&self, _intended_parent: &NodeId) -> bool {
        true
    }

    /// **The hook that was silently returning `false`.** Attach a shadow root to
    /// `location` (the host) and move the `<template>`'s contents into it.
    fn attach_declarative_shadow(
        &self,
        location: &NodeId,
        template: &NodeId,
        attrs: &[Attribute],
    ) -> bool {
        let mode = attrs
            .iter()
            .find(|a| a.name.local == html5ever::local_name!("shadowrootmode"))
            .map(|a| match a.value.as_ref() {
                "closed" => ShadowRootMode::Closed,
                _ => ShadowRootMode::Open,
            })
            .unwrap_or(ShadowRootMode::Open);

        let mut dom = self.dom.borrow_mut();
        let shadow = dom.attach_shadow(*location, mode);

        // The tree builder attaches the shadow root at the template's *start* tag and then
        // keeps inserting content into `get_template_contents(template)`. So point the
        // template's contents AT the shadow root: everything parsed inside the template
        // lands directly in the shadow tree. (Moving children here would move nothing —
        // none have been parsed yet.)
        dom.set_template_contents(*template, shadow);
        true
    }
}

/// A `QualName` in the HTML namespace, for callers building names by hand.
pub fn html_name(local: &str) -> QualName {
    QualName::new(None, html5ever::ns!(html), LocalName::from(local))
}

/// Expanded name of an arena element (HTML namespace, folded local name).
pub fn expanded<'a>(name: &'a ArenaElemName) -> ExpandedName<'a> {
    name.expanded()
}
