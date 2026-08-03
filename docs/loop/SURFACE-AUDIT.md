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

## Audit #24 — tick 508 (2026-07-24)

SOURCES (searched the web, not memory — the platform moved past the training data):
* Interop 2026 focus areas — the authoritative list: https://github.com/web-platform-tests/interop/blob/main/2026/README.md (WebKit https://webkit.org/blog/17818/announcing-interop-2026/ , web.dev https://web.dev/blog/interop-2026 , Mozilla https://hacks.mozilla.org/2026/02/launching-interop-2026/ )
* Baseline May 2026 monthly digest: https://web.dev/blog/baseline-digest-may-2026 (+ Baseline 2026 https://web.dev/baseline/2026 )
* IndexedDB getAllRecords / WebTransport signals: https://caniuse.com/wf-getallrecords , https://developer.mozilla.org/en-US/docs/Web/API/IDBIndex/getAllRecords

RECONCILED against CONSTELLATION.tsv. **The map is remarkably clean against Interop 2026** — all 20
focus areas are ALREADY on the map WITH a verdict: container-style-queries (97), anchor-positioning
(anchorpos:no), attr() (99), contrast-color() (182 gated), zoom (99), custom-highlights (highlights:no),
dialogs+popovers (gated + :open works t488), fetch-uploads+ranges (102 gated), IndexedDB + getAllRecords
(36/37/168 — getAllRecords WORKS, gated t420, and it carries 20% of the Interop score), JSPI (jspi:no),
media-pseudo (105), Navigation-API (navigationapi:yes), scoped-custom-registries (scopedregistry:no),
scroll-driven-anim (scrolldriven:no), scroll-snap (scrollsnap:yes), shape() (99), view-transitions
(93/189), web-compat (106), WebRTC (70 out-of-scope), WebTransport (103 measured-absent, HTTP/3-gated —
a deliberate V1-SCOPE deferral). Investigation areas likewise covered: JPEG XL (101), WebVTT (captions
gated), a11y (a11y tree), mobile (out of scope). No Interop phantom.

WHAT THE MAP WAS MISSING — 4 genuine Baseline-2026 gaps ADDED as `unknown` (the digest, not Interop,
surfaced them; a bigger uglier map is a good tick):
* **CSS `lh`/`rlh` line-height units** (Baseline Widely Available May 2026) — the STANDOUT: a direct
  sibling of the ch/ex/cap font-relative units just built (t499-502), zero hits in engine/css. `lh`
  resolves against the element line-height, `rlh` against the root's. Bounded, likely resolvable off
  the same length-resolution seam. This is the next atomic lever if one is wanted.
* **`:user-invalid`/`:user-valid`** (Baseline May 2026) — :invalid/:valid are built, but these gate on
  user-interaction state (turn red only after the user leaves the field), which every real validated
  form wants. May be a gecko-gated NonTSPseudoClass here.
* **`ToggleEvent.source`** (Baseline Newly Available May 2026) — the invoking element on the toggle/
  beforetoggle event (command-invoker wiring); the events already fire, the property is new.
* **`image-rendering`** (Baseline Newly Available May 2026) — pixelated/crisp-edges scaling filter;
  pixel-art/QR/retro sites blur without it.

WHAT WE HAD BEEN WRONG ABOUT: the map treated the font-relative-unit family as CLOSED after ch/ex/cap
(t499-502) — but `lh`/`rlh` are a Baseline-Widely-Available sibling that was never on the map at all.
The font-metric lever is one unit wider than Const-Check #29 recorded. Otherwise the standing finding
holds and is now triply-confirmed: atomic surface work is measured-near-exhausted; the honest frontier
is the PHASE0-BOUNDED-REMAINDER subsystems — with `lh`/`rlh` as a fresh bounded exception.

LAST_SURFACE_AUDIT 498→508; next due 518.

## Audit #25 — tick 518 (2026-07-24)

SOURCES (searched the web, not memory):
* Interop 2026 focus areas (20 areas + 4 investigations): https://webkit.org/blog/17818/announcing-interop-2026/ , https://web.dev/blog/interop-2026 , https://www.igalia.com/news/interop-2026.html , https://github.com/web-platform-tests/interop/blob/main/2026/README.md
* Baseline 2026 digests: https://web.dev/blog/baseline-digest-may-2026 , https://web.dev/blog/baseline-digest-jan-2026 , https://web.dev/baseline/2026
* Ladybird 2026 progress (independent engine, WPT + test262): https://ladybird.org/newsletter/2026-04-30/ , https://ladybird.org/newsletter/2026-06-30/

RECONCILED against CONSTELLATION.tsv. **The headline is a capability reconciliation, not a discovery.**
The one row that moved is the reason this audit fell where it did:
* **ESM module-graph loading — `partial` → `gated`.** t506's probe named the multi-module `import` GRAPH
  as THE GAP ("module_resolve_hook returns null, only self-contained modules run"). That gap is now
  BUILT and gated on BOTH real page paths, across ticks 512-517 (B1 registry → B2 resolve hook → B3
  population walk → B3b-i consumer → B3b-ii load_async producer → B3b-iii shell producer). Gates
  g_esm_page_graph + g_esm_prefetched_graph. The class (native-ESM / Vite-dev / no-bundler import-graph
  apps) is genuinely unlocked in the agent AND the window. This is one of the PHASE0-BOUNDED-REMAINDER
  subsystems closing — exactly the frontier audit #24 named.

**The map remains remarkably clean against Interop 2026 / Baseline 2026** — nearly every 2026 focus area
is ALREADY on the map WITH a verdict (the stale-pessimistic rule, again): anchor-positioning (98 missing),
advanced attr() (99 partial — the Level-5 typed form, a 2026 ~20%-score item, correctly still open),
IndexedDB getAllRecords() (168 WORKS/gated t420 — one of the 20%-score items, already done), JSPI (96
missing), WebTransport (103 measured-absent, HTTP/3-gated V1 deferral), View Transitions (92 gated / 93
cross-doc partial / 189 :active-view-transition missing), container style-queries (97), test262 (a
standing unknown — Ladybird now passes 52,045/53,207 = 97.8%, reconfirming it as the highest-value
unmeasured JS-conformance item since we embed SpiderMonkey). ric already tracked (ic/ric row). No
Interop phantom.

WHAT THE MAP WAS MISSING — 1 genuine gap ADDED as `unknown`:
* **name-only container queries** (`@container name { }`, Baseline Newly Available May 2026) — our
  @container supplement (t379) parses CONDITIONS; a name-ONLY query (empty condition) may pass through or
  be dropped. Bounded probe, added unknown.

WHAT WE HAD BEEN WRONG ABOUT (two things, both instructive):
1. The **ESM row was stale-pessimistic in the OTHER direction from usual** — not "marked missing but
   actually built," but "marked partial and NOW built by the work the probe scoped." The probe that
   pinned the gap (t506) directly enabled the subsystem that closed it (t512-517). The instrument working
   as designed: probe names the seam, bricks fill it, audit reconciles.
2. A first pass ADDED `IndexedDB getAllRecords()` as a new `unknown` — then the cross-check against
   audit #24 caught it already `works`/gated (t420). Removed the duplicate before commit. The lesson
   stands (audit #24 said it, this audit re-lived it): **grep the map for the capability, including
   near-synonyms, before adding it** — a stale-pessimistic ADD is as much drift as a stale-optimistic row.

STANDING FINDING HOLDS: atomic surface work is measured-near-exhausted; the honest frontier is the
PHASE0-BOUNDED-REMAINDER subsystems, one of which (ESM import graphs) just landed. next due 528.

LAST_SURFACE_AUDIT 508→518; next due 528.

## Audit #26 — tick 528 (2026-07-24)

SOURCES: web.dev Baseline (2024-2026), Interop 2026 focus areas, MDN — checked against
docs/loop/CONSTELLATION.tsv, with special attention to the media surface just built (t521-527).

ALREADY ON THE MAP + GATED (checked, no action): Media Session API (navigator.mediaSession — OS/lock-
screen controls), field-sizing:content (auto-grow textarea), computed CSS custom properties, @property/
registered custom properties. The stale-pessimistic rule again: several recent-Baseline features are
already built and gated.

ADDED (world names them, our map did not — now `unknown`, which is the GOOD kind of map growth; the
invariant is MEASURED, not unknown):
1. **requestVideoFrameCallback** (media) — frame-accurate `<video>`; the callback-per-presented-frame
   API frame-synced overlays and web video editors bind. Directly adjacent to the t521-524 playback
   model, which is exactly why the audit caught it now: building a surface reveals its neighbours.
2. **Promise.withResolvers** (app/JS) — Baseline 2024; likely already works via SpiderMonkey, unmeasured.
3. **Set methods** (union/intersection/difference/isSubsetOf) (app/JS) — Baseline 2024; likely works,
   unmeasured.
4. **scheduler.postTask / scheduler.yield** (app) — Baseline 2024/2025; may be genuinely absent (needs a
   real priority queue, not a setTimeout alias) — framework schedulers feature-detect it.

WHAT WE HAD BEEN WRONG ABOUT: nothing large — the map is in good shape post-media-arc (every Interop-2026
focus area already has a verdict, as Audit #25 also found). The blind spot this audit closes is the
MEDIA NEIGHBOURHOOD: having just built the `<video>` clock/seek/played/durationchange surface, the map
did not name rVFC — the sibling frame-callback API — which a real advanced player uses alongside exactly
the events we built. Building a subsystem is itself a map-discovery act, and the audit is where that gets
recorded rather than lost.

RE-RANK: none of the 4 is larger than the standing frontier (the fidelity-instrument rebuild / media XL
work). They are bounded probes/small builds for a later tick; the Check #32 pivot (fidelity instrument or
test262) stands. LAST_SURFACE_AUDIT set to 528.

## Audit #27 — tick 538 (2026-07-24)

SOURCES: Interop 2026 focus-area README (web-platform-tests/interop, the authoritative list) enumerated in
FULL, plus web.dev Baseline 2024-2026 and MDN — checked against docs/loop/CONSTELLATION.tsv. Audit #26 (same
calendar day, tick 528) focused on the just-built media neighbourhood; this audit does the thing #26 did not:
walk EVERY Interop 2026 line and confirm each has a verdict, rather than trusting a general "map looks good".

INTEROP 2026 — ALL 20 FOCUS AREAS + 4 INVESTIGATIONS RECONCILED, EVERY ONE ALREADY ON THE MAP WITH A VERDICT:
  Container style queries → gated (t379) + name-only unknown row · CSS anchor positioning → missing (t230) ·
  CSS attr() → partial (t409) · contrast-color() → partial (pref-flip) · CSS zoom → partial · Custom highlights
  → missing (t225) · Dialogs & popovers → gated · Fetch uploads & ranges → gated (G_MEDIA_SEGMENT_FETCH t228,
  ReadableStream body works row 134) · IndexedDB → gated (t278/t329) · JSPI for Wasm → missing (t230; wasm CORE
  gated) · Media pseudo-classes → partial (t344) · Navigation API → gated (t309) · Scoped custom element
  registries → missing (t225) · Scroll-driven animations → missing (t230) · Scroll snap → works · CSS shape()
  → missing · View transitions → gated (t308) · WebRTC → missing (out of scope) · WebTransport → missing
  (deferred, needs HTTP/3) · Web compat → n/a. INVESTIGATIONS: Accessibility testing → a11y roles gated ·
  JPEG XL → missing (t237, below ROI) · WebVTT → gated (t255-261) · Mobile testing → n/a.
  CONCLUSION: the map is COMPLETE against Interop 2026 — the "stale-pessimistic / map in good shape" finding
  of #25 and #26 holds, now proven line-by-line rather than asserted.

ADDED (world names them, our map did not — now `unknown`, the GOOD kind of growth; MEASURED, not unknown, is
the invariant):
1. **light-dark() CSS color function** (doc) — Baseline 2024, automatic dark mode. The color-scheme PROPERTY
   landed via a Stylo servo_pref flip (t464), but the CONSUMING function light-dark() is unrowed/unmeasured —
   it may parse-drop (gecko fence, à la @starting-style) or resolve. A site authored light-dark() with no
   @media fallback renders the wrong palette if we drop it. Bounded probe queued.
2. **CSS Level-4 math functions round()/mod()/rem()/sign()/abs()** (doc) — Baseline 2024. calc()/min()/max()/
   clamp() are exercised by css-values WPT, but the stepped/sign math is unrowed and unmeasured; an unsupported
   function invalidates the whole declaration. Bounded probe queued.

STALE-PESSIMISTIC RULE PAID AGAIN (the #24/#26 lesson, re-lived): first-pass alternation greps for WASM / AVIF /
scoped-registries came back EMPTY and nearly minted duplicate `unknown` rows — a re-grep with proper terms found
all three ALREADY on the map (WebAssembly gated, WebAssembly GC gated, AVIF gated, scoped registries missing).
GREP THE MAP WITH CLEAN SINGLE TERMS before adding — a stale-pessimistic ADD is as much drift as a stale-optimistic
row. Both survivors (light-dark, Level-4 math) were confirmed absent by clean single-term grep.

RE-RANK: neither add is larger than the standing frontier (the fidelity-instrument rebuild, CO-#1, mid-execution
— brick 4b landed t537). They are bounded probes/small builds for a later tick. The Check #33 steer stands:
continue the rebuild (next: §3b root-cause clustering, then the coverage→SHAPE gate-floor flip).

LAST_SURFACE_AUDIT set to 538; next due 548.

## Audit #28 — tick 548 (2026-07-25)

SOURCES (searched, not recalled — the platform moved and my training data did not):
- Interop 2026 focus areas, read from the authoritative list:
  https://github.com/web-platform-tests/interop/blob/main/2026/README.md — **20 focus areas**:
  container style queries · CSS anchor positioning · CSS attr() · CSS contrast-color() · CSS zoom ·
  custom highlights · dialogs and popovers · fetch uploads and ranges · IndexedDB · JSPI for Wasm ·
  media pseudo-classes · Navigation API · scoped custom element registries · scroll-driven animations ·
  scroll snap · CSS shape() · view transitions · web compat · WebRTC · WebTransport. **4
  investigations**: accessibility testing · JPEG XL · mobile testing · WebVTT.
- https://hacks.mozilla.org/2026/02/launching-interop-2026/ · https://webkit.org/blog/17818/announcing-interop-2026/
  · https://web.dev/blog/interop-2026 · https://www.igalia.com/news/interop-2026.html
- Ladybird newsletters (an independent engine, walking this same road):
  https://ladybird.org/newsletter/2026-04-30/ · .../2026-05-31/ · .../2026-06-30/
- Baseline 2025/2026: https://web.dev/baseline/2025

RECONCILED, one line per Interop-2026 area: **19 of 20 were already on the map** (web compat is not a
feature). Present and gated/works: container queries incl. style queries (97), dialogs+popovers,
fetch uploads and ranges (102), IndexedDB (36/37/168), media pseudo-classes, Navigation API (94),
scroll snap, view transitions (92) + cross-document VT (93), WebVTT. Present and correctly NAMED as
death-tail/out-of-scope: anchor positioning, custom highlights, JSPI, scoped registries, scroll-driven
animations, WebRTC, WebTransport, JPEG XL. **The stale-pessimistic rule paid AGAIN** — every clean
single-term grep found the row already there; nothing needed minting from scratch. That is the fifth
consecutive audit where the first instinct ("this can't be on the map") was wrong.

WHAT WE WERE WRONG ABOUT — and it is a MAP defect, not a capability one. Row 99 read
**`CSS attr() / zoom / shape() / contrast-color()`** and carried **ONE `partial`** for **FOUR
capabilities that Interop 2026 names SEPARATELY.** Three failure modes in one cell:
1. **It counts as ONE measured capability when it is four** — so the MEASURED ratchet invariant, the
   one number that is supposed to make discovery rewarded and rot punished, was being satisfied at a
   quarter price here.
2. **Its single verdict hides which of the four work.** The note said so honestly in prose
   ("STILL MISSING: … typed attr(), per-element zoom, shape(), contrast-color()") — which means the
   information existed and the *scoreable* field contradicted it. A cell whose note disagrees with its
   status is a cell nobody can act on.
3. **It made three unmeasured things inherit a `partial`.** Inherited-partial is the exact
   stale-optimistic shape this ledger exists to catch, and it is worse than `unknown`, because
   `unknown` gets probed and `partial` gets skipped.

ADDED (decomposed, all `unknown` on purpose — an inherited verdict is not a measurement):
- **CSS zoom (per-element property)** — t409's note said "only full-page zoom infra exists", which is a
  note, not a measurement.
- **CSS shape() function** — never probed on its own. Behavioural probe required, not `CSS.supports`:
  a parse-only yes is the trap t543 caught on light-dark().
- **CSS contrast-color()** — likely **stale-PESSIMISTIC**: a `servo_pref` for it was flipped in the
  t464–466 arc. Re-probe before building, and measure the RESOLUTION half separately from parse
  (t543/t544 precedent: a `partial` is not one verdict, each half is its own fact).
Row 99 keeps `partial` and is now **only** typed `attr()`.

CORRECTED: nothing stale-optimistic found this window. Map 205 → **208 capabilities**; MEASURED count
unchanged (the three adds are new `unknown`s, not downgrades), so the ratchet sees discovery, not rot —
exactly as the audit's own instructions promise.

RE-RANK: **no.** The three new unknowns are cheap bounded probes, and the standing CO-#1 is the
observer's STEP-1 exit verification, which is mid-execution: test262 ran at t546 (94.14% of 87,009
executed / 81.41% honest), the certificate became computable at t547, and the full-corpus sweep is the
item in flight. The probes reopen the cheap-probe vein that t543's journal declared mined out — a
genuine, small result of leaving the frame — but they queue behind the sweep.

SWEEP NOTE (measured during this tick, and it re-ranks nothing but it is the biggest thing seen this
window): the first corpus chunk gives **nytimes.com structural 0.0% — 2,406 of 2,407 elements MISSING**,
and **manuk 28.8s vs chromium 6.1s** on it (load budget exhausted + the 20,000-task event-loop ceiling
hit). theguardian.com then ran past 3.5 minutes without completing. So the corpus read is going to be
expensive AND informative, which is the argument for running it rather than reasoning about it.

LAST_SURFACE_AUDIT set to 548; next due 558.

## Audit #29 — tick 558 (2026-07-25)

SOURCES (searched, not recalled — #28 read Interop 2026's *priorities*, so this one deliberately read what
actually SHIPPED, which is a different question and found different gaps):
- web.dev Baseline monthly digests, Jan–Jun 2026: https://web.dev/blog/baseline-digest-jan-2026 ·
  .../baseline-digest-mar-2026 · .../baseline-digest-apr-2026 · .../baseline-digest-may-2026
- web.dev "New to the web platform" Jan–Jun 2026: https://web.dev/blog/web-platform-01-2026 ·
  .../web-platform-02-2026 · .../web-platform-04-2026 · .../web-platform-05-2026 · .../web-platform-06-2026
