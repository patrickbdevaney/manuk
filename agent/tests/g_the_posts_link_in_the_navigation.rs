//! **G_THE_POSTS_LINK_IN_THE_NAVIGATION — `(role, name)` is not a sufficient address for the real
//! web, and the landmark is the missing term.**
//!
//! `drive-probe` (t1459) measured it across six real sites: of the targets an agent can perceive but
//! cannot act on, **99% are ambiguous** — 516 of 521, against 2 ungrounded and 3 occluded. Listing
//! them named the mechanism in one run:
//!
//! ```text
//!   Ambiguous  link  "Posts"        Ambiguous  link  "GitHub"
//!   Ambiguous  link  "Spotlight"    Ambiguous  link  "Sitemap"
//!   Ambiguous  link  "About"        Ambiguous  link  "Back to top"
//! ```
//!
//! Every one appears **twice: once in the header nav, once in the footer.** Chrome's tree contains
//! the same twins, so this is not a projection defect to fix — it is an addressing scheme that
//! cannot express what a human says without thinking about it: *the `Posts` link **in the
//! navigation***.
//!
//! Re-keying the address as `(landmark, role, name)` was **priced on the corpus before it was
//! built**: `77.7% -> 81.1%` drivable, and `61.2% -> 77.6%` on the site whose duplication is purely
//! header-vs-footer. This gate is the mechanism that pricing was for.
//!
//! ⚠ **The unscoped call must keep working, unchanged.** `landmark: None` is the default and skips
//! the filter entirely, so every existing `click_by_name` resolves across the whole page exactly as
//! before. A new address that silently narrowed the old one would be a regression wearing a
//! feature's clothes.
//!
//! ⚠ And the landmark is **not the whole answer**, which the pricing also said: a site with no
//! landmarks does not move at all (news.ycombinator: `71.7% -> 71.7%`), and duplicates *within* one
//! landmark need a further term. The `Twin` row below is that limit, asserted so it cannot be
//! mistaken for a solved problem.
//!
//! Mutations that must turn this red:
//!   1. drop the landmark filter                → both scoped calls reach the same node
//!   2. filter on the node's OWN role           → nothing is inside a landmark; every call fails
//!   3. `matches` → equality on the scope        → `Region` vs an aria-labelled `<section>` diverge
//!   4. apply the filter BEFORE scoring         → the confidence margin counts excluded runners-up

use manuk_a11y::Role;
use manuk_agent::AgentBrowser;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8><title>Blog</title><style>
body { margin: 0; font: 16px/1.4 monospace }
a { display: block; width: 160px; height: 24px }
</style></head><body>
<header><nav aria-label="Main"><a href="/posts" id="nav-posts">Posts</a><a href="/about">About</a></nav></header>
<main><a href="/first">First article</a><a href="/dup">Twin</a><a href="/dup2">Twin</a></main>
<footer><a href="/posts-archive" id="foot-posts">Posts</a><a href="/about-us">About</a></footer>
</body></html>"##;

fn browser() -> (tokio::runtime::Runtime, AgentBrowser) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let path = std::env::temp_dir().join("g_posts_link_in_nav.html");
    std::fs::write(&path, HTML).unwrap();
    let url = format!("file://{}", path.display());
    let mut b = AgentBrowser::new(1024, 768);
    rt.block_on(async { b.navigate(&url).await.expect("navigate") });
    (rt, b)
}

#[test]
fn a_landmark_disambiguates_a_duplicated_link() {
    let (rt, b) = browser();
    let tree = b.a11y_tree().expect("a11y tree");
    let viewport = manuk_a11y::Rect {
        x: 0.0,
        y: 0.0,
        width: 1024.0,
        height: 768.0,
    };

    // ── VACUITY. There must genuinely be two `Posts` links, or nothing below disambiguates
    //    anything. This is the row that would have caught a fixture whose footer never rendered.
    fn count(n: &manuk_a11y::A11yNode, name: &str) -> usize {
        (n.role == Role::Link && n.name.trim() == name) as usize
            + n.children.iter().map(|c| count(c, name)).sum::<usize>()
    }
    assert_eq!(
        count(&tree, "Posts"),
        2,
        "VACUOUS: the page does not contain two `Posts` links, so the landmark is not being asked \
         to choose between anything"
    );

    let in_nav = manuk_agent::targeting::resolve_target_in(
        &tree,
        "Posts",
        Some(&Role::Link),
        Some(&Role::Navigation),
        viewport,
    )
    .expect("a Posts link inside the navigation");
    let in_foot = manuk_agent::targeting::resolve_target_in(
        &tree,
        "Posts",
        Some(&Role::Link),
        Some(&Role::ContentInfo),
        viewport,
    )
    .expect("a Posts link inside the contentinfo");

    assert_ne!(
        in_nav.node, in_foot.node,
        "the two landmarks resolved to the SAME node — the address is still (role, name)"
    );

    // Which is which: the nav one is the higher on the page.
    let (ny, fy) = (in_nav.point.1, in_foot.point.1);
    assert!(
        ny < fy,
        "the navigation's Posts must be the one nearer the top of the document, got nav y={ny} \
         footer y={fy}"
    );

    // ── THE UNSCOPED CALL IS UNCHANGED. It must still resolve, and to one of the two.
    let unscoped =
        manuk_agent::targeting::resolve_target_scoped(&tree, "Posts", Some(&Role::Link), viewport)
            .expect("the unscoped address must keep working");
    assert!(
        unscoped.node == in_nav.node || unscoped.node == in_foot.node,
        "the unscoped resolver stopped reaching either Posts link"
    );

    // ── ⚠ THE LIMIT, ASSERTED. Two `Twin` links inside ONE landmark are still ambiguous; the
    //    landmark cannot separate them and this gate must not imply it can.
    let twin = manuk_agent::targeting::resolve_target_in(
        &tree,
        "Twin",
        Some(&Role::Link),
        Some(&Role::Main),
        viewport,
    )
    .expect("a Twin link inside main");
    assert!(
        twin.confidence < 0.5,
        "two identical links in ONE landmark must still read as low confidence — the landmark is \
         not the whole answer, got {}",
        twin.confidence
    );

    // ── AND THE ACTION PATH IS WIRED, not just the scorer. A capability with no production caller
    //    is the shape this repo keeps finding (t1402, t1403).
    //    ⭐ The assertion is on WHICH href the activation followed, not on whether the navigation
    //    succeeded — these are `file://` links to nothing, so it cannot. The nav's Posts points at
    //    `/posts` and the footer's at `/posts-archive`, so the attempted URL names the link that
    //    was actually clicked. A bare `is_ok()` here would have been satisfied by clicking either.
    let mut b = b;
    let err = rt
        .block_on(async {
            b.click_by_name_in(&Role::ContentInfo, &Role::Link, "Posts")
                .await
        })
        .map(|a| format!("{a:?}"))
        .unwrap_or_else(|e| format!("{e:#}"));
    assert!(
        err.contains("posts-archive"),
        "click_by_name_in(contentinfo) followed the wrong link — it must reach the FOOTER's \
         /posts-archive and not the nav's /posts; got {err}"
    );

    let err_nav = rt
        .block_on(async {
            b.click_by_name_in(&Role::Navigation, &Role::Link, "Posts")
                .await
        })
        .map(|a| format!("{a:?}"))
        .unwrap_or_else(|e| format!("{e:#}"));
    assert!(
        !err_nav.contains("posts-archive"),
        "click_by_name_in(navigation) reached the FOOTER's link; got {err_nav}"
    );
}
