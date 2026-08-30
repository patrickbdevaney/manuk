//! **G_AX_TREE_EXCLUDES_DISPLAY_NONE — a `display: none` subtree is not in the agent's a11y tree.**
//!
//! The second finding of surface audit #79's ranked #1 sweep, and it is the same shape as the
//! first (t1379) one level up: **nothing in the tree builder ever asked about `display`.**
//!
//! ```text
//!   is_hidden(dom, child)   reads the DOM     `hidden`, `aria-hidden`, <input type=hidden>,
//!                                             non-rendered tags
//!   the `invisible` SET     from the caller   computed `visibility` ONLY
//!   ─────────────────────────────────────────────────────────────────────────────────────
//!   `display: none`         asked by NOBODY
//! ```
//!
//! So a closed mobile menu, a `display: none` modal and a `<dialog>` without `open` were all in the
//! agent's tree as fully-formed, addressable nodes. Chrome-measured through CDP
//! `Accessibility.getFullAXTree` — its tree contains none of them.
//!
//! ⭐⭐⭐ **t1379 PRODUCED THE SYMPTOM THAT NAMES THIS BUG.** That tick made the NAME walk read the
//! computed `display`, and left this walk alone — so `<button style="display:none">Hidden</button>`
//! sat in the tree with an **EMPTY NAME**. *A node whose name is correctly computed as nothing is a
//! node that should not be there.* A half-fixed hiding rule is louder than an unfixed one, which is
//! the argument for finishing a sweep rather than taking its first finding.
//!
//! ## ⭐ THE ASYMMETRY WITH `visibility` IS THE RULE, AND IT IS WHY THIS IS NOT ONE MORE SET
//!
//! `visibility` INHERITS and is UNDOABLE, so the tree drops the node and KEEPS WALKING — a
//! `visibility: visible` descendant survives (row `m6`). `display` does not inherit and cannot be
//! undone: **a child of a `display: none` box computes its own ordinary `display`** (row `m1`'s link
//! computes `display: inline`, not `none`), so a per-node test never fires on the child. The prune
//! has to happen at the ancestor, by not descending.
//!
//! ## THE BATTERY — every row Chrome-measured through CDP
//!
//! ```text
//!                                                        chrome   before   after
//!   <a> inside <nav class=menu>  .menu{display:none}      absent   PRESENT  absent
//!   the same <a> in a visible <nav>             CONTROL   present  present  present
//!   <button style="display:none">Hidden inline</button>   absent   PRESENT  absent
//!   <button class=vis>          .vis{visibility:hidden}   absent   absent   absent
//!   <button>Deep hidden</button> two levels inside .menu  absent   PRESENT  absent
//!   <button style="visibility:visible"> inside .vis       present  present  present
//!   <button>Normal</button>                     CONTROL   present  present  present
//!   <button> inside a <dialog> with no `open`             absent   PRESENT  absent
//!   <select> and its two <option>s              CONTROL   present  present  present
//! ```
//!
//! ⚠ **THE `<summary>` ROW WAS DROPPED FROM THE ASSERTED SET AND THE REASON IS A DIFFERENT GAP.**
//! It was written as the control for *"a closed `<details>` is still exposed"* and it fired: our
//! tree gives `<summary>` role `Generic` with an EMPTY NAME, where Chrome says
//! `DisclosureTriangle` / `"More"`. That is a role-mapping gap, not a hiding one — asserting it
//! here would make this gate fail for a reason it is not about, and asserting our answer would pin
//! it. Measured, recorded, left to its own tick.
//!
//! ⭐ **THE ROWS ARE ASSERTED AS WHOLE NAME SETS PER ROLE, not as per-node presence, and that is
//! forced by the subject.** `AgentBrowser` exposes the a11y tree and no DOM handle — which is the
//! agent's real view — so a node is identified the way the agent identifies one: by role and name.
//! Set equality is also the stronger claim: `Buttons == ["Normal", "Re-shown"]` rejects an EXTRA
//! button as well as a missing one, and the extra is what this tick is about.
//!
//! ⭐⭐ **`Links == ["Sign in"]` — ONE, NOT TWO — is the sharpest row.** Both `<a>`s carry the same
//! text; only the count separates a tree that pruned the closed menu from one that did not.
//!
//! ⭐ **`s1`/`o1`/`o2` are the control that stops this being an over-prune.** A collapsed `<select>`
//! shows no option list, and Chrome still exposes both `<option>`s. If the UA sheet hid them the way
//! it hides a closed `<dialog>`, this tick would have silently deleted every dropdown's contents
//! from the agent's perception — so the row asserts they survive.
//!
//! ## ⚠ NAMED, MEASURED, NOT BUILT — one row, and the mechanism is the UA SHEET, not this rule
//!
//! ```text
//!   <details><summary>More</summary><p id=p1>body</p></details>   (no `open`)
//!                                       chrome                 ours
//!     is <p id=p1> in the tree?         YES, with role `none`   NO (pruned by this tick)
//! ```
//!
//! Chrome's UA sheet spells closed-`<details>` content as **`content-visibility: hidden`**, which
//! keeps the element in the tree as an ignored node (that is what makes "hidden until found" and
//! find-in-page work). Ours spells it `display: none`, so this prune removes it. **The divergence is
//! in the UA sheet's spelling, not in the prune** — asserting it here would pin the prune to a
//! different mechanism's bug. The row is recorded and not asserted; the day the UA sheet gains
//! `content-visibility`, `p1` joins the present list.
//!
//! ## THE AGENT-VISIBLE PRICE — why this is a capability tick and not tidiness
//!
//! `m1` and `m2` are the SAME LINK TEXT, one in a closed menu and one in the visible header, which
//! is the single commonest shape on the responsive web. t1375 made the drive path REFUSE an
//! ambiguous target rather than guess — correctly — so a phantom duplicate does not merely add
//! noise, it **turns a resolvable click into a refusal**. The last arm drives that: `Sign in`
//! resolves, and to the visible one.

