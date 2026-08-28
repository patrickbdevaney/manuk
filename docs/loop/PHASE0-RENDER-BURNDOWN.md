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

---

## 8. The t1109 sweep — the corpus is FLAT, and the remaining gap has a SHARP work-list

**200-site CrUX trend corpus, run against the tree at t1108** (`docs/loop/SWEEP-t1109-rows.tsv`). The
board had flagged the sweep 602 hours stale, with ten ticks landed against an unmeasured headline.

```text
   scored 110 of 133 in-scope (82.7% scorability ceiling · 67 excluded bot-wall/unreachable)
   mean coverage 86.8%   mean shape 60.6%
   shape >= 0.75 ..................  39/133  29.3%   (t1099: 37/133  27.8%)
   M1 = shape>=0.75 AND jarring-clean  23/133  17.3%   (t1099: 17.3%)
   COMMON-SET BAND over the 104 sites scored in BOTH  ....  -0.12 pts
```

⚠⚠⚠ **THE ANCHOR IS NOT THE CORPUS, AND THE GAP BETWEEN THEM IS THE FINDING.** t1107 and t1108 moved
`en.wikipedia.org` **0.6609 → 0.7876 (+12.7 pts)** and killed its entire 394-element horizontal
overflow. The corpus moved **−0.12 pts**, with **73 of 104 sites byte-flat**, 14 up and 17 down. The
pass-count rose +1.5 pts, which is inside its own ±2–4 site noise.

That is not an argument that the ticks were wrong — `css/CSS2` moved +36 with 0 lost across the two,
and both fixes are Chrome-exact on their own subjects. It is the standing lesson firing at corpus
scale: **usage weight ranks where to LOOK; only a probe says whether anything is THERE.** t1107 priced
`::before`/`::after` on an inline element at 50% of corpus pages and the corpus still did not move,
because on most of those pages the pseudo is a quote mark or an icon that adds a few px of ink, not a
separator inside a width-constrained container that changes how a line breaks.

### 8.1 The ranked work-list, and it is much sharper than "219 root causes"

The mechanism oracle reports 219 distinct causes over 1,965 divergences and its top cause explains
**5 sites** — no single mechanism covers 5% of the scored corpus. Decomposing **M1's conjunction**
instead is far more actionable:

```text
   of the 110 scored sites
     M1 PASS (shape ok AND jarring-clean) ..........  20
     shape ok, blocked ONLY by jarring .............  19   <- the cheapest tier
     jarring-clean, blocked only by shape ..........  13
     neither .......................................  58
   which invariant blocks those 19:  reading_order 16 · h_overflow 11 · overlap 11 · dead_target 1
   shape near-miss (0.65 - 0.75, one tier below the bar)  19 sites
```

**THREE SITES ARE ONE DEFECT AWAY FROM M1**, and they are named:

```text
     sports.yahoo.com     shape 0.881    ONE reading-order pair
     hnhbkis.edu.in       shape 0.932    TWO h-overflow elements
     www.marktplaats.nl   shape 0.962    TWO h-overflow elements
     ...then aksesjambi.com (4), redinfor.com.pe (4), simplepdf.com (5), freesupertips (6)
```

Seven sites are blocked by six or fewer jarring pairs each. That is +7 M1 sites — 17.3% → 22.6% —
from defects small enough that each one is a single reducible container, which is the shape of work
this loop is good at. **Take them in that order.**

### 8.2 The open control

Four sites lost `overlap`-clean across the window (`desiviral.net`, `mayatoys.in`,
`pordentrodetudo.com.br`, `sestra.cc`) and two lost `h-overflow`-clean (`mayatoys.in` 0→77,
`www.repubblica.it` 0→320). The window spans **ten ticks and eight hours of live-site drift**, so
none of it is attributable yet: an OLD-BINARY run on those six sites, in the same hour, is what
decides whether any of it is ours. `mayatoys.in` is the ambiguous one in both directions — it also
gained **+0.436 shape**, which is the signature of a site that went from barely-rendering to mostly-
rendering and picked up new invariant hits on the way.


### 8.3 ⚠ A JARRING COUNT OF 1 DOES NOT MEAN THE PAGE IS NEARLY RIGHT (t1118)

