//! **G_AGENT_ACTIVATION_BEHAVIOUR — the agent's click was a SECOND, WRONG implementation of a rule
//! the engine already had right.**
//!
//! `manuk-page`'s `Page::dispatch_click` *is* the click's activation behaviour, built and
//! Chrome-arbitrated over many ticks: `<label>` forwarding, the pre-click activation steps,
//! radio-group exclusivity, a disabled control refusing, a submit queued as *requested* so the
//! page's own validator runs first, the `<summary>`/`<details>` disclosure with `name` accordion
//! exclusivity, `mousedown`/`mouseup`/`click`/`input`/`change`, and the cancelled-activation undo.
//!
//! **`manuk-agent` — the consumer this project exists to serve — called none of it.** It re-derived
//! the rule from a twenty-line `match` on the tag name. Every row below is headless Chrome
//! (145.0.7632.116) answering the same question, and every one of them disagreed with us:
//!
//! ```text
//!   d1.open after click on summary            = true      we: Inert, the section never opens
//!   cb1.checked after label[for] click        = true      we: Inert
//!   cb2.checked after wrapping-label click    = true      we: Inert
//!   after click rb: ra/rb/rc checked          = f/t/f     we: t/t/f   ← TWO radios checked
//!   after SECOND click rb                     = f/t/f     we: f/f/f   ← radios do not untoggle
//!   disabled checkbox checked after click     = false     we: true
//!   form submitted by disabled button         = false     we: submitted
//!   click <span> inside <button> submits      = true      we: nothing
//!   click <span> inside <a href> navigates    = #dest     we: nothing
//! ```
//!
//! ⭐⭐⭐ **AND NOT ONE OF THEM FAILED LOUDLY.** `Toggled(true)` came back for a *disabled*
//! checkbox — so an agent reads its own success out of a form the server will reject — and `Inert`
//! came back for the disclosure, which is indistinguishable from *"this page has nothing to open"*.
//! A retry loop cannot recover from either.
//!
//! ## What this gate is, and what it deliberately is not
//!
//! ⭐ **The observable is the ACCESSIBILITY TREE**, which is what makes this the drive loop closing
//! and not a unit test of a helper: the agent reads role + name + `checked` out of the tree, acts,
//! and reads the result back out of the *same* tree. **There is no script on the page at all** —
//! every behaviour asserted here is pure UA activation behaviour, so it holds in a build without
//! SpiderMonkey (which `manuk-agent`'s is), and a green result cannot be an artefact of a test
//! poking the DOM.
//!
//! ⚠ **So the event half is NOT claimed here.** `dispatch_click` also fires
//! `mousedown`/`mouseup`/`click`/`input`/`change` and honours `preventDefault()`; the agent now
//! inherits that by construction, but `manuk-agent` builds `manuk-page` without the `spidermonkey`
//! feature, so no page script runs and this gate can prove none of it. Recorded, not asserted.
//!
//! PROVEN RED by the mutations in the module tail.

use manuk_a11y::{Checked, Role};
use manuk_agent::{Activation, AgentBrowser};

/// One page, no script. Every construct here is priced on the CrUX sample (36 fetched pages):
/// `<button>` on 12 sites, `<label>` on 6, `disabled` on 8, `<details>`/`<summary>` on 3 (51 hits).
const HTML: &str = r#"<body>
<details id=d1><summary id=s1 style="display:block;width:200px;height:40px">Show more</summary><p>the disclosed secret</p></details>

<label id=lab1 for=cb1>Accept terms</label><input type=checkbox id=cb1>
<label id=lab2><span id=sp2 style="display:inline-block;width:120px;height:20px">Remember me</span><input type=checkbox id=cb2></label>

<input type=radio name=r value=a id=ra checked aria-label="Radio A">
<input type=radio name=r value=b id=rb aria-label="Radio B">
<input type=radio name=r value=c id=rc aria-label="Radio C">

