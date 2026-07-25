# PHASE-0 ROADMAP ANCHOR — the verified bound, the remaining ledger, and how we know it converges

_Anchored 2026-07-23 at tick 461. This is the FIXED reference the loop is measured against until
the Phase-0 certificate holds. Every full corpus sweep updates the ACTUALS columns; if actuals
drift >20% past the bound, the drift itself becomes a tick: re-derive, don't rationalize._

## 1. The claim, verified against measured data

**Phase 0 completes in ~85–150 ticks from tick 460** (so by roughly tick 545–610). Verification:

- **Cadence (measured, not assumed):** ticks 330→461 landed in 43 hours = **72 ticks/day**.
  The bound's own math assumed 25–40/day, so the calendar estimate (3–6 days) is conservative;
  at measured cadence the midpoint is **1.5–2.5 days of loop time**.
- **Convergence (measured twice):** t380 full sweep → t392 re-sweep collapsed the ledger
  627→392 clusters in ONE cycle; the two largest families (img/svg inline ~80 sites each,
  `<br>` missing box 64→5) vanished wholesale; all four jarring invariants improved.
  Head-heavy, cause-driven behavior — the opposite of a Presto tail.
- **Bound integrity so far:** the original 100–150 bound (set ~tick 330) has spent ~130 ticks
  and delivered the ENTIRE exit instrument, the full media/codec chain, and most of the S/M
  list — on budget with the 1.5× subsystem-overrun factor already priced in.
- **The cut line is half the proof:** 16 named exceptions (DRM, WebRTC, WebGPU/Figma-class,
  canvas-office EDITING, Web Audio, push, HTTP/3, …) mean the target is "almost no site is
  jarring," not "every capability exists." A bounded goal is what makes a bounded plan possible.

## 2. The remaining ledger (from tick 461)

### A. Named build items — ~49–88 ticks

| item | size (ticks) | status | verified by |
|---|---|---|---|
| **Rich-editing subsystem** (contenteditable commands, caret, typing; IME composition already landed t332) | 10–20 | ENTRY LANDED t456–457; in progress | new g_editing gates + Gmail-compose-class page gate |
| **Software WebGL backend** | 7–15 | not started; **down-scopeable** (Maps degrades to raster) | g_webgl context + honest-strings gate |
| WebAuthn/passkeys | 3–6 | not started | real-flow gate (register+assert round-trip) |
| Password vault UX (crypto core exists) | 3–6 | not started | save-prompt/fill-picker shell gates |
| bidi full line reordering | 3–4 | not started | RTL fixture gates + corpus RTL sites |
| Live CSS animation/transition timeline | 4–8 | end-state-only today | g_animation upgraded to timeline assertions |
| Form widget painting remainder (date/color/time pickers, select popup, accent-color) | 2–4 | not started | paint-diff gates vs measured Chrome |
| Visual-effects bundle (filter, backdrop-filter, mix-blend, clip-path) | 4–6 | not started | reftest-style paint gates |
| MathML | 3–6 | not started | Wikipedia-formula fixture gate |
| multicol | 3–4 | not started | layout gate + news-site corpus rows |
| Print output | 2–3 | @media print applies; output missing | printable-surface gate |
| ch/ex real font metrics (Parley borrowable) | 2–3 | stubbed | font-metrics gate + ~194 WPT |
| misc S items (residuals) | ~3 | — | per-item gates |

### B. Ledger-driven jarring fixes — ~30–60 ticks (the fuzzy bucket, see §4)

Current invariant state (t392 sweep, share of scored sites FAILING → required ≤5%):

| invariant | t380 | t392 | Δ/cycle | distance to bar |
|---|---|---|---|---|
| overlap | 45.2% | 43.7% | −1.5 | 38.7 pts |
| h-overflow | 33.5% | 30.3% | −3.2 | 25.3 pts |
| **reading-order** | 71.6% | 63.0% | −8.6 | **58.0 pts** ← the fat-tail risk |
| dead-target | 47.2% | 32.8% | −14.4 | 27.8 pts |

Points-per-cycle is NOT the right extrapolation (fixes target CAUSES; a killed cause removes
many points at once — dead-target dropped 14.4 in ~4 ticks). The ledger's cluster count per
invariant after each sweep is the honest progress meter.

### C. Exit verification — ~5–10 ticks

