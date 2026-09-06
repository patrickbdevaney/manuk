//! **G_A11Y_SCORE_COUNTS_PHANTOMS — the Track B node-match bar was being read off a RECALL-ONLY
//! number, computed by a script that is not in the repository.**
//!
//! ⭐⭐⭐ Every value the loop has quoted for *">=90% node match"* — 63.8%, 75.0%, 97.0%, and the
//! per-site rows in `g_a11y_name_from_content_context` — came out of a throwaway script under
//! `/tmp` that no longer exists. Nothing could recompute them, so nothing could question them; and
//! the half they computed was **recall**, which is blind to nodes the tree invents and *improves*
//! when the projection gets noisier.
//!
//! Measured by `a11y-score` (this tick's binary) against CDP `Accessibility.getFullAXTree`:
//!
//! ```text
//!                                 manuk  chrome  match     prec   recall      F1
//!   martinfowler.com                425     297    290    68.2%    97.6%   80.3%
//!   news.ycombinator.com            479     497    478    99.8%    96.2%   98.0%
//!   blog.rust-lang.org             1678    1673   1672    99.6%    99.9%   99.8%
//!   www.a11yproject.com             173     158    146    84.4%    92.4%   88.2%
//!   danluu.com                      414     416    414   100.0%    99.5%   99.8%
//!   en.wikipedia.org/wiki/…        2629     779    682    25.9%    87.5%   40.0%
//!   TOTAL (pooled)                 5798    3820   3682    63.5%    96.4%   76.6%
//! ```
//!
//! ⭐⭐ **THE `96.4%` IS THE NUMBER THE LOOP HAS BEEN REPORTING, AND IT IS THE HALF THAT FLATTERS.**
//! Precision is `63.5%` and F1 — the one to steer on — is `76.6%` against a `>=90%` bar. Wikipedia
//! publishes **2,629 nodes where Chrome publishes 779**: an agent resolving a target on that page
//! is choosing from a list that is three-quarters phantom.
//!
//! ⚠ The instrument reproduces the only hand-computed data point that existed — martinfowler at
//! **68.2% / 97.6%** against the remembered **67.7% / 97.3%**. That agreement is what licenses the
//! rest of the table; without it this would be a new number rather than a corrected one.
//!
//! ## What this gate checks, and what it deliberately does not
//!
//! It checks the **arithmetic**, on bags computed by hand — because a scorer whose own sums are
//! untested is precisely the class of instrument this loop keeps catching, and because the corpus
//! rows above need a browser and a network and cannot be a gate. The rows are recorded in the doc
//! comment as the measurement they are.
//!
//! Mutations that must turn this red:
//!   1. `multiset_overlap` using a SET intersection      → the dup MIRROR row scores 3, not 2
//!   2. precision and recall swapped                     → `phantoms` row reports 100%/50%
//!   3. `f1` returning the arithmetic mean               → `phantoms` row reports 75.0%, not 66.7%
//!   4. `is_structural` dropping nothing                 → `structural` row keeps `generic`

use manuk_agent::a11y_score::{f1, is_structural, multiset_overlap, normalize_chrome_role, score};

fn bag(rows: &[(&str, &str)]) -> Vec<(String, String)> {
    rows.iter()
        .map(|(r, n)| (r.to_string(), n.to_string()))
        .collect()
}

