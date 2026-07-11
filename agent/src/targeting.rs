//! Agent-native targeting (L17): turn a big accessibility tree + a natural-language task into
//! (a) a **pruned** tree of only the task-relevant, actionable nodes (AG2), and (b) a resolved
//! **click target** chosen by combining semantic and visual signals (AG3).
//!
//! Both are pure functions over [`manuk_a11y::A11yNode`] — no I/O, no model — so they are cheap,
//! deterministic, and unit-testable. They are the agent-native differentiator: an agent acts
//! against a small, intent-focused view of the page and picks targets robustly even when several
//! elements share a label.

use manuk_a11y::{A11yNode, Rect, Role};
use manuk_dom::NodeId;

/// Common filler words that carry no targeting signal — dropped from task/intent keywords so
/// "click the Sign in button" reduces to `["sign", "in"]`.
const STOP_WORDS: &[&str] = &[
    "the", "a", "an", "to", "of", "and", "or", "in", "on", "at", "for", "with", "click",
    "tap", "press", "button", "link", "please", "go", "then", "this", "that", "my", "your",
    "it", "is", "be", "into", "onto", "from",
];

/// Lowercase alphanumeric keywords (length ≥ 2, minus [`STOP_WORDS`]) from a task/intent string.
pub fn keywords(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 2)
        .map(|w| w.to_ascii_lowercase())
        .filter(|w| !STOP_WORDS.contains(&w.as_str()))
        .collect()
}

/// Total node count of a tree (root included) — for measuring how much [`prune_for_task`] cut.
pub fn node_count(tree: &A11yNode) -> usize {
    1 + tree.children.iter().map(node_count).sum::<usize>()
}

/// Whether `name` contains any of the `kw` keywords (case-insensitive substring).
fn name_matches(name: &str, kw: &[String]) -> bool {
    if kw.is_empty() {
        return false;
    }
    let lname = name.to_ascii_lowercase();
    kw.iter().any(|k| lname.contains(k.as_str()))
}

/// A node is *relevant* to the task if it is interactive (actionable) or its accessible name
/// matches the task keywords.
fn node_relevant(node: &A11yNode, kw: &[String]) -> bool {
    node.role.is_interactive() || name_matches(&node.name, kw)
}

/// **AG2 — task-intent AXTree pruning.** Return a copy of `tree` keeping only nodes that are
/// relevant to `task` (interactive, or name-matching) plus the ancestor chain needed to reach
/// them; subtrees with nothing relevant are dropped. The result is a strictly smaller, focused
/// view an agent can reason over cheaply. `None` if nothing in the tree is relevant.
pub fn prune_for_task(tree: &A11yNode, task: &str) -> Option<A11yNode> {
    let kw = keywords(task);
    prune_node(tree, &kw)
}

fn prune_node(node: &A11yNode, kw: &[String]) -> Option<A11yNode> {
    let pruned_children: Vec<A11yNode> =
        node.children.iter().filter_map(|c| prune_node(c, kw)).collect();
    // Keep this node if it is itself relevant, or it is an ancestor of something kept.
    if node_relevant(node, kw) || !pruned_children.is_empty() {
        Some(A11yNode {
            node: node.node,
            role: node.role.clone(),
            name: node.name.clone(),
            bbox: node.bbox,
            z: node.z,
            children: pruned_children,
        })
    } else {
        None
    }
}

/// A resolved click target from [`resolve_target`].
#[derive(Clone, Debug, PartialEq)]
pub struct Targeted {
    /// Arena node to act on.
    pub node: NodeId,
    pub role: Role,
    pub name: String,
    /// Where to click — the target's box center.
    pub point: (f32, f32),
    /// Combined semantic+visual score of the winner (higher = better).
    pub score: f32,
    /// Margin of the winner over the runner-up, `0.0..=1.0`. Low = ambiguous (several equally
    /// good targets); a caller can refuse to act, or ask, below a threshold.
    pub confidence: f32,
}

// AG3 scoring weights. Semantic dominates (the label is the primary signal); visual salience
// breaks ties between same-labeled elements. Kept as named consts so the policy is legible.
const SEMANTIC_W: f32 = 0.72;
const VISUAL_W: f32 = 0.28;
/// Small nudge toward buttons when the intent reads like an action and toward the exact label.
const EXACT_NAME_BONUS: f32 = 0.25;
const ACTION_ROLE_BONUS: f32 = 0.08;

