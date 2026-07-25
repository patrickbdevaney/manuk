# Daily-Driver Certification — the definitive way to prove Phase 0, finitely

_Written 2026-07-25 (tick ~581), synthesising a three-leg web-research pass (corpus representativeness +
sampling statistics · how browser projects measure real-web compat · the usage-weighted capability
taxonomy) against this project's own instrument history. This SUPERSEDES the measurement half of
`FIDELITY-SCORING-REDESIGN.md` (its shape/jarring metric is kept, as Layer C-render below) and sets the
exit definition `PHASE0-BOUNDED-REMAINDER.md` points at. The capability half of the ratchet is unchanged._

---

## 0. The problem this fixes (what the recent episode taught)

Re-measured correctly on the full corpus, the engine was **further from daily-driver than the
capability-count suggested**, and the fidelity instrument produced **six flattering-then-false numbers in
one session** (vacuous `0/0=100%`, `.SIG` keying reading healthy pages as 0%, the `mag_band` printer
faking quantisation, RSS eviction "returning nothing", `@supports` answering parse-not-render, a Color-4
gate wrong before the engine was). Three structural causes, and the fix addresses each:

1. **The corpus is a convenience sample, not a representative one.** 265 hand-curated sites, self-described
   as biased toward "sites that are easy to load", and the certificate scores only ~54–72 of them while
   **dropping timeouts** — survivorship bias in the worst direction, because the hard/slow sites (the ones
   most likely to fail) are exactly the ones excluded. Every reading is therefore biased **optimistic**.
2. **Render and function are measured by two systems that never compose.** The oracle measures *render*
   (structural diff / shape / jarring) on the corpus; the capability constellation measures *function*
   (auth, IndexedDB, media, editing) on **hand-built fixtures**. Neither proves the *corpus sites function*.
   "IndexedDB works" (a fixture) ≠ "the corpus sites that use IndexedDB work". Daily-driver is fundamentally
   about function, and it is not currently certified against real sites at all.
3. **Metrics are trusted before they are adversarially falsified.** `falsify.sh` mutation-tests the gate
   wall (it caught G_LOAD/G1/G6 measuring nothing) but was **never applied to the fidelity certificate**.

## 1. The governing principle (from the field, and from Presto's death)

Progressive enhancement layers a site: **content (HTML) → presentation (CSS) → behaviour (JS)**. Only
presentation and enhanced behaviour may degrade; content and core function must survive. Opera's Presto
died in 2013 not from one missing feature but from the **cumulative cost of chasing the compat tail the
leader defines** while sites served it broken paths. So:

