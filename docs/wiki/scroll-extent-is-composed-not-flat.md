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
