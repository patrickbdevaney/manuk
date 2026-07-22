//! **G_ACTIVE_PSEUDO — `:active` matches while the pointer is held, it matches ANCESTORS, and it
//! releases.**
//!
//! `:active` was the last of the three dynamic pseudo-classes left hard-coded `false` in
//! `stylo_dom.rs` (behind the comment "needs mousedown/mouseup, a separate input path from focus").
//! `:hover` (tick ~245) and `:focus` (tick 246) were both fed; `:active` was not, so the answer
//! stayed `false` for the life of every page and no press-feedback rule ever showed.
//!
//! ## What that cost
//!
//! `:active` is the press-feedback primitive. `button:active { transform: translateY(1px) }`,
//! `a:active { color: … }`, the nav item that darkens while tapped — the visual confirmation that a
//! control received the press, on essentially every interactive site. With the pseudo-class never
//! matching, every one of those was inert, and — like the hover-menu case — **nothing reported it**:
//! the page rendered exactly what it was told, minus a state that never arrived.
//!
//! ## The ancestor half is the mechanism, not a detail
//!
//! `:active` matches the pressed element **and every ancestor of it** — a press on a button inside a
//! card lights the card's own `.card:active` rule. Match only the exact target and a whole idiom
//! (press-anywhere-in-this-panel feedback) silently fails.
//!
//! ## The RED probes (run, not imagined)
//!
//! * `P::Active => false` restored (the old line) → `pressed:` and `ancestor:` both fail; nothing styles.
//! * `is_active` matching only the exact target → `ancestor:` alone fails.
//! * `set_active` not clearing on release, or `Page::set_active` skipping the recascade → `release:`
//!   fails: the element stays stuck in its pressed style forever.
//! * The button rule lives in an EXTERNAL sheet on purpose (the g_hover lesson): `Page::set_active`
//!   recascades via `recascade_all_sources`, and a fixture using only inline `<style>` cannot see the
//!   bug where a hover/press recascade drops every `<link>`ed stylesheet.

use manuk_text::FontContext;
use std::collections::HashMap;

const W: f32 = 800.0;

/// `#btn`'s `:active` rule is in an EXTERNAL sheet — see the module note (the g_hover trap).
const BTN_CSS: &str =
    "#btn { width: 100px; height: 40px; background: #ccc; padding: 0; border: 0; \
                       box-sizing: border-box; } \
                       #btn:active { width: 300px; }";

/// A button inside a card, plus a label the CARD's `:active` rule restyles — so pressing the button
/// (a descendant) must widen the label (a sibling), which only happens if `:active` matches the
/// ancestor card AND the ancestor gets dirtied and restyled.
const HTML: &str = r##"<!doctype html><html><head><style>
  body { margin: 0 }
  #label { width: 50px; height: 10px; }
  .card:active #label { width: 250px; }
</style>
<link rel="stylesheet" href="/btn.css">
</head><body>
<div class="card"><button id="btn">Press</button><div id="label"></div></div>
</body></html>"##;

fn node_of(page: &manuk_page::Page, sel: &str) -> manuk_dom::NodeId {
    let root = page.dom().root();
    manuk_css::query_selector_all(page.dom(), root, sel)[0]
}

fn width_of(page: &manuk_page::Page, sel: &str) -> f32 {
    let n = node_of(page, sel);
    page.node_rects().get(&n).map(|r| r.width).unwrap_or(-1.0)
}

/// One test in the binary — a second `Page::load` stands up a second SpiderMonkey context in the
/// same process and does not survive teardown (see `g_hover`, `g_mse`).
#[test]
fn pressing_matches_active_restyles_ancestors_and_releases() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://active.test/", &fonts, W);
    let external = HashMap::from([(
        "https://active.test/btn.css".to_string(),
        BTN_CSS.to_string(),
    )]);
    page.apply_stylesheets(&external, &fonts, W);

    let btn = node_of(&page, "#btn");
    let card = node_of(&page, ".card");

    // ── Baseline: nothing pressed. #btn is its unpressed width; the label is its base width.
    assert!(
        (width_of(&page, "#btn") - 100.0).abs() < 0.5,
        "G_ACTIVE: #btn is 100px unpressed — got {}. The fixture's own baseline is wrong.",
        width_of(&page, "#btn")
    );
    assert!(
        (width_of(&page, "#label") - 50.0).abs() < 0.5,
        "G_ACTIVE: #label is 50px before any press — got {}",
        width_of(&page, "#label")
    );
    assert!(
        !page.dom().is_active(btn) && !page.dom().is_active(card),
        "G_ACTIVE: nothing is active before a press"
    );

    // ── Press the button (mousedown). `:active` must match it, and its OWN external rule apply.
    let changed = page.set_active(Some(btn), &fonts, W);
    assert!(
        changed,
        "G_ACTIVE: pressing from nothing must report an active CHANGE"
    );
    assert!(
        (width_of(&page, "#btn") - 300.0).abs() < 0.5,
        "G_ACTIVE: `#btn:active {{ width: 300px }}` (from the EXTERNAL sheet) must apply while the \
         button is pressed — got {}. 100 means `:active` still answers false in the cascade (the \
         pseudo-class is unfed), or the recascade dropped the external stylesheet.",
        width_of(&page, "#btn")
    );

    // ── THE ANCESTOR CLAIM. The button is a descendant of `.card`; `.card:active` must match, so its
    //    rule widens the label. Matching only the exact hit target leaves this idiom dead.
    assert!(
        page.dom().is_active(card),
        "G_ACTIVE: `:active` must match the ANCESTOR .card while its descendant button is pressed"
    );
    assert!(
        (width_of(&page, "#label") - 250.0).abs() < 0.5,
        "G_ACTIVE: `.card:active #label {{ width: 250px }}` must apply — got {}. The base 50 means \
         the ancestor .card was never restyled: either :active does not match ancestors, or \
         set_active marked only the endpoints instead of walking both chains (a dirty bit is per \
         NODE, not per subtree).",
        width_of(&page, "#label")
    );

    // ── Pressing the SAME element again is not a change (no needless recascade).
    assert!(
        !page.set_active(Some(btn), &fonts, W),
        "G_ACTIVE: re-pressing the element already active must report no change"
    );

    // ── Release (mouseup). Everything must return to its unpressed geometry — a state only ever
    //    ADDED leaves every pressed control stuck lit forever.
    assert!(
        page.set_active(None, &fonts, W),
        "G_ACTIVE: releasing must report a change"
    );
    assert!(
        !page.dom().is_active(btn) && !page.dom().is_active(card),
        "G_ACTIVE: nothing is active after release"
    );
    assert!(
        (width_of(&page, "#btn") - 100.0).abs() < 0.5,
        "G_ACTIVE: #btn returns to 100px on release — got {}",
        width_of(&page, "#btn")
    );
    assert!(
        (width_of(&page, "#label") - 50.0).abs() < 0.5,
        "G_ACTIVE: #label returns to 50px on release — got {}",
        width_of(&page, "#label")
    );
}