- https://web.dev/baseline/2026

ADDED — **nine rows, all previously absent from the map.** Two are the kind of gap this instrument exists to
find:
- **`WebGPU` was not on the map at all** — only WebGL was. WebGPU reached Baseline across
  Chrome/Firefox/Safari/Edge during 2026, so the map was tracking the *predecessor* of a shipped
  cross-engine capability and had no row for the successor. It is XL and squarely in the observer's
  DEATH-TAIL (feature-detect, name as a post-Phase-0 exception, do NOT build) — recorded `missing` so the
  exception is **visible and counted** rather than absent. An unlisted exception is indistinguishable from
  an oversight.
- **`<search>` element** (Baseline Apr 2026) carries an *implicit ARIA landmark* `role=search`. Per
  CONSTITUTION VI.1 `manuk-a11y` is already load-bearing for the agent observation channel, so an unmapped
  landmark role is an **agentic** gap, not merely a rendering one. That is a class of miss the CSS-shaped
  audits keep walking past.
Also added: **multi-keyword `display: inline flex`** (a parse failure here does not degrade — the
declaration is DROPPED and the box falls back to inherited/initial display, i.e. a layout collapse, so it
must be probed behaviourally rather than with `CSS.supports`), **`animation-composition`**,
**`text-justify`** (adjacent to the t557 text-metrics work — measure it *after* the advance follows the
resolved face), **multicol Level 2 `column-wrap`/`column-height`** (kept as its own row so a Level-1 fix
cannot be silently credited with Level 2), **CloseWatcher** (the actuation surface an agent uses to dismiss
an overlay, so it belongs to the agentic thread too), **Reporting API**, and **Web Serial** (out of scope,
named so the cut is deliberate and visible).

WHAT WE WERE WRONG ABOUT: **#28 asked the wrong question and I did not notice.** It reconciled against
Interop 2026 — the vendors' *priority list* — concluded "19 of 20 already on the map", and read that as the
map being in good shape. Interop is what the vendors agreed to FIX; it is by construction a list of things
already known and partly implemented. It says nothing about what SHIPPED, and shipped-and-Baseline is what
real sites start using. Asking the shipping question instead produced **nine** absent rows in one pass,
including a cross-engine Baseline capability with no row at all. **A reconciliation is only as wide as the
source it reconciles against, and one source is not a survey** — rotate the source, not just the date.

CORRECTED: nothing stale-optimistic this window. Map 208 → **218 capabilities** (t548's nine adds plus this
one's — see the ratchet note below); MEASURED count unchanged, so the ratchet reads discovery, not rot,
exactly as the audit's instructions promise. The stale-pessimistic rule paid again on the second pass: a
first grep for `<search>` / `multicol` / `justify` came back looking empty or misleading (`search` matched
`url.searchParams`, `multicol` matched the Level-1 row) — **grep with clean single terms and READ the hit
before minting a row**, which is the same discipline #27 recorded.

RE-RANK: **no.** None of the nine outranks the live thread. t557 fixed named-font resolution and t558 made
the advance follow it (SHAPE on the probe page 36.4% → 90.9%), which is the largest measured fidelity move
of the session and it is mid-arc. The nine adds are bounded probes for later ticks, and two of them
(`<search>`, CloseWatcher) should be taken together with the agentic thread rather than as CSS rows.

LAST_SURFACE_AUDIT set to 558; next due 568.

## Audit #30 — tick 568 (2026-07-25)

**This audit is deliberately NOT another source-rotation — it is the one that closes the loop on #29.** #28
read Interop 2026 (priorities) and found ~nothing; #29 read what SHIPPED (Baseline digests) and found nine
absent rows. The obvious #30 move is a third source. But an audit that only ever ADDS is half an instrument:
its own instructions say *"RE-RANK. A newly-discovered capability may be larger than everything already on the
list"* — and #29's nine rows had sat unprobed for ten ticks while Constitution Checks #35, #36 and #37 each
flagged the same two of them as an overdue I3 queue. **A map that grows faster than it is measured is drifting
in a new direction, not being corrected.** So this audit measures what the last one added.

MEASURED, two rows, both from #29:
- **`<search>` element → `works`.** `Role::Search` already existed for the explicit `role="search"` ATTRIBUTE;
  the ELEMENT was missing from the tag→role map and fell through to `Role::Generic`. One arm, RED-proven
  (`the_search_element_is_a_search_landmark`; removing it yields `Some(Generic)`), with the explicit attribute
  and the neighbouring landmarks asserted untouched. **It is an AGENTIC fix**: per CONSTITUTION VI.1 the a11y
  tree already feeds `manuk-agent`'s observation channel, so on any site that adopted the wrapper the agent
  could not find "the search box" **by role** at all.
- **`CloseWatcher` → `missing` (measured, not assumed).** Absent from `engine/js` entirely (grep clean), so
  `typeof CloseWatcher` is `undefined` and a feature-detecting page takes its fallback — the honest outcome, not
  a break. Pinned `missing` rather than left `unknown` because **a measured absence outranks an untested one**
  (#24/#26). Scoped for when it is built: it is the **actuation** surface an agent uses to dismiss an overlay
  (Esc / Android back / a close request), so it belongs with the agentic thread beside dialog and popover, which
  are already gated — not with the CSS rows it was filed among.

WHAT WE WERE WRONG ABOUT: **the audit's own cadence had become additive-only.** Three consecutive constitution
checks named the same queue and three consecutive windows of "more legible" rendering work crowded it out — and
the surface audit, whose job includes re-ranking, kept widening the map instead of noticing that two of its own
rows were the highest-ranked unmeasured items on it. Recorded as a process defect against this instrument, not
as a scheduling accident. **The rule going forward: an audit that added rows last cycle MEASURES some of them
this cycle before it adds more.**

CORRECTED: map stays **218** capabilities (no adds this cycle, by design); MEASURED **+1 works, +1 missing**, so
two `unknown`s became verdicts and the ratchet reads real progress rather than growth. RE-RANK: the remaining
seven #29 rows (multi-keyword `display`, `animation-composition`, `text-justify`, multicol L2, Reporting API,
WebGPU, Web Serial) stay below the live leads — implied grid track sizing (t566 root cause, RED proof committed)
outranks all of them.

LAST_SURFACE_AUDIT set to 568; next due 578.

## Audit #31 — tick 578

SOURCES (deliberately ROTATED — #28 used Interop, #29/#30 used Baseline/web.dev, and #30's own lesson was
"rotate the SOURCE not just the date"):
- Ladybird monthly newsletters, Feb–Jun 2026 — https://ladybird.org/newsletter/2026-06-30/ (and 05-31, 04-30,
  03-31, 02-28)
- Servo blog — https://servo.org/blog/2026/06/30/may-in-servo/ , https://servo.org/blog/2026/04/30/march-in-servo/
- WPT top-level directory list, via the GitHub contents API — https://api.github.com/repos/web-platform-tests/wpt/contents/
  (wpt.fyi itself is JS-rendered and returns nothing to a fetcher, which is worth recording: **the obvious URL
  for "what does the platform contain" is unreadable by a crawler**)
- https://web.dev/blog/web-platform-06-2026 , https://web.dev/baseline/2026 — cross-check only
- **And the source no previous audit ever consulted: `engine/page/tests/`, our own gate corpus.**

### THE FINDING, and it is about the instrument rather than the platform

**281 page-gate files exist. 147 of them — 52% — are not referenced anywhere in `CONSTELLATION.tsv`.**
Verified mechanically, not taken on report. The map is therefore **not a map of the engine**; it is a map of
the loop's recent attention, and every readiness figure derived from it has been reading curation.

The shape is unmistakable once seen: the map carried rows for `subgrid`, `scroll-driven animations`,
`animation-composition` and `text-justify` — and **no row at all for CSS Grid, Flexbox, CSS transforms, CSS
transitions/`@keyframes`, `position: sticky`, `:has()`, `@layer`, dark mode, WebCrypto, PerformanceObserver,
contenteditable editing, focus management, or `inert`.** Every one of those is built, and almost every one is
RED-proven by a dedicated gate. **The frontier was curated and the substrate was invisible.**

Audit #45's Web Locks row filed this failure mode as a curiosity — *"inverse stale-pessimism: a green
capability invisible to the map"*. It was not a curiosity. **It was the median case.**

ADDED: **59 rows** — 24 banked directly as `gated` (built, gate file verified present, no row); 27 as
`missing` (grep of `engine/ shell/ agent/ store/` returns **zero** — a measured absence, not an untested one);
8 as `unknown` where the grep was ambiguous and honesty demands a probe. Highlights of the `missing` set the
world names and we did not: Web Audio, XPath (**agentic** — it is the lingua franca of Playwright-style
locators), Referrer Policy, HSTS/mixed-content, Permissions Policy, Trusted Types, Storage Access API, Client
Hints *headers*, bfcache/`persisted`, `rel=preload`, `mediaCapabilities.decodingInfo()`, MediaRecorder/
getUserMedia (**distinct from the out-of-scope WebRTC row**, and conflating them cuts a reachable capability by
accident), SharedArrayBuffer/COOP-COEP, Cookie Store, text-fragment navigation `#:~:text=` (the first thing a
user does after a search), Touch Events, `Intl.Segmenter`, the five UA pseudo-elements, WebDriver, and WPT's
new `ai` directory — which sits directly beside row 163 (`navigator.modelContext`/WebMCP), the agentic thread's
other half, unmapped.

CORRECTED:
- **Row 107 WebAuthn `missing` → `gated`.** `PublicKeyCredential`/`navigator.credentials` shipped at t484–485
  with `g_webauthn_surface.rs`. The map was wrong for ~94 ticks. Scope stated honestly on the row: it
  feature-detects and returns an honest `NotAllowedError`; a full round-trip is **not** claimed.
- **Row 102 vs row 185 — a direct CONTRADICTION**: the same capability (`contrast-color()`) carried `unknown`
  on one row and `gated` with a named receipt on the other. A map that gives one capability two verdicts is
  worse than a map that gives it the wrong one. Status corrected; both rows annotated for merge.
- **Rows 178≡188 and 183≡186** are semantic duplicates, annotated. The distinct capability count is ~3% below
  the raw row count, so audit #30's banked "map stays 218" was measuring a slightly inflated number.
- **Row 4's framing is the structural error**: "box model / floats / block layout" was the map's *only* layout
  row, which implied our layout story stopped in 1998.
- RE-RANK: **anchor positioning** (row 98) rests on a ~340-tick-old probe from when it was Chrome-only — it
  reached Baseline in January 2026 and Ladybird shipped it in April/June; it now outranks most `unknown` CSS
  rows. **`color-mix()`/`oklch`** is filed `unknown` rather than `missing` on purpose: zero occurrences in
  `engine/`, but Stylo is a *dependency* and may resolve them without us naming them — **a grep is not a
  measurement when the capability lives below you**. Tailwind v4's default palette is `oklch` and its opacity
  modifiers compile to `color-mix()`, so this is the largest unresolved question on the CSS list.

WHAT WE WERE WRONG ABOUT: **the audit had only ever looked outward.** Its whole design assumed the unknown
unknowns live in the world — in what standards bodies prioritise and what browsers newly ship — so it spent
five cycles curating the frontier while half the engine's own proven capabilities never reached the map. The
cheapest and most authoritative source available to it was sitting in the repository the entire time, and no
audit had ever run `ls engine/page/tests/ | grep -f` against the file. #30 corrected the cadence to
*"measure some of what you added before adding more"*; **#31's correction is one level under that: before
looking outward at all, diff the map against the receipts you already hold.** An instrument that cannot see
what its own project has built is not measuring the project.

STANDING RULE ADDED: **every surface audit begins by diffing the gate corpus against
`CONSTELLATION.tsv`.** Web research comes second. The unmapped-gate count is now a number to drive to zero,
and it starts at **147 of 281**.

> ⚠ **AMENDED at #32:** the rule as first written scanned only `engine/page/tests/`, and gates live in
> **seven** directories. Widened to `engine/*/tests`, `tests/wpt/tests` and `shell/tests` — see #32.

LAST_SURFACE_AUDIT set to 578; next due 588.

### Audit #31 follow-up — tick 581: the unmapped-gate count is ZERO

#31's standing rule was *"every surface audit begins by diffing `engine/page/tests/` against
`CONSTELLATION.tsv`"*, and it left a number to drive down: **147 of 281 gate files unreferenced.** t578
took it to 109 by adding the substrate rows. This closes it.

**147 → 109 → 0 of 283.** Every gate file the repository contains is now named by a row.

ADDED **47 rows** covering 86 gates, plus **14 backfill edits** to existing rows whose `gate` cell was `-`,
lowercase, abbreviated, or named the weaker of two available gates. Four rows changed status on the
evidence: rows 59 (WebSocket), 65 (clipboard), 137 (fetch streaming) and 129 (device identity) each read
`works` while a dedicated gate existed — and row 129 *already named one in its own gate cell*, which the
map's vocabulary directly contradicts. `works` is what you write when nothing proves it; `gated` is what a
gate earns.

**The map now reads: 211 gated · 55 missing · 23 partial · 19 works · 15 unknown · 1 measured.** For the
first time that distribution describes the *engine* rather than the loop's recent attention, so it is the
first honest input the Phase-0 readiness figure has ever had.

TWO CONTRADICTIONS SURFACED BY THE SWEEP, recorded rather than quietly resolved:
- **`img.currentSrc`'s gate header says we do NOT do `srcset`/`<picture>` candidate selection**, while the
  responsive-images row reads `works`. One of the two is wrong; both are now visible, and the row carries a
  ⚠ CHECK. This is exactly the shape audit #31 found for `contrast-color()` — the same capability with two
  verdicts — and it is the second instance, which suggests looking for more.
- **`g_fetch_stream`'s "not claimed here" scope note is stale** now that `G_FETCH_STREAM_INCREMENTAL`
  exists. An honest-limit comment that outlived its limit is the t576 rot in a doc comment rather than an
  assertion.

METHOD NOTE: every one of the 86 gate references was checked against a real file before the rows were
written, and two were wrong — `VTT_CAPTIONS` and the A/V-sync gates live in `engine/media/tests/`, not
`engine/page/tests/`. **A gate name is a claim like any other**; the check cost one command and caught two
errors in a batch that had already been reviewed.

## Audit #32 — tick 588

### The standing rule caught the audit's own blind spot, one level out

#31 added the rule *"every surface audit begins by diffing the gate corpus against the map"*, and #32 ran
it first as required. It found **one** unmapped gate — `g_storage_patchable`, landed by the previous tick,
which is the rule working exactly as intended.

Then the sharper question: **is `engine/page/tests/` actually the gate corpus?** It is not. Gates live in
**seven** directories, and the rule as written could see only one of them:

```text
engine/page/tests  285      engine/media/tests  10     engine/text/tests   3
engine/css/tests     1      engine/html/tests    1     tests/wpt/tests     1      shell/tests  1
```

Widening the diff to all of them found **7 unmapped of 302**, not 1 of 285. **The rule created to fix the
map's blind spot had the same blind spot one level out** — it assumed the gate corpus was the directory
that happened to hold most of it, which is precisely #31's finding ("the map tracked the loop's recent
attention") repeated in the instrument built to cure it.

