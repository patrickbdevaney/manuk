# THE WPT HORIZON — the parity-scope roadmap, counted from the tree

> **A third anchor of parity scope**, alongside (1) the **differential oracle** (265 real sites vs
> Chromium) and (2) the **doc-web → app-web → platform-web capability roadmap** (`PARITY-LEDGER.md`).
> Where the oracle measures *"what real pages do"* and needs Chromium to say what's right, WPT measures
> *"what the spec says"* and **carries its own verdict** — so this map is the spec-shaped horizon.

## TWO HORIZONS, and they are not the same target

**The NEAR horizon — the daily-driver speedrun.** The least-ticks path to a browser that handles *most of
the internet* versatilely: **doc web (HTML/CSS/JS) → app web (SPA, DOM nodes, shadow DOM, the frameworks'
vDOM commit path) → platform web (lazy-load, iframes, media)**. This is chosen for **broadest impact per
tick**, and it is deliberately *not* "pass every test" — it is "make the classes of the web a person
actually uses work, and decline the rest gracefully." The oracle's cluster ranking and the capability
ledger drive this; WPT informs it.

**The ULTIMATE horizon — full parity.** The complete surface: **WPT most of all (its ~50,000 tests are the
widest spec-shaped measure that exists)**, plus the full `PARITY-LEDGER.md`, the oracle at breadth, and the
accumulated wiki + journal. This is the asymptote — *how close to Chromium's whole capability envelope we
get* — and it is measured, not chased to 100%.

**The relationship:** the near horizon is what we *speedrun*; the ultimate horizon is what we *track*. A
tick almost always serves the near horizon (a class of the web that now works); WPT tells us the **shape and
size** of the ultimate one so the near-horizon choices are made against a real map rather than a guess.

**⚠ COUNTS ARE LIVE, NOT FABRICATED.** Every number here is counted from the local WPT checkout by
`scripts/wpt-horizon.sh` (or wpt.fyi's API when online). **Do not hand-copy a count from anywhere** —
WPT's totals shift as tests are added, and a stale number is worse than no number (Part 13's rule for the
residual-bug estimate applies here identically). **Regenerate on the EPOCH-audit cadence.**

**Structural fact:** every top-level WPT directory is one spec — **except `css/`**, which is one directory
holding *dozens* of separate CSS Working Group sub-specs (flexbox, grid, selectors, position, fonts,
colour, animations…). That is why `css/` is disproportionately large and why its sub-specs are tracked
**individually**, never as one aggregate.

---

## Measured (2026-07-13, local checkout) — the anchor points we can run TODAY

| Category | Spec dir | testharness files | Our subtests | Pass % | Bar 0 |
|---|---|---:|---:|---:|:--:|
| **DOM core** | `dom/` | 619 | 1,738 / 6,499 | **26.7%** | ✅ 0 |
| **HTML DOM** | `html/dom/` | 237 | 12,497 / 59,560 | **21.0%** | ✅ 0 |
| **Selectors** | `css/selectors/` | 531 | 514 / 1,840 | **27.9%** | ✅ 0 |
| **DOM Parsing** (`innerHTML`/serialize) | `domparsing/` | 64 | 126 / 1,273 | **9.9%** | ✅ 0 |

*(`html/dom/` counts 237 testharness files but **59,560 subtests** — its reflection tests assert every IDL
attribute of every element, which is why it dwarfs everything else in subtest count. It is the single
largest measurable surface we have.)*

---

## The horizon — categories mapped onto the platform-web map

Grouped to match the **full platform-web map** already in STATUS.md, so this is *one* coherent horizon,
not a competing taxonomy. Counts marked **[checked out]** are in the local tree now; the rest need
`./scripts/wpt-setup.sh` to add the dir before they can be measured.

### Document-web core — Bar 1, highest priority (the usage-frequency ledger)

| Spec dir | testharness .html | status |
|---|---:|---|
| `css/` (all sub-specs) | **4,190** | partial — see sub-specs below |
| `html/` | 373 | [checked out], `html/dom/` measured @ 21.0% |
| `dom/` | 619 | [checked out], **26.7%** |
| `css/selectors/` | 531 | [checked out], **27.9%** |
| `domparsing/` | 64 | [checked out], **9.9%** |
| `encoding/` | 156 | [checked out], not yet run |
| `url/` | 6 (+28 `.any.js`) | [checked out], `.any.js` needs wptserve wrappers |
| `cssom/` | 2 | [checked out] |

**`css/` sub-specs (counted individually — the point of tracking css/ granularly):**

| Sub-spec | .html | note |
|---|---:|---|
| `css/css-grid/` | **2,226** | the single largest css sub-spec |
| `css/css-flexbox/` | **1,433** | |
| `css/selectors/` | 531 | measured @ 27.9% |

*(the local checkout has only these three css sub-specs; `css-position`, `css-fonts`, `css-color`,
`css-animations`, `css-writing-modes` and dozens more need fetching to measure.)*

### Loading, network & app-shell substrate — the "session/network reality" gap (the invisible 41 discarded sites)

`fetch/` · `xhr/` · `streams/` · `workers/` · `service-workers/` · `websockets/` · `webtransport/` ·
`cookies/` · `storage/` · `IndexedDB/` · `FileAPI/` · `content-security-policy/` · `mixed-content/` ·
`credential-management/` · `webauthn/` — **none checked out yet.** This is the substrate behind the
oracle's hydration-failure class.

