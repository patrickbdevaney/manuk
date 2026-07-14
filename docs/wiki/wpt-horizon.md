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
