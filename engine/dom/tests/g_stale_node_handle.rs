//! **G_STALE_NODE_HANDLE — the arena's MUTATORS never asked whether the handle was still valid.**
//!
//! ```text
//!   a DOM native PANICKED — CONTAINED   native="el_append_child"
//!   index out of bounds: the len is 8 but the index is 337
//! ```
//!
//! Observed by the t1316 real-site fidelity sweep on `admin.munchbakery.com` — a live commercial
//! site, not a fixture. ⚠ The containment in `dom_bindings` is the only reason it was a wrong answer
//! rather than a dead browser: a panic cannot unwind out of `extern "C"`, so without that boundary it
//! aborts the process and **every tab in it**. And a caught panic is not a handled error — the
//! `appendChild` simply did not happen, so the page silently lost a subtree.
//!
//! ⭐⭐⭐ **THE OUT-OF-BOUNDS CASE IS THE MILD ONE, AND A BOUNDS CHECK WOULD HAVE HIDDEN THE OTHER.**
//! `NodeId` packs a slot index AND the generation it was minted at. An out-of-range handle panics —
//! loud, contained, findable. A handle whose slot has since been **reused** passes any bounds test
//! and **silently mutates a different node**: no panic, no error, wrong DOM. `t2` is that row, and it
//! is why this gate lives at the arena boundary rather than being driven through JS.
//!
//! ⚠⚠ **THE FIRST VERSION OF THIS GATE WAS VACUOUS AND WAS DELETED.** It drove the four mutators
//! from a JS fixture — stale handles, non-child reference nodes, re-appends after `innerHTML = ''` —
//! and it **passed with the fix disabled**, because every handle script can name in a document was
//! allocated by that document's own arena. Chasing the live site's path (`DOMParser`,
//! `createHTMLDocument`, an iframe's `contentDocument`, `document.open`) reproduced nothing either.
//! The defect is at the ARENA boundary, so the gate belongs where a dead handle can actually be
//! constructed. **A gate that cannot fail is worse than no gate.**
//!
//! ⭐ `Dom::is_alive` — bounds, liveness AND generation — has been correct in this file the whole
//! time; the mutators simply never asked it. Borrowed, not re-derived.

use manuk_dom::{Dom, NodeId};

#[test]
fn a_mutator_refuses_a_handle_the_arena_no_longer_honours() {
    // ── t1: OUT OF RANGE — the exact shape the live site produced.
    let mut dom = Dom::new();
    let root = dom.root();
    let real = dom.create_element("div");
    dom.append_child(root, real);
    let before = dom.children(root).count();

    let forged = NodeId(337);
    assert!(
        !dom.is_alive(forged),
        "the fixture must actually be forging a dead handle, or this gate measures nothing"
    );
    // Each of these indexed `self.nodes[..]` raw and panicked with
    // `index out of bounds: the len is N but the index is 337`.
    dom.append_child(root, forged);
    dom.append_child(forged, real);
    dom.insert_before(root, forged, real);
    dom.detach(forged);
    assert!(!dom.remove_child(root, forged));

    assert_eq!(
        dom.children(root).count(),
        before,
        "a refused mutation must leave the tree exactly as it was — the forged handle must not be \
         linked in, and the real child must not be unlinked"
    );

    // ── t2: THE REUSED SLOT, which no bounds check can catch. Free a node so its slot is recycled,
    // then use the OLD handle: same index, stale generation.
    let victim = dom.create_element("span");
    dom.append_child(root, victim);
    let stale = victim;
    dom.discard_subtree(victim);
    let reused = dom.create_element("b");
    assert_eq!(
        stale.index(),
        reused.index(),
        "this row needs the slot to be RECYCLED to mean anything; if the arena stopped reusing \
         slots, this assertion is the notice — not a failure of the engine"
    );
    assert!(
        !dom.is_alive(stale) && dom.is_alive(reused),
        "the stale handle must be dead and the new occupant alive — that is the whole distinction a \
         bounds check cannot see"
    );
    dom.append_child(root, reused);
    let kids_before = dom.children(root).count();
    // Under a bounds-only guard this SUCCEEDS and detaches the WRONG node — silently.
    dom.detach(stale);
    assert_eq!(
        dom.children(root).count(),
        kids_before,
        "detaching a STALE handle must be a no-op. If this drops a child, the arena followed a \
         recycled index and unlinked whatever now lives in that slot — a wrong DOM with no panic and \
         no error, which is strictly worse than the crash this gate started from"
    );

    // ── n1, THE CONTROL: after five refusals, ordinary mutation still works. A guard that refused
    // everything would pass every row above and fail only here.
    let fresh = dom.create_element("p");
    dom.append_child(root, fresh);
    assert!(
        dom.children(root).any(|c| c == fresh),
        "an ordinary appendChild with a LIVE handle must still work — if this fails, the backstop is \
         refusing valid nodes and has traded a rare crash for a broken DOM"
    );
}
