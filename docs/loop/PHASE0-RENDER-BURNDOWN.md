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

## 5. Two structural forks — DECIDED by the owner (2026-07-29)

Both settled; the sequencing below is now authoritative, not advisory:

- **Corpus — DECIDED: burn down on the 265 now, re-certify on `corpus-v2.tsv` at the end.** Fast iteration
  on stable anchors (hackernews/wikipedia/…) while the slope is being established; once in-scope-pass
  crosses ~50%, re-run the certificate on the representative CrUX/Tranco corpus-v2 (built t581, currently
  unused) for the headline "95% of the web" claim. Do NOT switch the working corpus mid-burndown.
- **Function-on-real-sites — DECIDED: build the BiDi per-site function leg AFTER the render burndown, as
  the final Phase-0 gate.** Render is the visible binding gap and goes first; function-on-real-sites
  (cert §4 Layer C-function) is the last gate before certification. The capability breadth to pass it is
  largely already there per `PHASE0-BOUNDED-REMAINDER.md` — this is a measurement build, not a big new
  capability arc. Until then, function remains fixture-tested only (a known, named Phase-0 gap, not a
  silent one).

## 6. Division of labour (unchanged doctrine)

- **Observer:** this doc, the winnable metric, the burndown slope + alerts, the board/launch-prompt steer,
  banking sweep actuals, flagging the §5 forks. Never edits `engine/`/`manuk-wpt`; never runs sweeps.
- **Agent:** the fresh sweep, the mechanism-oracle dimension, the per-primitive engine fixes, the per-site
  `boxes --why` diagnosis. Instrument + engine are agent territory.