test262 run · 100-tab RSS benchmark · large-DOM interactivity probe · repeat full-corpus
sweeps until the certificate holds on consecutive runs.

**Total: ~84–158 ticks — consistent with the 85–150 claim.**

## 3. Definition of done (unchanged, mechanical)

The FIDELITY-SCORING-REDESIGN certificate, measured by the rebuilt instrument on the stratified
corpus: **Bar 0 (zero crash/hang) + all four jarring invariants ≥95% clean + shape ≥0.75 on
≥95% of sites + interactivity ≥95% + only the 16 named exceptions unmet.** Never ready_pct,
never WPT count, never a vibe.

## 4. Risk register (ranked)

1. **Reading-order invariant variance** — 58 points to close; the widest error bar in the plan.
   Decision gate: after the next TWO full sweeps, if the reorder CLUSTER COUNT is not shrinking
   ≥30% per cycle, stop grinding and re-derive: either (a) the residue is one structural cause
   (float/abspos ordering — fix the cause, not the sites), or (b) the invariant OVER-COUNTS
   benign reorderings (flex `order`, positioned navs) and needs refining against measured Chrome
   behavior — refine the definition with evidence, never retune the bar to pass.
2. **Rich-editing overrun** — the one remaining true subsystem; historical L-overrun is 1.5–2×
   (priced in at the 20-tick top). If it exceeds ~25 ticks, decompose and interleave.
3. **WebGL scope** — 7–15 ticks for a capability whose absence mostly DEGRADES (raster Maps
   fallback). This is the designated pressure-relief valve: if the total tracks toward the top
   of the bound, down-scope WebGL to context+honest-strings and move the rest post-certificate.
4. **Wall flakes** (G_INTERACT timing, loaded-first-verify ceiling trips) — ~4/day, ~9 min each,
   all self-healing to date. Watch frequency; a rolling-min ratchet is the ready fix if it grows.

## 5. Re-anchoring protocol

- Each full corpus sweep appends one line here: `sweep @tick N: clusters X, invariants a/b/c/d,
  ticks-spent-since-anchor S` — actuals vs bound, in one place.
- Drift >20% over the 158 top → a mandatory re-derivation tick (measure why, update the bound
  with evidence, journal it). The bound is falsifiable on purpose; that is what makes it a bound.

## 6. ACTUALS — exit-verification runs (§2C), appended one line per run

- `test262 @tick 546: 94.14% of 87,009 executed subtests (81,908 passed / 5,101 failed) · honest
  81.41% of the 100,617 the ratified suite defines (13,608 skipped: 10,739 async · 1,642 module ·
  1,225 host-API · 2 measured-hang) · 51,922 files, tc39 rev 7a096c20 · wall 140s · runner
  manuk-wpt test262.` **First run ever.** Reference point: Ladybird publishes 97.8% — on a suite
  that includes the async and module goals we skip, so the comparable figure is the honest 81.41%
  until those two are wired, and the gap is mostly *ours to run*, not the engine's to pass. Named
  gaps behind the 5,101: `intl402/Temporal` 1,956 (Stage-3 proposal), `Atomics`+`SharedArrayBuffer`
  718 (embedder must enable shared memory), `DisposableStack`/`AsyncDisposableStack`/
  `SuppressedError` 360 (explicit resource management), `ShadowRealm` 114. Three findings the run
  produced on its own are in the tick-546 journal entry; one of them is Bar 0.

