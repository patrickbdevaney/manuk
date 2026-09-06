# A rotated run's fields are a mixed frame

A text run in a `writing-mode: vertical-*` box stores its position in the **same three struct
fields** as a horizontal one — `x`, `baseline`, `line_top` — and `translate()` shifts them the same
way for both (`f.x += dx`, `f.baseline += dy`). The slots are physical. **The values a sideways run
puts in them are not all in the same frame.**

Printing the two runs of an identical `width = 231.19` — one horizontal in a box at `y = 400`, one
`vertical-lr` in a box at `y = 200` — is the entire derivation:

```text
                   rect.y      x   baseline   line_top
  sideways=false      400      0     415        400      ← both carry the box's y
  sideways=true       200      0      15          0      ← neither does
```

The rotated run's **block-axis coordinates were never translated into the box's frame**, while its
`x` was. So for `sideways`:

| field | horizontal | sideways |
|---|---|---|
| `x` | absolute physical x | **absolute physical x** (unchanged) |
| `baseline` | absolute physical y | an offset **along x** *inside the line* (~15 for a 20px line) |
| `line_top` | absolute physical y | **0** — line-local |
| `width` | advance along x | advance **down y** |

## Why a flat transposition is worse than no fix

The obvious repair — swap which field feeds which axis — feeds an **absolute x into a y**. It
understates by the box's y-origin and it was measured at **−14** on `css/css-flexbox` on one attempt
and **−22** on another. Both times the entire loss was in the same two files,
`negative-overflow-002.html` and `negative-overflow-004-no-padding.html`.

## What those two files are actually asserting

They generate 72 containers over writing-mode × direction × flex-direction × wrap and compute every
expectation from a `bias` formula that reads **only those four properties**:

```js
container.setAttribute('data-expected-scroll-width',  bias ? 130 : 370);
container.setAttribute('data-expected-scroll-height', bias ? 370 : 130);
```

`370 = 3×110 items + 2×10 gap + 2×10 padding`, `130 = 110 + 20`. The expectations are **pure box
geometry**. Each item holds one 8px digit that must contribute *nothing*. That makes the pair a
precise arbiter for one question: does a rotated run in a **descendant** box add anything to the
scroll container's overflow?

It must not. A descendant's extent is already covered by the walk's painted-position arm
(`b.rect.y + b.rect.height - oy`); re-measuring its run under the mixed frame can only overstate.
**The correction belongs only where the run is the scroll container's own content.** Scoped that
way the same suite is net zero and the divergence is fixed.

## The degenerate rows

A 100×100 box whose text overflows on *neither* axis reads `100/100` before and after — **a square
cannot see a transposition.** Only a box that overflows on exactly one axis discriminates. This is
the same shape as [[the-fixture-is-part-of-the-instrument]]'s `width:0`, its symmetric `scale()` and
its zero border: the fixture has to be able to *tell the two answers apart* before it can certify
one.

## The area named for the mechanism cannot see it

`css/css-writing-modes` is **completely unmoved** by this fix — 241 failing before and after, a
byte-identical name list. `css/css-flexbox` is the only area that reacts, and it reacts by
*refusing* the over-broad version. An area name is a directory, not a cause: the suite that
exercises a feature is not necessarily the suite that can measure a bug in it.

See also [[a-rotated-runs-x-is-a-y]], [[block-extent-is-the-logical-one]],
[[the-end-padding-attaches-to-the-margin-box]].
