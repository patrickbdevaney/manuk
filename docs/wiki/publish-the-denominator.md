# Publish the denominator

The accessibility score counts nodes with a layout box. So a page that fails to lay out emits fewer
phantoms, its omissions never enter the comparison, and **precision rises**. A browser that renders
less scores better.

That is not hypothetical. It cost nine ticks:

```text
                         <li> in DOM    with a box       reported F1
  without finish_loading      770            126             94.8%   ← reported as "bar met"
  with finish_loading         776            614             82.0%   ← honest
```

Nothing in the output said which of the two you were looking at. Surface audit #89 ranked fixing
that #1 — *"the next under-rendering regression flatters itself the same way"* — because closing the
**mechanism** outlives closing the instance.

## The column

`Rendered { nodes, boxed }`, printed beside every rate:

```text
  url                     manuk  chrome  match    prec   recall     F1   rendered
  martinfowler.com          360     297    294   81.7%    99.0%  89.5%      97.8%
  news.ycombinator.com      477     495    476   99.8%    96.2%  97.9%      95.7%
  blog.rust-lang.org       1673    1673   1672   99.9%    99.9%  99.9%      99.9%
  www.a11yproject.com       158     158    154   97.5%    97.5%  97.5%      94.8%
  danluu.com                414     416    414  100.0%    99.5%  99.8%      99.4%
  en.wikipedia.org/…       2191     779    726   33.1%    93.2%  48.9%      96.1%
  TOTAL                    5273    3818   3736   70.9%    97.9%  82.2%      97.4%
```

⭐ **It earns its place on the first run.** Wikipedia reads **33.1% precision at 96.1% rendered** —
so its gap is *phantoms*, not under-rendering, and no reader has to know the history to see that. A
month ago the same site read 93.2% precision at what would have been ~16% rendered, and the column
would have said so at a glance.

**A precision that rises while `rendered` falls is not an improvement.**

## What it is, exactly

It counts nodes the accessibility tree kept, not DOM elements — this crate cannot see the DOM, and
the tree is what the score is computed over. So it is a **lower bound** on under-rendering: a page
can be under-rendered in ways this does not catch. Stated rather than implied, because a
lower-bounded instrument that reads as exact is how the first mistake happened.

⚠ A zero-area box counts as *not laid out*, which is what separated the two cases in the measurement
above — 126 of Wikipedia's 770 list items had a box and the rest were zero-area, not absent.

See also [[the-good-score-was-an-unrendered-page]], [[recall-is-not-node-match]].