#[test]
fn a_phantom_node_costs_precision_and_recall_cannot_see_it() {
    // ── THE PHANTOM ROW. We publish two nodes Chrome does not have. Recall is perfect and says
    //    nothing is wrong; precision is half; F1 splits the difference.
    let ours = bag(&[
        ("link", "Home"),
        ("link", "Cart"),
        ("button", "Ghost"),
        ("heading", "Ghost"),
    ]);
    let theirs = bag(&[("link", "Home"), ("link", "Cart")]);
    let s = score(&ours, &theirs);
    assert_eq!(s.matched, 2, "both real links match");
    assert!(
        (s.recall - 1.0).abs() < 1e-9,
        "recall is BLIND to phantoms and must read 100% here — got {:.4}",
        s.recall
    );
    assert!(
        (s.precision - 0.5).abs() < 1e-9,
        "precision must see the two phantoms — got {:.4}",
        s.precision
    );
    assert!(
        (s.f1 - 2.0 / 3.0).abs() < 1e-9,
        "F1 is the HARMONIC mean (0.667), not the arithmetic one (0.75) — got {:.4}",
        s.f1
    );

    // ── THE OMISSION ROW, the mirror: the two error kinds must not be interchangeable.
    let s2 = score(&theirs, &ours);
    assert!(
        (s2.precision - 1.0).abs() < 1e-9 && (s2.recall - 0.5).abs() < 1e-9,
        "swapping the arguments must swap precision and recall — got {:.4}/{:.4}",
        s2.precision,
        s2.recall
    );
}

#[test]
fn a_duplicate_node_is_a_different_tree() {
    // ── ⚠ A MULTISET, NOT A SET. A page with three `link "Home"` and one with two are not the same
    //    tree; a set intersection cannot tell them apart and would score this 3.
    let ours = bag(&[("link", "Home"), ("link", "Home"), ("link", "Home")]);
    let theirs = bag(&[("link", "Home"), ("link", "Home")]);
    assert_eq!(
        multiset_overlap(&ours, &theirs),
        2,
        "three of ours can match at most two of theirs"
    );
    let s = score(&ours, &theirs);
    assert!(
        (s.precision - 2.0 / 3.0).abs() < 1e-9 && (s.recall - 1.0).abs() < 1e-9,
        "the extra duplicate is a phantom — got {:.4}/{:.4}",
        s.precision,
        s.recall
    );

    // ── ⭐⭐⭐ **THE MIRROR, AND IT IS THE ONLY ROW THAT CAN SEE A SET.** `multiset_overlap`
    //    iterates its SECOND argument against counts built from the first, so when the extra
    //    duplicates sit on OUR side both a multiset and a set answer 2 — the row above passed
    //    happily under a `contains_key` mutation. The duplicates have to be on the ORACLE'S side
    //    for the difference to become visible: a set says 3, a multiset says 2.
    //
    //    This was found by the mutation coming back GREEN, not by reading the fixture.
    assert_eq!(
        multiset_overlap(&theirs, &ours),
        2,
        "with the duplicates on the ORACLE side a set intersection would say 3"
    );
    let s2 = score(&theirs, &ours);
    assert!(
        (s2.precision - 1.0).abs() < 1e-9 && (s2.recall - 2.0 / 3.0).abs() < 1e-9,
        "two of ours cannot cover three of theirs — got {:.4}/{:.4}",
        s2.precision,
        s2.recall
    );
}

#[test]
fn the_structural_drops_are_the_stated_ones() {
    // Each of these makes the score kinder, so the list must be exactly what the module documents.
    for r in [
        "generic",
        "none",
        "presentation",
        "statictext",
        "inlinetextbox",
        "",
    ] {
        assert!(is_structural(r), "{r:?} must be dropped from BOTH sides");
    }
    for r in [
        "link", "button", "heading", "listitem", "document", "checkbox",
    ] {
        assert!(!is_structural(r), "{r:?} is a real node and must be scored");
    }
    // Chrome's spellings have to arrive as ARIA tokens or every row of them scores zero.
    assert_eq!(normalize_chrome_role("RootWebArea"), "document");
    assert_eq!(normalize_chrome_role("textField"), "textbox");
    assert_eq!(normalize_chrome_role("RadioButton"), "radio");
    assert_eq!(normalize_chrome_role("Link"), "link");
    assert!(is_structural(&normalize_chrome_role("ListMarker")));
}

#[test]
fn an_empty_side_scores_zero_rather_than_nan() {
    // ⚠ A NaN formats as a plausible-looking value in a table and would read as a missing row
    // rather than a total failure to produce a tree.
    let s = score(&[], &bag(&[("link", "Home")]));
    assert_eq!((s.precision, s.recall, s.f1), (0.0, 0.0, 0.0));
    assert_eq!(f1(0.0, 0.0), 0.0);
}
