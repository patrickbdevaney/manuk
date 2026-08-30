# A trailing margin is inside the scrollable overflow region

> Landed t1381. Gate: `a_trailing_margin_is_inside_the_scrollable_overflow_region`
> (`agent/tests/g_scroll_overflow_end_margin.rs`), fourteen rows, headless-Chrome-measured.
> WPT `css/css-overflow` 496 → 508.

## The defect

`scrollHeight` was short by the last child's `margin-bottom`, on every scroll container on the web.

```text
  <div style="width:100px;height:100px;padding:10px 5px;overflow:scroll">
    <div style="height:200px;margin-bottom:50px"></div>
  </div>
                          chrome   before   after
    scrollHeight            270      220      270
```

`scrollTop + clientHeight >= scrollHeight` is the *"am I at the bottom"* test every infinite
scroller, lazy-image loader and virtualised list runs. A short `scrollHeight` makes it true too
early.

## The mechanism

`content_extent` unions the BORDER BOXES of the descendants. **A trailing margin with nothing after
it is invisible to that union**: every box that follows a margin has already accounted for it by
sitting lower down, and nothing follows the last one. A scroll container establishes a BFC, so that
last margin does not collapse out — it is real space inside the container.

The fix inflates each descendant's contribution by its END margins (right and bottom), through a
closure so `manuk-layout` does not need the style map and every other `content_extent` caller is
unchanged.

## Found by SURVEYING the area, not grinding it

`css/css-overflow` is the board's lowest-pass ★ CSS-LAYOUT row. Its 457 failing subtests decompose:

```text
  scrollHeight / scrollWidth wrong                                     117   ← this family
  `scroll-marker*`, `scroll-buttons`, `scroll-target-group`,
  `scroll-axis-lock`, `line-clamp` (the Overflow-4 shorthand),
  `max-lines`, `continue` — properties of an UNSHIPPED spec level     ~150   FRONTIER, refused
  `overflow-clip-margin`, `overflow-block/inline`, `block-ellipsis`    ~30
  the rest (promise rejections, querySelector throws, serialization)  ~160
```

> **An area percentage is not a work item.** Half of this "★ CSS-LAYOUT, build now" row is a spec
> level no engine ships, which no amount of grinding converts into daily-driver capability.

## The battery

```text
                                                        chrome   before   after
  c1  one 200px-tall child                    CONTROL     220      220      220
  c2  …with margin-bottom: 50px                           270      220      270
  c3  a 200px-WIDE child, margin-right: 50px  (scrollW)   260      210      260
  c5  a 0-height sibling AFTER it              CONTROL    270      270      270
  c6  a 200px-wide child, no margin  (scrollW) CONTROL    210      210      210
  d3  a child with margin-TOP: 50px            CONTROL    270      270      270
  d4  a child with margin-LEFT: 50px (scrollW) CONTROL    260      260      260
  d6  a FLOATED child with margin-bottom: 50px            270      220      270
  d7  …with margin-bottom: -30px                          190      220      190
  d9  a 0-height child after the margined one  CONTROL    270      270      270
  e3  a RELATIVE child (no offset), margin 50             270      220      270
  e5  two children, margins 50 and 70                     340      270      340
  e6  an inline-block child, margin-bottom 50             270      220      270
  f1  an AUTO-height wrapper, inner margin 50             270      220      270
```

⭐⭐ **`d7` — the NEGATIVE margin — makes this an INFLATION and not a `max`.** Chrome reports 190,
not 220: a negative end margin pulls the region IN. A `.max(bottom)` guard keeps the larger wrong
answer on every negative-margin card deck, and it is the mutation a reader adds to "be safe".

⭐⭐ **`d3`/`d4` — the START margins — are the control that says this is an END rule.** A start
margin already moved the box along the flow, so it is in its border box's POSITION; adding it again
double-counts it. Both rows read the same number in both engines precisely because nothing was added.

⭐ **`c5`/`d9` say the union was not simply broken.** A margin with a sibling AFTER it was always
counted. Only the LAST one was lost — which is why this was invisible to any fixture with a footer.

⚠ `scrollWidth` rows are chosen so the content is WIDER than the client box: our engine reserves a
scrollbar gutter and Chrome was measured with `--hide-scrollbars`, so a row whose content fits inside
the client box reports that floor and would compare two scrollbar policies, not two overflow regions.

⚠ Percentage margins resolve against each descendant's containing block; the caller has the scroll
container's own width. Named residue, alongside the percentage-padding one already recorded there.

## ⚠ Named, measured, not built — the other half of the 117

```text
                                                            chrome   ours
  a child at `position:relative; top:-1000px`, margin 50       270    105
    (scrollable-overflow-padding.html 30 subtests +
     scrollable-overflow-transform-unreachable-region.html 58)

  NESTED PROPAGATION, measured in the same pass:
  a 200px child inside a `width:0;height:0` wrapper           120    220
  …inside a `width:10px;height:0` wrapper                     210    220
  …inside a `width:10px;height:20px` wrapper                  210    220
  …inside a `width:0;height:0; overflow:hidden` wrapper       120    220
  an auto-height wrapper, inner margin-bottom: 50  CONTROL    270    270  ✓
```

The first is the **alignment rectangle** (Blink's *"inflow-bounds"*): a relatively-positioned box
contributes its ORIGINAL in-flow position to the region as well as its offset one, so moving it to
`top:-1000px` does not shrink the scroller. That needs layout to record a pre-offset rect — a
different mechanism, 88 WPT subtests, and the ranked next tick rather than something to bolt on.

⚠⚠ **The honest half: two of the nested rows MOVED, from 220 to 270, against Chrome's 120.** They
are the ones with a **zero-width intermediate wrapper** — Chrome propagates no scrollable overflow
through one, we propagate all of it, and the margin this tick adds rides along on a contribution that
should not have been there. Both were already wrong before (220 vs 120); this is the pre-existing
nested-propagation gap unmasked further, named here with its numbers. The realistic nested shape (an
auto-height wrapper whose inner child carries the margin) is `f1`, and it is Chrome-exact.

## How it was proven red

- **N1** — ignore the closure (the pre-tick `content_extent`): seven rows fail, every control green.
- **N2** — clamp the inflation to non-negative: only `d7`, at 220 against Chrome's 190.
- **N3** — inflate by the START margins too: `c5` reads 320 against 270, its second child's
  `margin-top` counted once in the position and once again here.
