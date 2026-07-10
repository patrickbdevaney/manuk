//! D2 **impedance resolution** — attaching Stylo's per-element style state to our
//! arena `manuk-dom`, and the handle type its DOM trait wall implements over.
//!
//! Stylo keeps each element's computed styles + restyle state in an
//! `AtomicRefCell<ElementData>` that, in Servo/Gecko, hangs off the element pointer.
//! Our arena DOM (`NodeId` indices) has no per-node pointer to hang it on — the
//! resolved impedance (pass-2) is a **`NodeId`-indexed side-table** of exactly that
//! cell ([`ElementDataStore`]), plus a lightweight `(&Dom, NodeId, &store)` handle
//! ([`StyloElement`]) over which `TElement`/`TNode`/`selectors::Element` are
//! implemented — the `blitz-dom` pattern (`atomic_refcell` + Stylo), without adopting
//! Servo's heavier machinery.
//!
//! **Status:** the store + handle foundation and the **`selectors::Element` wall
//! (30 methods) are landed and tested** — Stylo's *real* selector matcher
//! (`selectors::matching::matches_selector`), fed a Servo-parsed selector over
//! Servo's interned-atom `SelectorImpl`, matches against our arena DOM end-to-end
//! (type/id/class/attr operators, descendant + child combinators, `:empty`; see
//! `stylo_selector_matcher_runs_over_arena_dom`). The plain-`String` arena names
//! bridge to the interned atoms via deref-to-`str` at each comparison. Remaining for
//! the full cascade: the `TNode` (20) + `TElement` (76) wall and a
//! `ComputedValues`→`ComputedStyle` mapping — tracked in CLAUDE.md § D2. Pseudo-
//! classes, shadow DOM, `::part`, and custom state in `selectors::Element` return
//! `false`/`None` for now (documented in the impl). Step-0 (`stylo_probe`) proved the
//! non-DOM half (Device + parser + Stylist).

use std::collections::HashMap;

use manuk_dom::{Dom, NodeId};
use stylo::data::{ElementDataMut, ElementDataRef, ElementDataWrapper};

/// A `NodeId`-indexed side-table of Stylo's per-element style state.
///
/// The cell is Stylo's own **`ElementDataWrapper`** (an `UnsafeCell<ElementData>` +
/// a debug-only thread-safety check) — the *exact* type whose `borrow()`/`borrow_mut()`
/// produce the `ElementDataRef`/`ElementDataMut` that `TElement::borrow_data`/
/// `mutate_data` must return (those handles have private fields and are constructible
/// only through the wrapper). Storing the wrapper here — rather than a bare
/// `AtomicRefCell<ElementData>` — is what lets our arena satisfy `TElement`'s data
/// contract with Servo's genuine types. Only element nodes get an entry (text/comment
/// nodes have no style state), so a `HashMap` (not a dense `Vec`) avoids bloat.
#[derive(Default)]
pub struct ElementDataStore {
    data: HashMap<NodeId, ElementDataWrapper>,
}

impl ElementDataStore {
    pub fn new() -> Self {
        ElementDataStore {
            data: HashMap::new(),
        }
    }

    /// Ensure `node` has an `ElementData` cell (idempotent) — Stylo's `ensure_data`.
    pub fn ensure(&mut self, node: NodeId) {
        self.data.entry(node).or_default();
    }

    /// Whether `node` has style state attached (Stylo's `has_data`).
    pub fn has_data(&self, node: NodeId) -> bool {
        self.data.contains_key(&node)
    }

    /// Runtime-checked shared borrow of a node's `ElementData` (Stylo's `borrow_data`).
    /// Returns Stylo's own `ElementDataRef` — the exact `TElement::borrow_data` type.
    pub fn borrow(&self, node: NodeId) -> Option<ElementDataRef<'_>> {
        self.data.get(&node).map(|c| c.borrow())
    }

    /// Runtime-checked mutable borrow (Stylo's `mutate_data`), yielding `ElementDataMut`.
    pub fn borrow_mut(&self, node: NodeId) -> Option<ElementDataMut<'_>> {
        self.data.get(&node).map(|c| c.borrow_mut())
    }

    /// Drop a node's style state (on element removal / a full restyle reset).
    pub fn clear(&mut self, node: NodeId) {
        self.data.remove(&node);
    }

    /// Drop all style state.
    pub fn clear_all(&mut self) {
        self.data.clear();
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// A lightweight handle over `(dom, node)` plus the element-data store — the type the
/// Stylo DOM trait wall (`TNode`/`TElement`/`selectors::Element`) is implemented over.
/// `Copy` so it can be passed freely through Stylo's traversal.
///
/// The trait `impl`s themselves are the multi-session fill-in (see module docs); this
/// carries exactly the state they need (tree navigation via `dom`, style state via
/// `store`).
#[derive(Clone, Copy)]
pub struct StyloElement<'a> {
    pub dom: &'a Dom,
    pub node: NodeId,
    pub store: &'a ElementDataStore,
}

