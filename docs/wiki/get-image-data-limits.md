# `getImageData` asked for 85 GB — the Bar 0 the aperture found

> Landed t1389. Gate: `get_image_data_refuses_what_it_cannot_allocate_and_says_which_kind_of_wrong`
> (`engine/page/tests/g_get_image_data_limits.rs`). Found by t1388's aperture tick, on its first
> pass.

## The crash

```text
  html/canvas/element/pixel-manipulation/2d.imageData.get.large.crash.html
      CRASH (killed by a signal — Bar 0)
```

`ctx.getImageData(10, 0xffffffff, 2147483647, 10)`. The shim did `w = Math.max(1, w|0)` and handed
`2147483647 × 10 × 4` bytes to a `vec![0u8; …]` — **85 GB, and the process died.** A crash loses
every tab, so Bar 0 outranks every visual cluster. It was reachable from any page for as long as this
API has existed, and was invisible only because `html/canvas` was outside the primary metric until
t1388.

## The error surface, all of it Chrome-measured

```text
  getImageData(10, 0xffffffff, 2147483647, 10)   TypeError    outside the 'long' value range
  getImageData(0, 0, 1e10, 1)                    TypeError
  getImageData(NaN, 0, 10, 10)                   TypeError
  getImageData(Infinity, 0, 10, 10)              TypeError
  getImageData(0, 0, 2147483648, 1)              TypeError    one past int32
  getImageData(0, 0, 2147483647, 10)             RangeError   Out of memory at ImageData creation
  getImageData(0, 0, 32768, 32768)               RangeError   4.295e9 bytes
  getImageData(0, 0, 23000, 23000)               OK           2.116e9 bytes  ← the boundary
  getImageData(0, 0, 0, 10)                      DOMException "The source width is 0"
  getImageData(0, 0, -5, 10)                     OK  w=5 h=10  the rect NORMALISES
  getImageData(5, 5, -5, -5)                     OK  w=5 h=5
```

**The ceiling is `w × h × 4 ≤ 2³¹−1` bytes**, placed by the pair 23000² (allowed) / 32768² (refused)
rather than guessed at.

⭐⭐ **The negative row is the one that stops this being a clamp.** `Math.max(1, …)` turned `-5` into
`1`; the spec NORMALISES the rectangle, so `-5` is a five-pixel-wide read starting five pixels to the
LEFT. **A clamp silently returns the wrong pixels where a normalisation returns the right ones** —
and a guard added only to stop the crash would very naturally have been a clamp. Mutation N2 is
exactly that guard, and it passes eleven of thirteen rows.

⭐⭐ **Three different failure kinds in one function**, and a feature-detecting library branches on
all three: `TypeError` for an argument outside `long`, `RangeError` for an allocation that cannot be
made, `DOMException`/`IndexSizeError` for a zero extent. Collapsing them into one "invalid argument"
throw passes any test that only checks *that* it throws.

⚠ `0xffffffff` shows why `v|0` is the wrong conversion: it WRAPS to `-1`, a legal `long` that would
have been silently accepted as a negative origin. WebIDL `[EnforceRange]` throws instead.

## The guard lives twice, on purpose

The shim applies `[EnforceRange]` and the ceiling; `cv_get_image_data` applies the ceiling AGAIN and
answers `null`, which the shim turns into Chrome's `RangeError`. **A host function that will allocate
`w × h × 4` bytes on request must not depend on its only current caller staying its only caller** —
the refusal belongs at the allocation, not merely upstream of it.

## The receipt

```text
  WPT html/canvas/element/pixel-manipulation   48/70 = 68.6%  HANG/CRASH 1 (Bar 0)
                                            → 58/71 = 81.7%  HANG/CRASH 0
```

⭐ The denominator moves 70 → 71 because the crashing file now REPORTS its subtest. A crash scores
zero out of zero (t1266's rule), so closing one adds to both sides.

## ⚠ The wall does not run this gate

`engine/page/tests/` is outside the wall's crate list (surface audit #78). Its wall-independent guard
is the WPT `html/canvas` row — which became part of the primary metric at t1388, **one tick before
this fix** — and which reports `HANG/CRASH` on its own summary line as Bar 0. The aperture tick paid
for itself immediately: it found this, and it is also what will notice if it comes back.

## ⚠ Named, measured, not built

Our host marshals the pixel bytes into a JS **Array**, one `JS_SetElement` per byte. Chrome allows a
2.1e9-byte read; ours would take that path element by element. The ceiling here is Chrome's, so the
ERROR SURFACE matches, but a large-but-legal read is slow in a way Chrome's is not. The fix is a
typed-array construction on the host side (`Uint8ClampedArray` straight from the buffer) — a separate
mechanism, recorded rather than folded into a Bar-0 fix.
