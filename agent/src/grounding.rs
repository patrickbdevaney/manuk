//! Action grounding (L31, HEADLESS slice): turn a model-proposed [`Action`] into a **concrete,
//! executable target** on the current page by resolving its textual intent against the
//! accessibility tree with the dual semantic+visual scorer ([`crate::targeting::resolve_target`]).
//!
//! This is the deterministic half of prompt→action grounding: given the model's structured
//! output (a `ClickText`/`Type`/`ScrollTo`/… `Action`) it produces the node + click point to act
//! on, or reports the target as **ambiguous** (below a confidence margin — the caller should
//! disambiguate rather than act) or **unresolved**. The model *inference* that produces the
//! `Action` is the separate, backend-injected (external) half; this layer is pure and unit-tested
//! with canned actions + trees, so an agent's targeting is verifiable without a model.

use manuk_a11y::{A11yNode, Rect, Role};
use manuk_dom::NodeId;

use crate::targeting::resolve_target;
use crate::Action;

/// Default confidence margin below which a resolved target is treated as ambiguous. The margin is
/// the winner's lead over the runner-up (see [`crate::targeting::Targeted::confidence`]); a small
/// lead means several elements fit the intent about equally.
pub const DEFAULT_MIN_CONFIDENCE: f32 = 0.2;

/// The outcome of grounding an [`Action`] against a page.
#[derive(Clone, Debug, PartialEq)]
pub enum Grounded {
    /// The action targets no on-page element (navigate, scroll, back/forward, finish, click-by-
    /// index, click-at-coordinate, tab ops) — execute it directly, no a11y resolution needed.
    Direct,
    /// Resolved to a concrete node with acceptable confidence — click/type at `point`.
    Ready {
        node: NodeId,
        role: Role,
        name: String,
        point: (f32, f32),
        confidence: f32,
    },
    /// The target was resolved unambiguously and **something is on top of it** — no point inside
    /// its visible box hit-tests back to it. Clicking `point` would activate `by`, not the target.
    ///
    /// ⭐ This is reported, not refused, and the distinction is the capability: an agent that
    /// knows a consent banner covers the *Sign in* button can dismiss the banner and retry, while
    /// an agent handed `Ready` with a confidence of `1.0` clicks the banner, sees nothing change,
    /// and has no way to find out why. A caller that holds a node handle may still activate the
    /// node directly (`Browser::click_by_name` does) — that path is unchanged.
    Obstructed {
        node: NodeId,
        name: String,
        /// What intercepted the click.
        by: NodeId,
        point: (f32, f32),
        confidence: f32,
    },
    /// ⭐⭐⭐ **Resolved, and it is OFF THE SCREEN — scroll by `dy` and ground again.** Carries no
    /// point, deliberately: a coordinate outside the viewport is one no pointer can occupy, and an
    /// agent handed one clicks into the void and is told it succeeded.
    ///
    /// **Re-grounding after the scroll is not politeness, it is the correctness argument.** The
    /// obstruction map is a function of the scroll position — a `position:sticky` header's
    /// document rect moves with the view — so a point verified against the viewport the target was
    /// *found* in says nothing about the viewport the click *happens* in. Scrolling to a
    /// below-the-fold control is the ordinary way the web is driven, and it is exactly the motion
    /// that slides a sticky header over the thing you scrolled to.
    OffScreen {
        node: NodeId,
        name: String,
        /// Signed scroll delta (positive = down) that brings the target into view.
        dy: f32,
        confidence: f32,
    },
    /// A best target exists but its lead over the runner-up is below the threshold — the caller
    /// should disambiguate (ask, or narrow the intent) rather than act blindly.
    Ambiguous {
        node: NodeId,
        name: String,
        confidence: f32,
    },
    /// No element on the page matches the action's intent.
    Unresolved,
}

/// The textual intent an action wants to target on the page, or `None` if the action needs no
/// on-page target (it is [`Grounded::Direct`]).
fn action_intent(action: &Action) -> Option<String> {
    match action {
        Action::ClickText { name, .. } | Action::ScrollTo { name, .. } => Some(name.clone()),
        Action::Type { field, .. } => Some(field.clone()),
        Action::Submit { field: Some(f) } if !f.is_empty() => Some(f.clone()),
        _ => None,
    }
}