The §8.1 work-list ranks the "shape ok, blocked only by jarring" tier by the **number** of jarring
pairs, and the cheapest row on the t1117 sweep was `simplepdf.com` — shape 0.865, `h_overflow` **1**,
everything else clean. `MANUK_HOVF_TRACE` on it:

```text
   HOVF-TRACE main/div:nth-of-type(1)
       chrome [0   0 1200 1596]   ours [0   177 1200 10614]     body
       chrome [0 120 1200  846]   ours [0   177 1200  9812]     main
       chrome [217 120 766 846]   ours [-22 3316 1244 1106]  <-- FIRST DIVERGENCE
```

The overflow is real and small — 1244 against a centred 766, and our `x = -22` is that width centred
correctly, so the CENTRING is right and the WIDTH is not. But read the first two rows: **our page is
10,614px tall against Chrome's 1,596 — 6.6×** — and the column starts at y=3316 where Chrome puts it
at 120. A site can be one *counted* pair from M1 and nowhere near it.

**Why the count says otherwise:** `h_overflow` counts only elements whose right edge escapes the
VIEWPORT while Chrome keeps them in. A page that is six times too tall has no horizontal component
to report, and `shape` is parent-relative, so a subtree that is uniformly wrong scores well against
its own parent. The two invariants are doing exactly what they were designed to do.

**So the tier is a place to LOOK, not a queue to work in order.** Trace the top rows before picking
one — the trace is one env var and one sweep run, and it separates "one small defect" from "one
symptom of a page-scale divergence" in a single line. `hnhbkis.edu.in` and `www.marktplaats.nl`
remain the genuine one-element rows (t1112 traced both; every ancestor exact).

### 8.4 The 499,432px `<i>` was TWO BOXES, and the corpus lesson is about the METRIC (t1119)

t1112 localised `www.marktplaats.nl` to one `<i>` and wrote *"its OWN width is 499,432"*. It has no
such width. An out-of-flow child of a flex container was emitted twice — once by `position_absolutes`
(correct, padding box) and once as a taffy item (content box, and during a shrink-to-fit measurement
that item is laid out at a 1e6 available width) — and `LayoutBox::node_rects` reports the UNION.

⚠⚠⚠ **`node_rects` UNIONS, SO ANY DOUBLE-EMITTED ELEMENT REPORTS A SIZE NO CODE EVER COMPUTED.**
Every "impossible" width in this burndown's h-overflow column should be checked against that before a
mechanism is guessed at: the exemplar's *number* can be an artefact of the box tree having two
entries, not of a wrong calculation. The instrument was honest; the reading of it was not.

The fix is landed and scoped (`css/css-flexbox` 304 → 306, +2 / −0; grid, position and sizing
byte-identical). Two follow-ons are named with their addresses:

1. **`pre_transform_rect` is a first-write-wins cache a MEASURING pass can reach.** The intrinsic
   layout at 1e6 writes first, and `position_absolutes` prefers that map over `rects` whenever the
   containing-block chain carries a transform — so marktplaats's chevron is still at x=500,059 while
   `rects` held 431.53 all along. This is the remaining half of both h-overflow elements on the
   corpus's nearest-to-M1 site.
2. **An abspos child with definite GRID placement is positioned against its grid area** (Grid §9),
   which `abs_containing_block` cannot express; that is why the fix stops at flex.

## 9. The t1121 sweep — the gate MOVED, and both of its losses were the sweep

**200-site CrUX trend corpus, release binary, `--jobs 2`, run against the tree at t1120**
(`docs/loop/SWEEP-t1121-rows.tsv`). 614 hours stale when it started; eight ticks unpriced.

```text
   scored 110 of 132 in-scope (83.3% scorability · 68 excluded bot-wall/unreachable)
   mean coverage 87.2%   mean shape 62.5%
   shape >= 0.75 ...................  44/132  33.3%   (t1117: 29.3%)
   jarring-clean ...................  53/132  40.2%   (t1117: 35.3%)
   M1 = shape>=0.75 AND jarring-clean 29/132  22.0%   (t1117: 18.8% · t1109: 17.3%)
   COMMON-SET BAND over the 104 scored in BOTH ...... +0.54 pts (16 up · 7 down)
   CORPUS fidelity gauge ............................ 0.4738
```

