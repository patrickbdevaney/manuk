# Lever Map — daily-driver-weighted capability targeting

**What this is.** The targeting intelligence behind `scripts/lever-board.sh`: which capability each tick should
attack to unlock the most *real-web daily-driver* function and WPT subtests, fastest. Built from a deep-research
sweep (tick 119). Usage %/WPT counts are MEASURED [M] (Web Almanac 2024/25, Project Wallace 2026, chromestatus,
a Chrome 152 wpt.fyi run 2026-07-16); rankings/efforts are JUDGMENT [J]. **WPT remains ground truth; this is a
second oracle, never the authority.**

## ⚑ CURRENT PHASE DIRECTIVE (tick 138+) — enforced in lever-board.sh
**Build daily-driver CAPABILITY, not WPT-flip count.** html/dom is reasonably done (~93%); real sites still render
mislaid-out (CSS layout weak: flexbox 6%, grid 5%, sizing 12%) and video is dead (MSE absent). So this phase
targets, in order: **(1) CSS LAYOUT** — flex/grid intrinsic sizing (min/max-content; Taffy #204) first, then
sizing/position/values/overflow; **(2) MSE/MEDIA** — build the `MediaSource`/`SourceBuffer` JS surface and bind
GStreamer/FFmpeg (don't hand-write codecs). These are low WPT-flip, high daily-driver value — that's the point.
**The v1 path:** good-enough render+media foundation → the **agentic driving surface** (the differentiator; add
**agent fallbacks** for actions where DOM/CSS WPT coverage is still thin) → a **reasonable security sweep**. Only
after CSS layout + MSE are "good enough" do we return to general WPT-flip work.

## The core correction to the oracle
Ranking WPT areas by **raw failing-subtest count chases MASS, not daily-driver UNLOCK.** The two biggest WPT
areas (`html/dom` ~68k, `dom` ~66k) are already Manuk's strongest surfaces (~97.8% / ~99.8% pass) — grinding
their tail is low value. Meanwhile the APIs whose absence **blank-screens whole SPAs** are LOW-MASS and score
near-zero on raw count: **History, IntersectionObserver, Proxy/Reflect, fetch-streaming.** Rank by
daily-driver-weighted expected value + a gating multiplier for these boot-critical APIs.

## Ranked levers  (score = weight × WPT-mass(k) ÷ effort)
| Rank | Cluster | Wt | WPT(k) | Effort | Score | Type |
|---|---|--:|--:|--:|--:|---|
| 1 | Flex/Grid intrinsic sizing (min/max-content; Taffy #204) | 10 | 28.5 | 2.5 | 114 | ⚡ step-function |
| 2 | Fetch / Streams (ReadableStream body; gates Next RSC) | 9 | 21.2 | 2 | 95 | ⚡ step-function |
| 3 | Web fonts (@font-face, WOFF2, metrics) | 8 | 8.8 | 2 | 35 | fidelity |
| 4 | Shadow DOM + Custom Elements (+template) | 5 | 20.4 | 3 | 34 | ⚡ YouTube-gating |
| 5 | Pointer/touch + passive | 6 | 4.2 | 2 | 13 | interaction |
| 6 | History / SPA routing | 7 | 1.06 | 1 | 7.4 | ⚡ boot-critical |
| 7 | CSS custom properties & nesting | 8 | 0.87 | 1 | 7.0 | breadth |
| 8 | IntersectionObserver / ResizeObserver | 7 | 0.54 | 1 | 3.8 | ⚡ boot-critical |
| 9 | Images / lazy-loading | 6 | 0.8 | 1.5 | 3.2 | breadth |
| 10 | Compositor scrolling | 5 | 1.5 | 3.5 | 2.1 | DEFER (UX; low WPT) |
| 11 | MSE / media | 4 | 1.47 | 3.5 | 1.7 | DEFER (bind GStreamer) |
| 12 | CSS containment / content-visibility | 3 | 0.98 | 2 | 1.5 | DEFER |

**Formula blind spot (the crux):** the score UNDERVALUES small-WPT high-gating APIs. History (#6),
IntersectionObserver (#8) and Proxy/Reflect (not even in the table — near-zero marginal WPT mass) are
BOOT-CRITICAL: their absence blank-screens entire SPAs. **Ship them early despite low WPT mass.**

## Per-site dependency map (the 7-site daily-driver corpus)
- **Wikipedia** (MediaWiki, server HTML) — flex/block layout, CSS, web fonts. THE FLOOR, no JS required. Reachable now.
- **x.com** (React) — **History, Fetch, IntersectionObserver** (feed won't paginate without IO), flex/grid, CSS vars, MSE.
- **Instagram** (React) — **IO (pagination + lazy-img + video), Fetch, History**, MSE (Reels).
- **HuggingFace** (SvelteKit SSR+islands) — server HTML+CSS, flex/grid, Fetch, History. Degrades gracefully.
- **YouTube** (Polymer web-components + MSE/DASH) — **Shadow DOM + Custom Elements, MSE, CSS vars, Fetch, History**. HARDEST (4 subsystems at once).
- **Generic React SPA** — DOM events + bubbling fidelity, History, Fetch.
- **Generic Vue SPA** — **Proxy/Reflect (silent total gate)**, History, Fetch.

Clusters 1–8 unlock 6 of 7 sites; only YouTube needs the web-components + MSE tier.

## Oracle scoring (implemented in lever-board.sh)
`EV(C) = [ Σ failing(a)·attributable(C,a)·flip_prob(C,a) ] · daily_weight(C) · gating_multiplier(C) ÷ effort(C)`
- `daily_weight`: measured usage % (fonts .87, custom-props .50, lazy .33, IO .09, custom-elements .079, video .067…)
- `gating_multiplier`: **3–5×** for boot-critical low-mass APIs (History, IO, Proxy/Reflect, fetch-streaming)
- `effort`: S=1 / M=2 / L=3

## Pre-build FORECAST signals (compute from the existing WPT run; no build required)
1. **Error-signature clustering [STRONGEST].** Group failing subtests by assertion/exception string; the biggest
   same-signature cluster = one fix flips them all. Would have surfaced tick-113 (+10,249) a priori.
2. Interface/algorithm attribution — map failing subtests to the WebIDL interface they exercise.
3. Reachability delta — count tests that ERROR/timeout at load (API absent); stubbing makes them loadable (step-function).
4. DAG-readiness — zero out candidates whose prereqs are unmet; boost those that unblock downstream areas.
5. Reftest mass — grid 1600 / flexbox 1028 / transforms 821 reftests are invisible in the subtest column but are visual-correctness units.
6. Historical flip-density prior per area (from tick history).

Each tick should carry a falsifiable forecast (predicted ΔWPT + ΔDaily); after building, feed the residual back into the priors — a self-calibrating oracle.

## Corpus recommendation (for oracle-crawl.sh)
~265 is a good size for a *behavioral* (not statistical) corpus IF stratified by framework archetype. Guarantee
≥5 sites each of {WordPress, React SPA, Next SSR/RSC, Vue/Nuxt, Angular, Shopify, builder-markup, static/Astro} —
framework drives engine behavior more than topic. Don't over-fit to bespoke React: WordPress 36–41% + jQuery
67–75% is the "boring" reality. Strata (~265): social 30, news 30, ecommerce 30, WP-longtail 25, SaaS 20, video
18, dev/docs 18, finance 15, wiki 10, maps 10, edu 10, search 8, gov 8, adult 8, AI-tools 6, email 5, misc 4.

## Concrete next-3 ticks
1. **IntersectionObserver** (S, 402 WPT, unblocks x.com + Instagram feeds) — highest EV once gating applied; hidden by raw count.
2. **CSS intrinsic sizing / min-max-content** (M, feeds flex+grid ~28k reftest-heavy) — largest raw lever; Taffy #204 gap.
3. **Fetch streaming / ReadableStream body** (M, ~19k WPT, gates Next.js RSC).

Defer MSE + compositor scrolling to dedicated arcs; when MSE comes, bind GStreamer/FFmpeg (as Servo does) rather than build demux/codecs.