/// Ground `action` against `tree` for the given `viewport`, gating on `min_confidence`. Text-
/// targeting actions (`ClickText`/`ScrollTo`/`Type`/`Submit{field}`) resolve to a node via the
/// dual scorer; everything else is [`Grounded::Direct`].
pub fn ground_action(
    action: &Action,
    tree: &A11yNode,
    viewport: Rect,
    min_confidence: f32,
) -> Grounded {
    let Some(intent) = action_intent(action) else {
        return Grounded::Direct;
    };
    match resolve_target(tree, &intent, viewport) {
        // Ambiguity is checked FIRST: "which element did you mean" outranks "can it be reached",
        // because the obstruction answer is about a node that may not be the intended one.
        // "Where is it" is asked before "what is on top of it": a target the viewport does not
        // contain has no obstruction answer yet, because the obstruction map belongs to a scroll
        // position the caller has not reached.
        Some(t) if t.confidence >= min_confidence && t.offscreen_dy.is_some() => {
            Grounded::OffScreen {
                node: t.node,
                name: t.name,
                dy: t.offscreen_dy.unwrap_or_default(),
                confidence: t.confidence,
            }
        }
        Some(t) if t.confidence >= min_confidence => match t.obstructed_by {
            Some(by) => Grounded::Obstructed {
                node: t.node,
                name: t.name,
                by,
                point: t.point,
                confidence: t.confidence,
            },
            None => Grounded::Ready {
                node: t.node,
                role: t.role,
                name: t.name,
                point: t.point,
                confidence: t.confidence,
            },
        },
        Some(t) => Grounded::Ambiguous {
            node: t.node,
            name: t.name,
            confidence: t.confidence,
        },
        None => Grounded::Unresolved,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(id: u64, role: Role, name: &str, bbox: Option<Rect>, children: Vec<A11yNode>) -> A11yNode {
        A11yNode {
            node: NodeId(id as u64),
            role,
            name: name.to_string(),
            bbox,
            z: 0,
            hittable: true,
            state: Default::default(),
            children,
        }
    }
    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    fn login_form() -> A11yNode {
        n(
            1,
            Role::Document,
            "",
            Some(rect(0.0, 0.0, 1000.0, 800.0)),
            vec![
                n(
                    2,
                    Role::TextBox,
                    "Email",
                    Some(rect(400.0, 200.0, 200.0, 30.0)),
                    vec![],
                ),
                n(
                    3,
                    Role::TextBox,
                    "Password",
                    Some(rect(400.0, 250.0, 200.0, 30.0)),
                    vec![],
                ),
                n(
                    4,
                    Role::Button,
                    "Sign in",
                    Some(rect(400.0, 300.0, 200.0, 40.0)),
                    vec![],
                ),
            ],
        )
    }
    fn vp() -> Rect {
        rect(0.0, 0.0, 1000.0, 800.0)
    }

    #[test]
    fn grounds_click_text_to_the_button() {
        let tree = login_form();
        let a = Action::ClickText {
            role: "button".into(),
            name: "Sign in".into(),
        };
        match ground_action(&a, &tree, vp(), DEFAULT_MIN_CONFIDENCE) {
            Grounded::Ready {
                node, name, point, ..
            } => {
                assert_eq!(node, NodeId(4));
                assert_eq!(name, "Sign in");
                assert_eq!(point, (500.0, 320.0));
            }
            other => panic!("expected Ready, got {other:?}"),
        }
    }

    #[test]
    fn grounds_type_to_the_named_field() {
        let tree = login_form();
        let a = Action::Type {
            field: "Password".into(),
            text: "hunter2".into(),
        };
        match ground_action(&a, &tree, vp(), DEFAULT_MIN_CONFIDENCE) {
            Grounded::Ready { node, .. } => assert_eq!(node, NodeId(3), "the Password box"),
            other => panic!("expected Ready, got {other:?}"),
        }
    }

    #[test]
    fn direct_actions_need_no_target() {
        let tree = login_form();
        for a in [
            Action::Navigate {
                url: "https://x".into(),
            },
            Action::Scroll { dy: 100.0 },
            Action::Back,
            Action::ClickAt { x: 1.0, y: 2.0 },
            Action::Finish {
                answer: "done".into(),
            },
        ] {
            assert_eq!(
                ground_action(&a, &tree, vp(), DEFAULT_MIN_CONFIDENCE),
                Grounded::Direct
            );
        }
    }

    #[test]
    fn unmatched_intent_is_unresolved() {
        let tree = login_form();
        let a = Action::ClickText {
            role: "button".into(),
            name: "Checkout".into(),
        };
        assert_eq!(
            ground_action(&a, &tree, vp(), DEFAULT_MIN_CONFIDENCE),
            Grounded::Unresolved
        );
    }

    #[test]
    fn near_ties_are_flagged_ambiguous() {
        // Two identically-named buttons, equally salient → the winner's lead is tiny.
        let tree = n(
            1,
            Role::Document,
            "",
            Some(rect(0.0, 0.0, 1000.0, 800.0)),
            vec![
                n(
                    2,
                    Role::Button,
                    "Continue",
                    Some(rect(380.0, 380.0, 100.0, 40.0)),
                    vec![],
                ),
                n(
                    3,
                    Role::Button,
                    "Continue",
                    Some(rect(520.0, 380.0, 100.0, 40.0)),
                    vec![],
                ),
            ],
        );
        let a = Action::ClickText {
            role: "button".into(),
            name: "Continue".into(),
        };
        match ground_action(&a, &tree, vp(), DEFAULT_MIN_CONFIDENCE) {
            Grounded::Ambiguous { confidence, .. } => {
                assert!(
                    confidence < DEFAULT_MIN_CONFIDENCE,
                    "flagged below threshold: {confidence}"
                );
            }
            other => panic!("expected Ambiguous, got {other:?}"),
        }
    }
}
