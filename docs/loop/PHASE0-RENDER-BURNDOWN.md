# PHASE-0 RENDER BURNDOWN — the finite path to shape≥0.75 on ≥95% of real sites

> **Observer, 2026-07-29.** Counterpart to `PHASE0-BOUNDED-REMAINDER.md` (which sizes the FUNCTION
> remainder). That doc proved the *capability* tail is finite; this doc proves the *render/shape* tail is
> finite too, and turns it into a ranked, measurable burndown with a visible slope to 95%. Measurement
> authority remains `DAILY-DRIVER-CERTIFICATION.md`. This is the "how Phase-0 render conclusively ends"
> plan the user asked for: finite, bounded, tractable — not an endless CSS-conformance chase.

---

## 0. The honest number (the premise correction)

The capability count makes the engine *feel* nearly done. It is not, on the axis that defines "looks
right": **shape≥0.75 on 11/209 = 5.3% of the in-scope corpus** (t706, the only honest full-corpus sweep).
The old "4.2% of 265" headline divided by an unwinnable denominator — 56 of 265 sites (21%) are
permanently unreachable by our own no-stealth policy (bot-wall/probe-blocked/unreachable/4xx-5xx) and can
NEVER be rendered, so 95%-of-265 caps out at ~80% and is arithmetically impossible. Fixed in
`fidelity-progress.sh` (2026-07-29): EXCLUDED is watched separately and capped; the pass-rate divides by
the **in-scope** denominator (crashed/render-failed/shell-only stay IN — they are OUR bugs).

**Winnable target: shape≥0.75 on ≥95% of 209 in-scope = 199 sites. Today 11. Distance: +188.**

## 1. Why this is CONCENTRATED (finite), not a death tail

Three independent cuts of the t706 sweep all say the deficit is a small number of shared layout
primitives, not thousands of per-site CSS quirks:

1. **The DONE set is exactly the trivial pages.** All 11 passing sites are single-column, near-zero-CSS
   text pages: danluu.com 1.00, without.boats 0.98, jvns.ca 0.94, lite.cnn.com 1.00, text.npr.org 0.86,
   arxiv.org 0.83, httpd.apache.org 0.85. We render basic vertical text flow correctly. Shape collapses to
   0.44–0.72 the instant a page adds **composed layout** — flex navs, card grids, multi-level nesting.
2. **61% of failing sites have coverage ≥ 0.95.** Every box Chrome draws, we draw — just in the wrong
   place. This is a pure *position* error (a `dy`/size problem), the signature of a common cause, not
   missing elements or missing features.
3. **The `dy` propagation law** (measured, memory `session-674-687`): shape is dominated by a vertical
   term — one box whose HEIGHT is computed wrong shifts every box below it downward. So a single primitive
   error inflates into thousands of per-tag "geometry" divergences in `CLUSTERS.md` (14002 hits on `<div>`
   is NOT 14002 bugs — it is a handful of mechanisms cascading). Fix the mechanism → the whole subtree
   below every occurrence snaps into place → the entire band of ~73 pure-position sites rises together.

The simplest fully-covered failing sites are therefore the highest-leverage worklist, because they have
**no confound** (nothing missing, nothing exotic) and they share their primitives with the whole corpus:

    news.ycombinator.com  cov=1.00 shape=0.72      blog.rust-lang.org  cov=1.00 shape=0.63
    a11yproject.com       cov=1.00 shape=0.44      martinfowler.com    cov=1.00 shape=0.58
    en.wikipedia.org      cov=1.00 shape=0.52      whatwg.org          cov=1.00 shape=0.51

## 2. The blocker: the oracle discards the mechanism it ALREADY computes

`CLUSTERS.md` aggregates every geometry divergence to the HTML **tag** (`geometry:<div>`, `geometry:<span>`,
…) — it cannot say whether a delta is a wrong top-edge, a wrong height, a wrong width, or a column swap, so
it cannot rank *primitive* fixes. But this is nearly free to fix, because the signature already exists and
is thrown away (layout analysis, 2026-07-29, file:line verified):

- `oracle::cluster()` (`tests/wpt/src/oracle.rs:344-377`) already computes the full mechanism key —
  `geometry/{displaced|mis-sized}: {width|height|y|x} ~{mag_band}px (<tag>)` — separating displaced-vs-
  mis-sized, the axis, and the magnitude.
- `run_oracle_merge` (`tests/wpt/src/main.rs:2649-2654`) **collapses all of it back to `geometry:<tag>`**
  before writing the ledger the loop reads. The axis/magnitude/kind is discarded at the last step.