**The pass count and the drift-robust band moved the same way for the first time in this arc.** Read
with the ledger's own error bar (`PASS-COUNT = NOISY ±2-4 sites`, and +3.2 points is 4 sites): the
claim is *"both signals agree on the direction"*, not *"+3.2"*.

`www.marktplaats.nl` crossed exactly as t1120 predicted from a single-site A/B — `h_overflow` **2 → 0**,
shape 0.9617 → 0.9667, all four invariants clean.

### 9.1 Both attributable LOSSES were the sweep, not the engine

`manuk-wpt sweep-diff` partitions the 104 common sites into **0 instrument-changed · 7
population-changed · 21 attributable** (14 up, 7 down). Its two largest losses, run SOLO in the same
hour against the exact binary the t1117 sweep measured (the t1118 tree — t1117 and t1118 were both
measurement ticks):

```text
                        sweep t1117   sweep t1121   SOLO old   SOLO new
     serennu.com           0.574         0.393        0.574      0.574     byte-identical
     possssno.sbs          0.991         0.911        0.991      0.991     byte-identical
```

⚠⚠⚠ **Neither reproduces on either binary, and both solo runs return the OLD sweep's value.** The two
engines agree; the sweep disagrees with itself. This is a shading the rule did not have: these are
not site drift (the solo runs are stable to three decimals), they are the **sweep's own conditions** —
two concurrent Chrome+manuk pairs against a 12s load budget — costing a site part of its render on one
run and not the next. A `--jobs 2` row is bankable for the DENOMINATOR and still noisy PER SITE, so a
per-site loss on a sweep row is a question, never a finding.

### 9.2 The refreshed work-list (shape OK, blocked ONLY by jarring — 20 sites)

⚠ Read §8.3 first: the tier is a place to LOOK, not a queue. Trace before picking.

```text
   www.lyreco.com          shape 0.756   1   reading-order 1
   www.jatekshop.eu        shape 0.771   1   reading-order 1
   simplepdf.com           shape 0.865   1   h-overflow 1     <- t1118 traced: page is 6.6x too tall
   aksesjambi.com          shape 0.890   2   overlap 2
   hnhbkis.edu.in          shape 0.932   2   h-overflow 2     <- t1112 traced: ONE <div>, ancestors exact
   redinfor.com.pe         shape 0.861   4   h-overflow 3 · order 1
   www.kuechenmomente.de   shape 0.786   5   reading-order 5
   www.freesupertips.com   shape 0.769   6   h-ovf 1 · overlap 1 · order 4
   www.tz.de               shape 0.850   8   h-ovf 3 · overlap 1 · order 4
   rockstaractu.com        shape 0.897  12   reading-order 12
   www.wdimax.com          shape 0.966  12   reading-order 12
   www.otomoto.pl          shape 0.756  13   h-ovf 1 · overlap 1 · order 11
   www.ikea.com            shape 0.758  22   reading-order 22
```

**`hnhbkis.edu.in` is the one row on this tier that is BOTH cheap and already traced** — t1112 found a
single `<div>` 480px wide inside a 230px parent with every ancestor byte-exact, and refuted the obvious
Tailwind `object-cover` reading in the same tick. It is the next engine target.

⚠ **And the tier's composition has changed shape: `reading_order` is now the dominant blocker** (five
of the top thirteen rows are reading-order-only, including three sites whose ONLY defect is 12–22
reorder pairs at shape ≥ 0.90). That is a different mechanism family from the width/overflow arc the
last eight ticks worked, and it is where the next *family* sweep should look.

## 10. The t1127 sweep — the count is flat, the MEMBERSHIP is +2 engine / −2 refuted

```text
   scored 108 of 133 in-scope (82.4%)          t1121: 110 of 132 (84.0%)
   shape >= 0.75 ...........  42/133  31.6%    (t1121: 33.3%)
   jarring-clean ...........  52/133  39.1%    (t1121: 40.2%)
   M1 conjunction ..........  29/133  21.8%    (t1121: 22.0%)
   COMMON-SET BAND over 106 ... +0.15 pts (10 up · 9 down)
   corpus gauge ............... 0.4692         (t1121: 0.4738)
```