<input type=checkbox id=cbd disabled aria-label="Disabled box">
<form id=fd action="never.html"><button id=bd disabled style="display:block;width:200px;height:40px"><span style="display:inline-block;width:120px;height:20px">Disabled submit</span></button></form>

<input type=checkbox id=cbok aria-label="Ordinary box">
<button id=formless>Menu</button>
</body>"#;

fn data_url(html: &str) -> String {
    format!("data:text/html,{html}")
}

/// Read checkedness **back out of the accessibility tree** — the agent's own perception channel,
/// not a DOM back door. This is the "observe" leg of perceive → act → observe.
fn checked(b: &AgentBrowser, name: &str) -> Checked {
    b.a11y_tree()
        .expect("a page is loaded")
        .iter()
        .find(|n| n.name == name)
        .unwrap_or_else(|| panic!("VACUOUS: no node named {name:?} in the a11y tree"))
        .state
        .checked
        .unwrap_or_else(|| panic!("VACUOUS: {name:?} exposes no checked state to observe"))
}

/// The `<details>`'s own disclosure state, as the agent perceives it (`A11yState::expanded`).
fn disclosure(b: &AgentBrowser) -> Option<bool> {
    b.a11y_tree()
        .expect("a page is loaded")
        .iter()
        .find(|n| n.role == Role::Group && n.state.expanded.is_some())
        .and_then(|n| n.state.expanded)
}

/// Does the agent's OBSERVATION carry this text? A closed `<details>` lays out only its summary,
/// so the panel's content is absent from the perception channel until the disclosure opens.
fn observation_mentions(b: &AgentBrowser, needle: &str) -> bool {
    b.observe().expect("a page is loaded").text.contains(needle)
}

/// The named control's box, as the agent perceives it.
fn bbox(b: &AgentBrowser, name: &str) -> manuk_a11y::Rect {
    b.a11y_tree()
        .expect("a page is loaded")
        .iter()
        .find(|n| n.name == name)
        .and_then(|n| n.bbox)
        .unwrap_or_else(|| panic!("VACUOUS: {name:?} has no geometry to aim at"))
}

/// The centre of a box **inside** the named control — where a pointer actually lands when the
/// control's label is markup (`<button><span>Sign in</span></button>`, which is how every
/// icon-or-translated-label button on the web is written). Derived from perceived geometry, so no
/// coordinate is hard-coded.
fn inner_point(b: &AgentBrowser, name: &str) -> (f32, f32) {
    let tree = b.a11y_tree().expect("a page is loaded");
    let outer = tree
        .iter()
        .find(|n| n.name == name)
        .and_then(|n| n.bbox)
        .unwrap_or_else(|| panic!("VACUOUS: {name:?} has no geometry"));
    let inner = tree
        .iter()
        .filter(|n| n.name != name)
        .filter_map(|n| n.bbox)
        .find(|r| {
            r.x >= outer.x
                && r.y >= outer.y
                && r.x + r.width <= outer.x + outer.width
                && r.y + r.height <= outer.y + outer.height
                && r.width < outer.width
        })
        .unwrap_or_else(|| {
            panic!(
                "VACUOUS: {name:?} has no INNER box, so a click on it cannot \
             land on descendant markup and the ancestor-walk arm proves nothing"
            )
        });
    (inner.x + inner.width / 2.0, inner.y + inner.height / 2.0)
}

/// What the occlusion-aware hit-test says is at a point — the vacuity check for a coordinate arm.
fn hit_role_at(b: &AgentBrowser, x: f32, y: f32) -> Option<Role> {
    b.a11y_tree()
        .expect("a page is loaded")
        .hit_test(x, y)
        .map(|n| n.role.clone())
}

