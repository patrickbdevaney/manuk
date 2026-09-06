//! **G_AN_ORDINAL_NEEDS_AN_ENUMERATION — `nth` was measured as 15.2 of the 21 remaining drive
//! points, and nothing told a caller how many there were.**
//!
//! t1471 priced the agent's addressing gap and found the whole of it is naming, not geometry:
//!
//! ```text
//!                    rate    +landmark   +heading   ceiling(ordinal)
//!   TOTAL           78.5%       81.7%      84.3%        99.5%
//! ```
//!
//! At a 99.5% ceiling essentially every target is already grounded and unoccluded, so the shortfall
//! is entirely *which one did you mean* — and an ordinal recovers **15.2** of the 21 points against
//! the two semantic terms' 5.8. ⭐⭐⭐ **But an agent can only ask for *the third `Edit`* if something
//! told it there are three.** Without an enumeration an ambiguous resolve is a dead end: the caller
//! is handed one arbitrary winner and cannot see the set it came from.
//!
//! ⚠⚠ **THE PUBLISHED ORDER MUST BE THE ORDER `nth` INDEXES.** Both sort by node id — document
//! order — so `candidates(..)[i].node == resolve_target_at(.., nth = Some(i), ..)` by construction.
//! Publishing one order and indexing another is the t1402 shape: two halves of one system that
//! disagree about the thing they share, each with tests that pass. **The round-trip row below is the
//! whole point of this gate** — the enumeration and the ordinal are only useful together.
//!
//! Mutations that must turn this red:
//!   1. `candidates` sorts by SCORE                → the round-trip row mismatches
//!   2. `candidates` skips the role filter         → the `<button>Edit</button>` enters the list
//!
//! ⚠ **THAT BUTTON EXISTS BECAUSE MUTATION 2 CAME BACK GREEN.** With only links named `Edit` on the
//! page nothing else scored above zero, so dropping the role filter changed no answer and the filter
//! was untested. A same-named element of a DIFFERENT role is the only thing that can see it.
//!   3. `candidates` drops the `score > 0` filter  → unrelated nodes enter the list
//!   4. `landmark`/`heading` not populated         → the rows carry nothing to choose between

use manuk_a11y::Role;
use manuk_agent::AgentBrowser;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8><title>Doc</title><style>
body { margin: 0; font: 16px/1.4 monospace }
a { display: block; width: 160px; height: 24px }
</style></head><body>
<nav aria-label="Main"><a href="/edit-nav" id="e0">Edit</a></nav>
<main>
  <h2>History</h2><p>a</p><a href="/edit-history" id="e1">Edit</a>
  <h2>Usage</h2><p>b</p><a href="/edit-usage" id="e2">Edit</a>
  <button id="eb">Edit</button>
</main>
</body></html>"##;

#[test]
fn the_enumeration_and_the_ordinal_agree_by_construction() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let path = std::env::temp_dir().join("g_ordinal_enumeration.html");
    std::fs::write(&path, HTML).unwrap();
    let url = format!("file://{}", path.display());
    let mut b = AgentBrowser::new(1024, 768);
    rt.block_on(async { b.navigate(&url).await.expect("navigate") });

    let cands = b.candidates_for(&Role::Link, "Edit").expect("candidates");
    println!("CANDIDATES: {cands:?}");

    // ── VACUITY. There must be three `Edit` links, or an enumeration proves nothing.
    assert_eq!(
        cands.len(),
        3,
        "VACUOUS: expected three Edit LINKS (the page also holds a <button>Edit</button> that the \
         role filter must exclude), got {}",
        cands.len()
    );
    assert!(
        cands.iter().all(|c| c.role == Role::Link),
        "a non-link reached the candidate list — the role filter is not being applied"
    );

    // ── EVERY ROW CARRIES SOMETHING TO CHOOSE BETWEEN. An enumeration of three identical rows is
    //    not an enumeration — it is the same dead end with a length.
    assert_eq!(cands[0].landmark.as_deref(), Some("navigation"));
    assert_eq!(cands[1].heading.as_deref(), Some("History"));
    assert_eq!(cands[2].heading.as_deref(), Some("Usage"));
    assert!(
        cands.iter().all(|c| c.point.is_some()),
        "every candidate must publish a click point"
    );
    assert_eq!(
        cands.iter().map(|c| c.nth).collect::<Vec<_>>(),
        vec![0, 1, 2],
        "`nth` must be the row's own index"
    );

    // ── ⭐ THE ROUND TRIP, AND IT IS THE WHOLE POINT. What the caller counted is what `nth` picks.
    let tree = b.a11y_tree().unwrap();
    let vp = manuk_a11y::Rect {
        x: 0.0,
        y: 0.0,
        width: 1024.0,
        height: 768.0,
    };
    for c in &cands {
        let picked = manuk_agent::targeting::resolve_target_at(
            &tree,
            "Edit",
            Some(&Role::Link),
            None,
            None,
            Some(c.nth),
            vp,
        )
        .unwrap_or_else(|| panic!("nth={} resolved to nothing", c.nth));
        assert_eq!(
            picked.node, c.node,
            "candidates[{}] and nth={} name DIFFERENT nodes — the enumeration and the ordinal have \
             drifted apart, which makes both useless",
            c.nth, c.nth
        );
    }

    // ── AND THE TERMS THE ROWS PUBLISH ACTUALLY WORK as addresses.
    let by_heading = manuk_agent::targeting::resolve_target_at(
        &tree,
        "Edit",
        Some(&Role::Link),
        None,
        Some("Usage"),
        None,
        vp,
    )
    .expect("a heading-scoped Edit");
    assert_eq!(
        by_heading.node, cands[2].node,
        "the `heading` a candidate published did not select that candidate"
    );
}
