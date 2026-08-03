//! G_CLICK_POINT — **the agent's click point is the a11y bbox centre, and nothing asserted it.**
//!
//! Constitution check #72 found that invariant **I3** — the semantic model an agent acts through —
//! was being satisfied **by accident**. The chain is real and it is short:
//!
//! ```text
//!   LayoutBox::node_rects()  →  manuk_a11y::build_tree_with_rects  →  A11yNode.bbox  →  the CLICK POINT
//! ```
//!
//! Every geometry tick therefore moves where the agent clicks. Five ticks in one window did
//! (t846, t848, t849, t850, t851), and every one passed I3 *because `node_rects` is a shared
//! producer, not because anyone checked* — which stops protecting anything the moment a fix touches
//! the producer itself. Checks #72, #74 and #75 have now carried the same steer three times: **land
//! the assertion.** This is it.
//!
//! What it asserts is deliberately not "the rects are right" — the layout suite does that, in
//! layout's own units, and a rect that is right in px can still be un-clickable (t853: sixteen
//! Wikipedia links were lost to a tie-break, on rects that were *more* correct than before). This
//! gate asserts the property an agent actually depends on:
//!
//! > **Clicking the centre of the box an element reports must reach that element.**
//!
//! …and its adversarial half, which is the one that catches a whole class of silent I3 failure:
//!
//! > **A box that should not be there must not eat the click.**
//!
//! The three constructs are the ones this window's render fixes moved, each reduced to its minimum:
//!
//!   1. an icon-plus-label link inside a padded `float` (t871 — the anchor came out 29px too narrow
//!      and 18px too tall, so its centre was in the wrong place);
//!   2. a content column beside a float that establishes a BFC (t873 — the column rendered *under*
//!      the float, so the float ate its clicks);
//!   3. an off-canvas drawer translated off-screen (t874 — `transform` was silently discarded on a
//!      flex item that is itself a flex container, so an invisible full-height panel sat on top of
//!      the header. **Nothing looks wrong in a screenshot and every click lands on the wrong
//!      element** — the worst shape an I3 defect has.)
//!
//! ⚠⚠⚠ **RED PROOF — MEASURED, AND ONLY ONE OF THE FOUR CLAUSES GOES RED. SAYING WHICH IS THE
//! POINT.** All four of this window's render fixes were reverted one at a time against this gate:
//!
//! ```text
//!   t874  transform discarded in `extract_placed`      → FAILS  (drawer box becomes -260..260)
//!   t873  the §9.5 BFC float band                      → still green
//!   t871  `text-align` in the intrinsic probe          → still green
//!   t872  the static-position translate                → still green
//! ```
//!
//! That is honest and it is worth reading twice. **The falsifiable content of this gate is the
//! adversarial half**: a box that renders where it was authored *not* to, and eats the clicks
//! underneath it. The three positional clauses are **standing guards**, not proofs — when a box is
//! wrongly placed, its own centre usually moves *with* it, so the click still lands and only the
//! pixels are wrong. They exist for the class where that stops being true: t853, where sixteen
//! Wikipedia links became unclickable on rects that were *more* correct than before, and any future
//! change that separates an element's reported box from where it can actually be hit.
//!
//! A guard that cannot be shown to go red is not evidence, and is not claimed as any.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html>
<html><head><style>
  html, body { margin: 0; padding: 0; font: 16px/1.2 monospace; }

  /* 1 — the icon-plus-label link, in a padded float (t871). */
  .toolbar a { display: block; float: left; padding: 10px 15px; text-align: center; }
  .toolbar a span { display: inline-block; }

  /* 2 — a float beside a BFC column (t873). */
  .media { width: 600px; clear: both; }
  .media img { float: left; width: 120px; height: 80px; }
  .media .body { overflow: hidden; }

  /* 3 — the off-canvas drawer: a flex container inside a flex container,
         translated fully off-screen (t874). */
  .app { display: flex; flex-direction: column; width: 600px; clear: both; }
  .drawer {
    position: absolute; top: 0; left: 0; width: 260px; height: 400px;
    display: flex; flex-direction: column;
    transform: translateX(-100%);
  }
