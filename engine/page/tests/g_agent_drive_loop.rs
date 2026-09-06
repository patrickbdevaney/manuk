//! **G_AGENT_DRIVE_LOOP — perceive, act, observe: the loop closed, end to end, for the first time.**
//!
//! ⭐⭐⭐ **EVERY PIECE OF THIS HAS BEEN BUILT AND GATED FOR A LONG TIME AND NOTHING HAD EVER COMPOSED
//! THEM.** Surface audit #87 named that plainly: Track C had zero ticks in ~37 while
//! `A11yTree::hit_test`, `Page::dispatch_click_at`, the accessibility tree and its AccessKit
//! projection all sat finished and separately green. *Assembly is exactly the work that stays undone,
//! because no single piece of it looks like a tick.*
//!
//! The M2 milestone is not "can it click" — it is **can an agent verify its own action**. That is a
//! round trip, and this gate is the round trip:
//!
//! ```text
//!   1. PERCEIVE  read the a11y tree; find the target by ROLE + ACCESSIBLE NAME
//!                — never by selector, id or DOM order, because an agent has none of those
//!   2. GROUND    take the node's own bounding box and aim at its CENTRE
//!   3. ACTUATE   dispatch a real click at that COORDINATE — through hit-testing, so the click
//!                lands on whatever is actually on top, exactly as a user's would
//!   4. OBSERVE   re-read the tree and confirm the state CHANGED
//! ```
//!
//! ⭐⭐ **STEP 4 IS THE ONE THAT MAKES THE OTHER THREE WORTH ANYTHING.** An agent that clicks and
//! cannot see the result either proceeds on faith or clicks again and undoes itself. The three state
//! rows below are `aria-pressed` on a toggle button, `checked` on a checkbox, and `expanded` on a
//! disclosure — the three shapes almost every real control on the web is built out of.
//!
//! ⚠ The negative row is the one that keeps this honest: a click aimed at a **disabled** button must
//! change nothing. Without it, "the state changed" could be satisfied by an engine that toggles
//! everything it is pointed at.
//!
//! ⚠ HERMETIC BY CONSTRUCTION, and the real-site half is named rather than claimed: the board asks
//! for this on ONE real site, and a gate cannot depend on the network. What this proves is that the
//! four components compose; what it does not prove is that a real page's markup is reachable this
//! way. `agent-run` is where that measurement belongs.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8><style>
body { margin: 0; font: 16px/1.4 monospace }
button, .row { display: block; margin: 8px; padding: 6px 10px; border: 1px solid #888 }
</style></head><body>
<button id=follow aria-pressed="false">Follow</button>
<button id=nope disabled aria-pressed="false">Archived</button>
<div id=chk class=row role="checkbox" aria-checked="false" tabindex="0">Remember me</div>
<div id=disc class=row role="button" aria-expanded="false">Details</div>
<div id=advisory class=row role="button" aria-disabled="true" aria-pressed="false">Advisory</div>
<script>
document.getElementById('follow').addEventListener('click', function () {
  var p = this.getAttribute('aria-pressed') === 'true';
  this.setAttribute('aria-pressed', p ? 'false' : 'true');
});
document.getElementById('nope').addEventListener('click', function () {
  this.setAttribute('aria-pressed', 'true');   // must never run: the button is disabled
});
document.getElementById('chk').addEventListener('click', function () {
  var c = this.getAttribute('aria-checked') === 'true';
  this.setAttribute('aria-checked', c ? 'false' : 'true');
});
document.getElementById('disc').addEventListener('click', function () {
  var e = this.getAttribute('aria-expanded') === 'true';
  this.setAttribute('aria-expanded', e ? 'false' : 'true');
});
document.getElementById('advisory').addEventListener('click', function () {
  this.setAttribute('aria-pressed', 'true');   // MUST run: aria-disabled is advisory only
});
</script>
</body></html>"##;

/// Walk the tree for the first node with this role and accessible name — the only lookup an agent
/// actually has.
fn find<'a>(
    n: &'a manuk_a11y::A11yNode,
    role: &manuk_a11y::Role,
    name: &str,
) -> Option<&'a manuk_a11y::A11yNode> {
    if &n.role == role && n.name == name {
        return Some(n);
    }
    n.children.iter().find_map(|c| find(c, role, name))
}

