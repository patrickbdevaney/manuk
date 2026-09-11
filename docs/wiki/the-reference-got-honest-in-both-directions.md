# The reference got honest in both directions — and a 98.7% coverage was measured against 533 elements

**t1515.** t1514 gave the oracle `--disable-web-security`, so it can finally load the modules and
self-hosted fonts every real browser loads. t1514's own NEXT said *"re-sweep the corpus: every banked
rows file predates this and the fingerprint cannot say so."* This is that sweep — 34 sites, the same
slice t1507 banked, same binary on our side.

```text
  SCORED            29/34  ->  29/34      newly scored 2 · lost 2
  improved 9 · worse 6
  mean shape (scored)  72.6  ->  71.2
  shape >= 0.75        17    ->  16
```

**The headline went DOWN and that is the correct outcome.** Nothing in the engine changed; the
reference stopped being wrong.

## The two shapes of movement, and the coverage column separates them

```text
  site                      cov b   cov a    n b    n a    shape
  www.jatekshop.eu           97.8    97.9   1161   1161   76.7 -> 95.1   +18.3
  rockstaractu.com           92.2    92.2    784    784   96.8 -> 90.9    -5.9
  www.friulioggi.it          95.7    95.7    841    841   68.5 -> 48.4   -20.1
  ─────────────────────────────────────────────────────────────────────
  allticketscol.com         100.0    91.2      1     73    0.0 -> 72.6   NOW SCORED
  nysainfo.pl                51.3    44.7   1033    937   86.1 -> 74.1
  vk.com                    100.0     3.8      5      4   40.0 -> 25.0
  www.datacareservices.com   98.7     2.4    533     13   76.4 -> 23.1
```

**Above the line the oracle's element set is IDENTICAL** — same coverage, same sample — so only the
text boxes moved. That is the font fix, measured: `jatekshop` **+18.3** because we agree far better
with the real face than with the fallback the reference had been using, and `friulioggi`/`rockstaractu`
worse because on those our metrics disagree with the *real* face. ⭐ **Both directions are the same
correction; only one of them flatters us.**

**Below the line the oracle now renders a different page.** `allticketscol.com` went from **1 element
to 73** and became scorable. And then:

> ⭐⭐⭐ **`www.datacareservices.com` read COVERAGE 98.7% against an oracle that rendered 533 elements.
> The reference now renders ~20× more of it, and we render 13. Coverage 98.7% → 2.4%.**
> `vk.com` is the same shape: 100% of 5 elements → 3.8% of a real page.

A coverage number is *"of the elements Chrome renders, the fraction we render"*. When Chrome could
not boot the SPA either, the ratio was near-perfect agreement **about a page neither engine had
built**. t1425-1428's rule, and this is its largest instance: **a staleness that FLATTERS is not a
smaller bug than one that breaks.**

## What this changes for the loop

* **Two real engine gaps are newly visible** — `datacareservices` and `vk.com` render almost nothing
  of a page the reference now builds. They were hidden behind 98.7% and 100%.
* **Every banked ranking predates a different reference.** `SWEEP-t1406` is what t1506-1507's work
  order was computed from, and it was taken against an oracle that could not load a module or a
  self-hosted font. The `ORIGIN / METHOD / ENGINE` histogram, the burndown's ordering and every
  `font-resolution` conclusion are all in that class.
* **The certificate's terms may re-rank.** The coverage collapse on SPA sites suggests the
  scorability/coverage term is larger than the pre-CORS rows said — the opposite direction from
  t1506-1507's finding, and it has to be re-derived rather than assumed either way.

Both rows files are banked beside each other so the comparison can be re-run:
`docs/loop/SWEEP-t1507-precors-rows.tsv` and `docs/loop/SWEEP-t1515-cors-rows.tsv`.

## ⚠ What this sweep is NOT

⚠ **34 sites, one run per side.** The per-site moves above are large enough to clear the known
run-to-run bands (`a1.ro` ±6.3, and t1512 measured three false `--jobs` signals in two ticks), but
the **means are not** — 72.6 vs 71.2 is inside the noise of a 29-site mean and is reported as
direction, not magnitude. `probidas.lt` LOST SCORE to `unreachable`, which is the network.

⚠ **No engine behaviour changed in t1514 or here.** Every number that moved is the reference.