**The count says nothing moved. The membership says the opposite, and the membership is the true
statement** — §8's standing rule (*diff the state, not the net*) applied to the gate itself:

```text
   GAINED M1   www.wdimax.com          reading_order 12 → 0     t1124 (predicted)
               www.kuechenmomente.de   reading_order  5 → 0     t1124 (unpredicted, free)
   LOST M1     app.ordertime.com       cov 1.000 → 0.040 · reason `tree-divergence-31`
               gismart.com             h_overflow 0 → 6
```

Both losses are refuted by same-hour controls:

- **`app.ordertime.com` did not lose shape — it stopped RENDERING.** A scorability dropout wearing an
  M1 loss's clothes; the row's shape is still 1.000 over the 4% of the tree that appeared.
- **`gismart.com`, 2 binaries × 2 runs:** the OLD binary (the exact tree the t1121 sweep measured)
  produces h-overflow **5 on one of its own two runs and 0 on the other**, and all four runs read
  shape **84.0%** against the sweeps' 0.872 / 0.843. The site is flaky; the t1121 row was the lucky
  run.

⚠⚠⚠ **`probidas.lt` came back `crashed` and was checked FIRST, because Bar 0 outranks every visual
divergence.** Solo, same hour, both binaries render it at shape 29.9% with no crash. That is the
**third** site this window where the sweep's own conditions produced a reading neither binary
produces (§9.1 had two). The rule is now firm: **a `--jobs 2` row is bankable for the DENOMINATOR and
is not evidence about any single site.** Every per-site delta the sweep flags is a question.

### 10.1 What this says about pricing render work

Four Chrome-exact, RED-proven fixes; `css-flexbox` 304 → 309, `css-grid` 208 → 211, zero regressions;
two M1 crossings confirmed by an independent full corpus run. **The corpus-level pass count moved by
zero.** This is the high-usage/low-magnitude case VI.3 and check #72 both named: the gate prices a fix
by whether it crosses a per-site threshold, and most correct fixes do not cross one on most sites.
Report *"+N attributable sites, count flat"*, never a percentage — and rank the next tick on the
membership list, not on the headline.

## §11 — THE NEAR-MISS BAND, RANKED BY MECHANISM (t1316 sweep, `corpus-crux-trend.txt`)

The board has asked since t684 for the burndown to rank by PRIMITIVE rather than by tag. This is that
list, derived from the t1316 sweep (147 of 203 attempted before the Bar-0 death; 85 scored).

### 11.1 The bar, and the cheapest cohort

```text
   67 well-sampled sites (≥20 scored ids) · shape ≥ 75% = 30 = 44.8% · median 73.0%
   19 of them sit at 60–75% — ONE BAND below the bar
```

⭐ **Converting the 60–75% band alone takes the headline from 44.8% to ~73%.** Several of those sites
carry 1,000+ scored ids (`fragrantica` 2,985, `repubblica` 2,487, `crazyshop` 1,402,
`razaoautomovel` 1,196, `mobile.ir` 1,172, `mangaraw` 1,153, `puentedemando` 1,131), so they are
statistically solid, not lucky.

### 11.2 ⭐⭐⭐ The mechanism is ONE axis, and it is not the one coverage work targets

| signal | value across the band |
|---|---|
| first divergence on the **`dy`** axis | **16 of 16** sites that have one |
| first divergence on `dx` / `dw` / `dh` | **0** |
| median `dw`, median `dh` | **0 on 17 of 19 sites** |
| boxes MISSING | 2.5% (6.6% including `agoda.com`, an 88.7% bot wall) |
| boxes MISPLACED | **97–99%** on the large sites (`oilprice` 653/654, `fragrantica` 2,967/2,987) |

**We draw essentially every box, at essentially the right size, in the wrong vertical place.** That is
not a missing-box problem and it is not a sizing problem — the width and height arithmetic already
agrees with Chrome on these pages. MISSING_BOX work cannot move this band.

