//! **Can an agent actually click what it perceives?** — the classification behind `drive-probe`.
//!
//! `g_agent_drive_loop` (t1455) closed the perceive → ground → actuate → observe loop and said
//! plainly what it did not prove: *"what this does not prove is that a real page's markup is
//! reachable this way."* This module is that measurement, and it is deliberately **local** — it asks
//! nothing of Chrome, because the question is not whether our tree matches Chrome's but whether our
//! own tree is **actionable by our own agent**.
//!
//! An agent drives by role + accessible name, aims at a box, and clicks a coordinate. Each of those
//! three steps has its own way of failing, and a target only counts as drivable if it survives all
//! three:
//!
//! | verdict | what went wrong | why the agent is stuck |
//! |---|---|---|
//! | `Ungrounded` | no `bbox`, or a zero-area one | perceivable but not aimable — there is nowhere to click |
//! | `Ambiguous` | another target shares its (role, name) | resolution picks one **by chance**; the agent cannot say which |
//! | `MisHit` | the centre hit-tests to an unrelated node | the click lands somewhere else entirely |
//! | `Drivable` | — | — |
//!
//! ⭐ **A hit on a DESCENDANT is a success, not a miss.** Clicking the centre of a `<button>` very
//! often lands on the `<span>` inside it, and the event bubbles to the button either way. Counting
//! those as failures would report a catastrophe on every well-built page. What must not happen is
//! landing on something that is *not* in the target's subtree.

use manuk_a11y::{A11yNode, Role};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Drivable,
    Ungrounded,
    Ambiguous,
    MisHit,
}

/// The roles an agent is asked to operate. Deliberately narrow: a `paragraph` is perceivable and
/// not actionable, and counting it would dilute the number with nodes nobody wants to click.
pub fn is_actionable(role: &Role) -> bool {
    matches!(
        role,
        Role::Button
            | Role::Link
            | Role::CheckBox
            | Role::Radio
            | Role::TextBox
            | Role::SearchBox
            | Role::ComboBox
            | Role::Switch
            | Role::MenuItem
            | Role::MenuItemCheckBox
            | Role::MenuItemRadio
            | Role::Tab
            | Role::Option
    )
}

/// The landmark roles a human uses to say *which* "Posts" link they meant.
pub fn is_landmark(role: &Role) -> bool {
    matches!(
        role,
        Role::Banner
            | Role::Navigation
            | Role::Main
            | Role::ContentInfo
            | Role::Complementary
            | Role::Search
            | Role::Form
            | Role::Region
    )
}

/// Every actionable, named node, paired with the role of its nearest enclosing landmark.
///
/// ⭐⭐⭐ **`(role, name)` IS NOT A SUFFICIENT ADDRESS FOR THE REAL WEB, AND THE LANDMARK IS THE
/// MISSING TERM.** On a six-site corpus 521 of 526 non-drivable targets are ambiguous, and the
/// ambiguity is overwhelmingly the same handful of links appearing in **both the header nav and the
/// footer** — "Posts", "About", "GitHub", "Sitemap". Chrome's tree contains those twins too, so this
/// is not a defect in our projection: it is an addressing scheme that cannot say what a human says
/// without thinking about it, *the `Posts` link in the navigation*.
pub fn targets_scoped(root: &A11yNode) -> Vec<(&A11yNode, &'static str)> {
    let mut out = Vec::new();
    fn go<'a>(n: &'a A11yNode, scope: &'static str, out: &mut Vec<(&'a A11yNode, &'static str)>) {
        let scope = if is_landmark(&n.role) {
            n.role.as_str()
        } else {
            scope
        };
        if is_actionable(&n.role) && !n.name.trim().is_empty() {
            out.push((n, scope));
        }
        for c in &n.children {
            go(c, scope, out);
        }
    }
    go(root, "", &mut out);
    out
}

/// Every actionable, named node, paired with its enclosing landmark **and** the nearest heading
/// that precedes it in document order.
///
/// ⭐⭐ **THE TERM AFTER THE LANDMARK, PRICED BEFORE IT IS BUILT** — the same discipline that
/// produced the landmark itself (t1459 measured `+3.3` points before `resolve_target_in` existed).
/// The landmark separates the header nav from the footer; it cannot separate two `Edit` links inside
/// one `main`, and on Wikipedia that is most of what is left. The two candidates a human actually
/// uses are the **section heading** ("the Edit link under *History*") and an **ordinal** ("the third
/// one"). This function supplies the first; [`tally`] prices both.
///
/// ⚠ A heading is a *preceding sibling in document order*, not an ancestor — `<h2>` and the content
/// it introduces are siblings in HTML, so an ancestor walk finds nothing. That is why this is a flat
/// pre-order scan carrying the last heading seen rather than a scoped walk like the landmark's.
pub fn targets_sectioned(root: &A11yNode) -> Vec<(&A11yNode, &'static str, String)> {
    let mut out = Vec::new();
    let mut heading = String::new();
    fn go<'a>(
        n: &'a A11yNode,
        scope: &'static str,
        heading: &mut String,
        out: &mut Vec<(&'a A11yNode, &'static str, String)>,
    ) {
        let scope = if is_landmark(&n.role) {
            n.role.as_str()
        } else {
            scope
        };
        if matches!(n.role, Role::Heading { .. }) && !n.name.trim().is_empty() {
            *heading = n.name.trim().to_string();
        }
        if is_actionable(&n.role) && !n.name.trim().is_empty() {
            out.push((n, scope, heading.clone()));
        }
        for c in &n.children {
            go(c, scope, heading, out);
        }
    }
    go(root, "", &mut heading, &mut out);
    out
}

