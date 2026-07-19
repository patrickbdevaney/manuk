//! **G_HOVER — `:hover` matches, it matches ANCESTORS, and the mouse events fire in spec order.**
//!
//! `:hover` was hard-coded `false` in `stylo_dom.rs` alongside `:active` and `:focus`, behind a
//! comment that was *correct about a static render and wrong about a browser*: "a page is not being
//! hovered when it is laid out." True — and the shell never fed it afterwards either, so the answer
//! stayed `false` for the life of every page.
//!
//! ## What that actually cost, which is not "buttons don't light up"
//!
//! **The hover-reveal navigation menu is a desktop-web primitive.** `nav li:hover > ul { display:
//! block }` is how a very large share of sites build top navigation with **no JavaScript at all** —
//! the same structural trick as the checkbox hack that `:checked` unblocked. With `:hover` never
//! matching, every one of those menus is permanently closed. The links inside are unreachable to a
//! user and invisible to an agent, and **nothing reports a problem**: the page renders exactly what
//! it was told to render. It is a whole category of navigation missing in silence.
//!
//! ## The ancestor half is the mechanism, not a detail
//!
//! `:hover` matches the hovered element **and every ancestor of it**. Match only the exact hit
//! target and the menu above still fails — in a way that looks like it works. The pointer enters
//! the `<li>`, the submenu opens; the pointer moves one pixel *into that submenu* and is now over
//! an `<a>` inside the `<ul>`, so the `<li>` stops matching and the menu it just opened closes
//! underneath the cursor. That is the flickering-menu bug, and it is why claim `ancestor:` below is
//! the one carrying the design rather than decorating it.
//!
//! ## The RED probes (run, not imagined)
//!
//! * `P::Hover => false` restored           → `hovered:` and `ancestor:` both fail; nothing styles.
//! * `is_hovered` matching only the exact target → `ancestor:` alone fails; the flickering-menu bug.
//! * `set_hovered` marking only the endpoints, not the chains → `ancestor:` fails, because the
//!   `<li>` whose rule started matching is never restyled. A dirty bit is per node, not per subtree.
//! * `mouseout` moved after `mouseover` → `order:` fails. Menu code starts a close timer on leave
//!   and cancels it on enter, so the wrong order closes a menu the pointer is still inside.

use manuk_text::FontContext;
use std::collections::HashMap;

/// **`#btn`'s rules live in an EXTERNAL sheet on purpose.** The first version of this gate put them
/// in the inline `<style>` above, and it passed against a hover path that rebuilt the cascade from
/// inline `<style>` elements ONLY — silently dropping every `<link>`ed stylesheet on every hover.
/// A fixture that uses one kind of stylesheet cannot see a bug that only affects the other kind, and
/// "the fixture was inside the blast radius" is the difference between a gate and a decoration.
const BTN_CSS: &str = "#btn { width: 100px; height: 40px; background: #ccc; } \
                       #btn:hover { width: 300px; }";

/// A hover-reveal menu, written the way the web writes one: no script drives the reveal.
const HTML: &str = r##"<!doctype html><html><head><style>
  body { margin: 0 }
  nav ul.sub { display: none; }
  nav li:hover > ul.sub { display: block; }
</style>
<link rel="stylesheet" href="/btn.css">
</head><body>
<nav><ul><li id="item"><a id="top" href="#">Products</a>
  <ul class="sub"><li><a id="deep" href="/pricing">Pricing</a></li></ul>
</li></ul></nav>
<div id="btn"></div>
<div id="log">-</div>
<script>
  var seq = [];
  // Every handler republishes into the DOM, because that is the only channel the host can read
  // back — and it must be EVERY handler, not just the last kind: a gate that only sees the final
  // event cannot assert the ORDER of the ones before it.
  var pub = function (t, who) {
    seq.push(who + ':' + t);
    document.getElementById('log').textContent = seq.join(',');
  };
  ['mouseover','mouseout','mouseenter','mouseleave','mousemove'].forEach(function (t) {
    document.getElementById('item').addEventListener(t, function () { pub(t, 'item'); });
    document.getElementById('btn').addEventListener(t, function () { pub(t, 'btn'); });
  });
</script>
</body></html>"##;