### 11.3 The band splits in two, and the split is the work order

Comparing each site's FIRST divergence against its MEDIAN `dy` separates a shift from an accumulation:

```text
   ACCUMULATES  (median dy is 3.5–35× the first divergence)          8 sites
      www.repubblica.it     -1313 → 4552   (2487 ids)
      www.fragrantica.com     435 → 3908   (2985 ids)
      www.paypal.com           88 → 3075   ( 534 ids)
      www.razaoautomovel.com  303 → 3030   (1196 ids)
      www.ta3lemkonline.com  -229 → 2945   ( 459 ids)
      momon-ga.com           -390 → 1917   ( 572 ids)
      www.puentedemando.com   139 → 1277   (1131 ids)
      www.mobile.ir           -69 →  356   (1172 ids)

   CONSTANT SHIFT (median ≈ first)                                   2 sites
      mangaraw.ac             254 →  253   (1153 ids)
      ticket.jfa.jp            71 →   35   ( 682 ids)

   EARLY OUTLIER that does NOT propagate (median ≪ first)            6 sites
```

⭐⭐⭐ **THE ACCUMULATING EIGHT ARE THE TICK LIST, AND THEY CARRY THE BAND'S MASS** (10,536 of the
band's ~16,800 scored ids). A dy that grows 35× down the document while every matched box keeps the
right height means the error is in the vertical GAPS, not in the boxes: margins, line-box advance, or a
box that occupies space we never account for. The right-size/wrong-place signature is the one
`docs/wiki/box-layout.md` records for a coordinate-space fault (t1272), and *"a constant shift with
correct sizes"* is a different bug from *"a growing shift with correct sizes"* — the two classes above
must not be worked as one.

### 11.4 ⚠ Two steers in this document's own governing files were STALE, and both were checked

- **`CONSTITUTION.MD` PART VI.2 H0.1** cites `css/selectors/invalidation/has-complexity.html` as a
  reproducing Bar-0 `CRASH`. It is now **7/7 PASSING** — t1161's `:has()` anchor memo closed it. ⚠ It
  runs in **4,720ms against a 5,000ms script watchdog**, so it is one step from truncating instead, and
  the underlying mechanism (every mutation costs a full re-cascade) is untouched.
- **The lever board's CO-#1 item (2)**, *"MECHANISM ORACLE — NEARLY FREE: `run_oracle_merge` DISCARDS
  the signature back to `geometry:<tag>`"*, is **already implemented**. `main.rs` calls
  `manuk_wpt::oracle::signature_of(&d)` and carries a comment saying the key *"is no longer computed
  here."*

> **A steer is a claim with a date, and both of these outlived their fact.** The cost of checking each
> was one command; the cost of not checking was a tick spent implementing something that exists.

## §12 — THE FIRST COMPLETE SWEEP UNDER THE REPAIRED INSTRUMENT (t1322, `corpus-crux-trend.txt`, 200/200)

t1316's sweep died at site 147; this one finished all 200, and it is the first taken after t1319
corrected the reference's scrollbar provisioning. Every absolute number in §8–§11 predates that
repair and must not be diffed against these.

### 12.1 The gate, in the tracker's own framing (`scripts/fidelity-progress.sh`)

```text
   M1 GATE (shape>=0.75 AND jarring-clean)   33/133 = 24.8%   target 95% = 127  →  NEED +94
     ├─ shape >= 0.75                        48/133 = 36.1%
     └─ jarring-clean (TOL2)                 52/133 = 39.1%
   scorability                              108/133 = 81.2%   shape_mean 63.9%  cov_mean 86.9%
   EXCLUDED (bot-wall/unreachable, watched)  67/200 = 34%
```

⚠ **The tracker's printed Δ is against `t1275`, which was the CONTENDED `--jobs 8` sweep** — so its
`+17.4 pts` is an artifact of the baseline, not a gain. The honest comparator is the last clean
sweep, `t1268`:

```text
                        t1252    t1268    t1322
   in-scope shape        33.6     34.6     36.1
   jarring-clean         32.4?    38.3     39.1
   M1 gate               23.3?    23.3     24.8
```

### 12.2 The near-miss band, re-ranked on fresh data

