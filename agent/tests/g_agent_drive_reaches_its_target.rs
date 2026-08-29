//! **G_AGENT_DRIVE_REACHES_ITS_TARGET — the agent's own drive path never asked whether a pointer
//! could reach the thing it clicked.**
//!
//! `AgentBrowser::click_by_name` — the production entry point behind `Action::ClickText`, the one an
//! agent loop actually drives — resolved a name to a `NodeId` and called `activate`, which fires the
//! element's activation behaviour **structurally**: follow the `href`, submit the form, flip the
//! checkbox. It never asked the question a real pointer has to answer.
//!
//! ⭐⭐⭐ **SO THE AGENT SUCCEEDED WHERE A USER COULD NOT, IN TWO DIFFERENT WAYS.**
//!
//! * A target **below the fold** was activated with the viewport never moving to it. Every
//!   screenshot and every subsequent `observe()` then showed a part of the page the agent had not
//!   acted on — the perception channel and the actuation channel describing different documents.
//! * A target **under a consent banner** — which is most of the web — reported `Navigated(..)` for a
//!   click no user could have made. `to_viewport_lines` had been printing `obstructed` beside that
//!   very element since t1356, and the drive path did not read its own warning.
//!
//! t1356 built the verification (`A11yNode::landing`), t1359 gave it the off-screen answer
//! (`Landing::OffScreen`), and t1356's own doc **recorded this hole rather than closing it**:
//!
//! > *"A caller that holds a node handle may still activate the node directly
//! > (`Browser::click_by_name` does) — that path is unchanged."*
//!
//! This gate is that path, closed. The rule it enforces is the one already in the wiki index at
//! L62: *agent actions must go through the REAL hit-test, or agent testing is a privileged bypass.*
//!
//! ## The two answers, and why only one of them is an error
//!
//! ⭐ **`OffScreen` is not a refusal, it is a SCROLL.** The agent is driving a browser; the honest
//! response to *"the thing you named is 900px down"* is to go there. The scroll goes through
//! `scroll_by`, so it is clamped to the page exactly as a user's would be, and the landing is then
//! re-asked **in the viewport the click now happens in** — which matters because a `position:sticky`
//! header's document rect moves with the scroll (t1359).
//!
//! ⚠ **`Obstructed` IS refused, and that is the capability rather than a limitation.** An agent told
//! *"Sign in is covered by `generic "We use cookies"`"* can dismiss the banner and retry. An agent
//! handed a silent success clicks nothing, sees nothing change, and has no way to find out why.
//!
//! ⚠ `Unreachable` (no box, or on-screen `pointer-events: none`) is deliberately **not** an error:
//! an element the layout gave no geometry must not become unclickable for the agent, and the
//! structural activation is still the best available answer for it.
//!
//! PROVEN RED by two mutations — see the module tail.

use manuk_agent::AgentBrowser;

fn data_url(html: &str) -> String {
    format!("data:text/html,{html}")
}

/// A 2400px page: a control on screen, one 1400px down, and one buried under a banner.
///
/// ⚠ The targets are **checkboxes, not links**, and that is not incidental: `activate` on an `<a>`
/// performs a real navigation (it fetches the `href`), so a link fixture would make this gate a
/// network test. A checkbox activates locally and reports `Toggled`, so every arm below measures the
/// DRIVE and nothing else.
const HTML: &str = r#"<body style="margin:0"><div style="height:2400px">
  <input type="checkbox" id="near" aria-label="Near target" style="position:absolute;left:40px;top:100px;width:200px;height:60px;margin:0">
  <input type="checkbox" id="far" aria-label="Far target" style="position:absolute;left:40px;top:1400px;width:200px;height:60px;margin:0">
  <input type="checkbox" id="under" aria-label="Buried target" style="position:absolute;left:40px;top:1800px;width:200px;height:60px;margin:0">
  <div id="banner" style="position:absolute;left:0;top:1760px;width:700px;height:160px;z-index:60">We use cookies</div>
</div></body>"#;

