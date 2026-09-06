# The tree was always richer than the thing that could read it

**Tick 1452 — Track B started.** `manuk-a11y` now projects into **AccessKit**, the standard Rust
accessibility-tree crate (the one servo emits). Named at **ten consecutive constitution checks** as the
fastest greenfield win, and never begun until the observer made it an order.

## What was and was not missing

The tree itself has been built, correct and measured for a thousand ticks: roles (79 of them),
accessible names computed to the accname spec, interaction state (`checked` tri-state, `expanded`,
`selected`, `disabled`, `pressed`), absolute border boxes, and occlusion-aware hit testing.

> ⭐ **Every one of those facts was reachable only through Manuk's own types.** AccessKit is the shape
> a screen reader, an OS accessibility bridge and every Rust a11y harness already speak. Adopting it
> is the difference between *"we have an accessibility tree"* and *"an assistive technology can read
> this page"*.

## A projection, not a second source of truth

Nothing in `accesskit_bridge` computes a role, a name or a state — every field is read off the
`A11yNode` the existing builder produced.

> *One rule, one implementation* is this project's most-repeated lesson, and an accessibility tree
> computed twice would be the largest possible instance of it.

## What the mapping costs, said out loud

* `Role::Heading { level }` → `Role::Heading` **plus the `level` property**: AccessKit's heading role
  is level-less, and dropping the level announces every heading on the page as an `<h1>`.
* `Subscript` / `Superscript` → `GenericContainer`. AccessKit carries those as a `vertical_offset`
  property this bridge does not yet set. **The distinction survives in Manuk's tree and is lost in the
  projection** — recorded rather than hidden.
* Node ids are the **arena's own**, so an AccessKit consumer that reports a node can be taken straight
  back to the DOM element. That is what makes this useful to an *agent* and not only to a screen
  reader.

## ⚠ And running a crate the wall does not run found it RED

`manuk-a11y`'s unit tests are not in `verify.sh`'s crate list. Running them for the first time in this
tick found the crate **failing**, on two assertions that went stale when the engine got *more* correct:

```text
  t1411  the a11y root gained the document's title    "document" → "document \"Shop\""   ~40 ticks red
  t1404  `listitem` correctly stopped taking a name    "listitem \"One\"" → "listitem"    ~47 ticks red
         from its content (94% of the tree's error
         on a real corpus; 75.0% → 97.0%)
```

Both are the t1344 shape — *the engine got more correct and the test held a literal*. 21/21 green now.

> ⭐⭐ **A test outside the wall is a test that can be red indefinitely**, and these two were. The wall
> runs seven crates; this is an eighth.

## Gate

`engine/page/tests/g_accesskit_tree.rs` — role, name, heading level, `disabled`, tri-state `toggled`,
`selected`, bounds, and the tree/child-id contract AccessKit's consumer would otherwise panic on. Red
under five mutations (drop the label, drop the level, map `Checked::False` to nothing, drop the bounds,
emit dangling child ids).

⭐ Deleting the `Some(Checked::False)` arm outright does not compile — the match is exhaustive, so the
type system already forbids half of that mutation. The gate covers the half it cannot: mapping the arm
to the *wrong* thing rather than forgetting it.

---

## t1453 — the projection carried four of ten state fields, and named the root as focused

t1452 stood the bridge up and projected `checked`, `expanded`, `selected` and `disabled`. It dropped
`pressed`, `required`, `readonly`, `invalid` and `value`, and set `TreeUpdate::focus` to the document
root on every page.

### ⭐⭐⭐ A required field with a plausible default

AccessKit's `focus` is not optional — every update must name the focused node, and there is no `None`.
Pointing it at the root tells a screen reader *"the document has focus"* while the caret sits in a text
field, and tells an agent reading its own tree back that its `focus()` call went nowhere.

> **A required field with a plausible default is the most dangerous shape a projection has**: the
> consumer cannot tell *"we computed the root"* from *"we did not compute"*. Nothing is missing, so
> nothing looks wrong.

### ⭐⭐ `pressed` is a toggle button's only observable state

`Follow`, `Bold`, `Mute`, a filter chip, a "show password" eye — all `<button aria-pressed>`, never
checkboxes. This crate's own `A11yState` doc says so in a ⭐⭐⭐ comment, and the projection dropped it,
so the tree read `button "Follow"` before and after a click. **Identical** — the exact failure the
accessibility tree was built to prevent, reintroduced one layer out.

### The two narrowings, recorded

* `checked` and `pressed` are two ARIA sources for **one** AccessKit property (`toggled`). A node
  carries at most one meaningfully; `checked` wins where both somehow appear, because an element that
  is both a checkbox and a toggle button is an authoring error.
* `invalid` is a **bool** here and an **enum** in AccessKit (`True | Grammar | Spelling`).
  `aria-invalid="spelling"` is a real authored value this tree does not yet distinguish, so it maps to
  `True`.

### Gate

`engine/page/tests/g_accesskit_state_complete.rs` — red under B1 (drop `pressed` → both button rows),
B2 (focus at the root → the two focus rows) and B3 (drop the four form setters → one row each). **B4
is reported green**: preferring `pressed` over `checked` moves nothing in this fixture, because no
element carries both — the precedence is asserted in the source and is not gated.
