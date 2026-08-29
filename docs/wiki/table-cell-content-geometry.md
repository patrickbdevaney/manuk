# Table cell content geometry — the two things a cell forgot when its content made no line box

> Landed t1360. Gate: `a_cell_without_a_line_box_still_has_a_height_and_a_baseline` (`agent/tests/`).
> Every number below is headless-Chrome-measured on the gate's exact fixture
> (`16px/24px monospace`, 400px wrapper).

## The one-sentence mechanism

> **"This cell has no line box" and "this cell has nothing in it" are two different answers, and a
> table cell was giving the second one for both** — so a cell whose content is a block or a float
> contributed no baseline to its row, and a cell whose only content was a float had no height at
> all.

## Half one — the synthesized baseline

`vertical-align: baseline` is the initial value for a table cell, and `td { vertical-align:
baseline }` is in the reset sheet of **four of thirty-nine** sampled CrUX sites, so a row's cells
align their first-line baselines with each other constantly. t933 built that alignment for cells
that *have* a first line box, and wrote down what it did with the rest:

> *"A cell with no line box at all (an empty cell) has no baseline to contribute and does not join
> the max."*

The parenthesis is the bug. A cell whose content is a **block** — an icon `<div>`, a spacer, a
`display:block` image — produces no line box either, and Chrome joins it to the row's baseline
group, synthesizing the cell's baseline from the bottom edge of its content (CSS 2.1 §17.5.4). The
commonest table row on the legacy web is an icon cell beside a label cell, and it was aligning to
the text alone: the label sat at the top of its cell and the row came out several pixels short,
which then moves every box below the table.

### The three distinctions, none of which is guessable

1. ⭐ **It is the NATURAL content height, not the used one.** A cell forced to `height: 200px`
   around a 50px block still has its baseline at 50 — the neighbour's label lands at y=35, not
   y=185. This is the *same* distinction t933's free-space calculation already had to make, for the
   same reason: reading the box height back gets it wrong for exactly the cells with the most free
   space.
2. ⭐ **It is the bottom MARGIN edge of the content.** A block with `margin-bottom: 25px` puts the
   row's baseline at 75, not 50 — which falls out of asking the cell's own BFC height rather than
   the last child's border box.
3. ⭐ **The alignment moves BOTH ways.** When the text cell has the deeper baseline it is the
   *block* that must come down: a 6px block beside a text label belongs at y=11, not y=0. A fix that
   only pushes text down passes every other row and fails this one.

⚠ **An empty cell must still stay out, and that is why the synthesis is conditional.** Giving it a
baseline of 0 makes it demand *its own height plus the row's whole baseline shift* and grows the row
by that much — a declared-50 row measured 67. The old comment's parenthesis was describing a real
case; it was attached to the wrong test.

## Half two — a cell is a BFC root, so it contains its floats

`layout_cell` builds a `FloatContext` for the cell (it must, or the cell's own text would not flow
around its floats) and then took only `layout_children`'s in-flow height, which a float does not
contribute to. A cell whose only content is a float therefore collapsed to **zero**:

```text
  <td><div style="float:left;height:50px"></div></td>     Chrome 50x50     ours 0x0
```

The whole row vanished. `layout_block` has answered this correctly for every other BFC root since
floats were built (`own_bfc.lowest_bottom() - content_y`); this is that same line, at the one BFC
root that was constructing the context and never querying it.

The two halves compose: with the cell's height right, a float-only cell also gets the right
synthesized baseline, and the label beside it lands where Chrome puts it.

## The measured rows

