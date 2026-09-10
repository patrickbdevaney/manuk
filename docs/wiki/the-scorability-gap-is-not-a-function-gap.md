# The scorability gap is not a function gap

*Tick 1485. 200 sites, attributed by the instrument's own refusal tag. The mandate's binding
constraint is worth 6 sites; the term nobody is working is worth 23.*

## What was asked

The observer's 2026-09-10 mandate names *"the SCORABILITY/FUNCTION ceiling — ~82%, 24/135 in-scope
sites never yield a scored tree"* as **the binding constraint**, and directs every tick at *"the
SHARED function/JS/DOM/event-loop defects that make sites unscorable."* Its own P0 said: **confirm
the ~82% is current.**

Surface audit #90 confirmed it on 40 sites and found the attribution wrong. This is the same
measurement at 5× the sample, on the representative CrUX trend corpus
(`docs/bench/corpus-crux-trend.txt`, 200 sites, stride-2 of the 400-site cert corpus). Rows banked at
`docs/loop/SWEEP-t1485-refusal-tags.tsv`.

## The attribution

```text
  200 sites · SCORED 113 (56.5%) · refused 87

  ORIGIN — out of any engine's reach ................................ 58
      bot-wall 36 · unreachable 16 · http 5 · empty 1

  METHOD — the oracle sees a file:// snapshot, we render the LIVE url  23
      probe-blocked 8 · tree-divergence 7 · shell-only 4
      oracle-module-shell 3 · thin-overlap 1

  ENGINE — OURS ...................................................... 6
      css-starved 5 · render-failed 1
```

Bot-walls are excluded from the in-scope denominator (`DAILY-DRIVER-CERTIFICATION.md` §3), so:

```text
  in-scope 164 · scored 113 = 68.9%        (the mandate says ~82%)
  of the 51 in-scope refusals:  ORIGIN 22 · METHOD 23 · ENGINE 6
```

**The gap is bigger than the map says and almost none of it is a function defect.**

## And boot errors barely predict render quality either

Among the 107 scored sites carrying a shape number, split by whether the page threw during boot —
which `Page::boot_errors` can answer for the first time as of t1480:

```text
  boot CLEAN   n=53   mean SHAPE 67.7%   shape>=75 on 30 (57%)
  boot THREW   n=54   mean SHAPE 67.4%   shape>=75 on 23 (43%)
```

**A page that throws at boot renders essentially as well as one that does not.** The means differ by
0.3 points on n≈53 per arm — inside any plausible band. The crossing rate differs (57% vs 43%, i.e.
30 sites vs 23) and that is the only signal here; at this n it is suggestive and not decisive.

So the function axis is not gating scorability (6 of 200) *and* it is not what separates a
well-rendered page from a badly-rendered one among the pages that do score.

## ⚠ This does not say the four function ticks were wrong

t1480–t1483 each found a real, shared, Chrome-arbitrated defect and each is banked behind a gate
proven red. The OneTrust consent stub really did die on two of forty sites; `assignedElements` really
was absent; the rejection reporter really did cry wolf twice in three. **A mandate can name the wrong
bottleneck and still point at good work.**

What the measurement says is narrower and sharper: *that work does not move the exit metric*, and the
loop has now spent four ticks learning it the expensive way because P0 had not been run.

## Where the metric actually is

```text
  Phase-0 render bar:  shape >= 0.75 on >= 95% of in-scope sites
  measured here:       53 of 164 = 32.3%
```

## The two addressable terms, ranked

**1. METHOD — 23 sites, and the fix already exists and is barely engaging.** Every one of those five
tags is the same asymmetry: the oracle is fed a `curl` snapshot served from `file://` while we render
the live URL, so a module bundle is cross-origin for Chrome and not for us, a page's CSP blocks the
probe, or the two engines settle the same app into two different states. The instrument names its own
remedy in `Unmeasurable::OracleModuleShell`'s own doc comment — *"a loopback reverse PROXY so
document, bundle and XHR share ONE origin"* — and `tests/wpt/src/proxy.rs` exists. It printed
`PROXY REFERENCE ACCEPTED` on **6 of 200** sites and was mentioned at all on 12. Twenty-three sites
are refused for exactly what it was built to remove. **Find out why it engages for 6 and not 29.**

**2. RENDER — 53 of 164 against a 95% bar.** This is P4 on the mandate's own list, sequenced *after*
the ceiling is lifted. On this evidence the ceiling that is worth lifting is METHOD, not FUNCTION, and
P4 is the main line.

⚠ **`css-starved` (5) is the one genuinely engine-owned lever in the list**, and it is ours by the
instrument's own words: *"our own `load_deadline` cut those sheets, so this is our bug."* Five sites,
and a page whose author stylesheets never arrived is unscorable *and* renders wrong. Small, real, and
the only item in the ENGINE column worth a tick.

## ⚠ What this measurement does not carry

One run, no repeat, so no error band (t1410) — the refusal tags are discrete and per-site, which is
why they are quoted, and the shape means are single-run, which is why the 0.3-point difference is
reported as "inside the band" rather than as a number. Run at `--jobs 3`; contention did not manifest
as timeouts (`oracle-timeout` does not appear in the 200-site histogram at all, against 1 in the
serial 40-site run).
