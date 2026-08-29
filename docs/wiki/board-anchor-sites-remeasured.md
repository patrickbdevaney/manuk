# The board's ranked anchor sites, re-measured — five of six already clear the bar

> Measured t1367 with `manuk-wpt fidelity`, before that tick's own fix.

## Why this matters more than the numbers

The lever board's CO-#1 block says: *"RANKED BURNDOWN: ONE primitive per tick, ranked by (in-scope
sites × dy severity), **verified on the anchor sites** news.ycombinator 0.72 / a11yproject 0.44 /
wikipedia 0.52 / blog.rust-lang 0.63 / martinfowler 0.58 / whatwg 0.51."*

Those figures are from **2026-07-29**. Measured today:

```text
                          board says   today    delta    verdict
  news.ycombinator.com       0.72       0.999   +0.28    DONE       (807 elements scored)
  whatwg.org                 0.51       1.000   +0.49    DONE       (37)
  blog.rust-lang.org         0.63       1.000   +0.37    DONE       (1684)
  en.wikipedia.org           0.52       0.903   +0.38    clears 0.75 (5205)
  martinfowler.com           0.58       0.799   +0.22    clears 0.75 (333)
  www.a11yproject.com        0.44       0.433   -0.01    THE ONLY ONE STILL FAILING (217)
```

> ⭐⭐⭐ **Five of the six anchors now clear the 0.75 bar and three are essentially perfect, while
> the loop has been ranking its render work against the list as written.**

This is t1362's finding — *a "NOT BUILT" entry is a claim about the present tense, and nothing
re-runs it* — applied to the **steering list itself**, which is strictly higher leverage than a
backlog: a stale backlog wastes a tick, a stale ranking list mis-aims every tick that consults it. It
also explains a run of Track A ticks that kept measuring named defects and finding them already
correct.

## What it changes about a11yproject

The outlier is not "one of six hard sites", it is **the single site that has not moved** — every
other anchor gained 22–49 points while it gained −1. That reframes it: its gap is far more likely to
be one or two specific mechanisms than general layout debt, which is exactly what the evidence then
showed. Five plausible general causes (font face, webfont CORS, `rem`, line-box overflow,
`letter-spacing`) were each refuted by direct Chrome measurement, and the actual cause was a single
CSS rule in the site's own stylesheet — see
[flex/grid item margin collapse](./flex-grid-item-margin-collapse.md), which took it 43.3% → 49.3%
and martinfowler 79.9% → 89.8% in the same change.

## ⚠ Scope

The anchor list lives in `scripts/lever-board.sh`, which is **observer-owned**. The agent does not
edit it. The measurement is recorded here and in the tick's journal entry so the list can be
re-ranked deliberately; until it is, a reader of CO-#1 should treat `a11yproject` as the anchor and
the other five as banked.

⚠ And the standing caution applies to this table too: it is **one run per site**, so each figure
carries the population caveat from check #104 (a shape mean is only comparable when the scored
element set has not moved). The three at ~100% and the one at 43% are far outside any plausible
noise band; wikipedia's and martinfowler's would want a second run before being banked to a decimal.
