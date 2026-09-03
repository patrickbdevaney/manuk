# Name-from-content, and the one role whose answer depends on where it is

> Landed t1404. Gate: `listitem_is_never_named_from_content_and_row_is_only_inside_a_grid`
> (`agent/tests/g_a11y_name_from_content_context.rs`), 30 rows, oracle = CDP
> `Accessibility.getFullAXTree`. **Real-site a11y node match 75.0% → 97.0%** over six corpus pages;
> WPT `wai-aria` 399 → **400**, `accname` and `html-aam` unchanged.

## First: the tree had never been measured against a real site

Track B's `>=90% node match` bar was quoted from WPT `wai-aria` / `html-aam` / `accname` (91.9 / 94.0
/ 91.9%). Constitution check #131 had already written down why that is a different claim — Interop
2026 lists accessibility testing as an **investigation effort**, i.e. *no suite can decide this yet* —
and two ticks (t1379, t1380) had found shipping defects that moved those rows by zero.
`docs/loop/V1-SCOPE.md`'s completion bar for the agentic surface says *"measured vs the same real-site
corpus"*. So it was:

```text
  page                    chrome nodes   before    after
  danluu.com                       414    51.2%   100.0%
  a11yproject.com                  162    67.9%    89.5%
  blog.rust-lang.org              1673    74.9%    99.9%
  news.ycombinator.com             490    84.5%    86.7%
  whatwg.org                        32    90.6%    96.9%
  martinfowler.com                 297    95.6%    97.3%
  AGGREGATE                       3068    75.0%    97.0%
```

Method: `a11y-dump <url>` (this repo, `agent/src/bin/`) against `Accessibility.getFullAXTree`, matched
as a `(role, name)` multiset. Chrome's `StaticText`/`InlineTextBox`/`ListMarker`/`LabelText` and
both sides' unnamed `generic` containers are dropped — **symmetrically**, because counting them scores
a modelling difference rather than a correctness one, in both directions.

## 721 of the 766 missed nodes — 94% — were one expression

`Role::name_from_content()` listed `ListItem` and `Row`. Chrome names both `""`:

```text
  462 x ('row', '')          every table row announced as its whole row of text
  259 x ('listitem', '')     every <li> announced as its sentence instead of as the link inside it
```

A data table read as a wall of duplicated prose, and an agent matching on the accessible name got the
row rather than the cell it wanted.

## ⭐⭐ But `row` is not simply wrong — its answer depends on where the row is

Headless Chrome 145.0.7632.116, one fixture per row:

```text
  <div role=table><div role=row><div role=cell>X          row  name=""            static structure
  <div role=grid><div role=row><div role=gridcell>X       row  name="X"           ⭐ FROM CONTENT
  <div role=treegrid><div role=row><div role=gridcell>    row  name="TG-CELL"     also
  <div role=grid><div role=rowgroup><div role=row>        row  name="RG-CELL"     rowgroup TRANSPARENT
  <table role=grid><tbody><tr><td>NATIVE-GRID-CELL        row  name="NATIVE-GRID-CELL"  native counts
  <div role=grid><div role=table><div role=row>           row  name=""            ⭐ the NEAREST wins
  <div role=grid><div role=row aria-label=RowLabel>       row  name="RowLabel"    aria still wins
  <ul><li>Alpha                                           listitem  name=""       never
  <ul><li aria-label=ItemLabel>ItemText                   listitem  name="ItemLabel"
  <ul><li><a href>InnerLink</a>                           listitem  name=""; the LINK carries it
```

> ⭐⭐⭐ **A grid is the interactive widget and a table is static content.** That is the distinction
> `Role::Grid` was split out of `Role::Table` to preserve, and this is the first rule that consumes
> it. A row in a spreadsheet is a thing a user selects and hears described; a row in a page's data
> table is structure.

A predicate on the role alone cannot say this, so the name computation now asks
`takes_name_from_content(dom, node, role)`, which walks to the **nearest declared container** —
`rowgroup` (`<tbody>`, `<thead>`) transparent because it is a grouping *level* rather than a container
*kind*, a `table` or `tree` STOPPING the walk, everything else transparent.

⚠ **`treegrid` was absent from the role vocabulary**, so `role="treegrid"` fell through to the
element's implicit role and a data grid announced itself as a `<div>`. The rule is not expressible
without it, so it was added — and that alone is the `wai-aria` +1.

## The arm a GREEN mutation asked for

Deleting the `Table` stop from the ancestor walk left every other arm green, because none of them
nested a static table inside a grid. Chrome does the nesting case and answers `""`. **A mutation that
does not go red is a report about the gate**, and this is the third tick running where that rule paid
(t1402 found a hollow arm, t1403 found an inert guard, t1404 found a missing arm).

## The ranked remainder — and the first ranking of it was WRONG

93 misses remain. The 35 largest were names differing only in CASE (`'SKIP TO CONTENT.'` vs
`'Skip to content.'`) — which reads exactly like `text-transform` not reaching the accessible name,
and `engine/a11y/Cargo.toml`'s own comment claims it does, so the *"checkable claim that died
silently"* story wrote itself. **It was false.** One fixture through the same `a11y-dump` path:

```text
                                                   chrome                           manuk
  <a class=up>Skip to content.</a>                 'SKIP TO CONTENT.'               IDENTICAL
  <a><span class=up>inner span up</span></a>       'INNER SPAN UP'                  IDENTICAL
  <h2> text-transform:uppercase                    'A11Y STANDS FOR ACCESSIBILITY'  IDENTICAL
  <a class=cap>how do i get started?</a>           'How Do I Get Started?'          IDENTICAL
  <a>plain link</a>                       CONTROL  'plain link'                     IDENTICAL
```

> ⭐⭐⭐ **A signature names the TESTS; only a probe names the MECHANISM.** All 35 are on one page, and
> since the name path is provably correct the cause is upstream — a11yproject's `text-transform` never
> reaches those elements, a cascade/stylesheet question that must also be visible in the RENDERING.

```text
  27   a `cell`'s concatenated text            HN's cells — separator/whitespace inside the name
  16   the LayoutTable* family                 chrome DEMOTES a header-less layout table OUT of the
                                               table roles entirely; we keep table/row/cell
   5   the ROOT's name is the document TITLE   'The Rust Programming Language Blog' vs ''
   4   role `Abbr` (<abbr>)                    absent from the vocabulary
  35   [RECLASSIFIED, not a11y] a11yproject's text-transform never reaches its links/headings
```