/// Activate the node the tree names EXACTLY, through `AgentBrowser::activate` — the production
/// entry point behind `BrowserAction::ClickHandle` and, via `resolve`, behind every `Click`.
///
/// ⚠ **It does NOT go through `resolve_handle`, and that is a defect this tick MEASURED rather than
/// fixed.** `targeting::keywords` drops tokens shorter than two characters (`targeting.rs:24`), so
/// the distinguishing token of `"Radio B"` disappears and all three radios score *identically*:
///
/// ```text
///   INTENT "Radio A" -> "Radio C"   score 0.73571503
///   INTENT "Radio B" -> "Radio C"   score 0.73571503
///   INTENT "Radio C" -> "Radio C"   score 0.73571503
///   INTENT "Sign in" -> "Sign in"   INTENT "Delete" -> "Delete"     (>=2-char tokens are fine)
/// ```
///
/// An exact, complete, unambiguous name match does not win, and the tie falls to tree order — so an
/// agent told *"select Option A"* selects Option C. Single-character tokens are pagination
/// (`Page 2`), steps (`Step 1`), sizes (`S`/`M`/`L`) and seat/option letters. **That is the next
/// tick**; resolving here through the tree's own `node` handle keeps THIS gate measuring activation
/// behaviour rather than the scorer.
async fn click(b: &mut AgentBrowser, role: Role, name: &str) -> Activation {
    let node = b
        .a11y_tree()
        .expect("a page is loaded")
        .iter()
        .find(|n| n.role == role && n.name == name)
        .map(|n| n.node)
        .unwrap_or_else(|| panic!("VACUOUS: the tree has no {role:?} named exactly {name:?}"));
    b.activate(node)
        .await
        .unwrap_or_else(|e| panic!("{role:?} {name:?} activation errored: {e}"))
}

