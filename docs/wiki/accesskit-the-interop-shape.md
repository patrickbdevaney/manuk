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