22 sites sit at 60–75% carrying **16,915 scored ids**. Converting the band alone takes the
well-sampled headline from **46.6% to ~68%**.

```text
   www.fragrantica.com    73.3%  2980      probidas.lt            62.3%   650
   www.repubblica.it      72.8%  2566      momon-ga.com           60.3%   572
   www.crazyshop.pl       68.5%  1402      www.paypal.com         65.2%   534
   www.razaoautomovel.com 70.7%  1195      www.freesupertips.com  70.8%   473
   www.mobile.ir          70.0%  1179      www.ta3lemkonline.com  61.7%   459
   www.hdnails.it         66.1%  1116      www.5movierulz.discount 68.8%  439
   7info.ru               72.1%   820      pivaldi.restoplace.ws  74.6%   378
   nysainfo.pl            68.2%   770      www.unoeste.br         73.9%   276
   oilprice.com           66.5%   654      pasarbokep.com         71.7%   251
```

`ticket.jfa.jp` and `mangaraw.ac` have left the band (t1319 put `ticket.jfa.jp` at 82.1%).

### 12.3 ⭐⭐⭐ THE PER-SITE ERROR BAR IS NOT A CONSTANT, AND THE WORKLIST MUST BE BUILT ON THAT

Three sites in this sweep read as large regressions. All three were refuted, and the refutations took
a same-hour old-binary control:

```text
   serennu.com        sweep 49.2   serial re-run 73.8   (= its t1316 value)   contention
   www.unoeste.br     runs: 66.9 · 84.5 · 84.9 · 73.1 · 82.7   —  SPREAD 18.0 pts on 441–445 ids
   www.freesupertips  new 70.8  vs  OLD BINARY, same hour 68.6  —  the new code is BETTER
```

⭐ **`oilprice.com` re-ran at 66.1 / 66.1 — a spread of ZERO on 654 ids.** So the instrument's
per-site error bar ranges from **0.0 to 18.0 points**, and it is a property of the SITE, not of the
run. A single reading on `unoeste.br` means nothing; a single reading on `oilprice.com` is solid.

**The rule this imposes on every future tick in this document:** a per-site claim needs the site's own
error bar measured first (≥2 serial runs), and the ranked worklist should be worked from the STABLE
end. `oilprice.com` — 654 ids, spread 0.0, 66.5% — is the best anchor in the band by that test, which
is what §11's own NEXT already guessed and can now justify.

⚠ And the general rule the three refutations share: **a `--jobs 2` sweep is still a contended sweep**,
and a down-mover in a sweep is a SUSPECT, not a regression, until a same-hour serial run and (if it
survives that) an old-binary control have both been taken.

## §13 — THE BAND SPLITS INTO FONT-LIMITED AND LAYOUT-LIMITED, AND THE CLASSIFIER WAS ALREADY BEING PRINTED (t1334)

§11 ranks the near-miss band by mechanism, and its largest family is `geometry/mis-sized: width`. ⚠ At
least a quarter of that mass is **not a geometry mass**. The oracle already prints, on every `e.g.`
line, the font both engines measured through canvas `measureText` — `{family/size/metric} vs
{family/size/metric}` — and **nothing reads it**.

Reading it, one serial run per site:

```text
   site                     shape    ids    font-SAME  font-DIFF   class
   www.fragrantica.com      72.3%   2976       20          0       LAYOUT
   www.repubblica.it        73.0%   2467       20          0       LAYOUT
   www.crazyshop.pl         68.2%   1402       13          0       LAYOUT
   www.razaoautomovel.com   70.4%   1264       14          0       LAYOUT
   www.mobile.ir            70.1%   1176        6         13       FONT
   www.hdnails.it           62.1%   1076        0         17       FONT  (pure)
   7info.ru                 70.7%    877        0          9       FONT  (pure)
   nysainfo.pl              84.4%    969        3          1       LAYOUT — and it has LEFT the band
```

⭐⭐⭐ **Three of eight anchors are FONT-limited and two are pure** — zero examples where the two
engines measured the same face. They carry **3,129 of the 12,207 ids sampled, 26%**. And the band's
LOWEST-scoring anchor (`hdnails.it`, 62.1%) is the purest font case, so the worst sites are
disproportionately font rather than layout.

