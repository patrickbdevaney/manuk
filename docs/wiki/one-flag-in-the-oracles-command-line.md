# One flag in the oracle's command line — and two refusal cohorts stop existing

**t1514.** t1513 proved the reference lays out self-hosted webfont sites in a fallback face, because
it renders every document from `file://` where webfonts are cross-origin and CORS-gated. It priced
the fix and left it for its own tick. This is that tick, and **the fonts turned out to be the small
half.**

## The change

One line in `base_flags`:

```rust
"--disable-web-security".into(),
```

Measured first, so it is the minimum that buys the capability rather than the first thing that
worked: `--allow-file-access-from-files` alone does **not** work, and **no `--user-data-dir` is
needed** in `headless=new`.

## ⭐⭐⭐ ES MODULES ARE CORS-GATED TOO, AND THAT IS TWO WHOLE REFUSAL COHORTS

```text
  13-site slice, before -> after

  allticketscol.com     shape 0.000  shape_n 1   oracle-module-shell-1   ->  shape 0.726  shape_n 73  SCORED
  dashboard.twitch.tv   shape 69.5                                       ->  shape 72.9   +3.4
  ten others            unchanged · ZERO regressions

  refusal tags:   2 oracle-module-shell + 2 shell-only   ->   0 of either
```

A `type="module"` script is fetched under CORS. **From `file://` every module fetch is blocked**, so
the oracle saw a shell where the real site boots an SPA — and filed it as
`oracle-module-shell: the ORACLE rendered only 1 element … so THE SHELL is what the site is`.

⚠⚠⚠ **t1496–t1504 spent four ticks building a PROXY to work around this cohort.** The refusal it kept
hitting — *"the one-origin render is ALSO a shell"* — was the same wall seen from the other side. The
cause was one flag in the oracle's own command line.

On the full 200-site corpus the directly-named cohorts are **3 `oracle-module-shell` + 3
`shell-only`**, with `probe-blocked` (6) and `tree-divergence` (6) plausible neighbours — up to 18 of
the 95 unscored rows, on top of the font effect on the ~75% of sites that carry an `@font-face`.

## ⚠⚠ IT OVER-CORRECTS, AND THE PRINCIPLED FIX IS THE PROXY

A real browser at `https://site/` has the document **same-origin** with its own subresources, so its
self-hosted fonts and modules load and genuinely cross-origin ones are still checked. This flag
restores the first and **also permits the second** — it makes the reference more permissive than a
real browser, not equal to it.

⭐ So the honest reading of the proxy arc is the opposite of a waste: **giving the document its real
origin is the correct fix, and this flag is the cheap one.** The flag is a large net improvement
today; the proxy is what would make the oracle exact.

## ⚠⚠ THE INSTRUMENT FINGERPRINT DID NOT MOVE, AND IT SHOULD HAVE

```text
  allticketscol.com  before  …  oracle-module-shell-1  d3da5acb
  allticketscol.com  after   …  (scored)               d3da5acb
```

The fingerprint exists so *"a rows file can say that two readings of one site came from two different
instruments"*, and it is **the probes' own TEXT**. A browser flag is not in it. So the most
consequential oracle change in this arc is invisible to the mechanism built to catch exactly that —
and every rows file banked before today compares silently against rows taken with a different
reference. **Named, not fixed:** widening the fingerprint to cover `base_flags` is its own tick, and
it must not be done in the same commit as a change it would then be mis-attributed to.

## ⚠ I6 — page content is untrusted input, always

This weakens the same-origin policy in a **throwaway headless process that reads geometry** and is
never the shipping browser. The alternative is a reference that is wrong about text metrics on three
quarters of the corpus and blind to every module-booted SPA. Stated here rather than left for a
reader to discover.

## Where it is

`tests/wpt/src/chrome.rs` — `base_flags`.

⚠ **No engine behaviour changed.** Every movement in this tick is the REFERENCE becoming more like a
real browser; our side is byte-identical.