/// Semantic match of a candidate `name`/`role` against the intent keywords: the fraction of
/// intent keywords present in the name, plus a bonus for an exact-label match and a small nudge
/// for actionable roles. `0.0..≈1.33`.
fn semantic_score(name: &str, role: &Role, kw: &[String]) -> f32 {
    if kw.is_empty() {
        return 0.0;
    }
    let lname = name.to_ascii_lowercase();
    let hits = kw.iter().filter(|k| lname.contains(k.as_str())).count();
    let mut s = hits as f32 / kw.len() as f32;
    // Exact match (all keywords are exactly the name's tokens) is a strong signal.
    let name_tokens: Vec<&str> = lname.split(|c: char| !c.is_alphanumeric()).filter(|t| !t.is_empty()).collect();
    if !name_tokens.is_empty() && kw.iter().all(|k| name_tokens.contains(&k.as_str())) && name_tokens.len() == kw.len() {
        s += EXACT_NAME_BONUS;
    }
    if matches!(role, Role::Button) {
        s += ACTION_ROLE_BONUS;
    }
    s
}

/// Visual salience of `bbox` within `viewport`: in-view, larger, and more central boxes score
/// higher; fully-offscreen boxes score ~0. `0.0..=1.0`.
fn visual_score(bbox: &Rect, viewport: &Rect) -> f32 {
    if !bbox.intersects(viewport) {
        return 0.0;
    }
    let vp_area = (viewport.width * viewport.height).max(1.0);
    // Area share (capped): bigger targets are easier/more prominent.
    let area = (bbox.width * bbox.height).max(0.0);
    let area_score = (area / vp_area).sqrt().min(1.0); // sqrt so tiny buttons aren't ~0
    // Centrality: closer to viewport center = higher.
    let (cx, cy) = bbox.center();
    let (vcx, vcy) = viewport.center();
    let dx = (cx - vcx).abs() / (viewport.width / 2.0).max(1.0);
    let dy = (cy - vcy).abs() / (viewport.height / 2.0).max(1.0);
    let dist = ((dx * dx + dy * dy) / 2.0).sqrt().min(1.0);
    let central = 1.0 - dist;
    0.55 * area_score + 0.45 * central
}

