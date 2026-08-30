# `pointer-events` — inert on the cascade every agent gate runs on

> Landed t1373. Gate: `g_pointer_events_cascade` (`agent/tests/`). Found by surface audit #79.

## The one-sentence mechanism

> `ComputedStyle::pointer_events` exists, Stylo's mapper sets it, and `Page::non_hittable_nodes`
> reads it to build the a11y tree's `hittable` flag — **and `MinimalCascade` never parsed the
> property**, so on the harness every layout unit test and every `agent/tests` gate runs on, a
> `pointer-events: none` overlay was solid.

```text
  an overlay with `pointer-events: none` over a button:
  non-hittable nodes in the a11y tree     Stylo 1     MinimalCascade 0
```

`pointer-events` is declared **146 times** across 14 sampled CrUX stylesheets.

## ⭐⭐⭐ It lands squarely on the week's own work

- **t1359** DEFINED `Landing::Unreachable` as *"on screen and `pointer-events: none`"*.
- **t1366** made the agent's drive path refuse an obstructed target and scroll to an off-screen one.

Both are gated in `agent/tests` — **on the cascade where the property did nothing** — so the one arm
of `landing` that distinguishes *unaimable* from *off screen* was untestable there.

⚠ `manuk-a11y`'s own `hit_test_passes_through_a_pointer_events_none_overlay` does not catch it
either: **it builds the tree by hand**, setting `hittable` directly. It tests `hit_test`'s traversal
and never the cascade that feeds it.

> ⭐⭐⭐ **A GATE THAT CONSTRUCTS ITS OWN INPUT CANNOT DISCOVER THAT THE PRODUCER OF THAT INPUT IS
> BROKEN.** A sharper form of audit #77's *"a gate cannot discover a missing word in its own
> vocabulary"*: there the gate's sample was drawn from what was built; here the gate's whole input
> is.

## How it was found — the drift measured instead of tripped over a fifth time

This is the twin-cascade drift class for the fifth time in a week (t1361 `font-size` clobbering an
inherited `line-height`, t1364 `border-spacing`, t1369 the `content` alt syntax, t1372 `attr()`). The
first four were each accidents. Surface audit #79 asked how big the class is: 14 corpus stylesheets,
every declared property counted, every name checked against `MinimalCascade`'s source.

⚠⚠ **The first extraction was wrong, in the confident direction.** A regex over `"prop" =>` arms
reported `overflow` (419 declarations), `filter` and `border-bottom` as unhandled — all three are
handled, in multi-name arms the regex could not see. `SURFACE-AUDIT.md`'s own note says *"grep the
artefact, infer the engine — has now produced a wrong number three times"*, and it had just produced
a fourth **inside the audit whose job is to catch that**. The ranking was redone against every quoted
string in the file (erring toward "handled", so the surviving list understates), and the top row was
then measured on both cascades.

⭐ **Sort a drift table by what the property DOES, not by how often it is declared.** The biggest
number in it was `transition` at 447 — and an unparsed transition renders the static state, which is
the correct static answer. The row that mattered was ninth.

## The remaining layout drift, priced

```text
  grid-area 77 + grid-template-areas 50                     grid placement
  -webkit-box-flex 63 + -ms-flex 63 + -webkit-box-orient 60  legacy flex
```

Unlike `transition` and `cursor`, an unparsed `grid-area` puts a box in the wrong cell. Each is a
candidate tick with a price attached.

## The gate

Three arms plus a vacuity assert that both overlays really are on a higher stacking layer:

1. the **count** of non-hittable nodes is exactly one;
2. a click inside the transparent overlay passes **through** to the button beneath;
3. **CONTROL** — an ordinary overlay still intercepts, without which the gate passes against an
   engine that ignores overlays entirely.

⚠ Proven red by two mutations, and the ledger records which arm actually fired rather than the one
predicted: inverting the sense (`none` → Auto, else → None) trips the **count** arm first, not the
control, because it makes every element whose `pointer-events` is not `none` non-hittable. Counting
catches both directions; "the click passed through" catches only one.
