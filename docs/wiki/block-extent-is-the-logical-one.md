# A container's BLOCK extent is its CSS `width` in a vertical writing mode — 274 subtests on one predicate

**Tick 1445.** `css/css-grid` failing **3566 → 3429 (−137)** and `css/css-flexbox` failing
**1328 → 1191 (−137)**; `css/css-sizing`, `css/cssom-view` and `css/css-writing-modes` flat. **276
fixed, 2 index-swaps inside files whose score did not change.**

## The defect

Everything `solve_subtree` is handed is in the container's own **logical** space (t1438): `cw` is
already the logical **inline** size — the physical *height* for a vertical container — and the block
size beside it was still read straight off the CSS `height`.

```rust
let container_h = match self.style_of(node).height { Dim::Px(p) => Some(p), _ => None };
```

So an orthogonal grid distributed its rows down 300px of physical height while Chrome distributed them
across 400px of physical width.

```text
  width:400px; height:300px; grid-auto-rows:40px      Chrome    before    after
  vertical-lr   align-content: space-between           360,0     260,0     360,0
  vertical-lr   align-content: center                  200,0     150,0     200,0
  vertical-lr   align-content: end                     360,0     260,0     360,0
  vertical-rl   align-content: space-between             0,0     100,0       0,0
  vertical-lr   FLEX column, justify-content: s-b      360,0     260,0     360,0
  horizontal-tb align-content            CONTROL        0,260     0,260     0,260  ✓
  vertical-lr   JUSTIFY-content          CONTROL       80,0      80,0      80,0    ✓
  vertical-lr   width:auto               CONTROL       40,0      40,0      40,0    ✓
```

> ⭐⭐ **`260 = 300 − 40` and `360 = 400 − 40`.** The free space was measured against the wrong physical
> extent, so `end` was wrong by the whole difference and `center` by exactly half of it. **One
> arithmetic tell across three alignment values is one cause, not three.**

## The shape, and it is this file's most-repeated one

> ⭐ **One of the pair was mapped and the other was not.** The `justify-content` control in the *same*
> vertical container was already exact — the INLINE size had been transposed correctly for as long as
> orthogonal layout has existed here, and the block size beside it never was.

This is the residue t1438 named and did not fix, in its own words: *"`cross_size` is still read from
`solved_h`, and for an orthogonal container that is the CSS `height` — a physical length pinned as if
it were the logical block size."* Two ticks later it was worth **274 subtests across two areas**.

## Why it moved two areas

`flex-direction: column` in `vertical-lr` puts the flex MAIN axis on the block axis, so
`justify-content` distributes over the same width a grid's `align-content` does. One predicate, two
formatting contexts.

## Gate

`engine/page/tests/g_block_extent_is_the_logical_one.rs` — 8 rows, 3 controls. Red under X1 (read
`height` unconditionally → six rows), X2 (read `width` unconditionally → the horizontal control alone)
and X3 (key on `is_rl()` instead of `is_vertical()` → every `vertical-lr` row fails while `vertical-rl`
passes, which is exactly the pair that separates the two predicates).

⚠ The `width: auto` control is load-bearing: an indefinite block size must stay indefinite, or the
container collapses instead of sizing to its content.