- `placement_stats` computes median dx/dy/dw/dh and `first_divergence` a per-site originating dy, but they
  print to **stderr only** (`main.rs:856-895`) and are not in the banked TSV — they evaporate.

**The measurement upgrade is "stop discarding the signature," not "build a classifier"** (agent-owned, §4).
Until then the loop can already rank on `h_overflow` (in the banked TSV) as a proxy for the #1 mechanism.

## 3. The ranked primitive worklist (from the layout analysis, 2026-07-29 — file:line evidence)

**First, what is already Chrome-correct — so the loop does NOT re-grind it** (verified in source): margin
collapsing incl. collapse-through (`engine/layout/src/lib.rs:1771-1920`); `line-height:normal` height to
the pixel on 3 faces (`engine/text/src/lib.rs:59-114`); half-leading / strut / vertical-align
(`layout/lib.rs:5240-5375`); default UA margins on p/h*/ul/ol/blockquote/pre/hr incl. nested-list zero
(`engine/css/src/stylo_engine.rs:293-347`); default font Arial→Liberation Sans (`text/lib.rs:285-303`);
root scrollbar handling (`chrome.rs:796-802` + `layout/lib.rs:758-766`). **The gap is NOT one gross missing
primitive** — it is residual accumulation in ~4-5 mechanism families, ranked by (in-scope sites × dy):

1. **Container-WIDTH errors laundered into wrap/line-count → dy cascade — HIGHEST LEVERAGE, every prose
   site.** A container a few px too wide re-wraps its prose → different line count → whole-line (~20px)
   height error → cascades to every following block. Invisible as a *width* bug; it surfaces as dy. Signal
   already banked: `h_overflow` (wikipedia 52, apple 127, github 19). Suspects: `%`-width + box-sizing,
   `width:auto` fill (`extra`), min/max-content sizing (`layout/lib.rs:1985-2017`). **Do this first.**
2. **Flex/grid column sizing (taffy seam) → whole-column reading-order swaps — app chrome/sidebars.**
   `reading_order` high on exactly these (docs.python 36, github 18, gitlab 12). Fragile block↔taffy width
   handoff (`layout/lib.rs:714, 1968-1986`). Gross misplacement, not near-miss.
3. **Line-count drift from sub-pixel advance accumulation on long prose — every text-dense site.** Advances
   subpixel-unrounded (`text/lib.rs:83`), heights rounded per line; a paragraph near a wrap boundary flips
   its line count between engines even when median dx≈0 (rust-lang blog 0.63, this-week-in-rust 0.24).
4. **`display` mis-computation** — real `none→block` / `block→inline` / `inline→none` clusters (17/9/10
   sites, CLUSTERS.md); each element is both wrong-shaped and a displacer.
5. **Table layout** — no colspan/rowspan/border-collapse/border-spacing (`layout/lib.rs:24`); severe on
   wikipedia-class infobox/reference pages.
6. Replaced-element / image intrinsic height (media/card sites); 7. anonymous block boxes for mixed
   inline+block children (legacy markup); 8. list-item marker / `li` height (high-frequency ul/li).

**Cross-cutting trap:** the UA sheet exists twice — Stylo CSS text (`stylo_engine.rs:293`) and
`apply_ua_defaults` MinimalCascade (`css/src/lib.rs`) — kept in lockstep by hand; drift silently changes
default margins/display on the shipping path (see memory `two-cascades-stale-source-of-truth`).