- `sweep @tick 549 (STRATIFIED 72-site sample, 54 scored): certificate NOT MET on every term.`
  **shape ≥0.75 on 3 of 54 sites (5.6%)** · h-overflow clean 77.8% · overlap clean 59.3% ·
  reading-order clean 46.3% · dead-target clean 75.9% · 16 of 54 UNSCORED (shape sample <10). Bar on
  every term is 95%. Rows: `docs/loop/SWEEP-t549-rows.tsv`. Driven by
  `manuk-wpt fidelity --urls-file` in 24 timeout-isolated chunks of 3; 3 chunks hit the 600s cap and
  their sites are absent (18 of the 72 sampled), so **this reading is biased OPTIMISTIC — the sites
  that timed out are the slow ones.**
  ⚠ **The headline was 21.8% until the instrument was fixed mid-tick.** `shape_stats` returns a ratio,
  and `0/0` is `1.0`, so seven sites reported `SHAPE: 100.0% … (0 scored)` and were counted as meeting
  the placement bar — including `gov.uk`, whose 418 probed elements were **all missing**. Nine of the
  twelve apparent passes were vacuous. `CERT_MIN_SHAPE_SAMPLE = 10` now makes a thin sample UNSCORED.
  **NOT COMPARABLE to the §2 t380→t392 table**: different keying (selector-path), different metric
  (parent-relative SHAPE, not absolute placement), and a different corpus slice. This line is the new
  baseline; the next sweep is the first one that may be differenced against it.
  **THE FINDING that outranks the drift numbers: 13 of 54 sites render under 5% of what Chrome does** —
  nytimes 0.04%, stripe 0.14%, reactjs 0.13%, notion 0.32%, terraform 0.30%, bitbucket 0.36%, and
  cdc/intel/gov.uk/harvard/newyorker/propublica/squarespace at 0.0%. That is a CLASS failure, not
  placement drift, and it is where the next ticks go. Pooled root causes across the sweep, ranked by
  sites explained: `missing box: <div>` 20 · `geometry: height ~16px (<div>)` 17 ·
  `geometry: height ~64px (<div>)` 16 · `missing box: <a>` 15 · `missing box: <path>` 12 ·
  `missing box: <svg>` 9.

- `CORRECTION to the t549 line @tick 550:` the class-signature ablation shows **the sub-5%-coverage
  "class failure" was substantially the INSTRUMENT.** `gov.uk` coverage 0.0% → **82.8%** and
  `stripe.com` 0.1% → **43.1%** with `.SIG` off the path key, while three healthy sites
  (jvns.ca / blog.rust-lang.org / lobste.rs) are **byte-identical** — so the signature adds no
  discrimination and destroys the measurement whenever one ancestor's class list differs between the
  engines. Off by default from t550 (`MANUK_G1_CLASS_SIG=1` restores it). **The t549 coverage figures for
  that class are therefore wrong in the PESSIMISTIC direction, and the four jarring percentages change
  too** (they were computed over a key intersection that just grew). The t549 line stands as the record
  of what was measured; it is NOT the baseline. The re-sweep on the corrected keying is the next actuals
  line, and it is the first one that may be differenced. `nytimes.com` did **not** move (0.0% → 0.0%,
  2,381 of 2,382 still missing) — a genuine second failure that the homogeneous reading would have hidden
  behind the keying fix.

- `sweep @tick 551 (RE-SWEEP on the CORRECTED keying — THE BASELINE; 72 sampled, 55 scored):`
  **shape ≥0.75 on 3 of 55 sites (5.5%)** · h-overflow clean 74.5% · overlap clean 60.0% ·
  reading-order clean 43.6% · dead-target clean 70.9% · 15 of 55 UNSCORED (shape sample <10). Bar is 95%
  on every term. Rows: `docs/loop/SWEEP-t551-rows.tsv`. Same stratified 72-site sample and same driver as
  t549, with `.SIG` off the path key (t550).
  **THE DIFF THAT MATTERS — coverage moved a lot, SHAPE moved not at all.** Sites under 5% coverage:
  **13 → 8**. Sites at ≥90% coverage: **30 → 34**. And shape ≥0.75: **5.6% → 5.5%.** So the coverage
  collapse was substantially the instrument (t550), and **placement drift is the REAL gap** — it survived
  the correction untouched. That is the single most decision-relevant number in this file: the next ticks
  belong to placement, not to coverage.
  The four jarring terms each went slightly DOWN (h-overflow 77.8→74.5, overlap 59.3→60.0,
  reading-order 46.3→43.6, dead-target 75.9→70.9) and that is **expected and honest**: the key
  intersection grew, so more sites now have enough matched elements for an invariant to fire at all. The
  instrument sees more, so it reports more.
  Pooled root causes are now dominated by GEOMETRY, not absence: `geometry: width ~16px (<a>)` 18 sites ·
  `height ~128px (<div>)` 18 · `width ~8px (<a>)` 17 · `height ~16px (<div>)` 17 · `height ~64px (<div>)`
  16 · `height ~32px (<div>)` 13 — then `missing box: <path>` 11 and `missing box: <a>` 10. The 8/16/32/64/128
  clustering on `<div>` heights and 8/16 on `<a>` widths is a **quantised** signature, which is what a
  systematic box-model or line-height delta looks like, not what a thousand independent bugs look like.
  Residual <5% class, now 8 sites and NOT a keying artifact: nytimes.com 0.04% · newyorker.com 0.05% ·
  techcrunch.com 0.06% · reactjs.org 0.13% · stripe.com 0.14% · terraform.io 0.30% · notion.so 0.32% ·
  bitbucket.org 0.36%. **This is the named class for the next investigation.**

