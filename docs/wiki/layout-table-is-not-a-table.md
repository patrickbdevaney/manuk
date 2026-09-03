# A layout table is not a table — and the metric that measured it could not agree with itself

> Landed t1405. Gate: `a_layout_table_is_not_announced_as_a_table`
> (`agent/tests/g_a11y_layout_table_is_not_a_table.rs`), 25 rows, red under 7 mutations. Oracle = CDP
> `Accessibility.getFullAXTree`.

## The rule, every row measured

Chrome does not expose a header-less, border-less, small `<table>` with the table roles at all — it
demotes the subtree to `LayoutTable`/`LayoutTableRow`/`LayoutTableCell`, which no assistive technology
reads as tabular. We announced every one as `table`/`row`/`cell`.

```text
  DATA                                        LAYOUT
  role=table | grid | treegrid                nothing at all
  a <caption>                                 a <tbody> and nothing else  ⚠ every table has one
  a <th>                                      aria-label alone  ⚠ names it, does not type it
  summary=                                    headers= on a <td>
  <thead> or <tfoot>                          width:100%
  <colgroup> / <col>                          role=presentation, even WITH a <th>
  >= 20 rows                                  <= 19 rows
  a border (attribute or CSS) AND >1 cell     a border on a 1x1 table  ⚠ BOTH spellings
```

A demoted node is **dropped and its children reparented**, exactly as `role=presentation` is — so the
links and text inside a layout table survive; only the false tabular structure goes. The scan **stops
at a nested `<table>`**: layout tables nest, and a data table inside one must not make its container
data, nor the reverse.

## ⭐⭐⭐ The fixture suite proved the rule I wrote; the corpus found the rule I did not

The first version had every markup signal above, no size rule, and passed all eighteen Chrome-measured
fixture rows. Then the corpus:

```text
  blog.rust-lang.org node match   99.9%  ->  27.6%     (1,211 real nodes eaten)
```

That page's post archive is **one `<table>` with 403 rows and 806 cells** — no `<th>`, no `<caption>`,
no border — and Chrome calls it data. Bisecting Chrome on borderless header-less tables of
2 / 4 / 10 / 19 / 20 / 21 / 25 rows found the term exactly: **twenty rows.**

> **A fixture suite can only falsify the rule you wrote.** The corpus is not a slower fixture; it is
> the only instrument that can find a MISSING clause.

## ⚠ The price, recorded because it was taken in the wrong order

52 freshly-fetched CrUX corpus pages carry 6 `<table>`s between them and exactly **1** is a layout
table (1.9% of pages). The loop's own rule is *price the mechanism on the corpus before building*;
this was priced after. It is a **correctness** tick with a Chrome-arbitrated rule, not a corpus-moving
one, and it says so out loud rather than quoting a number it did not move.

## ⭐⭐⭐ The instrument half: a real-site metric needs a self-agreement control

`news.ycombinator.com`'s residual misses read `'201 comments'` vs `'219 comments'` and `'1 hour ago'`
vs `'5 hours ago'` — **the page changed between the two fetches.** So Chrome was run against Chrome,
same page, two fetches:

```text
  news.ycombinator.com   chrome vs chrome    85.3%    <- and we scored 86.7% against it
  danluu.com             chrome vs chrome   100.0%
```

**A real-site metric without a self-agreement control cannot tell an engine defect from a page that
changed.** On a live feed we were scoring above Chrome's agreement with itself; that page's number is
noise, and the six-page aggregate is a **lower bound**. Run the control row before quoting any
real-site number, and quote it beside the number.

## ⭐⭐ And a multiset match over Chrome's nodes is RECALL only

It cannot see a node we invent — and announcing a layout table adds a whole spurious subtree.
Precision, measured for the first time:

```text
  danluu.com          414 nodes    0 extra   100.0%
  blog.rust-lang.org 1678 nodes    7 extra    99.6%
  whatwg.org           32 nodes    1 extra    96.9%
  a11yproject.com     173 nodes   28 extra    83.8%
  martinfowler.com    427 nodes  138 extra    67.7%   <- reads 97.3% on recall
  AGGREGATE          3277 nodes  302 extra    90.8%
```

Both numbers are true; only one was being reported.
