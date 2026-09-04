# The start edge of the scrolling area — two bugs a fixture with a zero could not see

> Landed t1429, on top of t1427's unreachable-region rule. Gate:
> `g_unreachable_scrollable_overflow`, grown from 6 rows to 11, red under six mutations.
> `css/cssom-view` **795 → 905 (+110)**; `css/css-overflow` unchanged at 555.

t1427 introduced the flip — *the unreachable side of the scrolling area follows the scroll origin* —
and shipped it with a six-row gate whose containers have **no border**. Two defects lived in the
border term, and the survey that found them was not looking for them: `--show-failures` on
`css/cssom-view` put **410 of its 1,314 failures in one file**,
`scrollWidthHeight-negative-margin-002` (600 subtests, 190 passing).

## Bug 1 — the sign

The START edges come out in border-box coordinates and the region is measured in the padding box, so
the start border is **subtracted**, exactly as the maxima's `- bw.left` does. It was written `+`.

```text
  the file's wrapper: border-left 4px, padding-left 16px, a 300px child at margin:-100px
                                       chrome    with `+`    with `-`
    rtl, scrollWidth                     204        196        204
```

⭐ **A fixture with a zero in the term cannot see the term's sign.** With `border: 0`, `+ bw.left`
and `- bw.left` are the same expression.

## Bug 2 — the sentinel

`scrollable_overflow_start` seeded its accumulator at **0**, meaning *"no box reaches backwards"* —
but 0 is also a legal answer, *"a box reaches back to exactly the border-box origin"*. The caller then
subtracts the start border from it, and a container with an 80px left border reads **80px of start
overflow that is not there**.

```text
  css-overflow/overflow-outside-padding: six containers, border-width: 0 0 50px 80px
                          chrome   seeded at 0   seeded at MAX
    scrollWidth             200        280           200
```

> ⭐⭐⭐ **A SENTINEL THAT IS ALSO A LEGAL VALUE IS NOT A SENTINEL.** Seed at `f32::MAX` and clamp
> AFTER the conversion: `MAX - border` is still huge and clamps to 0, a box on the padding edge
> converts to exactly 0, and only a box that genuinely reaches back past the padding box comes out
> negative.

That WPT file's own assertion is the same sentence from the other side — *"blocks wholly outside
padding edges should not contribute to overflow"* — and it is the **only** subtest in `css-overflow`
that the sign fix broke, which is how it was found: **diff the failing NAMES, never the totals.**

## Bug 3, found on the way — inline fragments are not in physical coordinates

A run inside a vertical writing mode keeps its LOGICAL fields and carries the axis map instead
(`writing_mode::map_subtree`: *"silently re-pointing `x`/`width` at a different axis is how a field
ends up meaning two things"*). Reading `f.x` as a physical coordinate in the start walk reads the
inline advance of a run that is physically vertical. Boxes carry real physical rects after the map;
runs do not — so the start walk consults **boxes only**, and text that genuinely overflows backwards
does so inside a box, which is measured.

## The gate now

Eleven rows. The three added groups are each a class the original six could not see:

```text
  a…f   the six origin combinations                     (t1427)
  g,h,i asymmetric borders AND padding, WITH start overflow   → catches the SIGN
  j,k   a flipped origin, a border, and NOTHING overflowing   → catches the SENTINEL
```

```text
  N1  never flip                                   → c, d, e, f
  N2  flip x whenever the mode is VERTICAL         → the control b
  N3  flip both axes together                      → c, d, e
  N4  add the start overflow without dropping the end → c
  N5  `+ bw.left` instead of `-`                   → h
  N6  seed the start accumulator at 0              → j
```