impl<'a> StyloElement<'a> {
    pub fn new(dom: &'a Dom, node: NodeId, store: &'a ElementDataStore) -> Self {
        StyloElement { dom, node, store }
    }

    /// Nearest ancestor element (skips non-element nodes) — the shape
    /// `selectors::Element::parent_element` needs.
    pub fn parent_element(&self) -> Option<StyloElement<'a>> {
        let mut cur = self.dom.parent(self.node);
        while let Some(p) = cur {
            if self.dom.is_element(p) {
                return Some(StyloElement::new(self.dom, p, self.store));
            }
            cur = self.dom.parent(p);
        }
        None
    }

    /// This element's `ElementData`, if it has been `ensure`d.
    pub fn data(&self) -> Option<ElementDataRef<'a>> {
        self.store.borrow(self.node)
    }

    fn tag(&self) -> &'a str {
        self.dom.tag_name(self.node).unwrap_or("")
    }

    fn attr(&self, name: &str) -> Option<&'a str> {
        self.dom.element(self.node).and_then(|e| e.attr(name))
    }

    fn with(&self, node: NodeId) -> StyloElement<'a> {
        StyloElement::new(self.dom, node, self.store)
    }

    fn prev_element_sibling(&self) -> Option<StyloElement<'a>> {
        let mut cur = prev_sibling(self.dom, self.node);
        while let Some(n) = cur {
            if self.dom.is_element(n) {
                return Some(self.with(n));
            }
            cur = prev_sibling(self.dom, n);
        }
        None
    }

    fn next_element_sibling(&self) -> Option<StyloElement<'a>> {
        let mut cur = self.dom.next_sibling(self.node);
        while let Some(n) = cur {
            if self.dom.is_element(n) {
                return Some(self.with(n));
            }
            cur = self.dom.next_sibling(n);
        }
        None
    }

    fn first_element_child(&self) -> Option<StyloElement<'a>> {
        let mut cur = self.dom.first_child(self.node);
        while let Some(n) = cur {
            if self.dom.is_element(n) {
                return Some(self.with(n));
            }
            cur = self.dom.next_sibling(n);
        }
        None
    }
}

/// The previous sibling of `node` (the arena exposes next/first but this walk needs
/// prev — derive it from the parent's child list).
fn prev_sibling(dom: &Dom, node: NodeId) -> Option<NodeId> {
    let parent = dom.parent(node)?;
    let mut prev = None;
    let mut cur = dom.first_child(parent);
    while let Some(n) = cur {
        if n == node {
            return prev;
        }
        prev = Some(n);
        cur = dom.next_sibling(n);
    }
    None
}

impl std::fmt::Debug for StyloElement<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<{} #{}>", self.tag(), self.node.0)
    }
}

// -- The selector-matching trait wall (D2): Stylo's real matcher over the arena DOM.
// Reuses Servo's `SelectorImpl` (interned atoms); our plain-`String` names bridge via
// deref-to-`str`. Pseudo-classes/elements, shadow DOM, `::part`, and custom state are
// not modelled yet (return `false`/`None`) — a documented, well-bounded first tranche.
mod selector_impl {
    use super::{prev_sibling, StyloElement};
    use selectors::attr::{AttrSelectorOperation, CaseSensitivity, NamespaceConstraint};
    use selectors::bloom::BloomFilter;
    use selectors::matching::{ElementSelectorFlags, MatchingContext};
    use selectors::{Element, OpaqueElement};
    use stylo::selector_parser::SelectorImpl;

    /// Compare `a` and `b` under the requested case sensitivity.
    fn eq_case(a: &str, b: &str, case: CaseSensitivity) -> bool {
        match case {
            CaseSensitivity::CaseSensitive => a == b,
            CaseSensitivity::AsciiCaseInsensitive => a.eq_ignore_ascii_case(b),
        }
    }

    impl<'a> Element for StyloElement<'a> {
        type Impl = SelectorImpl;

