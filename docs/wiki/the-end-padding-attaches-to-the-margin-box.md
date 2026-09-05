# The end padding attaches to the MARGIN box — the model that resolves t1449's contradiction

**Tick 1450 — measurement.** t1449 refused a fix because two Chrome-measured batteries disagreed. They
do not. One of them had been read with an arithmetic error since t1434, and correcting it makes a
single rule fit every row.

## The four measurements, with the child's own rect

Coordinates are relative to the container's **padding box**; both containers are `overflow: scroll`.

```text
  A. padding 1/4/8/16, no border, one 350x10 child, no margins
     ltr   kid[16,1 350x10]     kid right 366   padding box right 120   scrollWidth 370
     rtl   kid[-234,1 350x10]   kid left −234                            scrollWidth 370

  B. the `negative-margin-002` wrapper: padding 1/4/8/16, border 1/50/40/4,
     one 300x300 child at `margin: -100px`
     ltr   kid[-84,-99 300x300]  kid right 216   padding box right 100   scrollWidth 216
     rtl   kid[-104,-99 300x300] kid left −104                           scrollWidth 204
```

## The rule that fits all four

> ⭐⭐⭐ **The end padding attaches to the child's MARGIN box, and the region is the union of that with
> the child's BORDER box.** Where the border box already reaches further, no padding is added — which
> is exactly the specification's *"additional padding … **as necessary** to enable scroll positions
> that satisfy the requirements of both `place-content: start` and `place-content: end`"*.

```text
  A ltr   margin-box right 366 + padding-right 4 = 370   border-box right 366   → 370  ✓
  B ltr   margin-box right 116 + padding-right 4 = 120   border-box right 216   → 216  ✓
  A rtl   margin-box left −234 − padding-left 16 = −250  border-box left −234   → 120+250 = 370  ✓
  B rtl   margin-box left  −4  − padding-left 16 = −20   border-box left −104   → 100+104 = 204  ✓
```

The `as necessary` clause was named as unmeasured residue by **surface audit #85** and again by check
#136. This is it, measured.

## ⚠ THE RECORD WAS WRONG, AND THE WRONG NUMBER BECAME A RULE

t1434's journal derived the RTL child's position **by hand** as `x = −84` and concluded that Chrome's
204 must be `184 + 20`, where 20 is *both* of the container's paddings. Chrome's actual child position
is **−104**, and `100 + 104 = 204` exactly — **there is no `+20` and there never was.**

> ⭐⭐ **A hand-derived coordinate became a rule, and the rule then made a correct fix look like a
> regression.** t1449 measured a real defect, implemented it correctly, saw 174 subtests fail, and
> refused — because the battery it appeared to contradict was being read through a number nobody had
> measured. *The refusal was right; the contradiction was not real.*

The lesson is not "derive more carefully". It is that **a coordinate is a measurement, and this loop
has a Chrome to measure it with** — the same instrument that produced every other row in that table.

## What the next tick implements

`compute_scroll_metrics`'s reversed-axis branch (`x_at_end` / `y_at_end`) needs the **margin-box**
start extent as well as the border-box one it already computes, and takes the further of
`border_box_start` and `margin_box_start − end_padding`. The walk already carries per-contribution
margins and padding (`OverflowContribution`); what it does not yet expose is the START-side pair.

## Status

Model resolved and fitted to four rows; the defect t1449 localised is unchanged and still real. Nothing
landed in the engine.