```text
                                              first cell   label      table h
  1  50px BLOCK child  |  text label                  0        35        57   <- was 2 / 50
  2  EMPTY cell h=50   |  text label                  0         2        50      NEGATIVE
  3  50px <img> child  |  text label                  0        35        57      CONTROL
  4  BLOCK + overflow:hidden on the cell              0        35        57   <- was 2 / 50
  5  40px/60px text    |  text label                  7        29        60      CONTROL
  6  BLOCK, cell padding:10 border:3                 13        48        76   <- was 2
  7  SHORT 6px block   |  text label                 11         2        24   <- was 0
  8  BLOCK 50, cell forced height:200                 0        35       200   <- was 2
  9  TWO blocks 20+30  |  text label                  0        35        57   <- was 2 / 50
 10  BLOCK 50 + margin-bottom:25                      0        60        82   <- was 2 / 50
 11  FLOAT-only cell h=60  |  text label              0        35        60   <- was 2
 12  FLOAT-only cell, no height (cell / table)             50 / 50            <- was 0 / 0
```

Rows 3 and 5 are controls on the path that already worked — an `<img>` is an atomic *inline*, so it
makes a line box and reaches the baseline through `first_line_baseline` rather than through the
synthesis. They must not move, or the fix has replaced the working path instead of adding to it.

## ⚠ How this was found, and the two instrument facts that came with it

The tick before this one ran `manuk-page` as a control arm and found `g_table_cell_valign` **RED**.
Bisecting by restoring `engine/layout` and `engine/css` to the previous commit showed it was red
there too — it had been red since **2026-08-06**.

⚠⚠⚠ **THE GATE WAS WRONG, NOT THE ENGINE — and it was wrong in the direction that pins a bug.** Its
CONTROL A asserted that a `<td>` with no `vertical-align` keeps its content at the top (y=2), on the
reasoning that *"the CSS initial value for a cell is `baseline`, approximated here as `top`"*. The
CSS initial value is not what a `<td>` computes. Measured in headless Chrome:

```text
  #a3   dy = 20      getComputedStyle(td).verticalAlign = "middle"
                     tbody = middle      tr = middle
```

Chrome's UA sheet is `tbody { vertical-align: middle }` + `tr, td { vertical-align: inherit }`, so a
plain cell computes **middle**. The row was a prose-derived value that had never been measured, and
it went red the moment the engine became correct. This is the t1344–1346 finding again: *measure
every red gate against headless Chrome before believing it.* The row now asserts Chrome's answer,
and the check it was *reaching* for — an explicit `vertical-align: baseline` degrading to top on a
single-line cell — was added beside it as its own row, because the value CONTROL A asserted was
indistinguishable from what an unconditional `top` produces.

⚠⚠ **It went red silently because it is not in the wall.** `scripts/verify.sh` runs
`manuk-css manuk-layout manuk-paint manuk-dom manuk-net manuk-agent manuk-shell` as crate suites and
launches `manuk-page` gates only by explicit name. `g_table_cell_valign` has no such line, so
twenty-three days of green walls ran over a red gate. `scripts/` is observer-owned, so the new gate
is placed in `agent/tests/` — where the wall already looks — rather than the wall being changed to
look at it.

⚠ **A gate in `agent/tests/` runs on `MinimalCascade`, not Stylo.** `manuk-agent` takes `manuk-page`
with *default* features. This gate was first written with the `font: 16px/24px monospace` shorthand
and read a **54px** row where Chrome and the shipping Stylo pipeline both read 57 — the two cascades
disagree about the shorthand's `line-height`. Recorded as its own finding; the fixture now states
`font-family`/`font-size`/`line-height` as longhands, because a table gate that can be reddened by a
font shorthand is not a table gate.

## NAMED, MEASURED, NOT BUILT

```text
  font shorthand line-height   `font: 16px/24px monospace` gives a 24px line box through Stylo
                               and something 3px shorter through MinimalCascade (57 vs 54 on a
                               table row). Two cascades, one declaration, different answers.
  line-box overflow            40px text in an inherited 24px line box: Chrome puts the glyph run
                               at dy=-11 (it overflows its line upward), we put it at +1.
  float-only cell + rowspan    untested; the rowspan path takes a different height route.
```

The three table defects t933 named — rowspan row-height distribution, `<caption>`, and `<thead>`
ordering — are unchanged and still measured in `g_table_cell_valign`'s header.