### What this changes about the work order

- **A font-limited site cannot be moved by a layout tick**, and its histogram's top bar
  (`geometry/mis-sized: width`) reads exactly like a layout lever. t1333's six-row control established
  that our shaping and advance accumulation are Chrome-exact on every system family — so on these
  sites the divergence is a webfont Chrome loaded and we did not, and the whole page's text is 20%
  wide.
- **Take the LAYOUT-limited anchors for layout ticks** (`fragrantica` 2,976 ids and `repubblica`
  2,467 ids are the two biggest samples in the band and are 20/0 clean).
- **Take the font-limited ones as ONE webfont-loading investigation**, not as eight geometry ticks.

⚠ `nysainfo.pl` reads 84.4% here against 68.2% at the t1322 sweep. Sites move; the band's membership
is a snapshot, and a per-site number older than a few hours is a hypothesis (t1327).

## §14 — THE FIRST SWEEP AFTER 21 BLIND TICKS, AND 4.1% OF THE SCORED CORPUS IS THE INSTRUMENT (t1344)

`SWEEP-t1344-rows.tsv`, `corpus-crux-trend.txt`, **200/200 rows**, one binary, no build sharing the
clock. The previous complete sweep is `SWEEP-t1322` — **21 ticks and 8 days ago**, so nothing below is
attributable to a single tick, and live-page churn is inside every per-site delta.

```text
                          t1322      t1344
   rows                     200        200
   scored                   124        122
   shape >= 0.75             50         52
   mean shape (scored)    0.5872     0.5993
   ── on the 120 sites scored in BOTH ──
   mean shape             0.5780     0.5938    (+0.0159)
   shape >= 0.75              47         50    (+3)
```

Distribution of the 122 scored: `<0.25` **25** · `0.25–0.50` 12 · `0.50–0.75` 33 · `0.75–0.90` 30 ·
`>=0.90` 22. Median **0.702**. The 78 unscored are `bot-wall-403` 35, `unreachable` 14,
`timeout-150s` 8, `probe-blocked` 6, `http-404` 5, `bot-wall-200` 4, `empty-202` 3, `css-starved` 3.

### 14.1 ⚠⚠⚠ FIVE OF THE TWELVE DEEP-FAIL ROWS ARE ONE SITE, AND THE REFERENCE IS UNSTYLED

The deep-fail cohort (`shape < 0.25`, `n >= 60`) is 12 sites / 13,975 ids. **Five of them are
trivago**, one codebase in five locales:

```text
   www.trivago.fr   0.116   cov 0.966   n 1350
   www.trivago.be   0.120   cov 0.966   n 1348
   www.trivago.pl   0.114   cov 0.966   n 1348
   www.trivago.jp   0.119   cov 0.966   n 1339
   www.trivago.de   0.113   cov 0.911   n 1280
```

Coverage **0.966** with shape **0.11** is the signature of *every box drawn, nearly every one in the
wrong place* — which reads exactly like the layout work this document ranks. **It is not.** The
oracle's own diagnostic lines say so, and they have been printed all along:

```text
   365 hit(s)  display: inline → block            (<a>)    inline vs block
   365 hit(s)  font-resolution: Times New Roman/16/148  vs  -apple-system/16/172   (<li>)
```

`Times New Roman` and `display: inline` on 365 anchors is a document with **no CSS applied at all**.
`-apple-system` and `block` is trivago's own stylesheet — **ours**. ⭐ **We are rendering this site
correctly and the REFERENCE is not**, and the metric charges us 0.11 for it, five times.

**Reproduced outside the oracle, and four candidate causes eliminated:**

```text
   the oracle's exact document (curl + spliced <base>), file://          sheets=0
   …the same, with EVERY <script> stripped                              sheets=0
   …the same, served over http://127.0.0.1 (a real origin)              sheets=0
   …the same, with --allow-file-access-from-files                       sheets=0
   a ONE-LINK control on the SAME href, same base, same flags           sheets=1   <-- loads fine
```