use manuk_agent::AgentBrowser;

fn data_url(html: &str) -> String {
    format!("data:text/html,{html}")
}

const HTML: &str = r#"<html><head><style>
.menu { display: none }
.vis  { visibility: hidden }
</style></head><body style="margin:0">
<nav class="menu"><a id="m1" href="/a">Sign in</a></nav>
<nav><a id="m2" href="/a">Sign in</a></nav>
<button id="m3" style="display:none">Hidden inline</button>
<button id="m4" class="vis">Hidden vis</button>
<div class="menu"><button id="m5">Deep hidden</button></div>
<div class="vis"><button id="m6" style="visibility:visible">Re-shown</button></div>
<button id="m7">Normal</button>
<select id="s1"><option id="o1">One</option><option id="o2">Two</option></select>
<dialog id="d1"><button id="m8">In closed dialog</button></dialog>
<details id="dt"><summary id="sm">More</summary><p id="p1">body</p></details>
</body></html>"#;

#[tokio::test]
async fn a_display_none_subtree_is_not_in_the_a11y_tree() {
    let mut b = AgentBrowser::new(1200, 800);
    b.navigate(&data_url(HTML)).await.unwrap();
    let tree = b.a11y_tree().unwrap();

    /// Every name the tree publishes for `role`, sorted — the agent's own view of what it can
    /// address. An EMPTY name is kept as `""` on purpose: t1379 emptied the `display:none`
    /// button's name and left the node, and that row must be visible here.
    fn names(tree: &manuk_a11y::A11yNode, role: &manuk_a11y::Role) -> Vec<String> {
        let mut v: Vec<String> = tree
            .iter()
            .filter(|n| &n.role == role)
            .map(|n| n.name.clone())
            .collect();
        v.sort();
        v
    }
    let all_names: Vec<String> = tree.iter().map(|n| n.name.clone()).collect();

    // ── VACUITY. The fixture is only a test of HIDING if the visible half is there: an empty tree
    //    satisfies every absence below.
    assert!(
        tree.iter().count() > 4,
        "VACUOUS: the tree has {} nodes, so the absences below prove nothing",
        tree.iter().count()
    );
    assert!(
        all_names.iter().any(|n| n == "Normal"),
        "VACUOUS: the visible controls are missing from the tree — names {all_names:?}"
    );

    // (role, the exact name set Chrome's tree publishes, what the row decides)
    let rows: &[(manuk_a11y::Role, &[&str], &str)] = &[
        (
            manuk_a11y::Role::Link,
            &["Sign in"],
            "THE SHARPEST ROW — ONE `Sign in`, not two. The link inside `<nav class=menu>` computes \
             its OWN display as `inline`, so only an ancestor-level prune removes it, and both \
             links carry the same text so only the COUNT separates the two implementations",
        ),
        (
            manuk_a11y::Role::Button,
            &["Normal", "Re-shown"],
            "the inline `display:none` button (whose NAME t1379 had already emptied, leaving an \
             unnamed addressable node), the one two levels inside a hidden div, and the one inside \
             a `<dialog>` with no `open` are all gone — while `Re-shown` (visibility:visible inside \
             a hidden ancestor) and `Normal` stay",
        ),
        (
            manuk_a11y::Role::Option,
            &["One", "Two"],
            "CONTROL AGAINST OVER-PRUNING — a collapsed <select> shows no list and Chrome still \
             exposes both options. If the UA sheet hid them the way it hides a closed <dialog>, \
             this tick would have deleted every dropdown from the agent's perception",
        ),
        (
            manuk_a11y::Role::Dialog,
            &[],
            "a `<dialog>` with no `open` is UA-hidden, so neither it nor anything in it is exposed",
        ),
    ];
    for (role, want, why) in rows {
        let got = names(&tree, role);
        let want: Vec<String> = want.iter().map(|s| s.to_string()).collect();
        assert_eq!(
            got, want,
            "G_AX_TREE_EXCLUDES_DISPLAY_NONE {role:?}: Chrome's tree publishes {want:?}, ours \
             publishes {got:?}.\n  {why}"
        );
    }

    // ── CONTROL — the <select> itself, which the option rows sit inside.
    assert_eq!(
        tree.iter()
            .filter(|n| n.role == manuk_a11y::Role::ComboBox)
            .count(),
        1,
        "CONTROL: the <select> must still be exposed"
    );

    // ── THE AGENT-VISIBLE PRICE. `Sign in` names a link in the CLOSED menu and one in the visible
    //    header — the commonest shape on the responsive web. t1375 made the drive path REFUSE an
    //    ambiguous target rather than guess, so the phantom did not merely add noise: it turned a
    //    resolvable click into a refusal.
    b.resolve_handle(&manuk_a11y::Role::Link, "Sign in").expect(
        "`Sign in` must resolve — with the closed menu's phantom copy in the tree there are TWO \
             links of this exact name and the drive path refuses ambiguity (t1375)",
    );
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  remove the `node_visibility(...).0` prune from `build_children` (the pre-tick behaviour)
//       -> m1, m3, m5 and m8 are all present again, and the `Sign in` resolve fails on ambiguity.
//          Four rows and the drive arm, which is the whole finding.
// N2  make the prune drop the node but KEEP WALKING (the `visibility` arm's shape)
//       -> m1 and m5 come back: their own computed `display` is not `none`, which is exactly the
//          asymmetry this gate is about.
// N3  prune on `visibility` here too, instead of leaving it to the caller's `invisible` set
//       -> m6 disappears: `visibility:visible` inside a hidden ancestor is UNDOABLE and must stay.
