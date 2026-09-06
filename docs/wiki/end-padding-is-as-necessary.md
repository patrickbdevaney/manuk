# "As necessary" is load-bearing — the scrollable region's extra padding attaches to the margin box

**Tick 1451.** `css/css-flexbox` **+26**, `css/css-overflow` **+17**, `css/cssom-view` **+1**,
`css/css-grid` flat. The rule t1450 measured, implemented and gated.

## The clause

> CSS Overflow 3: *"additional padding … **as necessary** to enable scroll positions that satisfy the
> requirements of both `place-content: start` and `place-content: end`"*, which *"typically ends up
> being exactly the same size as the box's own padding"*.

Surface audit #85 named "as necessary" and "typically" as **unmeasured residue** — two words the
engine had hard-coded past. t1450 measured them; this tick implements what they mean.

## The rule

> ⭐⭐⭐ **The padding attaches to a child's MARGIN box, and the region is the union of that with the
> child's BORDER box. Where the border box already reaches further, nothing is added.**

```text
  A  padding 1/4/8/16, no border, one 350x10 child, NO margins
     rtl   margin start −234 − 16 = −250   border start −234    → 120 + 250 = 370   padding WINS
  B  the negative-margin-002 wrapper, 300x300 child at margin:-100px
     rtl   margin start   −4 − 16 =  −20   border start −104    → 100 + 104 = 204   border WINS
```

**The two rows disagree about which term wins, and that is the whole rule.** A fixture without
margins, or without a border, cannot tell them apart — which is exactly how the engine came to have
half of it.

## What was actually wrong

`compute_scroll_metrics`'s reversed-axis branch (`x_at_end`/`y_at_end`) took only the **border-box**
start, so a scroller whose content had no margins came out one padding short on every reversed axis:
`direction: rtl`, and every `vertical-rl` block axis. `LayoutBox::scrollable_overflow_start` now
returns both starts — it already took a contribution callback and ignored it (`let _ = contribution;`)
— and the caller takes the further of `border_start` and `margin_start − start_padding`.

## The history this closes

t1449 localised this defect, implemented the correction, watched 174 `cssom-view` subtests fail and
**refused** it. The refusal was right and the contradiction it reported was not: it rested on a
coordinate that t1434 had derived **by hand** (`x = −84`) where Chrome's is `−104`. t1450 measured the
child rect and the contradiction evaporated. The rows that broke then are regression arms `b2`/`b3`
here, and they hold.

> ⭐⭐ **Three ticks: one to find the defect and refuse it, one to measure the number that made the
> refusal look necessary, one to land it.** The middle tick is the one without which this does not
> happen — and its entire content was printing a coordinate nobody had printed.

## Gate

`engine/page/tests/g_end_padding_is_as_necessary.rs` — 9 rows over three batteries (a plain block, the
negative-margin wrapper, a flex container), 3 controls and 2 regression arms. Red under Z1 (drop the
margin-box term → the four no-margin reversed rows), Z2 (drop the `min` against the border box →
`b2`/`b3` read 120), Z3 (use `padding-right` as the start padding → `a2`/`a3` read 358, which only an
ASYMMETRIC padding can show) and Z4 (drop `start_margin` → `b2`/`b3` read 220 by the other route).