/// **AG3 — dual (semantic + visual) targeting.** Pick the node in `tree` that best satisfies
/// `intent`, combining a semantic score (role + name match) with a visual score (in-viewport,
/// large, central). Only nodes with a box are candidates (an agent can't click a boxless node).
/// Returns the winner + its click point + a confidence margin over the runner-up. `None` if there
/// is no viable candidate (no boxed node scoring > 0).
pub fn resolve_target(tree: &A11yNode, intent: &str, viewport: Rect) -> Option<Targeted> {
    let kw = keywords(intent);
    let mut scored: Vec<(f32, &A11yNode)> = Vec::new();
    collect_scored(tree, &kw, &viewport, &mut scored);
    if scored.is_empty() {
        return None;
    }
    // Highest score first; stable so earlier reading-order wins exact ties.
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let (best_score, best) = scored[0];
    if best_score <= 0.0 {
        return None;
    }
    let runner = scored.get(1).map(|(s, _)| *s).unwrap_or(0.0);
    let confidence = if best_score > 0.0 {
        ((best_score - runner) / best_score).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let bbox = best.bbox?;
    Some(Targeted {
        node: best.node,
        role: best.role.clone(),
        name: best.name.clone(),
        point: bbox.center(),
        score: best_score,
        confidence,
    })
}

fn collect_scored<'a>(
    node: &'a A11yNode,
    kw: &[String],
    viewport: &Rect,
    out: &mut Vec<(f32, &'a A11yNode)>,
) {
    if let Some(bbox) = node.bbox {
        // Candidates are actionable nodes, or non-interactive nodes whose name matches the
        // intent (a heading/label the agent might click). A non-matching, non-interactive node
        // is never a target, however visually prominent.
        let is_candidate = node.role.is_interactive() || name_matches(&node.name, kw);
        if is_candidate {
            let sem = semantic_score(&node.name, &node.role, kw);
            let vis = visual_score(&bbox, viewport);
            let score = SEMANTIC_W * sem + VISUAL_W * vis;
            if score > 0.0 {
                out.push((score, node));
            }
        }
    }
    for c in &node.children {
        collect_scored(c, kw, viewport, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(id: u64, role: Role, name: &str, bbox: Option<Rect>, children: Vec<A11yNode>) -> A11yNode {
        A11yNode { node: NodeId(id as usize), role, name: name.to_string(), bbox, z: 0, children }
    }
    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, width: w, height: h }
    }

    fn sample_page() -> A11yNode {
        // A page: a nav with a "Sign in" button + a decorative logo image, a main with a heading
        // and a paragraph, and a footer with a same-text "Sign in" link far down the page.
        n(1, Role::Document, "", Some(rect(0.0, 0.0, 1000.0, 3000.0)), vec![
            n(2, Role::Navigation, "", Some(rect(0.0, 0.0, 1000.0, 60.0)), vec![
                n(3, Role::Image, "logo", Some(rect(10.0, 10.0, 40.0, 40.0)), vec![]),
                n(4, Role::Button, "Sign in", Some(rect(820.0, 12.0, 140.0, 36.0)), vec![]),
                n(5, Role::Link, "Products", Some(rect(200.0, 20.0, 90.0, 20.0)), vec![]),
            ]),
            n(6, Role::Main, "", Some(rect(0.0, 60.0, 1000.0, 2000.0)), vec![
                n(7, Role::Heading { level: 1 }, "Welcome", Some(rect(40.0, 100.0, 400.0, 50.0)), vec![]),
                n(8, Role::Paragraph, "Some marketing copy here", Some(rect(40.0, 160.0, 600.0, 80.0)), vec![]),
            ]),
            n(9, Role::ContentInfo, "", Some(rect(0.0, 2900.0, 1000.0, 100.0)), vec![
                n(10, Role::Link, "Sign in", Some(rect(40.0, 2940.0, 60.0, 16.0)), vec![]),
                n(11, Role::Paragraph, "Copyright 2026", Some(rect(400.0, 2940.0, 200.0, 16.0)), vec![]),
            ]),
        ])
    }

    #[test]
    fn prune_keeps_relevant_drops_decoration() {
        let tree = sample_page();
        let full = node_count(&tree);
        let pruned = prune_for_task(&tree, "sign in to my account").expect("something relevant");
        let kept = node_count(&pruned);
        assert!(kept < full, "pruning shrank the tree ({kept} < {full})");

        // The decorative paragraph "Some marketing copy" (non-interactive, non-matching) is gone;
        // the interactive nodes and the name-matching "Sign in" nodes remain (with ancestors).
        let names: Vec<String> = collect_names(&pruned);
        assert!(names.iter().any(|s| s == "Sign in"), "sign-in target kept");
        assert!(names.iter().any(|s| s == "Products"), "interactive link kept even if off-task");
        assert!(!names.iter().any(|s| s == "Some marketing copy here"), "decorative paragraph dropped");
        assert!(!names.iter().any(|s| s == "Copyright 2026"), "footer copyright dropped");
    }

    #[test]
    fn resolve_prefers_the_salient_button_over_the_footer_link() {
        let tree = sample_page();
        // The visible viewport is the top 1000px — both the nav button and (not) the footer link.
        let viewport = rect(0.0, 0.0, 1000.0, 1000.0);
        let t = resolve_target(&tree, "sign in", viewport).expect("a target");
        assert_eq!(t.node, NodeId(4 as usize), "the prominent nav button, not the footer link");
        assert_eq!(t.name, "Sign in");
        assert!(t.point.0 > 800.0 && t.point.1 < 60.0, "click point is the button center");
        assert!(t.confidence > 0.0, "some margin over the runner-up");
    }

    #[test]
    fn ambiguous_targets_report_low_confidence() {
        // Two identical buttons side by side, equally central → near-zero confidence margin.
        let tree = n(1, Role::Document, "", Some(rect(0.0, 0.0, 1000.0, 1000.0)), vec![
            n(2, Role::Button, "Continue", Some(rect(400.0, 480.0, 100.0, 40.0)), vec![]),
            n(3, Role::Button, "Continue", Some(rect(520.0, 480.0, 100.0, 40.0)), vec![]),
        ]);
        let t = resolve_target(&tree, "continue", rect(0.0, 0.0, 1000.0, 1000.0)).expect("a target");
        assert!(t.confidence < 0.15, "two equally-good targets are ambiguous (conf {})", t.confidence);
    }

    #[test]
    fn no_candidate_returns_none() {
        let tree = n(1, Role::Document, "", Some(rect(0.0, 0.0, 100.0, 100.0)), vec![
            n(2, Role::Paragraph, "just text", Some(rect(0.0, 0.0, 50.0, 20.0)), vec![]),
        ]);
        assert!(resolve_target(&tree, "buy now", rect(0.0, 0.0, 100.0, 100.0)).is_none());
    }

    fn collect_names(node: &A11yNode) -> Vec<String> {
        let mut v = vec![node.name.clone()];
        for c in &node.children {
            v.extend(collect_names(c));
        }
        v
    }
}
