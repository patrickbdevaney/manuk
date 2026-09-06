# A rotated run's `x` is the pen's starting *y* — and the extent walk read it as an x

**Tick 1456 — REFUSED.** The defect is proven and localised to one expression; the obvious correction
is refuted with a number. Nothing landed.

## The defect, proven three ways

`writing-mode: vertical-lr`, `overflow: scroll` inline-flex, three 110px items whose text is
`1`/`2`/`3`:

```text
                    Chrome    ours
  scrollWidth        110       245
  scrollHeight       350       350   ✓
  the item RECTS   [3,3 110x110 | 3,123 | 3,243]   IDENTICAL to Chrome in every row
```

1. **The layout is exact.** Every box rect matches Chrome; only the metric differs. That alone says
   this is a *reading* of the fragment tree, not a defect in it.
2. **`245 = 243 + 2`** — the third item's own **y**, plus its glyph advance.
3. **Removing the text makes it Chrome-exact** (`110/350`). The runs are the cause.

`TextStyle::sideways` marks a run inside a vertical writing mode, and its own doc comment already says
what the fields mean there:

> *"for a rotated run the item's `x` is the pen's starting **y** and its `baseline` is the baseline's
> **x**, which is what a ninety-degree rotation makes of those two words."*

`scrollable_overflow_extent` reads them physically, so a run's **downward** advance accumulates as
**horizontal** overflow.

## ⭐⭐ One rule, two implementations, and only one had learned it

`scrollable_overflow_start` — the *other* walk over the same tree — already refuses to consult inline
fragments, in a comment that says *"a START edge from a run would be a number in the wrong axis"*. The
start walk can decline because text that overflows **backwards** does so inside a box; the end walk
cannot, because a run overflowing forwards is the commonest overflow there is.

## What was refused, and with what number

Transposing the two fields for a sideways run — `w` from `baseline`, `h` from `x + width` — makes
`scrollHeight` explode (350 → 795). Taking only the `w` half makes **all six probe rows Chrome-exact**
and reads **−14** on `css/css-flexbox`: 14 new failures, 0 fixed, all of them in the two
`negative-overflow` files this probe was reduced from.

> ⚠⚠⚠ **A SIX-ROW FIXTURE I WROTE AGREED WITH A RULE THE REAL MATRIX REFUTES.** That is the session's
> most-repeated lesson arriving one more time and pointed at the probe rather than the engine: the six
> rows could not discriminate `baseline` from the right answer, because in all six the two coincide.

## What the next tick must establish first

**What `f.x`, `f.baseline` and `f.width` actually mean for a sideways run, measured rather than read
off the doc comment.** The comment says `x` is a `y` and `baseline` is an `x`; the −14 says that is not
the whole story — most likely one of them is line-relative where the other is absolute. Print the three
fields for a known three-line vertical run and compare against the box rects, exactly as t1450 printed
the child rect that dissolved t1449's contradiction.

## Status

Open. Defect proven, cause named, the naive fix refuted with a measurement.

**RESOLVED at t1457** — the fields were printed instead of reasoned about, and the answer was a
MIXED frame rather than a clean swap: see [[a-rotated-runs-fields-are-a-mixed-frame]].
