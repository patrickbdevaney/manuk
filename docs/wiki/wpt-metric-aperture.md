# The metric's aperture — nine checked-out trees the primary number never counted

> Landed t1388. Gate: `the_wpt_total_is_the_sum_of_its_rows`
> (`agent/tests/g_wpt_areas_total_matches_rows.rs`). This is t1273's finding one level in: that tick
> found trees missing from the CHECKOUT; this one found trees present in the checkout and missing
> from the METRIC.

## ⭐⭐⭐ "Measured" meant two different things, and the weaker one reported 100%

`scripts/blindspot.sh` — the existing aperture instrument — reports `svg` and `mathml` as
**✓ measured**, because it defines *measured* as **checked out**. `docs/loop/WPT-AREAS.tsv`, which is
the loop's PRIMARY per-tick metric, had never counted a single one of their 4,470 subtests.

> **Two instruments, one word, two meanings — and the weaker meaning was the one printing a
> reassuring number.**

The aperture diff that finds it is two lines: list the checked-out directories, list the area rows,
`comm -23`.

## What was invisible

```text
  tree              pass / total       pct     note
  html/semantics    4922 / 11264      43.7%    the LARGEST failing area in the whole corpus
  html/canvas        674 /  4514      14.9%    ⚠ and it carries a Bar 0 — see below
  css/CSS2          2095 /  2210      94.8%
  html/browsers      221 /  1891      11.7%
  mathml             289 /  2362      12.2%
  svg                307 /  2108      14.6%
  accname            445 /   484      91.9%    exit-gate condition 4
  wai-aria           399 /   434      91.9%    exit-gate condition 4
  html-aam           315 /   335      94.0%    exit-gate condition 4
  ──────────────────────────────────────────
  total             9667 / 25602               ~2% of the reachable denominator
```

⭐⭐⭐ **`html/semantics` is now the largest failing area on the board — ahead of every CSS row — and
it was not on the board at all.** `html/dom` was one of `html`'s four checked-out subtrees, and it
was the only one with a row. Every ranking the loop has made for hundreds of ticks was made inside a
frame that omitted the biggest thing in it.

⚠ **THE DENOMINATOR TRAP, stated rather than discovered later:** the board's headline moved
**80.4% → 74.4%**. Nothing got worse. The metric got honest, and a number that FALLS when you open
the aperture is the aperture working.

## ⚠⚠⚠ And the first thing it revealed is a Bar 0

```text
  html/canvas/element/pixel-manipulation/2d.imageData.get.large.crash.html
      CRASH (killed by a signal — Bar 0)
```

`ctx.getImageData(10, 0xffffffff, 2147483647, 10)` — the shim does `w = Math.max(1, w|0)` and hands
`2147483647 × 10 × 4` bytes to `vec![0u8; …]`. **85 GB, and the process dies.** Bar 0 outranks every
visual cluster, and it has been reachable from any page for as long as the API has existed —
invisible only because the tree it is tested in was outside the metric.

Chrome's full error surface, measured for the fix (`--headless=new`):

```text
  getImageData(10, 0xffffffff, 2147483647, 10)   TypeError    "outside the 'long' value range"
  getImageData(0, 0, 1e10, 1)                     TypeError    (non-integral / out of range)
  getImageData(NaN, 0, 10, 10)                    TypeError
  getImageData(Infinity, 0, 10, 10)               TypeError
  getImageData(0, 0, 2147483648, 1)               TypeError
  getImageData(0, 0, 2147483647, 10)              RangeError   "Out of memory at ImageData creation"
  getImageData(0, 0, 23000, 23000)                OK   2.116e9 bytes
  getImageData(0, 0, 32768, 32768)                RangeError   4.295e9 bytes
  getImageData(0, 0, 0, 10)                       DOMException "The source width is 0"
  getImageData(0, 0, -5, 10)                      OK   w=5 h=10   (the rect is NORMALISED)
  getImageData(5, 5, -5, -5)                      OK   w=5 h=5
```

The allocation boundary is **`w × h × 4 ≤ 2³¹−1` bytes** — 23000² passes, 32768² does not.

## ⚠ And the TOTAL row was 268 behind its own table

t1381 and t1382 each refreshed the `css/css-overflow` row after re-running the area (481 → 508 →
513) and neither updated `TOTAL`. For four ticks the headline read 268 passes behind the rows it
summarises.

> **A DERIVED FIGURE THAT IS STORED RATHER THAN COMPUTED NEEDS A CHECK, AND "I WILL REMEMBER TO
> UPDATE BOTH" IS NOT ONE.**

`g_wpt_areas_total_matches_rows` is that check: TOTAL equals the sum, every row's printed percentage
is its own arithmetic, and no area appears twice (a duplicate double-counts a tree into a total the
ratchet reads as monotonic).

## The wall audit (due t1388)

```text
  total 221s   ·   T (crate tests) 100s = 45%   ·   G6 14s   ·   G1 8s   ·   P 5s   ·   F 4s
```

**The wall is 221s against a 300s target — under it.** ⚠ And that corrects t1383's single failing
self-audit item, which read **2231s**: the self-audit reads the LAST verify receipt, and t1382's was
a COLD rebuild (it touched `engine/layout`). Both numbers are true of different runs; the target is
met on a warm wall and the cold case is a rebuild cost, not standing bloat. Nothing was trimmed —
the admissible optimisations (`cargo-nextest`, shared runtimes) are all in `scripts/`, which is
observer-owned.

⚠ Reported, not fixed: `blindspot.sh` prints *"We measure 72829 of 72829 upstream test files
(100.0%)"* on the same screen as a list of areas marked `✗ INVISIBLE`. Observer-owned script.
