//! **G_ACCESSKIT_TREE — the accessibility tree is finally in the shape something else can read.**
//!
//! ⭐⭐⭐ **THE TREE WAS ALWAYS RICHER THAN THE THING THAT COULD READ IT.** `manuk-a11y` has computed
//! roles, accessible names, interaction state and border boxes for a thousand ticks — and every one
//! of those facts was reachable only through Manuk's own types. **AccessKit is the shape a screen
//! reader, an OS accessibility bridge and every Rust a11y harness already speak**; it is what servo
//! emits. Adopting it is the difference between *"we have an accessibility tree"* and *"an assistive
//! technology can read this page"*.
//!
//! The board has named this Track B at **ten consecutive constitution checks** as the fastest
//! greenfield win and it had never been started. This is the first tick of it.
//!
//! ⚠ **A PROJECTION, NOT A SECOND SOURCE OF TRUTH.** Nothing in the bridge computes a role, a name or
//! a state — every field is read off the `A11yNode` the existing builder produced, so the two cannot
//! disagree. *One rule, one implementation* is this project's most-repeated lesson, and an
//! accessibility tree computed twice would be the largest possible instance of it.
//!
//! What the fixture asserts, in AccessKit's own vocabulary:
//!
//! ```text
//!   <h2>Settings</h2>                       Heading  label "Settings"  level 2
//!   <button disabled>Save</button>          Button   label "Save"      is_disabled
//!   <input type=checkbox checked>           CheckBox                    toggled=True
//!   <div role=switch aria-checked=false>    Switch                      toggled=False
//!   <a href=#>Docs</a>                      Link     label "Docs"      bounds present
//!   <div role=tab aria-selected=true>       Tab                         is_selected
//! ```
//!
//! ⭐ **The node IDs are the ARENA's own**, which is what makes this useful to an agent and not only
//! to a screen reader: an AccessKit consumer that reports a node can be taken straight back to the
//! DOM element it came from.
//!
//! ⚠ NAMED, because the projection loses it: AccessKit has no `subscript`/`superscript` ROLE — it
//! carries them as a `vertical_offset` property this bridge does not yet set, so both land on
//! `GenericContainer`. The distinction survives in Manuk's own tree and is lost here.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8></head><body>
<h2>Settings</h2>
<button id=save disabled>Save</button>
<input id=cb type=checkbox checked aria-label="Remember me">
<div id=sw role=switch aria-checked="false" aria-label="Dark mode"></div>
<a id=docs href="#x">Docs</a>
<div id=tb role=tab aria-selected="true" aria-label="Home"></div>
</body></html>"##;