        fn opaque(&self) -> OpaqueElement {
            // A stable per-element identity for the duration of a (read-only) match:
            // the arena `Node`'s address (the Vec does not reallocate while matching).
            OpaqueElement::new(self.dom.node(self.node))
        }

        fn parent_element(&self) -> Option<Self> {
            StyloElement::parent_element(self)
        }

        fn parent_node_is_shadow_root(&self) -> bool {
            false
        }

        fn containing_shadow_host(&self) -> Option<Self> {
            None
        }

        fn is_pseudo_element(&self) -> bool {
            false
        }

        fn prev_sibling_element(&self) -> Option<Self> {
            self.prev_element_sibling()
        }

        fn next_sibling_element(&self) -> Option<Self> {
            self.next_element_sibling()
        }

        fn first_element_child(&self) -> Option<Self> {
            StyloElement::first_element_child(self)
        }

        fn is_html_element_in_html_document(&self) -> bool {
            true // the arena DOM models HTML documents
        }

        fn has_local_name(
            &self,
            name: &<SelectorImpl as ::selectors::SelectorImpl>::BorrowedLocalName,
        ) -> bool {
            self.tag() == &**name
        }

        fn has_namespace(
            &self,
            _ns: &<SelectorImpl as ::selectors::SelectorImpl>::BorrowedNamespaceUrl,
        ) -> bool {
            // Single (HTML) namespace: an explicit-namespace selector matches it.
            true
        }

        fn is_same_type(&self, other: &Self) -> bool {
            self.tag() == other.tag()
        }

        fn attr_matches(
            &self,
            _ns: &NamespaceConstraint<&stylo::Namespace>,
            local_name: &stylo::LocalName,
            operation: &AttrSelectorOperation<&stylo::values::AtomString>,
        ) -> bool {
            let value = self.attr(local_name);
            match operation {
                AttrSelectorOperation::Exists => value.is_some(),
                AttrSelectorOperation::WithValue { .. } => {
                    value.is_some_and(|v| operation.eval_str(v))
                }
            }
        }

        fn match_non_ts_pseudo_class(
            &self,
            _pc: &stylo::selector_parser::NonTSPseudoClass,
            _context: &mut MatchingContext<'_, SelectorImpl>,
        ) -> bool {
            false // pseudo-classes (:hover/:focus/:link…) not modelled yet
        }

        fn match_pseudo_element(
            &self,
            _pe: &stylo::selector_parser::PseudoElement,
            _context: &mut MatchingContext<'_, SelectorImpl>,
        ) -> bool {
            false
        }

        fn apply_selector_flags(&self, _flags: ElementSelectorFlags) {
            // Invalidation/nth-child bookkeeping — a no-op for one-shot matching.
        }

        fn is_link(&self) -> bool {
            matches!(self.tag(), "a" | "area" | "link") && self.attr("href").is_some()
        }

        fn is_html_slot_element(&self) -> bool {
            false
        }

        fn has_id(&self, id: &stylo::values::AtomIdent, case: CaseSensitivity) -> bool {
            self.attr("id").is_some_and(|v| eq_case(v, id, case))
        }

        fn has_class(&self, name: &stylo::values::AtomIdent, case: CaseSensitivity) -> bool {
            let name: &str = name;
            self.dom
                .element(self.node)
                .map(|e| e.classes().any(|c| eq_case(c, name, case)))
                .unwrap_or(false)
        }

        fn has_custom_state(&self, _name: &stylo::values::AtomIdent) -> bool {
            false
        }

        fn imported_part(
            &self,
            _name: &stylo::values::AtomIdent,
        ) -> Option<stylo::values::AtomIdent> {
            None
        }

        fn is_part(&self, _name: &stylo::values::AtomIdent) -> bool {
            false
        }

        fn is_empty(&self) -> bool {
            // No child elements and no non-whitespace text.
            self.dom.children(self.node).all(|c| {
                if self.dom.is_element(c) {
                    return false;
                }
                self.dom.text_content(c).trim().is_empty()
            })
        }

        fn is_root(&self) -> bool {
            // The root element (<html>) whose parent is the Document node.
            match self.dom.parent(self.node) {
                Some(p) => p == self.dom.root() && self.dom.is_element(self.node),
                None => false,
            }
        }

        fn add_element_unique_hashes(&self, _filter: &mut BloomFilter) -> bool {
            false // opt out of the ancestor bloom-filter fast path
        }
    }

