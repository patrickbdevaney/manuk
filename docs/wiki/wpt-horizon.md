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

## A sparse checkout is a claim about where the tests are (t1219)

`WPT-AREAS.tsv` carried `cssom 0 0 0.0` for as long as anyone looked, and the loop's memory recorded
it as *"`cssom` is STILL missing (FILES 0)"*. It was not missing — the **aperture was aimed at the
old path**:

```text
   ~/wpt/cssom/       →  crashtests/ only, 2 files   ← the pre-move location, and what the
                                                        sparse checkout names
   ~/wpt/css/cssom/   →  225 files                   ← where the tests actually are
```

WPT moved its CSSOM tests from top-level `cssom/` into `css/cssom/`. The checkout faithfully pulls
the old directory; the runner faithfully reports `FILES 0`.

> **A sparse checkout is a claim about where the tests are, and it goes stale when upstream moves
> them. A `0/0` area is not "unmeasured because it is hard" — it is a claim that failed silently.**

Second instance: t1176 found `/css/support/` omitted — nine testharness helpers whose absence made
~700 files report ONE error instead of hundreds of subtests (+8,265 on repair).

**Measured after `git sparse-checkout add css/cssom`: 1917 / 3502 = 54.7%, 0 crashes.** 1,917
subtests were already passing and nobody knew, and 1,585 fail in the area this session spent seven
ticks inside — **every CSSOM lever priced this session was priced without its own area on the board.**

### ⚠ The PRIMARY metric goes DOWN, and that is the point

```text
   before   87750 / 122042  =  71.90%
   after    89667 / 125544  =  71.42%
```

Numerator +1,917, denominator +3,502 — an area below the running average pulls the headline down.
That is the correct behaviour of an honest denominator. **The engine did not get worse; the
measurement got wider.**

⚠ **Not durable:** `scripts/wpt-setup.sh`'s `SUBSETS` still names `cssom`, so a setup re-run reverts
this. Observer-owned; the one-line change needed is `cssom` → `css/cssom`.

## The newly-visible area, classified (t1220)

`css/cssom` entered the board at **1917/3502 (54.7%)** one tick after t1219 found the aperture aimed
at the pre-move path — after 22 ticks of CSSOM work priced *without its own area on the board*.

```text
   css/cssom   1,547 failing   ·   1.7% unshipped/very-new   →  the cleanest classifier result
                                                                 of the session (cleaner than dom's 3.4%)
     432   absolutize % / calc() into px   (computed `top`/`bottom`/`left`/`right`)
     164   raw inline style serialization  (`.5%` must serialize as `0.5%`)
     116   absent API / undefined          (`caretRangeFromPoint`, …)
     835   other
```

### Both named mechanisms are subsystem work

**1. Absolutize into pixels (432).** CSSOM's *resolved value* for an inset on a positioned element is
the **used** value in px:

```text
   top: 10%               →  expected "20px"   got "10%"
   top: calc(-1px + 10%)  →  expected "19px"   got "calc(-1px + 10%)"
```

**The blocker is that resolving it needs the CONTAINING BLOCK's size, and the serializer has only the
element's own border box.** Layout-info plumbing, not serialization — the same shape as every
CSSOM-lossiness instance: *the engine knows the answer and the reporting surface cannot reach it.*

**2. Raw inline style serialization (164).** `el.style.backgroundPosition` returns `"5% .5%"` where
CSSOM requires `"5% 0.5%"`. ⚠ A targeted *prepend-0* fix passes all 164 and is a **band-aid**:
`el.style` does not serialize values at all, it **echoes** the author's text, so every other CSSOM
normalisation is silently wrong the same way.

Both written down with their blockers rather than half-built — the third such call in one session
(t1213's frame `ReflowCtx`, t1217's `@namespace` validator), and each time the alternative was a
change on a seam that deserves a fresh session.
