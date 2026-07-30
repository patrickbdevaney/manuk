# PHASE-0 MEASUREMENT SYSTEM — the calibrated, minimum-steps path to a daily driver

**Owner:** observer synthesis, 2026-07-30. Fuses four independent research probes (2 internal codebase
audits + 2 external-web SOTA studies) into ONE measurement-and-ranking system. This is the "set and forget
until Phase-0" calibration the owner asked for. It **supersedes the sequencing** in
`PHASE0-RENDER-BURNDOWN.md` §3 (shape-primitive worklist) and the milestone framing in
`phase0-milestone-sequence` (memory) — those stand as the *visual* leg; this adds the leg that was the
binding constraint all along. Authority for the cert itself remains `DAILY-DRIVER-CERTIFICATION.md`.

---

## 0. Why trust this (the convergence)

Four probes ran independently — two auditing our own instrument/engine, two studying how Chrome, Ladybird,
Flow, Servo, and Mozilla actually measure "web compatibility." They **converged on the same architecture we
are already building** (tail-stratified CrUX corpus + shape/box-coverage visual + JS-exception functional),
which is strong validation that the *direction* is right. They also **converged on the same correction**: we
have the **sequence backwards**. That agreement — internal + external, four angles — is why this is
actionable, not one more re-scoping.

External SOTA's one-line thesis, which our own two audits independently reproduced from our data:
> Measure **exceptions-and-primary-path (works) + shape/box-coverage (looks right)** on a **rank-stratified,
> tail-weighted, byte-frozen CrUX corpus**; require only **Baseline ∩ ≥0.1%-of-page-loads** while refusing
> everything below ~0.001% and the leader's quirk tail; treat **WPT/Interop as the build instrument, never
> the certificate.**

---

## 1. THE UNIFYING INSIGHT — function is the binding sub-constraint of render, and it comes first

Four separate findings are **one problem**:

| Probe | Finding |
|---|---|
| burndown-optimality | M1 has a **hard ~63% ceiling**: 48 of 130 in-scope t767 sites (render-failed 12, shell-only 12, thin-overlap 15, timeout-150s 6, css-starved 3) **do not render a real page at all**, so no shape/jarring fix can lift M1 past 82/130 = 63.1%. |
| function-design | **Throw-class killers CORRUPT the render number** — welt.de scored 0% coverage because one `ReferenceError: HTMLMetaElement is not defined` aborted boot. A page that throws during load never lays out. |
| measurement-map | **Function is entirely uncertified per real site** (M2 unbuilt); the render metric is a single static post-load frame that sees no interaction. |
| external-SOTA | The **#1 predictor of daily-driver readiness is app-halting-exception rate** — the instrument Flow and Ladybird actually ship. "A site that throws is unusable regardless of how it looks." |

**They are the same thing.** A site that lands in the 48-site unscored ceiling is, in the majority of cases,
a site whose **JavaScript threw (or hung) during boot**. Therefore:

> **Killing boot-halting exceptions simultaneously (a) unlocks the render ceiling, (b) builds the M2 function
> certificate for free, and (c) attacks the single highest-predictive daily-driver dimension.**

Function is **not** "M2, built after M1." It is the **binding sub-constraint of M1**, and per every external
source it comes **first**. Our metric already counts these sites as M1 fails (honest) — but the *ranking, the
slope, and the ETA* optimize shape on the 82-site island under a 63% cap they never track. That is why M1 is
flat at 2.3% while shape-only rose: **we have been grinding the wrong leg.**

Caveat (accuracy): of the 48, ~30 (render-failed + shell-only + timeout) are function/boot problems; ~15
thin-overlap are a **measurement** problem (rendered, but too few boxes overlap Chrome's to score — an
instrument/coverage issue, not necessarily an engine bug); ~3 css-starved. So the majority — not all — is
the function leg.

---

## 2. THE CALIBRATED METRIC (what "done" means, per origin)

A single per-origin verdict with an **ordered gate**:

1. **FUNCTION gate** — page reaches a stable DOM with **no app-halting uncaught exception**, AND its
   **primary user path executes** (login form present+submittable / feed renders N items / nav works). Probe
   **inside the page**, not from the finished output — a bug in the *order* the page was assembled
   (load-event / CSS timing) is invisible to any final-vs-final diff. A page failing this is **unscorable for
   shape** — this is the 63% ceiling made a first-class gate.
