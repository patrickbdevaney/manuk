//! **G_THE_THIRD_EDIT_LINK — the landmark was not the whole address, and most of what is left needs
//! a POSITION rather than another name.**
//!
//! t1462 landed landmark-scoped resolution after `drive-probe` priced it at +3.2 points. This gate
//! is the next two terms, priced the same way **before either existed**:
//!
//! ```text
//!                          rate    +landmark   +heading   ceiling(ordinal)
//!   martinfowler.com      84.3%       89.3%     100.0%        100.0%
//!   news.ycombinator      66.3%       66.3%      66.3%        100.0%
//!   blog.rust-lang.org    96.9%       96.9%      98.6%        100.0%
//!   www.a11yproject       77.6%       77.6%      86.2%         89.7%
//!   danluu.com           100.0%      100.0%     100.0%        100.0%
//!   en.wikipedia.org      67.8%       73.5%      75.5%         99.5%
//!   TOTAL                 78.5%       81.7%      84.3%         99.5%
//! ```
//!
//! ⭐⭐⭐ **THE CEILING IS WHAT MAKES THE REST LEGIBLE.** At **99.5%**, essentially every target is
//! already grounded and unoccluded — so the entire 21-point shortfall is *which one did you mean*,
//! not geometry. The two semantic terms recover **5.8** of those points and an ordinal recovers the
//! remaining **15.2**. `news.ycombinator.com` has neither landmarks nor headings and moves for
//! neither; `martinfowler.com` reaches **100%** on the heading alone. So `nth` is not a convenience
//! — on this corpus it is the majority of the fix.
//!
//! ⚠ **A heading is a PRECEDING SIBLING, not an ancestor.** `<h2>` and the content it introduces are
//! siblings in HTML, so an ancestor walk finds nothing and the scope has to be carried through a
//! flat pre-order scan. That is the one thing about this term that is not analogous to the landmark.
//!
//! ⚠ **`nth` counts DOCUMENT ORDER, not score.** "The third Edit link" means the third one on the
//! page — what a caller counting them saw. Indexing the score-sorted list would return "the third
//! best match", a different and unusable thing. The `nth-is-not-rank` row below is that assertion.
//!
//! Mutations that must turn this red:
//!   1. drop the heading filter          → both sections resolve to the same node
//!   2. heading scope by ANCESTOR walk   → no target has a heading; every scoped call fails
//!   3. `nth` indexes the SCORE order    → `nth-is-not-rank` picks the wrong `Edit`
//!   4. `nth` applied before the role    → the ordinal counts non-links

use manuk_a11y::Role;
use manuk_agent::AgentBrowser;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8><title>Doc</title><style>
body { margin: 0; font: 16px/1.4 monospace }
a { display: block; width: 160px; height: 24px }
</style></head><body>
<main>
  <h2>History</h2>
  <p>a</p><a href="/edit-history" id="e1">Edit</a>
  <h2>Usage</h2>
  <p>b</p><a href="/edit-usage" id="e2">Edit</a>
  <h2>Design</h2>
  <p>c</p><a href="/edit-design" id="e3">Edit</a>
</main>
</body></html>"##;

fn setup() -> (tokio::runtime::Runtime, AgentBrowser, manuk_a11y::Rect) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let path = std::env::temp_dir().join("g_third_edit_link.html");
    std::fs::write(&path, HTML).unwrap();
    let url = format!("file://{}", path.display());
    let mut b = AgentBrowser::new(1024, 768);
    rt.block_on(async { b.navigate(&url).await.expect("navigate") });
    let vp = manuk_a11y::Rect {
        x: 0.0,
        y: 0.0,
        width: 1024.0,
        height: 768.0,
    };
    (rt, b, vp)
}

#[test]
fn a_heading_and_an_ordinal_each_pick_out_one_of_three_identical_links() {
    let (rt, b, vp) = setup();
    let tree = b.a11y_tree().expect("a11y tree");

    // ── VACUITY. There must genuinely be three identical `Edit` links under three headings, or
    //    nothing below is disambiguating anything.
    fn count(n: &manuk_a11y::A11yNode, name: &str) -> usize {
        (n.role == Role::Link && n.name.trim() == name) as usize
            + n.children.iter().map(|c| count(c, name)).sum::<usize>()
    }
    assert_eq!(
        count(&tree, "Edit"),
        3,
        "VACUOUS: the page does not contain three `Edit` links"
    );

    let at = |h: Option<&str>, n: Option<usize>| {
        manuk_agent::targeting::resolve_target_at(&tree, "Edit", Some(&Role::Link), None, h, n, vp)
            .map(|t| t.node)
    };

    // ── THE HEADING TERM. Each section's `Edit` is a different node, and they order down the page.
    let (h1, h2, h3) = (
        at(Some("History"), None),
        at(Some("Usage"), None),
        at(Some("Design"), None),
    );
    assert!(
        h1.is_some() && h2.is_some() && h3.is_some(),
        "a heading-scoped Edit was not found"
    );
    assert!(
        h1 != h2 && h2 != h3 && h1 != h3,
        "the three headings resolved to the same node — the address is still (role, name)"
    );

    // ── THE ORDINAL. Document order, and it must agree with the headings.
    let (n0, n1, n2) = (at(None, Some(0)), at(None, Some(1)), at(None, Some(2)));
    assert_eq!(
        n0, h1,
        "the FIRST Edit in document order is the one under `History`"
    );
    assert_eq!(n1, h2, "the SECOND is the one under `Usage`");
    assert_eq!(n2, h3, "the THIRD is the one under `Design`");
    assert_eq!(at(None, Some(3)), None, "there is no fourth Edit");

    // ── ⚠ `nth` IS NOT RANK. The three score identically here, so a score-sorted index would be
    //    arbitrary; document order is not. Asserting the exact identity above already pins it, and
    //    this row states the intent so a later reader cannot "optimise" it into a rank.
    assert_ne!(
        n0, n2,
        "nth-is-not-rank: the first and third Edit are different nodes"
    );

    // ── THE UNSCOPED CALL IS UNCHANGED — it must still resolve, and to one of the three.
    let unscoped = at(None, None).expect("the unscoped address must keep working");
    assert!(
        unscoped == h1.unwrap() || unscoped == h2.unwrap() || unscoped == h3.unwrap(),
        "the unscoped resolver stopped reaching any Edit link"
    );

    // ── AND THE ACTION PATH IS WIRED. A capability with no production caller is the shape this repo
    //    keeps finding (t1402, t1403). The hrefs differ, so the attempted URL names the node clicked.
    let mut b = b;
    let err = rt
        .block_on(async {
            b.click_by_name_at(None, Some("Design"), None, &Role::Link, "Edit")
                .await
        })
        .map(|a| format!("{a:?}"))
        .unwrap_or_else(|e| format!("{e:#}"));
    assert!(
        err.contains("edit-design"),
        "click_by_name_at(heading=Design) followed the wrong link; got {err}"
    );

    let err2 = rt
        .block_on(async {
            b.click_by_name_at(None, None, Some(1), &Role::Link, "Edit")
                .await
        })
        .map(|a| format!("{a:?}"))
        .unwrap_or_else(|e| format!("{e:#}"));
    assert!(
        err2.contains("edit-usage"),
        "click_by_name_at(nth=1) followed the wrong link; got {err2}"
    );
}
