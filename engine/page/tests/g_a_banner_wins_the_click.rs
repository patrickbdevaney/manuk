//! **G_A_BANNER_WINS_THE_CLICK — a positioned element with `z-index: auto` paints above in-flow
//! content, and this engine gave it the same layer, so an agent clicked THROUGH a cookie banner and
//! reported success.**
//!
//! CSS 2.1 Appendix E orders painting within a stacking context: in-flow blocks, floats and inlines
//! are steps 3-7, and **positioned descendants with `z-index: auto` or `0` are step 8** — strictly
//! later, so strictly on top. `z_index_map` computed `s.z_index.unwrap_or(parent_z)`, handing an
//! `auto` overlay its parent's layer; `A11yNode::hit_test` then broke the tie by **smaller area**,
//! which the content underneath wins.
//!
//! ⭐⭐⭐ **A COOKIE BANNER IS EXACTLY THIS MARKUP** — `position: absolute` or `fixed`, no `z-index`,
//! larger than what it covers. The click landed on the link *behind* the banner and every layer
//! above reported success. That is the silent misfire, which is worse than an error: nothing
//! downstream can tell it happened.
//!
//! Chrome-measured (`document.elementFromPoint` at each link's centre):
//!
//! ```text
//!                                                Chrome    before    after
//!   l1  auto-z overlay over an in-flow link        b1        l1        b1
//!   l2  z-index:5 overlay                          b2        b2        b2   ✓ (explicit z worked)
//!   l3  z-index:-1 underlay                        l3        l3        l3   ✓ negative stays under
//!   l4  no overlay                    CONTROL      l4        l4        l4   ✓
//! ```
//!
//! ⭐⭐ **`l2` AND `l3` ARE WHAT MAKE THIS A MISSING CASE RATHER THAN A MISSING FEATURE.** An
//! explicit `z-index: 5` already won and an explicit `-1` already lost, so the layer machinery was
//! working — only the `auto` spelling of it was absent. And `l3` is the row that stops the naive fix:
//! "positioned beats in-flow" applied unconditionally would raise the `z-index: -1` underlay too.
//!
//! The scale is `n * 1024 + 1` for an explicit `z-index` and `parent + 1` for `auto`, so 1023 levels
//! of nested `auto` positioning still sit below `z-index: 1`, and an explicit `z-index: 0` — also
//! step 8 — still clears in-flow content at 1.
//!
//! ⚠⚠ **WHAT THIS DOES NOT FIX, MEASURED AND NAMED.** When the covered content is *itself*
//! positioned with `z-index: auto`, the two are step-8 **peers** and the spec orders them by tree
//! order — later wins. `A11yNode::hit_test` breaks an equal-layer tie by smaller area instead, so a
//! positioned link under an auto overlay still wins the click (Chrome says the overlay). Fixing it
//! needs a `positioned` bit on `A11yNode`, because area is the right question only for *unrelated*
//! in-flow boxes — the t853 `<li>`/`<a>` float dust the tie-break was written for. The `l5` row
//! below asserts the current answer so the limit cannot be mistaken for correctness.
//!
//! ⚠⚠⚠ **AND `document.elementFromPoint` IS A SECOND IMPLEMENTATION THAT IGNORES z-index ENTIRELY.**
//! It is a flat scan over the layout rects resolving by smallest area, consulting `pointer-events`
//! and SVG paintedness but never a layer — so it reads `l1=l1 l2=l2 l3=l3 l4=l4 l5=l5`, wrong on
//! *both* overlay rows including the explicit `z-index: 5` one that the a11y path gets right. Two
//! implementations of one rule, disagreeing. Named here with its measurement rather than fixed in
//! the same tick as the paint-order change, because they need separate controls.
//!
//! Mutations that must turn this red:
//!   1. `s.z_index.unwrap_or(parent_z)`      → l1 reads l1 (the original bug)
//!   2. `None => parent_z + 1024`            → l3's `z-index: -1` underlay rises above the link
//!   3. explicit arm `n` instead of `n*1024` → the `Deep vs z1` row: a 4-deep `auto` chain outranks
//!      an explicit `z-index: 1`
//!
//! ⭐⭐ **MUTATIONS 2 AND 3 CAME BACK GREEN THE FIRST TIME, AND NAMED A HOLE IN THIS FIXTURE.** The
//! stated reason for row 2 was wrong: a `z-index: -1` underlay is negative and can never rise, no
//! matter how large the `auto` bump. Neither a shallow overlay nor a negative one can discriminate
//! the SCALE — only a deep `auto` chain measured against a *small* explicit `z-index` can, because
//! that is the only place the two encodings order differently. `Deep vs z1` is that row (Chrome:
//! `z1`), and it is red under both.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8><style>
body { margin: 0; font: 16px/1.4 monospace }
.row { position: relative; width: 300px; height: 60px; margin: 4px; background: #eef; overflow: hidden }
.row a { display: block; width: 200px; height: 30px; background: #cfc }
.over  { position: absolute; left: 0; top: 0; width: 300px; height: 60px }
.zover { position: absolute; left: 0; top: 0; width: 300px; height: 60px; z-index: 5 }
.under { position: absolute; left: 0; top: 0; width: 300px; height: 60px; z-index: -1 }
.nest  { position: relative }
.deep  { position: absolute; left: 0; top: 0; width: 300px; height: 60px }
.z1    { position: absolute; left: 0; top: 0; width: 300px; height: 60px; z-index: 1 }
</style></head><body>
<div class="row" id="r1"><a href="/a" id="l1">Auto over</a><div class="over" id="b1"></div></div>
<div class="row" id="r2"><a href="/b" id="l2">Z over</a><div class="zover" id="b2"></div></div>
<div class="row" id="r3"><a href="/c" id="l3">Negative</a><div class="under" id="b3"></div></div>
<div class="row" id="r4"><a href="/d" id="l4">No over</a></div>
<div class="row" id="r6">
  <div class="nest"><div class="nest"><div class="nest"><div class="deep" id="deep"></div></div></div></div>
  <a href="/f" id="l6">Deep vs z1</a>
  <div class="z1" id="z1"></div></div>
<div class="row" id="r5"><a href="/e" id="l5" style="position:absolute;left:0;top:0;width:200px;height:30px">Both positioned</a><div class="over" id="b5"></div></div>
</body></html>"##;

fn id_of(page: &manuk_page::Page, n: manuk_dom::NodeId) -> String {
    page.dom()
        .element(n)
        .and_then(|e| e.attr("id").map(str::to_string))
        .unwrap_or_else(|| "?".into())
}

fn find<'a>(n: &'a manuk_a11y::A11yNode, name: &str) -> Option<&'a manuk_a11y::A11yNode> {
    if n.name.trim() == name {
        return Some(n);
    }
    n.children.iter().find_map(|c| find(c, name))
}

