//! In-process automation surface (L30): the reliable primitives an external agent or a test
//! script drives the browser with — **stable selectors**, **wait-for conditions**, and
//! **assertions** — all pure functions over the accessibility tree + observation stream.
//!
//! This is the agent-native differentiator: instead of fragile "click element #7" indices, an
//! automation step references an element by durable semantics (role + name), waits on an
//! observable post-condition, and asserts page state — composing with [`crate::targeting`] and
//! [`crate::grounding`]. No timers or I/O live here: the caller feeds observation snapshots and
//! this layer decides, so it stays deterministic and unit-testable.

use manuk_a11y::{A11yNode, Role};
use manuk_dom::NodeId;

/// A durable reference to an element: an optional role, a case-insensitive accessible-name
/// substring, and which match to take (0-based, document order). Because it resolves by semantics
/// rather than position, it survives unrelated DOM mutations that would shift a positional index.
#[derive(Clone, Debug, PartialEq)]
pub struct Selector {
    /// Required role, or `None` to match any role.
    pub role: Option<Role>,
    /// Case-insensitive substring the accessible name must contain (`""` = any).
    pub name: String,
    /// Which match, in document order (0 = first).
    pub nth: usize,
}

impl Selector {
    /// A selector for the `nth`-first element of `role` whose name contains `name`.
    pub fn role_name(role: Role, name: impl Into<String>) -> Self {
        Selector { role: Some(role), name: name.into(), nth: 0 }
    }
    /// A selector matching any role whose name contains `name`.
    pub fn by_name(name: impl Into<String>) -> Self {
        Selector { role: None, name: name.into(), nth: 0 }
    }
    /// Take the `n`-th match instead of the first.
    pub fn nth(mut self, n: usize) -> Self {
        self.nth = n;
        self
    }

    fn matches_node(&self, node: &A11yNode) -> bool {
        let role_ok = self.role.as_ref().is_none_or(|r| &node.role == r);
        let name_ok = self.name.is_empty()
            || node.name.to_ascii_lowercase().contains(&self.name.to_ascii_lowercase());
        role_ok && name_ok
    }
}

/// Every node matching `sel`, document (preorder) order.
fn all_matches<'a>(sel: &Selector, tree: &'a A11yNode, out: &mut Vec<&'a A11yNode>) {
    if sel.matches_node(tree) {
        out.push(tree);
    }
    for c in &tree.children {
        all_matches(sel, c, out);
    }
}

/// Resolve `sel` against `tree` to a concrete node id (the `nth` match), or `None`.
pub fn resolve<'a>(sel: &Selector, tree: &'a A11yNode) -> Option<&'a A11yNode> {
    let mut out = Vec::new();
    all_matches(sel, tree, &mut out);
    out.into_iter().nth(sel.nth)
}

/// Resolve `sel` to just the arena `NodeId`.
pub fn resolve_id(sel: &Selector, tree: &A11yNode) -> Option<NodeId> {
    resolve(sel, tree).map(|n| n.node)
}

/// A checkable condition about page state — the shared vocabulary for wait-for and assertions.
#[derive(Clone, Debug, PartialEq)]
pub enum Condition {
    /// The selector resolves to a node that has a box (present and clickable).
    Visible(Selector),
    /// The selector resolves to nothing (element absent / removed).
    Gone(Selector),
    /// Some node's accessible name contains this text (case-insensitive).
    TextPresent(String),
    /// The current URL contains this substring.
    UrlMatches(String),
    /// At least `n` nodes match the selector.
    CountAtLeast(Selector, usize),
}

fn any_name_contains(tree: &A11yNode, needle_lc: &str) -> bool {
    if !needle_lc.is_empty() && tree.name.to_ascii_lowercase().contains(needle_lc) {
        return true;
    }
    tree.children.iter().any(|c| any_name_contains(c, needle_lc))
}

fn count_matches(sel: &Selector, tree: &A11yNode) -> usize {
    let mut out = Vec::new();
    all_matches(sel, tree, &mut out);
    out.len()
}

/// Evaluate `cond` against an accessibility snapshot `tree` and the current `url`.
pub fn evaluate(cond: &Condition, tree: &A11yNode, url: &str) -> bool {
    match cond {
        Condition::Visible(sel) => resolve(sel, tree).is_some_and(|n| n.bbox.is_some()),
        Condition::Gone(sel) => resolve(sel, tree).is_none(),
        Condition::TextPresent(t) => any_name_contains(tree, &t.to_ascii_lowercase()),
        Condition::UrlMatches(p) => url.contains(p.as_str()),
        Condition::CountAtLeast(sel, n) => count_matches(sel, tree) >= *n,
    }
}

/// The result of [`wait`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// A snapshot satisfied the condition (at 0-based index `at`).
    Met { at: usize },
    /// No snapshot in the (bounded) stream satisfied it.
    Timeout,
}