/// One test in the binary — a second `Page::load` stands up a second SpiderMonkey context in the
/// same process and does not survive teardown (see `g_mse`, `g_quirks_mode`, `g_globals`).
#[test]
fn hovering_reveals_the_menu_restyles_ancestors_and_fires_mouse_events_in_order() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://hover.test/", &fonts, 800.0);
    let external = HashMap::from([(
        "https://hover.test/btn.css".to_string(),
        BTN_CSS.to_string(),
    )]);
    page.apply_stylesheets(&external, &fonts, 800.0);

    let rect_of = |page: &manuk_page::Page, sel: &str| {
        let root = page.dom().root();
        let n = manuk_css::query_selector_all(page.dom(), root, sel)[0];
        page.node_rects().get(&n).copied()
    };
    let node_of = |page: &manuk_page::Page, sel: &str| {
        let root = page.dom().root();
        manuk_css::query_selector_all(page.dom(), root, sel)[0]
    };

    // ── Baseline. Nothing is hovered, so the submenu is display:none and has no box at all.
    assert!(
        rect_of(&page, "#deep").is_none(),
        "G_HOVER: the submenu link must have NO box before anything is hovered — `ul.sub` is \
         display:none until `li:hover` matches. A box here means the fixture is not testing what it \
         claims and every assertion below is vacuous."
    );
    let btn0 = rect_of(&page, "#btn").expect("#btn has a box");
    assert!(
        (btn0.width - 100.0).abs() < 0.5,
        "G_HOVER: #btn is 100px wide unhovered — got {}. The fixture's own baseline is wrong.",
        btn0.width
    );

    // ── Hover the button. The simple case: the hovered element's OWN rule.
    let target = btn0;
    let changed = page.dispatch_hover_at(
        target.x + target.width / 2.0,
        target.y + target.height / 2.0,
        &fonts,
        800.0,
    );
    assert!(
        changed,
        "G_HOVER: moving the pointer onto #btn from nothing must report a hover CHANGE"
    );
    let btn1 = rect_of(&page, "#btn").expect("#btn still has a box");
    assert!(
        (btn1.width - 300.0).abs() < 0.5,
        "G_HOVER: `#btn:hover {{ width: 300px }}` must apply once the pointer is over it — got {}. \
         100 means `:hover` still answers false in the cascade, i.e. the pseudo-class is not fed.",
        btn1.width
    );

    // ── Moving WITHIN the same element is not a change. Pointer-move arrives at motion rates and
    //    almost every event lands on the element already hovered; recascading for those is a
    //    per-frame cost for no visual change.
    let again = page.dispatch_hover_at(target.x + 5.0, target.y + 5.0, &fonts, 800.0);
    assert!(
        !again,
        "G_HOVER: a second pointer move landing on the SAME element must report no change — \
         otherwise every mouse-move recascades the whole document"
    );

    // ── THE ANCESTOR CLAIM. Hover the <a> INSIDE the <li>; the rule is on the <li>.
    let top = rect_of(&page, "#top").expect("#top has a box");
    page.dispatch_hover_at(
        top.x + top.width / 2.0,
        top.y + top.height / 2.0,
        &fonts,
        800.0,
    );
    assert!(
        page.dom().is_hovered(node_of(&page, "#item")),
        "G_HOVER: `:hover` must match the ANCESTOR <li> while the pointer is over the <a> inside \
         it. Matching only the exact hit target is the flickering-menu bug: the submenu opens, the \
         pointer moves into it, the <li> stops matching and the menu closes under the cursor."
    );
    let deep = rect_of(&page, "#deep");
    assert!(
        deep.is_some_and(|r| r.width > 0.0 && r.height > 0.0),
        "G_HOVER: `nav li:hover > ul.sub {{ display: block }}` must reveal the submenu, giving \
         #deep a real box — got {deep:?}. None means the ancestor <li> was never restyled: either \
         :hover does not match ancestors, or set_hovered marked only the endpoints instead of \
         walking both chains (a dirty bit is per NODE, not per subtree)."
    );
    assert!(
        (rect_of(&page, "#btn").expect("#btn").width - 100.0).abs() < 0.5,
        "G_HOVER: #btn must return to 100px once the pointer leaves it. A hover state that is only \
         ever ADDED leaves every element the pointer has ever touched stuck in its hover style."
    );

    // ── The events, and their order.
    let log = {
        let root = page.dom().root();
        let n = manuk_css::query_selector_all(page.dom(), root, "#log")[0];
        page.dom().text_content(n)
    };
    for (claim, why) in [
        ("btn:mouseover", "a page listens for mouseover to build tooltips, previews and hover cards; without it none of them ever appear"),
        ("btn:mouseenter", "mouseenter is the non-bubbling pair every menu library actually binds"),
        ("btn:mousemove", "mousemove is what drag/hover-tracking UIs read for coordinates"),
        ("btn:mouseout", "leaving must fire, or a tooltip opens and never closes"),
        ("item:mouseover", "the events must reach the element the pointer moved ONTO, not only the one it left"),
    ] {
        assert!(
            log.contains(claim),
            "G_HOVER: missing {claim:?} — {why}.\n  seq: {log}"
        );
    }

    // ── ORDER. out/leave on what was left, THEN over/enter on what was entered.
    let out = log.find("btn:mouseout").expect("mouseout present");
    let over = log.find("item:mouseover").expect("mouseover present");
    assert!(
        out < over,
        "G_HOVER: mouseout on the element being LEFT must fire before mouseover on the element \
         being ENTERED. Menu code starts a close timer on leave and cancels it on enter — reverse \
         the order and the timer is armed after its own cancel, so the menu closes behind a pointer \
         that is still inside it.\n  seq: {log}"
    );
}
