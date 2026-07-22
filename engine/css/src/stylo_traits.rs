//! D2 — the Stylo DOM trait wall (`TDocument`/`TNode`/`TShadowRoot`/`TElement` +
//! `NodeInfo`/`AttributeProvider`) over the arena DOM.
//!
//! Why this exists: Stylo's cascade entry points (`Stylist::compute_for_declarations`,
//! `properties::cascade`) are `where E: TElement`. We drive them with `element = None`
//! (verified: no `TElement` method is invoked on that path — the element is only touched
//! via `Option::map`/`map_or`), so the wall's job is purely to **name a type that
//! implements `TElement`**. Accordingly:
//!
//! * Cheap, always-safe methods (tree navigation, node kind, the `ElementData` accessors
//!   that delegate to [`super::ElementDataStore`]) are implemented for real.
//! * Methods that cannot be satisfied over a read-only arena handle and are provably
//!   unreachable on the `None` cascade path are `unimplemented!()`. If a future code path
//!   ever calls one, it panics loudly rather than returning a wrong answer — a documented,
//!   auditable boundary.
//!
//! Selector *matching* does **not** go through this wall — it uses the separate,
//! fully-real `selectors::Element` impl in [`super`]. This wall is only the cascade's
//! type-parameter requirement.

use app_units::Au;
use selectors::matching::ElementSelectorFlags;
use stylo::context::{QuirksMode, SharedStyleContext};
use stylo::data::{ElementDataMut, ElementDataRef};
use stylo::dom::{
    AttributeProvider, LayoutIterator, NodeInfo, TDocument, TElement, TNode, TShadowRoot,
};
use stylo::properties::declaration_block::PropertyDeclarationBlock;
use stylo::selector_parser::{AttrValue, Lang};
use stylo::servo_arc::{Arc, ArcBorrow};
use stylo::shared_lock::{Locked, SharedRwLock};
use stylo::values::AtomIdent;
use stylo::{Atom, LocalName, Namespace};
use stylo_dom::ElementState;

use manuk_dom::{Dom, NodeData, NodeId};

use crate::stylo_dom::{prev_sibling, ElementDataStore, StyloElement};

// ---------------------------------------------------------------------------
// Handles for the non-element node kinds the trait graph references.
// ---------------------------------------------------------------------------

/// A generic node handle (any arena node) — the `TNode` of the wall.
#[derive(Clone, Copy)]
pub struct StyloNode<'a> {
    pub dom: &'a Dom,
    pub node: NodeId,
    pub store: &'a ElementDataStore,
}

/// The document handle — the `TDocument` of the wall.
#[derive(Clone, Copy)]
pub struct StyloDocument<'a> {
    pub dom: &'a Dom,
    pub store: &'a ElementDataStore,
}

/// A shadow-root handle — the `TShadowRoot` of the wall.
#[derive(Clone, Copy)]
pub struct StyloShadowRoot<'a> {
    pub dom: &'a Dom,
    pub node: NodeId,
    pub store: &'a ElementDataStore,
}

impl std::fmt::Debug for StyloNode<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StyloNode(#{})", self.node.0)
    }
}
impl std::fmt::Debug for StyloDocument<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StyloDocument")
    }
}
impl std::fmt::Debug for StyloShadowRoot<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StyloShadowRoot(#{})", self.node.0)
    }
}

impl PartialEq for StyloNode<'_> {
    fn eq(&self, o: &Self) -> bool {
        self.node == o.node && std::ptr::eq(self.dom, o.dom)
    }
}
impl PartialEq for StyloShadowRoot<'_> {
    fn eq(&self, o: &Self) -> bool {
        self.node == o.node && std::ptr::eq(self.dom, o.dom)
    }
}

// StyloElement gains the identity traits TElement requires (matching is by node id).
impl PartialEq for StyloElement<'_> {
    fn eq(&self, o: &Self) -> bool {
        self.node == o.node && std::ptr::eq(self.dom, o.dom)
    }
}
impl Eq for StyloElement<'_> {}
impl std::hash::Hash for StyloElement<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.node.0.hash(state);
    }
}

impl<'a> StyloNode<'a> {
    fn make(dom: &'a Dom, node: NodeId, store: &'a ElementDataStore) -> Self {
        StyloNode { dom, node, store }
    }
    fn as_stylo_element(&self) -> Option<StyloElement<'a>> {
        self.dom
            .is_element(self.node)
            .then(|| StyloElement::new(self.dom, self.node, self.store))
    }
}