#[test]
fn an_auto_z_overlay_wins_the_click_over_in_flow_content() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://banner.test/", &fonts, 800.0);
    let tree = page.a11y_tree();

    let hit = |name: &str| -> String {
        let t = find(&tree, name).unwrap_or_else(|| panic!("no node named {name:?}"));
        let b = t.bbox.unwrap_or_else(|| panic!("{name:?} has no box"));
        tree.hit_test(b.x + b.width / 2.0, b.y + b.height / 2.0)
            .map(|h| id_of(&page, h.node))
            .unwrap_or_else(|| "none".into())
    };

    // ── VACUITY. With no overlay at all the link must win its own centre, or every row below is
    //    measuring whether hit-testing works rather than which LAYER wins.
    assert_eq!(
        hit("No over"),
        "l4",
        "VACUOUS: a link with nothing on top of it does not win its own centre"
    );

    // ⭐ THE DEFECT: an `auto` overlay must take the click.
    assert_eq!(
        hit("Auto over"),
        "b1",
        "a positioned `z-index: auto` overlay is CSS 2.1 step 8 and paints above in-flow content — \
         an agent clicking here hits the banner, not the link behind it"
    );

    // The two rows that make this a missing CASE, not a missing feature.
    assert_eq!(hit("Z over"), "b2", "an explicit z-index already worked");
    assert_eq!(
        hit("Negative"),
        "l3",
        "a `z-index: -1` underlay must stay UNDER — this is the row a blanket \
         `positioned beats in-flow` rule would break"
    );

    // ⭐ THE SCALE. A 4-deep chain of positioned `auto` boxes must still lose to an explicit
    //   `z-index: 1`. This is the only row that can see the difference between `n * 1024` and `n`,
    //   or between an `auto` bump of 1 and one of 1024 — and both of those mutations passed
    //   everything else.
    assert_eq!(
        hit("Deep vs z1"),
        "z1",
        "a nested `auto` chain must stay below an explicit `z-index: 1` — Chrome says z1"
    );

    // ⚠ THE LIMIT, ASSERTED SO IT CANNOT BE MISTAKEN FOR CORRECTNESS. Chrome says `b5`: two
    //   step-8 peers are ordered by tree order, and `hit_test` breaks an equal-layer tie by area.
    assert_eq!(
        hit("Both positioned"),
        "l5",
        "if this now reads `b5` the equal-layer tie-break has learned tree order — DELETE this row \
         and assert Chrome's answer instead, which is the outcome this gate wants"
    );
}