- `CORRECTION to the t551 line @tick 552 — the "QUANTISED" READING WAS THE PRINTER, NOT THE ENGINE.`
  t551 recorded that the geometry deltas were *"QUANTISED — 8/16/32/64/128 on div heights, 8/16 on anchor
  widths — the signature of ONE systematic box-model or line-height delta."* **`oracle::mag_band` rounds
  every magnitude DOWN to the largest power of two**, so every geometry cluster this instrument has ever
  printed carries a power-of-two headline **by construction**. The pattern read as evidence was a property
  of the printer. Lesson #4 (*every number has a harness, and the harness is part of the number*) firing on
  a conclusion drawn ONE TICK after writing the constitution check that warns about it.
  **The REAL medians, measured live at t552 with the magnitude now travelling beside the band:**
  `width (<a>)` band ~16px → **median 19px** · `height (<div>)` ~16px → **24px** · `height (<div>)` ~32px →
  **39px** · `width (<a>)` ~8px → **12px** · `x (<li>)` ~8px → **9px** · `width (<a>)` ~32px → **37px** ·
  `y (<nav>)` ~32px → **45px** · `x (<a>)` ~128px → **182px** · `x (<a>)` ~256px → **281px**. Not a power of
  two among them.
  **What the corrected numbers actually point at, which is a DIFFERENT lead:** the largest clusters by
  sites are `<a>` **WIDTH** deltas with medians of 12–19px, and `<div>` **HEIGHT** deltas with a median of
  24px ≈ one line-height. An anchor's width is its *text's* width, so that cluster is a **text-measurement**
  signature (shaping / font metrics / letter-spacing), not a box-model one — and a 24px div-height median is
  a line-box signature. t551's "one systematic box-model delta" would have sent the next tick at the wrong
  subsystem. The band still earns its place (it separates a 20px near-miss from a 4,000px collapse) but it
  must never again be read as the measurement.

