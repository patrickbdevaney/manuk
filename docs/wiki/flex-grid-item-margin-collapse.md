# A flex or grid item establishes an independent formatting context

> Landed t1367. Gate: `a_flex_or_grid_items_child_margin_does_not_collapse_through_it`
> (`agent/tests/`). Every number headless-Chrome-measured.

## The one-sentence mechanism

> **What makes a box a flex or grid ITEM is its parent** — so a predicate that reads the box's own
> computed style cannot see it, and `top_margin_collapses` read only the box's own style.

CSS Flexbox §3: *"the margins of a flex item do not collapse"*; CSS Grid §6 says the same for grid
items. An item is an ordinary `display: block` div, and `establishes_bfc` inspects `ComputedStyle`.
So a first child's `margin-top` collapsed out through the item and off the top of the container, and
the whole subtree moved up by it.

```text
                                             first child dy    the wrapper
  plain block chain (margin collapses)             0               80      CONTROL
  the container is display:grid                   60              140      <- was 0 / 80
  the container is display:flex                   60              140      <- was 0 / 80
  margin directly on a block's first child         0               80      CONTROL
  the same margin, inline-styled, plain chain      0               80      CONTROL
```

⚠ **The three CONTROL rows are the point.** Collapsing is correct for an ordinary block chain and
has to stay — this is a **narrowing** of the predicate, not a removal. A fix that simply stopped
collapsing passes the two grid/flex rows and breaks the other three, and margin collapse is
load-bearing on every ordinary page. That is mutation N2, and it fails a control.

## ⭐⭐⭐ How it was found — five refused hypotheses, then the site's own stylesheet

`www.a11yproject.com` is the worst site on the board's anchor list (shape **43.3%**), and its
`--shape-dump` showed `y +60` on ten elements plus a systematic width shortfall. Five mechanisms were
proposed and **all five were killed by direct measurement against Chrome**:

```text
  1  the fallback FACE differs        REFUTED  sans/serif/mono at 100px: 755.92 / 698.05 / 903.08
                                               in BOTH engines — ratio 1.0000
  2  we apply the site's webfont      REFUTED  Chrome refuses the cross-origin @font-face from a
     and Chrome (CORS) does not                null origin — and so do we: both 755.92
  3  `rem` against a non-16px root    REFUTED  html{font-size:20px}: 2rem/1rem/1.6rem/nested-em
                                               all Chrome-exact
  4  line-box overflow                REFUTED  40px text in a 24px line box: dy -11, box 46 in a
                                               33px line — Chrome-exact
  5  `letter-spacing` (incl. `ch`)    REFUTED  .15ch / 2px / .1em / -.025em / word-spacing / 10ch
                                               — all eight rows Chrome-exact
```

The sixth came from **reading the site's stylesheet instead of guessing**:

```css
.c-homepage-card__image { margin-top: 3rem; … }
html { font-size: 20px }
```

`3rem` at a 20px root is **exactly the 60px the dump was printing**, and the card is a grid
container.

> ⭐ **A dump names a SITE; the site's stylesheet names the MECHANISM.** Four of the five refuted
> hypotheses were guesses about the engine. The one that worked came from grepping the CSS the page
> actually ships, for the number the dump was already printing.

## The receipt

```text
                      before   after    delta
  a11yproject          43.3%   49.3%    +6.0    (absolute placement 10.6% -> 18.0%)
  martinfowler         79.9%   89.8%    +9.9    (absolute placement 16.5% -> 74.5%)
  wikipedia            90.3%   90.1%    -0.2    sample 5205->5207, inside the noise band
  news.ycombinator     99.9%   99.9%     0.0    CONTROL
```

⚠ The wikipedia row is reported as noise rather than a regression **because its element population
moved** (5205 → 5207) — the between-sweep comparison check #104 ruled inadmissible on its own.

⚠ The gate's fixture uses `margin-top: 60px`, not `3rem`. The `rem` is what made the real site's
number 60 and it belongs in the story, but it is not the mechanism, and a gate that can be reddened
by a font-size unit is not a margin-collapse gate.