</style></head><body>
  <div class="toolbar"><a href="#"><i>&#9776;</i> <span>Menu</span></a></div>

  <div class="media">
    <img src="data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7" alt="thumb">
    <div class="body"><a href="#">Read the article</a></div>
  </div>

  <div class="app">
    <div class="drawer"><a href="#">Offscreen drawer link</a></div>
    <button>Save</button>
  </div>
</body></html>"##;

/// The first node in the tree whose accessible name is exactly `name`.
fn by_name<'a>(root: &'a manuk_a11y::A11yNode, name: &str) -> &'a manuk_a11y::A11yNode {
    fn walk<'a>(n: &'a manuk_a11y::A11yNode, name: &str) -> Option<&'a manuk_a11y::A11yNode> {
        if n.name.trim() == name {
            return Some(n);
        }
        n.children.iter().find_map(|c| walk(c, name))
    }
    walk(root, name).unwrap_or_else(|| {
        panic!(
            "no a11y node named {name:?} — the tree is:\n{:#?}",
            root.to_observation_lines()
        )
    })
}

/// Does `hit` land on `want`, or on something inside it? A click that reaches a descendant of the
/// element has reached the element — the shell walks UP from whatever was hit.
fn is_self_or_descendant(want: &manuk_a11y::A11yNode, hit: manuk_dom::NodeId) -> bool {
    if want.node == hit {
        return true;
    }
    want.children.iter().any(|c| is_self_or_descendant(c, hit))
}

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn the_click_point_of_every_named_control_reaches_it() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://click.test/", &fonts, 800.0);
    let tree = page.a11y_tree();

    // ── The invariant, stated once and applied three times. ────────────────────────────────────
    let centre_reaches = |name: &str| {
        let want = by_name(&tree, name);
        let b = want.bbox.unwrap_or_else(|| {
            panic!(
                "{name:?} has NO bbox — an element an agent can name and \
                                       cannot locate is an I3 failure on its own"
            )
        });
        assert!(
            b.width > 0.0 && b.height > 0.0,
            "{name:?} reports a degenerate box {b:?} — its centre is not a click point"
        );
        let (cx, cy) = (b.x + b.width / 2.0, b.y + b.height / 2.0);
        let hit = tree
            .hit_test(cx, cy)
            .unwrap_or_else(|| panic!("clicking the centre of {name:?} ({cx}, {cy}) hit NOTHING"));
        assert!(
            is_self_or_descendant(want, hit.node),
            "clicking the centre of {name:?} at ({cx}, {cy}) — the point an agent computes from \
             the box {name:?} itself reports — reached {:?} {:?} instead. The element names a \
             target it cannot be clicked at.",
            hit.role,
            hit.name
        );
    };

    // 1 — the icon-plus-label link in a padded float (t871).
    centre_reaches("☰ Menu");
    // 2 — the link in a BFC column beside a float (t873): the float must not be covering it.
    centre_reaches("Read the article");
    // 3 — the in-flow control the off-canvas drawer used to cover (t874).
    centre_reaches("Save");

    // ── THE ADVERSARIAL HALF: a box that should not be there must not eat the click. ───────────
    //
    // The drawer is translated fully off-screen, so nothing of it may be reachable at a positive
    // x. Before t874 the transform was discarded for a flex container inside a flex container and
    // this panel sat at x=0..260 over the whole column — invisible in a screenshot, and every
    // click in that band landed on it.
    let drawer = by_name(&tree, "Offscreen drawer link");
    let db = drawer
        .bbox
        .expect("the drawer link still has a box; it is merely off-screen");
    assert!(
        db.x + db.width <= 1.0,
        "an element with `transform: translateX(-100%)` must sit entirely at a negative x — got \
         {db:?}. A panel that renders where it was authored NOT to is the worst kind of I3 defect: \
         the screenshot looks right and every click underneath it goes to the wrong element."
    );
    let save = by_name(&tree, "Save");
    let sb = save.bbox.expect("the button has a box");
    let hit = tree
        .hit_test(sb.x + sb.width / 2.0, sb.y + sb.height / 2.0)
        .expect("the button is hittable");
    assert!(
        !is_self_or_descendant(drawer, hit.node) && drawer.node != hit.node,
        "the off-screen drawer took the click meant for the Save button — it reached {:?} {:?}",
        hit.role,
        hit.name
    );
}