/// Every actionable, named node in the tree, in document order.
pub fn targets(root: &A11yNode) -> Vec<&A11yNode> {
    let mut out = Vec::new();
    fn go<'a>(n: &'a A11yNode, out: &mut Vec<&'a A11yNode>) {
        if is_actionable(&n.role) && !n.name.trim().is_empty() {
            out.push(n);
        }
        for c in &n.children {
            go(c, out);
        }
    }
    go(root, &mut out);
    out
}

/// Whether `hit` is `target` or anything beneath it — see the note above on descendants.
pub fn in_subtree(target: &A11yNode, hit: manuk_dom::NodeId) -> bool {
    if target.node == hit {
        return true;
    }
    target.children.iter().any(|c| in_subtree(c, hit))
}

/// Classify one target against the whole tree.
///
/// `duplicates` is how many targets share this one's `(role, name)`, itself included — so `1` means
/// unique. It is passed in rather than recomputed because the caller already has the whole bag.
pub fn classify(root: &A11yNode, target: &A11yNode, duplicates: usize) -> Verdict {
    let Some(b) = target.bbox else {
        return Verdict::Ungrounded;
    };
    if b.width <= 0.0 || b.height <= 0.0 {
        return Verdict::Ungrounded;
    }
    if duplicates > 1 {
        return Verdict::Ambiguous;
    }
    let (cx, cy) = (b.x + b.width / 2.0, b.y + b.height / 2.0);
    match root.hit_test(cx, cy) {
        Some(hit) if in_subtree(target, hit.node) => Verdict::Drivable,
        _ => Verdict::MisHit,
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Tally {
    pub total: usize,
    pub drivable: usize,
    pub ungrounded: usize,
    pub ambiguous: usize,
    pub mishit: usize,
    /// How many would be drivable if the address included the enclosing landmark — the size of the
    /// fix, measured before it is built.
    pub drivable_scoped: usize,
    /// …and if it also included the nearest preceding heading.
    pub drivable_sectioned: usize,
    /// …and if an ORDINAL were allowed instead ("the third `Edit`"), which by construction makes
    /// every target addressable — so this is the CEILING, not a proposal. It is here to say how much
    /// of the gap is inherent ambiguity rather than a missing term.
    pub drivable_ordinal: usize,
}

impl Tally {
    /// The drivable rate once the address includes the landmark AND the nearest heading.
    pub fn sectioned_rate(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.drivable_sectioned as f64 / self.total as f64
        }
    }

    /// The ceiling: every target addressable by ordinal, so only grounding and occlusion can fail.
    pub fn ordinal_rate(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.drivable_ordinal as f64 / self.total as f64
        }
    }

    /// The drivable rate once the address includes the enclosing landmark.
    pub fn scoped_rate(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.drivable_scoped as f64 / self.total as f64
        }
    }

    pub fn rate(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.drivable as f64 / self.total as f64
        }
    }
}

/// Classify every actionable target in a tree.
pub fn tally(root: &A11yNode) -> Tally {
    let ts = targets(root);
    let mut counts = std::collections::HashMap::<(String, String), usize>::new();
    for t in &ts {
        *counts
            .entry((t.role.as_str().to_string(), t.name.trim().to_string()))
            .or_insert(0) += 1;
    }
    let mut out = Tally {
        total: ts.len(),
        ..Default::default()
    };
    for t in &ts {
        let d = counts[&(t.role.as_str().to_string(), t.name.trim().to_string())];
        match classify(root, t, d) {
            Verdict::Drivable => out.drivable += 1,
            Verdict::Ungrounded => out.ungrounded += 1,
            Verdict::Ambiguous => out.ambiguous += 1,
            Verdict::MisHit => out.mishit += 1,
        }
    }

    // The same classification with the enclosing landmark in the key.
    let scoped = targets_scoped(root);
    let mut scounts = std::collections::HashMap::<(&str, String, String), usize>::new();
    for (t, s) in &scoped {
        *scounts
            .entry((s, t.role.as_str().to_string(), t.name.trim().to_string()))
            .or_insert(0) += 1;
    }
    for (t, s) in &scoped {
        let d = scounts[&(*s, t.role.as_str().to_string(), t.name.trim().to_string())];
        if classify(root, t, d) == Verdict::Drivable {
            out.drivable_scoped += 1;
        }
    }

    // …and with the nearest preceding heading as a third term.
    let sectioned = targets_sectioned(root);
    let mut hcounts = std::collections::HashMap::<(&str, &str, String, String), usize>::new();
    for (t, sc, h) in &sectioned {
        *hcounts
            .entry((
                sc,
                h.as_str(),
                t.role.as_str().to_string(),
                t.name.trim().to_string(),
            ))
            .or_insert(0) += 1;
    }
    for (t, sc, h) in &sectioned {
        let d = hcounts[&(
            *sc,
            h.as_str(),
            t.role.as_str().to_string(),
            t.name.trim().to_string(),
        )];
        if classify(root, t, d) == Verdict::Drivable {
            out.drivable_sectioned += 1;
        }
    }

    // The ceiling: an ordinal makes every duplicate unique by construction, so only Ungrounded and
    // MisHit can still fail. The distance between this and `sectioned` is inherent ambiguity that no
    // further NAMING term can remove.
    for t in &ts {
        if classify(root, t, 1) == Verdict::Drivable {
            out.drivable_ordinal += 1;
        }
    }
    out
}
