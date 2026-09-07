//! **G_THE_DENOMINATOR_IS_PUBLISHED — a metric that excludes what is not rendered rewards a browser
//! that renders nothing, and this one did for nine ticks.**
//!
//! The accessibility tree omits boxless nodes, so an incompletely loaded page emits fewer phantoms
//! and its omissions are invisible to precision. That is not hypothetical:
//!
//! ```text
//!                          <li> in DOM    with a box       reported F1
//!   without finish_loading      770            126             94.8%   ← withdrawn at t1470
//!   with finish_loading         776            614             82.0%   ← honest
//! ```
//!
//! ⭐⭐⭐ **The number that looked good was the browser not having laid out 84% of the document**, and
//! nothing in the output said so. Surface audit #89 ranked publishing the denominator #1: *"the next
//! under-rendering regression flatters itself the same way."* `Rendered { nodes, boxed }` is that
//! column, printed beside every rate.
//!
//! ⚠ **It counts nodes the a11y tree kept, not DOM elements** — this crate cannot see the DOM from
//! here, and the tree is what the score is computed over. So it is a *lower bound* on
//! under-rendering, stated rather than implied.
//!
//! Mutations that must turn this red:
//!   1. `rendered` counts every node as boxed        → the hidden-subtree row reads 100%
//!   2. `rendered` accepts a zero-area box           → same
//!   3. `rate()` divides by `boxed` instead of nodes → the rate is always 100%

use manuk_a11y::{A11yNode, A11yState, Rect, Role};
use manuk_agent::a11y_score::rendered;

fn node(role: Role, w: f32, h: f32, kids: Vec<A11yNode>) -> A11yNode {
    A11yNode {
        node: manuk_dom::NodeId(1),
        role,
        name: String::new(),
        bbox: if w < 0.0 {
            None
        } else {
            Some(Rect {
                x: 0.0,
                y: 0.0,
                width: w,
                height: h,
            })
        },
        z: 0,
        hittable: true,
        state: A11yState::default(),
        children: kids,
    }
}

#[test]
fn the_rendered_denominator_sees_a_page_that_did_not_lay_out() {
    // ── A fully laid-out tree: root (no box, as documents have none) + three boxed children.
    let full = node(
        Role::Document,
        -1.0,
        0.0,
        vec![
            node(Role::Paragraph, 100.0, 20.0, vec![]),
            node(Role::Link, 50.0, 10.0, vec![]),
            node(Role::Heading { level: 2 }, 80.0, 24.0, vec![]),
        ],
    );
    let r = rendered(&full);
    assert_eq!((r.nodes, r.boxed), (4, 3));

    // ── VACUITY. A laid-out page must NOT read low, or the column cannot distinguish the two cases
    //    and would fire on every healthy measurement.
    assert!(
        r.rate() > 0.7,
        "VACUOUS: a laid-out tree reads {:.2} — the column would flag every healthy page",
        r.rate()
    );

    // ── ⭐ THE FAILURE MODE IT EXISTS FOR: the same tree, not laid out. Precision and recall cannot
    //    tell these apart — the boxless nodes simply leave the comparison — and `rendered` can.
    let unlaid = node(
        Role::Document,
        -1.0,
        0.0,
        vec![
            node(Role::Paragraph, -1.0, 0.0, vec![]),
            node(Role::Link, 0.0, 0.0, vec![]), // a ZERO-AREA box is not laid out either
            node(Role::Heading { level: 2 }, 80.0, 24.0, vec![]),
        ],
    );
    let u = rendered(&unlaid);
    assert_eq!((u.nodes, u.boxed), (4, 1));
    assert!(
        u.rate() < 0.3,
        "an under-rendered tree must read LOW — got {:.2}",
        u.rate()
    );

    // ── The two must be distinguishable by a wide margin, or the column is decoration.
    assert!(
        r.rate() - u.rate() > 0.4,
        "rendered must separate a laid-out page from an unlaid one by a wide margin: {:.2} vs {:.2}",
        r.rate(),
        u.rate()
    );

    // An empty tree divides by zero if nobody guards it.
    assert_eq!(manuk_agent::a11y_score::Rendered::default().rate(), 0.0);
}
