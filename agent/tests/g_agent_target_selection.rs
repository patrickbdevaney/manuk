//! **G_AGENT_TARGET_SELECTION — the production path picked the first substring match in tree
//! order, and the scorer built to do better had no production consumer.**
//!
//! `AgentBrowser::resolve` is the entry point behind every `click_by_name`, `type_into`,
//! `resolve_handle` and `submit`. It called `A11yNode::find_containing`, which is precisely *"the
//! first node in tree order whose name CONTAINS the needle"*:
//!
//! ```text
//!   a page with "Sign in with Google" ABOVE "Sign in"
//!     find_containing("Sign in")  ->  "Sign in with Google"    the first substring hit
//!     the dual scorer             ->  "Sign in"                 the exact-name bonus wins
//! ```
//!
//! ⭐⭐⭐ **An agent told to click *Sign in* clicking *Sign in with Google* is not a near miss — it
//! is a different account**, and on a consent page a different consequence entirely.
//!
//! Meanwhile `targeting::resolve_target` — semantic score + visual salience, with an exact-name
//! bonus and a confidence margin — had **no production consumer at all**: it was reachable only
//! through `ground_action`, which nothing outside a test called. Two halves of one system, built and
//! never joined. That is the t1356 shape (perception and actuation both built, nothing between
//! them), one layer up.
//!
//! ## ⚠ And the scorer never saw the ROLE either
//!
//! `Action::ClickText { role, name }` carries a role and `action_intent` dropped it, so the scorer
//! ranked by name and visual salience across every node on the page. `resolve` is always called
//! *with* a role — `type_into` passes `Role::TextBox` — so scoring without it means *"type into the
//! field called Search"* can score a BUTTON called Search. Filtering is applied **after** scoring so
//! the confidence margin is computed against the candidates that survive the role: a runner-up the
//! role excludes is not competition and must not make the winner look ambiguous.
//!
//! ## ⚠ A low-confidence winner is returned, not refused
//!
//! Ambiguity has a best answer. That is the difference from t1366's `Obstructed`, where acting is a
//! lie: two similar buttons still have a most-likely one, and refusing would turn every such page
//! into an error where the previous behaviour at least picked something. `Grounded::Ambiguous`
//! remains the surface for a caller that wants to disambiguate first.
//!
//! PROVEN RED by two mutations — see the module tail.

use manuk_agent::AgentBrowser;

fn data_url(html: &str) -> String {
    format!("data:text/html,{html}")
}

/// The substring trap in both directions, plus the role trap.
///
/// ⚠ `#decoy` is written FIRST on purpose: `find_containing` returns tree order, so a fixture with
/// the exact match first passes against both implementations and proves nothing.
const HTML: &str = r#"<body style="margin:0">
  <button id="decoy" style="position:absolute;left:0;top:0;width:200px;height:40px">Sign in with Google</button>
  <button id="exact" style="position:absolute;left:0;top:60px;width:200px;height:40px">Sign in</button>
  <button id="btn"   style="position:absolute;left:0;top:120px;width:200px;height:40px">Search</button>
  <input  id="field" type="text" aria-label="Search" style="position:absolute;left:0;top:180px;width:200px;height:40px">
  <button id="only"  style="position:absolute;left:0;top:240px;width:200px;height:40px">Unique Label</button>
</body>"#;

#[tokio::test]
async fn the_drive_path_picks_the_exact_match_and_the_right_role() {
    let mut b = AgentBrowser::new(800, 600);
    b.navigate(&data_url(HTML)).await.unwrap();

    // ── VACUITY. The decoy must come FIRST in the tree and must itself contain the needle, or the
    // substring trap is not set and ARM 1 passes against the old implementation.
    {
        let tree = b.a11y_tree().unwrap();
        let names: Vec<String> = tree
            .iter()
            .filter(|n| n.role == manuk_a11y::Role::Button)
            .map(|n| n.name.clone())
            .collect();
        assert_eq!(
            names.first().map(String::as_str),
            Some("Sign in with Google"),
            "VACUOUS: the decoy is not the first button in tree order, so `find_containing` would \
             already have returned the exact match and ARM 1 tests nothing. Got {names:?}"
        );
        assert!(
            names.iter().any(|n| n == "Sign in"),
            "VACUOUS: there is no exact-match button to prefer"
        );
    }

    // ── ARM 1 · EXACT BEATS AN EARLIER SUBSTRING. The needle is contained by the decoy, which comes
    //    first; only a scorer with an exact-name bonus gets this right.
    let exact = b
        .resolve_handle(&manuk_a11y::Role::Button, "Sign in")
        .expect("`Sign in` must resolve");
    let want = b.a11y_tree().unwrap();
    let want = want
        .iter()
        .find(|n| n.name == "Sign in")
        .expect("the exact button is in the tree")
        .node;
    assert_eq!(
        exact, want,
        "ARM 1: `Sign in` must resolve to the button named exactly that, not to the \
         `Sign in with Google` that precedes it in tree order — that is a different account."
    );

    // ── ARM 2 · THE ROLE SCOPES THE SEARCH. `Search` names a BUTTON (earlier) and a TEXT FIELD; a
    //    resolve for `TextBox` must find the field.
    let field = b
        .resolve_handle(&manuk_a11y::Role::TextBox, "Search")
        .expect("`Search` must resolve as a text box");
    let tree = b.a11y_tree().unwrap();
    let field_node = tree
        .iter()
        .find(|n| n.role == manuk_a11y::Role::TextBox)
        .expect("the text field is in the tree")
        .node;
    assert_eq!(
        field, field_node,
        "ARM 2: resolving `Search` as a TextBox must find the FIELD, not the earlier BUTTON of the \
         same name — `type_into` passes a role and the scorer has to honour it."
    );

    // ── ARM 3 · CONTROL — a unique name still resolves, and to itself. The change is a better
    //    CHOICE among candidates, not a new way to fail.
    let only = b
        .resolve_handle(&manuk_a11y::Role::Button, "Unique Label")
        .expect("CONTROL: an unambiguous name must still resolve");
    let only_node = tree
        .iter()
        .find(|n| n.name == "Unique Label")
        .expect("the unique button is in the tree")
        .node;
    assert_eq!(only, only_node, "CONTROL: a unique name resolves to itself");
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  `resolve` calls `find_containing` again (the pre-tick behaviour)
//       -> ARM 1 resolves to `Sign in with Google`. ARM 2 stays GREEN, because `find_containing`
//          does filter by role — which is exactly why ARM 2 needs its own mutation.
// N2  drop the role filter from `resolve_target_scoped`
//       -> ARM 2 resolves the BUTTON named Search instead of the field, because it scores higher on
//          nothing but tree position and salience. ARM 1 stays green.
