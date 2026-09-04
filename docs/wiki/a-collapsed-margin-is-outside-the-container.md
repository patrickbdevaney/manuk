# A collapsed end margin is outside the container

> Landed t1431. Gate: `a_margin_that_collapsed_out_of_the_container_is_not_inside_it`
> (`engine/page/tests/g_collapsed_end_margin_is_outside.rs`), five rows, four of them controls.

## The rule

A block container with **no BFC, no block-end padding and no block-end border** lets its last in-flow
child's end margin collapse straight through its own edge — so the margin is OUTSIDE the container,
and adding it to the scrollable overflow region reports overflow that is not there. The WPT file says
it in its own title: *"scroll{Width,Height} shouldn't account for collapsed margins, in order not to
report unnecessary overflow"*.

```text
                                         chrome sh/ch   before
  display:block; overflow:visible           60 / 60      80 / 60   ← the collapsed-out margin
  …overflow:hidden          (a BFC)        100 / 100    100 / 100  CONTROL
  …padding-bottom: 2px                      82 / 82      82 / 82   CONTROL
  …border-bottom: 3px solid                 80 / 80      80 / 80   CONTROL
  …height: 50px             (definite)      60 / 50      80 / 50   ← NOT a carve-out
```

⚠ **The block axis only — margins do not collapse in the inline axis.** Which physical edge is
block-end depends on the writing mode, so the condition is asked of that edge alone. An earlier
version bundled `margin-right` into the same test and withheld a margin CSS never collapses.

## ⭐ The condition that measurement refused

CSS 2.1 §8.3.1 reads as though a **definite block-size** should stop the collapse, and the first
version of this rule carved it out. The first fixture written for that carve-out used
`height: 200px` — where the client floor is 200 and both answers agree.

> ⭐⭐ **A CONTROL THAT CANNOT FAIL IS NOT A CONTROL.** Re-measured at `height: 50px`, a height the
> content EXCEEDS, Chrome answers **60** — the last child's border box — where the carve-out answers
> 80. The condition was invented, and only a fixture whose floor does not hide the answer could say
> so.

## The half that is measured, better, and REFUSED

The same 140-subtest matrix goes from **90 failing configurations to 45** if containment is judged on
the child's MARGIN box instead of its border box — a 20px box with `margin: 20px` inside a 20px-tall
flex item ends its border box exactly at the item's edge and its margin box 20px past it, so on the
border-box test it counts as contained and contributes its own trailing margin on top.

**It turns three banked gates red** (`g_scroll_overflow_end_margin`,
`g_scroll_overflow_alignment_rect`, `g_scroll_extent_end_padding_containment`), because a margin that
collapses THROUGH an auto-height wrapper leaves that wrapper's own `end_margin` at zero, so the
wrapper's margin box does not cover the child that carries it. **The missing input is the wrapper's
COLLAPSED margin, not a different comparison** — the ratchet refuses the trade, and the next attempt
should publish the used (post-collapse) margin from layout rather than re-deriving it from the style.