    // Keep `prev_sibling` reachable from this module for the sibling walks above.
    #[allow(unused_imports)]
    use prev_sibling as _prev_sibling;
}

#[cfg(test)]
mod tests {
    use super::*;
    use stylo::invalidation::element::restyle_hints::RestyleHint;

    #[test]
    fn element_data_attaches_per_node_with_borrow_semantics() {
        let mut dom = Dom::new();
        let body = dom.create_element("body");
        let p = dom.create_element("p");
        dom.append_child(dom.root(), body);
        dom.append_child(body, p);

        let mut store = ElementDataStore::new();
        assert!(store.is_empty());
        store.ensure(body);
        store.ensure(p);
        assert_eq!(store.len(), 2);

        // Mutate p's ElementData through the cell, read it back.
        {
            let mut d = store.borrow_mut(p).unwrap();
            d.hint.insert(RestyleHint::RESTYLE_SELF);
        }
        assert!(store
            .borrow(p)
            .unwrap()
            .hint
            .contains(RestyleHint::RESTYLE_SELF));
        // A node without a cell borrows to None.
        assert!(store.borrow(dom.root()).is_none());

        // The handle navigates the arena tree and reaches its ElementData.
        let el = StyloElement::new(&dom, p, &store);
        assert_eq!(el.parent_element().map(|e| e.node), Some(body));
        assert!(el.data().is_some());

        store.clear(p);
        assert!(store.borrow(p).is_none());
    }

    /// End-to-end proof: Stylo's *real* selector matcher (`selectors::matching::
    /// matches_selector`), driven by Servo's `SelectorImpl` and a Servo-parsed
    /// selector, runs over our arena DOM through the `selectors::Element` impl.
    #[test]
    fn stylo_selector_matcher_runs_over_arena_dom() {
        use selectors::context::{
            MatchingContext, MatchingForInvalidation, MatchingMode, NeedsSelectorFlags,
            QuirksMode, SelectorCaches,
        };
        use selectors::matching::matches_selector;
        use stylo::selector_parser::SelectorParser;
        use stylo::servo_arc::Arc as ServoArc;
        use stylo::stylesheets::UrlExtraData;

        // <body><div id="main" class="box wide"><a href="/x">hi</a></div><p></p></body>
        let mut dom = Dom::new();
        let body = dom.create_element("body");
        let div = dom.create_element("div");
        dom.set_attr(div, "id", "main");
        dom.set_attr(div, "class", "box wide");
        let a = dom.create_element("a");
        dom.set_attr(a, "href", "/x");
        let txt = dom.create_text("hi");
        let p = dom.create_element("p");
        dom.append_child(dom.root(), body);
        dom.append_child(body, div);
        dom.append_child(div, a);
        dom.append_child(a, txt);
        dom.append_child(body, p);

        let store = ElementDataStore::new();
        let url = ::url::Url::parse("about:manuk-match").unwrap();
        let url_data = UrlExtraData(ServoArc::new(url));

        // Match one selector string against one arena node, expecting `want`.
        let check = |sel: &str, node: NodeId, want: bool| {
            let list =
                SelectorParser::parse_author_origin_no_namespace(sel, &url_data).unwrap();
            let el = StyloElement::new(&dom, node, &store);
            let mut caches = SelectorCaches::default();
            let mut ctx = MatchingContext::new(
                MatchingMode::Normal,
                None,
                &mut caches,
                QuirksMode::NoQuirks,
                NeedsSelectorFlags::No,
                MatchingForInvalidation::No,
            );
            let got = list
                .slice()
                .iter()
                .any(|s| matches_selector(s, 0, None, &el, &mut ctx));
            assert_eq!(got, want, "selector {sel:?} on node {node:?}");
        };

        // Type, id, class, attribute, descendant, child, and negative cases.
        check("div", div, true);
        check("p", div, false);
        check("#main", div, true);
        check("#other", div, false);
        check(".box", div, true);
        check(".wide.box", div, true);
        check(".missing", div, false);
        check("div#main.box", div, true);
        check("a[href]", a, true);
        check("a[href=\"/x\"]", a, true);
        check("a[href=\"/y\"]", a, false);
        check("body div a", a, true); // descendant combinator walks parents
        check("div > a", a, true); // child combinator
        check("body > a", a, false); // a is not a direct child of body
        check("p:empty", p, true); // structural pseudo over the arena
        check("div:empty", div, false);
    }
}
