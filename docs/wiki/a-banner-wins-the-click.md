# A banner wins the click

CSS 2.1 Appendix E orders painting inside a stacking context: in-flow blocks, floats and inlines are
steps 3–7, and **positioned descendants with `z-index: auto` or `0` are step 8** — strictly later,
so strictly on top.

`z_index_map` computed `s.z_index.unwrap_or(parent_z)`. An `auto` overlay therefore got its
*parent's* layer — the same one as the content it covers — and `A11yNode::hit_test` broke the tie by
**smaller area**, which the content underneath wins.

⭐ **A cookie banner is exactly this markup**: `position: absolute` or `fixed`, no `z-index`, larger
than what it covers. The agent's coordinate click landed on the link *behind* the banner, and every
layer above it reported success. That is the silent misfire — worse than an error, because nothing
downstream can tell it happened.

```text
                                               Chrome    before    after
  auto-z overlay over an in-flow link            b1        l1        b1
  z-index:5 overlay                              b2        b2        b2   ✓ explicit z already worked
  z-index:-1 underlay                            l3        l3        l3   ✓ negative stays under
  no overlay                       CONTROL       l4        l4        l4   ✓
  4-deep auto chain vs z-index:1                 z1        z1        z1   ✓ the SCALE row
```

**`l2` and `l3` are what make this a missing case rather than a missing feature.** An explicit
`z-index: 5` already won and an explicit `-1` already lost, so the layer machinery worked — only the
`auto` spelling was absent. And `l3` is the row that kills the naive fix: *"positioned beats
in-flow"* applied unconditionally would raise the `z-index: -1` underlay too.

## The encoding

`n * 1024 + 1` for an explicit `z-index`, `parent + 1` for `auto`, unchanged for non-positioned. So
1023 levels of nested `auto` positioning still sit below `z-index: 1`, and an explicit `z-index: 0` —
also step 8 — still clears in-flow content at 1. `TOP_LAYER_Z` (1e9) is unreachable by any z-index a
page would write.

## Two green mutations, and what they named

Bumping `auto` by 1024 instead of 1, and dropping the `* 1024` from the explicit arm, **both passed
the first fixture**. The stated reason for the first was simply wrong: a `z-index: -1` underlay is
negative and can never rise, however large the bump.

Neither a shallow overlay nor a negative one can see the **scale** — only a deep `auto` chain
measured against a *small* explicit `z-index` can, because that is the only place the two encodings
order differently. `Deep vs z1` (Chrome: `z1`) is that row, and it is red under both.

## What this does not fix, measured and named

When the covered content is *itself* positioned with `z-index: auto`, the two are step-8 **peers**,
and the spec orders peers by tree order — later wins. `hit_test` breaks an equal-layer tie by smaller
area instead, so a positioned link under an auto overlay still takes the click. Fixing it needs a
`positioned` bit on `A11yNode`, because area is the right question only for *unrelated* in-flow
boxes — the t853 `<li>`/`<a>` float dust the tie-break was written for.

## ⚠⚠ And `document.elementFromPoint` is a second implementation that ignores z-index entirely

It is a flat scan over the layout rects resolving by smallest area, consulting `pointer-events` and
SVG paintedness but never a layer. It reads `l1=l1 l2=l2 l3=l3 l4=l4` — wrong on **both** overlay
rows, including the explicit `z-index: 5` one the a11y path gets right. Two implementations of one
rule, disagreeing, which is this repo's most-repeated shape. Named with its measurement rather than
fixed in the same tick as a paint-order change, because they need separate controls.

```text
  WPT css/css-position   285 failing -> 285   0 fixed / 0 new   (clean same-hour control)
```

See also [[role-plus-name-is-not-an-address]], [[the-overflow-walk-was-flat]].