#[tokio::test]
async fn the_agents_click_performs_the_platforms_activation_behaviour() {
    let mut b = AgentBrowser::new(800, 900);
    b.navigate(&data_url(HTML)).await.unwrap();

    // ── VACUITY. The page must actually expose the controls, unchecked, with Radio A pre-selected
    // (so "the group became exclusive" is a real change and not the initial state).
    assert_eq!(
        checked(&b, "Accept terms"),
        Checked::False,
        "VACUOUS: the label's control starts checked"
    );
    assert_eq!(
        checked(&b, "Radio A"),
        Checked::True,
        "VACUOUS: no radio is pre-selected, so \
         exclusivity cannot be observed"
    );
    assert_eq!(
        checked(&b, "Radio B"),
        Checked::False,
        "VACUOUS: Radio B starts selected"
    );
    assert_eq!(
        checked(&b, "Disabled box"),
        Checked::False,
        "VACUOUS: the disabled box starts checked"
    );
    assert_eq!(
        disclosure(&b),
        Some(false),
        "VACUOUS: the <details> does not start CLOSED, so opening it proves nothing"
    );
    assert!(
        !observation_mentions(&b, "the disclosed secret"),
        "VACUOUS: a CLOSED <details> already exposes its panel, so opening it proves nothing"
    );

    // ── ARM 1 · DISCLOSURE. `<summary>` has no accessible name in our tree yet (measured this
    // tick: it is `Generic ""`), so the agent reaches it the way the model is told to — by the
    // COORDINATE `to_viewport_lines` publishes, through the real occlusion-aware hit-test.
    // Chrome: `d1.open after click on summary = true`.
    let act = b
        .click_at(100.0, 20.0)
        .await
        .expect("the summary is hit-testable");
    assert!(
        matches!(act, Activation::Disclosed(true)),
        "clicking a <summary> must OPEN its <details> and say so; got {act:?}. This is the web's \
         standard 'show more' — every docs FAQ and folded diff — and it carries no script, so \
         `Inert` was not just wrong, it was unfalsifiable from outside"
    );
    assert_eq!(
        disclosure(&b),
        Some(true),
        "THE DRIVE LOOP DID NOT CLOSE: the agent reported the disclosure opened and its own \
         perception channel still reports it collapsed"
    );
    assert!(
        observation_mentions(&b, "the disclosed secret"),
        "the panel opened and its CONTENT is still absent from the observation the model reads — \
         an agent cannot act on what it cannot see"
    );

    // ── ARM 2 · A `<label>` FORWARDS TO ITS CONTROL. The visible target on most forms is the
    // text, not the 12px box — and the label is `Generic ""` in our tree (measured this tick, the
    // same gap as `<summary>`), so the agent reaches it the only way it can: by COORDINATE, which
    // is also the way the model is told to act. The point is DERIVED from the control's own
    // perceived geometry — the label text sits immediately to its left on the same line.
    // Chrome: `cb1.checked after label[for] click = true`.
    let cb1 = bbox(&b, "Accept terms");
    let (lx, ly) = (cb1.x - 20.0, cb1.y + cb1.height / 2.0);
    assert!(
        matches!(hit_role_at(&b, lx, ly), Some(Role::Generic)),
        "VACUOUS: ({lx},{ly}) is not on the label — the hit is {:?}, so this arm would be \
         measuring something else",
        hit_role_at(&b, lx, ly)
    );
    let act = b
        .click_at(lx, ly)
        .await
        .expect("a label click cannot error");
    assert!(
        matches!(act, Activation::Toggled(true)),
        "a click on <label for=cb1> must tick cb1 and say so; got {act:?}"
    );
    assert_eq!(
        checked(&b, "Accept terms"),
        Checked::True,
        "the label's control was not ticked — clicking 'Accept terms' on a checkout page does \
         nothing"
    );

    // ── ARM 3 · AND THE FORWARDING WALKS UP. A pointer lands on the `<span>` inside the label,
    // never on the label box itself, and `labeled_control` used to require an exact match — so
    // `<label><span>Remember me</span><input></label>` was inert while the identical page with a
    // bare text child worked. Chrome: `cb2.checked after wrapping-label click = true`.
    let cb2 = bbox(&b, "Remember me");
    let (sx, sy) = (cb2.x - 60.0, cb2.y + cb2.height / 2.0);
    assert!(
        matches!(hit_role_at(&b, sx, sy), Some(Role::Generic)),
        "VACUOUS: ({sx},{sy}) is not on the label's inner span"
    );
    let act = b.click_at(sx, sy).await.expect("a span click cannot error");
    assert!(
        matches!(act, Activation::Toggled(true)),
        "a click on markup INSIDE a wrapping <label> must tick its control; got {act:?}"
    );
    assert_eq!(
        checked(&b, "Remember me"),
        Checked::True,
        "the wrapping label's control was not ticked"
    );

    // ── ARM 4 · A RADIO IS A GROUP, NOT A TOGGLE. Chrome: `f/t/f`, and `f/t/f` again on a second
    // click. We used to answer `t/t/f` then `f/f/f` — two selected options, then none.
    let act = click(&mut b, Role::Radio, "Radio B").await;
    assert!(
        matches!(act, Activation::Toggled(true)),
        "selecting Radio B must report it selected; got {act:?}"
    );
    assert_eq!(
        (
            checked(&b, "Radio A"),
            checked(&b, "Radio B"),
            checked(&b, "Radio C")
        ),
        (Checked::False, Checked::True, Checked::False),
        "selecting a radio must DESELECT its group — two checked radios is a form that submits the \
         wrong value with nothing reporting a problem"
    );
    let act = click(&mut b, Role::Radio, "Radio B").await;
    assert!(
        matches!(act, Activation::Inert),
        "a second click on a SELECTED radio changes nothing, so the honest report is Inert, not a \
         toggle; got {act:?}"
    );
    assert_eq!(
        checked(&b, "Radio B"),
        Checked::True,
        "a radio must never UNCHECK on a second click — Chrome: f/t/f twice"
    );

    // ── ARM 5 · A DISABLED CONTROL IS INERT — the arm that matters most for an agent, because the
    // old answer was a SUCCESS. Chrome: `disabled checkbox checked after click = false`.
    let act = click(&mut b, Role::CheckBox, "Disabled box").await;
    assert!(
        matches!(act, Activation::Inert),
        "a disabled checkbox must not activate, and must not report that it did; got {act:?}. The \
         old `Toggled(true)` is how an agent ticks a disabled consent box, reads it back ticked, \
         and reports success on a form the server will reject"
    );
    assert_eq!(
        checked(&b, "Disabled box"),
        Checked::False,
        "the disabled box was ticked"
    );

    // ── ARM 6 · A DISABLED SUBMIT BUTTON DOES NOT SUBMIT. Chrome: `form submitted by disabled
    // button = false`. `never.html` does not exist, so a submission here would ALSO be a load
    // error — the assertion is doubly falsifiable.
    let before = b.current_url().map(str::to_string);
    let act = click(&mut b, Role::Button, "Disabled submit").await;
    assert!(
        matches!(act, Activation::Inert),
        "a disabled submit button must not submit its form; got {act:?}"
    );
    assert_eq!(
        b.current_url().map(str::to_string),
        before,
        "the disabled button navigated"
    );

    // ── ARM 6b · AND DISABLEDNESS IS THE SUBMITTER'S QUESTION, NOT THE HIT NODE'S. The pointer
    // lands on the `<span>` inside the button, and `is_disabled` on a `<span>` inside
    // `<button disabled>` is FALSE — only the `<fieldset>` propagates. Asking the hit node would
    // make every disabled icon-button live again, which is the same class of silent success as
    // ARM 5.
    let (dx, dy) = inner_point(&b, "Disabled submit");
    assert!(
        matches!(hit_role_at(&b, dx, dy), Some(Role::Generic)),
        "VACUOUS: ({dx},{dy}) is not on markup inside the button — hit is {:?}",
        hit_role_at(&b, dx, dy)
    );
    let act = b.click_at(dx, dy).await.expect("the click cannot error");
    assert!(
        matches!(act, Activation::Inert),
        "a click on markup inside a DISABLED submit button must not submit; got {act:?}"
    );
    assert_eq!(
        b.current_url().map(str::to_string),
        before,
        "the click inside the disabled button navigated"
    );

    // ── ARM 7 · A FORMLESS `<button>` IS INERT, NOT AN ERROR. Chrome: nothing observable happens.
    // We used to try to submit it and return `Err(NoForm)` / `Err(cannot resolve form action)` —
    // an error for the single commonest control on the modern web (a `<button>` with a JS handler
    // and no form), which an agent cannot tell from a real failure.
    let act = click(&mut b, Role::Button, "Menu").await;
    assert!(
        matches!(act, Activation::Inert),
        "a <button> outside any form must be Inert, not an error; got {act:?}"
    );

    // ── CONTROL. An ordinary checkbox still behaves exactly as it did before this tick. Without
    // this row every arm above could be satisfied by an `activate` that had stopped working.
    let act = click(&mut b, Role::CheckBox, "Ordinary box").await;
    assert!(
        matches!(act, Activation::Toggled(true)),
        "CONTROL: an ordinary checkbox must still toggle and report it; got {act:?}"
    );
    assert_eq!(
        checked(&b, "Ordinary box"),
        Checked::True,
        "CONTROL: it did not tick"
    );
}