**Honest read (the analysis's verdict):** CONCENTRATED and tractable — *not* a silver bullet, *not* a death
tail. Fixing families #1 and #2 would move the largest block of the corpus at once. The width→dy laundering
(#1) is the key insight: the loop has been staring at `geometry:<div>` dy symptoms whose cause is upstream
width.

## 4. The finite closing plan (how render-Phase-0 ends, in bounded ticks)

1. **FRESH SWEEP NOW.** 30+ capability ticks have landed since t706 with the shape number unmeasured — the
   loop is flying blind on its own headline. The agent runs a full sweep, banks `SWEEP-t<N>-rows.tsv`;
   `fidelity-progress.sh` records it and prints the burndown slope. *(agent; observer NEVER runs the sweep
   — contention false-REDs the perf gates.)*
2. **Mechanism oracle.** Agent adds a mechanism dimension to the divergence record: classify each geometry
   delta (top-edge / height / width / x-offset) and attribute the dominant CSS input. Turns the tag ledger
   into a rankable primitive ledger. *(agent — instrument territory.)*
3. **Ranked burndown.** Each render tick fixes ONE primitive, chosen by (sites × dy severity), verified on
   the anchor sites (§1), and **must prove the band moved on the next sweep** — a fix that does not raise
   in-scope-pass is reverted or re-scoped. Keep function-axis capability ticks interleaved, but render is
   the binding constraint until in-scope-pass clears ~50%.
4. **Certainty of done.** `fidelity-progress.sh` prints `+X pts/sweep → ~N sweeps to 95%`. When the slope
   is positive and steady, Phase-0-render has a finite ETA. Flat/negative slope = escalate (the primitive
   list is wrong, or a second mechanism is hiding — instrument deeper before grinding).

## 5. The path — REVISED by the owner (2026-07-30): representative CrUX from the start

> **Durable enforcement:** an ORDERED **2**-milestone state machine, tracked mechanically by
> `scripts/phase0-milestones.sh` (every observer heartbeat via ops-check + the board's CORPUS-SWITCH /
> PHASE-0 SEQUENCE blocks). Computes the current milestone from observable signals and AUTO-ADVANCES at
> each 95% gate — surviving board drift, relaunches, context resets. M1 = this doc (render on CrUX).
> M2 = the function leg → `docs/loop/FUNCTION-CERT.tsv`. Then the v1.0.0 trigger.

**Superseded the 2026-07-29 "265 now → corpus-v2 at the end" plan.** Reason (the owner's correct challenge):
driving the curated 265 to 95% would over-fit the easy head and hit the CrUX tail (quirks-mode, legacy
table layout, malformed HTML) **blind, at the very end** — a second hill the t602 pilot already showed
scores worse. Since the primitive fixes are corpus-agnostic (they help any site) and we were only 2 sweeps
in, switching to the representative corpus **now** costs almost nothing and eliminates the late surprise.
This collapses the old 3 milestones (v1-render → v1-function → v2-recert) into two — there is no separate
"re-cert on v2" because CrUX is the corpus from day one.

- **M1 RENDER — DECIDED: drive the representative CrUX corpus now.** Trend sweeps on the ~200-site
  stratified subsample `docs/bench/corpus-crux-trend.txt` (fast); cert on the full 400 `corpus-v2.tsv`.
  Keep reachable head anchors (hackernews/wikipedia, in the HEAD stratum) for per-tick debugging. The first
  CrUX sweep is a NEW baseline (likely lower — the tail is harder); `fidelity-progress.sh` auto-detects the
  corpus (ledger f13) and never diffs CrUX against the 265 (the metric-swap guard). The final 265 slope was
  **+1.8 pts from 4 dy fixes** (real, modest — ~50 sweeps to 95% as a first noisy ETA); CrUX recalibrates it.
- **M2 FUNCTION — DECIDED: build the BiDi per-site leg AFTER M1.** Render is the visible binding gap and
  goes first. BUT a cheap early signal rides along now: the M1 sweeps flag **throw-class killers** (a
  touched API that throws/no-ops — IndexedDB/observer-trio/etc — takes down the page), so function-fatal
  sites surface continuously instead of as a late pile-up. The full A/B leg (cert §4 Layer C-function) is
  the last gate; its capability breadth is largely already there per `PHASE0-BOUNDED-REMAINDER.md`.

## 6. Division of labour (unchanged doctrine)

- **Observer:** this doc, the winnable metric, the burndown slope + alerts, the board/launch-prompt steer,
  banking sweep actuals, flagging the §5 forks. Never edits `engine/`/`manuk-wpt`; never runs sweeps.
- **Agent:** the fresh sweep, the mechanism-oracle dimension, the per-primitive engine fixes, the per-site
  `boxes --why` diagnosis. Instrument + engine are agent territory.

## 7. Measured rate is noisy — band rises faster than crossings (t752→t767, 2026-07-30)

Two CrUX slopes in: t752→t758 **+1.6 pts** (real: scored↑, pass 5→7), t758→t767 **+0.0 pts** (pass 7→7 FLAT).
But the FLAT sweep hid real work: the denominator-trap guard flagged the raw shape_mean +2.2 as inflated
(4 sites dropped from scored), and the **common-scored-set control** (79 sites in both) showed a **real
+2.0pt band move — 14 sites improved, 2 regressed, 63 flat**. So the 7-fix batch DID lift the band; it just
crossed 0.75 on ZERO sites this round. Lessons:
- **Pass-count (shape≥0.75) is a threshold that LAGS the band** — report shape_mean (governing term) AND
  pass-count together; a flat pass-count with a rising common-set band is real progress, not a stall.
- **ALWAYS run the common-scored-set control before claiming a band move** — the raw shape_mean is
  denominator-trap-prone (a dropped low-shape site fakes a rise). Do not narrate a band-lift off the raw mean.
- **Rank primitives by CORPUS-SAMPLE frequency, not by whatever bug you find.** 3 of 7 fixes were RTL
  (flex/table/grid) — real, but the CrUX-200 sample is RTL-light, so they crossed ~0 MEASURED sites. The
  honest ledger already ranks by sites-affected; follow it to maximise measured movement.
- **Conservative rate ≈ +0.8 pts/sweep** (2-slope avg) → order ~100 sweeps; but the band-rise implies
  crossings come in WAVES as clusters approach 0.75, so linear extrapolation is an upper bound. Finite, long,
  noisy — recompute the rate every 2-3 crux sweeps, never off one.

## 8. The crossing-ranked worklist (measured from SWEEP-t777, 2026-07-31)

§7 warned that the band rises faster than crossings. **t758 → t767 → t777 confirms it outright**: three
honest sweeps, nineteen ticks, `in-scope pass 5.4% / 5.4% / 5.4%`, `7 / 7 / 7` passing sites, M1 gate
`2.3% / 2.3% / 2.3%` — while `shape_mean` climbed 41.3 → 46.3. **Zero crossings for +5.0 points of
mean.** (The `+2.3 pts/sweep` `fidelity-progress.sh` printed diffs t777 against the *contaminated*
t771 row and is an artefact; the honest slope is 0.0 and no ETA follows from it.)

That trips the escalation rule at the top of §4.3, so the sweep was re-ranked on the axis the board
actually asks for — **marginal M1 crossings** — instead of tag frequency or mean shape:

```
shape   distance  site                          jarring       n     cov     elements to cross
0.729    +0.021   chat.google.com               ALL ZERO      59    1.00    ~2
0.725    +0.025   www.kicktipp.com              r=1           80    0.99    ~2
0.698    +0.052   255md.com                     ALL ZERO      43    1.00    ~3
0.692    +0.058   secure5.entertimeonline.com   ALL ZERO      39    1.00    ~3
0.709    +0.041   www.marktplaats.nl            r=1 d=2      810    1.00
```

**Four fully-covered, jarring-clean sites sit within 0.06 of the bar with fewer than 100 scored
elements each** — roughly **eight mis-placed elements across three pages stand between the M1 gate and
a doubling of it** (3/129 → 6/129). Rank render ticks from this table, not from cluster mass.

⚠ **Caveat that must travel with the table** (memory `session-654-657`): a live site's shape varies a
few points run-to-run, so a +0.021 distance is **at or below the noise floor**. The table ranks *where
to look*, and a fix is only real if it names a mechanism — never if it only moves one site's number.

### First probe result — the mechanism is family #1, and it is gross, not near-miss

Running the instrument on the two cleanest targets (the mechanism oracle already prints per-element
`[x y w×h] {font/size}` for both engines — §2's "nearly free" signature, and it is live):

* **`secure5.entertimeonline.com`** — `<article>` is ours `[33 120 1134×452]` vs Chrome
  `[0 120 487×354]`. **Our article is 1134px wide; Chrome's is 487.** We give a `width:auto` box the
  full containing block and then centre it (33 = (1200−1134)/2); Chrome shrink-wraps it to 487 at
  x=0. Every descendant inherits the x-shift (`div` at 389 vs 56, and so on down).
* **`255md.com`** — the `<form>` is 400 wide in *both*, but ours sits at x=400 (dead-centre of its
  680-wide parent) and Chrome's at x=309. Same y, same width, different x: we are centring something
  Chrome does not centre. Its `<textarea>` is also `97px` tall in `{monospace/16}` where Chrome has
  `29px` in `{-apple-system/16}` — **the author's `font-family` is not reaching the form control**,
  and the height error follows the font error.

Both are **family #1 (container-width → cascade)**, but the sharp correction to §3 is that on these
sites it is **not** the subtle "a few px re-wraps the prose" form — it is a **shrink-to-fit /
intrinsic-width box being given the full containing block**, a 647px error on one element. That is the
`min-content`/`max-content` sizing lever the lever-board has carried as a step-function item all
along, and it is now attached to named sites with per-element evidence instead of a WPT count.

**Next render tick starts here**: determine why the `secure5` `<article>` resolves `width:auto` to the
containing block instead of shrink-to-fit (float / inline-block / abspos / flex-item / `fit-content`),
fix that one primitive, and prove it on all four near-bar sites before claiming a crossing.