/// Poll `cond` across a bounded stream of `(tree, url)` observation snapshots — the caller drives
/// the ticks (navigation/relayout produce new snapshots) — returning [`Outcome::Met`] at the
/// first satisfying snapshot, else [`Outcome::Timeout`] when the stream is exhausted. No timers
/// live here; "timeout" = the caller's snapshot budget ran out.
pub fn wait<'a, I>(cond: &Condition, snapshots: I) -> Outcome
where
    I: IntoIterator<Item = (&'a A11yNode, &'a str)>,
{
    for (i, (tree, url)) in snapshots.into_iter().enumerate() {
        if evaluate(cond, tree, url) {
            return Outcome::Met { at: i };
        }
    }
    Outcome::Timeout
}

/// The result of an [`assert_that`] check.
#[derive(Clone, Debug, PartialEq)]
pub struct AssertResult {
    pub passed: bool,
    /// Human-readable detail — empty on pass, the failed condition on failure.
    pub detail: String,
}

/// Assert `cond` holds for the given snapshot — the primitive an automation/test script uses to
/// verify page state after an action. On failure the `detail` names what was expected.
pub fn assert_that(cond: &Condition, tree: &A11yNode, url: &str) -> AssertResult {
    if evaluate(cond, tree, url) {
        AssertResult { passed: true, detail: String::new() }
    } else {
        AssertResult { passed: false, detail: format!("expected {cond:?}") }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use manuk_a11y::Rect;

    fn n(id: u64, role: Role, name: &str, boxed: bool, children: Vec<A11yNode>) -> A11yNode {
        A11yNode {
            node: NodeId(id as usize),
            role,
            name: name.to_string(),
            bbox: boxed.then(|| Rect { x: 0.0, y: 0.0, width: 50.0, height: 20.0 }),
            z: 0,
            children,
        }
    }

    fn page(extra: Vec<A11yNode>) -> A11yNode {
        let mut kids = vec![
            n(2, Role::Heading { level: 1 }, "Dashboard", true, vec![]),
            n(3, Role::Button, "Save", true, vec![]),
        ];
        kids.extend(extra);
        n(1, Role::Document, "", true, kids)
    }

    #[test]
    fn selector_is_stable_across_sibling_mutation() {
        let before = page(vec![]);
        let sel = Selector::role_name(Role::Button, "Save");
        let id_before = resolve_id(&sel, &before).expect("resolves");

        // Insert an unrelated button BEFORE it (a positional index would now be wrong).
        let after = n(1, Role::Document, "", true, vec![
            n(9, Role::Button, "Cancel", true, vec![]),
            n(2, Role::Heading { level: 1 }, "Dashboard", true, vec![]),
            n(3, Role::Button, "Save", true, vec![]),
        ]);
        let id_after = resolve_id(&sel, &after).expect("still resolves");
        assert_eq!(id_before, id_after, "role+name selector survives the sibling insert");
    }

    #[test]
    fn nth_selects_among_duplicates() {
        let tree = page(vec![
            n(4, Role::Link, "Item", true, vec![]),
            n(5, Role::Link, "Item", true, vec![]),
        ]);
        assert_eq!(resolve_id(&Selector::role_name(Role::Link, "Item"), &tree), Some(NodeId(4)));
        assert_eq!(
            resolve_id(&Selector::role_name(Role::Link, "Item").nth(1), &tree),
            Some(NodeId(5))
        );
    }

    #[test]
    fn conditions_evaluate() {
        let tree = page(vec![]);
        let url = "https://app.test/dashboard";
        assert!(evaluate(&Condition::Visible(Selector::role_name(Role::Button, "Save")), &tree, url));
        assert!(evaluate(&Condition::Gone(Selector::role_name(Role::Button, "Delete")), &tree, url));
        assert!(evaluate(&Condition::TextPresent("dashboard".into()), &tree, url));
        assert!(!evaluate(&Condition::TextPresent("checkout".into()), &tree, url));
        assert!(evaluate(&Condition::UrlMatches("/dashboard".into()), &tree, url));
        assert!(!evaluate(&Condition::UrlMatches("/settings".into()), &tree, url));
        assert!(evaluate(&Condition::CountAtLeast(Selector::role_name(Role::Button, ""), 1), &tree, url));
        assert!(!evaluate(&Condition::CountAtLeast(Selector::role_name(Role::Button, ""), 2), &tree, url));
    }

    #[test]
    fn wait_meets_on_a_later_snapshot() {
        let loading = page(vec![]); // no "Done" yet
        let done = page(vec![n(7, Role::Button, "Done", true, vec![])]);
        let url = "https://app.test/";
        let cond = Condition::Visible(Selector::role_name(Role::Button, "Done"));
        let snaps = vec![(&loading, url), (&loading, url), (&done, url)];
        assert_eq!(wait(&cond, snaps), Outcome::Met { at: 2 });
        // Never appears → Timeout.
        assert_eq!(wait(&cond, vec![(&loading, url), (&loading, url)]), Outcome::Timeout);
    }

    #[test]
    fn assert_reports_failure_detail() {
        let tree = page(vec![]);
        let url = "https://app.test/";
        let pass = assert_that(&Condition::Visible(Selector::role_name(Role::Button, "Save")), &tree, url);
        assert!(pass.passed && pass.detail.is_empty());
        let fail = assert_that(&Condition::Visible(Selector::role_name(Role::Button, "Publish")), &tree, url);
        assert!(!fail.passed);
        assert!(fail.detail.contains("Publish"), "detail names the expectation: {}", fail.detail);
    }
}
