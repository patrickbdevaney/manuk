# Two reversals that cancel — and the order the compensations run in

**Tick 1439.** `css/cssom-view` **+30** (1268 → 1298), `css/css-flexbox` **+2**, `css/css-grid` flat,
`css/css-sizing` flat. **`scrollWidthHeight-negative-margin-002.html` is 600/600.**

## The defect

Two of this engine's taffy compensations act on the same physical axis:

* `mirror_rtl_inline` (t764/t1272-era) — taffy has no `direction`, so an RTL container's inline axis
  is mirrored on the placed slots on the way out.
* `shift_wrap_reverse_overflow` (t1437) — taffy packs an overflowing `wrap-reverse` line from the
  wrong edge, so the slots are translated by the negative free space.

t1437 ran the shift **after** the mirror. But the shift is expressed in **taffy's own un-mirrored
logical space**, where a reversed cross axis always overflows toward negative; running it after
subtracts on an axis that has already been flipped.

> ⭐ **It only goes wrong when BOTH reversals are present.** `direction: rtl` puts a COLUMN flex's
> cross start at the RIGHT edge and `wrap-reverse` flips it back to the LEFT, so the two together are
> the un-reversed case and the box must not move at all. That is the last ten rows of
> `cssom-view/scrollWidthHeight-negative-margin-002`.

`80 − (−220) − 300 = 0` is the mirror doing exactly the right arithmetic — once the shift is on the
side of it that speaks taffy's coordinates.

## Why the fix is an ORDERING and not a rule

```text
                                                  Chrome    before    after
  column, rtl, wrap-reverse, 300px item            0,0      -220,0     0,0     ← the only row that moves
  column, rtl, `wrap`               CONTROL       -220,0    -220,0    -220,0   ✓
  column, LTR, wrap-reverse         CONTROL       -220,0    -220,0    -220,0   ✓
  row,    rtl, wrap-reverse         CONTROL       60,-220   60,-220   60,-220  ✓
```

Each control has exactly **one** of the two reversals (or has both on a `row` container, whose cross
axis the inline mirror does not touch), and all three were Chrome-exact before and after. Nothing
about either rule changed — only their composition.

> ⚠ **The two mutations disagree about everything except the row they both get wrong.** Restoring
> t1437's order breaks that row alone; deleting the shift entirely breaks two other rows *and* leaves
> that row at −220 as well. So "no shift" is not the right answer for it either: the right answer is a
> shift the mirror then undoes.

## The general rule this is an instance of

Every taffy compensation in `layout_flex_or_grid` is a coordinate transform on the placed slots. They
compose, and **composition has an order**. The invariant that makes it decidable:

> **A compensation is written in the space taffy produced, so it runs before any transform that
> leaves that space.** `mirror_rtl_inline` leaves it; the wrap-reverse shift does not.

## Gate

`engine/page/tests/g_flex_wrap_reverse_rtl_order.rs` — 10 rows, 9 of them controls, including the
FITTING twin of each of the four reversal combinations (a line that fits must be untouched however
many reversals are stacked on it). Red under Q1 (restore t1437's order → one row) and Q2 (delete the
shift → two other rows).
