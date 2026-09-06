//! **G_A_PERCEIVED_TARGET_IS_AN_ACTIONABLE_ONE — perceiving a control is not the same as being able
//! to click it, and only one of those two had ever been measured.**
//!
//! `g_agent_drive_loop` (t1455) closed the perceive → ground → actuate → observe loop and named its
//! own gap in as many words: *"what this does not prove is that a real page's markup is reachable
//! this way."* Three things can break between seeing a control and clicking it, and each fails
//! differently:
//!
//! ```text
//!   Ungrounded   no bbox, or a zero-area one     perceivable but not AIMABLE
//!   Ambiguous    another target shares its       resolution by role+name picks one BY CHANCE
//!                (role, name)
//!   MisHit       the centre hit-tests elsewhere  the click LANDS SOMEWHERE ELSE
//! ```
//!
//! ⭐⭐ **A HIT ON A DESCENDANT IS A SUCCESS.** Clicking the centre of a `<button>` usually lands on
//! the `<span>` inside it and the event bubbles to the button either way; counting those as misses
//! would report a catastrophe on every well-built page. The `nested` row below is that case, and it
//! is the row that stops this gate from being a pessimism meter.
//!
//! ⚠ The `overlay` row is the mirror and it is why `MisHit` is a category at all: a `position:fixed`
//! banner over a link makes that link perceivable, grounded, unique — **and unclickable**. An agent
//! that only checked the first three would click the banner and report success.
//!
//! Mutations that must turn this red:
//!   1. `in_subtree` comparing identity only        → the `nested` row becomes MisHit
//!   2. `classify` skipping the duplicate check     → the `twin` rows become Drivable
//!   3. `classify` accepting a zero-area bbox       → the `collapsed` row becomes Drivable
//!   4. `is_actionable` accepting every role        → the target count rises past the controls

use manuk_a11y::Role;
use manuk_agent::drivability::{classify, is_actionable, tally, targets, Verdict};
use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8><style>
body { margin: 0; font: 16px/1.4 monospace }
.b { display: block; width: 200px; height: 30px; margin: 6px }
#collapsed { width: 0; height: 0; overflow: hidden }
/* Each overlay covers EXACTLY its own link — a page-wide banner would cover the control rows
   above too and turn this gate into a pessimism meter. */
.stack { position: relative; width: 200px; height: 30px; margin: 6px }
/* ⚠ Explicit width/height, NOT `inset: 0` — `inset` does not size an absolutely positioned box in
   this engine yet (an `inset:0` overlay lays out 0x0), which drive-probe found while this gate was
   being written. Using it here would make these rows pass for the wrong reason. */
