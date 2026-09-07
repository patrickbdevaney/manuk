# Clickability does not measure paint order

t1468 replaced the hit-test's equal-layer area tie-break with tree order and was refused at **62
unclickable links**. Its own conclusion was that area had been a proxy for the whole of CSS 2.1
Appendix E's in-flow steps, and that closing the step-8 peer case needed those steps modelled.

t1475 modelled them — four sub-ranks inside each stacking context — and measured every variant
against the same oracle, now running in two seconds:

```text
                                                   missed / 477    clickability
  baseline (area tie-break)                              0            100.0%
  block 0 · float +1 · inline +2 · positioned +3         86             76.8%
  …without the inline rank                              351              5.1%
  …without the float rank                                98             73.5%
  …inline rank that does not accumulate                 331             10.5%
```

**Every variant is far worse, and two of them are catastrophic.** A model strictly closer to the
specification scores an order of magnitude worse on the metric. That is the finding.

## Why: the metric is defined over the ancestor chain, not the pixel

G6 asks *"can the browser find this link?"*, and the way it finds one is stated in the hit-test's own
t853 comment:

> the shell walks **up** from whatever was hit looking for an `<a href>`, and an ancestor `<li>` has
> no link above it

So the metric is not "did the topmost box win". It is **"is there an `<a href>` on the ancestor chain
of whatever won"**. Those come apart precisely when a rank change hands the click to a *sibling*
subtree instead of a nested one — and every in-flow sub-rank does exactly that, because steps 4–7
reorder boxes that are siblings of each other rather than nested.

⭐⭐ **The area tie-break is not a proxy for Appendix E. It is a proxy for CONTAINMENT** — the smaller
box is usually the one *inside* the other, and the ancestor walk needs the innermost box precisely
because that is the one with the link above it. Ordering by paint correctness discards containment,
and containment is what the metric is made of.

That single mechanism explains all five numbers, including t1468's 62.

## What this means for the peer case

The step-8 peer case is not reachable by reordering the hit-test at all: any ordering change that is
paint-correct is containment-incorrect somewhere, and the metric measures containment. Closing it
needs the hit-test to resolve **containment first and paint order second** — i.e. keep the structural
ancestor/descendant resolution and apply paint rank *only* between candidates where neither contains
the other, which is a different shape from a comparator.

⚠ Until then the peer case stays open with its price known: **−62 to −351 clickable links**,
depending on how much of Appendix E is modelled.

## The instrument is the reusable part

Four wrong models were measured and discarded in one tick because the oracle takes two seconds
(t1472). The same four would have cost four ticks and ~3½ hours of wall time a day earlier. **When a
gate refuses you twice, make it local before attempting a third time.**

See also [[the-area-tie-break-was-a-proxy]], [[the-refusing-oracle-runs-in-two-seconds]].