impl<'a> StyloElement<'a> {
    fn as_stylo_node(&self) -> StyloNode<'a> {
        StyloNode::make(self.dom, self.node, self.store)
    }
}

// ---------------------------------------------------------------------------
// NodeInfo
// ---------------------------------------------------------------------------

impl NodeInfo for StyloNode<'_> {
    fn is_element(&self) -> bool {
        self.dom.is_element(self.node)
    }
    fn is_text_node(&self) -> bool {
        matches!(self.dom.data(self.node), NodeData::Text(_))
    }
}

// ---------------------------------------------------------------------------
// TDocument
// ---------------------------------------------------------------------------

impl<'a> TDocument for StyloDocument<'a> {
    type ConcreteNode = StyloNode<'a>;

    fn as_node(&self) -> Self::ConcreteNode {
        StyloNode::make(self.dom, self.dom.root(), self.store)
    }
    fn is_html_document(&self) -> bool {
        true
    }
    fn quirks_mode(&self) -> QuirksMode {
        // `StyloDocument` already holds `&Dom`, so the parser's verdict is a field read away — this
        // hard-coded `NoQuirks` was the last consumer of a value that had been detected, stored and
        // discarded since the tree sink was written (tick 241).
        if self.dom.quirks() {
            QuirksMode::Quirks
        } else {
            QuirksMode::NoQuirks
        }
    }
    fn shared_lock(&self) -> &SharedRwLock {
        // Not carried on the handle; never reached on the `element = None` cascade path.
        unimplemented!("StyloDocument::shared_lock is not used by the None-cascade path")
    }
}

// ---------------------------------------------------------------------------
// TShadowRoot
// ---------------------------------------------------------------------------

impl<'a> TShadowRoot for StyloShadowRoot<'a> {
    type ConcreteNode = StyloNode<'a>;

    fn as_node(&self) -> Self::ConcreteNode {
        StyloNode::make(self.dom, self.node, self.store)
    }
    fn host(&self) -> StyloElement<'a> {
        unimplemented!("StyloShadowRoot::host is not used by the None-cascade path")
    }
    fn style_data<'b>(&self) -> Option<&'b stylo::stylist::CascadeData>
    where
        Self: 'b,
    {
        None
    }
}

// ---------------------------------------------------------------------------
// TNode
// ---------------------------------------------------------------------------

impl<'a> TNode for StyloNode<'a> {
    type ConcreteElement = StyloElement<'a>;
    type ConcreteDocument = StyloDocument<'a>;
    type ConcreteShadowRoot = StyloShadowRoot<'a>;

    fn parent_node(&self) -> Option<Self> {
        self.dom
            .parent(self.node)
            .map(|n| StyloNode::make(self.dom, n, self.store))
    }
    fn first_child(&self) -> Option<Self> {
        self.dom
            .first_child(self.node)
            .map(|n| StyloNode::make(self.dom, n, self.store))
    }
    fn last_child(&self) -> Option<Self> {
        let mut last = None;
        let mut cur = self.dom.first_child(self.node);
        while let Some(n) = cur {
            last = Some(n);
            cur = self.dom.next_sibling(n);
        }
        last.map(|n| StyloNode::make(self.dom, n, self.store))
    }
    fn prev_sibling(&self) -> Option<Self> {
        prev_sibling(self.dom, self.node).map(|n| StyloNode::make(self.dom, n, self.store))
    }
    fn next_sibling(&self) -> Option<Self> {
        self.dom
            .next_sibling(self.node)
            .map(|n| StyloNode::make(self.dom, n, self.store))
    }
    fn owner_doc(&self) -> Self::ConcreteDocument {
        StyloDocument {
            dom: self.dom,
            store: self.store,
        }
    }
    fn is_in_document(&self) -> bool {
        true
    }
    fn traversal_parent(&self) -> Option<Self::ConcreteElement> {
        self.parent_node().and_then(|n| n.as_stylo_element())
    }
    fn opaque(&self) -> stylo::dom::OpaqueNode {
        unimplemented!("StyloNode::opaque is not used by the None-cascade path")
    }
    fn debug_id(self) -> usize {
        self.node.0 as usize
    }
    fn as_element(&self) -> Option<Self::ConcreteElement> {
        self.as_stylo_element()
    }
    fn as_document(&self) -> Option<Self::ConcreteDocument> {
        // The arena's Document node is the root; treat only it as the document.
        (self.node == self.dom.root()).then_some(StyloDocument {
            dom: self.dom,
            store: self.store,
        })
    }
    fn as_shadow_root(&self) -> Option<Self::ConcreteShadowRoot> {
        self.dom
            .is_shadow_root(self.node)
            .then_some(StyloShadowRoot {
                dom: self.dom,
                node: self.node,
                store: self.store,
            })
    }
}

