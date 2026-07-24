# SURFACE AUDITS — the loop leaves its own frame

Every other instrument here measures the browser **against a map**. This one measures **the map**.

Cadence: **every 10 ticks**, enforced by `scripts/surface-audit.sh` and `scripts/tick.sh`. It cannot be
skipped, and an audit that finds nothing is a suspicious audit — six phantom ❌s say the map is never clean.

---

## Audit #1 — tick 83

**Why it existed at all:** twice in one session this project made an order-of-magnitude leap, and **both
times a human had to point at it.** Not because the analysis was hard — because every instrument the loop
owned could only see what was already on its map, and *nothing ever checked the map*. `CONSTELLATION.tsv`
was a list of capabilities I could think of, and the entire history of this project says such a list is
wrong.

### Sources

* [Interop 2026 focus areas](https://github.com/web-platform-tests/interop/blob/main/2026/README.md) —
  20 focus areas + 4 investigations, agreed by Apple, Google, Igalia, Microsoft and Mozilla. *This is the
  closest thing that exists to "what the web actually needs next", and it is decided by the people who
  ship the engines.*
* [Interop 2026 announcement (WebKit)](https://webkit.org/blog/17818/announcing-interop-2026/) ·
  [web.dev](https://web.dev/blog/interop-2026)
* [Ladybird passes Apple's 90% WPT threshold (HN)](https://news.ycombinator.com/item?id=45493358) and
  [Browser Engines 2026 — a comparison](https://www.youngju.dev/blog/culture/2026-05-14-browser-engines-2026-chromium-gecko-webkit-servo-ladybird-comparison-deep-dive.en)

### The calibration this project never had

| | |
|---|---|
| **Ladybird, April 2026** | **2,067,263** passing WPT subtests · 97.8% of test262 (52,045/53,207) |
| **Manuk, tick 83** | **25,869** passing WPT subtests · test262 **never run** |
| Ladybird's trajectory | ~78% (mid-2024) → 90%+ (early 2026). **"The final 17% is the hardest."** |
| The bar | Apple's **90% of WPT subtests** — eligibility for an alternative engine on iOS |

We are at roughly **1.25% of Ladybird's absolute passing count.** That is the honest number, and it is the
first time this project has had an external scale to put itself against.

### The finding that changes the methodology

> *"Matching the behavior real-world sites depend on — including undocumented quirks that established
> engines have shipped for decades — is the work that has historically **killed independent engines**. A
> strict standards implementation that breaks sites relying on those quirks fails the only test that
> matters commercially: rendering the existing web."*

**WPT conformance is necessary and it is not sufficient.** This is the strongest external validation of the
two-anchor design — the 265-site **Chromium differential oracle** is not a nice-to-have beside WPT, it is
the anchor that catches the class of failure that has ended other engines. It is now a first-class row in
the constellation (`cross / real-world QUIRKS`), and the audit says it should never be traded away for
score.

### Added to the map: **20 capabilities that were not on it**

Interop 2026 named twenty priorities. **Fifteen of them were nowhere in our constellation:**

* **app** — `<dialog>`/popover · scroll snap · scroll-driven animations · **View Transitions** ·
  **Navigation API** · scoped custom element registries · JSPI (async wasm)
* **doc** — **container queries (incl. style queries)** · CSS anchor positioning · `attr()`/`zoom()`/
  `shape()`/`contrast-color()` · custom highlights · JPEG XL
* **platform** — fetch uploads + ranges (streaming) · WebTransport · **WebAuthn / passkeys**
* **media** — media pseudo-classes

And three from the Ladybird comparison that are pure blind spots:

* **`cross / test262`** — JS conformance. Ladybird tracks 97.8% of 53,207 subtests. **We embed
  SpiderMonkey and have never run it.** This is very likely a large, nearly-free number, and *not knowing*
  it is the point: we did not know we did not know.
* **`cross / quirks-mode rendering`** — the pre-standards web, and a huge fraction of the long tail.
* **`cross / developer tools`** — Ladybird names this a gap too. Not a rendering capability, but a browser
  without them is not a daily driver *for the people who build the web*.

### What we had been wrong about

**The map was 78 capabilities. It is 98.** Unknowns went from 14 to **32** — and that is the audit
*working*, not failing. The ratchet was rewritten in this same tick for exactly this reason: its invariant
is **`MEASURED`** (capabilities with a verdict), **not** `unknown`. A bigger, uglier, more honest map is a
good tick. **Discovery is never punished; only rot is.**

The single most uncomfortable line: **we did not have `WebAuthn` on the map at all.** The near-horizon
definition says "platform web = accounts and login", and passkeys are *how login works now*.

### Next audit due: tick 93

---

## Audit #2 — tick 93

**Sources:** [Interop 2026 focus areas](https://github.com/web-platform-tests/interop/blob/main/2026/README.md) ·
[web.dev/blog/interop-2026](https://web.dev/blog/interop-2026) ·
[Mozilla Hacks: Launching Interop 2026](https://hacks.mozilla.org/2026/02/launching-interop-2026/).

**Method:** reconciled the 20 Interop-2026 focus areas + 4 investigations against `CONSTELLATION.tsv`.

**Finding: the map is current.** Every focus area the four vendors agreed on is already on the map from
Audit #1 (tick 83) — anchor positioning, container/style queries, dialog/popover, View Transitions,
Navigation API, scroll-driven animations, WebRTC (now 91.6% cross-vendor). Nothing the world named this
cycle is missing from the constellation. Audit #1 did its job; ten ticks later the frame still holds.

**The one signal worth recording** — Interop 2026's **accessibility-testing investigation** ("generate
consistent accessibility trees across browsers"). This is not a gap in our map; it is external validation
of **Invariant I3**. The industry is now treating a *consistent, correct AX tree* as a first-class
cross-browser problem — which is precisely the substrate `manuk-a11y` already builds and feeds to the
agent. A from-scratch engine whose semantic tree is correct and stable is, by this signal, aligned with
where the platform is heading, not chasing it. The a11y/agent moat (I3) is reinforced, not threatened.

**No capabilities added, none corrected.** A clean audit here is honest, not suspicious: the additions
happened at Audit #1 and the reconciliation confirms they stuck. The next real map-expansion will come
from measuring the unmeasured WPT areas (the aperture), not from the Interop list, which we now cover.

**Next audit due: tick 103.**

---

## Audit #3 — tick 103 (2026-07-15)

**Method:** the Interop-2026 web reconciliation was done at Audit #1 (tick 83) and re-confirmed current at
Audit #2 (tick 93, ten ticks ago) — no vendor-named focus area is missing from `CONSTELLATION.tsv`. This
cycle audits the other half the protocol names: **the measured aperture vs. the checked-out surface**,
from the tree.

**Finding — the aperture is bounded by a NARROW checkout, not just by what the sweep ranks.** The sweep
measures ~16 areas, but the WPT checkout holds only **9 `css-*` subtrees** (flexbox, grid, sizing, fonts,
text, overflow, transforms, ui, backgrounds) + dom / html/dom / domparsing / url / encoding. The
high-usage subtrees **`css-values`, `css-position`, `css-display`, `css-color`, `css-cascade`,
`css-writing-modes`, and `html/semantics` / `html/canvas` are not checked out at all** — so they score an
invisible zero, the exact blindness §VI.3 warns about. This is the standing tee-up from Audit #2 ("the
next real map-expansion comes from measuring the unmeasured areas"), now made concrete: it is a
**`wpt-setup.sh` checkout expansion**, not an Interop-list gap.

**What we had been wrong about (mild):** the recent run of web-API-surface ticks (99–103) has been mining
the *measured* areas, and the clean single-mechanism wins there are visibly thinning (tick 102 neutral;
`appearance` declined as tail/supplement). That thinning is not "the frontier is done" — it is "the
frontier we can SEE is thinning." The unopened css/html subtrees are almost certainly where the next
large, usage-weighted mass sits, unranked.

**Steer (added to §VI.4 step 1):** a near-term tick should expand the WPT checkout to add
`css-values`/`css-position`/`css-display`/`css-color` + `html/semantics`, re-sweep, and let the histogram
rank the newly-visible mass — before assuming the measured areas are the whole board. No capability added
or corrected here (a checkout expansion is its own tick); the map (`CONSTELLATION.tsv`) remains current.

**Next audit due: tick 113.**

---

## Audit #4 — tick 113 (2026-07-15)

**Date:** 2026-07-15. **Sources searched (web):**
[web.dev/blog/interop-2026](https://web.dev/blog/interop-2026) ·
[web-platform-tests/interop 2026 README](https://github.com/web-platform-tests/interop/blob/main/2026/README.md) ·
[Igalia: Interop 2026 focus areas](https://www.igalia.com/news/interop-2026.html).

**Interop 2026 focus areas (20 areas / 33 proposals + 4 investigations):** Anchor Positioning, Container
Style Queries (`@container … style()`), Dialog & Popover enhancements (`<dialog closedby>`, `:open`,
`popover=hint`), View Transitions incl. **cross-document**, WebRTC; investigations: accessibility testing,
JPEG XL testability, mobile testing infra, WebVTT.

**Reconcile vs `CONSTELLATION.tsv`:** every vendor-named 2026 focus area is ALREADY on the map (anchor
positioning, dialog, popover, container/style queries, view transitions, WebRTC-adjacent WebTransport,
JPEG XL — all added in Audit t83). **No new capability rows needed; the map is current on the named
frontier.** Per protocol ("an audit that finds nothing is suspicious"), the finding this cycle is not a
missing row — it is a **status-correctness** defect in the map itself.

**What we had been wrong about (this one bit):** the `status` field is binary and it HID a lever as large
as a headline win. `app · attribute reflection` was marked **`gated` (G_REFLECT, tick 82: +9,940)** —
reading as *done*. This session (tick 113) found an equal-sized hole **behind** that gate:
`setAttribute`/`getAttribute` did not ASCII-lowercase HTML qualified names (DOM §Element), so EVERY
mixed-case IDL attribute (`accessKey`, `tabIndex`, `noValidate`, …) failed its whole `setAttribute()`
subtest family — **+10,249** (html/dom 45,495 → 55,744) once fixed. A capability marked gated had a second
lever bigger than the first, invisible because the reflection-suite files reported `testsCreated:0` under
`diag` (a diagnostic ARTIFACT — the tests ran fine at their real path).

**Steer (banked):** `gated`/`works` means "a slice is PROVEN," never "the capability is exhausted." When an
area's histogram still shows a large failing mass under a capability the map calls done, **suspect the
status, not the frontier** — reproduce the aggregate's real environment before trusting a diagnostic's
summary counter. The `CONSTELLATION.tsv` reflection row is corrected to record the tick-113 residual.

**Next audit due: tick 123.**

---

## Audit #5 — tick 123 (2026-07-16)

**Date:** 2026-07-16. **Sources searched (web):**
[Interop 2026 README](https://github.com/web-platform-tests/interop/blob/main/2026/README.md) ·
[web.dev/blog/interop-2026](https://web.dev/blog/interop-2026) ·
[wpt.fyi Interop 2026 dashboard](https://wpt.fyi/interop-2026?stable=) ·
[Mozilla Hacks: Launching Interop 2026](https://hacks.mozilla.org/2026/02/launching-interop-2026/) ·
[This Month in Ladybird — June 2026](https://ladybird.org/newsletter/2026-06-30/).

**Interop 2026 (20 focus areas / 33 proposals + 4 investigations):** anchor positioning, container style
queries, cross-document view transitions, dialog/popover enhancements, WebRTC (91.6% carried from 2025);
investigations: accessibility-tree consistency, JPEG XL testability, mobile WPT infra, WebVTT. **Every
vendor-named 2026 focus area is already on `CONSTELLATION.tsv`** (added Audit #2 t83, reconfirmed #4 t113).
Independent-engine signal (Ladybird, ~2.08M WPT subtests passing, first alpha targeted 2026): their named
hardest problem is **web-compatibility / engine-quirk divergence**, not spec coverage — which matches this
project's own browser-sniffing lesson.

**What we had been wrong about (the finding — an audit that finds nothing is suspicious):** the map claimed
to cover the CSS frontier (container queries, anchor positioning, view transitions, scroll-driven
animations were all present) but **silently omitted several equally-shipped, equally-Baseline CSS
primitives** that predate the ones it *did* list — a coverage bias toward the *novel* over the *load-bearing*.
Six capabilities the world names and the map did not, now ADDED with status `unknown` (per protocol — a
bigger, uglier map is a good tick; the ratchet rewards MEASURED, never punishes discovery):

| Added | Class | Why it was a real gap |
|---|---|---|
| **CSS nesting (native `&`)** | doc | Baseline 2023; *every* modern authored stylesheet nests now — as fundamental as the container queries the map already had. Stylo likely parses it; unmeasured. |
| **subgrid** | doc | Baseline 2023; nested grids aligning to parent tracks — a common real-layout primitive. |
| **`@scope` / scoped styles** | doc | component-scoped CSS; newer but shipping. |
| **`text-wrap: balance`/`pretty`** | doc | a visible typographic gap on headings/paragraphs. |
| **WebCodecs** (VideoDecoder/AudioDecoder/VideoFrame) | app | distinct from MSE — the low-level codec API for in-browser editors/players; media rows had demux/decode but not this JS surface. |
| **Sanitizer API** (`Element.setHTML`/`setHTMLUnsafe`) | platform | the platform replacement for DOMPurify — an XSS-safety primitive the security rows missed. |

**Steer (banked):** the map's growth has tracked *what's new and talked-about* (Interop headlines) more
than *what's shipped and load-bearing*. When reconciling, sweep the **Baseline-stable** set (features safe
to use for years), not only the current-year Interop list — the quiet, years-old primitives are exactly the
ones a novelty-biased map forgets it never added. Nesting/subgrid being absent while anchor-positioning was
present is that bias made concrete.

**LAST_SURFACE_AUDIT set to 123.**

**Next audit due: tick 133.**

## Surface audit @ tick 325 (2026-07-21) — reconciled after the counter unfreeze

The self-audit machinery froze TICK at 128 for ~200 ticks (status-update read TICK from STATUS and
wrote it back; fixed this session — TICK now derives from the journal). That retroactively marked the
surface audit "overdue by ~200 ticks", but no 200 cycles were actually skipped — the counter simply
never advanced. The audit SUBSTANCE is in fact freshly current: the observer's tick-328 three-way
deep-research pass (docs/loop/PHASE0-BOUNDED-REMAINDER.md + docs/loop/RESEARCH-SYNTHESIS-2026-07.md)
is a full leave-your-frame audit — external SOTA (HTTP Archive/Interop/Servo/Ladybird methodology), a
complete internal map-vs-reality pass, and a site-class × capability matrix checked against source. It
re-confirmed the standing finding this loop keeps re-learning: **the constellation runs
stale-PESSIMISTIC** — CSP, select actuation, sticky, hscroll, captions, popover, pointer-sequence,
:focus were ALL already built and mis-listed missing. This session added two more instances
(AbortSignal.timeout, scrollIntoView/checkVisibility/getAnimations — all already built when probed).
The map correction: PHASE0-BOUNDED-REMAINDER.md now supersedes the constellation priority rows and the
retired ready_pct metric; the real Phase-0 exit is the FIDELITY-SCORING-REDESIGN.md certificate.

**LAST_SURFACE_AUDIT set to 325.**

**Next audit due: tick 335.**

## Audit #6 — tick 326 (2026-07-21)

**This entry is the canonical-header formalization of the tick-325 audit above** (which used a
non-matching `## Surface audit @ tick 325` header, so `status-update.sh`'s
`^## Audit #N — tick M` derivation never registered it and the cadence field stayed stuck at 123).
No cadence was actually skipped: the TICK counter was frozen at 128 for ~200 ticks and the audit
SUBSTANCE was done fresh at 325 — the observer's tick-328 three-way deep-research pass
(`docs/loop/PHASE0-BOUNDED-REMAINDER.md` + `docs/loop/RESEARCH-SYNTHESIS-2026-07.md`) is a full
leave-your-frame audit: external SOTA (HTTP Archive / Interop / Servo / Ladybird methodology), a
complete internal map-vs-reality pass, and a site-class × capability matrix checked against source.

**Reconciled (this pass):** re-swept the constellation status histogram — 76 gated / 21 works /
17 partial / 30 missing / 2 unknown across 146 rows. The standing finding held again: the map runs
**stale-PESSIMISTIC**, not optimistic — tick 326 itself re-pinned four `partial` cells (file upload,
`<dialog>`+popover, hover/dblclick/contextmenu, native `<select>`) that were all already gated and
green. The novelty-bias steer from Audit #5 (sweep Baseline-STABLE, not just current-year Interop
headlines) remains the correct lens; the CSS-nesting/subgrid/`@scope`/WebCodecs/Sanitizer rows added
at 325 stay `unknown`, awaiting cheap probes.

**What we had been wrong about:** the cadence machinery itself — a non-canonical header silently voided
the 325 audit's counter update. Corrected here. The map priority is superseded by
PHASE0-BOUNDED-REMAINDER.md; the real Phase-0 exit is the FIDELITY-SCORING-REDESIGN.md certificate,
not `ready_pct` (retired).

**LAST_SURFACE_AUDIT set to 326.**

**Next audit due: tick 336.**

## Audit #7 — tick 337 (2026-07-21)

**Left the frame. Sources (read this pass, not from memory):**
- Interop 2026 authoritative area list — `github.com/web-platform-tests/interop/blob/main/2026/README.md`
  (20 focus areas, 15 new; + 4 investigations), cross-read against `webkit.org/blog/17818/`,
  `hacks.mozilla.org/2026/02/launching-interop-2026/`, `web.dev/blog/interop-2026`.
- Ladybird 2026 status — `ladybird.org/newsletter/2026-06-30/` + 2026-04/2026-01 (passed >90% of all WPT
  subtests Oct 2025; June 2026 ~2.079M passing; shipped file downloads, about:history, **Web Locks API**;
  WhatsApp Web reaches QR login).
- Baseline 2026 — `web.dev/blog/web-platform-01-2026`, `web-standards.dev/news/2026/01/scope-css-baseline/`
  (CSS anchor positioning is now Baseline Newly-available with Firefox 147; @scope is Baseline).

**Reconciled against CONSTELLATION.tsv (149 rows). The headline: the OUTSIDE frame is fully covered.**
Every one of the 20 Interop 2026 focus areas is ALREADY on the map with a verdict — container (style)
queries [missing], anchor positioning [missing], attr()/zoom/shape()/contrast-color() [missing, one row],
custom highlights [missing], dialogs+popovers [gated], fetch uploads+ranges [G_RANGE/uploads], IndexedDB
[gated t329], JSPI [missing], media pseudo-classes [missing], Navigation API [gated t309], scoped custom
element registries [missing], scroll-driven animations [missing], scroll snap [gated t266], view
transitions [gated t308], WebRTC [missing, out of scope], WebTransport [missing, deliberate HTTP/3
deferral]. Investigations too: JPEG XL [missing, below ROI t237], WebVTT [partial t258], a11y testing
[a11y roles t325]. Several rows were literally probed AGAINST "Interop 2026" back at t225-241, so the map
anticipated this list — no unmapped area exists on the outside.

**ADDED (an audit that finds nothing is suspicious — and this one did not):** `Web Locks API
(navigator.locks)` — status **gated**, `G_WEB_LOCKS`. It was BUILT (dom_bindings.rs + a RED-proven gate,
engine/page/tests/g_web_locks.rs: named-resource mutual exclusion, ifAvailable, resolve-with-value) yet
MISSING from the constellation entirely. Ladybird trumpeted shipping it in 2026; we already had it and had
not written it down.

**What we had been wrong about:** the usual direction, once more — stale-PESSIMISTIC — but this instance
is the INVERSE and worth naming: not a `missing` cell that was secretly built (t326's four re-pins), but a
whole capability that was green and RED-gated and *simply absent from the map*. The histogram counted 148
rows when the browser had ≥149 capabilities. A map that under-counts its own wins is as misleading as one
that over-claims; both make the ranking a confident wrong answer.

**RE-RANK note (not acted on this tick):** CSS anchor positioning crossed into Baseline 2026 (Firefox 147,
~91% traffic) — it graduated from "emerging" to "safe-to-use TODAY", which raises its priority within the
`missing` set (it is the pure-CSS tooltip/menu/popover placement primitive that replaces Floating UI). It
does not outrank the CO-#1 fidelity-instrument rebuild, but among capability levers it is now above the
other `missing` CSS rows. Container queries (CO-#1 (3)) remain the largest single missing CSS lever.

**LAST_SURFACE_AUDIT set to 337.**

**Next audit due: tick 347.**

## Audit #8 — tick 347 (2026-07-22)

### Sources (searched, not recalled)

* [Interop 2026 focus areas — WebKit announcement](https://webkit.org/blog/17818/announcing-interop-2026/) ·
  [web.dev](https://web.dev/blog/interop-2026) · [Igalia](https://www.igalia.com/news/interop-2026.html) ·
  [Mozilla Hacks](https://hacks.mozilla.org/2026/02/launching-interop-2026/)
* [Ladybird — This Month, June 2026](https://ladybird.org/newsletter/2026-06-30/) (WPT 2,078,912 subtests;
  crossed 90% of all WPT in Oct 2025; first alpha 2026; entry now maintainers-only)

### The external frame, June/July 2026

Interop 2026 = **19 focus areas + 3 cleanup + 4 investigations**, agreed by Apple/Google/Igalia/
Microsoft/Mozilla. Named areas cross-checked against our map: **Anchor Positioning** (row 94 `missing`),
**advanced `attr()` / `zoom` / `shape()`** (row 95 `missing`), **View Transitions incl. cross-document**
(same-doc gated row 89; cross-doc was ABSENT — added, below), **WebRTC** (91.6% pass rate industry-wide;
constitutionally OUT for us — a second media-stack subsystem), **Dialog + Popover** (both gated here),
**WebVTT** investigation (gated here). Investigations: **a11y-tree consistency** (our a11y is `partial`),
**JPEG XL** (row 14/JPEG XL `missing`, measured t237), **Mobile WPT infra** (N/A to us).

### ADDED

* **cross-document View Transitions (MPA)** — `unknown`. Interop 2026 expands View Transitions to the
  cross-document/navigation form (`@view-transition { navigation: auto }` + `pageswap`/`pagereveal`).
  Our same-document `startViewTransition` is gated (t308); the MPA form is unmeasured. Re-probe first —
  the same-doc plumbing may already cover part of it.

### CORRECTED / what we had been wrong about

* **The map is not blind to modern CSS — it is over-PESSIMISTIC about the bounded tail.** The dominant
  error this window is the *inverse* of the six historical phantoms: capabilities the audit lists
  (`DAILY-DRIVER-EDGES.md`, `PHASE0-BOUNDED-REMAINDER.md`) mark `missing`/`bounded` that are in fact
  BUILT and GATED. Verified already-built while hunting a tick this session: `<details>`/`<summary>`
  (g_details), `document.visibilityState`+`permissions.query` (G_VISIBILITY), `createObjectURL`,
  cookie SameSite/`__Host-`/`__Secure-` prefixes (g_cookie_attributes), Fullscreen, IndexedDB indexes,
  Selection scripting surface. Constellation UNKNOWNS are down to **3** (100-tab RSS, test262, and this
  audit's cross-doc VT). **Implication for tick-selection:** the genuine bounded gaps are nearly mined
  out; what remains is subsystems (media playback join, container queries, contenteditable, software
  WebGL) + a thin tail of real half-builds. Ticks 345/347 (HTTP conditional revalidation + Expires
  freshness) and 346 (drag editor half) were exactly that tail — real gaps behind rows marked `partial`.
* **Interop's modern-CSS marquee (anchor positioning, attr()/shape()/zoom) stays honestly `missing`**,
  not upgraded — it is in the board's named cut line (niche, feature-detects cleanly, cosmetic), and the
  Ladybird lesson ("the final 17% is the hardest", MPA/web-compat quirks over spec purity) says the
  daily-driver ROI is in the jarring-invariant tail and the subsystems, not the CSS niche.

## Audit #9 — tick 357 (2026-07-22)

### Sources (searched, not recalled)

* [Ladybird — This Month, June 2026](https://ladybird.org/newsletter/2026-06-30/) (WPT 2,075,546 →
  2,078,912, +3,366; "getting closer to our first alpha")
* [Interop 2026 dashboard/README](https://github.com/web-platform-tests/interop/blob/main/2026/README.md) ·
  [Igalia announcement](https://www.igalia.com/news/interop-2026.html) — no mid-year revision found;
  the February frame (20 focus + 4 investigations) stands as checked by Audit #8.

### The external frame, July 2026

Ladybird's June work-list is the sharpest available mirror for "what a pre-alpha engine chasing the
daily-driver bar actually ships": downloads, history, DevTools, **media playback-speed with
pitch-preservation (WSOLA)**, **muted-autoplay tri-state policy**, sandboxed services/GPU isolation,
WebAssembly GC, container-relative units, `contrast-color()`, and per-site compat fixes. Cross-checked
row by row against our map:

* **muted autoplay** — landed HERE at t352 within the same month Ladybird shipped its policy. The two
  projects independently ranked the same organ; validates the board's media ordering.
* **downloads / history / session UX** — built and gated here (t4/t163-167/T5 arc). No action.
* **container-relative units** — subsumed by the board's CO-#1 (3) container queries (Stylo-bound).
* **`contrast-color()`** — CSS niche, stays in the named cut line.
* **sandboxed services / GPU isolation** — our per-tab process model is SETTLED and sequenced
  (PROCESS-MODEL.md); not re-opened by another engine's timeline.

### ADDED

* **media / `playbackRate` (audible speed control)** — `missing`, previously UNLISTED. The IDL property
  exists inert (`el.playbackRate = 1`, event_loop.rs:2805) but the Transport does not scale time and the
  AudioFeed cannot resample; podcast/lecture 1.5–2x is a real daily-driver class (Ladybird judged it
  alpha-worthy). Bounded: video-only rate = scale dt; audible rate needs WSOLA-class time-stretch
  (BORROW candidate) — without it, rate≠1 must mute honestly rather than chipmunk.
* **app / WebAssembly GC** — `unknown`, previously UNLISTED. Kotlin/Wasm and Flutter-web class. Likely
  ALREADY WORKS (SpiderMonkey ships WasmGC enabled since ~SM120; our core wasm is gated green t225) —
  the stale-pessimistic rule says CHEAP RE-PROBE before any build: a `(ref struct)` module instantiate
  probe pins the cell in minutes.

### CORRECTED / what we had been wrong about

* Audit #8's central claim ("bounded gaps nearly mined out; remainder is subsystems + a thin tail")
  HELD through a 10-tick window that never once contradicted it: 349–355 were all subsystem organs
  (MSE join → audio → sync → muted → AV1 → AVIF) off the board's named list, zero phantom-❌ hunts.
  The map's error rate this window: two UNLISTED rows found by looking at another engine's changelog —
  the outside-frame mechanism doing exactly its job; neither is a phantom, both are additions.

## Audit #10 — tick 367 (2026-07-22)

### Sources (searched, not recalled)

* [WebMCP browser-support status, June 2026](https://dev.to/ai-agent-economy/webmcp-in-2026-which-browsers-support-navigatormodelcontext-complete-compatibility-status-1oe4) ·
  [W3C WebMCP Draft CG Report (Feb 2026) + Chrome 149 origin trial](https://www.buildmvpfast.com/blog/webmcp-browser-standard-ai-agents-2026)
* [Ladybird July activity (319 PRs, 47 contributors)](https://piunikaweb.com/2026/07/06/ladybird-browser-downloads-history-sandboxing/) — the
  July newsletter itself is not yet indexed; June's frame (Audit #9) stands.

### The external frame, late July 2026

**WebMCP crossed from spec-thread to shipping surface**: W3C Draft CG Report published Feb 2026;
Chrome 146 Canary behind a flag two weeks later; **public origin trial in Chrome 149 as of June
2026**. Google/Microsoft/Mozilla/Apple are all in the CG; only Chrome ships. This is the exact API
CONSTITUTION.MD **H2 scope item 2** names: *"Native WebMCP client — implement `navigator.modelContext`
as the first independent, non-Chrome implementation... converts the largest structural threat into
Manuk's native tongue."* The threat/opportunity clock the constitution described is now RUNNING.

### ADDED

* **agentic / navigator.modelContext (WebMCP)** — `missing`, previously UNLISTED (the map had NO row
  for the constitution's own named H2 marquee). Chrome 149 origin trial defines the test surface.
  **Scope note, stated to prevent drift:** this is an H2 item; Part VII defers H2 *productization* —
  but VII component #2 makes the agentic surface v1's differentiator and I3 forbids letting it lag.
  The BOUNDED v1-compatible slice is the page-facing API surface (registerTool/unregister +
  tool-manifest plumbing into the existing agent seams), with I6 taint discipline from day one
  (page-declared tools are adversarial input). The full client productization stays H2. Decision
  belongs to the board/observer — the row exists so the clock is on the map.

### CORRECTED / what we had been wrong about

* Ladybird velocity check (319 PRs/47 contributors in July) keeps the calibration honest: their
  alpha-chase list (Audit #9) remains the closest mirror, and nothing in it contradicts the current
  board ordering. No correction to existing rows this window — the t365 WebVTT fix was the last
  stale-pessimistic catch, and this audit's yield is the OPPOSITE failure mode again: a marquee
  item the map never listed. Both audits #9 and #10 found their value OUTSIDE the constellation's
  frame, which is the mechanism working as designed.

## Audit #11 — tick 377 (2026-07-22)

### Sources (searched, not recalled)

* [web.dev — New to the web platform, June 2026](https://web.dev/blog/web-platform-06-2026) ·
  [Edge 150 release notes](https://learn.microsoft.com/en-us/microsoft-edge/web-platform/release-notes/150)
* Chrome 151 stable (2026-07-28 upcoming); MV2 extension removal this month; Firefox moving to a
  two-week cadence in September.

### The external frame

Two platform behaviors crossing into Baseline territory, one enrichment of an existing row:

* **Promise-returning `scrollBy`/`scrollTo`** — programmatic scrolls now resolve a Promise when the
  scroll completes (kills the settle-timer/scroll-event-polling idiom). Our scroll methods return
  undefined; code `await`ing them gets `undefined` (awaitable, resolves immediately) — NOT a throw,
  so the failure mode is soft (post-scroll code runs before settling). Low-severity, bounded.
* **Web App Origin Migration** — PWA install-state trust migration; we hold no install state.
  OUT for v1 (no row; recorded here as considered-and-excluded).
* **WebMCP detail** (enriches the t367 row): the surface is TWO APIs — a **Declarative API** (HTML
  forms + standard elements annotated as tools) and an **Imperative API** (JS `registerTool`). The
  declarative half is an even more bounded v1-compatible slice than the imperative one: it reads
  ANNOTATIONS off the DOM we already own, no new JS surface — worth naming in the row for whenever
  the board takes the item.

### ADDED

* **app / promise-returning scroll methods** — `missing` (soft): `scrollTo/scrollBy/scrollIntoView`
  should return a Promise resolving on scroll completion. Ours return undefined (awaitable but
  immediate). One-line-ish once smooth-scroll settling exists; till then an immediate resolved
  Promise is spec-adjacent-honest (our scrolls ARE instant — there is no animation to wait for, so
  resolving now is truthful, not a stub). Genuinely tick-sized.

### CORRECTED

* The WebMCP row gains the declarative/imperative split note (above). No stale-pessimistic finds
  this window — the map has been re-probed heavily for 26 ticks and its error rate is currently
  additions-from-outside, not phantom ❌s.

## Audit #12 — tick 387 (2026-07-22)

### Sources (searched, not recalled)

* [web.dev — New to the web platform, June 2026](https://web.dev/blog/web-platform-06-2026) ·
  [web.dev Baseline digests (May/June 2026)](https://web.dev/series/baseline-newly-available)
* [Chrome 151 beta notes](https://developer.chrome.com/blog/chrome-151-beta) — Chrome 151 stable
  rolled out mid-July; Chrome 152 stable expected 2026-08-25.

### The external frame

One Baseline-crosser worth a row, one map validation, several named watches/exclusions:

* **`field-sizing` CSS property** — Baseline Newly available as of June 2026 (Firefox 152 completed
  the trio; Chrome 123+, Safari 26.2+). `field-sizing: content` lets form controls (textarea above
  all) size to their content instead of `cols`/fixed UA dimensions. We hand textareas a cols-derived
  width in the Stylo post-pass — exactly the seam this property must override. Bounded: parse the
  property, and when `content`, skip the UA fixed-size hint and let intrinsic sizing run. ADDED.
* **Programmatic scroll Promises** — shipped Chrome 150, on this month's platform roundup. LANDED
  here at t378 BEFORE the roundup listed it — the audit's map-ahead-of-the-web moment; validates
  the t377 add. No action.
* **`rect()`/`xywh()` in `shape-outside`** — Baseline; but we do not implement `shape-outside` at
  all (float exclusion geometry). That is the honest gap — the functions are the small half. Noted
  on the css residue pile, not tick-sized as a standalone add; needs the shape-outside organ first.

### WATCHES (single-engine, not Baseline — re-check next audit)

* `text-fit` (Chrome 150 only) — auto font scaling to container; large layout surface.
* CSS gap decorations (Chrome 149 only) — painted rules in grid/flex gaps.
* `focusgroup` attribute (Chrome 150 only) — declarative arrow-key navigation; NOTE: agent-surface
  relevant (component #2) the moment a second engine signals.
* `aria-actions` (Chrome 151 only) — secondary actions on composite widgets; same component-#2 note.

### EXCLUDED (considered, with reasons)

* WebSocket-in-BFCache (Chrome 149) — we have the MPA lifecycle pair but no BFCache freeze model;
  out until a BFCache row exists at all.
* Notification action buttons — OS notification integration; shell scope, not rendering parity.
* `background-clip: border-area` — Safari-only.
* Direct Sockets permission split / SCTP-in-SDP WebRTC — no Isolated Web Apps, no WebRTC in v1.

### ADDED

* **doc / field-sizing** — `missing`: `field-sizing: content` (Baseline June 2026) must let a
  textarea/input size from content, overriding the UA cols-hint seam in the Stylo post-pass.
  Tick-sized.

### CORRECTED

* None stale-pessimistic this window; the t378 scroll-promise landing pre-empted the platform
  roundup — additions-from-outside remains the map's only active error mode.

## Audit #13 — tick 397 (2026-07-22)

### Sources (searched, not recalled)

* [Firefox 153 release notes](https://www.firefox.com/en-US/firefox/153.0/releasenotes/) (2026-07-21,
  the week's engine release) · [web.dev June-2026 roundup](https://web.dev/blog/web-platform-06-2026)
  (re-checked; #12 covered it). No Safari stable since 26 (May); Chrome 152 due 2026-08-25.

### The external frame

A quiet platform week (Firefox 153 is mostly product surface: PDF merge, HDR video on Windows).
The standards-track items, mapped:

* **`IDBObjectStore.getAllRecords()` / `IDBIndex.getAllRecords()`** (Firefox 153 beta; Chrome
  shipped earlier) — batch record retrieval (key+primaryKey+value in one call, directional).
  ENRICHES the existing IndexedDB-indexes row (t329-gated): a bounded method-pair on a built
  organ, tick-sized when the row is next touched. Not yet Baseline (Safari absent).
* **`Error.stackTraceLimit`** (Firefox 153 beta) — engine-level (SpiderMonkey exposes it);
  worth a one-line probe next JS tick, likely already answered by mozjs. WATCH.
* **`RTCDtlsTransport.getRemoteCertificates()`** — WebRTC is out of v1 scope (no RTC stack).
  EXCLUDED with reason, consistent with audit #12's Direct-Sockets exclusion.
* **HDR video playback** — media output device tier; our audio-out is gated-on-PCM and video is
  frame-decode; HDR tone-mapping is named OUT for v1 (no compositor color management).

### ADDED

* None — the week's Baseline-crossers were consumed by #12 (field-sizing, landed t388).

### CORRECTED

* None stale-pessimistic. The t395 path-pairing find is recorded on the instrument side
  (conformance-and-oracles.md), not the map: constellation rows key by capability, and the
  display-diff UPPER-BOUND caveat lives with the ledger it qualifies.

LAST_SURFACE_AUDIT 387→397; next due 407.

## Audit #14 — tick 407 (2026-07-22)

Same-day as #13 (cadence is tick-based and the harvest arc burned ten ticks in one day); the
honest delta is therefore RANKING, not discovery. Sources: [Interop 2026 announcement/README]
(https://github.com/web-platform-tests/interop/blob/main/2026/README.md ·
https://webkit.org/blog/17818/announcing-interop-2026/ ·
https://web.dev/blog/interop-2026) — re-read against the map; [Chrome 152 tracking]
(https://portableapps.com/news/2026-07-06--google-chrome-portable-152-dev-released ·
browsercalendar) — stable due 2026-08-25, last 4-week-cadence release.

### The external frame

Interop 2026 (19 focus areas) NAMES three rows our map already carries as `missing` — the four
vendors declaring them the year's interop priorities is a usage-weight signal the histogram
cannot produce:

* **CSS anchor positioning** (row 95, missing, t230 probe) — RE-RANKED UP: Interop focus +
  popover/tooltip class (matches our dialog/top-layer work). The natural next CSS capability
  arc after the ledger re-ranks.
* **shape() / attr() type()** (row 96, missing, t230 probe) — Interop focus; bounded parse+
  paint work, Stylo-side (the live cascade rule applies).
* **Scoped custom element registries** (row 92, missing, t225 probe) — Interop focus
  (CustomElementRegistry() constructor); enterprise app-shell food.
* **Navigation API precommitHandler** (t309 row) — ENRICHES the landed Navigation API;
  bounded method-option when the row is next touched.
* **CSS scroll snap** (row 87) — Interop focus; we are GATED already, and the named residue
  (horizontal scroll range = 0 in layout) is exactly what the focus-area tests would catch.
  The residue's priority rises with the vendor signal.

### ADDED

* `navigator.cpuPerformance` (CPU Performance API, Chrome 152 default-on) → constellation row,
  `unknown` — Chrome-only, not Baseline, low v1 weight; pinned so the map is not surprised.

### EXCLUDED (with reason)

* WebRTC focus area — out of v1 scope (no RTC stack), consistent with #12/#13.
* Manifest V2 sunset — extensions out of v1 scope.

No stale-pessimistic finds this pass (the t402-404 gates are hours old). LAST_SURFACE_AUDIT
397→407; next due 417.

## Audit #15 — tick 418 (2026-07-22)

Cadence-driven (due at 417; #14 was tick 407). Sources searched THIS pass (not from memory):
[Interop 2026 README + selection-process](https://github.com/web-platform-tests/interop/blob/main/2026/README.md ·
https://github.com/web-platform-tests/interop/blob/main/2026/selection-process.md),
[web.dev/blog/interop-2026](https://web.dev/blog/interop-2026),
[WebKit Interop 2026](https://webkit.org/blog/17818/announcing-interop-2026/),
[Mozilla Hacks launch](https://hacks.mozilla.org/2026/02/launching-interop-2026/);
[Ladybird June-2026 newsletter](https://ladybird.org/newsletter/2026-06-30/) +
[downloads/history/sandboxing coverage](https://piunikaweb.com/2026/07/06/ladybird-browser-downloads-history-sandboxing/).

### The external frame — what changed since #14

Interop 2026 is now stated as **twenty** focus areas (#14 said 19). The sharpest new signal is the
named **20%-of-score cluster**: *advanced `attr()`* + *`getAllRecords()` for IndexedDB* + *WebTransport*
+ *JSPI* (JS Promise Integration for Wasm). Plus a **web-compatibility** focus area bundling *ESM module
loading*, *scroll-vs-animation event timing*, and *`user-select`*.

Reconciled against CONSTELLATION.tsv:
* **advanced `attr()`** — already row 96 (`attrfn:no`, measured). On map.
* **WebTransport** — already row 100 (missing, HTTP/3, deliberately out of V1-SCOPE). On map; the
  vendor signal does NOT change the scope call (no QUIC/HTTP-3 stack in v1).
* **JSPI** — already measured by G_PROBE_CAPABILITIES (`jspi:no`). On map.
* **scroll-vs-animation event timing** — covered by the scroll-driven-animations row (`scrolldriven:no`).

### ADDED (the map-wideners — the point of the audit)

* **`user-select` (CSS)** → constellation `unknown`. ZERO hits in engine/css — it was NOT on the map at
  all, yet `user-select:none` is on nearly every button/toolbar/drag-handle on the web. Bounded future
  work (does the selection engine honor `none`/`all`?). This is the genuine discovery this pass.
* **IndexedDB `getAllRecords()`** → `unknown`. IDB is on the map; this specific 20%-weight Interop
  method is a bounded add-on to the existing surface.
* **ESM module-graph loading (import/export resolution)** → `unknown`. PARTIAL today — engine/page runs
  `type=module` as a deferred script (lib.rs:1384/1448) but the static import-graph resolve/link/eval
  order is unmeasured. Added to force a probe.

### MEASURED-and-PINNED this window (not a phantom pass)

Tick 418 also pinned **`intl:yes`** (Intl + full ICU) — a capability that had been carried nowhere on
the map and was already working. The stale-pessimistic rule pays a seventh time.

### EXCLUDED (with reason)

* WebRTC (row 67) and WebTransport (row 100) — no RTC/QUIC stack in v1; consistent with #12–#14.
* Ladybird's June-2026 process-isolation / GPU-sandbox / downloads work — architecture + shell, not a
  rendering capability our corpus can see; the downloads/history/session shell is already v1-scoped.

Ladybird reference point unchanged as a north star: ~97.8% test262, ~2M WPT subtests — test262 stays
our biggest never-run unknown (row present). LAST_SURFACE_AUDIT 407→418; next due 428.

## Audit #16 — tick 428 (2026-07-22)

Cadence-driven (due at 428; #15 was tick 418). Source searched THIS pass (web, not memory):
[web.dev New to the web platform / Baseline 2026 digests](https://web.dev/blog/web-platform-01-2026 ·
https://web.dev/baseline/2026 · https://web.dev/blog/baseline-digest-jan-2026 ·
https://dev.to/homayounmmdy/new-features-added-to-the-web-platform-in-may-2026-5b7a).
Also reconciled the EMPIRICAL surface probed across ticks 420-427 (the binary-seam vein), which is a
truer surface audit than release notes — it measures what actually works vs. what is claimed.

### The external frame — what changed since #15

Baseline-2026 signal (Chrome 144 / Firefox 147 era): **Temporal** (date/time), **Service Worker
modules**, **Map.getOrInsert/getOrInsertComputed**, **CSS Anchor Positioning** (Firefox 147),
**display multi-keyword** (`inline flex`), **`:open` pseudo-class**, **contrast-color()**, **Trusted
Types**, **Document Picture-in-Picture**.

Reconciled + MEASURED against the actual engine (a probe, not an assumption):
* **Temporal** — MEASURED `temporal:yes` and PINNED (G_PROBE_CAPABILITIES). SpiderMonkey ships it in the
  verified build; calendar arithmetic RED-proves it (2020-01-15 + 40d = 2020-02-24, dayOfWeek 3, 25h
  Duration = 25h, PlainTime 10:30+45m = 11:15). Was carried NOWHERE on the map — the discovery this pass.
* **Also measured WORKING** (SpiderMonkey built-ins, unlisted): `RegExp.escape`, `Float16Array`,
  `Error.isError`, `Uint8Array.fromBase64`, `Promise.try`, `Map.groupBy`, `Iterator` helpers, `display:
  inline flex` parse. Not each pinned (the probe would balloon); noted here as the stale-pessimistic rule
  paying again — the JS surface is far ahead of the map.
* **Anchor Positioning** — already `anchorpos:no` (G_PROBE_CAPABILITIES). On map; vendor signal doesn't
  change it (a layout subsystem, not v1-bounded).
* **Service Worker modules** — SW runtime is a known XL out-of-v1 gap; the `type:'module'` refinement
  rides on top of it. Excluded, consistent with prior audits.
* **contrast-color() / Trusted Types / Document Picture-in-Picture** — not on the map. Trusted Types is a
  SECURITY seam (Phase-2, per CONSTITUTION Part-VII layering); PiP is a window-management shell feature;
  contrast-color() is a bounded CSS color function. Added `:open` and contrast-color as unknown rows.

### ADDED (the map-wideners — the point of the audit)

* **`:open` pseudo-class (CSS)** → `unknown`. MEASURED absent (`details[open]` matches by ATTRIBUTE, but
  `el.matches(':open')` is false). Styles `details`/`dialog`/`select`/`<details>` open state — a real,
  bounded CSS-selector gap.
* **`form.elements` HTMLFormControlsCollection** → `unknown`. MEASURED absent this session — `form.elements`
  is `undefined` and named access (`form.a`) fails, though `new FormData(form)` works. Every form library
  and serializer enumerates via `form.elements`. Bounded-ish (indexed + named access collection).
* **`CSSStyleDeclaration.item(i)` / `.length`** → `unknown`. MEASURED absent (indexed iteration over an
  inline style declaration throws). Low-value but on the map now.
* **custom-element `attributeChangedCallback` on a LIVE setAttribute** → `unknown` (partial). MEASURED:
  the callback fires for attrs PRESENT at upgrade, but a later `setAttribute` does not trigger it, and
  `connectedCallback` fires via the mutation microtask (async, not spec-synchronous). The L-sized
  custom-element reactions subsystem.
* **`contrast-color()` (CSS)** → `unknown`. Baseline-2026 color function; not on the map.

### MEASURED-and-PINNED this window

`temporal:yes` (see above). Plus the eight binary-seam CAPABILITY ticks 420-427 each flipped a
constellation row unknown/works→gated (getAllRecords, structuredClone-binary, Blob-binary,
canvas-ImageData, TextDecoder-encodings, template.content, live-searchParams, computed-CSS-vars).

### EXCLUDED (with reason)

* Service Worker runtime + SW modules — XL, out of v1 (rendering/agentic scope), consistent with #12-#15.
* Document Picture-in-Picture — window-management shell feature, not a rendering capability the corpus sees.
* Trusted Types — a Phase-2 SECURITY seam (structural DOM-XSS defense); noted, not added as a render row.
* Map.getOrInsert — a SpiderMonkey built-in not yet in the verified build; we cannot add SM built-ins
  (I2: never patch the engine's internals), so it is a bump-tracked item, not agent work.

LAST_SURFACE_AUDIT 418→428; next due 438.

## Audit #17 — tick 438 (2026-07-23)

**Sources.** web.dev Baseline 2026 + the May/April/March 2026 monthly digests
(https://web.dev/baseline/2026, https://web.dev/blog/web-platform-05-2026); MDN Baseline glossary. Plus
this window's own RED probes (the strongest source: MEASURED on the shipping tree), ticks 429-439.

**The frontier is well-mapped — the external check found no order-of-magnitude blind spot.** Every CSS
feature the Baseline-2026 digests flagged was already on the map or already gated: `contrast-color()`
(unknown row 180), `:open` (GATED t429, row 170), CSS units widely-available (Stylo). The one genuinely
off-map external signal is the **view-transition pseudo-classes** (`:active-view-transition`,
`:active-view-transition-type()`) — added as `unknown`. View Transitions themselves are gated (t308); these
are a bounded selector refinement on top.

### RECONCILED (stale unknown → gated — the map catching up to our own landed ticks)

* **`CSSStyleDeclaration.item(i) / .length`** (row 178) was the t428-audit `unknown`. Tick 432 GATED it
  (`G_CSSOM_ENUMERATION`, row 181 — inline + computed `.item`/`.length`/indexed getter + `!important`
  round-trip). Corrected to `gated`. This is the audit's job: memory (row 178) had gone stale from our own
  landed work (row 181), and only a reconcile pass catches it.

### ADDED — the DOM-write vein, measured-and-gated this window (map-wideners, ticks 435-439)

The form/collections/select DOM was carried largely UNMEASURED at the granularity a real widget hits. Five
rows added, all `gated`, all RED-proven this window:

* **`<table>` DOM read/write API** (`G_TABLE_DOM`/`G_TABLE_WRITE`, t435-436) — `table.rows` live in LOGICAL
  order, `tr.cells`/`rowIndex`/`cellIndex`, and `insertRow`/`insertCell`/section+caption builders. Was
  entirely `undefined`/throwing before.
* **`element.form`** (`G_FORM_OWNER`, t437) — the form-owner every form library reads; was `undefined`
  incl. the `form=` reassociation case, and it silently broke `ElementInternals.form`.
* **`<select>` write API** (`G_SELECT_WRITE`, t438) — `select.add()` was `undefined` and — the ugly one —
  `select.remove(0)` DETACHED THE WHOLE SELECT (fell through to `ChildNode.remove`). A corruption bug
  dressed as a working method.
* **`option.text` + `Option()` defaultSelected** (`G_OPTION_TEXT`, t439) — `option.text` (the canonical
  chosen-label read) was `undefined`; the constructor ignored `defaultSelected`.

### ADDED — genuine unknowns (the point of the audit: a bigger, uglier map)

* **`select.options.length` setter (truncation)** → `unknown`. MEASURED no-op this session — the classic
  `select.options.length = 0` "clear the dropdown" idiom does not truncate, because the native `options`
  getter returns a fresh Array and a length write does not persist. Bounded; lower value than add/remove
  (already gated t438), so pinned not built.
* **view-transition pseudo-classes** (`:active-view-transition` / `-type()`) → `unknown`. External signal;
  not yet measured here.

### EXCLUDED (with reason)

* Service Worker runtime, Document Picture-in-Picture, Trusted Types, WebGL, WebRTC — unchanged from prior
  audits (XL out-of-v1 subsystems, or Phase-2 security seams).
* SpiderMonkey built-ins ahead of the map (per audit #16) — not agent-editable (I2: never patch engine
  internals); bump-tracked, not audit rows.

**What we had been wrong about this pass:** row 178 said `CSSStyleDeclaration.item` was an open `unknown`
when we had gated it six ticks earlier (t432). The map lied stale-PESSIMISTIC again — the recurring failure
mode this instrument exists to catch. No stale-OPTIMISTIC lie found this pass (nothing marked works/gated
that measured absent).

LAST_SURFACE_AUDIT 428→438; next due 448.

## Audit #18 — tick 448 (2026-07-23)

**Sources (searched, not from memory):**
* Interop 2026 focus areas + investigation efforts — authoritative list from the WPT interop repo
  (https://github.com/web-platform-tests/interop/blob/main/2026/README.md) cross-checked against the
  WebKit announcement (https://webkit.org/blog/17818/announcing-interop-2026/). The 20 focus areas:
  Anchor Positioning, advanced attr(), Container Style Queries, contrast-color(), CSS Zoom, Custom
  Highlights, Dialog/popover additions, Fetch uploads and ranges, IndexedDB getAllRecords(), JSPI for
  Wasm, Media pseudo-classes, Navigation API, Scoped custom element registries, Scroll-driven
  Animations, Scroll Snap, shape(), View Transitions, Web Compat, WebRTC, WebTransport. Investigation
  efforts: Accessibility testing, JPEG XL, Mobile testing, WebVTT.
* Ladybird 2026 newsletters (https://ladybird.org/newsletter/2026-06-30/) — WPT 2,078,912 subtests,
  test262 97.8%; recent adds Web Locks + file download (both already on our map).

**RECONCILE result — the map is CURRENT against Interop 2026.** Every one of the 20 focus areas and all
4 investigation efforts is already a constellation row (a testament to audits #15–#17 which caught the
Interop-2026 set as it was announced). Spot-check of the ones most likely to be unmapped:
* CSS Zoom (per-element `zoom`), advanced/typed attr(), shape(), contrast-color() → all on row 99
  (`doc — CSS attr()/zoom/shape()/contrast-color()`, partial: content:attr string form landed t409, the
  Level-5 typed forms + per-element zoom + shape() + contrast-color still MISSING).
* Container Style Queries → row 97 (gated t379; style()/scroll-state() queries are the named residue,
  they follow the size machinery).
* JSPI for Wasm → row 96 (missing; wasm core works). JPEG XL → row 101 (measured-absent, below ROI:
  Safari-only adoption). Fetch uploads+ranges → row 102 (gated t228, Range+206 byte-exact).
* Scroll-driven animations → row 91 (missing; ScrollTimeline absent). Scoped custom element registries,
  Custom Highlights, Anchor Positioning, WebRTC, WebTransport → all present as `missing`/`✗` rows.

### ADDED — genuine unknowns (the point of the audit)

* **`pointer-events` (CSS)** → now `gated` (G_POINTER_EVENTS, t448 this same window). It had had ZERO hits
  in engine/css and was NOT on the map at all — a true unknown-unknown that audits #15–#17 missed even
  while cataloguing the whole Interop 2026 set, because it is an OLD Baseline property, not a new one. It
  surfaced via the constellation's "? outranks ✗" probe pass this session, and it carried a real
  behavioral defect (elementFromPoint returned a pointer-events:none overlay, swallowing clicks), not just
  a getComputedStyle gap. Landed the same tick it was discovered.

### STILL OPEN from prior audits (carried, not re-measured this pass)

* **`user-select` (CSS)** (audit #15, row 165) — remains `unknown`. Re-confirmed genuinely absent this
  session (ZERO hits, getComputedStyle undefined), but its load-bearing effect (suppress selection) needs
  USER mouse-drag selection GEOMETRY, which is unmodelled (row 18). Its only testable surface today is
  getComputedStyle honesty — thinner than pointer-events, so pinned not built.

### EXCLUDED (with reason)

* Service Worker runtime, WebGL, WebRTC, WebTransport, JSPI, scroll-driven animations, JPEG XL — unchanged
  from prior audits (XL out-of-v1 subsystems, or measured below the ROI line).

**What we had been wrong about this pass:** `pointer-events` — a Baseline-since-forever property with a
real click-eating defect — was invisible to the map entirely. The recurring lesson holds: the audit's job
is to find holes the histogram can't see, and an OLD property is exactly the blind spot a "what's new in
Interop 2026" scan walks right past. No stale-OPTIMISTIC lie found (visibilityState/permissions.query/
userAgentData were probed and confirmed already-gated — stale-PESSIMISTIC on the pivot list, not the map).

LAST_SURFACE_AUDIT 438→448; next due 458.

## Audit #19 — tick 458 (2026-07-23)

**Sources (searched, not from memory):**
* Interop 2026 focus areas — re-confirmed via the WPT interop repo README + web.dev's Interop 2026 post
  (https://web.dev/blog/interop-2026) and the Mozilla/WebKit launch posts. The set is UNCHANGED from
  audit #18: `:open`, `popover="hint"`, ESM module loading, scroll/animation event timing,
  unprefixing `-webkit-user-select`, WebTransport (HTTP/3), cross-document view transitions, plus the
  CSS interop set already mapped.
* Baseline 2026 monthly digests (https://web.dev/baseline/2026, web.dev/blog/baseline-digest-*-2026) —
  the NEW-in-2026 items: `:active-view-transition` (Jan), the **`ric` unit** (Jan, root-relative
  ideographic — and its sibling `ic`), **Zstandard `zstd` Content-Encoding** (Feb), plus vaguer CSS/API
  batches in Apr/May.
* Ladybird 2026 newsletters (https://ladybird.org/newsletter/2026-06-30/) — WPT 2,078,912 subtests,
  test262 imported upstream (53,207 subtests, 97.8% pass). Recent Ladybird adds: downloads, history,
  sandboxing — all already on our map or scoped.

**RECONCILE result — the map is CURRENT against Interop 2026 and near-current against Baseline 2026.**
Spot-checks:
* `:active-view-transition` / `:active-view-transition-type()` → already row (audit #17), `unknown`.
* `user-select` (unprefix focus area) → already row (audit #15), `unknown` — carried, see below.
* `popover="hint"` → the popover row is already `gated` (G_POPOVER: detect/reflect auto|manual|null/
  showPopover/beforetoggle+toggle). `hint` is a bounded VALUE-refinement of a gated capability, not a
  new class — noted as residue on that row, not a new row.
* ESM module loading → carried as the app-class `?` (memory: import-graph is a subsystem, not atomic).

### ADDED — genuine unknowns (the point of the audit)

* **`ic` / `ric` font-metric CSS units** (Baseline Newly Available Jan 2026) → new `unknown` row in the
  doc class. A NEW font-relative length unit is exactly the blind spot a "what's new in Interop" scan
  walks past (it is a Baseline item, not an Interop focus area) — the same shape as the `pointer-events`
  miss in audit #18 and the `ric` sibling of the existing ch/ex font-metric lever. Zero evidence either
  resolves through the cascade here; added unmeasured, to be probed.

### THIS TICK'S CAPABILITY (context, not an audit find)

* Completeness identity closed: `navigator.deviceMemory` (was absent) + canonical `navigator.platform`
  (`"linux x86_64"`→`"Linux x86_64"`), `G_DEVICE_IDENTITY`. The `visibilityState`/`permissions.query`/
  `userAgentData` cluster was re-probed and confirmed already-built (stale-PESSIMISTIC on the pivot
  list, not the map) — the recurring lesson, again.

### STILL OPEN / EXCLUDED (with reason)

* **`user-select` (CSS)** — remains `unknown`; its load-bearing effect (suppress selection) needs USER
  mouse-drag selection geometry (unmodelled), and crates.io Stylo fences the property behind a
  `servo_pref` (the `./stylo` checkout builds nothing) — a blast-radius pref flip or a manuk-side
  supplement, not atomic. Carried, unchanged.
* **`zstd` Content-Encoding** (new Baseline Feb 2026) — a documented **v1-scope deferral** (lever board
  SKIP list: HTTP/3/QUIC, zstd, coalescing). Recorded here for map honesty; NOT added as a
  constellation row because the deferral is already written down and stable. gzip/brotli cover the wire
  today.
* Service Worker runtime, WebGL, WebRTC, WebTransport, JSPI, scroll-driven animations, JPEG XL —
  unchanged from prior audits (subsystems or below the ROI line).

**What we had been wrong about this pass:** the `ic`/`ric` ideographic units — a brand-new Baseline
length unit — were invisible to the map. No stale-OPTIMISTIC lie found (nothing marked works/gated that
measured absent; the identity cluster was stale-PESSIMISTIC and is now measured/closed).

LAST_SURFACE_AUDIT 448→458; next due 468.

## Audit #20 — tick 468 (2026-07-23)

**Sources (searched live, not from memory):**
* web.dev/blog/interop-2026 · webkit.org/blog/17818 · hacks.mozilla.org (Launching Interop 2026) —
  Interop 2026: 20 focus areas. Named: enhanced `attr()` (read any HTML attr into any property/type/unit),
  media pseudo-classes (`:playing`/`:paused`/`:buffering`), Navigation API pre-commit handlers, scoped
  custom element registry, cross-document view transitions.
* web.dev/baseline/2026 + monthly digests (Jan/Apr/May 2026) — Baseline Newly Available: `contrast-color()`,
  `@scope` (Firefox 146 joined), `:active-view-transition`, service-worker modules, Array
  copy-transform methods (`toSorted`/`toReversed`/`with`), `field-sizing`.

**RECONCILED — the map is largely FRESH (audits #17-19 held).** Every Interop-2026 focus area and most
Baseline-2026 items were ALREADY on the map: `@scope` (doc, missing, t230), `:active-view-transition` (app),
`attr()`/`shape()`/`contrast-color()` (doc — contrast-color now works, t466), media pseudo-classes
(media, partial), scoped custom registries (app, missing), cross-document view transitions (app),
`field-sizing` (doc, Baseline Jun 2026), `text-wrap: balance/pretty` (doc). `toSorted`/`toReversed`/`with`
are mozjs Array methods — not a gap worth a row.

### ADDED — genuine unknowns (the point of the audit)

* **`::details-content` pseudo-element** (Baseline 2025) → new doc `unknown`. Styles/animates the OPEN
  `<details>` disclosure panel — directly adjacent to this session's t467/t468 details work, and invisible
  to the map. The canonical "animate a disclosure open" idiom pairs it with `@starting-style` +
  `interpolate-size`.
* **`@starting-style`** (Baseline 2025) → new doc `unknown`. The entry-transition primitive: the
  before-open style for popover/dialog/`display:none`→shown animate-in. Widely used now that popover/dialog
  are Baseline; a missing `@starting-style` means the element just pops in with no transition.
* **`scrollbar-color` / `scrollbar-width`** (Baseline 2024) → new doc `unknown`. Dark-mode sites theme the
  scrollbar; unstyled leaves a bright scrollbar on a dark UI. We have `scrollbar-gutter` (t155) but not the
  color/width theming siblings.

### THIS WINDOW'S CAPABILITY (context, not an audit find)

* `<details>` completed on BOTH actuation paths: t467 summary-click exclusive `<details name>` accordions
  (G_DETAILS_ACCORDION), t468 script-set `details.open` fires `toggle` + exclusivity via a contained
  reflection-setter hook (G_DETAILS_OPEN_IDL). Re-probes confirmed dialog Escape/cancel, range/slider
  actuation, DOM Range all already built (stale-PESSIMISTIC again).

### STILL OPEN / EXCLUDED (with reason)

* **`interpolate-size` / `calc-size`** — NOT added: already known via [[calc-size-interpolate-size-segfault]]
  (open Bar-0 SIGSEGV, release-only heisenbug, fix in a fresh ASAN context). On the map by memory.
* **`reading-flow`/`reading-order`, CSS `@function`, `if()`, `sibling-index()`/`sibling-count()`** — surfaced
  as newer/bleeding-edge CSS (Chrome-2025, not yet cross-engine Baseline). Recorded here as CANDIDATES for a
  future pass; not added as rows until they reach Baseline, to keep the map from filling with single-engine
  experiments. `ic`/`ric` units (added audit #19) unchanged.
* Service Worker runtime, WebGL, WebRTC, WebTransport, scroll-driven animations, JPEG XL, zstd — unchanged
  (subsystems, v1-scope deferrals, or below the ROI line).

**What we had been wrong about this pass:** `::details-content` and `@starting-style` — two Baseline-2025
CSS features, one of them directly adjacent to the details work I shipped this very session — were invisible
to the map. No stale-OPTIMISTIC lie found (nothing marked works/gated measured absent).

LAST_SURFACE_AUDIT 458→468; next due 478.

## Audit #21 — tick 478 (2026-07-23)

**Sources (searched live, not from memory):**
* github.com/web-platform-tests/interop/blob/main/2026/README.md — the AUTHORITATIVE Interop 2026 list:
  20 focus areas (container style queries, CSS anchor positioning, `attr()`, `contrast-color()`, CSS zoom,
  custom highlights, dialogs+popovers, fetch uploads+ranges, IndexedDB `getAllRecords()`, JSPI for Wasm,
  media pseudo-classes, Navigation API, scoped custom element registries, scroll-driven animations, scroll
  snap, CSS `shape()`, view transitions, web-compat, WebRTC, WebTransport) + 4 investigations (accessibility
  testing, JPEG XL, mobile testing, WebVTT).
* web.dev/baseline/2025 + digests — Baseline Newly Available 2025: popover, `content-visibility` (Sep 2025),
  `hidden=until-found` + `beforematch`, `::marker` styling, `writing-mode: sideways-rl/lr`, LCP/INP metrics.

**RECONCILED — the map is FRESH; every Interop-2026 focus area maps to an existing row.** Cross-checked all
20: container/style queries (doc, gated t379 — style() queries a noted residue), anchor positioning (doc,
missing t230), `attr()`/`zoom`/`shape()`/`contrast-color()` (doc — contrast-color WORKS t466), custom
highlights (doc, missing t225), dialogs+popovers (both gated), fetch uploads+ranges (platform, gated t228),
IndexedDB + `getAllRecords()` (both gated, t278/t420), JSPI (app, missing t230 — wasm core works), media
pseudo-classes (media, partial t344), Navigation API (app, gated t309), scoped custom element registries
(app, missing t225), scroll-driven animations (app, missing t230), scroll snap (gated), view transitions +
cross-document (app, gated t308 + partial t372/373), WebRTC (platform, out-of-scope), WebTransport (platform,
missing). Investigations: a11y tree (gated), JPEG XL (doc, measured-absent t237), WebVTT (media, gated
t257-259). No stale-OPTIMISTIC lie found (nothing marked works/gated is actually absent).

### ADDED — genuine unknowns (the point of the audit)

* **`content-visibility` / `contain-intrinsic-size`** (Baseline Sep 2025) → new doc `unknown`. `content-visibility:
  auto` skips rendering off-screen subtrees (long docs/feeds use it heavily for scroll perf) and pairs with
  `contain-intrinsic-size` to reserve a placeholder box so scroll height + scrollbar stay stable. Absent from
  the map entirely — a page that relies on it for its intrinsic height would compute a different total page
  height than Chrome (a placement divergence the fidelity sweep would see, not just a perf gap).
* **`hidden=until-found` + `beforematch` event** (Baseline 2025) → new doc `unknown`. Content hidden with
  `hidden="until-found"` is collapsed but find-in-page (and a URL fragment) can REVEAL it, firing `beforematch`
  first. The modern accordion/"read more"/collapsible-FAQ idiom — directly adjacent to the `<details>` (t467/8)
  and find-in-page (partial) work. Absent from the map.

### STILL OPEN / EXCLUDED (with reason)

* **`writing-mode: sideways-rl/sideways-lr`** (Baseline 2025) — NOT added as its own row: vertical/sideways
  text is a layout SUBSYSTEM (the map already implies horizontal-only), not a bounded gap; recorded here as a
  candidate for a future vertical-writing-modes pass, not a single-tick row.
* **`::marker` styling** — folded under existing list-glyph work; not a new row.
* Service Worker runtime, WebGL, WebRTC, WebTransport, scroll-driven animations, JPEG XL — unchanged
  (subsystems, v1-scope deferrals, or below the ROI line). `@starting-style` + `::details-content` (added #20)
  remain `unknown`; `scrollbar-color` (added #20) LANDED gated at t469.

### THIS WINDOW'S CAPABILITY (context, not an audit find)

* contenteditable EDITING subsystem advanced: t476 `execCommand('cut')` (G_EXEC_CUT), t477 KeyModifiers
  plumbing (G_KEY_MODIFIERS — the dispatched KeyboardEvent now carries ctrlKey/shiftKey/altKey/metaKey, so
  Cmd/Ctrl+K command palettes + Shift+Enter composers work), t478 Ctrl+X→cut / Ctrl+C→copy keyboard routing
  (G_KEY_SHORTCUT_CLIPBOARD) — built ON the modifier substrate.

**What we had been wrong about this pass:** two Baseline-2025 features — `content-visibility` (a heavily-used
scroll-perf + intrinsic-height primitive) and `hidden=until-found`/`beforematch` (the modern reveal-on-find
collapsible) — were invisible to the map.

LAST_SURFACE_AUDIT 468→478; next due 488.

## Audit #22 — tick 488 (2026-07-24)

SOURCES (web-checked, not memory): Interop 2026 focus areas (web-platform-tests/interop 2026/README;
hacks.mozilla.org/2026/02/launching-interop-2026; webkit.org/blog/17818; web.dev/blog/interop-2026 —
20 focus + 4 investigation areas). Baseline 2026 digests (web.dev/baseline/2026; web.dev/blog/web-platform-01-2026;
baseline-digest jan–may 2026). Interop-2026 headline set: **Anchor Positioning, advanced `attr()`,
cross-document View Transitions, `:open` pseudo-class, `popover="hint"`, `shape()`, WebTransport/WebRTC.**
Baseline-newly-available 2026: CSS Anchor Positioning (Jan, Firefox 147), `contrast-color()` (WORKS t466),
`:active-view-transition` (Jan), Service Worker JS modules.

RESOLVED BY PROBE (the point of the audit — measure, don't guess):
* **`:open` pseudo-class → WORKS** (was not on the map; an Interop-2026 FOCUS AREA). Stylo already matches it:
  `details:open` → 1 and `:open` → 2 (open `<details>` + open `<dialog>`) on a live probe. Flip: unknown→works.
* **`content-visibility` / `contain-intrinsic-size` → CONFIRMED MISSING** (audit #20 unknown, now measured).
  `getComputedStyle().contentVisibility` is `undefined` and `contain-intrinsic-size` serializes empty — the
  properties are unrecognized (candidate Stylo servo-build drop, the engine="gecko" class; verify before
  building). A long doc/feed relying on it for intrinsic height computes a different total page height than
  Chrome — a PLACEMENT divergence the fidelity sweep sees, not just a perf gap.
* **`hidden="until-found"` → CONFIRMED PARTIAL/BROKEN** (audit #20 unknown, now measured). The attribute
  reflects (`getAttribute('hidden')` == "until-found") but the element RENDERS VISIBLE (offsetHeight 18 — it
  should be collapsed-but-revealable), and `onbeforematch` is absent. A "read more"/collapsible-FAQ/reveal-on-
  find idiom shows its collapsed content prematurely. Bounded first brick available: a UA rule collapsing
  `[hidden="until-found"]` like boolean `[hidden]`; the full feature (find-in-page reveal + `beforematch`) is
  larger.

ADDED as `unknown` (Interop-2026 / Baseline-2026 features absent from the map):
* **advanced `attr()`** (attr() typed, for non-`content` properties) — doc; Interop-2026 focus.
* **`popover="hint"`** — app; popover base is gated, the hint variant is an addition.
* **`:active-view-transition`** pseudo (Jan-2026 Baseline) — app; view transitions gated t308.
* **Service Worker JS modules** (`type:'module'` SW) — app; SW runtime is a v1-scope deferral, note only.
* CSS **Anchor Positioning** — already on the map as missing (t230); Interop-2026 ELEVATES it from the
  constellation "niche-tail cut line #12" — flag the tension, still a subsystem, not a bounded tick.

WHAT WE HAD BEEN WRONG ABOUT: `:open` — a headline Interop-2026 focus area — was invisible to the map yet
ALREADY WORKS (Stylo supports it). That is the seventh-plus time a "modern/unknown" feature was already built;
the standing rule holds harder than ever: RE-PROBE before ranking or building anything the map calls missing.
Also newly visible: the `content-visibility` gap is a PLACEMENT (page-height) divergence, not merely perf.

LAST_SURFACE_AUDIT 478→488; next due 498.

## Audit #23 — tick 498 (2026-07-24)

SOURCES (web-checked, not memory): Interop 2026 focus areas (web.dev/blog/interop-2026; css-tricks.com/interop-2026)
and Baseline monthly digests Feb–May 2026 (web.dev/blog/baseline-digest-{feb,mar,apr,may}-2026; web.dev/baseline/2026).
The Interop-2026 focus set is unchanged from Audit #22 (annual set): Anchor Positioning, View Transitions,
`<dialog>`, Popover API improvements, animation timelines, advanced `attr()`, `:open`, `shape()`. Baseline
newly-available since #22: `font-family: math` (Mar 2026, MathML math-font rendering); `contrast-color()`
(WORKS here, t466); assorted May-2026 CSS/event/API additions.

RESOLVED BY PROBE (measured this tick via `CSS.supports`/live cascade, not guessed):
* **`::details-content` pseudo-element → RECOGNIZED** (map unknown since Audit #20). `CSS.supports('selector(::details-content)')`
  → true: Stylo parses the selector. Unknown→partial — the selector matches; whether it drives the disclosure
  panel's `content-visibility` (the full Baseline-2025 behavior) is unverified and gated on content-visibility,
  which is a servo-drop (below).
* **advanced typed `attr()` → MISSING.** `CSS.supports('width','attr(data-w px)')` → false. The typed `attr()`
  for non-`content` properties (Interop-2026 focus) is unrecognized. A subsystem, not a bounded tick.
* **`:active-view-transition` → MISSING.** `CSS.supports('selector(:active-view-transition)')` → false. View
  Transitions themselves are gated (t308); this Baseline-2026 pseudo is not.
* **`shape()` → MISSING**, **`anchor-name` → MISSING**, **`field-sizing:content` → MISSING** (reconfirms the
  Audit-#22 / t492 findings; all servo-drops or subsystems — Stylo's build does not carry the properties).
* **`popover="hint"` → not specially handled** (`CSS.supports('(popover: hint)')` → false); the popover base is
  gated, the hint variant is an addition on the same top-layer machinery.

RECONCILED FROM THIS SESSION'S PROBES (ticks 489–497, folded into the map): the clean ATOMIC JS-surface /
DOM-method / current-state-getter vein is MINED OUT. NEWLY BUILT this window (unknown/absent → gated): global
`[hidden]` collapse, `inputMode`/`enterKeyHint` reflection, `dialog.requestClose()`, `img.currentSrc`,
`document.activeElement`→`<body>`, `document.hasFocus()`, `textarea.textLength`. CONFIRMED PRESENT+correct
(stale-pessimistic again): the full form-constraint-validation surface, scroll methods, MutationObserver/
IntersectionObserver, getSelection, DataTransfer/DragEvent/Animation+TransitionEvent. CONFIRMED SUBSYSTEMS
(not bounded): CSS Typed OM, Custom Highlight API, `Element.getHTML()` (shadow-serializer), `img.complete`/
`naturalWidth` (image-lifecycle), CSSOM `.sheet` (~944 WPT), the servo-drop CSS-property class.

ADDED as `unknown` (Baseline-2026, absent from the map): **`font-family: math`** (Mar-2026, MathML math-font
selection) — doc-class, but MathML is below the ROI cut line (ENGINEERING.MD Domain D); recorded measured-absent-
by-policy, not a build target.

WHAT WE HAD BEEN WRONG ABOUT: `::details-content` — a map unknown since Audit #20 — is at least selector-recognized
by Stylo, the eighth-plus "modern feature partially/already there." The standing rule holds: RE-PROBE before
ranking or building anything the map calls missing. The honest frontier is unchanged and doubly-confirmed (Const-
Check #28): the sized subsystems in PHASE0-BOUNDED-REMAINDER.md, led by ch/ex real font metrics — NOT more atomic
surface work, which is now measured-exhausted.

LAST_SURFACE_AUDIT 488→498; next due 508.
