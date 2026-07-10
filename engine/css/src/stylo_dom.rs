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
//! **Status:** this is the store + handle foundation (compiles + tested). The ~126
//! trait-body methods (`TElement` 76, `TNode` 20, `selectors::Element` 30) that drive
//! Stylo's matcher/cascade over these handles — including bridging our plain-`String`
//! names to Servo's interned-atom `SelectorImpl` — are the dedicated multi-session
//! fill-in tracked in CLAUDE.md § D2. Step-0 (`stylo_probe`) already proved the
//! non-DOM half (Device + parser + Stylist).

use std::collections::HashMap;

use atomic_refcell::{AtomicRef, AtomicRefCell, AtomicRefMut};
use manuk_dom::{Dom, NodeId};
use stylo::data::ElementData;

/// A `NodeId`-indexed side-table of Stylo's per-element `AtomicRefCell<ElementData>`.
/// Only element nodes get an entry (text/comment nodes have no style state), so a
/// `HashMap` (not a dense `Vec`) avoids bloating on non-element nodes.
#[derive(Default)]
pub struct ElementDataStore {
    data: HashMap<NodeId, AtomicRefCell<ElementData>>,
}

impl ElementDataStore {
    pub fn new() -> Self {
        ElementDataStore {
            data: HashMap::new(),
        }
    }

    /// Ensure `node` has an `ElementData` cell (idempotent) — Stylo's `ensure_data`.
    pub fn ensure(&mut self, node: NodeId) {
        self.data
            .entry(node)
            .or_insert_with(|| AtomicRefCell::new(ElementData::default()));
    }

    /// Runtime-checked shared borrow of a node's `ElementData` (Stylo's `borrow_data`).
    pub fn borrow(&self, node: NodeId) -> Option<AtomicRef<'_, ElementData>> {
        self.data.get(&node).map(|c| c.borrow())
    }

    /// Runtime-checked mutable borrow (Stylo's `mutate_data`).
    pub fn borrow_mut(&self, node: NodeId) -> Option<AtomicRefMut<'_, ElementData>> {
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
    pub fn data(&self) -> Option<AtomicRef<'a, ElementData>> {
        self.store.borrow(self.node)
    }
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
}
