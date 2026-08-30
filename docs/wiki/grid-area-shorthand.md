# `grid-area` — the shorthand that placed nothing, and auto-placement that hid it

> Landed t1376. Gate: `the_grid_area_shorthand_places_on_both_axes` (`agent/tests/`).
> Found by surface audit #79's drift table.

## The one-sentence mechanism

> `MinimalCascade` parsed `grid-column` and `grid-row` and **not** `grid-area`, so a rule that placed
> an item with the combined shorthand placed nothing — **and the item fell back to auto-placement,
> which lands in the right cell often enough to hide the bug.**

`grid-area` and `grid-template-areas` are declared **127 times** across 14 sampled CrUX stylesheets.

⚠ This is the weaker of the two cascades: the shipping (Stylo) path places both forms correctly,
measured. The value is the `--no-default-features` build **and** instrument fidelity —
`engine/layout`'s 191 unit tests run on `MinimalCascade`, and
`a_grid_generated_containing_block_is_the_grid_area_for_children_and_descendants_alike` writes
`style="grid-area:1/1"` on an intermediate box. That declaration did nothing; the box landed in cell
1/1 by auto-placement instead, so the gate was green **for a different reason than it states**.

## ⭐ The order is row / column / row / column

Not the row-then-column *pairs* that `grid-row: a / b` and `grid-column: a / b` use. Reading it as
two pairs transposes the placement — and on a symmetric fixture that is **invisible**.

```text
  a 2×2 grid of 100×50 cells, the item's rect relative to its container:

  grid-area: 2 / 2 / 3 / 3      [100 50 100x50]
  grid-area: 2 / 2              [100 50 100x50]   the omitted ENDS are auto — one cell
  grid-area: 2                  [  0 50 100x50]   row-start only; the column is auto
  grid-area: 1 / 1 / 3 / 3      [  0  0 200x100]  spanning both cells on both axes
  grid-area: span 2 / span 2    [  0  0 200x100]  `span N` is a grid line like any other
```

The one-value row is the asymmetric one on purpose: a transposed read puts it at `[100 0]` where
Chrome says `[0 50]`, and it is the only row that catches the transposition when a single value is
given.

## ⚠ The NAMED form is deliberately not parsed

`grid-area: header`, resolved against `grid-template-areas`, is **not representable**: `GridLine` is
`Auto` / `Line(i16)` / `Span(u16)` with no ident, and the shipping path resolves names before this
type is reached. The only alternatives are *ignore it* and *silently turn it into `Auto` at some
other cell* — and a name becoming a number places the item somewhere the author never asked for. The
parser detects an ident and does nothing, and the gate asserts that as a **pinned negative**: with
one item and no `grid-template-areas`, an unparsed declaration auto-places into cell 1/1, which a
mis-parse into `Line(0)` or a span would not.

## Proven red

- **N1** drop the arm (the pre-tick behaviour) → every item auto-places into cell 1/1; the spanning
  rows read one cell instead of four. The pinned negative stays green, because an unparsed named
  form is exactly what it asserts — which is why it cannot carry the gate alone.
- **N2** read the values as row/row/col/col → the four-value row reads `[200 50 10x50]`.
  ⚠ The ledger's first draft predicted the one-value row alone, on the reasoning that the others are
  symmetric — true of the one- and two-value rows, **false of the four-value ones**. Corrected to
  what actually fired.


---

## t1377 — the drift table's other two layout rows, priced and withdrawn

Surface audit #79 ranked `MinimalCascade`'s missing properties by **declaration count** and called
three of them layout drift. `grid-area` (above) was real. The other two were not:

```text
  -ms-flex          63 declarations   CHROME IGNORES IT. Two children with `-ms-flex: 1` and `2`
                                      in a 300px flex row are 10px each — their content width.
                                      Implementing it would make this engine DIVERGE.
  -webkit-box-flex  63 declarations   ONE of 14 sites declares it, and that site also declares
                                      `-webkit-box-orient: vertical` — the case that IS implemented.
                                      Corpus-wide: vertical 40, horizontal 19.
```

> ⭐⭐⭐ **A declaration count is not a site count, and a site count is not a divergence.** #79 had
> already re-sorted its table by *what a property does* after finding `transition` (447 declarations,
> inert) at the top. This is the other half: price by SITE, then check whether the browser honours
> the property at all.

Both behaviours are banked by `g_legacy_webkit_box` (`agent/tests/`), the `-ms-flex` row as a
**pinned negative** whose only job is to stop a future reader implementing it off a drift table —
proven red by adding the `flex_grow` arm, which makes the children read 103 and 197.