/// The two arms that NAVIGATE, on their own `file://` fixture so the destinations resolve without a
/// network. Split out because a navigation replaces the page the arms above observe.
#[tokio::test]
async fn a_click_on_inner_markup_still_reaches_the_link_and_the_submit_button() {
    let dir = std::env::temp_dir().join(format!("manuk-g-activation-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("landed.html"),
        "<title>LANDED</title><body>arrived</body>",
    )
    .unwrap();
    std::fs::write(
        dir.join("submitted.html"),
        "<title>SUBMITTED</title><body>arrived</body>",
    )
    .unwrap();
    std::fs::write(
        dir.join("start.html"),
        r#"<title>START</title><body>
<a id=lnk href="landed.html" style="display:block;width:200px;height:40px"><span id=sp>Go there</span></a>
<form action="submitted.html"><button id=btn style="display:block;width:200px;height:40px"><span id=bsp>Send it</span></button></form>
</body>"#,
    )
    .unwrap();
    let start = format!("file://{}/start.html", dir.display());

    // ── ARM 8 · A CLICK ON THE `<span>` INSIDE AN `<a href>` FOLLOWS THE LINK. The activation
    // behaviour belongs to the nearest ANCESTOR that has one. Chrome: `location.hash = #dest`.
    //
    // ⚠ The inner span is `Generic ""` in our tree — it has no NAME to resolve by — so the arm
    // aims at its box, which is also the only way an agent could reach it.
    let mut b = AgentBrowser::new(800, 600);
    b.navigate(&start).await.unwrap();
    let (lx, ly) = inner_point(&b, "Go there");
    assert!(
        matches!(hit_role_at(&b, lx, ly), Some(Role::Generic)),
        "VACUOUS: ({lx},{ly}) hits {:?}, not markup inside the link — the ancestor walk is not \
         being exercised",
        hit_role_at(&b, lx, ly)
    );
    let act = b
        .click_at(lx, ly)
        .await
        .expect("the link click cannot error");
    assert!(
        matches!(&act, Activation::Navigated(u) if u.ends_with("landed.html")),
        "a click inside a link must follow it; got {act:?}"
    );
    assert_eq!(
        b.current_title(),
        Some("LANDED"),
        "the agent reported a navigation it did not perform"
    );

    // ── ARM 9 · AND A CLICK ON THE `<span>` INSIDE A SUBMIT `<button>` SUBMITS ITS FORM.
    // `<button><span>Sign in</span></button>` is how every icon-or-translated-label button is
    // written. Chrome: `click <span> inside <button> submits form = true`. We fired nothing.
    let mut b = AgentBrowser::new(800, 600);
    b.navigate(&start).await.unwrap();
    let (sx, sy) = inner_point(&b, "Send it");
    assert!(
        matches!(hit_role_at(&b, sx, sy), Some(Role::Generic)),
        "VACUOUS: ({sx},{sy}) hits {:?}, not markup inside the button",
        hit_role_at(&b, sx, sy)
    );
    let act = b
        .click_at(sx, sy)
        .await
        .expect("the submit click cannot error");
    assert!(
        matches!(&act, Activation::Submitted(u) if u.contains("submitted.html")),
        "a click inside a submit <button> must submit its form; got {act:?}"
    );
    assert_eq!(
        b.current_title(),
        Some("SUBMITTED"),
        "the agent reported a submission it did not perform"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ── PROVEN RED BY MUTATION ────────────────────────────────────────────────────────────────────
//
// M1  `activate` restored to its own tag-name `match` (the defect itself)
//       -> ARM 1 Inert, ARM 2/3 Inert, ARM 4 two radios checked then none, ARM 5 Toggled(true)
//          on the disabled box, ARM 6 submits, ARM 7 Err, ARMS 8/9 Inert.
// M2  `submit_target` reverted to an exact match on the clicked node (no ancestor walk)
//       -> ARM 9 Inert.
// M3  `submit_target`'s disabledness checked on the hit node instead of the submitter
//       -> ARM 6 submits (a `<span>` inside `<button disabled>` is not itself disabled).
// M4  `labeled_control` reverted to `el.name != "label" -> None` (no ancestor walk)
//       -> ARM 3 Inert, ARM 2 still green — which is exactly the shape that hid this bug:
//          the bare-text label worked and the wrapped-span label did not.
// M5  `Activation::Toggled` reported unconditionally rather than on an observed CHANGE
//       -> ARM 4's second click and ARM 5 both report Toggled.