So it is **not** the `<base>` splice (`document.querySelector('base').href` is exactly
`https://www.trivago.be/`, its parent is `HEAD`, and all 7 `link[rel=stylesheet]` resolve to the real
absolute https URLs), **not** the `file://` origin, **not** scripts, and **not** the CSS — which
answers `200 text/css`, 36 KB, to any User-Agent, with or without a `Referer`. Every one of the 7
links has `sheet === null`: the fetches *failed*, in a document whose `<head>` has **103 children**,
while the identical fetch in isolation succeeds.

**NEXT PROBE, named:** bisect the head — keep the 7 stylesheet links and delete the other ~96 children
(preloads, preconnects, `data-next-head` metas) — and if they then load, the cause is the request
BURST (Akamai shedding ~100 concurrent requests that carry no page Referer), not the markup. Until it
is known, **these five rows are not layout work**, and any burndown that ranks on them is ranking the
instrument.

### 14.2 §13's FONT-LIMITED CLASSIFICATION MOVED — ON A FONT-METRICS TICK, WHICH IS THE POINT

§13 (t1334) split the near-miss band into LAYOUT-limited and FONT-limited and named `hdnails.it`,
`7info.ru` and `mobile.ir` as FONT-limited, two of them *pure*. On this sweep:

```text
   www.hdnails.it   0.661 -> 0.792   (+0.130)
   www.mobile.ir    0.700 -> 0.812   (+0.112)
```

Both have **left the band**, and t1343 — which fixed `line-height: normal` to union the metrics of the
FALLBACK faces a run actually draws from — is the obvious candidate: `mobile.ir` is Persian, so its
glyphs come from Noto Sans Arabic through exactly that path. ⭐ **The classifier was right that these
were font problems and wrong about which font problem**: it read "different family measured" and
inferred *a webfont Chrome loaded and we did not*; at least part of it was *the same text, measured
with the wrong face's METRICS*. The webfont investigation (§13's own (b)) is therefore **smaller than
it looked and still open** — re-price it before building.

### 14.3 The re-ranked near-bar band (0.55–0.75, `n >= 60`) — 21 sites, 17,538 ids

```text
   www.alphanews.live       0.747  n 3858     <- +0.003 from the bar, and the largest sample
   pivaldi.restoplace.ws    0.743  n  378     <- reads 1.000 FROZEN; this row is live churn
   serennu.com              0.737  n   61
   www.puentedemando.com    0.733  n 1136
   7info.ru                 0.728  n  820
   www.razaoautomovel.com   0.713  n 1295
   pasarbokep.com           0.713  n  251
   www.freesupertips.com    0.712  n  501
   www.repubblica.it        0.706  n 2456
   patrickmorin.com         0.697  n  142
   nysainfo.pl              0.695  n  761
   ru.restaurantguru.com    0.682  n  611
   mydogcot.com             0.676  n  463
   ticket.jfa.jp            0.665  n  218
   www.crazyshop.pl         0.664  n 1402
   www.paypal.com           0.647  n  534
   www.ikea.com             0.643  n  648
   momon-ga.com             0.638  n  572
   www.ta3lemkonline.com    0.616  n  459
   developers.google.com    0.608  n  340
   www.netvasco.com.br      0.575  n  632
```

Two named mechanisms already dumped out of this band, both Chrome-measured:

- **`momon-ga.com` — a WRAP.** `x +753  c[763 0 231x336]  m[10 393 231x336]`: four identical 231px
  cards, one 1004px container, same sizes — Chrome fits four on the row, we wrap after three.
- **`www.puentedemando.com` — a COLLAPSED section.** `height +443  c[0 186 1200x463]
  m[0 186 1200x20]`, and the whole page below it shifts by the same 443. Its own top diagnostic is
  `120 hit(s) font-resolution: Google Sans/16/168 vs Google Sans/16/162` — the SAME family name with a
  6px-per-168 metric difference, which is a webfont *variant* question, not a fallback question.

⚠ `pivaldi.restoplace.ws` reads **0.743 live and 1.000 frozen**. A per-site row is a draw from a
distribution whose width is the site's own churn (t1341/t1343): rank on it, but re-measure frozen and
with a 3-run spread before pricing anything against it.
