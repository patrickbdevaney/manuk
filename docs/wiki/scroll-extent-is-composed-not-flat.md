# The scrollable overflow walk was flat, so a grandchild sailed past its parent's negative margin

> Landed t1417. Gate: `a_negative_end_margin_clamps_its_subtrees_scroll_contribution`
> (`engine/page/tests/g_negative_margin_scroll_extent.rs`), 3 arms, red under 2 of 4 mutations — and
> it says which two it cannot catch, and why.

Located by the t1416 concentration survey: `css/cssom-view`'s 1,480 failing subtests are 769 subtests
of one rule, and the WPT file names it in its own title — *"scroll{Width,Height} shouldn't account for
collapsed margins, in order not to report unnecessary overflow."*

## ⭐⭐⭐ The layout was never wrong

```text
                     chrome                        before
  children's rects   [13,33][28,48][43,63][58,78]  IDENTICAL
  clientHeight       75                            75        IDENTICAL
  scrollHeight       75                            80        ← 5px of overflow that is not there
```

Every box in the right place, the client box right, the extent wrong. **The walk was flat**: each
descendant was measured directly against the scroll container, so the inner 20px box contributed its
own bottom (78) and its parent's `margin-bottom: -5px` never applied to it. The parent's margin box
ends at 73 — exactly the content-box bottom — which is why Chrome reports no overflow at all.

The fix clamps a subtree's contribution to its parent's margin box **when that parent's end margin is
negative**. Scoped deliberately: a positive end margin genuinely extends the region (t1119's rule,
held by `g_scroll_overflow_end_margin`). `css/cssom-view` 563 → **602**, same binary both ways.

## ⚠⚠ The rule underneath, found while building the arms

```text
  height:30px; padding:10px scroller > a 10px child > a 60px box
      chrome  scrollHeight 70        ours  80
```

A grandchild that already overflows gets the container's **end padding** added on top, and Chrome does
not add it. That is an older, separate rule — and it is the only shape that distinguishes *"clamp only
negative margins"* from *"clamp every margin"*. So the arm this gate most wants cannot be written yet:
Chrome's answer would make it red for a defect this tick does not fix, and our answer would pin the
engine to a bug.

> ⭐⭐ **A gate that names what it cannot catch is worth more than one that pretends.** Two green
> mutations here are the measurement that says which fix comes next — the end-padding rule is now a
> located, Chrome-measured work item rather than a suspicion.

## t1418 — and the end padding belongs to the content the container CONTAINS

```text
                                                         chrome   before
  the filler is the DIRECT CHILD                           120      120     ✓ t1258's rule
  a GRANDCHILD of a 10px-tall wrapper                      110      120     ← the +10 again
  a GREAT-GRANDCHILD, same shape                           110      120
  an AUTO-HEIGHT wrapper > 100px child + 30px margin       150      150     ⭐ the padding DOES apply
  the HORIZONTAL axis: 10px wrapper > an 80px box           90      100
```

⭐⭐⭐ **The rule is not DEPTH, and an existing gate refused that in one run.** The first fix was
*"only a direct child gets the end padding"*; it satisfied six rows and turned
`g_scroll_overflow_end_margin` red on a Chrome-measured counterexample that had been in the tree since
t1119 — *"an auto-height wrapper whose inner child carries the margin"*, expecting 270 where depth
gives 260.

**The discriminator is containment.** An auto-height wrapper grows to contain its child, so the child
is the scroller's in-flow content and the padding applies; a fixed-height wrapper whose child
overflows it is a different thing, and the overflowing part gets nothing. **A plausible rule that fits
every fixture you happened to write is the most expensive kind of wrong, and the gate that refused the
proxy is the only reason the right rule was found.**

This also closed the gap t1417 named: the arm t1417 had to move out is back, and
`g_negative_margin_scroll_extent` is now red under the mutation it previously could not see.