ADDED **6 rows** covering all 7 files, and the ones outside `engine/page/tests` are not marginal:
**`G_VIDEO_PLAYER`** (media step M6 — the *join* that owns demux, decode, timeline and A/V sync together,
without which five green gates could each demonstrate a step of playback while nothing could play),
**`G_TEARDOWN`** (no exit path bypasses `Drop` — the gate standing between a teardown-crash workaround and
silently losing the user's session), the **CSS and HTML fuzz** harnesses (Bar-0: a parser panic on the first
untrusted bytes the web sends), **`G_CAP_TOUCH_PROBE`** (the certificate's FUNCTION producer), and
**`G_STORAGE_PATCHABLE`**. One row is honestly `unknown`: `probe_script_fallback.rs` is a *probe, not a
gate*, and its own header says why — the fallback machinery is present, whether it fires is a different
question, and this project has been wrong about exactly that four times.

**Unmapped-gate count: 7 → 0 of 302.** The rule is amended above so the next audit scans all seven
directories.

**The transferable half:** a rule that says "diff X against Y" is only as good as its definition of X, and
the cheapest way to be wrong is to define X as *where you last looked*. `manuk-page` cannot depend on
`manuk-wpt` — the instrument depends on the engine, never the reverse — so the certificate's own gates
*must* live outside `engine/`, and any rule that assumes otherwise will keep missing them structurally.

### The outward half — usage counters, and the map had been ranking by the wrong thing

SOURCES (rotated again — #28 Interop, #29/#30 Baseline, #31 Ladybird/Servo/WPT, so #32 goes to *what real
pages actually use*):
- **`https://chromestatus.com/data/csspopularity`** and **`https://chromestatus.com/data/featurepopularity`**
  — the raw Blink use-counter dumps, snapshot **2026-07-24**: 948 CSS properties and 4,030 feature counters,
  each as a **% of page loads**. Pulled and parsed rather than read off a summary page.
- `https://almanac.httparchive.org/en/2025/capabilities` (the 2025 Almanac has **no** CSS, JavaScript or
  Markup chapter — those URLs 404, so this angle yielded only Capabilities).
- `https://webtransitions.org/servo-readiness/` (aggregate only, names no features — low yield, said so).
- `https://wiki.mozilla.org/Compatibility/System_Addon/Interventions` (categories only; the concrete
  injections data 404'd — **this angle largely failed and was not padded**).

ADDED **20 rows**, every one carrying a real page-load percentage. The headline is not any single row, it is
the shape of what was missing:

| unmapped capability | % of page loads | and it is |
|---|---|---|
| `appearance: none` | **49.3% / 60.5%** | **not implemented** — no `clone_appearance`, no ComputedStyle field |
| `filter` / `backdrop-filter` | **51.9% / 34.3%** | **not implemented** |
| `font-display` | **51.8%** | absent |
| `unicode-range` | **45.1%** | absent |
| `clip-path` | **43.8%** | **not implemented** |
| `-webkit-box` legacy flexbox | **29.9%** | absent — the highest-usage *layout mode* with no row |

**Four of those were verified UNIMPLEMENTED, not merely unmapped.** `stylo_map.rs` — the bridge deciding
which Stylo-computed properties we actually read — has **zero** entries for `filter`, `clip-path`,
`appearance`, `mix-blend-mode` and `writing-mode`. Stylo computes them correctly and **we throw the result
away.** Checked directly rather than taken on report.

CORRECTED:
- **`images: PNG/JPEG/GIF/WebP` `works` → `partial`.** `image::load_from_memory` at three call sites returns
  **one frame**, and `AnimationDecoder`/`into_frames`/`frame_count` appear nowhere in the repo — so animated
  GIF, animated WebP and APNG all render a still. Verified directly. **A `works` can be true of the FORMAT
  and false of the USAGE.**
- **`Touch Events`** was framed as *"ontouchstart feature detection"*. `PassiveTouchEventListener` is
  **66.47%** of page loads. The risk was never detection; it is `TouchEvent`/`Touch`/`TouchList` being
  absent when a carousel library constructs or `instanceof`-checks one.
- **RE-RANKED UP:** WebGL (**~28%** of page loads — the largest `missing` we carry, filed as a peer of
  WebRTC and EME), Referrer Policy (**65.7%**), Client Hints headers (**32%**, and a self-inconsistency
  since we ship the JS half), Compression Streams (the Almanac's **#1 adopted** capability), and
  Subresource Integrity (**22%**, and ignoring it means *executing scripts Chrome would block* — a security
  divergence the fidelity oracle is structurally blind to).
- **RE-RANK CONFIRMED LOW, recorded deliberately:** built-in AI / Prompt API at **0.08%**, Summarizer 0.13%.
  *Confirming a low rank is as much a result as raising one*, and #31 added those rows partly on novelty.
- **`interpolate-size` is 8.16% of page loads** — one in twelve — and this project carries an open,
  unresolved release-only **SIGSEGV** under `calc-size`/`interpolate-size`. That is a **Bar-0 crash sitting
  under a feature we had implicitly treated as exotic.**

WHAT WE WERE WRONG ABOUT: **we had been ranking by standards-body attention while believing we were ranking
by usage** — the same trap as #28–#30, entered from the opposite side. The map carries detailed, correct,
well-gated rows for `Temporal`, `CloseWatcher`, `Node.moveBefore`, `scheduler.postTask`, `light-dark()` and
`contrast-color()`, and carried **nothing at all** for `filter`, `appearance`, `clip-path`, `unicode-range`
or `font-display`. Reading Interop and Baseline as if they were usage data is what produced that: they
report what vendors are *working on*, which is systematically the opposite of what is already everywhere.
The unifying lesson is the one the font-family and `property_at(i)` ticks taught in another register —
**a capability's name is not its shape.** "GIF" names a decoder; the web uses it as a video codec. "Touch
Events" names a feature detect; the web uses it as two-thirds of all event registration.

STANDING RULE ADDED: **rank by measured page-load usage, not by where the capability sits on a standards
roadmap.** The Blink use-counter dumps are two URLs and parse in seconds; there is no excuse for the map's
implicit ranking to disagree with them again.

LAST_SURFACE_AUDIT set to 588; next due 598.

## Audit #33 — tick 598

**METHOD, and the first two numbers it produced were both wrong.** Per the standing rule (t581), the
audit opens by diffing `engine/page/tests/` against `CONSTELLATION.tsv`. The first pass said **27 of
293 gate files are unreferenced by the map**. The second, checking whole rows rather than the gate
column, said **18**. The true answer is **2**.

Both earlier figures were artifacts of **my own matcher**: it upper-cased gate names and grepped for
`G_[A-Z0-9_]+`, while the map cites plenty of gates in lowercase (`g_canvas_text`, `g_a11y_roles`).
Recorded because it is the loop's own recurring lesson pointed at the loop's own instrument: **when a
measurement produces an alarming number, the instrument is a suspect before the subject is.** Acting
on 27 would have meant "fixing" two dozen rows that were never broken.

**FINDING 1 — the t581 rule HELD. Gate/map coverage is 291 of 293.** The two exceptions:
- `engine/page/tests/webfont_live.rs` does not follow the `g_*.rs` convention, so a name-based audit
  cannot see it at all. Left as-is (renaming a gate file risks the wall, which is observer-owned) and
  recorded here so the next audit does not re-derive it.
- `g_scroll_anchor` is unnamed in the map, though `g_scroll_anchor_live` is cited by the same row.

**FINDING 2 — THE HEADLINE: the board's CO-#1 list is substantially stale, and the loop is being
steered at work that is already done.** Checked each letter against the map and, where the map's own
claim needed confirming, by *running the gate*:

| board CO-#1 | board says | actual |
|---|---|---|
| **(A) MEDIA/YouTube** | "5% — biggest gap; build MSE → symphonia → cpal → …" | MSE **gated**, container demux **gated**, audio output **gated**, `<video>` playback + A/V sync **gated**, WebVTT **gated**. Only decode *breadth* is `partial`; EME is deliberately out of scope. (The observer already said this at t264 — the PHASE MANDATE text was never updated.) |
| **(B) OAUTH** | "O1 redirect flow → … → O5 FedCM" | redirect flow **gated**, popup + `postMessage` **gated**, cross-frame `postMessage` **gated**. Only FedCM is `unknown`. |
| **(C) canvas fillText** | "HIGH-LEVERAGE: wire the existing swash raster to the 2D ctx" | **DONE and gated** — `g_canvas_text` passes, asserting real ink, ink colour, transparent surround, per-glyph widths, `textAlign`/`textBaseline`. Verified by running it this tick, not by reading the map. |
| **(D) probe the unknowns** | "~35 unknowns" | **17** remain, listed below. Genuinely open, and now the only letter that is. |

`scripts/lever-board.sh` is observer-owned and was not touched. This is the report.

The 17 remaining unknowns: CSS `zoom`, `shape()`, multi-keyword `display`, `animation-composition`,
`text-justify`, multicol L2, Reporting API, `X-Frame-Options`/CSP `frame-ancestors`, Subresource
Integrity, File System Access + OPFS, `window.screen` + Screen Orientation, FedCM + Digital
Credentials, SVG filters/patterns/SMIL, `@counter-style`, the 2026 CSS frontier bundle, per-glyph font
fallback across scripts, Speculation Rules + `document.prerendering`.

**FINDING 3 — THE MAP WAS NOT MACHINE-READABLE, and one landed capability had been invisible for nine
ticks.** Three integrity defects, none of them visible by reading the file:
1. **Two rows joined by a missing newline** → an 11-field row. The second half was the tick-587
   `G_STORAGE_PATCHABLE` row — a landed capability with a full receipt, **unreadable by every
   column-based consumer** (the lever board, `phase0-progress.sh`, `--gaps`, this audit) since t587.
   It also lost its `class` column in the join; restored to `storage`.
2. A **stray blank row**, which shifts every line-number reference into the map.
3. A row whose status read **`measured`** — not one of the five values anything downstream
   understands, so the capability silently dropped out of every tally including the readiness
   percentage the phase gate is judged on. (100-tab RSS budget; it names a gate, so: `gated`.)

**FINDING 4 — two rows cited a gate that does not test them.** `cross-document View Transitions (MPA)`
and `promise-returning scroll methods` both had a gate column reading `g_mse_join claims` — prose
pasted from an unrelated MEDIA row. Corrected to `G_VIEW_TRANSITION` and `G_ELEMENT_SCROLL_TO`.

**WHAT WAS BUILT SO FINDING 3 CANNOT RECUR SILENTLY: `G_CONSTELLATION_WELLFORMED`.** Six fields
exactly on every row, five legal statuses, no blank rows, and a floor on the row count so a
truncating write is loud. RED-proven by re-injecting the exact defect (join two rows → the gate names
the offending line and its field count).

**AND ONE ASSERTION WAS DELETED RATHER THAN TUNED.** The companion "every cited gate exists on disk"
test found Finding 4 and was then removed: the gate column's vocabulary is heterogeneous *by design*
(file gates, crate-internal unit-test function names, perf floors like `F1/F2`, bare subsystem names,
multi-gate expressions), and every version that admitted those also admitted the prose that caused
the bug. **A gate tuned until it is green is the thing this repo refuses.** What it would take is
recorded in the gate's own header: a canonical gate registry emitted by the harness, cited by key —
which would also make `verify.sh`'s coverage countable, an open question memory already carries
("gated" ≠ "watched"). Observer-owned; named, not attempted.

## Audit #34 — tick 608

**METHOD.** The standing rule (t581) first: diff `engine/page/tests/` against `CONSTELLATION.tsv`,
then leave the frame and search the web. Both halves produced a finding, and the gate-corpus half
produced a *third* number that was again an artifact of the matcher — audit #33's exact lesson,
repeated one audit later.

**THE MATCHER LIED AGAIN, AND IT LIED THE SAME WAY.** My first pass reported **261 of 303 gate files
unreferenced by the map** — a catastrophic-looking regression from #33's 291-of-293. It was my own
one-liner: I compared bare file stems (`g_iface_surface`) against `grep -oE 'g_[a-z0-9_]+'` of the
map, which only ever matches the *lowercase* citations and misses every `G_UPPERCASE` one — the exact
mirror of the bug #33 caught (which missed the lowercase ones). **The rule to carry: this audit's
opening diff has now produced a wrong number on two consecutive runs, in opposite directions, from
the same case-sensitivity assumption.** The real instrument for this is `scripts/map-reconcile.sh`,
which does the matching properly; the hand-rolled diff should not be re-derived a third time.

**`map-reconcile.sh` — the honest number: 26 drift rows** (225 OK, 25 descriptive-floor, 82 honestly
unbacked). 20 are **bare assertions** (a `works`/`partial`/`gated` status with `gate='-'`: tables,
lists, web fonts, ResizeObserver, ESM, fetch/XHR, History, localStorage, custom elements, POST forms,
test262, scroll anchoring, forced reflow, …) and 6 are **dangling gates** (a `G_*` named with no
backing test: `G_AVIF_PAINT`, `G_AUDIO_PUMP`/`G_AUDIO_JOIN`, `G_TAB_DISCARD_RELEASES_TO_OS`,
`G_FIDELITY`, `G_RATE`, `G_CAP_TOUCH_PROBE`). This is the board's live CO-#1 item (3) and it is **not
closed by this audit** — it is a tick of its own, recorded here so it is not rediscovered.

**FINDING 1 — THE MAP HAD NO ROW FOR THE INTERFACE-OBJECT SURFACE, AND ITS ABSENCE COST A TOP-1K SITE
100% OF ITS CONTENT.** Zero hits for "interface object" across 359 rows. Nothing named the ~183
constructors a browser exposes on `globalThis`, and a probe found **63 of them missing**. This is
audit #31's finding recurring: the map tracked *the loop's attention* (21 interfaces, each added by
whichever test happened to need it) rather than the platform. The cost was measured, not imagined —
`www.welt.de`'s loader read `HTMLMetaElement`, took the `ReferenceError`, **concluded it was being
ad-blocked and aborted its own boot**: 3,242 of 3,243 elements unrendered, scored as `0.0% coverage`
by an instrument that could say *how much* was missing and never *why*. Row added, now `gated` by
`G_IFACE_SURFACE` (t608 took the surface 120 → 174 of 183).

**FINDING 2 — one Interop 2026 focus area was entirely absent, and two more named items with it.**
Checked all 20 focus areas + 4 investigation efforts against the map. **19 of 20 were present** —
which is a good result and worth stating, since an audit that only reports misses reads as if the map
were worthless. The misses:
- **container STYLE queries** (`@container style(--x: y)`) — zero hits. The map had container *size*
  queries (t386) and nothing for the style half, which is a separate mechanism. **PROBED: missing** —
  the rule did not apply (computed colour stayed `rgb(0,0,0)`).
- **`scrollend`** — zero hits. **PROBED: missing** — `'onscrollend' in el` and `in window` both false.
- **ESM top-level await / cyclic module records** (Interop's *web compat* area, alongside
  `user-select` and scroll/animation event timing, both already mapped) — zero hits. Added
  **`unknown` ON PURPOSE**: the probe only showed that `async`/`await` *parses*, which is not the
  claim, and booking `works` off it would have been testing the probe rather than the engine.

**WHAT WE HAD BEEN WRONG ABOUT — and it is a mis-attribution, not a gap.** The board and t606's pilot
both read welt.de's near-zero coverage as a **timing** result: *"the 12s load budget is exhausted, so
pages paint incomplete"*. It was nothing of the sort. The page was not painted incomplete — it was
**never booted**, and the 31s was the cost of an aborted load rather than a slow one. The reasoning
that produced the wrong answer is worth naming: a plausible mechanism (we are measurably slow) was
already on the table, so a symptom consistent with it was filed under it **without reading the
console**. `OURS IS SLOW` fires on wall-clock alone and cannot tell a slow boot from a dead one.

**SOURCES** (searched 2026-07-26, not recalled):
- <https://web.dev/blog/interop-2026> — the authoritative 20 focus areas + 4 investigation efforts
- <https://hacks.mozilla.org/2026/02/launching-interop-2026/> · <https://webkit.org/blog/17818/announcing-interop-2026/>
- <https://github.com/web-platform-tests/interop/blob/main/2026/README.md> — the *web compat* area's
  actual contents (ESM cyclic modules + multiple TLA, scroll/animation event timing, `user-select`)
- Ladybird 2026 status (2.07M WPT subtests, 97.8% of test262) — the independent-engine reference
  point for what order a from-scratch engine takes this in

**ADDED:** 4 rows (interface objects `gated`; container style queries `missing`; `scrollend`
`missing`; ESM TLA `unknown`). **CORRECTED:** welt.de's failure re-attributed from *timing* to
*aborted boot*. **CARRIED:** the 26 map-reconcile drift rows, unclosed and owned by a later tick.

---

## Audit #34 — tick 618

**What this audit measured:** the map, against the tree, twice — once by diffing `engine/page/tests/`
against `CONSTELLATION.tsv` (the standing first step since audit #31), and once with
`scripts/map-reconcile.sh`, whose 26 drift rows were the observer's live CO-#1 ask since t601.

### The instrument was wrong FIRST, twice, and both times in my favour to believe it

**262 of 311 gate files "unreferenced".** That is what the naive check said. It matched gate FILENAMES
against the map, and the map cites `G_*` CONSTANTS. Keyed correctly the answer is **6** — and all six
are **this session's own new gates** (t612/613/615/616/617), which I added across five ticks and never
mapped. Audit #31's rule held perfectly; the drift was entirely mine and entirely recent.

**`G_CAP_TOUCH_PROBE` reported as a DANGLING gate, and it is not.** It exists at
`tests/wpt/tests/g_cap_touch_probe.rs`; `map-reconcile.sh` scans only `engine/page/tests/`. I was one
edit from downgrading a real gate's claim on an instrument's say-so. Every other dangling report was
re-verified with a repo-wide `grep -rl` before anything moved — six of the seven are genuinely absent:
`G_AVIF_PAINT`, `G_AUDIO_PUMP`, `G_AUDIO_JOIN`, `G_TAB_DISCARD_RELEASES_TO_OS`, `G_FIDELITY`, `G_RATE`.

**Suspect the instrument before the subject — fourth time in this session** (t611's `[id]` message,
t613's grep-derived getter-only list, t614's oracle shell, and now both halves of this audit).

### The headline finding: the map claimed `works` for a capability that does not exist

> **`ES modules + dynamic import()` — status `works`, gate `-`.**

Static ESM is real and now gated four ways. **Dynamic `import()` is not implemented at all**:
SpiderMonkey answers *"Dynamic module import is disabled or not supported in this context"*, and a
direct probe confirms the module never completes. The row asserted **two** capabilities and reported
the stronger one's verdict for both. It is now split, with `import()` at `missing` and the evidence in
its receipt.

A second row was false in the same way: **`fetch / XHR / AbortController — works`, with no gate**,
while `xhr.addEventListener` was a `TypeError` for as long as anyone had looked (t613, 8 of 16 HEAD
sites construct an XHR). **A bare `works` is not a weak claim — it is an unfalsifiable one**, and two
of the twenty were provably false the moment a gate was written.

### Drift 26 → 0

| action | rows | how it was decided |
|---|---|---|
| cited a REAL backing gate | 11 | each verified to *assert* the claim, not merely to have a plausible filename — `pushState` appears 4× in `G_DOCUMENT_LOCATION`, `contrast-color` 12× in `G_CONTRAST_COLOR` |
| set `unknown` (no gate exists) | 14 | `@font-face` has **zero** occurrences in the whole gate directory; `ResizeObserver` appears only in presence lists, and *"the global exists"* is not *"the observer fires"* |
| split a two-capability row | 1 | ES modules vs `import()` |
| new rows for this session's gates | 5 | t612/613/615/616/617 |

`unknown`, not `missing`: `G_CONSTELLATION_WELLFORMED` rejected my first vocabulary (`unmeasured`) and
was right to — but `missing` would have been **a new lie in the opposite direction**, since most of
these capabilities probably do work. "We have not measured it" is the honest state and the map has a
word for it.

### ⚠ FOUR RATCHET MARKS LOWERED, DELIBERATELY, WITH THE EVIDENCE

`CONST:doc 24→22 · CONST:media 11→10 · CONST:cross 17→16 · MEASURED 362→356`

Those marks counted capabilities as **gated** whose named gate **has no test file anywhere in the
repo**. A gated count that includes phantoms was never a measurement of gating. The constitution's
provision — *"explain in the journal why the mark itself was wrong and lower it deliberately"* — is
exactly this case, and the test I applied before doing it: **would I make this change if it did not
help me land?** Yes: a map claiming `gated` for a gate that does not exist is the precise false
presence the reliability doctrine names as defect #1, and keeping it to protect a number is the worst
available reason. Each downgrade was verified independently of the reconciler that reported it.

The five capability marks that went UP in the same pass — `app 80→81, dom 19→20, js 1→3, net 5→7` —
are this session's real gates, and they are banked at the same moment. The correction is not a net
retreat; it is a *truer* number in both directions.

**Next audit due: tick 628.**

---

## Audit #35 — tick 628

**This audit's finding is that AUDIT #34 WAS WRONG, in a way that its own standing first step caught
ten ticks later.** #34 demoted 14 rows to `unknown` on the grounds that "no gate exists". **Three of
those already had gates**, one of them for **420 ticks**:

| row | #34 said | reality |
|---|---|---|
| scroll anchoring | `missing` | `G_SCROLL_ANCHOR` + `G_SCROLL_ANCHOR_LIVE`, built **tick 203/204**, passing today |
| forced reflow | `unknown`, then credited to #34's own new gate | `G_FORCED_REFLOW`, built **tick 213** |
| real-world quirks | `unknown` | `g_quirks_mode.rs` exists and passes |

**THE METHOD WAS THE BUG, AND IT IS WORTH STATING PRECISELY.** Audit #34 asked *"does this ROW cite a
gate?"* and answered *"no gate exists."* **Those are different questions.** A gate can exist under a
name the row never mentioned — and the map's whole failure mode is rows that do not know what backs
them. Searching the map for the gate can only ever confirm what the map already says.

> **The correct first step is to search the GATE DIRECTORY for the capability, not the map for the
> gate.** Audit #31 established "diff `engine/page/tests/` against the map" — #34 ran that diff in one
> direction (gates with no row) and not the other (rows with no *searched-for* gate).

**AND t623 COMPOUNDED IT WHILE APPLYING THE RULE MEANT TO PREVENT IT.** t622 established *"before
publishing an absence, name the code path that would deliver it and show that path ran."* At t623 I
applied that to scroll anchoring, found `overflow-anchor` absent from all four of Stylo's built
property tables, and called the negative confirmed. **`overflow-anchor` is the OPT-OUT property.** The
capability is the behaviour — a feed that does not jump when content loads above the read position —
and it has been implemented and gated since t203. The rule is only sound if **the path you name
delivers the thing you are ruling on**; I named the path for a different thing and got a true fact
about it, which is the most convincing possible way to be wrong.

**THAT IS THREE FALSE ABSENCES IN ONE SESSION** — `ResizeObserver` (t621, corrected t622), scroll
anchoring and forced reflow (t618/t621, corrected here) — against **zero** false presences of my own
making. Constitution check #44 named the asymmetry (*"a negative result feels like it needs no
confirmation"*) and this audit is its third instance, including one where I believed I was checking.
**The loop's scepticism is well calibrated for good news and badly calibrated for bad news**, and that
is now measured rather than suspected.

**Gate-vs-map diff (the standing step), run in BOTH directions this time:**
```text
  315 gate files · 7 unmapped by name
     G_DEFER · G_SILENT_FAIL          — long-standing, mapped under their capability rows, not by name
     G_WEBFONT_RELAYOUT_EXTERNAL      — t619's second half; the row cites the primary
     G_FORCED_REFLOW · G_SCROLL_ANCHOR · G_SCROLL_ANCHOR_LIVE · WEBFONT_LIVE
                                      — the gates this audit found, now cited
```

Drift **0** after the corrections; `G_CONSTELLATION_WELLFORMED` green.

**Next audit due: tick 638.**

## Audit #36 — tick 638

**This audit's finding is that the map had TWO CITATION DIALECTS, and each instrument was blind to
exactly one of them.** Both directions of the standing gate-vs-map diff ran, and they disagreed
about the same rows for opposite reasons:

- `scripts/map-reconcile.sh` validates only tokens matching `G_[A-Z0-9_]+`. A row citing
  **`g_a11y_roles`** in lowercase is not a token it recognises, so the row was filed as
  `descriptive-floor` — *prose*, validated by nothing.
- The gate-directory diff (`ls engine/page/tests/` uppercased, against the map's `G_*` tokens)
  compares uppercase, so it counted **the very same gate** as UNMAPPED.

**Eleven rows sat in that blind spot** — a11y interactive roles, canvas `fillText`, canvas
`drawImage`, View Transitions, the Navigation API, the Sanitizer API, `scheduler.postTask`,
`ToggleEvent.source`, two Baseline-2024 JS rows and `<video>` element layout. Every one *had* its
gate; not one was *checked*. Uppercasing the citations moved validated claims **259 → 271** with
drift still 0, which is the proof they were real all along: a wrong citation would have surfaced as
drift the moment it became visible.

> **A claim that no instrument can read is not a weak claim, it is an unaudited one** — and it looks
> exactly like a strong claim from every direction anyone actually checks.

**A capability with NO ROW AT ALL: `URLPattern`.** `engine/page/tests/g_urlpattern.rs` has existed
and passed, and the map had zero occurrences of the string. Not a wrong claim — an **absent** one,
which is the failure mode a map structurally cannot report on itself, and the reason this audit
exists. Added as a gated `app` row.

**Three media rows were stale by my own hand, from this same session:**

| row | said | reality |
|---|---|---|
| `navigator.mediaCapabilities.decodingInfo()` | `missing` | **landed at t635**, three ticks earlier |
| container demux (MP4/WebM) | cited the MP4-era gate alone | WebM demux landed t633 (`G_MEDIA_WEBM`) |
| video decode (H.264/VP9/AV1) | `video_decode` only | AV1-in-WebM landed t634 (`G_MEDIA_WEBM_AV1`, `G_WEBM_AV1_DRIVE`) |

**None of these were caught by anything that reads the map.** All three surfaced from the
gate-directory side — a gate file with no citing row. Landing a capability and updating the map are
two actions, and the second one is the one that gets skipped *by the person who just did the first*,
because to them the capability is obviously present.

**A THIRD instrument blind spot, and it is the observer's to fix.** `map-reconcile.sh` searches
`engine agent tests` and **not `shell/`** — so it cannot see the seven gates that live as `#[test]
fn` inside `shell/src/media.rs` (`g_avif_paint`, `g_av1_drive`, `g_media_drive`, `g_mp3_drive`,
`g_muted_out`, `g_idl_feed`, and t634's `g_webm_av1_drive`). The AVIF row citing `G_AVIF_PAINT` was
therefore **true and unverifiable at the same time**, and reported as a DANGLING GATE. Handled on
the agent side by naming the engine-side gate that always existed — `engine/media/tests/avif_alpha.rs`
is now `G_AVIF_ALPHA` — so the decode half is machine-checkable and the paint half is named for a
human. **The scripts are harness-owned: noted, not touched.**

**The two rows I created last tick were themselves the first drift this audit found**, which is the
cadence working as designed rather than an embarrassment: `CSS ic/ric` was `partial` with `gate='-'`
(a bare assertion — a status claiming capability with nothing behind it) and AVIF was the dangling
citation above. The `ic` row now cites **`G_IC_UNIT_PARSES`**, which asserts *only* the falsifiable
half — that `ic`/`ric` are RECOGNISED units, proved against a bogus-unit control (`width: 10zz` must
drop to `auto` and fill the container) — and explicitly **not** that the value is the styled face's
real 水 advance, which t637 established is unprovable on any available font.

**Gate-vs-map diff (the standing step), both directions:**
```text
  gate files under engine/page/tests/ : 318
  map rows                            : 371  (was 370; +1 URLPattern)
  map -> gate drift                   : 2 -> 0     (bare assertion, dangling gate)
  gate -> map, no citing row          : 18 -> 9
  of those 9: 2 are prefix-matched (G_POPOVER_RENDER, G_WEBFONT_RELAYOUT_EXTERNAL)
              2 are reliability gates with no capability row by design (G_DEFER, G_SILENT_FAIL)
              5 are redundant siblings of rows that cite a different gate for the same capability
  claims machine-validated            : 259 -> 271
  descriptive-floor (unvalidated)     : 24 -> 12
```

## Audit #37 — tick 649

**Audit #36's fixes held, and the audit's own method caught MY drift from two ticks earlier.**

**Direction 1 (map → gate): clean.** `map-reconcile.sh` reports **drift 0** across 374 rows, 277
machine-validated claims. **The lowercase-citation dialect that #36 found is completely gone** — zero
rows now cite a `g_foo` token that no instrument can read, down from eleven. That fix held without
maintenance, which is the test of whether it was a fix or a sweep.

**Direction 2 (gate → map): one real finding, and it was mine.** `G_SVG_CLIENT_RECT` — landed at
**t647, two ticks ago** — appeared **zero times** in the map. The gate existed, passed, and was
cited in the journal and the wiki; the map did not know about it.

**THE CAUSE IS A SCRIPTED EDIT THAT MATCHED NOTHING AND SHIPPED.** t647's map update anchored on
`'doc\t`SVGGraphicsElement.getBBox()`…'`. The row's class is **`dom`**. The replacement silently
matched zero rows, the script exited 0, the tick landed, and I reported the citation as done.

> **`[[scripted-edit-silent-noop]]` — the fourth this session, and the first to reach a commit.** The
> other three were caught within minutes because they produced a *visibly* wrong measurement (every
> library reporting `tail:false`; a probe printing nothing). This one produced **no output at all**,
> which is what made it survive: a no-op edit on a data file has no symptom until something else
> reads the file. **Assert every replacement — and on a data file, assert the COUNT**, because
> "matched nothing" and "matched what I meant" are the same silence.

The fix is re-applied with `assert hits == 1`, and the row records that it is being written for the
second time and why.

**Gate-vs-map diff, both directions:**
```text
  gate files under engine/page/tests/ : 324   (was 318 at #36; +6 this session)
  map rows                            : 374   (was 371)
  map -> gate drift                   : 0
  gate -> map, no citing row          : 9 -> 8 after the fix
  of those 8: 2 prefix-matched (G_POPOVER_RENDER, G_WEBFONT_RELAYOUT_EXTERNAL)
              2 reliability gates with no capability row by design (G_DEFER, G_SILENT_FAIL)
              4 redundant siblings of rows citing a different gate for the same capability
  lowercase-dialect rows              : 11 -> 0   (audit #36's fix, holding)
  machine-validated claims            : 277
  constellation unknowns              : 0
```

**And the standing note, for the third audit running:** `map-reconcile.sh` searches
`engine agent tests` and **not `shell/`**, so the seven gates living as `#[test] fn` in
`shell/src/media.rs` and `shell/src/audio.rs` — now eight, with t646's `g_audio_rate` — remain
invisible to it. Rows that depend on them cite an engine-side equivalent and name the shell gates in
the receipt. Harness-owned; reported, not touched.

## Audit #38 — tick 659

**Direction 1 (map → gate): clean, and it stayed clean without maintenance.** `map-reconcile.sh`
reports **drift 0**. Audit #36's lowercase-citation fix and #37's re-applied `G_SVG_CLIENT_RECT` row
both held.

**Direction 2 (gate → map): SIX gates, none of them on the map — and every one is mine, from the last
seven ticks.**

```text
  G_SCRIPT_LOAD_EVENT                      t652
  G_CSS_SURVIVES_BUDGET                    t654
  G_EXTERNAL_CSS_SURVIVES_RESTYLE          t654
  G_IMAGES_SURVIVE_BUDGET                  t655
  G_IMAGE_NATURAL_SIZE_SURVIVES_RESTYLE    t656
  G_H2_LARGE_RESPONSE_HEADERS              t658
```

This is audit #37's finding **recurring, in a worse form**. #37 found *one* uncited gate and traced it
to a scripted edit that matched nothing and shipped — a silent no-op. Its lesson was *"assert the
COUNT on data-file edits."* This time there was no failed edit to assert: **I never attempted the map
update at all, six ticks running.** Each tick wrote its gate into the journal, the wiki and the
pattern ledger — three of the four places — and skipped the one that other instruments *read*.

> **A lesson aimed at the mechanism of a mistake does not cover the case where the mechanism is never
> invoked.** #37 hardened *how* the map gets edited. Nothing made the map get edited. The gate→map
> direction of this audit is the only thing in the loop that notices, which is why it runs every ten
> ticks and why an audit that finds nothing is the suspicious one.

All six rows are added with real receipts. Remaining uncited, unchanged and by design: `G_DEFER` and
`G_SILENT_FAIL` (reliability gates with no capability row), `G_WEBFONT_RELAYOUT_EXTERNAL`
(prefix-matched by a row citing `G_WEBFONT_RELAYOUT`).

### AND THE AUDIT'S OWN ARITHMETIC CAUGHT A SECOND DEFECT THE FIRST FIX INTRODUCED

After adding six rows, the reconciler's buckets read

```text
  rows 380  ·  OK 282 + descriptive-floor 14 + missing 83 = 379      <- one row in NO bucket
```

Before the edit the same three buckets summed **exactly** to the row count. One row had gone
somewhere, and the answer was that **my append left the file with no trailing newline**: the
reconciler reads it with `while IFS=$'\t' read -r …`, and `read` returns non-zero on a final line
with no terminator, so **the last row of the file is silently invisible to every consumer that reads
it that way.** `grep` finds the row. `wc -l` under-counts it. The reconciler skips it entirely and
still prints `✓ RECONCILED`.

Restoring the newline (and dropping a stray blank line the same edit introduced) gives
`283 + 14 + 83 = 380`, exactly.

> **Nothing else would have caught this.** No gate covers the TSV's byte layout, the row was present
> to every text search, and the reconciler reported success. It was found by **a number that did not
> add up** — which is `STATUS.md`'s meta-instrument #3 (*"8 of 30 process defects were caught by a
> number that did not add up, not by any gate"*) paying out again, and the strongest argument in the
> file for keeping the buckets printed even when the verdict is green. **A summary line whose parts
> do not sum is a bug report; print the parts.**

**Gate-vs-map diff, both directions:**
```text
  gate files under engine/{page,net}/tests/ : 331   (was 324 at #37; +7 this session)
  map rows                                  : 380   (was 374)
  map -> gate drift                         : 0
  gate -> map, no citing row                : 9 -> 3
  of those 3: 1 prefix-matched (G_WEBFONT_RELAYOUT_EXTERNAL)
              2 reliability gates with no capability row by design (G_DEFER, G_SILENT_FAIL)
  machine-validated claims                  : 277 -> 283
  bucket arithmetic (OK+floor+missing==rows) : 379/380 -> 380/380
  constellation unknowns                    : 0
```

**Standing note, fourth audit running:** `map-reconcile.sh` searches `engine agent tests` and not
`shell/`, so the eight gates living as `#[test] fn` in `shell/src/media.rs` and `shell/src/audio.rs`
remain invisible to it. Harness-owned; reported, not touched. **New this audit:** the same search DOES
reach `engine/net/tests/`, which is how `G_H2_LARGE_RESPONSE_HEADERS` validates from outside
`engine/page/tests/` — so the standing "diff `engine/page/tests/` vs the map" step is widened here to
`engine/{page,net}/tests/`, or a gate landing in any other engine crate would be invisible to the
audit that exists to see it.

## Audit #39 — tick 669

**Direction 1 (map → gate): clean.** `map-reconcile.sh` reports **drift 0** across 383 rows, and the
bucket arithmetic **reconciles exactly** — `286 + 14 + 83 = 383` — which is the check audit #38 had to
invent after a missing trailing newline hid a row from every `while read` consumer. This time the
append asserted its own row-count delta *and* the trailing newline before writing. **The lesson held
because it became an assertion, not because it was remembered.**

**Direction 2 (gate → map): three gates uncited, all from this window.**

```text
  G_SCRIPT_ERROR_HAS_A_LOCATION   t662
  G_CSSOM_SHEET_BRIDGE            t665
  G_DRAIN_BOUNDS_THE_PAGE         t667
```

All three added with real receipts. Remaining uncited and unchanged, by design: `G_DEFER` and
`G_SILENT_FAIL` (reliability gates with no capability row), `G_WEBFONT_RELAYOUT_EXTERNAL`
(prefix-matched by a row citing `G_WEBFONT_RELAYOUT`).

### AND A CORRECTION TO AUDIT #38'S OWN FRAMING

#38 found six uncited gates and called them *"audit #37's finding recurring, in a worse form"*, blaming
a lesson that hardened *how* the map gets edited while nothing made the map get edited. That reading
was too harsh on the mechanism and too generous about what a fix would look like.

> **The gate→map direction is the ONLY instrument that catches this, it runs every ten ticks, and a
> ten-tick lag is therefore its designed steady state — not a failure.** Three gates over the ten
> ticks since #38 is exactly what a healthy loop looks like: gates land, the audit sweeps them in.
> #38's "six" spanned t652–t658, which is more than one window, so part of that batch was #37's
> backlog and not a single lapse.

What would actually remove the lag is a landing-time check — the same shape as the journal and
`WIKI:` trailer checks that already run in one second — and that lives in `scripts/tick.sh`, which is
**observer-owned**. Reported, not touched, and stated here as a standing request rather than a
recurring self-criticism: *the cadence is doing its job; the lag is the cost of doing it every ten
ticks instead of every one.*

**Gate-vs-map diff, both directions:**
```text
  gate files under engine/{page,net}/tests/ : 336   (was 331 at #38; +5)
  map rows                                  : 383   (was 380)
  map -> gate drift                         : 0
  gate -> map, no citing row                : 6 -> 2
  of those 2: 1 prefix-matched (G_WEBFONT_RELAYOUT_EXTERNAL)
              1 reliability gate with no capability row by design (G_DEFER)
  machine-validated claims                  : 283 -> 286
  bucket arithmetic (OK+floor+missing==rows): 383/383  ✓ asserted at write time
  constellation unknowns                    : 0
```

**Standing note, fifth audit running:** `map-reconcile.sh` searches `engine agent tests` and not
`shell/`, so the eight gates living as `#[test] fn` in `shell/src/media.rs` and `shell/src/audio.rs`
remain invisible to it. Harness-owned; reported, not touched.

## Audit #40 — tick 679

**Sources read (2026-07-27, not from memory):**

- [Interop 2026 — web.dev](https://web.dev/blog/interop-2026) — the full list of 20 focus areas and
  4 investigation efforts, fetched rather than recalled.
- [Announcing Interop 2026 — WebKit](https://webkit.org/blog/17818/announcing-interop-2026/) ·
  [Launching Interop 2026 — Mozilla](https://hacks.mozilla.org/2026/02/launching-interop-2026/) ·
  [Igalia](https://www.igalia.com/news/interop-2026.html)
- [Baseline 2026 — web.dev](https://web.dev/baseline/2026) plus the monthly digests (Jan/Mar/Apr/May
  2026) — what is considered safe-to-use TODAY.
- [This Month in Ladybird — June 2026](https://ladybird.org/newsletter/2026-06-30/) — the independent
  engine's own account of what is hard, and in what order.

**The frame check, first: is our map inside the right frame?**

All **20 Interop 2026 focus areas** are on `CONSTELLATION.tsv`, with a status:

```text
anchor positioning · container style queries · dialogs+popovers · scroll-driven animations
view transitions · attr() · contrast-color() · custom highlights · fetch uploads+ranges
IndexedDB · JSPI · media pseudo-classes · Navigation API · scoped custom element registries
scroll snap · shape() · web compat · WebRTC · WebTransport · zoom
```

…and 3 of the 4 investigation efforts (WebVTT gated, JPEG XL missing, mobile testing out of scope as
a desktop-first engine). The fourth — **accessibility testing** — is represented by capability rows
(`a11y node STATES`, `a11y interactive roles`, `accessibility / semantic tree`) but not as a *testing*
concern, which is the honest reading: we gate a11y *behaviour* and have no a11y *conformance* suite.
Recorded, not inflated into a row we would not act on.

**Ladybird's account is worth one sentence, because it is the only other team walking this exact
path:** their hard set is *"complex web apps, WebAssembly-heavy sites, and some modern CSS layouts"*,
and their named biggest challenge is **web compatibility against undocumented engine quirks** — not
spec conformance. That is the same ordering this project reached from its own measurements (the app
web and the certificate's `thin-overlap` rows, not the CSS tail), which is mild corroboration that the
frame is right. They crossed 90% of all WPT subtests in Oct 2025.

**WHAT WE HAD BEEN WRONG ABOUT — one FALSE YES, and it is a shape, not an instance.**

Probing the five Baseline-2026 CSS features the map did not name turned up a new instance of the
false-YES class:

```text
sibling-index() / sibling-count()   calc dropped (784px auto)  ·  supports=false   HONEST no
random()                            calc dropped (784px auto)  ·  supports=false   HONEST no
reading-flow                        computed undefined         ·  supports=false   HONEST no
text-box / -trim / -edge            40px box measured 46px     ·  supports=false   HONEST no
corner-shape                        computed UNDEFINED         ·  supports=TRUE    ** FALSE YES **
```

⚠ **All eight `corner-*-shape` LONGHANDS were already on `PARSE_ONLY_LONGHANDS`, and the SHORTHAND was
not.** `honest_supports` subtracts what this list names and a page asks about whichever spelling it
writes — so listing every longhand of a property and not its shorthand corrects *nothing for the
spelling authors actually use*. `mask-position` is the same shape against `mask-position-x`/`-y`.

**The rule, stated so it can be applied rather than remembered:** *a shorthand must answer NO iff
EVERY one of its longhands is parse-only.* `mask` deliberately does **not** qualify — `mask-image` is
real (the icon-mask paint phase reads it) — so answering no there would be a **false NO**, which costs
a page its enhancement branch just as surely as a false yes costs it a fallback. `G_ZOOM_AND_PROBE_PINS`
now asserts `supMask=true` for exactly that reason: without it, a blanket fix that listed every mask
shorthand would pass.

This is **"one rule, N implementations — fix one, GREP FOR THE OTHER"**, ninth occurrence.

**ADDED (5 rows, every one with a MEASURED verdict rather than `unknown`):**
`sibling-index()/sibling-count()` · `random()` · `reading-flow` · `text-box/-trim/-edge` ·
`corner-shape/mask-position shorthand honesty` (the last one **gated**, since the honesty of the
answer is what landed — the feature is still missing and says so).

**CORRECTED:** `corner-shape` and `mask-position` added to `PARSE_ONLY_LONGHANDS`; four new assertions
in `G_ZOOM_AND_PROBE_PINS`, including the over-correction guard.

**Gate-vs-map diff, both directions:**
```text
  gate files under engine/{page,net}/tests/ : 335   (was 336 at #39; two scratch probes removed)
  map rows                                  : 388   (was 383; +5 from this audit)
  map -> gate drift                         : 0     (map-reconcile.sh: RECONCILED)
  machine-validated claims                  : 286 -> 287
  bucket arithmetic (OK+floor+missing==rows): 388/388  ✓
  constellation unknowns                    : 0     (the five new rows landed MEASURED)
```

**Standing note, sixth audit running:** `map-reconcile.sh` searches `engine agent tests` and not
`shell/`, so the eight gates living as `#[test] fn` in `shell/src/media.rs` and `shell/src/audio.rs`
remain invisible to it. Harness-owned; reported, not touched.

## Audit #41 — tick 689

**A SOURCE NO PREVIOUS AUDIT HAD READ**, which is the point of the cadence:

- [Microsoft Edge — 2026 web platform top developer needs](https://microsoftedge.github.io/TopDeveloperNeeds/)
  — ranked by developer VOTES for features they cannot use, which is a different ordering from Interop's
  (vendor-agreed test mass) and from ours (oracle cluster mass). Three orderings, three blind spots.
- [Interop 2026 — web-platform-tests](https://github.com/web-platform-tests/interop/blob/main/2026/README.md)
  · [WebKit: Improving Web Accessibility with WPT](https://webkit.org/blog/15400/improving-web-accessibility-with-web-platform-tests/)
  — the accessibility-testing investigation continues from 2024: *consistent accessibility trees from the
  same DOM and CSS across browsers*. That is our agent-native moat stated as an interop goal, and it is
  worth knowing the bar will be a WPT suite.

**EIGHT ROWS ADDED, every one with a MEASURED verdict.** Six are honest NOs
(`appearance: base-select`, `calc-size()`/`interpolate-size()`, `field-sizing`, `justify-self` in block,
`text-wrap: pretty`, `navigator.virtualKeyboard`) — `CSS.supports` answers false and the computed value is
absent, so a page's `@supports` fallback still runs. `moveBefore` and `CSSStyleSheet.replaceSync` were
already present and already on the map.

### ⚠⚠ WHAT WE HAD BEEN WRONG ABOUT — 1: A Bar-0 ITEM WITH NO MAP ROW

`calc-size()` / `interpolate-size()` had **no capability row at all**, while this project carries an **open
Bar-0 SIGSEGV recorded against exactly those properties** (release-only, not a regression, needs a fresh
ASAN build). **A Bar-0 item with no capability row is invisible to every audit that reads the map** — and
the map is what `map-reconcile.sh`, `phase0-progress.sh` and the readiness percentage all read. The crash
was remembered and the capability was not; the row now carries both.

### ⚠⚠ WHAT WE HAD BEEN WRONG ABOUT — 2: TWO NEW FALSE-YES CANDIDATES

```text
  customElements.define('x-btn', class extends HTMLButtonElement {}, {extends:'button'})   ACCEPTED
  new IntersectionObserver(cb, { trackVisibility: true, delay: 100 })                      ACCEPTED
```

Neither throws. **ACCEPTED IS NOT IMPLEMENTED.** Whether `<button is=x-btn>` upgrades and keeps button
semantics is unmeasured; whether any entry carries a truthful `isVisible` is unmeasured. An options bag that
is silently ignored is the worst shape available: the page is told yes and reads `undefined`.

Both are filed **`partial`** rather than works-or-missing, deliberately. This session has now paid four
times for presence standing in for behaviour — `reportError` pinned `WORKS` by `typeof` (t675), the
`corner-shape` shorthand answering `true` while unread (audit #40), and these two. **The standing form:
`typeof`, "no throw", and "the constructor accepted my options" are all statements about PRESENCE. Only a
measured effect is a statement about BEHAVIOUR.** Verifying these two is a named next probe, not a claim.

**Gate-vs-map diff, both directions:**
```text
  map rows                                  : 396   (was 388 at #40; +8)
  map -> gate drift                         : 0     (map-reconcile.sh: RECONCILED)
  machine-validated claims                  : 287 -> 289
  bucket arithmetic (OK+floor+missing==rows): 396/396  ✓
  constellation unknowns                    : 0     (all eight landed MEASURED; two as honest `partial`)
```

**Standing note, seventh audit running:** `map-reconcile.sh` searches `engine agent tests` and not `shell/`,
so the eight gates living as `#[test] fn` in `shell/src/media.rs` and `shell/src/audio.rs` remain invisible
to it. Harness-owned; reported, not touched.

## Audit #42 — tick 699

**Date:** 2026-07-28. **Sources read (not remembered):**

- <https://github.com/web-platform-tests/interop/blob/main/2026/README.md> — the authoritative Interop
  2026 list: **20 focus areas + 4 investigation efforts**.
- <https://webkit.org/blog/17818/announcing-interop-2026/> · <https://web.dev/blog/interop-2026> —
  20 areas, **15 of them new**, 5 carried over from 2025.
- <https://ladybird.org/newsletter/2026-06-30/> · <https://ladybird.org/newsletter/2025-10-31/> —
  the independent-engine reference point: Ladybird passed **90% of all WPT subtests** (Oct 2025) and
  sits at ~2,078,912 subtests (Jun 2026). Its own stated hardest problem is **web compat — sites
  coded to undocumented Blink/WebKit quirks** — not spec conformance.

### RECONCILIATION — the map already had all 24

Every one of the 20 focus areas and all 4 investigation efforts resolves to at least one existing
`CONSTELLATION.tsv` row. **Nothing was added**, and that is a real (if unexciting) result: this is the
first audit in a while where the outside world named nothing the map had not.

```text
  container style queries  · anchor positioning · attr() · contrast-color() · zoom · custom highlights
  dialogs+popovers · fetch uploads/ranges · IndexedDB · JSPI · media pseudo-classes · Navigation API
  scoped custom element registries · scroll-driven animations · scroll snap · shape() · view transitions
  web compat · WebRTC · WebTransport      + a11y testing · JPEG XL · mobile testing · WebVTT
```

### WHAT WE HAD BEEN WRONG ABOUT

**1. Two capabilities were on the map TWICE, with CONTRADICTORY statuses.** Found by token-set
matching the capability column rather than exact string (the earlier exact-match check reported one
duplicate; the real count was two, because `CSS contrast-color()` and `contrast-color() (CSS)` are the
same capability written in two orders):

```text
  contrast-color()   status=missing gate=-   ×   status=gated gate=G_CONTRAST_COLOR
  XPath              status=missing gate=-   ×   status=partial gate=G_XPATH_SUBSET
```

Both gates were **verified to exist and RUN GREEN** before touching the map
(`engine/page/tests/g_contrast_color.rs`, `engine/page/tests/g_xpath_subset.rs`, 1 passed each) — a map
edit that guesses which side is true is worse than the drift it fixes. The two stale `missing` rows are
deleted; **396 → 394 rows, asserted on the edit.** Contradictory duplicates now **0**.

⚠ The standing `map-reconcile.sh` drift check compares map→gate and could not see this: both
capabilities DID have a real backing gate on their other row. **A duplicate row is invisible to a
per-row reconciler** — the check has to be name-vs-name, and it now is.

**2. `constellation unknowns` is 0, and the LAUNCH PROMPT still says "PROBE the ~35 unknowns".**
Status distribution is **263 gated / 91 missing / 30 partial / 10 works — zero `unknown`**. Item (D) of
the standing agent prompt is stale by construction and cannot be actioned; noted here because a prompt
fix only lands on relaunch (the same failure mode recorded at t684).

**3. Interop 2026 names SIX of our declared death-tail items as top-20 focus areas** — anchor
positioning, custom highlights, scroll-driven animations, JSPI, scoped custom element registries, and
WebTransport (plus JPEG XL as an investigation). This is **not** an argument to build them: Interop
ranks by *cross-engine developer pain*, and I4 ranks by *usage-weighted breadth*, which are different
questions and deliberately so. It is recorded because the death-tail list should be re-derived from
evidence rather than inherited, and the next audit should check whether any of the six has crossed into
real usage — `scroll-driven animations` and `anchor positioning` are the two most likely to.

**Gate-vs-map diff:**
```text
  map rows                                  : 394   (was 396 at #41; -2, both stale duplicates)
  contradictory duplicate names             : 2 -> 0
  constellation unknowns                    : 0
  Interop 2026 areas absent from the map    : 0 of 24
```

**Standing note, eighth audit running:** `map-reconcile.sh` searches `engine agent tests` and not
`shell/`, so the eight gates living as `#[test] fn` in `shell/src/media.rs` and `shell/src/audio.rs`
remain invisible to it. Harness-owned; reported, not touched.

## Audit #43 — tick 710

**Date:** 2026-07-28. **Sources (read today, not recalled):**
`github.com/web-platform-tests/interop/blob/main/2026/README.md` · `web.dev/blog/interop-2026` ·
`webkit.org/blog/17818/announcing-interop-2026/` · `ladybird.org/newsletter/2026-06-30/` ·
Chrome platform status / caniuse usage figures for scroll-driven animations and anchor positioning.

### THE ASSIGNMENT #42 LEFT, ANSWERED: YES, AND IT IS BOTH OF THE TWO IT NAMED

Audit #42 closed with *"the next audit should check whether any of the six [death-tail rows] has
crossed into real usage — `scroll-driven animations` and `anchor positioning` are the two most likely
to."* Measured:

```text
  scroll-driven animations   ~4.695% OF CHROME PAGE LOADS   ·  82.58% global support (caniuse)
                             Chrome/Edge since 115 (Jul 2023) · Safari 26 (Sep 2025) · Firefox 152 flagged
  CSS anchor positioning     Chrome/Edge/Opera STABLE and FEATURE-COMPLETE · Safari 18.x partial → full
```

**One page load in twenty-one uses a feature we have written down as do-not-build.** That is not a
map error — both rows are present and honestly marked `missing`. It is a **PRICING** error, and the
standing rule is the one from t654-672: *when a capability is deferred as "needs X", RE-PRICE X.* The
t543 death-tail priced these when they were experimental and single-engine; they are now shipped in
three engines with measurable production usage, and 82.58% support means authors ship them
**unguarded** — so "feature-detect cleanly and degrade" degrades on a real page, not a hypothetical.

⚠ **This is a re-pricing, NOT a scope change, and I am not taking it.** Phase 0 is *"runs almost
every website"* and the binding constraint is still SHAPE 43% (t706). But the deferral must now be a
DELIBERATE, dated, priced decision rather than an inherited one, and the next audit should re-check
the number rather than the list.

### THE INDEPENDENT ENGINE AGREES WITH OUR OWN DOCTRINE, WHICH IS THE USEFUL PART

Ladybird crossed **90% of all WPT subtests** in Oct 2025 and reached 2,078,912 subtests by Jun 2026 —
and still reports its hardest problems as *web compatibility, real-site quirks, complex web apps and
modern CSS layouts*, i.e. **not** the conformance number. An independent engine at >90% WPT is not
daily-drivable for the same reason we are not: *capability% cannot see feature-present-but-site-
broken.* That is our PROCESS RULE (1) confirmed from outside the frame, which is the only place it
can be confirmed. Our t706 certificate — 131 scored, shape ≥0.75 on **11** — is the same finding
measured from the other end, and it is the reason the loop should keep spending on fidelity rather
than on WPT flips.

### GATE-VS-MAP DIFF

```text
  map rows                                  : 395   (was 394 at #42; +1)
  constellation unknowns                    : 0     (fourth audit running)
  Interop 2026 focus areas                  : 20    (#42 counted 24 — the published list is 20 today)
  Interop 2026 areas absent from the map    : 0 of 20
  Interop 2026 areas marked `missing` here  : 8 of 20
  ...of those 8, on our explicit death-tail : 6     (anchor positioning · custom highlights ·
                                                     scroll-driven animations · JSPI · scoped custom
                                                     element registries · WebTransport/HTTP3)
  ...of those 8, NOT excused by any list    : 2     (container STYLE queries · CSS shape())
```

**ADDED:** nothing — the map already covers the world's own 2026 priority list, four audits running.
**CORRECTED:** nothing in the map.
**WHAT WE HAD BEEN WRONG ABOUT:** the *price* of two death-tail rows, above. And one more, found
while reconciling — **`constellation unknowns` has been 0 for four audits, yet the agent's LAUNCH
PROMPT still lists "(D) PROBE the ~35 constellation unknowns" as a CO-#1 item.** There are none and
there have been none for ~40 ticks. A launch prompt only updates on RELAUNCH, so this is reported for
the observer and cannot be fixed from here — the same staleness class as the t684 "do NOT grind the
CSS-layout tail" block, which contradicted the board for three ticks before anyone checked.

**The two genuinely un-excused gaps** — `container STYLE queries` and `CSS shape()` — are small, are
Interop 2026 focus areas, and are on no deferral list. They are the honest output of this audit as
work items.

**Standing note, ninth audit running:** `map-reconcile.sh` searches `engine agent tests` and not
`shell/`, so the eight gates living as `#[test] fn` in `shell/src/media.rs` and `shell/src/audio.rs`
remain invisible to it. Harness-owned; reported, not touched.

## Audit #44 — tick 721 (2026-07-28)

**Sources (searched, not recalled):**
- <https://github.com/web-platform-tests/interop/blob/main/2026/README.md> — the canonical Interop
  2026 list: 20 focus areas + 4 investigation efforts.
- <https://webkit.org/blog/17818/announcing-interop-2026/> · <https://web.dev/blog/interop-2026>
- <https://web.dev/blog/baseline-digest-may-2026> · <https://web.dev/baseline/2026>
- <https://ladybird.org/newsletter/2026-06-30/> — the independent engine's own trajectory.

### GATE-VS-MAP DIFF

```text
  map rows                                  : 395
  Interop 2026 focus areas                  : 20    (unchanged from #43)
  Interop 2026 areas absent from the map    : 0 of 20
  Interop 2026 INVESTIGATION efforts        : 4     (a11y testing · JPEG XL · mobile testing · WebVTT)
  ...absent from the map                    : 0 of 4
  ADDED                                     : nothing
  CORRECTED                                 : ONE ROW, from `works` to `partial` — see below
```

### ⚠⚠ WHAT WE HAD BEEN WRONG ABOUT: A `works` ROW THAT DOES NOT WORK

`CSS lh / rlh line-height units` has read **`works`** since tick 509. It is `partial`, and the audit
found it by testing a property the probe does not.

```text
  root line-height 2 (=32px) · element line-height 20px      CHROME     MANUK
    width:  5lh                                                 100       100   ok
    height: 5lh                                                 100       100   ok
    width:  5rlh                                                160        96   ✗
    height: 5rlh                                                160        96   ✗
    CSS.supports('width','5lh')                                true     FALSE   ✗
    CSS.supports('height','5lh')                               true     FALSE   ✗
    CSS.supports('width','5rlh')                               true     FALSE   ✗
```

**Two distinct defects, and the first one was predicted in writing by the row itself.** The t509
receipt ends *"`rlh` is the root-relative sibling on the identical Stylo line-height-relative length
path … **not separately geometry-tested**"* — and the half nobody tested is the half that is broken.
`96 = 5 × 19.2` is the **initial `normal` line-height** (16 × 1.2), so `rlh` resolves against neither
the root's computed line-height (5 × 32 = 160) nor the element's (5 × 20 = 100). It is not
root-relative at all; it is initial-relative.

**The second defect is feature detection, and it points the other way from the usual one.**
`CSS.supports` returns **false** for `lh`/`rlh` in *every* property tested, while the unit demonstrably
works. This project's standing hazard is a false *presence* (`@supports` answering "does it parse" →
31 phantom properties; `typeof null === 'object'` → t717). This is the inverse: **a working
capability that reports itself absent**, so every page guarding `lh` behind a support check takes its
fallback path for no reason. `lh`/`rlh` reached **Baseline Widely Available in May 2026**, which is
precisely the threshold at which authors stop guarding — so the false negative gets *more* expensive
from here, not less.

### THE LESSON THIS AUDIT PAID FOR

> **A probe that tests one property has measured one property.** `lhunit` is a good behavioural
> probe — `width:5lh` against `line-height:20px`, passing only on exactly 100, RED-proof against both
> a 1em fallback and a dropped declaration. It is not wrong. It is *narrow*, and the map recorded its
> narrow result under a wide name (*"lh / rlh line-height units"*), which is how a `works` row comes
> to cover a unit that was never tested.

This is the same shape as audit #43's finding one level down: there, a **deferral's price** went
stale; here, a **probe's scope** was wider in the ledger than in the code. Both are the map describing
something the instrument never looked at.

**Standing note, tenth audit running:** `map-reconcile.sh` searches `engine agent tests` and not
`shell/`, so the eight gates living as `#[test] fn` in `shell/src/media.rs` and `shell/src/audio.rs`
remain invisible to it. Harness-owned; reported, not touched.

## Audit #45 — tick 732 (2026-07-28)

**Sources (searched, not recalled):**
- <https://web.dev/baseline/2026> and the 2026 monthly digests — Baseline movement.
- <https://blog.mozilla.org/netpolicy/2026/03/23/competition-innovation-and-the-future-of-the-web/>,
  <https://techcrunch.com/2026/07/03/...> — the engine landscape and what trails where.
- Interop 2026 re-checked against #44's list: **20 focus areas, 0 absent from the map** (six audits).
- **This session's own differential probes** (t726, t728, t729, t730) — see below, because they are
  the finding.

### ⚠⚠ THE FINDING IS ABOUT THE MAP'S SOURCES, NOT ITS CONTENTS

Six audits running, the reconciliation against the world's own lists comes back **clean**: every
Interop 2026 focus area and investigation effort is on the map. So I reconciled against a source the
audit protocol does not name — **our own measurements from the last ten ticks** — and found this:

```text
  capability                                          measured vs Chrome     row in the map?
  document.caretRangeFromPoint / caretPositionFromPoint   function / undefined     NO
  CSS Typed OM (computedStyleMap, CSSNumericValue)        function / undefined     NO
  container-query length units (cqw/cqi/…)                200px    / 400px         NO
  @container rule CASCADE ORDER (a defect)                red      / green         NO
  document.elementsFromPoint                              3        / TypeError     NO  (built t729)
  document.fonts                                          object   / undefined     yes (`missing`)
```

**Five of six were absent, and every one of them was measured against Chrome by this loop before this
audit ran.** The map is fed by external lists — Interop, Baseline, the Blink use-counter dump — and
those are excellent at *"what does the world think matters"* and structurally blind to *"what did we
just measure ourselves."* A probe finding has no filing path, so it lives in a journal entry and a
NEXT list until someone re-reads them.

That is the tick-42 principle (*"raising what the instrument can SEE outranks fixing what it already
sees"*) pointing at the ledger instead of the engine: **the oracle's ceiling binds the MAP too.**

**ADDED (4 rows, 395 → 399+):** `caretRangeFromPoint`/`caretPositionFromPoint` · CSS Typed OM ·
container-query length units · `@container` cascade order (as a *defect* row, since container queries
themselves are gated). `elementsFromPoint` needed no row — it landed at t729 before this audit.
Every one carries its Chrome measurement and, where known, the reason it is hard.

**CORRECTED:** nothing this cycle. #44's `lh`/`rlh` correction was built at t722–723 and the row is
now `gated`.

**RE-RANKED, with the reason written down so it is not re-derived:** Typed OM is ranked **low**
despite being a real absence, because it has a universal string-parsing fallback that every library
still ships — absence degrades to *slower*, not *broken*. `document.fonts` was ranked high for the
opposite reason and built at t730. **The discriminator is not popularity, it is what happens on
absence: a THROW, a HANG, or a fallback.**

**WHAT WE HAD BEEN WRONG ABOUT:** that a clean reconciliation means a clean map. Five measured gaps
were sitting in journal entries the whole time.

**Standing note, eleventh audit running:** `map-reconcile.sh` searches `engine agent tests` and not
`shell/`, so the eight gates living as `#[test] fn` in `shell/src/media.rs` and `shell/src/audio.rs`
remain invisible to it. Harness-owned; reported, not touched.

## Audit #46 — tick 743 (2026-07-29)

**Sources (searched, not recalled):**
- <https://webkit.org/blog/17818/announcing-interop-2026/>, <https://web.dev/blog/interop-2026>,
  <https://github.com/web-platform-tests/interop/tree/main/2026> — the 20 focus areas + the four
  investigation efforts (accessibility testing, JPEG XL, mobile testing, WebVTT).
- <https://web.dev/baseline/2026> + the monthly digests (Jan/Mar/Apr/May/Jun 2026) — what crossed
  into Baseline this year: `:active-view-transition`, `font-family: math`, `contrast-color()`,
  `field-sizing`.
- <https://ladybird.org/newsletter/2026-01-31/>, <https://ladybird.org/newsletter/2026-02-28/>,
  <https://ladybird.org/newsletter/2026-04-30/> — **the productive source this cycle.** What an
  independent engine had to build, in the order it chose.
- **This loop's own measurements, t741–t743** — the second productive source, and the one the
  protocol still does not name.

**THE EXTERNAL RECONCILIATION IS CLEAN FOR THE SEVENTH AUDIT RUNNING.** Every Interop 2026 focus
area, every investigation effort, and every 2026 Baseline crossing already has a row (`anchor
position`, `view transition`, `WebTransport`, `popover="hint"`, `:open`, `attr()`, `field-sizing`,
`contrast-color()`, `:active-view-transition`, JPEG XL, WebVTT — checked by grep, not by memory).
That leg of the protocol has stopped paying, and audit #45 already said why: the map is fed by lists
of *what the world thinks matters* and is structurally blind to *what we just measured ourselves*.

### ⚠⚠⚠ THE FINDING: AN AGGREGATE ROW CANNOT GO RED OR GREEN

Audit #45 found five measured gaps with **no row**. This audit asked the next question — *why did
they have nowhere to go* — and the answer is on the map in the map's own words. The row
`css / SVG filters, patterns, and SMIL animation` describes its own neighbours like this:

> *"rows 16/166 carry ONE flat `partial` for all of SVG — inline, `<img>`, namespaces, filters,
> patterns and SMIL together"*

That sentence has been sitting in the receipt column since **tick 602**, filed as context. It is not
context; it is the defect. **A capability recorded as one flat `partial` over a whole format has no
state that any measurement can change.** Every finding lands inside it and it still reads `partial`
afterwards — so the ledger's status column is *satisfied in advance*, and the only place a specific
result can live is a journal NEXT list, which is exactly where these five were:

```text
  MEASURED against Chrome, t741-t743                      row in the map?   verdict
  <svg viewBox> is a RATIO not a SIZE                          NO            landed t742
  <use href="#icon"> across two <svg> elements                 NO            landed t743
  SVG <text>: y is the BASELINE, ours is the box TOP           NO            open, 6px
  <symbol>/<defs> content: Chrome 0x0, we emit nothing         NO            open
  cross-<svg> url(#id) for fill/clip-path/mask/filter          NO            UNMEASURED
```

Two of those **shipped, with RED-proven gates, into a map that still says `partial` for SVG.** A
capability the loop has proven and cannot find is indistinguishable, to every ranking instrument it
owns, from one it never built. This is the project's own recurring shape — *a number's NAME is not
its definition*, *a `works` row whose receipt tested one property* — one level up: **a row's SCOPE
is part of its assertion, and a wide name over a narrow (or absent) measurement is a lie with a
true word in it.**

⚠ And the fifth line is the one that matters most, because it is the mechanism t743 proved broken
and fixed **for one property only**. `external_use_defs` injects defs for `<use>`; `fill="url(#g)"`,
`clip-path`, `mask` and `filter` still dangle across the same serialisation boundary by the same
construction. It is filed `unknown`, not `missing`, because nobody has run it — the honest absence
is of a *measurement*.

### THE ONE THING THE WORLD KNEW THAT WE DID NOT

Ladybird's 2026 newsletters, read for *what did they have to build*: **"inline flex or grid
containers now derive their baseline from their child's first line box."** No row, no gate, never
measured here. It is the same class as t695–t701's §10.8.1 inline-block baseline work — which was
spec-correct, Chrome-exact 8/8 on its fixture, and **still a refused TRADE** (blog.rust-lang
383→13, desitales2 SHAPE 61.5→57.2). So its price is known to be real before the first line of it
is written. Filed `unknown`. Their `dominant-baseline`-for-SVG-text item independently corroborates
that our measured 6px `<text>` offset is a distinct capability and not a rounding residue.

**ADDED (6 rows, 400 → 406):** the five SVG sub-capabilities above, each carrying its Chrome
measurement or an explicit *unmeasured*, plus inline-flex/grid baseline. Ranked by **what happens on
absence**, not popularity (I4): `<text>` baseline ranks ABOVE filters/patterns because absence is a
*wrong position on text that is present*, which the fidelity instrument scores, where a missing
filter is a decoration it does not.

⚠ One of the new rows exists to **STOP a fix**: `<symbol>`/`<defs>` content is 0x0 in Chrome and
absent here, and the ledger ranks absence as MISSING_BOX — so part of the `Ccd7f MISSING BOX:
<path>` mass (34 sites / 1658 hits) is content that **must not be drawn**. A row whose job is to
prevent work is still a row.

**CORRECTED (2 rows):** `SVG (inline + <img>)` and `SVG filters, patterns, and SMIL animation`. The
first now says outright that it is an umbrella and that the verdicts live in the sub-rows; the second
is **rescoped to the three things it names** instead of standing in for the whole format. Neither
status changed, and that is deliberate — this was wrong about SHAPE, not about state.

**WHAT WE HAD BEEN WRONG ABOUT:** that a missing row is a *gap in coverage* of the map. It was a gap
in the map's **resolution** — the row existed, was too wide to hold an answer, and had been
confessing that in its own receipt text for 141 ticks.

**Standing note, twelfth audit running:** `map-reconcile.sh` searches `engine agent tests` and not
`shell/`, so the eight gates living as `#[test] fn` in `shell/src/media.rs` and `shell/src/audio.rs`
remain invisible to it. Harness-owned; reported, not touched.

## Audit #47 — tick 754 (2026-07-30)

**Method, and it is a deliberate departure.** The previous audits reconciled the map against what the
*world* names (MDN/caniuse/spec indexes). This one reconciles it against what a **freshly measured
representative corpus** turned out to need: the first CrUX sweep (t752, 200 sites) plus the fixture work
under ticks 749–754. The reason is that this session found three real capability facts by *measuring*,
and none of the three was on the map — which is exactly the failure mode this instrument exists to catch,
arriving through a door it had not been pointed at.

### ADDED

| class | capability | status | why it is here |
|---|---|---|---|
| doc | **`font-size-adjust`** | `missing` | Measured against live Chromium: `16px serif; font-size-adjust: ex-height 0.53` is **738.75×21** in Chrome and **640×18** here — Chrome scales the used size 16px → 18.46px; with `EB Garamond`, 853.61 vs 649. `grep` over `engine/{css,layout,text}/src` finds **no parse, no computed value, no application**. It scales the *used font-size*, so every `em`/`ch`/`rem` and every `line-height` beneath it is wrong on a page that declares it. |
| doc | **`system-ui` / `ui-*` generic families** | `gated` | Was silently aliased to the sans generic (tick 749), which is both the wrong face *and* a short-circuit of the whole `font-family` stack. Now gated. |

### WHAT WE HAD BEEN WRONG ABOUT

1. **The map had no row for the font-family generics at all** — not `missing`, not `unknown`: absent. So
   the fact that `system-ui` resolved to the wrong face was not a known gap being deferred, it was
   invisible, on a property that sets the body text of most of the modern web.
2. **`font-size-adjust` is unimplemented and nothing said so.** It is the sort of property that is easy to
   read as cosmetic and is not: it changes the *used font-size*, so it is a `dy` multiplier on the whole
   document.
3. ⚠ **The audit's own ranking rule saved a tick from being spent.** `font-size-adjust` explains
   `matklad.github.io` completely — cov 1.000, shape 0.004, 807 nodes — and that is a *hit-count*
   argument. Measuring the population first (scan of the 24 worst-shape sites' HTML + stylesheets) gave
   **1 of 24 sites**. Recorded as `missing`, **not built**. The ledger ranks by DISTINCT SITES precisely
   to refuse the seductive single-site story, and it worked.

### NOT ADDED, AND WHY

No new rows from the CrUX tail's failure modes yet. The t752 baseline's in-scope failures are dominated by
`thin-overlap` (16), `shell-only` (12) and `render-failed` (11), which are *instrument* classifications
rather than named capabilities — turning those into map rows needs the per-site cause, and the mechanism
ledger that would supply it was **invalidated the same tick** (see below). Deferred honestly rather than
guessed at.

### ⚠ STANDING CORRECTION THIS AUDIT MUST RECORD

`docs/loop/CLUSTERS.md` — the cluster ranking STATUS.md calls *the priority ledger* — **is invalidated as
of tick 754** and needs a re-crawl before any tick is chosen from it. The oracle keyed paths by class
signature, so one differing ancestor booked entire subtrees as `missing box`; measured, ~68% of reported
divergences were phantom (2750 → 892 over three sites). Any re-rank done from that file before the
re-crawl is a re-rank of an artefact.

**Sources:** live Chromium 16px fixtures (`/tmp/adv*.html`, `/tmp/fsa.html`, `/tmp/flex.html`),
`fc-match`, `docs/loop/SWEEP-t752-rows.tsv`, and the site stylesheets of the 24 worst-shape corpus sites.

## Audit #48 — tick 764 (2026-07-30)

**Method.** Same door as #47 — reconcile the map against what a *measurement* just turned up, not against
what the world names. The trigger was the CrUX sweep's `reading_order` column: the two worst sites in the
whole corpus (`mobile.ir` **874**, `ta3lemkonline.com` **817**) are both RTL, so I built a `<html dir=rtl>`
fixture, measured it against live Chromium, and then went to the map to see what it claimed.

### ⚠ THE FINDING: A GATE NAMED FOR A SCRIPT COVERED ONE PRIMITIVE OF SEVERAL

The map's row read:

```text
doc   bidi (Arabic/Hebrew)   RTL web is unreadable   gated   G_BIDI_BASE   tick 215
```

`gated` — i.e. done, with a receipt. What `G_BIDI_BASE` actually asserts is the **paragraph base
direction inside `engine/text`**: that a run of Arabic shapes and reorders right-to-left. That is one
primitive. `direction: rtl` is a *layout* property, and the fixture found three more, each of which
misplaces boxes on every RTL page:

| markup (`<html dir=rtl>`, 600px body in a 1200px viewport) | Chrome | ours |
|---|---|---|
| flex row of three 100px items (x within the row) | **500 / 400 / 300** | 0 / 100 / 200 ❌ |
| `body{width:600px}` — the block's own x | **600** | 0 ❌ |
| `<li>` inside a default `<ul>` | **x=0 w=560** | x=40 w=560 ❌ |
| two `<span>`s on one line | 548 / 575 | 548 / 575 ✅ |
| `<p>` of mixed Arabic + Latin | matches | matches ✅ |

The two ✅ rows are why this survived: the *text* half — the half the gate is named for, and the half a
human eyeballing a screenshot notices first — is correct. Everything the gate does not name is wrong.

**The row is DOWNGRADED `gated` → `partial`**, its receipt now says what it covers, and the three
measured-missing primitives are individual map rows (block inline-start edge, list/UA logical padding,
grid column order) so none of them can be re-discovered as a surprise.

### THE RULE THIS ADDS

> **A capability row whose name is a SCRIPT, a LANGUAGE or a REGION is a suspect row.** "bidi
> (Arabic/Hebrew)" names a population, not a mechanism, and a population needs *every* mechanism it
> touches — text shaping, inline reordering, box placement, logical padding, flex/grid axis order. A row
> named for a mechanism (`flex-wrap on a column container`) can be gated by one test honestly; a row named
> for a population cannot, and its `gated` is an average pretending to be a verdict.

Same family as audit #45's ranking rule and #47's "the map had no row at all": the failure is never that a
row is wrong, it is that a row is **coarser than the thing it is standing in for**.

### ADDED / CHANGED

| class | capability | status | note |
|---|---|---|---|
| doc | bidi (Arabic/Hebrew) | `gated` → **`partial`** | receipt now enumerates built vs missing |
| doc | RTL: block box inline-start edge | **`missing`** | Chrome 600, ours 0 |
| doc | RTL: `ul`/`ol` UA padding is inline-start | **`missing`** | Chrome x=0, ours x=40 |
| doc | RTL: grid column order reverses | **`missing`** | named when the flex half was built, not later |
| doc | %-height child of an indefinite-height column flex container | **`missing`** | tick 762's control residue (9 vs 18) |
| doc | `-webkit-box-orient:horizontal` (legacy flex row) | **`missing`** | tick 763's deliberate narrowing |

### NOT ADDED, AND WHY

The `overlap` invariant's worst sites (`razaoautomovel` 71, `puentedemando` 18) were not opened. `overlap`
has no fixture yet and I will not guess a capability row from a number I have not reproduced — that is
exactly the "rank from the artefact" mistake check #60 caught. It is the next fixture to build.

**Sources:** `/tmp/rtl.html` measured against live Chromium 1200×800, `docs/loop/SWEEP-t758-rows.tsv`,
`engine/layout/src/taffy_tree.rs`, `engine/text/tests/g_bidi_base_direction.rs`.

## Audit — tick 775, 2026-07-31

**Sources checked (external, this session):** Interop 2026 focus areas —
[web.dev](https://web.dev/blog/interop-2026), [WebKit](https://webkit.org/blog/17818/announcing-interop-2026/),
[Mozilla Hacks](https://hacks.mozilla.org/2026/02/launching-interop-2026/),
[Igalia](https://www.igalia.com/news/interop-2026.html),
[web-platform-tests/interop 2026 README](https://github.com/web-platform-tests/interop/blob/main/2026/README.md).

### What the world named, and what our map already said

Interop 2026's 20 areas: **Anchor Positioning, advanced `attr()`, View Transitions (now including
cross-document), Container Queries, Subgrid, the `zoom` property, WebRTC**, plus investigations into
accessibility testing, **JPEG XL**, mobile testing infrastructure and **WebVTT**.

**Every one of them is already on `CONSTELLATION.tsv` with a verdict** — `zoom` gated
(`G_ZOOM_AND_PROBE_PINS`), container queries gated, View Transitions gated with cross-document `partial`,
`attr()` `partial`, anchor positioning and subgrid `missing`. JPEG XL, WebVTT and WebRTC are on the
explicit death-tail cut line, which is a *decision*, not a gap. **No phantom, no rot, nothing added from
the external list.** That is a real result and the first clean one — but "an audit that finds nothing is
a suspicious audit", so the rest of this entry is what the audit found by looking somewhere else.

### ⚠⚠⚠ WHAT WE WERE WRONG ABOUT — and it is not on the capability map at all

**1. The board's own ranking of the scorability lever is off by ~7×.** The M1 priority order prices the
unscored cohort as "~30 of 48 are function/boot problems". Measuring all 22 boot-broken t767 origins
(t773) found **4**. Eleven are `shell-only` — *the ORACLE rendered only N elements* — and on seven of
those our coverage of what Chrome drew is **100%** (`forums.moneysavingexpert` 9 paths / 0 missing;
`booking.directferries` 3/0). I read the Chrome-side PNGs rather than the labels: moneysavingexpert is a
header over an **empty body in Chrome**; directferries is a **blank grey page in Chrome**. So the "63%
scorability ceiling" is substantially a corpus/oracle-validity fact, not an engine ceiling. Recorded, not
acted on — moving the denominator is the most self-serving edit available and t771 already refused its
own version of it.

**2. The burndown and the defect population are not in the same frame.** t774 (every stylesheet
mojibake'd) and t775 (out-of-flow `::before` pushing its own text) are both real, Chrome-differential,
pixel-verified fixes on the cohort the ranking itself named — and **both moved the metric by exactly
zero**. Shape scores ELEMENT geometry; both defects live *inside* an element's box. A metric used to RANK
work silently deprioritises everything in its blind spot, and the near-bar pages it ranks are exactly
where those defects sit. **This is the map-of-the-map correction this audit exists to make**, and it is
not something the capability list could ever have shown.

**3. Four capability gaps found by MEASUREMENT while probing, now added to `CONSTELLATION.tsv`:**

| added | status | how it was found |
|---|---|---|
| `getComputedStyle(el, '::before')` pseudo styles | missing | returned `undefined` while building a t775 probe |
| `ResizeObserver.takeRecords()` | missing | one rule, two implementations — `IntersectionObserver` has it, `ResizeObserver` never did |
| out-of-flow pseudo VERTICAL insets | partial | named residue of t775's horizontal half |
| `DOMStringMap` | missing | t773 residue: `dataset` works, but no distinguishing shape exists to write a truthful `instanceof` predicate over, so it is deliberately left absent rather than guessed |

### Re-rank

None of the four is larger than the current line of work. The material re-rank is **finding 1**: the
throw-killer leg is a **4-site** lever, not a 30-site one, so it should stop being priced as the way to
break the M1 ceiling. Finding 2 says the near-bar shape leg needs its *own* correction — rank by what
the metric can see, or accept that some of the work it surfaces will never show up in it.

## Audit #49 — tick 776 (2026-07-31)

**Method.** The same door as #47 and #48 — reconcile the map against what a *measurement* just turned
up. The trigger this time is new: the running CrUX sweep's **own stderr**. Every uncaught page throw is
already routed through `__reportError` and printed (`uncaught (reported): …`), so a sweep that nobody
asked to harvest errors is producing a ranked throw-class worklist as a by-product. Aggregating it over
the first ~75 sites gave, among others:

```
  1  TypeError: b.createRange is not a function                   (Google CSE dynamic.js)
  1  TypeError: b.createRange … / t.getClientRects is not a function
  2  Minified React error #446                                    ("currentResources" was expected to exist)
```

`b.createRange` is a **direct call on a document that is not the singleton**, which sent me to the map.

### ⚠ THE FINDING: A GATE THAT WAS FAITHFUL TO ONE LIBRARY AND BLIND TO THE PLATFORM

Three rows claim this territory, all `gated`:

```text
dom  NodeIterator / TreeWalker with the filter protocol              gated  G_TRAVERSAL
dom  detached Document from DOMImplementation + pre-insertion valid  gated  G_DOM_IMPL, G_CREATED_DOCUMENT_IS_REAL
app  third-party library boot (… DOMPurify …)                        gated  G_NODE_TYPE_ENUMERATION, G_SECOND_DOCUMENT_IS_REAL
```

All three are green, and **`Document.prototype` carried none of the seven `create*` methods**. A probe:
`Document.prototype.createRange === undefined`, and 19 members sitting as own properties of the one
`document` object every fixture in this tree builds.

The reason the gates missed it is precise and generalisable. `G_SECOND_DOCUMENT_IS_REAL`'s central
claim is

```js
document.createNodeIterator.call(b.ownerDocument || b, d.body, …)
```

and the gate's header explains — correctly — that this expression is *transcribed from the library
rather than invented*. It is: DOMPurify destructures `createNodeIterator` off the **original** document
and `.call`s it with the parsed root's document as `this` (verified against the shipped
`dompurify@3.2.4`, lines 350–352 and 856–857, not from memory).

So the transcription is faithful, and it takes the **function from the singleton**, supplying only the
receiver. It exercises the ALGORITHM over a second document and never performs the LOOKUP on one. **It
passes for exactly as long as `otherDoc.createNodeIterator` is `undefined`.**

The corollary corrected this tick's own commit message before it landed: **DOMPurify was never broken
by this defect.** The story "the sanitiser everyone runs was dead" was the one I had already written
into the journal, the wiki, the pattern ledger and the gate header. The audit is what refuted it.

### THE RULE THIS ADDS

> **If a gate obtains the thing under test from somewhere other than the subject, it has not tested the
> subject's surface.** `X.method.call(subject, …)` proves the algorithm; only `subject.method(…)`
> proves the lookup. And the trap has a specific flavour: *transcribing the real library's idiom* is
> normally the strongest form of evidence this project has (t633-649: fetch the REAL shipped bundle and
> run its boot path), so the substitution felt **more** rigorous, not less. A library's defensive idiom
> is a statement about that library, not about the platform.

Same family as #48 (a row coarser than the thing it stands in for) and #45 (rank by what happens on
absence), with a new edge: here the row was not coarse and the gate was not lazy — it was *aimed at one
caller*.

### ADDED / CHANGED

| class | capability | status | note |
|---|---|---|---|
| dom | `Document.prototype` carries the `create*` family | **`gated`** (new row) | G_DOC_PROTOTYPE, tick 776. Seven methods promoted off the singleton; ownership asserted, not just presence |
| dom | `Range.prototype.getClientRects` / `getBoundingClientRect` | **`missing`** | absent (`typeof … === 'undefined'`). Live throw: `t.getClientRects is not a function`, agoda.com. The primitive under every tooltip, highlighter, caret and text-measurement library. ⚠ We hold NO per-text-run geometry, so this is not a shim — name the bound before building |
| app | React Float resource bookkeeping (`"currentResources" was expected to exist`) | **`unknown`** | React error #446, **2 sites** in the first 75 of the t776 sweep — the highest-frequency single throw in the harvest. React DOM keys hoisted resources (`<link rel=preload>`, stylesheets) off the DOCUMENT; an unstable `ownerDocument`/`getRootNode()` identity would produce exactly this. NOT diagnosed — the code is confirmed absent from the map, not confirmed broken here |
| — | (instrument) the sweep's stderr **is** an unhandled-error harvester | **noted** | STATUS's meta-instrument #1 has effectively existed for ticks, unaggregated. Ranking leg-1 throw-killers by distinct sites needs only an aggregation over a file that is already being written |

### NOT ADDED, AND WHY

No external search this round, and that is a deliberate departure recorded rather than hidden: the
board's current mandate is leg-1 **scorability** — get the ~48 non-rendering in-scope sites to boot —
and the corpus is producing a *measured, ranked, first-party* worklist for that faster than any
external list can. Interop/Baseline reconciliation is owed and is the next audit's job (#50), before
the leg-2 shape work resumes.

## Audit #50 — tick 787 (2026-07-31)

**Method.** The same door as #47–#49 — reconcile the map against what a measurement just turned up —
but this time the measurement is the cheapest one available and the loop had not been using it:
**write a four-line fixture and ask Chrome for the number.** Two ticks in a row (t785 nested `@media`,
t787 form controls) found a shipped, mapped, gated capability that was wrong by an amount no gate could
see, and in both cases the reference could simply have been asked.

### ⚠ FINDING 1 — THE ROW WAS COARSER THAN THE FEATURE, AGAIN (and the residue was already on the map)

`doc · CSS nesting (native &) · gated · G_CSS_NESTING · "tick 124: MEASURED — Stylo backs native
nesting (& descendant + bare &)"`.

CSS Nesting has two halves. A nested **style rule** (`& .c {}`) has its own selectors and worked since
t124. Declarations written **directly inside a nested group rule** have none — the spec wraps them in
an implicit `& { … }`, Stylo materialises that as `CssRule::NestedDeclarations`, and the rule-index
walker's `_ => {}` dropped them whole:

```css
article { max-width: 423px; @media (min-width: 1018px) { max-width: 974px } }
```

The row's own receipt names the two forms that were tested. **It is accurate about what it measured and
silent about what it did not** — the #48 shape (a row coarser than the thing it stands in for), third
sighting.

⚠ **And the map had already written down a sighting of it, filed under a different row.** The
`container queries` row's residue list ends with *"style-rule-NESTED @container (&-relative) skipped"*.
That is the same symptom, recorded at t379 as one at-rule's residue rather than as a fact about nested
group rules in general. **A residue noted on the row of the capability you were building is invisible to
the row of the capability it actually belongs to.**

### FINDING 2 — WHAT THE FIX DID AND DID NOT REACH, measured after landing

Probe (`#c` inside a 600px `container-type: inline-size` wrapper), Chrome vs ours after t785:

```
  nested @media   (matching)      Chrome 300   ours 300   ✓
  nested @media   (non-matching)  Chrome 100   ours 100   ✓  (the negative control)
  nested @supports                Chrome  ok   ours  ok   ✓
  nested @layer   (only decl)     Chrome 222   ours 222   ✓  (was 100 before t785)
  nested @container               Chrome 400   ours 100   ✗  STILL SKIPPED
```

`@container` is the exception because it does not come through the same door: it is a **source-lifting
supplement** (`extract_container_blocks` reads blocks out of the stylesheet TEXT, because Stylo's servo
build cfg-drops the at-rule), and a supplement that scans for top-level blocks cannot see one nested
inside a style rule. Two mechanisms for one syntax means a fix to one of them proves nothing about the
other — new row added, `missing`, with the probe recorded.

### FINDING 3 — A NEW DEFECT THE FIX MADE VISIBLE: `@layer` HAS NO PRECEDENCE

While probing nested `@layer` I measured the ordinary one:

```css
#h { width: 100px }
@layer L { #h { width: 333px } }        Chrome 100   ·   ours 333
```

**Unlayered author declarations beat layered ones regardless of document order** — that is the whole
point of a layer, and we flatten layers into document order (`CssRule::LayerBlock` recurses and keeps
nothing). This is *independent of t785*: it reads the same for a top-level layer, so it has been true
since layers were walked at all. The cascade's sort key needs a layer term between origin and
specificity. Added as `missing`, with both directions measured — because the complementary case (a
declaration that exists ONLY in a layer) is now RIGHT, and a row that recorded only the failure would
have sent the next tick to re-derive that half.

### FINDING 4 — A CALIBRATION CONSTANT IS AN UNMEASURED CLAIM WEARING EVIDENCE'S CLOTHES

`<input>`'s intrinsic width shipped as `size * 8.0 + 13.0` under a comment reading *"the same
approximation Chrome's own default ends up at (`size=20` → ~173px)"*. Chrome ends up at **205**. The
slope was right; the constant was 26px short on every text field on the web that the author did not
size. Nothing could catch it: it never throws, it reads as evidence, and the error is invisible without
running the reference.

> **THE RULE THIS ADDS: any number in this engine that claims to match Chrome must carry the command
> that produced it.** `G_UA_BLOCK_MARGINS` already does this (its header pastes the `--dump-dom`
> output); the form-control constants did not, and were wrong for as long as they existed.

### ADDED / CHANGED

| class | capability | status | note |
|---|---|---|---|
| doc | CSS nesting (native `&`) | `gated` (receipt CORRECTED) | the nested-group-rule half was dropped whole until t785; nested `@container` still skipped |
| doc | form-control INTRINSIC metrics (`size`/`cols`/`rows` + the control's own font) | **`gated`** (new row) | `G_FORM_CONTROL_METRICS`, t787 — three defects in one measured table |
| doc | `<select>` reserves the dropdown arrow in its intrinsic width | **`missing`** (new row) | exactly 17px short, both a long and a one-character option — a constant, not a text-measurement difference |
| doc | `@layer` PRECEDENCE (unlayered beats layered) | **`missing`** (new row) | Chrome 100, ours 333; independent of t785, true since layers were walked |

## Audit #51 — tick 798 (2026-07-31)

**Method.** A new door, and it is the one this whole session ran on: **the map is measured against
Chrome with four-line fixtures**, not against a capability list. Nine probe areas since #50, each one
`google-chrome --dump-dom` versus `manuk-wpt boxes` on the same file. The audit's job here is to record
what the probes said about the MAP — including, and especially, the areas that came back clean.

### THE CLEAN AREAS ARE MAP INFORMATION, NOT A NON-RESULT

Five areas were probed and found Chrome-exact. Each is a row that could have been carrying an
unmeasured `?`, and each retires a hypothesis the burndown's numbers actively invite:

| area probed | cases | verdict |
|---|---|---|
| **CSS custom properties** — `var()`, fallback, var-in-`calc()`, scoped redefinition, two vars in one `calc()` | 6 | **exact** — the highest-usage modern CSS feature there is |
| `gap` (flex + grid), `aspect-ratio`, `calc(100% - Npx)` | 7 | **exact** |
| **Image intrinsic sizing** — intrinsic, `width`/`height` attrs with ratio, CSS width, `max-width:100%` over an attr, `width:100%;height:auto` | 6 | **exact** |
| **Tables** — `border-collapse`, `table-layout:fixed`, `colspan`, `rowspan`, `border-spacing`, auto width distribution | 13 | **exact** (≤3px, all of it the collapsed-border edge convention) |
| **Flex/overflow** — `min-width:auto` floor, `flex-wrap`, `overflow:hidden` not shrinking a child, `min-width:0`, blown `1fr` tracks, oversized `flex:0 0 auto` | 10 | **exact** |
| **Text advance widths** — 9 strings × 5 font stacks | 45 | **exact within 0.5px** |

That last row deserves its own line: *"our font metrics are systematically narrow"* is the hypothesis
the h-overflow and wrap-divergence numbers most invite, it would have been a subsystem to chase, and
one fixture killed it.

### WHAT THE SAME METHOD FOUND — six defects the map called covered

`CSS nesting` (the group-rule half), form-control intrinsic metrics, the `<select>` arrow, `@layer`
precedence, the solidus line break, the float containing block (twice: limit, then origin), flex/grid
`order`, the inline-block baseline, and the flex percentage height. **Every one of them sat under a row
the map already had.** #48's shape — *a row coarser than the thing it stands in for* — is now the
dominant finding of three consecutive audits, and the correction is mechanical rather than editorial:
**a row's receipt should name the CASES measured, because that is exactly the set the next reader will
assume is covered.**

### ⚠ THE FINDING WITH THE LONGEST LIFE: A FIXED AXIS AND AN UNFIXED MIRROR

t798's percentage-height bug is *the same defect the width axis fixed at tick 14*, and
`taffy_item_width` — the fix — has sat beside the unfixed block axis for 784 ticks with a comment
naming the failure mode. t770 recorded the identical shape for `box-sizing` (applied on the main path,
never on the FLOAT path). **Two sightings make it a rule: when a fix is written for one axis or one
variant, grep for the mirror before recording the class as closed.** The grep is one word long.

### ADDED / CHANGED

| class | capability | status | note |
|---|---|---|---|
| doc | percentage `height` on a flex/grid ITEM resolves once | **`gated`** (new row) | `G_FLEX_PERCENT_HEIGHT`, t798 — was squared; the width axis's tick-14 mirror |
| doc | CSS custom properties (`var()`, fallbacks, scoped redefinition, in `calc()`) | **`gated`-by-measurement** | 6 cases Chrome-exact (t798 probe); previously unmeasured |
| doc | `<img>` intrinsic sizing + `max-width:100%` over a dimension attribute | **verified exact** | 6 cases (t798 probe) |
| doc | text advance widths vs Chrome | **verified exact within 0.5px** | 45 measurements (t795 probe) — retires the systematic-metrics hypothesis |

## Audit #52 — tick 808 (2026-07-31)

**Method.** Same door as #51 — Chrome measured with small fixtures — but this window the probes went
at *properties and computed values* rather than at box arithmetic, and the map turned out to be
wrong in a **shape it does not have a column for**.

### ⚠⚠⚠ THE MAP TRACKS "IS IT THERE?" AND EVERY DEFECT THIS WINDOW WAS "IS IT APPLIED?"

Nine engine defects landed across t799–t809. Grouping them by what an honest capability map would have
said *before* each was found:

| defect | what the map said | what was true |
|---|---|---|
| `text-align: justify` (t805) | present — it **parses**, computes, inherits, and `getComputedStyle` reports it | reached layout and fell through a `_ => 0.0` wildcard. **Rendered as `left` for the engine's whole life** |
| `letter-spacing` on the inter-word space (t806) | present — and the word's own width was correct | the space was the one character on the line that never got it |
| vertical padding on an inline (t808) | present — horizontal padding worked, `inline-block` worked | the box never grew, so every padded pill painted at half height |
| anonymous-block inheritance (t799) | not a capability at all | `text-align` + strut silently dropped for any container mixing inline and block children |
| `max-width` + auto margins (t801) | both present and both correct in isolation | §10.4's *re-run* was missing, so `.container{max-width;margin:0 auto}` rendered flush left |
| control `line-height` (t802) | `font: -webkit-small-control` was "done" at t787 | the shorthand's third property was never copied |
| `getComputedStyle(source).display` (t809) | `<source>` "handled" — it correctly draws nothing | Chrome computes `inline`; we said `none`. Right box, wrong answer |

**Every one of these is a capability the map would mark ✅.** They parse, they compute, they inherit,
they report, and they do not do the thing. `G_CAPABILITY` asserts 42 ledger claims and could not have
caught a single one, because all 42 ask *does the surface exist*.

**The missing column is `applied`,** and it is not the same as `present`. The cheapest available
instrument for it already exists and is not being used for this: a **positional** fixture. Every
defect above is invisible to a presence check and visible in one Chrome box diff.

### WHAT WE HAD BEEN WRONG ABOUT

1. **"The UA sheet is a solved area."** It produced three of this window's defects (t802 control
   `line-height`, t809 the `display:none` list, and t799's anonymous-block strut is adjacent). The
   `display:none` list was **half wrong** — `source`/`track`/`area`/`noscript` are `inline` in Chrome
   — and nobody had ever measured it; it was written from memory of what "should not render".
2. **"`position:absolute` works."** `<div style="position:absolute">Menu</div>` measured **0×0**
   (t803). Every probe that had ever exercised abspos used an *element* child, which is the shape a
   test-writer reaches for and the shape the bug does not fire on.
3. **A guard I wrote myself was inert** (t809). `never_rendered()` was load-bearing in my head and did
   nothing in the tree; deleting it changed no number and *improved* wikipedia's coverage to 1.000.
   The map has no column for *"machinery whose necessity was never tested"* either.

### ⚠ THE INSTRUMENT'S OWN FRAME, CORRECTED TWICE THIS WINDOW

- **A paired band still confounds engine with site drift** (t800). The fix is the **old-binary
  control** — rebuild the previous tree and re-measure *now*. It changed the verdict three times in
  nine ticks: land (t803), **refuse** (t804), clear as drift (t807). The observer has since wired the
  caveat into `fidelity-progress.sh`.
- **A harness defect disappearing looks exactly like progress** (t807). 48 false `crashed` rows became
  0 and scorability leapt 53% → 79%. `uniq -c` over the unscored-reason column separates the two, and
  nothing in the map required anyone to look.

### RE-RANK

No re-rank: the `applied`-not-`present` class is not a new frontier, it is the *current* one seen more
clearly — it is precisely what the shape metric measures and what the §8 near-bar method finds. Four
M1 crossings this window came out of it. The steer stands.

**Next audit due: tick 818.**

## Audit #53 — tick 818 (2026-08-01)

**Method.** Same door as #51/#52 — Chrome measured with small fixtures — but this window one probe
was aimed by the board's own *marginal-crossing* rank rather than by curiosity, and that changed what
it found. Three engine fixes landed (t815, t816, t817) plus two measurement ticks (t813, t814).

### ⚠⚠⚠ #52 SAID THE MISSING COLUMN IS `applied`. THIS WINDOW SAYS THE ONE UNDER IT IS `COMPARED`.

#52's defects were all *present but never applied*. Two of this window's three are a rung below that:
the property was applied, the arithmetic was right, and the **comparison** that consumed it was wrong.

| defect | what was correct | what actually failed |
|---|---|---|
| orphan `table-cell` (t816) | the cell's width was Chrome-exact; the cascade, the display value, the shrink-to-fit were all right | *"is this box atomic?"* — asked in **two** places, with two copies of the same `matches!` list, both missing the same three variants |
| flex percentage line-break (t817) | every width computed correctly **to within 0.00004px** | `line_length > available` — an exact `>` on `f32`, and Bootstrap's `66.66666667%` is exactly the value that overflows it |
| rowless `display:table` (t815) | the table formatter, which was never the defect | a filter written for *elements*, handed a child list containing **text** |

**t817 is the sharpest thing this window produced and it is not a formula error at all.** No number
was computed wrong. A page stacked because a comparison had no quantum. **Chrome does not compare
with a tolerance — it snaps every length to 1/64 px so the comparisons come out exact**, and that is
a structural choice we had not made anywhere. The map has no column for *"what grid do these operands
live on"*, and nothing in the ledger would ever have asked.

### ⚠⚠ WHAT I WAS WRONG ABOUT, AND THE CONTROLS THAT CAUGHT IT

1. **"The 3px-short boxes are a line-height / half-leading problem."** Adjacent, plausible, and wrong.
   An `inline-block` with **byte-identical content** was already Chrome-exact at `85x20` — so the
   error was one code path, not the strut. **One control turned a symptom into a diagnosis**, and
   without it the fix would have gone into the font metrics, which are already correct.
2. **"The flex wrap bug is the sum exceeding 100%."** Killed by the row that sums to *under* 100%:
   `33.33333333% × 3` still wrapped, because each third rounds **up** in `f32`. I had also blamed
   `flex-wrap`, then the `flex: 0 0 auto` shorthand, then the decimal count. Three wrong causes, each
   retired by one more fixture row. **The wide fixture pays in its ninth row** (I5, again).
3. **"A residue's stated cause is a guess"** (t814's own lesson) held twice more: t815 shipped a
   named residue, and t816's `#c3` is asserted at *our* number precisely so a future fix must change
   that line deliberately rather than inherit a label.

### ⚠⚠ THE INSTRUMENT CORRECTED ITSELF TWICE, BOTH TIMES BY REFUSING A NUMBER

- **A gate caught me carrying a Chrome number ACROSS FIXTURES** (t816). The first draft asserted
  `#ib` at `79x20`, measured on an earlier probe whose text was `"inline block"`; the gate file's
  text is `"cell no table"`. It failed instantly with `got 85.390625x20`. Every number in both new
  gates was then re-measured **from the gate file's own `const HTML`, extracted by regex, not
  retyped** — and t817 promptly made the same class of slip in its *y* values and was caught the same
  way. **A Chrome table is evidence about the fixture it was measured on and no other.**
- **A ratio gate went RED from its DENOMINATOR** (t817). `F2 pipeline large/mid` read `8.11x` against
  a `7.5x` bar on a run where `large` was unchanged (233.92 → 232.72 ms) and `mid` was **17% faster**
  (34.75 → 28.68 ms). A re-run on a quieter box read `6.60x`. A ratio gate exists to divide out
  machine speed, but it only does that when both legs move together. **Nothing was retuned** — the
  gate was re-run, which is the only admissible response to a suspected-noise red.

### ⚠ PART VII, and the harness item that is now three audits old

`scripts/` untouched in t813–t818. Wall-audit #27 was run and recorded: **1661s total, 445s
attributed, 1216s (73%) UNATTRIBUTED** — up from 58% at #26 and named by subtraction at #25. **Three
consecutive audits have produced the same subtraction and no new information**, which is itself the
finding: the four rigor-preserving questions are each questions *about a named section*, and the
largest cost has no name. The self-audit's one open item (verify wall 744s vs the 300s target) is
**explicitly ACCEPTED, not fixed**: every remedy it names — mold/lld, cargo-nextest, workspace-hack,
risk-based gate scheduling — is harness work this agent does not own, and no *aimed* remedy exists for
anyone while three quarters of the time is unlabelled.

### RE-RANK

No re-rank of the steer — but a sharpening. #52 said to reach for a **positional** fixture over a
presence check. That still holds and produced all three fixes. What this window adds is that the
positional fixture must carry **a control that is already correct** (t816's `inline-block`, t817's
four exactly-representable percentage pairs) and **a case that must still FAIL** (t817's `70% + 40%`,
asserted to keep wrapping). Without the first, a symptom names the wrong organ; without the second, a
fix can degenerate into "never break a line" and every assertion still passes.

**Next audit due: tick 828.**

## Audit #54 — tick 828 (2026-08-01)

**Method.** Same door as #51-#53 (Chrome, small fixtures), but this window's centre of gravity moved:
of five landed ticks, **two were about the INSTRUMENT** and one of those was the highest-leverage
thing in the window. Covers t823-t827.

### ⚠⚠⚠ #53 SAID THE MISSING COLUMN IS `COMPARED`. THIS WINDOW SAYS IT IS `RE-COMPUTED`.

#52's defects were *present but never applied*; #53's were *applied but wrongly compared*. Three of
this window's four are a rung further out again: the value was computed **correctly, by the right
component, against the right reference** — and then a second component computed it **again**, against
the wrong one.

| defect | who got it RIGHT | who did it AGAIN |
|---|---|---|
| `max-width: <pct>` on a flex item (t823) | taffy — clamped against the real containing block | `layout_block`, against the SLOT ⇒ the percentage squared |
| a flex item's MARGINS (t823) | taffy — slot positioned with margins out of the line | `border_x = x + ml` ⇒ every margin doubled |
| `max-height: <pct>` (t827) | taffy, again | the block-axis clamp, against `pch` = the slot |
| the sweep's `crashed` rows (t824) | the watchdog — wrote an honest `timeout` row | the parent's re-spawn cap, re-filing the same event as Bar-0 `crashed` |

**The shared shape is a GUARD WITH ONE CONSUMER.** `taffy_item_width` was introduced ~120 ticks ago
with exactly the right sentence — *"a flex/grid item's width was already decided by taffy, do not
resolve it a second time"* — and applied to `width` alone, while the clamp ten lines below and the
margins twenty lines above went on doing it. **The rule was right; its coverage was one property
wide.** The map has no column for *"who else reads this value"*, and nothing in the ledger would ask.

### ⚠⚠ TWO MASKS, AND THEY ARE DIFFERENT KINDS OF MASK — THIS IS THE WINDOW'S REAL FINDING

Why did a defect this wide survive 120 ticks? Because of the two things that hid it, which yield to
different techniques:

1. **An input that cannot express the bug.** A `px` clamp re-applied to the slot is a no-op;
   `min-width: <pct>` of a slot can never exceed the slot. Of four min/max × px/pct combinations,
   exactly **one** is observable. *Yields to input variation* — a wider fixture finds it.
2. **A later write.** (t827.) A percentage `max-height` *still* hides unless the item also carries a
   percentage `height`, because with `height: auto` the box is overwritten by `extract_placed`'s slot
   adoption **after** the clamp runs. **The wrong arithmetic produced the right box.** *Does not
   yield to input variation at all* — only to asking why a row that SHOULD be wrong is right.

⚠ **#2 is a class this project has not named before**, and it is invisible to every test that checks
the final box. It is now in the gate as `#v1`, a row that passes both before and after, kept
specifically to document the mask.

### ⚠⚠ WHAT I WAS WRONG ABOUT, AND THE CONTROLS THAT CAUGHT IT

1. **"The sweep is dying to a mozjs teardown crash."** Carried by t820 AND t821, read off
   `pthread_mutex_destroy failed: Device or resource busy` — the last line before each death.
   **It is not a crash.** It is `process::exit` skipping `JS_ShutDown()`, and
   `engine/js/src/spidermonkey.rs` predicts that exact string in its own doc comment. The line that
   named the cause (`UNMEASURABLE [timeout-150s]`) was **one line higher, three times.**
   ⚠ *The last line before a death is not the cause of it.*
2. **"kicktipp's near-bar divergence is a `box-sizing`/`font` defect."** A reduction reproduced it
   exactly — `103.00x49.20` against Chrome's `103.00x30.34` — under `manuk-layout`'s harness, and
   **evaporated on `Page::load`.** The `font:` shorthand is unparsed in `MinimalCascade` only. It
   reproduced *the right number for the wrong reason* (49.20 = 2×18 + 13.2, two lines at the default
   16px metrics). ⚠ *A reduction is not confirmed until it has run on the SHIPPING cascade.*
3. **"F2 is a regression."** 7.84x on a loaded box, **5.82x** on a settled one, on a docs-only tree.
   The ratio's DENOMINATOR moved (`mid` 15% faster). Re-run, never retune — for the second time in
   two audit windows.

### ⚠ THE INSTRUMENT, WHICH IS WHERE THIS WINDOW'S LEVERAGE WAS

t824 is the highest-value tick here and it changed no rendering code. Three sweeps had been refused;
the board had said "MEASURE NOW" for ~12 ticks; three sessions obeyed literally and moved on. **The
fourth run treated the contamination as the tick**, and one arithmetic fix converted a 12-tick blind
spot into a 40-minute, 200-of-200 number that priced five unpriced fixes at once (M1 8.0% → 10.0%,
count **11 → 13 sites**). ⚠ *A measurement that has failed three times is a capability gap, not a
chore to retry.*

### THE GAP THIS AUDIT LEAVES OPEN

- **`manuk-wpt` is in NEITHER the wall's crate-test list nor CI's** — the crate that produces the
  Phase-0 headline is the one crate no lane tests. `chunk_spawn_budget` is real and RED-proven and
  runs only by hand. Both files are observer-owned; reported, not patched.
- **The verify wall is 776s against a 300s target** — the self-audit's single ✗ this window, and
  harness-owned. Reported, not patched.
- **`www.freesupertips.com`** fell 0.7637 → 0.6674 at near-flat coverage and is the one t825 row not
  explained. It owes an old-binary control.

## Audit #55 — tick 838 (2026-08-01)

SOURCES: the fresh t832 CrUX sweep rows; `gismart.com` (WordPress + a `data-lazy` image loader);
`possssno.sbs` (Persian RTL, `#aside` off-canvas drawer); the engine's own `--images` probe.

### WHAT WAS ADDED TO THE MAP — **the JS-driven lazy-load image swap, and it was not on it**

The map has `IntersectionObserver` (confirmed, gated, with `trackVisibility` accepted) and it has
`loading="lazy"`. It does **not** have the thing the corpus actually ships, which is neither:

```html
<img class="fit" data-lazy
     src="data:image/gif;base64,…1×1 transparent GIF…"
     data-src="https://…/footer-1.png"
     data-srcset="…640w, …768w, …">
```

**41 of `gismart.com`'s images are that shape.** The real URL is in `data-src`; the site's own
JavaScript moves it to `src`. This is the dominant lazy-load idiom of the WordPress/theme web —
older and far more common than `loading="lazy"`, because it predates it and works everywhere — and
the map had no row for it at all. Added with status **`unknown`**, because this audit measured the
SYMPTOM and deliberately did not guess the cause (below).

### WHAT WE HAD BEEN WRONG ABOUT — **a 1×1 placeholder is not a missing image, it is a WRONG RATIO**

The instinct (and the map's framing) is that a lazy image that never loads shows *nothing* — a
missing box, a coverage problem. **That is not what it does.** The placeholder is a real, decoded,
1×1 GIF, so `apply_natural_size` gives the element an intrinsic **ratio of 1:1**, and every
downstream sizing rule then works perfectly from a wrong premise:

```text
  gismart.com                                Chrome     ours
  section:nth-of-type(3)/img:nth-of-type(1)  258x258   258x851    ← ours TALLER
  section:nth-of-type(3)/img:nth-of-type(2)  258x258   258x928
  section:nth-of-type(3)/img:nth-of-type(4)  258x258   258x1005
  section:nth-of-type(5)/…/img               419x851   419x419    ← ours SQUARE
```

Note the two directions. It is not "our images are small" or "our images are tall" — it is that
**every one of them is the shape the 1×1 placeholder implies rather than the shape the real asset
implies**, so the error's sign depends on the real image. That is why it reads as scattered
geometry noise in the cluster ledger and never as one cause. `coverage` is **0.983** — the boxes are
all there. This is a FUNCTION defect wearing SHAPE's clothing, and it is the fourth distinct time
this project has recorded that shape (`typeof null`, the correct-but-empty Array, the half-installed
`performance.mark`, now a correct-but-placeholder image).

### CORRECTED

* **`IntersectionObserver: confirmed` is true and was answering a question nobody asked.** The map
  recorded that the API exists and fires. What it does not record — and what decides whether a page
  renders — is whether the *page's own* lazy-load path completes end to end. A confirmed API is not
  a confirmed capability when the capability is a chain.
* **`loading="lazy"` is the minority spelling.** `gismart.com`: `data-lazy` 41, `data-src` 41,
  `data-srcset` 17, `loading="lazy"` **0**. The map ranks the attribute the spec added; the corpus
  ships the attribute the ecosystem invented.

### RE-RANK — this is a FUNCTION-leg row, and the constitution check at t836 just said that is the ceiling

Check #70 recorded that scorability is flat at **101/131 = 77.1%** and that no render fix can move
it. This audit produces the first concrete, corpus-measured instance of the *next* rung: a site that
scores (0.7153, gap 0.035 to the bar) and whose remaining error is not layout math at all. It goes
on the map above further replaced-element geometry.

⚠ **NOT DIAGNOSED, DELIBERATELY.** Two candidates survive and this audit refuses to pick between
them without measuring: (a) the site's lazy-load script never runs to completion for us, or (b) it
is `IntersectionObserver`-gated and every one of these images is below the fold (`y` = 1261, 2301,
9425, 13415, 14216 against a 720px viewport), so the observer legitimately never fires for a page we
never scroll — while the oracle's Chrome rasterises full-page and does. **The discriminator is
cheap and named: find one `data-lazy` image ABOVE the fold and see whether its `src` swapped.** On
this page none of the 41 is above the fold, which is itself the reason the two cannot be separated
here — the next probe needs a site whose first hero image is lazy, or a synthetic fixture.

Set `LAST_SURFACE_AUDIT: 838`.

## Audit #56 — tick 848 (2026-08-02)

SOURCES (web, not memory): the Interop 2026 focus-area list
(`github.com/web-platform-tests/interop/blob/main/2026/README.md`, and the vendor announcements at
`webkit.org/blog/17818/`, `web.dev/blog/interop-2026`, `igalia.com/news/interop-2026.html`,
`hacks.mozilla.org/2026/02/launching-interop-2026/`); Baseline 2026 (`web.dev/baseline/2026` and the
2026 monthly digests); Ladybird's 2026 newsletters (`ladybird.org/newsletter/2026-06-30/`,
`2026-01-31/`) for what an independent engine finds hard and in what order.

### THE RECONCILIATION — the map is CURRENT, and that is the finding that made the gap visible

All **20** Interop 2026 focus areas and **3 of the 4** investigation efforts already have rows, with
honest statuses: container style queries `missing`, anchor positioning `missing`, `attr()` partial,
`contrast-color()` partial, CSS zoom **gated**, custom highlights `missing`, dialogs+popovers
**gated**, fetch uploads/ranges **gated**, IndexedDB **gated**, JSPI `missing`, media pseudo-classes
partial, Navigation API **gated**, scoped custom element registries `missing`, scroll-driven
animations `missing`, scroll snap **gated**, `shape()` partial, view transitions **gated**, WebRTC
`missing` *(explicitly out of scope, STATUS.md)*, WebTransport `missing`, JPEG XL `missing`, WebVTT
**gated**. Baseline 2026's newly- and widely-available lists likewise all landed on existing rows —
`lh`/`rlh` **gated**, multi-keyword `display` **gated**, `Content-Encoding: zstd` `missing`,
`:active-view-transition` `missing`, `shape()`, `contrast-color()`.

**The one Interop 2026 item with NO row is the accessibility-tree investigation — and specifically the
ACCESSIBLE NAME.** `accname` matched **zero rows in the whole file**, while its neighbours are green:
a11y STATES gated by `G_A11Y_STATE`, a11y interactive ROLES gated by `G_A11Y_ROLES`, focus management
gated, `inert` gated. **That is exactly what makes an absent row invisible — the capability looks
covered because everything beside it is.** And `STATUS.md`'s platform map item 8 has said for ~800
ticks that whether the tree's *roles, names and focus order* are correct is UNMEASURED, so the map
and the status file disagreed and nothing compared them.

It is doubly weighted here: **the agent identifies elements by name.** t845 established that the
agent's click point IS layout geometry; the same argument applies one field over — the agent's
element *identity* is the accessible name, and a wrong accname is a wrong click on a page where every
box is in the right place.

### WHAT WAS ADDED — 8 rows, all `unknown`, `MEASURED` unchanged at 432

| row | why it was invisible |
|---|---|
| **accessible NAME computation (accname)** | states and roles are gated; the name has never had a row |
| **`command` / `commandfor` invoker attributes** | the map gated `ToggleEvent.source`, the EVENT that reports which element opened a popover, while the ATTRIBUTES that do the opening had no row — **a gated consumer of an unmeasured producer** |
| **CSS `if()`** | not present under any spelling |
| **`::scroll-marker` / `::scroll-marker-group` / `::scroll-button`** | `carousel` matched only the `scroll snap` row: the snap axis is measured, the carousel's own controls are not |
| **`interesttarget` / interest invokers** | zero rows |
| **Translator / LanguageDetector / Summarizer** | the map stopped at `Prompt API (LanguageModel)`; a **half-present family** is the exact shape t772 proved routes a page into a wall rather than to its fallback |
| **Compute Pressure (`PressureObserver`)** | zero rows |
| **`writing-suggestions`** | zero rows |

### WHAT WE HAD BEEN WRONG ABOUT

Not a phantom this time — a **blind spot with green edges**. The previous audits have been hunting
rows that were WRONG (six phantoms). This one found a row that was ABSENT, and absent specifically
because the capabilities either side of it are gated, which is the failure mode a coverage count
cannot see: `280 gated / 108 missing / 35 partial / 9 works` looked like a map with no holes.

### RE-RANK

**It does not displace the render burndown**, and the honest reason is arithmetic rather than
enthusiasm: M1 is `shape≥0.75 AND jarring-clean` on the CrUX corpus and accname moves neither term.
But it is now the **top-ranked row on the agentic axis**, above the remaining function-leg work,
because it is one probe away from a verdict — the same "cheap measure-and-pin probe" shape that has
paid every time — and because a wrong name is a silent mis-actuation exactly like t845's click point.

**Next: a probe, not a build.** Take the accname precedence order (aria-labelledby > aria-label >
native `<label>`/`alt`/`caption` > `title` > content), run it against Chrome's computed
`accessibleName` on a fixture that exercises each rung, and record the verdict. `unknown` is the
honest status until then, and the invariant this loop is graded on is MEASURED, not `missing`.
