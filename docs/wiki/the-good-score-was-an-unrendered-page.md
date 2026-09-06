# The good score was an unrendered page

t1461 moved the agent's browser from `Page::load` to `Page::load_async` and Track B's node-match F1
went **76.6% → 94.8%**. The bar is `>=90%`, so the loop recorded the bar as met.

t1467 then measured the *next* phase, `finish_loading`, at **94.8% → 82.2%** and refused it on the
ratchet: recall improved, precision collapsed, and Wikipedia's excess was `486 listitem ""` — read as
"`finish_loading` undoes the collapse".

**Both readings were wrong, and the second one was wrong because the first was.**

## The measurement that settles it

Count how many of Wikipedia's list items have a layout box at all:

```text
                          <li> in DOM    with a box    zero-size
  without finish_loading      770            126           644
  with finish_loading         776            614           162
```

⭐⭐⭐ **Without `finish_loading` the page is not rendered — 84% of its list items have no box.** The
accessibility tree excludes boxless nodes, so the "good" 94.8% was measuring a browser that had not
laid out most of the page. The denominator was suppressed by **under-rendering**, not by correctly
hiding anything.

And the collapse hypothesis was false in both directions. The DOM is nearly identical either way
(770 vs 776 `<li>`), and in *neither* configuration does MediaWiki's collapsible run:

```text
  ready=complete | jq=function | mkc=function | mkcState=ready | citeState=ready | hook=yes
  navbox=3 | collapsible=13 | collapsed=13 | content=0 | toggles=0
```

`jQuery.fn.makeCollapsible` **exists and is ready** — and there are zero `.mw-collapsible-content`
wrappers and zero toggles, so it was never *called* on the content. The `mw-collapsed` class is
server-side and hides nothing by itself. Chrome's tree is smaller than ours because Chrome runs that
initialisation and we do not.

## So the drop is a change of instrument, not of engine

`finish_loading` is adopted. The engine strictly gains: a rendered page, completed module loaders,
488 more laid-out list items, and the 42 `aria-label="Jump up"` names MediaWiki's cite enhancement
adds. The score falls because it is now being taken on a page that exists.

This is the same class of correction `CONSTITUTION-CHECK` already records for the t1023 sweep
re-baseline — *"the −2.1 points is a change of instrument, not of engine, and check #93's steer #1
required it be labelled one."*

```text
                     precision   recall      F1
  t1458 (no JS)          63.5%    96.4%    76.6%
  t1461 (JS, unrendered) 93.2%    96.4%    94.8%   ← withdrawn: 84% of the page had no box
  t1470 (rendered)       70.9%    97.2%    82.0%   ← the honest number
```

## Where the remaining gap actually is

```text
  blog.rust-lang.org   99.9%      danluu.com          99.8%
  news.ycombinator     97.9%      www.a11yproject     97.5%
  martinfowler.com     89.5%      en.wikipedia.org    48.5%   ← the whole gap
```

Five of six sites are 89.5–99.9% F1. **Wikipedia alone is 48.5%**, and its excess is the navbox
content Chrome hides. One named mechanism — `makeCollapsible` is loaded, ready, and never invoked —
stands between 82.0% and something close to the pooled rest.

⚠ **Track B's `>=90%` bar is NOT met.** It was reported as met for nine ticks on a number taken from
an unrendered page.

See also [[one-phase-short-and-the-price-of-the-next-one]],
[[the-agents-browser-had-no-javascript]], [[recall-is-not-node-match]].