#[tokio::test]
async fn the_drive_path_scrolls_to_its_target_and_refuses_a_covered_one() {
    let mut b = AgentBrowser::new(800, 600);
    b.navigate(&data_url(HTML)).await.unwrap();

    // ── VACUITY. Three named links with geometry, and a banner on a HIGHER layer covering one of
    // them. A page that laid none of this out would sail through every arm below.
    {
        let tree = b.a11y_tree().unwrap();
        let named: Vec<_> = tree
            .iter()
            .filter(|n| n.name.contains("target") && n.bbox.is_some())
            .collect();
        assert_eq!(
            named.len(),
            3,
            "VACUOUS: expected 3 boxed named checkboxes, got {named:#?}"
        );
        assert!(
            named
                .iter()
                .all(|n| n.state.checked == Some(manuk_a11y::Checked::False)),
            "VACUOUS: a target starts checked, so 'it toggled' proves nothing"
        );
        assert!(
            tree.iter().any(|n| n.z >= 60 && n.bbox.is_some()),
            "VACUOUS: the banner is not on a higher stacking layer, so nothing is covered"
        );
    }
    assert_eq!(
        b.scroll_offset(),
        0.0,
        "VACUOUS: the page did not start at the top, so 'it scrolled' proves nothing"
    );

    // ── ARM 1 · CONTROL — an on-screen target is unchanged: it activates, and the viewport does
    //    NOT move. This tick adds a reachability step; it must not turn every click into a scroll.
    let act = b
        .click_by_name(&manuk_a11y::Role::CheckBox, "Near target")
        .await
        .expect("an on-screen control must still activate");
    assert!(
        matches!(act, manuk_agent::Activation::Toggled(true)),
        "CONTROL: the on-screen checkbox must toggle, got {act:?}"
    );
    assert_eq!(
        b.scroll_offset(),
        0.0,
        "CONTROL: an on-screen target must not move the viewport — this tick adds a reachability \
         step, it must not turn every click into a scroll"
    );

    // ── ARM 2 · THE DRIVE REACHES BELOW THE FOLD — the load-bearing arm. The target is at y=1400
    //    in a 600px viewport, so a pointer cannot be on it until the page scrolls.
    let mut b = AgentBrowser::new(800, 600);
    b.navigate(&data_url(HTML)).await.unwrap();
    let act = b
        .click_by_name(&manuk_a11y::Role::CheckBox, "Far target")
        .await
        .expect("a below-the-fold control is reachable by scrolling, not an error");
    assert!(
        matches!(act, manuk_agent::Activation::Toggled(true)),
        "the below-the-fold checkbox must toggle, got {act:?}"
    );
    assert!(
        b.scroll_offset() > 600.0,
        "THE DRIVE DID NOT GO TO ITS TARGET: the target sits at y=1400 in a 600px viewport and the \
         browser is still at scroll {}. Activating it without scrolling leaves every screenshot and \
         every later observe() showing a part of the page the agent did not act on.",
        b.scroll_offset()
    );

    // ── ARM 3 · HONEST REFUSAL — a covered target names its cover instead of reporting success.
    let mut b = AgentBrowser::new(800, 600);
    b.navigate(&data_url(HTML)).await.unwrap();
    let err = b
        .click_by_name(&manuk_a11y::Role::CheckBox, "Buried target")
        .await
        .expect_err(
            "a link under a banner must be REFUSED, not silently activated — a click there would \
             hit the banner, and reporting Navigated for it is a lie the agent cannot retry",
        );
    let msg = format!("{err:#}");
    assert!(
        msg.contains("covered by"),
        "ARM 3: the refusal must NAME what to dismiss first, got {msg:?}"
    );
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  drop the `reach` call from `click_by_name` (the pre-tick behaviour)
//       -> ARM 2's `scroll_y` reads 0 (the click succeeded without going there) and ARM 3 returns
//          `Ok(Navigated("…/under"))` instead of an error. Both defects in one mutation, which is
//          what makes this one call the whole tick.
// N2  treat `Landing::OffScreen` as an error rather than a scroll
//       -> ARM 2 fails with an error where Chrome-like behaviour is to scroll and click. This is
//          the mutation that separates "verify before acting" from "act like a browser".
