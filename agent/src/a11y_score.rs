//! Scoring Manuk's accessibility tree against Chrome's — **precision, recall and F1**.
//!
//! ## Why this module is not inside the `a11y-score` binary
//!
//! ⭐⭐⭐ **THE TRACK B BAR RESTS ENTIRELY ON THIS ARITHMETIC, AND IT HAD NEVER BEEN CHECKED.**
//! Every number the loop has quoted for *">=90% node match"* — 63.8%, 75.0%, 97.0% — came out of a
//! throwaway script under `/tmp` that no longer exists. A gate whose value cannot be recomputed is
//! a memory of a gate, and a scorer whose own sums are untested is exactly the instrument this loop
//! keeps catching: [`crate::a11y_score`] is a library so a test can hand it two bags it computed by
//! hand and check the answer without launching a browser.
//!
//! ## Recall is not "node match"
//!
//! A multiset match taken over **the oracle's** nodes answers only *how many of Chrome's nodes did
//! we produce*. It cannot see nodes we invent. The one time both halves were computed by hand
//! (martinfowler.com) they were **97.3% recall and 67.7% precision** — nearly a third of what that
//! tree offered an agent was not in Chrome's tree at all. A screen reader reads those; an agent
//! clicks them. Recall also *improves* as the projection gets noisier, which is the wrong direction
//! for a bar to move under a mistake.
//!
//! [`f1`] is the number to steer on.

use std::collections::HashMap;

/// One node, as the comparison sees it: its role token and its accessible name.
pub type Key = (String, String);

/// Roles carrying neither a role nor a name worth matching on.
///
/// Dropped from **both** sides, each for a stated reason:
///
/// | dropped | why |
/// |---|---|
/// | `generic` / `none` / `presentation` | no role and no name; a pure wrapper-count difference |
/// | `statictext` / `inlinetextbox` | Chrome's text leaves — Manuk folds text into its parent's name, so counting them measures the modelling difference and nothing else |
///
/// ⚠ Every one of these makes the score **kinder**, which is why the binary reports how many it
/// dropped on each side. A drop that flatters has to stay visible or it stops being a modelling
/// decision and becomes a thumb on the scale.
pub fn is_structural(role: &str) -> bool {
    matches!(
        role,
        "generic" | "none" | "presentation" | "statictext" | "inlinetextbox" | "ignored" | ""
    )
}

/// Chrome's AX role vocabulary is CamelCase and spells a handful of roles differently from ARIA.
pub fn normalize_chrome_role(r: &str) -> String {
    let lower = r.to_ascii_lowercase();
    match lower.as_str() {
        "rootwebarea" => "document".into(),
        "listmarker" | "genericcontainer" | "abbr" => "generic".into(),
        "textfield" => "textbox".into(),
        "radiobutton" => "radio".into(),
        "progressindicator" => "progressbar".into(),
        "splitter" => "separator".into(),
        "descriptionlist" => "list".into(),
        "descriptionlistterm" => "term".into(),
        "descriptionlistdetail" => "definition".into(),
        _ => lower,
    }
}

/// Accessible names differ between engines in their whitespace far more often than in their words,
/// and no consumer can act on that difference.
pub fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The size of the multiset intersection of two `(role, name)` bags.
///
/// ⚠ A multiset, not a set: a page with nine `link "Home"` entries and one with two are not the
/// same tree, and a set intersection would call them identical.
pub fn multiset_overlap(a: &[Key], b: &[Key]) -> usize {
    let mut counts: HashMap<&Key, i64> = HashMap::new();
    for k in a {
        *counts.entry(k).or_insert(0) += 1;
    }
    let mut hit = 0usize;
    for k in b {
        if let Some(c) = counts.get_mut(k) {
            if *c > 0 {
                *c -= 1;
                hit += 1;
            }
        }
    }
    hit
}

/// Precision, recall and F1 for `ours` against the oracle `theirs`.
///
/// `precision` = of the nodes we published, how many Chrome also has (**phantoms**);
/// `recall` = of the nodes Chrome publishes, how many we produced (**omissions**).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Score {
    pub ours: usize,
    pub theirs: usize,
    pub matched: usize,
    pub precision: f64,
    pub recall: f64,
    pub f1: f64,
}

/// The harmonic mean, and `0.0` when either half is zero rather than a `NaN` that formats as a
/// plausible-looking dash.
pub fn f1(precision: f64, recall: f64) -> f64 {
    if precision + recall == 0.0 {
        0.0
    } else {
        2.0 * precision * recall / (precision + recall)
    }
}

pub fn score(ours: &[Key], theirs: &[Key]) -> Score {
    let matched = multiset_overlap(ours, theirs);
    let precision = if ours.is_empty() {
        0.0
    } else {
        matched as f64 / ours.len() as f64
    };
    let recall = if theirs.is_empty() {
        0.0
    } else {
        matched as f64 / theirs.len() as f64
    };
    Score {
        ours: ours.len(),
        theirs: theirs.len(),
        matched,
        precision,
        recall,
        f1: f1(precision, recall),
    }
}

/// Flatten a Manuk a11y tree into the comparison's bag, counting what was dropped.
pub fn manuk_bag(root: &manuk_a11y::A11yNode) -> (Vec<Key>, usize) {
    let (mut out, mut dropped) = (Vec::new(), 0usize);
    walk(root, &mut out, &mut dropped);
    (out, dropped)
}

fn walk(n: &manuk_a11y::A11yNode, out: &mut Vec<Key>, dropped: &mut usize) {
    let role = n.role.as_str().to_ascii_lowercase();
    if is_structural(&role) {
        *dropped += 1;
    } else {
        out.push((role, collapse_ws(&n.name)));
    }
    for c in &n.children {
        walk(c, out, dropped);
    }
}

/// The multiset DIFFERENCE `ours - theirs`, as `(count, role, name)` ranked by count.
///
/// ⭐ Precision is the binding half of the Track B bar (63.5% pooled against 96.4% recall), and a
/// percentage cannot say what to fix. This is what turns it into work: the phantoms, named and
/// ranked, so a tick can start from *what those nodes are* rather than from a guess that they are
/// wrappers.
pub fn excess(ours: &[Key], theirs: &[Key]) -> Vec<(usize, String, String)> {
    let mut have: HashMap<&Key, i64> = HashMap::new();
    for k in theirs {
        *have.entry(k).or_insert(0) += 1;
    }
    let mut extra: HashMap<&Key, usize> = HashMap::new();
    for k in ours {
        match have.get_mut(k) {
            Some(c) if *c > 0 => *c -= 1,
            _ => *extra.entry(k).or_insert(0) += 1,
        }
    }
    let mut v: Vec<(usize, String, String)> = extra
        .into_iter()
        .map(|(k, n)| (n, k.0.clone(), k.1.clone()))
        .collect();
    v.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    v
}