// ---------------------------------------------------------------------------
// AttributeProvider
// ---------------------------------------------------------------------------

impl AttributeProvider for StyloElement<'_> {
    fn get_attr(&self, attr: &LocalName, _namespace: &Namespace) -> Option<String> {
        self.dom
            .element(self.node)
            .and_then(|e| e.attr(attr))
            .map(str::to_string)
    }
}

// ---------------------------------------------------------------------------
// TElement — the bulk. Real where cheap/safe; unimplemented!() where provably
// unreachable on the `element = None` cascade path.
// ---------------------------------------------------------------------------

impl<'a> TElement for StyloElement<'a> {
    type ConcreteNode = StyloNode<'a>;
    type TraversalChildrenIterator = std::iter::Empty<StyloNode<'a>>;

    fn as_node(&self) -> Self::ConcreteNode {
        self.as_stylo_node()
    }

    fn traversal_children(&self) -> LayoutIterator<Self::TraversalChildrenIterator> {
        LayoutIterator(std::iter::empty())
    }

    fn is_html_element(&self) -> bool {
        true
    }
    fn is_mathml_element(&self) -> bool {
        false
    }
    fn is_svg_element(&self) -> bool {
        false
    }

    fn style_attribute(&self) -> Option<ArcBorrow<'_, Locked<PropertyDeclarationBlock>>> {
        // Inline `style=` is folded into the merged block by our own cascade, not read
        // through Stylo here.
        None
    }

    fn animation_rule(
        &self,
        _: &SharedStyleContext,
    ) -> Option<Arc<Locked<PropertyDeclarationBlock>>> {
        None
    }
    fn transition_rule(
        &self,
        _: &SharedStyleContext,
    ) -> Option<Arc<Locked<PropertyDeclarationBlock>>> {
        None
    }

    fn state(&self) -> ElementState {
        ElementState::empty()
    }

    fn has_part_attr(&self) -> bool {
        false
    }
    fn exports_any_part(&self) -> bool {
        false
    }

    fn id(&self) -> Option<&Atom> {
        None
    }

    /// **This is the fast path, and stubbing it out was costing us the entire cascade.**
    ///
    /// The old comment here said "class matching uses `selectors::Element::has_class`, not this
    /// (bloom/invalidation)". That is only half true, and the half it gets wrong is the expensive
    /// half. Stylo uses `each_class` in two places that decide how much work a cascade does:
    ///
    ///   * `SelectorMap::lookup_with_state` (selector_map.rs:540) — to look up the **class-bucketed
    ///     rules**. Rules are filed under their rightmost class at insertion time; if the element
    ///     never enumerates its classes, that bucket is never consulted, and every one of those rules
    ///     has to be found the slow way instead.
    ///   * `each_relevant_element_hash` (bloom.rs:127) — to feed the **ancestor Bloom filter**, which
    ///     is what lets a descendant selector like `.mw-body .reference` be rejected in one hash
    ///     probe instead of a walk up the ancestor chain. With no classes in the filter, essentially
    ///     nothing can be rejected early.
    ///
    /// Wikipedia: 18,631 elements, and the cascade took **339ms** — about 18µs per element, roughly
    /// twenty times what it should be. `has_class` was doing all the work, one rule at a time.
    fn each_class<F>(&self, mut callback: F)
    where
        F: FnMut(&AtomIdent),
    {
        if let Some(e) = self.dom.element(self.node) {
            for c in e.classes() {
                callback(&AtomIdent::from(c));
            }
        }
    }
    fn each_custom_state<F>(&self, _callback: F)
    where
        F: FnMut(&AtomIdent),
    {
    }
    fn each_attr_name<F>(&self, _callback: F)
    where
        F: FnMut(&LocalName),
    {
    }

    fn has_dirty_descendants(&self) -> bool {
        false
    }
    fn has_snapshot(&self) -> bool {
        false
    }
    fn handled_snapshot(&self) -> bool {
        true
    }
    unsafe fn set_handled_snapshot(&self) {}
    unsafe fn set_dirty_descendants(&self) {}
    unsafe fn unset_dirty_descendants(&self) {}

    fn store_children_to_process(&self, _n: isize) {}
    fn did_process_child(&self) -> isize {
        unimplemented!("did_process_child is parallel-traversal only; not used here")
    }

    unsafe fn ensure_data(&self) -> ElementDataMut<'_> {
        unimplemented!("ensure_data needs &mut store; use ElementDataStore::ensure directly")
    }
    unsafe fn clear_data(&self) {
        unimplemented!("clear_data needs &mut store; use ElementDataStore::clear directly")
    }
    fn has_data(&self) -> bool {
        self.store.has_data(self.node)
    }
    fn borrow_data(&self) -> Option<ElementDataRef<'_>> {
        self.store.borrow(self.node)
    }
    fn mutate_data(&self) -> Option<ElementDataMut<'_>> {
        self.store.borrow_mut(self.node)
    }

    fn skip_item_display_fixup(&self) -> bool {
        false
    }
    fn may_have_animations(&self) -> bool {
        false
    }
    fn has_animations(&self, _: &SharedStyleContext) -> bool {
        false
    }
    fn has_css_animations(
        &self,
        _: &SharedStyleContext,
        _: Option<stylo::selector_parser::PseudoElement>,
    ) -> bool {
        false
    }
    fn has_css_transitions(
        &self,
        _: &SharedStyleContext,
        _: Option<stylo::selector_parser::PseudoElement>,
    ) -> bool {
        false
    }

    fn shadow_root(&self) -> Option<StyloShadowRoot<'a>> {
        None
    }
    fn containing_shadow(&self) -> Option<StyloShadowRoot<'a>> {
        None
    }

    fn lang_attr(&self) -> Option<AttrValue> {
        None
    }
    fn match_element_lang(&self, _override_lang: Option<Option<AttrValue>>, _value: &Lang) -> bool {
        false
    }
    fn is_html_document_body_element(&self) -> bool {
        self.dom.tag_name(self.node) == Some("body")
    }

    fn synthesize_presentational_hints_for_legacy_attributes<V>(
        &self,
        _visited_handling: selectors::matching::VisitedHandlingMode,
        _hints: &mut V,
    ) where
        V: selectors::sink::Push<stylo::applicable_declarations::ApplicableDeclarationBlock>,
    {
        // Presentational hints (e.g. <img width>) are handled by our own UA pass.
    }

    fn local_name(
        &self,
    ) -> &<stylo::selector_parser::SelectorImpl as selectors::parser::SelectorImpl>::BorrowedLocalName
    {
        unimplemented!("TElement::local_name (interned) is not used by the None-cascade path")
    }
    fn namespace(
        &self,
    ) -> &<stylo::selector_parser::SelectorImpl as selectors::parser::SelectorImpl>::BorrowedNamespaceUrl
    {
        unimplemented!("TElement::namespace (interned) is not used by the None-cascade path")
    }

    fn query_container_size(
        &self,
        _display: &stylo::values::computed::Display,
    ) -> euclid::default::Size2D<Option<Au>> {
        // Answered from the PREVIOUS layout pass's content-box sizes (installed only on the
        // sized re-pass — see `ElementDataStore::set_container_sizes`). Stylo only calls this
        // on an element it has already validated as a container, but the AXES offered must
        // still honour `container-type`: an `inline-size` container answers width only, so a
        // height query against it stays unknown (and its rule off) exactly as in Chrome.
        let Some((w, h)) = self.store.container_size(self.node) else {
            return euclid::default::Size2D::new(None, None);
        };
        let ct = match self.store.borrow(self.node) {
            Some(data) => match data.styles.get_primary() {
                Some(style) => style.get_box().clone_container_type(),
                None => return euclid::default::Size2D::new(None, None),
            },
            None => return euclid::default::Size2D::new(None, None),
        };
        use stylo::values::computed::ContainerType;
        if ct.intersects(ContainerType::SIZE) {
            euclid::default::Size2D::new(Some(Au::from_f32_px(w)), Some(Au::from_f32_px(h)))
        } else if ct.intersects(ContainerType::INLINE_SIZE) {
            // Horizontal writing modes only in this engine: inline axis == width.
            euclid::default::Size2D::new(Some(Au::from_f32_px(w)), None)
        } else {
            euclid::default::Size2D::new(None, None)
        }
    }

    fn has_selector_flags(&self, _flags: ElementSelectorFlags) -> bool {
        false
    }
    fn relative_selector_search_direction(&self) -> ElementSelectorFlags {
        ElementSelectorFlags::empty()
    }
}