> **The cut-line rule:** a feature is safe to omit **iff (a) its absence is detectable** (`'x' in window`,
> `@supports`, `getContext()` → null, a rejected promise) **and (b) the site's own fallback keeps it
> usable.** The one non-negotiable is **honesty of the negative** — a stub that *claims* support then fails
> silently defeats feature-detection and produces a broken page. A dishonest stub is worse than absence.
> (This is the engine-facing form of this repo's `honest-answer-is-not-a-fixed-answer` law.)

## 2. What "certified daily driver" must mean

**Phase 0 is done when, on a statistically-representative sample of the real web, ≥95% of sites both
RENDER acceptably and FUNCTION on the capabilities they actually use — every certificate term
adversarially falsified, every denominator reconciled, and only named exceptions failing.** That is a
defensible "daily driver for ~the whole web", and it is finite because the capability set is bounded by
real usage (§5), not by the infinite tail.

No incumbent publishes exactly this. The field measures three things separately — spec conformance (WPT),
interop-weighted conformance (Interop + Baseline), and real usage/breakage (Chrome use-counters,
webcompat) — and the load-bearing lesson (Interop's own "Web Compatibility" thesis, issue #187) is that
**real breakage is usually not a missing feature but spec-*drift* in a feature already shipped.** So a high
feature/WPT count is necessary but not sufficient; the certificate must prove *shipped behaviour matches on
real sites*. The differentiator no incumbent publishes — and our real Phase-0 leg — is a **stratified,
functional, real-site smoke suite A/B-diffed against Chromium.**

---

## 3. The corpus (fix cause #1): representative, two-stratum, fixed-denominator

**Frame:** the **CrUX** origin universe (page-view-weighted, monthly, origin-level, and exactly what HTTP
Archive crawls — so per-site tech-stack + captured HTML/CSS attach for free), ordered by **Tranco** for a
stable, manipulation-resistant, citable ranking (0.6%/day churn vs Alexa's 50%). **Pin a Tranco list-ID +
a CrUX YYYYMM per build** for reproducibility.

**Two strata, reported as separate claims** — because web page-views are extraordinarily head-concentrated
(CrUX: top **1k ≈ 50%**, **10k ≈ 70%**, **100k ≈ 87%**, **1M ≈ 95%** of all loads) *but breakage lives in
the tail* (the head is well-engineered and already tested against mainstream engines; rendering diversity,
legacy/quirks markup and broken pages concentrate in the low-traffic millions). Traffic-weighting alone
proves "covers what people visit" and **starves** "handles the web's diversity".

- **HEAD** — traffic-weighted draw from the top tiers (weight toward top-1k/10k). Claim: *"≥X% of real
  page-views render+function correctly."*
- **TAIL** — **uniform-random** draw from the deep tail (100k–1M, and 1M–10M). Uniform, not
  popularity-weighted, or the tail's diversity is invisible. Claim: *"X% ± e of the long tail works."*

Stratify each further by **content-category × tech-stack** (tech-stack from HTTP Archive Wappalyzer /
Blink-features; category from the on-page capability probe). **Neyman allocation** (`n_h ∝ N_h·S_h`) puts
sample where a stratum is large *and* internally variable (the tail), shrinking the N for a given margin
and yielding per-stratum pass rates for free.

**Size — from the statistics of a proportion, not "every URL":** Cochran `n₀ = z²·p(1−p)/e²`
(95% CI, worst-case p=0.5): **±5% → 384 · ±3% → 1067 · ±2% → 2401.** FPC negligible at web scale. Because
the web mostly works (p≫0.5), once the pass-rate climbs the required N shrinks (at p̂≈0.90, ±5% needs ~139)
— so **run a ~100-site pilot, plug in p̂, then size.** Target: grow from 265 → **~400 (±5%) now, ~1000
(±3%) as the cert stabilises**, split head/tail.

**Fixed denominator — the survivorship-bias fix, and it is load-bearing:** the denominator is the *sampled
set*, frozen at build time. A site that times out, crashes, or is a bot-wall/login/error page is a
**counted outcome** (`FAIL` with a reason, or `EXCLUDED` with a reason that is auditable and capped) — it
is **never silently dropped**, because dropping the hard sites is what made every past reading optimistic.
`EXCLUDED` (genuine bot-wall/paywall, not our failure) is reported separately and its rate is itself a
watched number; if it rises, the corpus or the fetch path is degrading.

Built by `scripts/build-corpus.sh` (this tick); the artefact is `docs/bench/corpus-v2.tsv` with provenance
(frame, list-id, date, seed, strata sizes) stamped in the header.

## 4. The unified per-site certificate (fix cause #2): RENDER × FUNCTION

One instrument, run per corpus site, composing both axes. A site **passes** iff it clears **both**.

**Layer C-render — does it render? (keep FIDELITY-SCORING-REDESIGN's metric, on the representative corpus):**
- **SHAPE** ≥ 0.75 parent-relative on ≥95% of nodes (per-category floor ≥0.70).
- **JARRING invariants** clean on ≥95%: no overlap, no horizontal-overflow, reading-order preserved, no
  dead click-targets. (Layer 2 of the redesign — the actual "a user does not notice they left Chromium" bar.)

**Layer C-function — does it work? (the new leg, the daily-driver core):**
- The **on-page capability probe** records which capabilities *this site actually touches* (every global
  read, every method call) — turning "MDN lists 4,000 APIs" into "these are what this site calls".
- The certificate then requires those touched capabilities to **exercise green FOR THIS SITE**, driven via
  **WebDriver BiDi** (we already have BiDi) and, where a reference is needed, A/B-diffed against Chromium:
  auth/SSO round-trip completes, storage (localStorage/IndexedDB/Cache/SW) persists across reload, `<video>`
  plays+seeks, SPA client-route + History/Navigation + back/forward, forms submit + validate,
  contenteditable input. A capability the site touches that **throws or no-ops** fails the site (this is the
  IndexedDB-class killer: absence that throws crashes unrelated page scripts).

**The composition:** daily-driver-pass(site) = renders(site) ∧ functions(site). Report per-stratum and
per-category, never a single vanity number.

## 5. The finite capability taxonomy (keeps it tractable): usage-weighted, with the cut line

Adopt the **`web-features` / Baseline** catalog as the finite enumerable capability set (already split
CSS-render / API-function, with the 30-month "widely available" inclusion filter). Weight every capability
by **Chrome use-counter % of page loads** so a failure in a 40%-of-loads feature dominates a 0.01% one.
The required set is the features **≥0.1% of page loads** (~450 CSS properties + several hundred web
features); everything below Chrome's ~0.001% removal threshold is the **death-tail — feature-detect,
degrade, name, cut.**

**Required — RENDER (must be correct):** HTML parse → accessible DOM (tolerate obsolete tags); CSS box
model + cascade (display/position/margin/color ≥90%); **flexbox (74% of pages — the single biggest
make-or-break)**; transforms/transitions/animations; media queries; **WOFF2** fonts + text shaping;
JPEG/PNG/GIF/**WebP**/**SVG** decode; then Grid (12%, modern-heavy), `var()` (43%), `calc()`.

**Required — FUNCTION (absence = broken, not degraded):** a real JS engine + ES2017+ built-ins
(Promise/async, Array/Object/String/Proxy/WeakMap/Symbol/modules/classes); the full DOM/BOM/CSSOM surface
sites call (a missing API that *throws* takes down the page); events incl. **Pointer/Mouse** + correct
**passive-listener** semantics; the **observer trio** (Intersection/Resize/Mutation); `fetch`+XHR;
forms + constraint-validation + submission; navigation/URL + History; cookies + localStorage; then
**IndexedDB** (the sharpest killer — Firebase/Firestore throw on init without it); basic `<video>`/`<audio>`
+ **2D canvas**; SPA hydration; auth (OAuth redirect+postMessage+popup, cookies, WebCrypto); Service Worker
fetch-interception; contenteditable (editor sites).

**Cut line — omit or honestly stub** (all uniformly feature-detected, absence non-fatal): WebGPU/WebXR/
WebCodecs/WebTransport, WebRTC/getUserMedia, Web Bluetooth/USB/Serial/HID, Background/Periodic Sync + Push,
File System Access (→`<input type=file>`), Payment Request (→form), Web Share (→copy-link), **WebGL**
(`getContext` → null → 2D/static fallback). Floor: 2D canvas + basic video/audio decode are *required*;
everything GPU-accelerated or real-time is above the line.

**UA reality (the Presto trap, restated):** even a complete engine is served broken paths if its UA is
unrecognised. A new engine must present a **Chrome-compatible UA**; feature-detection is the answer sites
*should* use and many don't. (In-scope as a compat necessity, not evasion — completeness ≠ evasion.)

## 6. Non-delusion guards (fix cause #3): the instrument may not flatter itself

Baked into the certificate, each a gate that fails the sweep if violated:

1. **Falsify the certificate.** Extend `falsify.sh` to the cert: for each term, deliberately break the
   engine (disable flexbox; make IndexedDB throw; nudge a box 30px) and confirm that term goes **red**. A
   term unmoved by a real break is measuring nothing — the exact defect that produced G_LOAD/G1/G6 and this
   session's six flattering numbers. **No cert term is trusted until it is proven to go red.**
2. **Reconciliation accounting, as a gate.** `sampled == attempted == scored + FAIL + EXCLUDED`; and
   `parsed == probed == scored`. Any imbalance is an instrument bug, not a result. (8 of 30 historical
   process defects were caught by a number that did not add up.)
3. **Explicit denominators; thin sample = UNSCORED-against-bar.** Every ratio carries its N; `0/0` is never
   a pass (`CERT_MIN_SHAPE_SAMPLE`, generalised to every term). A page we render nothing of scores 0, not 100.
4. **Two-population cross-check.** Measure each headline capability two independent ways — the fixture gate
   AND the corpus-site exercise. Disagreement means the instrument (or the engine) is lying; investigate
   before believing either.
5. **No silent caps.** If the sweep bounds coverage (chunk timeouts, sampling), it `log()`s exactly what was
   dropped and counts it against the denominator. A timeout is a FAIL, never an omission.

## 7. The ratchet, extended to real sites

- **Per-tick (in the wall):** a fast **stratified sub-slice** of the corpus (a few dozen sites, fixed seed)
  runs render+function — so a capability regression *on real sites* fails the tick, not just a fixture.
  This is the anti-regression guard the user asked for, on the representative corpus.
- **Off-tick (weekly / on-demand):** the **full ~400→1000-site certificate sweep** — chunked, resumable,
  `nice`d, fixed-denominator. Produces the Phase-0 exit number + per-stratum/-category breakdown.
- **Monotonic floors:** every certificate term is a ratchet floor; a tick that drops any term is reverted.
  A term may only be re-baselined by the observer, never retuned by the agent to land its own tick.

## 8. The finite closing plan (how Phase 0 conclusively ends)

1. **Build the representative corpus** (§3) — `scripts/build-corpus.sh` → `corpus-v2.tsv`. _(this tick)_
2. **Migrate the instrument** to the unified render×function certificate (§4) on `corpus-v2`, with the
   non-delusion guards (§6). _(agent, steered by the board; the fidelity instrument is agent-territory)_
3. **Wire BiDi functional assertions** (§4 Layer C-function) for the F1/F2 capabilities, A/B vs Chromium.
   _(agent)_
4. **Run the pilot** (~100 sites) → get p̂ → finalise corpus size. Run the first full unified sweep → the
   first honest exit number, per-stratum. _(agent sweep; observer banks the actuals)_
5. **Close gaps ranked by (sites-affected × use-counter weight)** until every term ≥ its bar on the
   representative corpus, named exceptions only. This is finite: bounded by the ≥0.1%-usage taxonomy (§5),
   not the tail.
6. **Certify:** ≥95% render×function pass on the representative corpus, every term falsify-proven,
   denominators reconciled, only named exceptions. Journal it and **stop for the owner's Phase-1 go**
   (per the standing trigger in OBSERVER-PROMPT.md — v1.0.0 release then Phase 1).

## 9. Division of labour

- **Observer builds:** this doc; `scripts/build-corpus.sh` + `corpus-v2.tsv`; `falsify.sh` extension to the
  cert; the reconciliation gate wiring in `verify.sh`; the board steer + exit-criteria update. (Harness,
  corpus, methodology — all observer-owned.)
- **Agent builds (steered via the board):** the unified render×function certificate in the fidelity
  instrument (`manuk-wpt`), the BiDi functional assertions, the per-tick corpus sub-slice gate. (The
  instrument and engine are agent-territory; the observer supplies the spec + corpus + steering, never
  edits `manuk-wpt` under the running loop.)

## 10. Sources (for the numbers above)

Corpus/stats: Tranco (tranco-list.eu, NDSS'19) · CrUX (developer.chrome.com/docs/crux, github.com/zakird/
crux-top-lists) · HTTP Archive / Web Almanac (almanac.httparchive.org) · Cochran *Sampling Techniques* ch.4.
Methodology: WPT/wpt.fyi · Interop (github.com/web-platform-tests/interop, issue #187) · Baseline
(web.dev/baseline, github.com/web-platform-dx/web-features) · Chrome use-counters (chromestatus.com/metrics,
use_counter_wiki) · Mozilla webcompat (webcompat.com, Compatibility/WebCompat_Priority_Flags) · WebDriver
BiDi. Taxonomy: Web Almanac 2022/2024 (css/javascript/markup/fonts/media/capabilities) · chromestatus data ·
MDN Progressive_Enhancement · Presto post-mortem (Wikipedia, InfoQ 2013) · Firebase IndexedDB throw
(github.com/firebase/firebase-js-sdk#3573).