- `CORRECTION to the t552 re-aim @tick 553 — the text-measurement lead is FALSIFIED, and the signature is
  too coarse to have carried it.` t552 corrected t551's power-of-two artifact and re-aimed at
  text-measurement because the largest clusters were `<a>` WIDTH with 12–19px medians ("an anchor's width IS
  its text's width"). The **first instance ever printed** falsified it:
  `lobste.rs …/li:nth-child(3)/div:nth-child(2)/a:nth-child(3): [434 183 92×17] vs [305 183 92×17]` —
  **identical in size, 129px displaced.** That is an ancestor-layout fact, not a text one. The width cases
  in the same band are 12–30px icon-ish anchors (`[130 471 18×46] vs [0 459 30×46]`), a third thing again.
  **One signature (`geometry: width ~8px (<a>)`), at least two causes.**
  So the ranked cluster list is a **grouping hypothesis, not a cause list**, and the honest statement about
  the placement gap is: it is not yet resolved to a subsystem. Two consecutive inferences drawn from cluster
  headlines were wrong, and both times the fix was to open an instance — which nothing printed until t553.
  **The tightening the ledger needs before it can rank causes again: split the geometry signature by
  whether the SIZE matches** (pure displacement vs mis-sizing are different bugs with different fixes), and
  only then re-read the ranking. That is the next instrument brick, and it outranks acting on the current
  ordering.

- `SIGNATURE SPLIT @tick 554 — the ranking is a CAUSE LIST now, and t552's lead is RESTORED on a signature
  that can carry it.` `geometry:` is split into `geometry/displaced:` (size matches within 2px — an
  ANCESTOR-layout fact, one parent's frame is off and every descendant inherits it, so the fix is upstream
  and fixes many at once) and `geometry/mis-sized:` (the element's own box is wrong). Measured on four
  sites, the top causes are now unambiguous and they are **all MIS-SIZED**:
  `mis-sized: width ~8px (<a>)` 3 sites / **138 hits** / median **12px** ·
  `mis-sized: width ~16px (<a>)` 3 sites / 62 hits / median **20px** ·
  `mis-sized: height ~16px (<div>)` 3 sites / 26 hits / median **24px** ·
  `mis-sized: height ~32px (<div>)` 3 sites / 10 hits / median 39px. The largest `displaced` cluster is
  far behind (`<li>` vertical drift, 2 sites / 11 hits).
  **So t552's `<a>`-WIDTH lead was right in substance and drawn from a signature that could not support
  it** — the instance that falsified it at t553 was a *displacement* wrongly merged into the width bucket,
  and with the confound separated the width cause stands as the largest by both sites and hits. The lead is
  live again, and it is now falsifiable in the right place. ⚠ Next step is to open SEVERAL instances, not
  one: a 12–20px width error on a *text* anchor is a shaping/metrics question, on an *icon* anchor
  (`[130 471 18×46] vs [0 459 30×46]`) it is an intrinsic-sizing question, and the printer makes telling
  them apart cheap now.

- `FORECAST COMPLETE @tick 555 — the top cause is TEXT METRICS, and the sample is homogeneous.` With three
  instances printed per leading cause instead of one, the `<a>`-width cluster resolves cleanly. All three
  instances of `mis-sized: width ~8px (<a>)` (3 sites, 139 hits) are **text** anchors in the same table:
  `[224 25200 266×30] vs [219 25244 255×32]` · `[224 7044 347×30] vs [219 7084 358×32]` ·
  `[224 25469 426×30] vs [219 25513 435×32]`. The `~16px` band is the same shape:
  `288×30 vs 269×32` · `318×30 vs 301×32` · `419×30 vs 397×32`.
  **Two signals, and together they name one subsystem:**
  1. **Width errors go BOTH WAYS** — −11, +11, +9, −19, −17, −22 px. A constant padding or margin error
     cannot do that; **per-glyph advance differences can.** So this is text measurement, not box model.
  2. **Height is a CONSTANT +2px on every one** — Chrome 30, ours 32, on every single instance across both
     bands. A constant line-box height delta beside variable width deltas is the signature of **a different
     font face being used, or the same face's metrics being read differently** (ascent+descent → line box).
  So the next tick is a **font-metrics measurement, not a layout change**: for one known string and the
  page's declared font stack, compare our advance width and our line-box height against Chrome's, and
  determine whether the divergence is **font SELECTION** (we resolved a different face) or **advance
  COMPUTATION** (same face, different measurement). `manuk-text::{zero_advance, x_height, cap_height}` is
  the seam and the t499–502 ch/ex work is the nearest precedent. Do not touch layout until that answer
  exists — the constant +2px says the line box is derived from metrics we may simply be reading wrong.

- `MEASURED @tick 556 — it is font SELECTION, not advance COMPUTATION, and the failure is specific.`
  The t555 forecast asked exactly one question and it now has an answer. Probe pages committed at
  `tests/wpt/probes/`.
  **Generic stacks are FINE.** `sans-serif`, `serif`, `monospace` and a real site's
  `"Fira Sans",Helvetica,Arial,sans-serif` all measure within the 8px tolerance of Chromium on the same
  string — so the advance *computation* is not the problem.
  **Explicitly named sans families are IGNORED.** On the same 44-character string, Chromium gives
  `"DejaVu Sans"` **374px**, `"Noto Sans"` **348px** and a deliberately non-existent `"NoSuchFontXYZ"`
  **299px** — three different faces. We give **330 · 330 · 330**: the two real, installed families and the
  fake one all land on the same fallback face. `fc-list` confirms 23 DejaVu faces installed and
  `fc-match "DejaVu Sans"` resolves it, so this is not a missing font.
  **Resolution is PARTIAL, not absent:** `"DejaVu Serif"` DOES move us (299px vs Chromium's 380px), so some
  names reach the face lookup and sans names do not. And our fallback for an unknown family is a sans face
  where Chromium's is serif — a second, smaller divergence in the same code path.
  **This explains both t555 signals at once**, which is what makes it the right cause: the ±9–22px
  sign-changing anchor widths on real sites (they name specific sans families → we substitute a face with
  different advances) and the constant +2px line-box height (a substituted face has different
  ascent+descent). One defect, both symptoms.
  Next tick is the FIX in `manuk-text`'s face lookup, and its RED proof already exists: these probe pages
  must report `"DejaVu Sans"` ≠ `"Noto Sans"` ≠ `"NoSuchFontXYZ"`.
- `ALSO MEASURED @tick 556 (separate defect, found by the same probe):` an author `* { margin:0 }` does
  **not** beat the UA `body { margin:8px }` — Chromium put body at `[0 0 1200×92]`, we put it at
  `[8 8 1184×91]`. An author universal selector outranks a UA rule by ORIGIN, so this is a cascade-origin
  bug and it has the shape of the recorded `apply_ua_defaults`-vs-Stylo two-cascades trap: a UA default
  applied *outside* the cascade cannot be overridden by anything in it. Every CSS-reset page on the web hits
  this. Filed as its own lead, not folded into the font work.

- `FIXED @tick 558 — the advance now follows the resolved face, and the probe page moves 36.4% -> 90.9%
  SHAPE.` t557 fixed DETECTION (`resolve_family` no longer lowercases the name for fontdb's case-sensitive
  query) and the rendered widths did not move a pixel: five families, five distinct `Named(...)` ids, **one
  `FaceId(0)` and one width (330px)**. The case was thrown away one line later — `intern_family` stored the
  LOWERCASED key in `family_names`, so `face_id` re-queried `fontdb::Family::Name` with lowercase, missed,
  and fell back to `Family::SansSerif` for all of them. **A fix upstream of a lossy step is not a fix**, and
  a resolution-level assertion could not see it, which is why the new test measures the WIDTH.
  Dedup stays case-INSENSITIVE (CSS family matching is, so `ARIAL` and `Arial` must intern to one id);
  storage is now case-PRESERVING. Measured on the committed probe against live Chromium:
  **SHAPE 36.4% → 90.9%**, misplaced spans **5 of 5 → 1 of 11**, and the four real families
  (`"DejaVu Sans"` · `"Noto Sans"` · `"DejaVu Serif"` · `"Liberation Mono"`) now all land within the 8px
  tolerance of Chromium where they previously shared one width.
  **The one residual is known and named:** `"NoSuchFontXYZ"` — Chromium falls back to a *serif* default
  (299px), we fall back to *sans* (330px). A default-family divergence, not a resolution one, and it is its
  own (small) row rather than folded into this fix.

- `sweep @tick 559 (POST-FONT-FIX, same stratified sample; 59 rows, 43 scored):` **the certificate did NOT
  move** — shape ≥0.75 on **3 of 59 (5.1%)** vs t551's 3 of 55 (5.5%) · h-overflow 72.9% · overlap 55.9% ·
  reading-order 42.4% · dead-target 72.9%. Rows: `docs/loop/SWEEP-t559-rows.tsv`.
  **And that headline is misleading on its own, which is the point of keeping the per-site rows.** Across
  the **38 sites scored in BOTH sweeps**, mean SHAPE moved **+1.34 points**, with real individual wins:
  `newegg.com` **5.2% → 28.3%** · `sentry.io` **36.2% → 57.8%** · `spotify.com` **59.3% → 72.5%** ·
  `gutenberg.org` **46.6% → 58.4%** · `fastly.com` 8.6% → 13.1% · `css-tricks.com` 34.8% → 38.2%.
  The certificate's site-bar is **≥0.75 per site**, and almost nothing in the corpus is near it — a
  20-point gain on a site at 5% still leaves it at 28%. **A binary per-site bar cannot see a broad
  distribution shift**, and reading only the bar would have recorded the largest text fix of the session as
  "no effect". Both numbers stay.
  ⚠ **ONE REGRESSION, and it is not dismissed:** `martinfowler.com` **68.2% → 49.2%**. The plausible
  mechanism is the fix working *too* literally — we now resolve a named system family that Chromium does
  NOT use there (because Chromium is using the site's `@font-face` webfont), so a page that previously
  agreed by accident now disagrees on purpose. That is a **webfont-precedence** question (an `@font-face`
  family must beat a same-named system face) and it is its own row, not a reason to revert: three sites
  gained 12–23 points against one losing 19.
  Also: `scala-lang.org` −5.8, `usa.gov` −2.1 — small, same suspected mechanism.

- `DIAGNOSED @tick 560 — the martinfowler.com regression is the WEBFONT-SHADOWING rule, not the resolution.`
  Measured, not assumed. On a **local** page with no `@font-face` anywhere,
  `tests/wpt/probes/font-local-vs-webfont-name.html` scores **100% SHAPE, 0 of 7 misplaced**:
  `"Open Sans"`, `sans-serif` and `"Open Sans",sans-serif` all agree with Chromium exactly. So t557/t558's
  named-family resolution is **correct**, and the regression is elsewhere.
  What martinfowler.com actually declares: `Open Sans, sans-serif` · `Lora, serif` ·
  `Inconsolata, monospace` · `'Marydale'` · `"remixicon"`, with `@font-face` rules for the last two and
  Google-Fonts-delivered faces for the first three. Of those, **only `Open Sans` is installed on this box**
  (13 faces) — so it is the only declaration whose behaviour the fix changed: it used to fall back to
  `sans-serif`, and now it resolves to the **local** Open Sans.
  **The spec rule we are getting wrong (CSS Fonts):** once an `@font-face` rule defines family
  `"Open Sans"` for a document, a locally-installed family of the same name is **SHADOWED** for that
  document. If every `src` in that rule fails to load, the family yields **no usable face** and matching
  continues to the **NEXT entry in the font-family list** (`sans-serif`) — it does *not* fall back to the
  same-named local face. We do fall back to it, so a **failed webfont load is now silently masked by a
  local face with the same name**, and we diverge from Chromium (which loaded the webfont) on a page where
  we previously agreed by both falling back.
  That is a strictly better bug than the one it replaced — it only bites where a webfont fails AND a
  same-named local face exists — but it is a real one, it is spec-anchored, and its RED proof is already
  committed as a sweep row (`martinfowler.com` 68.2% → 49.2%). **Next tick: shadowing.**

- `CORRECTION to the t560 diagnosis @tick 561 — shadowing was the WRONG mechanism for martinfowler.com, and
  the right one is BETTER news.` The shadowing rule is implemented and RED-proven at the unit level (a
  declared-but-unloaded `@font-face` family now falls through to the next stack entry instead of being masked
  by a same-named local face — spec-correct, and it stays). **It does not fire on martinfowler.com, because
  the site has no webfont `<link>` for Open Sans at all** — grepping the fetched HTML for a fonts link returns
  nothing. It simply names `Open Sans, sans-serif` and relies on the local install, which this box has. So
  Chromium and we now use the **same** local face, and t560's "failed webfont masked by a local face" story
  was wrong. Third self-correction in this arc, same cause each time: a mechanism that fits the numbers is not
  the mechanism until the page is read.
  **What the page now actually says, and it is the useful part:**
  `structural 100.0% (384 paths, 0 missing) · SHAPE 46.1% · [diag] absolute PLACEMENT 4.9%, median dx=0
  dy=82 dw=1 dh=2`. **`dw=1` and `dh=2` — the boxes are now the RIGHT SIZE** (that is the font fix landing on
  this site too), and the whole page is displaced **dy≈82px**. So martinfowler did not regress into a sizing
  error; the font fix **removed** a sizing error that had been *compensating* for a vertical displacement,
  and the displacement is now visible on its own. A score can fall because a confound was removed.
  That makes it an instance of the `geometry/displaced` class — an ANCESTOR-layout fact with one upstream
  cause — and 82px near the top of the document is the shape of a mis-measured header/nav block. It is the
  same class the t554 split was built to separate, which is where it should be chased.

- `MEASURED @tick 562 — the line-box derivation is EXACT, so the residual is face-specific and the instrument
  cannot yet name it.` `tests/wpt/probes/line-box-height.html` scores **100% SHAPE, 0 of 12 misplaced, and
  absolute placement 100% (dx=dy=dw=dh=0)** across `Open Sans`, `DejaVu Sans`, the generic stack,
  `line-height:1.5`, `line-height:24px`, and a 400px wrapping paragraph. So `line-height: normal` derivation,
  explicit line-heights, and **wrap points** all agree with Chromium on a controlled page — the general
  metrics formula is right.
  **Which means martinfowler.com's residual is face- or size-specific to that page**, and the instrument
  cannot say which: its instances read `[427 3270 74×16] vs [144 3373 76×18]` — Chromium's anchors are 16px
  tall, ours 18px — and **a rect cannot tell you which FACE or what SIZE each engine used to produce it.**
  Every remaining lead of this shape (2px here, 2px there, compounding into wrap-point and vertical drift) is
  blocked on the same missing datum.
  **So the next instrument brick is: carry the COMPUTED FONT (resolved family + used font-size) alongside the
  rect for text-bearing elements**, on both sides. Chromium's probe can read `getComputedStyle(el).fontFamily`
  and `fontSize`; ours has the resolved `FontFamily` and used size at the same point it records the box. Then
  a 2px height divergence reads as *"Chromium used Face A at 13px, we used Face B at 14px"* instead of as an
  unattributable displacement. That is the same step the `.SIG`, `median_mag`, instance-printing and
  displaced-vs-mis-sized bricks each were: **make the diff carry the datum the next question needs.**

- `UNBLOCKED @tick 563 — the diff now names the FONT on both sides, and it answers t562's question twice.`
  `oracle::Seen` gained `font` (`"<resolved family>/<used px>"`), emitted by Chromium's probe from
  `getComputedStyle` and by ours from the resolved `FontFamily` + used size, and printed on every instance.
  On the site that was blocked:
  **ANSWER 1 — same face, same size, different metrics.**
  `…/a:nth-child(37): [551 3126 51×16] {Open Sans/13} vs [112 3229 57×18] {Open Sans/13}` — **identical
  `{Open Sans/13}` on both sides**, yet Chromium renders 51×16 and we render 57×18. So it is **not** face
  selection (t557/t558) and **not** font-size: it is the **advance and line box of the SAME face at the SAME
  size** — we are ~12% wider and 2px taller. That points at the *variant* (Open Sans ships as a variable font;
  a different named instance/weight has different advances) or at hinting/rounding, and it is a question that
  could not even be ASKED before this brick.
  **ANSWER 2 — and it is a separate, larger finding: a webfont Chromium loads and we do not.**
  `…/p:nth-child(3): [20 2029 293×20] {Lora/13} vs [20 1752 619×20] {serif/13}` — **Chromium resolves `Lora`
  and we fall back to `serif`.** `fc-list` reports **zero** Lora faces installed, so Chromium is getting it
  over the network from a declaration we are not fetching or not parsing. The consequence is not subtle: that
  `<p>` is **293px wide in Chromium and 619px in ours** — a completely different wrap width, which is the
  single biggest per-element divergence on the page and it cascades to everything below.
  So the martinfowler residue is **two** causes, now separated and each independently actionable, where twelve
  ticks of rect-only diffing could only say "displaced". **Rank: the missing `Lora` webfont first** (a wrong
  wrap width dominates a 2px line box), then the same-face metric delta.

- `MEASURED @tick 565 — CSS Grid named-area placement is EXACT, so martinfowler's two-column failure is a
  CASCADE question, not a layout one.` `tests/wpt/probes/grid-template-areas.html` scores **100% SHAPE, 0 of 13
  misplaced, absolute placement 100% (dx=dy=dw=dh=0)** across all four shapes the site uses: auto placement on
  `grid-template-columns:1fr 1fr`, `grid-template-areas:"l r"` with `grid-area` names, explicit `grid-column`
  line placement, and a `1fr 200px` fixed/flexible mix. Grid placement is **not** the defect.
  Which relocates the lead precisely: our section is **619px wide = the full container**, i.e. it is a block in
  a **non-grid parent**, so the container is simply **not receiving `display:grid`**. The rules are in
  `home.css` (which we do fetch), so the question is now *which* — a selector we do not match (`main .top`),
  an `@media` we evaluate differently at the 1200px probe viewport, or a cascade-origin/specificity loss. That
  is a **cascade** investigation, and `getComputedStyle`-style verification on the container is the cheap next
  step; the instrument already carries `display` per element in `Seen`, so the next sweep can answer it without
  new plumbing.
  **Fifth lead in this arc to die on contact with a measurement** (line-box derivation t562, webfont shadowing
  t560/t561, text-measurement t552/t553, the power-of-two artifact t551, grid t565) — and the fifth time that
  cost one cheap probe instead of one expensive wrong-subsystem tick.

