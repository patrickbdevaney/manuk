# A transform is physical in every writing mode

> Landed t1426. Gate: `a_transform_is_physical_in_every_writing_mode`
> (`engine/page/tests/g_transform_is_physical_in_vertical_modes.rs`), four claims, proven red under
> three mutations.

## The defect

An orthogonal (vertical) run is laid out in a **transposed style space** and mapped back to page
coordinates by `writing_mode::map_subtree`. `transpose_in_place` transposed the `transform-origin`
and **not the transform itself** — so a transform applied inside that space rode the swap out of it.

Headless Chrome 145, a `100x200` child of a `100x200` container, rect relative to the container:

```text
                                       chrome              before
  horizontal-tb  translate(-3px,-6px)  [-3,-6,97,194]   [-3,-6,97,194]   ✓ CONTROL
  vertical-lr    the same              [-3,-6,97,194]   [-6,-3,94,197]   ← x and y swapped
  vertical-rl    the same              [-3,-6,97,194]   [ 6,-3,106,197]  ← swapped AND mirrored
  horizontal-tb  scale(1.10)           [-5,-10,105,210] [-5,-10,105,210] ✓ CONTROL
  vertical-lr    scale(1.10)           [-5,-10,105,210] [-5,-10,105,210] ✓ and THIS is why it hid
```

> ⭐⭐⭐ **A SYMMETRIC FUNCTION CANNOT SEE A SWAP.** Every `scale()` row was already exact in both
> writing modes, because a uniform scale is invariant under transposition. A fixture built from
> `scale`, or from `rotate` about the centre, or on a square box, gives a broken axis map a clean
> bill of health. **The discriminating input is an asymmetric one** — and half a transposition is
> exactly the kind of thing that survives for a long time behind symmetric fixtures.

## The rule

The conjugation **distributes** over the function list (`J⁻¹ABCJ = (J⁻¹AJ)(J⁻¹BJ)(J⁻¹CJ)`), so each
function is transposed on its own and the composition still comes out right. The axis map is
`x_phys = f(ey)`, `y_phys = by + ex`:

* **`vertical-lr`** — a swap. Determinant −1 (a reflection), which **reverses** a rotation.
* **`vertical-rl`** — a swap plus a mirror on the block axis. Determinant +1 (a quarter turn), which
  **preserves** a rotation.

```text
  Translate(tx, ty) -> Translate(ty,  tx)      vertical-lr
  Translate(tx, ty) -> Translate(ty, -tx)      vertical-rl
  Scale(sx, sy)     -> Scale(sy, sx)           both
  Rotate(r)         -> Rotate(-r) / Rotate(r)  lr / rl
  Skew(ax, ay)      -> Skew(ay, ax)            vertical-lr only
```

A `%` translate travels with its axis and only its SIGN depends on the block direction — the
percentage resolves against the transposed box, which is already the right reference.

⚠ **Named residue, not fixed:** `matrix()` in either mode, and `skew()` under `vertical-rl`. The
general conjugation of a raw matrix by the quarter turn is not expressible as any of the function
forms the enum holds, and an approximate one is worse than a value that is wrong in a way the next
reader can find.

## What it did NOT fix, with the numbers

`css-overflow/scrollable-overflow-transform-unreachable-region` — a
`flow-root|flex|grid × direction × writing-mode × flex-direction × flex-wrap` matrix over an
`overflow:scroll` wrapper with a transformed child — went from **1 of 6** shapes exact to **2 of 6**
(measured on fresh pages, so no stale-read luck):

```text
                       want          before        after
  ltr/horizontal-tb   102 / 204     102 / 204     102 / 204   ✓
  ltr/vertical-lr     102 / 204      99 / 207     102 / 204   ✓ this tick
  ltr/vertical-rl     108 / 204      96 / 207      87 / 204
  rtl/horizontal-tb   108 / 204      87 / 204      87 / 204
  rtl/vertical-lr     102 / 216      99 / 192     102 / 189
  rtl/vertical-rl     108 / 216      96 / 192      87 / 189
```

Every remaining error is the **unreachable scrollable overflow region**: when the scroll origin is at
the right or the bottom (`rtl`, `vertical-rl`), overflow on the *start* side becomes reachable and
overflow on the *end* side does not, and our extent clamps the wrong side (`w.max(0.0)` is the
`horizontal-tb ltr` case hard-coded). That is a second mechanism and the ranked next tick — see
[the-scrolling-area-of-every-element](the-scrolling-area-of-every-element.md), which is blocked
behind it.
