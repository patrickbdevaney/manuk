# Two Chrome-measured batteries that cannot both be satisfied by one end-padding rule

**Tick 1449 — REFUSED.** `css/css-flexbox` +26 and `css/cssom-view` **−174** (all 174 in one file).
Reverted. The product is the conflict, stated precisely.

## The defect that is real

`compute_scroll_metrics` takes the scroll container's end padding as
`(padding-right, padding-bottom)` — unconditionally. That is the axis end only while the scroll origin
is top-left. Reverse either axis (`direction: rtl`, or a `vertical-rl` block axis) and the region's end
is the LEFT or TOP edge, so the padding that belongs there is `padding-left`/`padding-top` and the one
that was added lands on the unreachable side, where it is discarded.

The simplest witness is not a flex box at all:

```text
  display:block; writing-mode:vertical-rl; overflow:scroll; 100x100; padding:10px
  around one 350px-wide child                      Chrome 370      ours 360
```

## The fix that works, and the battery it breaks

Adding the origin-side padding in the reversed branch makes **eight of ten** probe rows Chrome-exact,
including four that were wrong:

```text
                                          Chrome     before     after
  horizontal-tb ltr        CONTROL        370/130    370/130    370/130  ✓
  direction: rtl                          370/130    360/130    370/130
  writing-mode: vertical-rl               130/370    120/370    130/370
  vertical-rl + rtl                       130/370    120/360    130/370
  BLOCK, vertical-rl, 350px child         370/130    360/130    370/130
```

`css/css-flexbox` gains **+26**. And `cssom-view/scrollWidthHeight-negative-margin-002` — the
600-subtest matrix this session drove from 420/600 to **600/600** across ticks 1434–1439 — falls to
**426/600**. All 174 regressions are in that one file.

> ⭐⭐⭐ **BOTH BATTERIES ARE CHROME-MEASURED AND THEY CANNOT BOTH BE SATISFIED BY THIS RULE.** One says
> a reversed axis gains its origin-side padding; the other says the same reversed axis is already
> correct without it. That is not a bug in either measurement — it is a statement that **the end-padding
> term depends on something neither battery varies**, and the rule as written is under-determined.

## What the next tick must do FIRST

Not implement. **Find the variable the two batteries hold constant in different places.** The obvious
candidates, and each is one probe:

* The two fixtures differ in whether the region's far edge comes from a CONTRIBUTION (the
  negative-margin matrix reaches its extent through a child's box) or from the PADDING BOX floor (the
  flexbox matrix overflows past it). The end padding may attach to one and not the other.
* `negative-margin-002`'s padding is asymmetric (`1px 4px 8px 16px`) and the flexbox matrix's is
  uniform (`10px`). An asymmetric fixture can tell `padding-left` from `padding-right`; a uniform one
  cannot — **so the flexbox battery cannot say WHICH padding it needs, only that it needs ten.**

⚠ That second point is the more likely resolution and it is the session's own recurring lesson wearing
a new hat: *a uniform fixture cannot tell two sides apart.* Re-run the flexbox battery with asymmetric
padding before believing either rule.

## Status

Open. The defect is real and localised; the rule is not yet determined. Nothing landed.