#[test]
fn the_a11y_tree_projects_into_accesskit() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ak.test/", &fonts, 800.0);
    let tree = page.a11y_tree();
    let update = manuk_a11y::accesskit_bridge::tree_update(&tree);

    // ── VACUITY. A projection of an empty tree passes every assertion below by having nothing to
    //    contradict, so the node count is checked first and against a floor the fixture guarantees.
    assert!(
        update.nodes.len() >= 7,
        "VACUOUS: the projection produced {} nodes for a six-widget document — there is nothing \
         here for the rows below to be wrong about",
        update.nodes.len()
    );
    assert!(
        update.tree.is_some(),
        "a TreeUpdate without `tree` cannot INITIALISE a consumer — AccessKit requires it on the \
         first update, and a projection that can only ever be an increment is not a tree"
    );

    let find = |want: accesskit::Role, label: &str| -> Option<&accesskit::Node> {
        update
            .nodes
            .iter()
            .map(|(_, n)| n)
            .find(|n| n.role() == want && n.label().as_deref() == Some(label))
    };
    let role_only = |want: accesskit::Role| -> Option<&accesskit::Node> {
        update
            .nodes
            .iter()
            .map(|(_, n)| n)
            .find(|n| n.role() == want)
    };

    // ── ROLE + NAME, the two facts an assistive technology cannot work without.
    let h = find(accesskit::Role::Heading, "Settings")
        .expect("a Heading labelled \"Settings\" — role AND accessible name must both survive");
    assert_eq!(
        h.level(),
        Some(2),
        "⭐ A HEADING'S LEVEL IS A SEPARATE PROPERTY IN ACCESSKIT — `Role::Heading` is level-less \
         there, so dropping it announces every heading on the page as an <h1>."
    );

    let save = find(accesskit::Role::Button, "Save").expect("a Button labelled \"Save\"");
    assert!(
        save.is_disabled(),
        "⭐ DISABLED IS THE STATE AN AGENT NEEDS MOST: one that clicks a disabled button waits \
         forever for a result that is never coming."
    );

    // ── STATE. The whole point of the tree for an agent is that it can observe its own action.
    let cb = find(accesskit::Role::CheckBox, "Remember me").expect("a CheckBox");
    assert_eq!(
        cb.toggled(),
        Some(accesskit::Toggled::True),
        "a checked checkbox must project as Toggled::True — without it the tree reads identically \
         before and after a click"
    );
    let sw = find(accesskit::Role::Switch, "Dark mode").expect("a Switch");
    assert_eq!(
        sw.toggled(),
        Some(accesskit::Toggled::False),
        "⚠ FALSE IS NOT ABSENT. `aria-checked=\"false\"` must project as Toggled::False, not as no \
         toggle at all — 'this switch is off' and 'this is not a switch' are different facts."
    );
    let tab = role_only(accesskit::Role::Tab).expect("a Tab");
    assert!(
        tab.is_selected().unwrap_or(false),
        "`aria-selected` must survive the projection — it is how an agent knows which tab it is on"
    );

    // ── GEOMETRY. An agent grounds a click on the bounds; a screen reader draws its focus ring there.
    let docs = find(accesskit::Role::Link, "Docs").expect("a Link labelled \"Docs\"");
    let b = docs.bounds().expect(
        "⭐ BOUNDS ARE WHAT MAKE THIS TREE ACTIONABLE — a node an agent cannot point at is a \
                 node it can only read",
    );
    assert!(
        b.x1 > b.x0 && b.y1 > b.y0,
        "the link's bounds must be a real rectangle, got {b:?}"
    );

    // ── IDENTITY. The ids are the arena's, so a consumer can walk back to the DOM.
    let root_id = update.tree.as_ref().unwrap().root;
    assert!(
        update.nodes.iter().any(|(id, _)| *id == root_id),
        "the root named by `tree` must itself be present in `nodes` — AccessKit rejects an update \
         whose root it cannot find, so this is the difference between a tree and a list"
    );
    for (id, n) in &update.nodes {
        for c in n.children() {
            assert!(
                update.nodes.iter().any(|(i, _)| i == c),
                "node {id:?} names child {c:?}, which is not in the update — AccessKit's contract is \
                 that every child is either already in the tree or in this same list, and a \
                 projection that emits a dangling id panics the consumer rather than degrading"
            );
        }
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// A1  drop the `set_label` call
//       -> every `find` by name fails; the role-only Tab row still passes, which is what separates
//          "the projection is empty" from "the names are missing".
// A2  drop the heading `set_level`
//       -> the level row alone; every other assertion holds, because level is the one fact
//          AccessKit's `Role` cannot carry.
// A3  map `Checked::False` to no toggle at all (`Some(Checked::False) => {}`)
//       -> the Switch row: "off" becomes indistinguishable from "not a switch".
//       ⭐ Deleting the ARM outright does not compile — the match over `Option<Checked>` is
//          exhaustive, so the type system already forbids half of this mutation. That is a stronger
//          guarantee than a red test and is worth saying: the gate covers the case the compiler
//          cannot, which is mapping the arm to the WRONG thing rather than forgetting it.
// A4  drop `set_bounds`
//       -> the Link row; the tree still reads correctly and is no longer actionable, which is the
//          precise failure this project would otherwise ship without noticing.
// A5  emit children ids without emitting the children
//       -> the dangling-id walk at the end; AccessKit's own consumer would panic on it, so the gate
//          asserts the contract rather than discovering it in a screen reader.
