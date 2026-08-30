# `display: none` in the agent's accessibility tree — the fact nobody asked for

> Landed t1380. Gate: `a_display_none_subtree_is_not_in_the_a11y_tree`
> (`agent/tests/g_ax_tree_excludes_display_none.rs`), Chrome-measured through CDP
> `Accessibility.getFullAXTree`. Second finding of surface audit #79's ranked #1 sweep; the first
> was `docs/wiki/name-hidden-by-stylesheet.md` (t1379).

## The defect

```text
  is_hidden(dom, child)   reads the DOM     `hidden`, `aria-hidden`, <input type=hidden>,
                                            non-rendered tags
  the `invisible` SET     from the caller   computed `visibility` ONLY
  ────────────────────────────────────────────────────────────────────────────────────
  `display: none`         asked by NOBODY
```

A closed mobile menu, a `display: none` modal and a `<dialog>` without `open` were all in the
agent's accessibility tree as fully-formed, addressable nodes.

## ⭐⭐⭐ t1379 produced the symptom that names it

t1379 made the NAME walk read the computed `display` and left the TREE walk alone. So
`<button style="display:none">Hidden</button>` sat in the tree with an **empty name**.

> **A node whose name is correctly computed as nothing is a node that should not be there.**

A half-fixed hiding rule is louder than an unfixed one, which is the argument for finishing a sweep
rather than shipping its first finding and moving on.

## ⭐ The asymmetry with `visibility` is the rule

`visibility` **inherits** and is **undoable**, so the tree drops the node and keeps walking — a
`visibility: visible` descendant survives. `display` does neither: a child of a `display: none` box
computes its own ordinary `display` (the link inside `<nav class="menu">` computes `inline`, not
`none`), **so a per-node test never fires on the child.** The prune has to happen at the ancestor, by
not descending. That is why this cannot join the caller's `invisible` set, which is a per-node
membership test.

The resolver is t1379's `node_visibility`, reused: the computed `display` when the caller has a style
map, and the element's inline `style=` attribute when it does not — so `build_tree(dom)` on a bare
DOM behaves exactly as it did.

## The battery — Chrome via CDP

```text
                                                       chrome   before   after
  <a> inside <nav class=menu>  .menu{display:none}      absent   PRESENT  absent
  the same <a> in a visible <nav>            CONTROL    present  present  present
  <button style="display:none">Hidden inline</button>   absent   PRESENT  absent
  <button class=vis>          .vis{visibility:hidden}   absent   absent   absent
  <button>Deep hidden</button> two levels inside .menu  absent   PRESENT  absent
  <button style="visibility:visible"> inside .vis       present  present  present
  <button>Normal</button>                    CONTROL    present  present  present
  <button> inside a <dialog> with no `open`             absent   PRESENT  absent
  <select> and its two <option>s             CONTROL    present  present  present
```

⭐⭐ **`Links == ["Sign in"]` — one, not two — is the sharpest row.** Both `<a>`s carry the same
text, so only the COUNT separates a tree that pruned the closed menu from one that did not.

⭐ **The `<option>` rows are the control against over-pruning.** A collapsed `<select>` shows no
list and Chrome still exposes both options. If the UA sheet hid them the way it hides a closed
`<dialog>`, this change would have silently deleted every dropdown from the agent's perception.

⭐ **Rows are asserted as whole name sets per role**, because `AgentBrowser` exposes the tree and no
DOM handle — which is the agent's real view, so a node is identified the way the agent identifies
one. Set equality is also the stronger claim: it rejects an EXTRA node as well as a missing one, and
the extra is what this is about.

## The agent-visible price

`Sign in` names a link in the closed menu and a link in the visible header — the commonest shape on
the responsive web. t1375 made the drive path **refuse** an ambiguous target rather than guess, so
the phantom did not merely add noise: **it turned a resolvable click into a refusal.** The gate's
last arm drives that.

## ⚠ Named, measured, not built — and the mechanism is the UA SHEET

```text
  <details><summary>More</summary><p id=p1>body</p></details>   (no `open`)
                                      chrome                 ours
    is <p id=p1> in the tree?         YES, with role `none`   NO (pruned here)
```

Chrome's UA sheet spells closed-`<details>` content as **`content-visibility: hidden`**, which keeps
the element in the tree as an ignored node (that is what makes find-in-page and "hidden until found"
work). Ours spells it `display: none`, so this prune removes it. **The divergence is in the UA
sheet's spelling, not in the prune** — asserting it here would pin the prune to a different
mechanism's bug. The day the UA sheet gains `content-visibility`, `p1` joins the present list.

## ⚠ And one more gap the gate's own control found

The `<summary>` row was written as the control for *"a closed `<details>` is still exposed"* and it
fired: our tree gives `<summary>` role `Generic` with an **empty name**, where Chrome says
`DisclosureTriangle` / `"More"`. A role-mapping gap, not a hiding one — measured, recorded, left to
its own tick, and dropped from the asserted set rather than making this gate fail for a reason it is
not about.

## How it was proven red

- **N1** — remove the prune (the pre-tick behaviour): `Links` reads `["Sign in", "Sign in"]`.
- **N2** — drop the node but KEEP WALKING (the `visibility` arm's shape): `Links` reads
  `["Sign in", "Sign in"]` again, because the child's own `display` is not `none`. That is the
  asymmetry, isolated.
- **N3** — prune on `visibility` here too, instead of leaving it to the caller's set: `Buttons` loses
  `Re-shown`, because `visibility: visible` inside a hidden ancestor is undoable.