### Interaction & input surface — cross-ref `docs/wiki/interaction-surface.md`

`uievents/` · `pointerevents/` · `touch-events/` · `input-events/` · `selection/` · `clipboard-apis/` ·
`intersection-observer/` · `resize-observer/` · `pointerlock/` · `fullscreen/` · `page-visibility/` —
**none checked out yet.** `intersection-observer/` is the highest-leverage (the live-viewport primitive).

### Graphics & media — "weeks, not ticks" (track separately; do not let scale distort Bar-1 core)

`webgl/` · `webgl2/` · `webgpu/` · `webcodecs/` · `media-source/` · `webaudio/` ·
**`encrypted-media/` — PERMANENT WALL (EME/DRM, settled); track as a FIXED known-gap, not a moving target.**

### Accessibility & i18n — cross-ref Part 12 (a11y-tree-as-oracle)

`accessibility/` (ARIA/accname) · `css-writing-modes/` · `MathML/` (if in scope) — none checked out yet.

### Platform / real-time — explicitly deferred; track but do NOT compete with doc-web core for priority

`webrtc/` · `webxr/` · `push-api/` · `background-sync/` · `background-fetch/` · `geolocation-API/` ·
`battery-status/` · `payment-request/` · `notifications/`.

---

## How this feeds the priority ledger

Each measured category's `(1 − pass%)` is a **divergence weight**; multiplied by the category's
**usage frequency** (the same `usage × divergence` formula the oracle's cluster ranking already uses,
Part 4), it slots directly into `PARITY-LEDGER.md` — **not a separate ranking scheme.** A category that is
huge but rarely load-bearing (much of `html/dom/`'s IDL-reflection surface) must not outrank a small,
ubiquitous one (`dom/` mutation, `css/selectors/`).

**The honest note the whole map turns on:** we do not need Chromium's *number*. We need **enough of the
spec that most of the real web works, and a graceful, honest decline for the rest.** WPT is how we see the
*shape* of "enough" — not a score to chase to 100%.

## Rank mechanisms by FLIP RATE, not failing-subtest count — CSS layout is a multi-assertion slog

`check-layout-th.js` files assert MANY geometry values and fail the whole file if any one is wrong. Manuk's
flex/grid geometry is off in several independent ways per file, so a single CORRECT fix (tick 97 offset
rounding, tick 98 margin-box extent) flips ZERO files — the area's pass count does not move even though the
fix is right. Lesson: an area's failing-subtest COUNT overstates its reachability when its tests are
multi-assertion. Rank by **flip rate** — how many subtests one fix actually turns green — not raw failing
mass. Corollary for flex/grid: either batch several geometry fixes per tick so a file crosses the line, or
prefer higher-flip areas (DOM/CSSOM property reflection, `css/selectors`, the html/dom attribute-reflection
mass) where one fix turns subtests green directly. [[conformance-and-oracles]]

## The leverage ranker has no term for unshipped spec (t1204)

`scripts/wpt-leverage.sh` ranks areas by
`usage × winnable-tests × room-to-grow × flip-rate`. Four consecutive ticks took its **#1** row
(`dom`) and paid **+975 subtests**. Its **#2** row, `css/css-values` (leverage 815, 1708/4201 =
40.7%, 2,496 failing), was measured the same way and refused:

```text
   random(          231     if(style)        217     calc-size()      194
   attr()           179     sibling-index    124     random-item()     63
   progress()        60     {{hosts}}         46     sibling-count     25
   ident()           24     interpolate-size  10     if(supports)      10
   if(media)          6
   ─────────────────────────────────────────────────────────────────────
   1,189 of 2,337 failing subtests = 50.9%   unshipped spec + wptserve templating
```

> **The formula has no term for whether the failing mass is SHIPPABLE SPEC.** An area whose gap is
> `random()` and `if(style)` scores exactly like an area whose gap is real, because both are *failing
> tests in a high-usage directory*.

This is I4's Pareto trap wearing the ranker's own arithmetic — the tick-84 shape (climbing the
encoding hill) with a subtler slope. Encoding was *visibly* exotic; `css/css-values` is **values**,
which sounds like the core of CSS, and half of it is a 2026 working draft.

**The remedy is one command, not a build:** before taking an area, run it with `--show-failures` and
classify the failing mass **by construct** — the same assertion-message histogram that found four
mechanisms in this window, asked one level up. `dom` survives that test (`assert_throws_dom`, missing
interface objects, real selector gaps — all shippable). `css/css-values` does not.

⚠ **The loop already knew and had not priced it.** Its own note read *"areas inflated by unshipped
spec (css-values = calc-size/random-item)"* — a caution recorded and never turned into a number.
**A caution that is not a number does not survive contact with a ranker that produces one.**

### The one real lever inside the remainder, measured and left named

`object-position` is wrong on **every** non-default value — `70% 60px`, `30px 50%`,
`calc(100% - 20px) …` all read back as `50% 50%`, which is the INITIAL value. So the declaration is
not applied at all on the shipping Stylo path, and `ObjectPosition` stores `x`/`y` as **fractions**
(`cs.object_position.x * 100.0`) so it cannot represent a length even once it is. Two changes: widen
the type, then map the property. It matters past its 24 subtests — `object-position` is how every
cropped hero and avatar keeps its subject in frame under `object-fit: cover`.