#[test]
fn an_agent_perceives_acts_and_observes_the_result() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://drive.test/", &fonts, 800.0);

    // ── 1. PERCEIVE. Everything below is found through the tree, by role and name.
    let tree = page.a11y_tree();
    let follow = find(&tree, &manuk_a11y::Role::Button, "Follow").expect(
        "VACUOUS: the agent cannot even SEE the button — perception fails before actuation",
    );
    assert_eq!(
        follow.state.pressed,
        Some(manuk_a11y::Checked::False),
        "the BEFORE state must be observable, or 'it changed' is unprovable"
    );

    // ── 2. GROUND. The node's own box, aimed at its centre.
    let b = follow.bbox.expect(
        "⭐ A NODE WITH NO BOX CANNOT BE CLICKED — grounding is what separates a readable \
                 tree from an actionable one",
    );
    let (cx, cy) = (b.x + b.width / 2.0, b.y + b.height / 2.0);
    assert!(
        b.width > 0.0 && b.height > 0.0,
        "the target's box must be real, got {b:?}"
    );

    // ── 3. ACTUATE, by COORDINATE — through hit-testing, exactly as a user's click resolves.
    let hit = page.dispatch_click_at(cx, cy, &fonts, 800.0);
    assert!(
        hit,
        "⭐⭐ THE CLICK MUST LAND. `dispatch_click_at` hit-tests the point and dispatches a real \
         event; a false here means the agent aimed at its own target and missed it, which is the \
         failure mode a coordinate-driven agent cannot recover from."
    );

    // ── 4. OBSERVE. Re-read the tree; the loop closes only if the state moved.
    let after = page.a11y_tree();
    let follow2 = find(&after, &manuk_a11y::Role::Button, "Follow").expect("still there");
    assert_eq!(
        follow2.state.pressed,
        Some(manuk_a11y::Checked::True),
        "⭐⭐⭐ THE ROUND TRIP. The agent found this control by name, clicked its centre, and read the \
         change back out of the same channel it perceived through. Without this row the other three \
         are a click into the dark."
    );

    // ── The other two control shapes, same loop.
    for (role, name, read) in [
        (
            manuk_a11y::Role::CheckBox,
            "Remember me",
            0, // 0 = checked, 1 = expanded
        ),
        (manuk_a11y::Role::Button, "Details", 1),
    ] {
        let t = page.a11y_tree();
        let node = find(&t, &role, name).unwrap_or_else(|| panic!("no {role:?} named {name:?}"));
        let bb = node.bbox.expect("a box to aim at");
        let before = if read == 0 {
            node.state.checked.map(|c| c == manuk_a11y::Checked::True)
        } else {
            node.state.expanded
        };
        assert_eq!(
            before,
            Some(false),
            "{name}: the BEFORE state must be observable and false"
        );
        assert!(page.dispatch_click_at(
            bb.x + bb.width / 2.0,
            bb.y + bb.height / 2.0,
            &fonts,
            800.0
        ));
        let t2 = page.a11y_tree();
        let node2 = find(&t2, &role, name).expect("still there");
        let now = if read == 0 {
            node2.state.checked.map(|c| c == manuk_a11y::Checked::True)
        } else {
            node2.state.expanded
        };
        assert_eq!(
            now,
            Some(true),
            "{name}: the state must change and be READABLE — `checked` and `expanded` are the other \
             two shapes nearly every real control is built out of"
        );
    }

    // ── THE NEGATIVE. A disabled control must not move, or "the state changed" means nothing.
    let t = page.a11y_tree();
    let nope = find(&t, &manuk_a11y::Role::Button, "Archived").expect("the disabled button");
    assert!(
        nope.state.disabled,
        "the tree must SAY it is disabled — that is how an agent knows not to wait for a result"
    );
    let bb = nope.bbox.expect("a box");
    page.dispatch_click_at(bb.x + bb.width / 2.0, bb.y + bb.height / 2.0, &fonts, 800.0);
    let t2 = page.a11y_tree();
    let nope2 = find(&t2, &manuk_a11y::Role::Button, "Archived").expect("still there");
    assert_eq!(
        nope2.state.pressed,
        Some(manuk_a11y::Checked::False),
        "⚠ A DISABLED BUTTON MUST NOT MOVE. Without this row, 'the state changed' could be satisfied \
         by an engine that toggles everything it is pointed at."
    );

    // ── AND THE CONTROL ON THE NEGATIVE. `aria-disabled` is ADVISORY: Chrome fires the handler.
    //    Suppressing both would be a different bug wearing the same fix.
    let t = page.a11y_tree();
    let adv = find(&t, &manuk_a11y::Role::Button, "Advisory").expect("the aria-disabled button");
    assert!(
        adv.state.disabled,
        "`aria-disabled` must still read as disabled in the TREE — that is the whole point of the \
         attribute; what it must not do is change what the DOM dispatches"
    );
    let bb = adv.bbox.expect("a box");
    page.dispatch_click_at(bb.x + bb.width / 2.0, bb.y + bb.height / 2.0, &fonts, 800.0);
    let t2 = page.a11y_tree();
    let adv2 = find(&t2, &manuk_a11y::Role::Button, "Advisory").expect("still there");
    assert_eq!(
        adv2.state.pressed,
        Some(manuk_a11y::Checked::True),
        "⭐⭐ `aria-disabled` IS ADVISORY AND ITS HANDLER STILL RUNS — Chrome-measured. A great many \
         real UIs use it precisely so their own handler can explain why the control is unavailable. \
         This row is what stops the native-disabled fix from becoming 'suppress anything that looks \
         disabled'."
    );
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// C1  make `dispatch_click_at` a no-op
//       -> every OBSERVE row; the perceive and ground rows stay green, which separates "the agent
//          cannot see" from "the agent cannot act".
// C2  drop `bbox` from the tree
//       -> the grounding row: a readable tree that is not an actionable one, and the distinction the
//          whole M2 milestone rests on.
// C3  stop dispatching the JS `click` event (dispatch geometry only)
//       -> the OBSERVE rows again but with the click LANDING — the shape where an agent believes it
//          acted and the page never heard.
// C4  let a NATIVELY disabled control receive the click
//       -> the `Archived` negative row alone.
// C5  suppress the click for `aria-disabled` too
//       -> the `Advisory` row alone — the mutation that turns a correct fix into a different bug.
// C6  drop `CheckBox`/`Radio` from `Role::name_from_content`
//       -> the checkbox loop cannot even FIND its target: the agent can see, ground and click a
//          control it has no way to refer to.