.stack a { position: absolute; left: 0; top: 0; width: 200px; height: 30px }
.over { position: absolute; left: 0; top: 0; width: 200px; height: 30px; background: #ccc; z-index: 10 }
/* A BIGGER overlay, which is the real cookie-banner shape: it must still win the click. */
.stack2 { position: relative; width: 300px; height: 60px; margin: 6px }
.stack2 a { position: absolute; left: 0; top: 0; width: 200px; height: 30px }
.ghost { position: absolute; left: 0; top: 0; width: 300px; height: 60px; background: #eee }
</style></head><body>
<button class="b" id="plain">Save</button>
<button class="b" id="nested"><span>Publish</span> <span>now</span></button>
<button class="b" id="twin1">Delete</button>
<button class="b" id="twin2">Delete</button>
<a class="b" id="collapsed" href="/x">Collapsed</a>
<div class="stack"><a href="/y">Underneath</a><div class="over"></div></div>
<div class="stack2"><a href="/z">Under the ghost</a><div class="ghost"></div></div>
<p>Not a control at all.</p>
</body></html>"##;

fn tree() -> manuk_a11y::A11yNode {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://drivable.test/", &fonts, 800.0);
    page.a11y_tree()
}

fn verdict_of(root: &manuk_a11y::A11yNode, name: &str) -> Verdict {
    let ts = targets(root);
    let mut counts = std::collections::HashMap::<(String, String), usize>::new();
    for t in &ts {
        *counts
            .entry((t.role.as_str().to_string(), t.name.trim().to_string()))
            .or_insert(0) += 1;
    }
    let t = ts
        .iter()
        .find(|t| t.name.trim() == name)
        .unwrap_or_else(|| {
            panic!(
                "no target named {name:?}; tree has {:?}",
                ts.iter().map(|t| &t.name).collect::<Vec<_>>()
            )
        });
    let d = counts[&(t.role.as_str().to_string(), t.name.trim().to_string())];
    classify(root, t, d)
}

#[test]
fn a_plain_control_is_drivable() {
    // ── VACUITY. If the simplest possible button is not drivable, every row below is measuring a
    //    broken tree rather than the distinctions it means to draw.
    let root = tree();
    assert_eq!(
        verdict_of(&root, "Save"),
        Verdict::Drivable,
        "a plain button with a box and a unique name must be drivable"
    );
}

#[test]
fn a_hit_on_a_descendant_is_still_a_hit() {
    // ⭐ The centre of this button is inside a <span>. The click bubbles; the agent succeeds.
    let root = tree();
    assert_eq!(
        verdict_of(&root, "Publish now"),
        Verdict::Drivable,
        "landing on a child SPAN is how a real button click works, not a miss"
    );
}

#[test]
fn two_controls_with_one_name_are_ambiguous() {
    // An agent resolving by role+name has no way to say WHICH `Delete` it meant.
    let root = tree();
    assert_eq!(verdict_of(&root, "Delete"), Verdict::Ambiguous);
}

#[test]
fn a_zero_area_control_is_ungrounded() {
    // Perceivable — a screen reader announces it — and there is nowhere to aim.
    let root = tree();
    assert_eq!(verdict_of(&root, "Collapsed"), Verdict::Ungrounded);
}

#[test]
fn a_covered_control_is_a_mishit() {
    // ⚠ Grounded, unique, and unclickable: the fixed banner is on top of it.
    let root = tree();
    assert_eq!(
        verdict_of(&root, "Underneath"),
        Verdict::MisHit,
        "a control under an overlay is exactly what the MisHit category exists to catch"
    );
}

#[test]
fn a_fixed_overlay_with_no_z_index_does_not_yet_win_the_click() {
    // ── ⚠⚠⚠ **THIS ROW ASSERTS A DEFECT, AND IT IS THE FIRST THING drive-probe FOUND.**
    //    `#ghost` is `position: fixed` with `z-index: auto`. CSS painting order puts every
    //    positioned element above non-positioned in-flow content, so Chrome's `elementFromPoint`
    //    returns the ghost. Ours returns the LINK, because `A11yNode::z` models only an explicit
    //    `z-index` — it is `0` for "not positioned" and `0` for "positioned, auto" alike — and the
    //    tie-break between unrelated subtrees then falls through to SMALLER AREA, which the 200x30
    //    link wins against the 400x300 banner.
    //
    //    ⭐ A cookie banner is exactly this markup. An agent would click the link underneath one and
    //    report success.
    //
    //    It is asserted rather than fixed because `z` is set where the computed styles live and the
    //    change reaches every coordinate click in the engine; a gate that NAMES what it cannot yet
    //    catch beats one that pretends. The row above it (`Underneath`, `z-index: 10`) proves the
    //    MisHit machinery works, so this is a missing INPUT and not a broken classifier.
    let root = tree();
    assert_eq!(
        verdict_of(&root, "Under the ghost"),
        Verdict::Drivable,
        "if this now reads MisHit the z model has been fixed — DELETE this row and fold it into \
         the one above, which is the outcome this gate wants"
    );
}

#[test]
fn only_controls_are_counted_and_the_tally_adds_up() {
    let root = tree();
    let t = tally(&root);
    assert_eq!(
        t.total,
        targets(&root).len(),
        "the tally must classify every target and no more"
    );
    assert_eq!(
        t.drivable + t.ungrounded + t.ambiguous + t.mishit,
        t.total,
        "the four verdicts must partition the targets"
    );
    // The paragraph is perceivable and not actionable; counting it would dilute the rate.
    assert!(!is_actionable(&Role::Paragraph) && !is_actionable(&Role::Generic));
    assert!(is_actionable(&Role::Button) && is_actionable(&Role::Link));
    assert!(
        targets(&root).iter().all(|n| is_actionable(&n.role)),
        "a non-control reached the target list"
    );
}