2. **VISUAL gate** (only on function-passing pages) — **shape ≥ 0.75 AND jarring-clean AND coverage ≥ floor.**
   Coverage is a NEW exit term: today a page can drop 30% of the DOM and still pass M1 by placing the
   survivors well (map-probe blind spot #3). A daily driver may not silently lose a third of the page.

**Phase-0 exit = ≥95% of in-scope origins pass BOTH gates**, on the tail-stratified, byte-frozen CrUX corpus.
M1 (visual) and M2 (function) stop being sequential milestones and become **two terms of one number**, with
function gating visual.

---

## 3. THE THREE TRACKED LEGS (the burndown, re-ranked)

1. **SCORABILITY / FUNCTION — the ceiling. Attack FIRST.** Track `scored / in-scope` as a first-class axis;
   it is the hard cap on M1 (63% today). Own worklist = the ~48 unscored sites, ranked by throw-class killer
   frequency (function-design probe: interface-objects #1 = 63/183 absent; IndexedDB+indexes #2 = the
   Firebase killer; observer-trio #3). Each site cleared raises scorability **and** the M2 number.
2. **VISUAL near-bar conversion — shape-nudge the jarring-clean cohort.** A +0.06 shape nudge on the ~6
   jarring-clean near-bar sites = **6 M1 crossings**; clearing any single jarring dimension everywhere = **1
   crossing**. Shape-nudge is **~6× more M1-productive** at the current margin. **Rank fixes by marginal M1
   crossings** using the per-site distance data already in `SWEEP-*.tsv`, NOT by CLUSTERS.md tag-frequency
   (which ranks breadth → "band lifted +2.0, 0 sites crossed"). Jarring is a **first-class gate** (keep it)
   but a **low-priority fix target**: gross misplacement tanks shape AND spawns jarring together (every
   jblk≥3 site has shape<0.28), so the shared width→dy / flex mechanism clears **both** — only ~4 sites
   (777juegos H=11, sports.yahoo O=13, payb.jp R=14) have an *independent* jarring tail needing per-site work.
3. **CERTIFICATION — full 400-site recert at the milestone** (`corpus-v2.tsv`), not the 200-site trend driver.

## 4. LEADING INDICATORS (replace pass-count as the health signal)

Pass-count lags a threshold on a bimodal distribution (§7 of the burndown doc: a batch lifted the band +2.0pt
while crossing **zero** sites). Track instead, every sweep:
- **scorability rate** (scored/in-scope) — the ceiling;
- **convertible-cohort count** — # sites with shape-short ≤0.10 AND jarring-blockers ≤1 (moves *before*
  crossings; currently ~9–11);
- **continuous distance-to-M1 aggregate** — Σ per-site `max(0, 0.75−shape) + jarring-excess` (moves every
  sweep even at 0 crossings).

## 5. THE DEATH-TAIL CUT LINE (so the work is finite)

- **Require:** Baseline "Widely Available" ∩ Chrome UseCounter **≥0.1% of page loads** — the finite,
  defensible "must-work" set. Inside it, prioritize by *what happens on absence*: features that **throw/hang**
  (break the whole page) rank above features that merely degrade. Weight the Web-Almanac reality: a working
  **jQuery + jQuery-Migrate + WordPress core** beats any amount of exotic CSS (75% of the web).
- **Exclude (never chase):** anything **<0.001%** usage; no-real-user features that inflate WPT (WebVTT,
  WebNFC, gamepad); and **the Chrome quirk/bug tail** — undocumented leader behavior no spec mandates (the
  Presto death-tail trap: a *more* standards-compliant engine died reverse-engineering WebKit's quirks).
- **WPT/Interop is the build instrument, never the certificate.** Servo's 79% coexists with unusability;
  Apple's 90% bar is a *minimum*, not readiness. Keep the ratchet as a regression net; never headline it.

## 6. SITE-GATING — the minority-engine tax (the one policy divergence)

Mozilla's data: the **#1 real-world breakage class is faulty UA-sniffing + `-webkit-` prefixed CSS with no
standard fallback** — almost none of the top breakage classes is a missing feature.
- **`-webkit-` prefix aliasing is COMPAT, not stealth — in-scope, pursue it.** Every non-WebKit engine
  (Moz/Opera/MS, 2012) had to alias `-webkit-` prefixes to render the real web.
- **UA-spoofing past bot-walls is the contested part.** Our no-stealth policy (memory: `scope-botdetection`)
  accepts the ~35% excluded corpus as a **known, capped cost** — that scope decision is already made. Flagged
  here only because it is the single place our policy and SOTA (Ladybird spoofs Chrome's UA) diverge; the
  excluded rate is watched and capped by `fidelity-progress.sh`, not hidden.

## 7. THROUGHPUT (feed the loop faster)

- Parallel sweep (already steered: ~2h→~20min, xargs -P8).
- **Fast ~30-site convertible-cohort micro-sweep between full sweeps** — per-fix attribution under the 3.7pt
  run-to-run variance (a 5-fix batch loses individual fixes in the noise; §7 had to fall back to a
  common-scored-set control). Reserve the 200-site sweep for periodic recalibration.
- Batch 6–8 fixes per full sweep.

## 8. THE ANTI-GOODHART RULES (so "passes the test" = "real sites work")

1. **The corpus is the oracle, not the test suite.** The certificate is function+shape on the tail corpus;
   WPT% is a build tool — never the headline.
2. **Measure interop-drift, not conformance** (Interop #187): the risk is not the feature you skipped, it's
   the feature you shipped that behaves *subtly wrong*. Gate Chrome-differential behavior on shipped features.
3. **Probe inside the page** for anything schedule/timing-dependent — a reader of the finished answer cannot
   see a bug in the order it was assembled.
4. **Control before blame** — a live site's shape varies run-to-run; re-measure the same tree's spread before
   attributing any delta (don't revert good work chasing noise).
5. **Site-gating is a first-class capability** — UA/prefix handling, or the corpus serves you degraded code
   and mismeasures the engine.
6. **A capability is done only when the corpus confirms it end-to-end** — `typeof`/feature-detect answers
   presence, never behavior; a correct-but-wrong value (empty array, plausible-but-false boolean) passes
   every feature-detect and still breaks the site.

## 9. RANKED — how much each dimension predicts "usable daily" (the priority order)

1. **Function: app-halting-exception rate + primary-path completion** on the tail corpus — highest predictor.
2. **Visual: shape + box-coverage** vs Chrome — "looks right" for the pages that ran.
3. **Corpus tail-stratification** — a multiplier on 1–2; without it they measure the easy head and lie.
4. **Site-gating handling (UA/prefix)** — gates whether 1–2 even see the real page.
5. **Finite required set = Baseline ∩ ≥0.1%** — bounds the work, keeps us off the death-tail.
6. **WPT/Interop %** — *last*: a regression net and shared vocabulary, the weakest readiness predictor.

---

## 10. WHAT THIS CHANGES RIGHT NOW (the concrete calibration)

- **Re-steer (supersedes the t13d571f4 jarring-first steer):** priority is **(1) scorability / throw-class
  killers → (2) shape-nudge the jarring-clean near-bar cohort → (3) independent jarring tail (~4 sites)**.
  Jarring stays a first-class *gate* (the t04b3ee50 gating change is correct); it drops as a *fix target*
  because the shared dy/flex mechanism clears it alongside shape.
- **Instrument:** `fidelity-progress.sh` prints the **SCORABILITY CEILING** explicitly (scored/in-scope +
  unscored-reason breakdown) so the loop can no longer grind shape under an untracked cap.
- **Two owner decisions surfaced** (do not need answering to proceed, but confirm the framing):
  1. **Function-first**: reframe M1/M2 as two terms of one function-gated number (function first), vs the
     current strictly-ordered "M1 render then M2 function." The evidence (all four probes) favors function
     first. *This touches the owner-locked milestone sequence — surfaced, not unilaterally flipped.*
  2. **No-stealth cost**: confirm we accept the ~35% excluded corpus as a capped cost while pursuing
     `-webkit-` prefix aliasing (compat, in-scope). Scope decision already on record; re-confirming because
     SOTA weighs it heavily for minority engines.

**One-line answer to "fastest path to M1 and M2":** they are the same leg. **Kill boot-halting exceptions on
the ~48 unscored sites first** (raises the render ceiling AND builds the function cert), **then shape-nudge
the jarring-clean near-bar cohort** (6× marginal M1 productivity), ranking every fix by *marginal M1
crossings* from the per-site sweep data — on a tail-stratified, byte-frozen corpus, with WPT kept as the
build net and the death-tail explicitly refused.
