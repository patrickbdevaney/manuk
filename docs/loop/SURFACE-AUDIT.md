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

## Audit #57 — tick 859 (2026-08-03)

SOURCES (web, not memory): the Interop 2026 focus-area list
(`github.com/web-platform-tests/interop/blob/main/2026/README.md`) re-read in full rather than recalled
from audit #56; the Interop 2026 **web-compat** issue (`web-platform-tests/interop` #187) for the item
that names actual site-breaking bugs; `web.dev/blog/baseline-digest-may-2026` (the newest digest, which
did not exist at audit #56's date); Servo's layout wiki + 2026 layout PRs
(`github.com/servo/servo/wiki/Servo-Layout-Engines-Report`, `Layout-revamp-ideas`, PR #41812).

### WHAT THE RECONCILIATION FOUND — two rows, and both are of a kind the map keeps missing

The map held up: **all 20** Interop 2026 focus areas and **5 of the 6** May-2026 Baseline features
already carried an explicit verdict, and audit #56's finding (accname) is the only structural hole
still open. Two genuine additions, now on the map as `unknown`:

1. **`text-decoration-skip-ink: all`** — Baseline *newly available* May 2026, **zero hits** in the whole
   file. The other five items in that digest (container style queries, `:open`, `ToggleEvent.source`,
   `image-rendering`, `SharedWorker`) all landed on existing rows.
2. **scroll-event / animation-event ORDERING** — one of the three items Interop 2026's `web compat`
   area actually contains ("a small collection of WPT selected because failing them causes real
   websites to not work"). The other two — ESM cyclic module records + multiple top-level `await`, and
   unprefixing `-webkit-user-select` — already have rows.

⚠ **THE ORDERING ROW IS THE INTERESTING ONE, AND IT IS THE t712-714 CLASS AGAIN.** An instrument that
diffs the *finished* answer cannot see a bug in the ORDER the answer was assembled — only a probe
running *inside* the page can. So this is a capability the oracle is **structurally blind to**, on a
list whose entire selection criterion is "this breaks real sites". That is precisely the sort of item
that stays off a map drawn by looking at what the instrument reports.

### ⚠⚠ AN INDEPENDENT ENGINE NAMES THIS TICK'S BUG CLASS AS ITS TOP LAYOUT BUG AREA

Servo's own layout wiki: *"Most Servo layout bugs are in the area of interactions between block sizes,
line breaking, floats, and margin collapsing"*, and its January-2026 PR #41812 is literally
*"let floats know that margins can collapse thru phantom lines"*. Tick 859 — landed the same day as
this audit — fixed a float↔margin-collapse interaction that had been wrong for 700 ticks.

**This is external corroboration of a RANKING, which is the rarest thing this audit produces.** The
loop reached float/margin-collapse by ranking the t857 sweep's own reading-order column; an independent
memory-safe engine, with a different architecture and a decade of different bugs, reports the same
neighbourhood as *where its layout bugs live*. Read it as a prior for the next few render ticks: the
remaining shape/jarring residue is more likely to be **float ↔ block-size ↔ line-breaking interaction**
than a missing property.

### WHAT WE HAD BEEN WRONG ABOUT

Nothing was found to be falsely claimed this round. The correction is to the audit's own frame: #56
concluded *"the map is CURRENT"* from a check that read the focus-area **titles**. Reading the web-compat
area's **contents** — which are not in the title list — produced a row immediately. **A focus area is not
an atom; the ones named "web compat" and "investigation" are containers, and a title-level reconciliation
cannot see inside them.**

## Audit #58 — tick 869 (2026-08-03)

SOURCES (web, not memory): the Interop 2026 focus-area + investigation list re-fetched in full from
`github.com/web-platform-tests/interop/blob/main/2026/README.md`; `web.dev/blog/interop-2026`;
`webkit.org/blog/17818/announcing-interop-2026/`; and — the search that mattered —
`webkit.org/blog/15400/improving-web-accessibility-with-web-platform-tests/` plus
`web-platform-tests/interop` issue #526 and `web-platform-tests/interop-accessibility` issue #3, for
**how** the accessibility investigation is actually scored.

### THE RECONCILIATION — the map is clean, and that is not the finding

All **20** Interop 2026 focus areas carry a verdict. So do the four investigation efforts. Nothing new
to add: audits #56 and #57 did that work, and it held.

### ⚠⚠⚠ THE FINDING — THE SAME STRUCTURAL HOLE, THIRD AUDIT RUNNING, AND NOW ITS MECHANISM

Audit #56 (t848) named the accessible NAME as the one Interop item with no row, and weighted it
doubly: *"the agent identifies elements by name … a wrong accname is a wrong click on a page where
every box is in the right place."* Audit #57 (t859) recorded it as **"the only structural hole still
open."** It is tick 869 and it is still open — and `CONSTELLATION.tsv` row 21 has said since **tick
618** that *"the TREE's role+name correctness is still unmeasured."*

**This audit's contribution is not the hole. It is why nobody has ever closed it, and the answer is
one line:**

```text
  $ ls ~/wpt
  common  css  cssom  dom  domparsing  encoding  html  mathml  resources  svg  url  …
  $ ls ~/wpt/accname ~/wpt/wai-aria ~/wpt/html-aam
  ABSENT · ABSENT · ABSENT
```

**The measuring instrument was never downloaded.** Our WPT checkout is a nine-directory partial, and
the three suites that measure exactly the thing our constitution calls our moat are not among them.
The map said "unmeasured", the status file said "unmeasured", three audits said "unmeasured" — and
the reason was never that it was hard. It was that the tests were not on disk, and no instrument
looks at what is *missing from the corpus*, only at what the corpus reports.

That is a new shape for the ledger: **an absent MEASUREMENT hides exactly as well as an absent
capability, and neither `map-reconcile.sh` nor the WPT runner can see one.** A directory that was
never fetched reports no failures.

### AND THE MEASUREMENT EXISTS, IS SPEC-AUTHORED, AND IS WHAT FOUR VENDORS SCORE THEMSELVES ON

WPT tests computed role and accessible name through **`testdriver.js` → WebDriver
`get_computed_role` / `get_computed_label`**, which is precisely what makes a single automated test
run on Chromium, Gecko and WebKit alike; Interop has scored it since the 2023 investigation and
carries it forward in 2026. So there is a ready-made, adversarial, cross-browser-validated oracle for
the capability `I3` calls *"the single most durable moat"*, and this project has never run one test
from it.

⚠ **And the seam is ours to exploit rather than to reimplement.** Those tests need
`get_computed_role`/`get_computed_label` because an incumbent can only reach its a11y tree through a
WebDriver round-trip. **I3's whole claim is that ours is synchronous and in-process** — so the two
testdriver entry points can be bound straight to `manuk_a11y` instead of a driver protocol. The thing
that makes the suite expensive for everyone else is the thing this architecture makes cheap, which is
the constitutional claim being cashed rather than asserted.

### RE-RANK — this outranks the shape burndown for one tick

Check #74 closed by saying the next window is engine work and named the 13 jarring-only sites. This
displaces it by exactly one tick, on I3 grounds: fetch `accname` + `wai-aria` + `html-aam`, bind the
two testdriver entry points to the in-process tree, and get **the first honest number the moat has
ever had.** A capability that has been `partial` and unmeasured for 250 ticks, is constitutionally
the differentiator, and whose test corpus is one `git sparse-checkout` away, is not a thing to leave
for a fourth audit to re-notice.

MEASURED count unchanged (no rows added — the hole was already on the map; what changed is that it is
now actionable).

## Audit #59 — tick 879 (2026-08-03)

SOURCES (web, not memory): the Interop 2026 focus-area + investigation list re-fetched in full from
`github.com/web-platform-tests/interop/blob/main/2026/README.md`; `web.dev/blog/interop-2026`;
`webkit.org/blog/17818/announcing-interop-2026/`; the **2026 Baseline monthly digests**
(`web.dev/blog/baseline-digest-{feb,mar,apr,may}-2026`); and Ladybird's current public numbers.

### What was CHECKED, and the result that is worth stating plainly

**All twenty Interop 2026 focus areas and all four investigation efforts have a row on
`CONSTELLATION.tsv`** — checked one at a time, not by a bulk grep, because a grep for `shape()` or
`zoom` hits another row's prose and reads as a hit. Each has an honest status:
`container style queries` missing · `anchor positioning` missing · `attr()` partial ·
`contrast-color()` present · `zoom` gated · `custom highlights` missing · `dialog + popover` gated ·
`fetch uploads + ranges` gated · `IndexedDB getAllRecords()` works · `JSPI` missing ·
`media pseudo-classes` partial · `Navigation API` gated · `scoped custom element registries` missing ·
`scroll-driven animations` missing · `scroll snap` gated · `shape()` present · `view transitions`
gated · `WebRTC` missing · `WebTransport` missing · and of the investigations, `accessibility
testing` is the one audit #58 opened and t870 put a number on (797/1250).

That is audits #34–#58 having done their job, and it is the reason this audit had to go somewhere
Interop does not: **the Baseline monthly digests, which move every month and name things the
focus-area list never will.**

### ADDED — three capabilities the world names and our map did not

| class | capability | why it was invisible |
|---|---|---|
| cross | **`scroll` event TIMING** (ordering vs rAF, coalescing, the `scrollend` pair) | the one Interop 2026 web-compat item with no row. The other two are gated (`ESM` cyclic modules + multiple top-level await; `user-select` unprefixing). |
| css | **`update` media feature** (`@media (update: fast\|slow\|none)`) | Baseline newly available **March 2026** — after audit #58's frame was drawn. |
| css | **`offset-path` / motion path** | surfaced *by* the `shape()` Baseline note, which names `clip-path` **and** `offset-path` as its two consumers. |

### WHAT WE HAD BEEN WRONG ABOUT

⚠⚠⚠ **We gate the scroll SURFACE and nowhere the scroll SCHEDULE.** `IntersectionObserver`,
`scroll snap`, `scroll anchoring` and `infinite scroll` are all gated — and every one of them asserts
*what* ends up on screen. Interop 2026 picked scroll **event timing** precisely because that is not
the same question: a page that reads `scrollTop` inside a scroll handler, or sequences a sticky
header against `requestAnimationFrame`, gets a different answer per engine, and *"the boxes are in
the right place"* cannot see it. This is t712-714's finding recurring in a new subsystem — **an
instrument reading the finished answer cannot see a bug in the ORDER it was assembled** — and it
took an external list to notice, which is what this audit exists for.

⚠⚠ **A row that exists for ONE of a feature's two consumers reads as covered.** `CSS shape()` is on
the map; `offset-path`, its other consumer, was not. That is the half-built-spec shape of t704-710
(*"a build spec whose 2nd half is unbuilt is an untriaged tick with good prose"*) appearing in the
MAP rather than in a build spec.

⚠ **Interop is an annual frame and Baseline is a monthly one.** Two of the three additions came from
Baseline, not Interop, and both post-date audit #58 by weeks. An audit that only re-reads the Interop
list will find the map clean *by construction* from now on — the digests have to be part of the
routine.

### SCALE, recorded as context and explicitly NOT as a steer

Ladybird publicly reports **2,067,263 passing WPT subtests** and **97.8% of test262**, against our
`WPT:TOTAL` mark of 422,865. Per **PART VII** the WPT total is *"a bookkeeping mark, not a ranking"*
and `83% and beyond is explicitly OUT OF SCOPE for v1`, so this changes no priority. It is written
down because a number that large, unrecorded, is the kind of thing a future audit rediscovers and
mistakes for news. Their stated hardest problem is also worth keeping: **real sites depend on
undocumented Blink/WebKit quirks, so a spec-correct implementation can be the wrong answer.** Our
North Star already answers that — Chromium is the *capability* target and every fix this project
lands is diffed against `chromium --dump-dom`, not against prose — and this is the first external
confirmation that the choice was load-bearing rather than merely convenient.

### RE-RANK

None of the three additions outranks the current CO-#1. All are `unknown` and cheap to answer;
`scroll` event timing is the largest of them and is the one to probe first, because it is the only
one an existing gate could be *silently wrong* about rather than simply absent.

---

## Audit #60 — tick 889 (2026-08-04)

**Sources (read, not remembered):** `web.dev/blog/baseline-digest-jun-2026` · a `web.dev` search for
the June/July 2026 Baseline features (the July digest 404s — it does not exist yet, which is itself
worth recording so the next audit does not re-fetch it).

⚠⚠⚠ **AUDIT #59'S FINDING 3 RECURRED TWICE IN ONE READ, AND IT IS THE MOST RELIABLE WAY THIS MAP GOES
WRONG.** *"A row that exists for one of a feature's two consumers reads as covered."*

* `CSS shape() function (clip-path / shape-outside)` is on the map. **`shape-outside: rect()` and
  `xywh()`** — a different function set on the same property, Baseline **newly available** June 2026 —
  are not, and a grep for `shape-outside` hits the `shape()` row and reads as covered.
* `lazy loading (loading=lazy + IO)` is on the map, gated by `G_VIEWPORT`. That row is the **`<img>`**
  consumer. **`HTMLIFrameElement.loading`** — the *iframe* consumer, and the one that matters for
  below-the-fold embeds, maps and players — is a separate feature that went Baseline widely available
  in June, and a grep for `loading=` hits the image row.

The countermeasure is the one #59 already found and it held: **check each candidate individually
against the row it appears to match**, never on the grep count. Five of the fifteen candidates read as
present and only three actually were.

⚠⚠ **AND THE AUDIT PROBED RATHER THAN FILING `unknown`, so nine of the ten new rows carry a VERDICT.**
One fixture, `chromium --dump-dom` against our own engine:

```text
                                    Chrome      manuk
  counter-set                        true       FALSE     ← missing
  shape-outside: rect()              true       FALSE     ← missing
  shape-outside: xywh()              true       FALSE     ← missing
  :dir(rtl) matches                     1           0     ← missing
  CanvasRenderingContext2D.reset()  function  undefined    ← missing
  getComputedStyle().counterSet   'mycount 7'  undefined   ← missing
  CSS pow()                          true       true      partial (PARSE-LEVEL)
  linear() easing                    true       true      partial (PARSE-LEVEL)
  @media (scripting: enabled)        true       true      partial (behavioural)
  URL.canParse                     function   function    partial (behavioural)
  HTMLIFrameElement.loading          true       true      partial (IDL only)
  field-sizing                       true       FALSE     ← already on the map, twice
```

⚠ **TWO OF THE `partial`s ARE DELIBERATELY NOT `gated`, AND THE REASON IS THIS PROJECT'S OWN
PRECEDENT.** `pow()` and `linear()` were answered by `CSS.supports`, which is a **parse-level**
question — and at t574-583 `@supports` answering *"does it parse"* produced **31 phantom properties**
on this very map. The rows say so in their receipts. `@media (scripting)`, `URL.canParse` and
`iframe.loading` were answered behaviourally (a match, a `typeof`, an `in`) and are honestly
`partial`: present, ungated.

⚠ **A REFERENCE FACT WORTH RECORDING BECAUSE IT LOOKS LIKE A GAP AND IS NOT:** `text-fit` reads
`false` in **Chrome too** on this box. The search results say Chrome 150 shipped it; our reference
Chromium predates that. So it is not a divergence, and an audit that filed it as one would have
manufactured a backlog row.

**ADDED (449 → 459 rows):** `counter-set` · `shape-outside: rect()/xywh()` · CSS gap decorations ·
`:dir()` · CSS math `pow()` · `<easing-function>` · `@media (scripting)` · `URL.canParse()` ·
`CanvasRenderingContext2D.reset()` · `HTMLIFrameElement.loading`.

**CORRECTED:** the lumped row *"2026 CSS frontier: if() / @function / inherit() / text-fit / gap
decorations / progress()"* can no longer carry a verdict for **gap decorations**, which shipped
(Chrome 149, Baseline June 2026) while the rest of that row has not. Split out. A lumped row is a row
that cannot ratchet — the same defect shape as the lumped assertion that failed permanently at t855.

**RE-RANK:** none of the ten outranks the current lever. The five `missing` are all small, contained
CSS/canvas surfaces with no evidence of corpus pressure behind them; the M1 crossing cohort measured
at t888 (eight sites one jarring dimension from crossing, +6.2 points) is still the largest known
lever. Recorded as *"discovered, not urgent"* rather than promoted — which is what the map is for.

## Audit #61 — tick 900 (2026-08-04)

**SOURCES** (searched, not recalled — the audit's own rule, and the cutoff is three months stale):

* <https://github.com/web-platform-tests/interop/blob/main/2026/README.md> — the authoritative Interop
  2026 list, fetched rather than summarised
* <https://web.dev/blog/interop-2026> · <https://webkit.org/blog/17818/announcing-interop-2026/> ·
  <https://hacks.mozilla.org/2026/02/launching-interop-2026/> · <https://www.igalia.com/news/interop-2026.html>
* <https://web.dev/baseline/2026> and the monthly digests (Jan/Feb/Apr/May 2026)
* <https://ladybird.org/newsletter/2026-06-30/> — the independent engine that has walked this road

### RECONCILIATION: all 24 Interop 2026 items were ALREADY on the map, and that is the headline

Twenty focus areas — container style queries · anchor positioning · `attr()` · `contrast-color()` ·
`zoom` · custom highlights · dialogs and popovers · fetch uploads and ranges · IndexedDB · JSPI ·
media pseudo-classes · Navigation API · scoped custom element registries · scroll-driven animations ·
scroll snap · `shape()` · view transitions · web compat · WebRTC · WebTransport — plus four
investigations (accessibility testing · JPEG XL · mobile testing · WebVTT). **Every one has a row and
a status.** The three Baseline-2026 arrivals (`zstd`, `:active-view-transition`, `contrast-color()`)
are on it too. Map: 280 gated · 112 missing · 41 partial · **16 unknown** · 9 works — 3.5% unknown.

⚠ **AND AN AUDIT THAT FINDS NOTHING IS A SUSPICIOUS AUDIT, so the `gated` rows were PROBED rather
than believed.** A differential fixture against Chrome on twenty claims the map calls settled found
**four defects behind three green-looking rows**:

```text
                                          Chrome            ours
  getComputedStyle(el).zoom                    2        undefined     <- row says GATED
  offsetWidth of a zoom:2, width:50px box     50              100     <- and the geometry is wrong
  getComputedStyle(el).containerType   inline-size      undefined     <- row says GATED
  'duplex' in a POST Request                true            false     <- row says GATED
  CSS.highlights                          object        undefined     <- row says MISSING (correct)
```

1. **`zoom` and `containerType` are a THIRD and FOURTH member of t897's class**, not three separate
   bugs: *`getComputedStyle` declines to publish what the pipeline has already computed*. `width`,
   `height` and `transform` were the first three. **This is now a pattern and should be swept as one**
   — enumerate every property the cascade or layout resolves and diff the whole object against
   Chrome, rather than discovering one member per tick.
2. **`offsetWidth` under `zoom` is a real geometry defect**, not a plumbing one: Chrome reports the
   element's own unzoomed box (50) while `getBoundingClientRect().width` is the zoomed 100. We report
   100 for both. `zoom` is an Interop 2026 focus area.
3. **`fetch uploads and ranges` is gated by `G_MEDIA_SEGMENT_FETCH`, which covers RANGES only.**
   `Request.duplex` — the flag a streaming upload requires — is `false`. **This is exactly t889's
   defect** (*"one row for one of a feature's consumers reads as covered"*), recurring eleven ticks
   later on a differently-shaped row, which is the argument for probing gated rows every audit rather
   than trusting the gate name.

⚠ **ONE APPARENT DIVERGENCE IS NOT ONE, recorded so a later audit does not file it as a backlog row:**
`CSS.supports('color','contrast-color(black)')` is **`true` here and `false` in our reference
Chromium**, and `contrast-color(black)` resolves to white for us and is discarded as invalid by
Chrome. We are AHEAD, not wrong — Chromium is the CEILING on capability, so being past it is the
point and not a bug. The map's `gated` row is accurate.

**ADDED (459 → 460 rows):** *modern ECMAScript built-ins* — and it went straight in as **`works` with
a verdict rather than `unknown`**, because it was measured in the same pass: thirty built-ins
(`toSorted`/`toReversed`/`toSpliced`/`with`, `at`, `findLast`, `Object.groupBy`, `Map.groupBy`,
`Promise.withResolvers`, `Array.fromAsync`, `Set.union`/`intersection`, `structuredClone`, the RegExp
`v` flag, `Error.cause`, `WeakRef`, `FinalizationRegistry`, `Intl.Segmenter`/`ListFormat`/
`RelativeTimeFormat`, **`Temporal`**, iterator helpers) are **30/30 byte-identical to Chrome**. This
was the one Baseline-2026 line with no row, and the honest result is that SpiderMonkey is fully
current — a bundle using `array.toSorted()` unguarded runs here.

**CORRECTED:** three rows lose their green. `CSS zoom (per-element property)` **gated → partial** (the
property is not readable and `offsetWidth` is wrong under it); `container queries` gains an explicit
`containerType`-not-exposed note; `fetch uploads + ranges` **gated → partial** (`G_MEDIA_SEGMENT_FETCH`
covers ranges; `Request.duplex` is false).

**WHAT WE HAD BEEN WRONG ABOUT:** that a `gated` status means the capability answers correctly *to a
page*. Three of the twenty probed rows are gated on the behaviour and silent on the READBACK, and a
page branches on the readback. The gate proves the engine does the thing; nothing proved the engine
would *say* it does.

**RE-RANK:** the getComputedStyle-readback sweep is now the ranked follow-on to t897 and is larger
than it looked — four known members and no enumeration. It does not outrank t888's crossing cohort on
M1 points, but it is the cheapest well-understood work on the board and it is the class three of this
window's ticks have already been paid by.


---

## Audit #62 — tick 910 (2026-08-04)

**SUBJECT: the 112 `missing` and 16 `unknown` rows — do any of them describe a capability that is
actually BUILT?** t900 audited the map from the other side (probe `gated` rows; three were lying) and
t905 supplied the reason to audit this side: **`aspect-ratio` had no row at all** while being fully
built and Chrome-exact, and STATUS.md's platform map records that as the *fifth* time a tick was
aimed at something that already existed (`localStorage`, `FormData`, `position:sticky`,
`IntersectionObserver`). A negative row is the same failure wearing different clothes.

### The probe, and the result

Ten `missing` rows chosen for being observable in a box: `content-visibility` /
`contain-intrinsic-size` · `multicol` · `writing-mode` vertical text · `justify-self` in block layout
· legacy `-webkit-box` · `@scope` · `field-sizing: content` · `subgrid` · `column-span: all` ·
container STYLE queries. One fixture, `--dump-dom` against
`google-chrome-stable --headless=new --hide-scrollbars`, our side through `manuk-wpt boxes`.

**ZERO map errors.** Every row that could be answered was answered *correctly negative*:
`content-visibility:auto` + `contain-intrinsic-size:300px 111px` → Chrome 111, ours 24;
`grid-template-columns: subgrid` → Chrome puts the grandchild in the 100px track, ours spans all 300;
`column-count` in every form → Chrome grows the box, ours does not. On this sample the map is honest
in the negative direction, which is the opposite of what t900 found in the positive direction and is
worth recording as a contrast rather than as a null.

### ⚠⚠⚠ THE FINDING IS THE PROBE, AND IT IS THE FIFTH TIME THIS SESSION

**Six of the ten probes were structurally incapable of answering their own question**, because each
fixed the very dimension the capability would have changed:

* `writing-mode: vertical-rl` was given `height: 200px`, so both engines returned 200.
* `column-span: all` and `-webkit-box` children were given `height: 10px`.
* `justify-self: end` moves `x`; the probe recorded only width and height.
* The `@scope` rule (`i { … }`, specificity 0,0,1) lost to the fixture's own `#q6 > i` (1,0,1), so
  **Chrome did not apply it either** — the probe measured its own cascade mistake.

**And a seventh was MISREAD for want of a control arm.** `column-count:3` gave Chrome 144 and us 120,
and 120 is far short of what one 300px column of that text would be — so it read as *"both engines do
multicol, slightly differently"*, and the map row looked wrong. Adding one no-feature control row
settled it in a line:

```text
                          Chrome   ours
  no columns (CONTROL)     72       72
  column-count:3          120       72     <- ours is IDENTICAL to the control
  column-count:2           96       72
  column-width:100px       96       72
```

We ignore multicol entirely. The map was right, and the only reason it briefly looked wrong is that
the probe had nothing to compare its own output against.

> **A capability probe needs a NO-FEATURE CONTROL ARM in the same fixture, and it must never fix the
> dimension the capability changes.** Without the control, "different from Chrome in the direction
> support would take it" is indistinguishable from "different from Chrome".

**Running tally, because the ratio is the point: five fixture defects across t905-t910** — a missing
`--hide-scrollbars` (15px), floats leaking between un-isolated rows (120px), a confounded
`width:400px`, a probe that could not tell `0,0,0` from no-box, and now a capability probe with no
control arm. Each was caught by reading the numbers rather than the verdict; none reached a commit.
The differential fixture is still this project's best discovery engine (t784-796: nine engine defects
in thirteen ticks) — and its failure mode is now well enough characterised to be a checklist:
**one variable per case · a control arm · never fix the measured dimension · absence is not zero.**

### RE-RANK

Nothing changes at the top: the t909 sweep's ranked causes are all `<div>`, and the map's negative
rows are honest on this sample, so there is no unbuilt-but-mapped or built-but-unmapped work to
promote. The audit's output is a **probe checklist**, not a backlog item.

---

## Audit #63 — tick 921 (2026-08-04)

**SUBJECT: the six map rows THIS SESSION wrote, re-probed — because two of the fixes behind them were
reverted, and because a row is written once and read for months.** t900 audited `gated` rows the loop
inherited; #62 audited the `missing`/`unknown` side. This one audits **the rows the auditor just
wrote**, which is the shortest possible feedback loop and, as it turns out, not a short enough one.

### The gates: all present, all green, and nothing cites a gate that no longer exists

```text
  G_RATIO_INSET_FLOAT                 file exists   ok
  G_BFC_SPECIFIED_WIDTH_FLOAT_BAND    file exists   ok
  G_TABLE_HEIGHT_IS_A_MINIMUM         file exists   ok
  G_TABLE_BORDER_SPACING_UA_DEFAULT   file exists   ok
  G_VERTICAL_ALIGN_ON_TEXT            file exists   ok
  G_FORM_CONTROL_BASELINE             file ABSENT   (reverted at t919 — cited by the map ZERO times)
```

The reverted gate is the interesting one and it came out clean: t919 removed the file, lowered the
GATES ratchet mark deliberately, and rewrote the row it backed. **A revert that leaves a dangling
gate name in the map is the classic residue, and this one left none.**

### ⚠⚠⚠ THE FINDING: A ROW I WROTE AT t914 WAS STALE BY t916, AND TWO OF MY OWN TICKS WALKED PAST IT

`vertical-align` on TEXT still carried, seven ticks later:

* *"super 29 vs 30, sub 26 vs 28"* — **superseded at t915**, where both became exact.
* *"approximates what Chrome derives from the font OS/2 superscript offsets"* — **wrong**, and t915
  measured the actual rule: the PARENT's font size × 0.375 (super) and × 0.25 (sub), verified at
  16/24/32px and invariant under `line-height`.
* No mention of **t916** closing `text-top` (27) and `text-bottom` (28) at all.

t915 and t916 each updated the **journal**, the **wiki** and **WEB-PATTERNS.md**. **Only the map fell
behind** — and the map is the artefact the loop reads when it asks *"is this already built?"*

> **The map is not stale because it is old; it is stale because it is the fourth artefact you update
> and the first one anybody reads.** A tick that supersedes its own predecessor updates the prose it
> is proud of and forgets the row it filed. The standing rule *"when you supersede a decision, grep
> for what asserted it"* has to include `CONSTELLATION.tsv` by name, or it keeps meaning the journal.

The row is rewritten in this commit with all three ticks' measurements and its own staleness recorded
in the receipt, so the next reader sees the correction and not just the corrected text.

### RE-RANK

None. The five surviving rows are honest, the reverted one left no residue, and the one stale row is
fixed. The audit's output is a **process correction**: superseding a finding means editing the map
row in the same commit, not only the journal.

---

## Audit #34 — tick 932 (2026-08-05)

**Sources, searched rather than recalled:**

* [Interop 2026 focus areas + investigations](https://github.com/web-platform-tests/interop/blob/main/2026/README.md) — the authoritative list, fetched
* [Launching Interop 2026 — Mozilla Hacks](https://hacks.mozilla.org/2026/02/launching-interop-2026/) · [Announcing Interop 2026 — WebKit](https://webkit.org/blog/17818/announcing-interop-2026/) · [Interop 2026 — web.dev](https://web.dev/blog/interop-2026)
* [Baseline 2026 — web.dev](https://web.dev/baseline/2026) and the [January 2026 digest](https://web.dev/blog/baseline-digest-jan-2026)
* [This Month in Ladybird — January 2026](https://ladybird.org/newsletter/2026-01-31/)

### RECONCILIATION — all 20 focus areas + 4 investigations, checked one by one

**Every Interop 2026 area already has a row, and every row carries a truthful verdict.** This is the
first audit in a while where the map survived the whole list without an addition, and the *statuses*
are the informative part rather than the presence:

```text
  container style queries        missing      CSS anchor positioning        missing
  CSS attr()                     partial      CSS contrast-color()          gated
  CSS zoom                       partial      custom highlights             (rows present)
  dialogs and popovers           gated        fetch uploads and ranges      (rows present)
  IndexedDB                      gated        JSPI for Wasm                 missing
  media pseudo-classes           partial      Navigation API                gated
  scoped custom element regs     missing      scroll-driven animations      missing
  scroll snap                    gated        CSS shape()                   (rows present)
  view transitions               gated        cross-document VT             partial
  WebRTC                         missing      WebTransport                  missing
  WebVTT (investigation)         gated        JPEG XL (investigation)       missing
```

Map totals after this audit: **282 gated · 112 missing · 44 partial · 17 unknown · 10 works** — 465
rows, of which 448 carry a verdict. The invariant the ratchet actually defends (capabilities
*measured*, not capabilities *green*) is intact.

### ⚠⚠ THE ONE ADDITION: MODULE SERVICE WORKERS, AND IT IS A HALF-INSTALLED API

Baseline Newly available **January 2026**: `navigator.serviceWorker.register(url, {type: 'module'})`
across all three engines. It is how a modern PWA ships its worker — an `import` on the first line of
`sw.js`.

Our `Service Worker` row is `gated` and says nothing about the script type, so the map read as though
this were covered. Reading the producer rather than the row: **`register(url, opts)` reads
`opts.scope` and never `opts.type`** (`engine/js/src/event_loop.rs:2693-2700`), and the script is
evaluated through `W.evaluate` — the classic path.

That is the **t772-775 shape exactly — absence routes to a fallback, HALF-presence routes into a
wall.** The option is accepted silently, the module script then fails to parse, the registration
promise rejects, and a PWA that awaits it never boots. A page that could have feature-detected its
way to a classic worker is instead told *yes* and then broken.

Added as its own row with status **`unknown`, deliberately, not `missing`** — the mechanism is read
from source and a claim about another subsystem is a hypothesis until a probe runs. The row carries
the probe that settles it: register a worker whose first line is `import {x} from "./m.js"` and
record whether the promise resolves.

### Also checked, already correctly on the map

`Content-Encoding: zstd` (Baseline Feb 2026) — present, `missing`. The `ic`/`ric` units — present.
`:active-view-transition` — present. No other Baseline-2026 item was absent.

### RE-RANK

**None, and the reason is worth recording rather than leaving implicit.** The newly-named surface is
all Interop/Baseline *feature* work, and the binding Phase-0 constraint is still the RENDER metric
(`shape >= 0.75` on the in-scope corpus). None of the 20 areas outranks the burndown: the ones that
are `missing` are large and modern (anchor positioning, scroll-driven animations, WebTransport, JSPI)
but none of them is why a corpus page lays out at the wrong size today. The module-service-worker gap
is a **function-leg** item and goes in the M2 queue, not ahead of M1.

The audit's value this cycle is therefore the negative one: **the frame is still the right frame**,
checked against what the four vendors agreed on rather than against memory.

---

## Audit #35 — tick 942 (2026-08-05)

**Sources, searched rather than recalled:**

* [15 updates from Google I/O 2026 — Chrome for Developers](https://developer.chrome.com/blog/chrome-at-io26) · [What's new in web UI](https://developer.chrome.com/blog/new-in-web-ui-io26)
* [2026 Browser Updates: Chrome 148, Safari 26.5 & Interop 2026](https://www.smartform.dev/blog/2026-browser-updates-chrome-148-safari-265-interop-2026) · [CSS in 2026 — LogRocket](https://blog.logrocket.com/css-in-2026/)

Audit #34 (ten ticks ago) took the **Interop/Baseline** axis and found the map complete but for one
row. This one deliberately took a **different axis — the Chrome-only frontier** — because a map
checked against the same list twice is checked once.

### ⚠⚠ THREE CAPABILITIES THE WORLD HAS NAMED AND THE MAP DID NOT

Each confirmed absent by grep over `engine/` (0 files), so `missing` is a verdict rather than a
guess. All three are Chrome-only and none is Baseline:

```text
  Soft Navigations API              js    an SPA's route changes are invisible to LCP/INP/CLS
  HTML-in-Canvas                    js    real DOM elements composited into a canvas
  Declarative Partial Updates       html  out-of-order HTML streaming, no JS DOM manipulation
```

Map totals: **470 rows · 283 gated · 115 missing · 44 partial · 17 unknown · 10 works.**

### The one that matters, and it is not the one with the biggest name

**HTML-in-Canvas is the first named 2026 API whose SHAPE would be expensive to retrofit here**, and
that is the only kind of finding this instrument exists to produce early. It couples the **layout
tree** to the **canvas compositor** — searchable, accessible, translatable DOM inside a WebGL/WebGPU
scene — and `manuk-compositor` is 526 LOC of aspirational Vello comments over a **tiny-skia CPU
raster**, with no seam of that shape anywhere. Recorded as **WATCH, do not build**: it is Chrome-only,
its adoption is unknown, and building for it now would violate I4 as squarely as chasing the encoding
tail did. But if it spreads, the cost is architectural rather than incremental, and the map should
have been carrying it before that becomes obvious.

The other two are cheap by comparison: Soft Navigations is a **measurement** surface (nothing on a
page breaks without it — the page works and the site's own RUM reports nothing), and Declarative
Partial Updates is parser-side, adjacent to streaming machinery this engine already has.

### RE-RANK

**None, and for the same reason as #34** — the binding Phase-0 constraint is the RENDER metric, and
none of these three is why a corpus page lays out wrong today. ⚠ But this window produced a *stronger*
reason than "they rank low", and it belongs here rather than only in a journal entry: **t937/t938
measured that ~2-3 of the 29 unscored in-scope sites are engine-attributable at all** — 17 are the
oracle's or the origin's, 7 bound a clock shared with Chrome. **The M1 ceiling is not a capability
backlog.** Adding capabilities, from this list or any other, cannot move it. That is an argument
about the METRIC, not about the map, and it is the thing most worth carrying out of this window.

---

## Audit #36 — tick 954 (2026-08-05) — the SERVO axis

**Sources, searched rather than recalled:**

* [April in Servo — new Android UI, focus, forms, security fixes](https://servo.org/blog/2026/05/31/april-in-servo/) (fetched, full feature list) · [February in Servo — faster layout, pause/resume scripts](https://servo.org/blog/2026/03/31/february-in-servo/)
* [Servo 0.4.0 released — LWN](https://lwn.net/Articles/1086555/) · [Servo Browser Engine Starts 2026 With Many Notable Improvements — Phoronix](https://www.phoronix.com/news/Servo-January-2026)

**Third axis in three audits, on purpose.** #34 took Interop/Baseline, #35 the Chrome-only frontier,
#36 takes **what an independent Rust engine chose to build** — which is a different signal from both:
not what the vendors agreed matters, and not what one vendor shipped first, but **what a team in our
exact position found it could not do without.** Servo has walked this road and its release notes are
a worklist written by someone with our constraints.

### RECONCILIATION — four additions, and one of them is a `dy` bug

Checked fourteen named Servo additions against `CONSTELLATION.tsv`, then against `engine/` by grep so
each verdict is evidence rather than a guess. Already present and correct: `color-mix()` (gated),
`activeElement` (gated), StorageManager (gated), `::details-content` (partial). Absent:

```text
   tab-size                    css   missing   0 files
   text-align: match-parent    css   missing   0 files
   revert-rule                 css   missing   0 files
   <select multiple>           html  unknown   "multiple" in 25 files, none a select-sizing path
```

⚠⚠ **`<select multiple>` IS THE ONE THAT MATTERS, AND IT IS A `dy` BUG.** A multi-select renders as a
**sized scrolling list box**, not a one-line dropdown — so on every filter sidebar, admin form and
faceted search that uses one, the control is the wrong height and **the entire column below it is
displaced**. That is the burndown's dominant error term arriving through a form control, on exactly
the form-heavy pages the CrUX tail is full of. Filed **`unknown`, not `missing`**: `multiple` appears
in 25 engine files and none of them is a select-sizing path, which is a grep result and not a probe.
The row carries the probe that settles it — render `<select multiple size=4>` against Chrome and
compare the box.

`tab-size` is the cheap second: every `<pre>` that indents with tabs currently collapses each tab to
one space's advance, so indented code loses its structure **and every line's wrap point moves**. It
is an advance-width rule in the shaper, not a layout algorithm.

Map now **474 rows · 283 gated · 118 missing · 44 partial · 18 unknown · 10 works.**

### RE-RANK

⚠ **Yes — a partial one, and it is the first re-rank any audit in this window has produced.** #34 and
#35 both concluded "none, the render metric binds". This axis found something that **is** the render
metric: `<select multiple>` is a control-height error, and a control-height error is a `dy` cascade.
It does not outrank the located `tz.de` footer divergence from t953 (which has numbers, an address
and a local reproduction), but it belongs **above** the remaining `overlap`/`h_overflow` dimension
work, because it is a single named control with a one-fixture probe rather than a site-shaped hunt.

**The methodological point, since three audits in one window now support it:** the axis you check
against determines what you can find. Interop and Baseline are lists of what the *platform* is
adding, and this engine's gap is not there. **An independent engine's release notes are a list of
what a browser needs to exist**, and that is the axis that found a `dy` bug.

## Audit #37 — tick 966 (2026-08-05) — the axis is the CORPUS, and it re-ranks everything

**Sources, searched rather than recalled:**

* [Interop 2026 focus areas](https://github.com/web-platform-tests/interop/blob/main/2026/README.md) (fetched, full list) · [Announcing Interop 2026 — WebKit](https://webkit.org/blog/17818/announcing-interop-2026/) · [Interop 2026 — web.dev](https://web.dev/blog/interop-2026)
* [This Month in Ladybird — July 2026](https://ladybird.org/newsletter/2026-07-31/) (fetched, full feature list) · [May 2026](https://ladybird.org/newsletter/2026-05-31/)

**A fourth axis, and it is not a source — it is a POPULATION.** #34 took Interop/Baseline, #35 the
Chrome-only frontier, #36 an independent engine's release notes. Each asked *"what does the world say
matters?"* This one asks the question those three cannot: **"and how much of it is on the pages that
compute our score?"** t965 built the instrument that answers it (`docs/loop/CORPUS-CONSTRUCTS.md` —
`curl` all 200 CrUX-trend URLs, grep the markup, three minutes, no build).

### RECONCILIATION — two genuine absences, and both are cheap to state

Checked all 20 Interop 2026 focus areas plus 14 named Ladybird July additions against
`CONSTELLATION.tsv` by grep, then against `engine/` by grep. Present and mapped: container queries,
anchor positioning, `attr()`, `contrast-color()`, CSS zoom, custom highlights, popover, IndexedDB,
JSPI, media pseudo-classes, Navigation API, scoped registries, scroll-driven animations, scroll snap,
`shape()`, view transitions, WebRTC, WebTransport, `contenteditable`, `system-ui`, JPEG XL, WebVTT.
**Zero occurrences in the map AND zero in the engine:**

```text
   CSS RELATIVE COLORS   rgb(from …) / oklch(from … l c h)    css    missing   0 files
   execCommand undo / redo                                    app    missing   0 files
```

Relative colors are CSS Color 5 and are how a design system derives a hover/disabled shade from one
token (`oklch(from var(--brand) calc(l - .1) c h)`); absent, the declaration fails to parse and the
element falls back to an inherited colour — a **visible** divergence, not a silent one. `undo`/`redo`
are the residue of an editing subsystem that already has eleven gates. Both added as `unknown` rows.

### ⚠⚠⚠ THE RE-RANK, and it is the largest this instrument has produced

**Interop 2026 — the list of what four vendors agreed matters most — is almost entirely ABSENT from
the corpus that scores us.** Priced on the same 171 pages:

```text
    3/171  1.8%  container-type / @container        0/171  0.0%  anchor-name / position-anchor
    3/171  1.8%  popover attribute                  0/171  0.0%  view-transition-name
    2/171  1.2%  scroll-timeline / animation-…      0/171  0.0%  IndexedDB (inline)
    2/171  1.2%  scroll-snap-type                   0/171  0.0%  @function
    1/171  0.6%  RELATIVE COLOR rgb(from …)         6/171  3.5%  zoom:
    1/171  0.6%  oklch()/lab()/lch() at all         7/171  4.1%  <dialog>
```

against, from the same run:

```text
   95/171  55.6%  <button>            69/171  40.4%  <input placeholder>
   88/171  51.5%  <input>             40/171  23.4%  <button> with an SVG child
   84/171  49.1%  @media              19/171  11.1%  appearance:none on a control
```

**This is not a criticism of Interop** — it ranks the *frontier for developers*, which is the right
job for it and the wrong input for a burndown. It is a statement that **the axis this audit has used
three times running and the metric the loop is scored on measure different populations**, and that
every "no re-rank found" verdict in #34 and #35 was partly an artefact of asking a list about a
corpus it was never about.

### ⚠⚠ AND THE PROBE SAYS THE TOP-RANKED CONSTRUCTS ARE ALREADY FINE — a NEGATIVE result, run before
### the ranking was believed

Frequency says where to look; it does not say there is anything there. A differential probe on
`<button>` and `<input>` — the corpus's #1 and #2 — against headless Chrome:

```text
                                        Chrome      ours      Δw     Δh
   <button>Search</button>             66.7 x 24   65 x 22   -1.7    -2
   <button><svg/></button>             32.0 x 24   30 x 23   -2.0    -1
   <button><svg/> Search</button>      87.1 x 24   85 x 23   -2.1    -1
   …the same, padding:8px 16px        107.1 x 38  105 x 37   -2.1    -1
   <input placeholder="…">            238.0 x 24  245 x 22   +7.0    -2
   <input value="…">                  238.0 x 24  245 x 22   +7.0    -2
   <input size=10 placeholder="…">    145.0 x 24  149 x 22   +4.0    -2
   <button appearance:none>            86.7 x 38   85 x 36   -1.7    -2
```

**The defect class t963 found on `<select>` — an intrinsic size model that is ABSENT rather than
wrong — does not generalise to `<button>` or `<input>`.** Both are within ~2px. Had the frequency
table alone been believed, the next several ticks would have gone grinding the corpus's two commonest
controls for a 2px return.

### THE ONE MEASURED LEAD, with its numbers

⚠⚠ **A BLOCK CONTAINING A LONE FORM CONTROL IS UP TO 10px TALLER HERE THAN IN CHROME, AND IN CHROME
IT IS EXACTLY THE CONTROL'S HEIGHT.** Reading the wrapper `<div>`s rather than the controls:

```text
   the <div> around …            control h      Chrome div     ours div
     <button>Search</button>         22/24          24            32     +10
     <button><svg/></button>         23/24          24            32      +9
     <button><svg/> Search</button>  23/24          24            25      +2
   ─────────────────────────────────────────────────────────────────────────
     y of #end, after nine controls                244           249
```

Chrome's wrapper is **exactly** the control's border box every time; ours adds a content-dependent
0–10px below it. That is the inline-block baseline/strut interaction t934/t935 worked in, arriving
through the corpus's most common construct — **and it is a `dy` term on 51–56% of the corpus**, which
is an order of magnitude more pages than the `<select>` work this session landed. Not built here; the
probe, the numbers and the fixture (`/tmp/fc.html`) are the specification, exactly as #36 → t958 →
t963 ran.

### THE METHODOLOGICAL POINT, since this audit changed its own method

> **Frequency ranks where to LOOK. A differential probe says whether anything is THERE. Neither is
> the other, and this audit produced its best finding and its most useful negative result from the
> two used together** — the negative on `<button>`/`<input>` cost one probe and saved a window.

Map now **476 rows** (+2: relative colors, execCommand undo/redo), both `unknown`.

### ⚠⚠⚠ AMENDMENT to #37's Finding 3, written at t967 — I NAMED THE WRONG ORGAN

Finding 3 above reported *"a block containing a lone form control is up to 10px taller here than in
Chrome"* and ranked it as a form-control defect at 51–56% corpus frequency. **The control was not the
cause and the number was inferred, not measured.** Finding 3's wrapper heights were computed as *the
next control's `y` minus this one's* — the `<div>`s carried no ids — and that quantity is only the
wrapper's height if nothing else is on the line.

**Re-measured with ids on the wrappers, the rows split cleanly:**

```text
   the <div> around …                    Chrome     ours
     <button>Search</button>               24        22     wrapper == control, both engines
     <input placeholder="…">               24        22     wrapper == control, both engines
     <button><svg/></button>               24        32     ✗
     <button><svg/> Search</button>        24        32     ✗
```

**Every diverging row contains an inline `<svg>`, and every clean row does not.** t967 then isolated
it away from `<button>` entirely — a bare `<div><svg 16x16></div>` is **30 here against Chrome's 20**,
while a 16×16 `<img>` in the same fixture is 20 in both — and fixed it: **a replaced element's
baseline is its bottom margin edge, and the §10.8.1 last-line-box search was being run on it.**

**What the audit got right and what it got wrong, kept separate because they rank differently:**

* ✅ The corpus-as-axis method found a real, high-frequency `dy` term where three source-based audits
  found none. Inline `<svg>` is **34.5%** of the corpus — *higher* than the `<button>`-with-an-icon
  figure the finding leaned on, and it was in the frequency table the whole time.
* ✅ The negative result on `<button>`/`<input>` (within ~2px) **stands, and is now better
  supported**: those rows are the ones that were already clean.
* ❌ The attribution was wrong, from a quantity that was inferred rather than measured, in an audit
  whose own headline is *"frequency ranks where to look; a probe says whether anything is there."*
  **Putting an id on the box you intend to talk about costs nothing and is not optional.**
* ⚠ Still open, and genuinely a form-control question: `<div><button><svg/></button></div>` is **32
  against Chrome's 24** *after* the svg fix — the same shape one level up, an inline-block's baseline
  when its last line box holds a vertically-aligned replaced item.

## Audit #38 — tick 977 (2026-08-06) — a FOURTH vendor axis says the same thing, and a CODE-shaped one says something new

**Sources, searched rather than recalled:**

* [WebKit Features for Safari 26.5](https://webkit.org/blog/17938/webkit-features-for-safari-26-5/) · [26.4](https://webkit.org/blog/17862/webkit-features-for-safari-26-4/) · [26.2](https://webkit.org/blog/17640/webkit-features-for-safari-26-2/) · [26.0](https://webkit.org/blog/17333/webkit-features-in-safari-26-0/) · [Touring New CSS Features in Safari 26 — CSS-Tricks](https://css-tricks.com/touring-new-css-features-in-safari-26/)

**Fourth vendor axis: WebKit.** #34 took Interop/Baseline, #35 the Chrome frontier, #36 Servo, #37
the *corpus as a population*. This one closes the vendor set with the one engine not yet asked.

### RECONCILIATION — and it is the third axis in a row to return the same structural answer

Twelve named Safari 26.x features checked against `CONSTELLATION.tsv`, against `engine/` by grep, and
then **priced on the burndown corpus** with t965's instrument:

```text
   field-sizing            0/171   0.0%      margin-trim             0/171   0.0%
   overflow-block/inline   0/171   0.0%      initial-letter          0/171   0.0%
   dynamic-range-limit     0/171   0.0%      Grid Lanes              0/171   0.0%
   :open pseudo-class      1/171   0.6%      position-try            1/171   0.6%
   justify-self            3/171   1.8%      align-self             14/171   8.2%   <- the exception
```

**Ten of twelve price at or below 0.6%.** #37 found this for Interop 2026's twenty focus areas and
called it a statement about *populations*; a second vendor list reproducing it makes it a property of
the axis rather than of Interop. **A vendor release list is a ranking of the developer frontier, and
the frontier is not where this engine's corpus lives.** That is now measured three times and should
stop being re-derived.

### THE ONE EXCEPTION, PROBED — and it is 5/6 already correct

`align-self` is the only item on the list with real corpus weight (8.2%), so it got a differential
probe rather than a map row:

```text
                                                    Chrome        ours
     align-self:center on a flex item              [0,  36]     [0,  36]   ✓
     align-self:flex-end                           [0, 152]     [0, 152]   ✓
     align-self:flex-start (container centres)     [0, 178]     [0, 178]   ✓
     align-self:stretch (container centres)     [0,264,60,80] [0,264,60,80] ✓
     align-self:end in a GRID                      [0, 456]     [0, 456]   ✓
     justify-self:end in a GRID                    [140, 350]   [  0, 350]  ✗
```

**`align-self` is correct in flex AND grid; `justify-self` in a grid is unimplemented** — the item
sits at the track's start where Chrome puts it at the end, a 140px error on the one row. Priced at
**1.8%** of the corpus, so it is a real but small lever, recorded with its number rather than ranked
above the burndown.

### ⚠⚠⚠ THE NEW AXIS, and it is CODE-shaped rather than source-shaped

t975 and t976 each found a **whole capability** hidden inside the engine's own structure — not behind
a missing feature but behind a construct that reads as a decision:

* **t975** — `Translate3D`/`Scale3D`/`Rotate3D`/`Matrix3D` fell into a `_ => {}` whose comment said
  *"3D/perspective skipped — our paint model is 2D"*: true of a genuine 3D effect, false of
  `translate3d(x,y,0)`. **The reason was doing the work a measurement should do.**
* **t976** — `transform-origin` was unimplemented behind a **parameter that existed, was documented,
  and was passed a hard-coded constant by all three call sites**.

Neither is findable from any vendor list, from Baseline, or from the corpus — the property *parses*,
the name *greps*, the doc *explains it*. So this audit adds a sweep of the engine's own silent
fall-throughs:

```text
   silent `_ => {}` / `_ => None` arms across engine/*/src      112
     engine/css/src/lib.rs            29        engine/paint/src/lib.rs        7
     engine/page/src/lib.rs           14        engine/css/src/stylo_map.rs     5
     engine/js/src/dom_bindings.rs    11        engine/dom/src/lib.rs           5
     engine/css/src/values.rs          9        engine/a11y/src/lib.rs          4
     engine/layout/src/lib.rs          9        (compositor, html)              2
     engine/css/src/stylo_engine.rs    7
   ── of those, carrying a JUSTIFYING comment (t975's exact shape)   28
```

**112 is a worklist, not a defect count** — most catch-alls are correct. The audit's claim is
narrower and testable: **the 28 with a justification are the ones no instrument can audit**, because
the justification is indistinguishable from a measurement to every reader including the author. The
two checked so far were both wrong. **The next audit should sample from that 28 rather than from a
vendor list**, and this one is recording the list so the sampling is possible.

> ⚠⚠⚠ **AMENDED AT t978 — I TESTED THIS RECOMMENDATION ONE TICK LATER AND THE RATE ABOVE IS WRONG.**
> The two highest-priced justified catch-alls (`display: table*` 8.8%, `clip-path` 6.4%) were probed
> against Chrome: **9 of 9 rows identical, both justifications hold.** *"The two checked so far were
> both wrong"* is a prior **selected on the outcome** — t975 and t976 became visible precisely
> because they were wrong. The honest rate is **2 of 3 testable**.
>
> **And the third candidate was not testable by the instrument used, which is the larger finding.**
> `clip-path` is a PAINT effect: it changes no box in either engine, so a geometry dump reports
> identical rows whether the clip is applied perfectly or not at all. **A geometry probe cannot audit
> a paint-only fall-through, and it reports success while failing to.** The 28 must be triaged by
> *which instrument can see them* — geometry (`boxes`), paint (a raster diff), or JS/DOM (a page
> probe) — before they are a worklist. **An enumerable population is not an auditable one.**

### RE-RANK

**No re-rank on the vendor axis** — ten of twelve items are absent from the corpus, and the one with
weight is already correct. **A partial re-rank on the code axis:** the 28 justified catch-alls are now
a named, enumerable population with a 2-for-2 hit rate, which is a better prior than any list this
audit has checked. `justify-self` in grid is recorded as the vendor axis's one measured lead (1.8%).

Map now **478 rows** (+2: `align-self` gated by probe, `justify-self` grid partial with its number).

## Audit #39 — tick 987 (2026-08-06) — the axis is a FIXTURE, and it prices its own negative space

**Sources, searched rather than recalled:** none. **That is the finding.** #34 took Interop/Baseline,
#35 the Chrome frontier, #36 Servo, #37 the corpus as a population, #38 WebKit — closing the vendor
set. Three of those five returned the same structural answer (the developer frontier is not where
this corpus lives), and #38 said it "should stop being re-derived". This audit takes the loop at its
word and asks a different question: **not *what has the industry shipped* but *what does OUR engine
do to the CSS it already claims to support*.**

### THE AXIS — a twenty-row property battery against headless Chrome, one run

t984 built one fixture covering twenty flex/grid features that had no gate: `place-self`, `order`, the
`gap` / `flex-flow` / `flex` / `grid` shorthands, `span`, `grid-template-areas`, `min-content`,
`fit-content`, percentage gaps, baseline self-alignment, `space-evenly`, `inline-grid`,
`aspect-ratio`, `min-height:0`, `flex-basis:content`, `margin:auto`. t986 built a second, sixteen rows
of positioned/overflow/stacking geometry. Both were diffed against Chrome in a single run each.

```text
   battery              rows   exact   diverging   real defects   instrument artefacts
   flex/grid sizing       20      18           2              2                      0
   positioned/overflow    16      13           3              2                      1
   ─────────────────────────────────────────────────────────────────────────────────
                          36      31           5              4                      1
```

**Four real defects from two fixtures, and none of them was predicted.** All four landed as ticks
985–987: `width: fit-content` given up on inside flex/grid, a percentage `gap` with nowhere to be
stored, a transformed ancestor not acting as a containing block, and `will-change`/`contain`/
`perspective` likewise.

### WHY THIS AXIS BEATS THE VENDOR ONES, stated as a measurement and not a preference

A vendor list ranks *the frontier*. #37 and #38 measured what that costs here: ten of twelve Safari
26.x features and Interop 2026's twenty focus areas price at **≤0.6% of the corpus**. The battery
axis has the opposite property — every row is a construct the engine **already claims**, so a
divergence is by construction a broken promise rather than an unbuilt feature. Corpus weight of the
four found: `transform` **34.5%**, `display:grid` **18.7%**, `width:fit-content` (the `w-fit` idiom)
and percentage gaps unmeasured but structurally inside those two populations.

⚠⚠⚠ **AND IT PRICES ITS OWN NEGATIVE SPACE, WHICH NO OTHER AXIS THIS LEDGER HAS TRIED DOES.** The
31 exact rows are 31 constructs nobody now has to re-check, banked at the same cost as the 5. Every
previous audit produced a *worklist*; this one produces a worklist **and a cleared field**. That is
the difference between "here is what to look at" and "here is what is already right", and only the
second one ever shrinks.

### THE ROW THAT PAID FOR THE PATTERN LEDGER

One of the three positioned-battery divergences was **not a defect**:

```text
   overflow-y:scroll content width      Chrome 300      ours 285
```

The reference runs `--hide-scrollbars`; our scrollbar model correctly reserves the 15px Chrome would
also reserve if it had one. Recorded in `WEB-PATTERNS.md` since t930 as *"an INSTRUMENT bug; never fix
the engine for it"* — and it presented here as a clean, plausible 15px divergence **in a fixture built
for something else entirely**. A fresh reading would have filed it. **The ledger caught a false
finding in the one place a false finding looks most convincing: inside an otherwise-correct batch.**

### WHAT THE FOUR DEFECTS HAVE IN COMMON, and it is a shape worth naming

None of them is a missing arm. Every one is a construct that **greps as covered**:

```text
   fit-content   the keyword parses, maps out of Stylo, and the BLOCK path consumes it in SIX
                 places — the taffy path had `FitContent => return None`
   gap %         the value arrives from Stylo intact and is narrowed to 0.0 on the NEXT LINE,
                 because the field is an `f32` and cannot hold a percentage
   transform CB  `abs_containing_block` asked `position != Static`, got a truthful answer, and
                 acted correctly on it — the QUESTION was wrong
   will-change   no `ComputedStyle` field at all: not an unhandled value, a value with nowhere
                 to live
```

**Two of the four are "the value has nowhere to live" and one is "the right code asked the wrong
question".** A `grep -rn "will-change" engine/` returns nothing and reads as an absence; a
`grep -rn "fit-content" engine/` returns *eleven* hits and reads as coverage. **Neither grep finds
the defect. A fixture finds both.**

### STEER

1. **Run a battery per area, not per property.** Two fixtures, ~40 minutes of authoring, four landed
   defects and 31 cleared constructs. The families still unbatteried: **text/inline metrics**
   (`letter-spacing`, `word-spacing`, `text-transform`, `word-break`, `overflow-wrap`, `text-indent`,
   `white-space` variants), **backgrounds/borders** (`background-size` keywords, `border-image`,
   multi-layer backgrounds, `border-radius` with two-value corners), and **tables** — which VI.2 has
   named as residue mass since check #82 and which no battery has yet touched.
2. **Write the negative rows first.** t987's `will-change` predicate would have shipped wrong from
   the property names alone; the four Chrome-measured NEGATIVE rows (`will-change: opacity`,
   `contain: style`, `contain: size`, nothing-declared) are what made it right, and the naive
   predicate passes all ten positive rows.
3. **Stop re-deriving the vendor answer.** Three axes, one conclusion, now four audits old.

## Audit #40 — tick 998 (2026-08-07) — the map had NO ROW for the family that was carrying an inverted cascade

**Sources searched today** (not from memory — the protocol's first requirement):

```text
  https://www.w3.org/TR/css-logical-1/                     css-logical-1 §Cascading, §Inheritance
  https://github.com/w3c/csswg-drafts/issues/7054          `revert`/`revert-layer` with logical properties (OPEN)
  https://webkit.org/blog/17818/announcing-interop-2026/   Interop 2026 focus areas
  https://web.dev/blog/interop-2026                        …and the vendor-side framing
  https://github.com/web-platform-tests/interop/blob/main/2026/README.md
  https://web.dev/blog/baseline-digest-may-2026            Baseline, May 2026
  https://ladybird.org/                                    + coverage of Ladybird's 2026 WPT position
```

### ⚠⚠⚠ WHAT WE HAD BEEN WRONG ABOUT — and it is the audit's own failure mode, not a mis-score

`grep -ic "logical propert" docs/loop/CONSTELLATION.tsv` over 478 rows returned **0**.

The map names `zoom`, `attr()`, style queries, view transitions, anchor positioning, WebTransport,
Service Worker, WebGPU, `revert-rule`, `subgrid`, `text-wrap: balance`, `tab-size`,
`text-align: match-parent` — a long, specific, well-maintained frontier. It had **no row at all for
the logical-property family**, in any of the six classes.

> **This is the shape the audit exists to catch, and it is the first time it has caught it this
> cleanly.** The family was not scored optimistically. It was not `unknown`. It was **absent** — so no
> instrument the loop owns could have surfaced it, including this one, until something outside the
> ranking walked into it. Tick 996 walked into it while writing a `<fieldset>` UA rule in logical
> properties and finding that `* { margin: 0 }` did not reset it.

And what was behind the missing row was not a missing feature — the logical longhands all parse, map
and reach layout — but a **cascade inversion, 7 conflicts of 7, on 25.9% of the corpus**:
`PropertyDeclarationBlock::push` de-duplicates on `id()`, which collapses same-longhand pairs and made
an ascending merge look correct for sixty ticks; a logical/physical pair is the one case it cannot
collapse. Full mechanism in `docs/wiki/css-cascade.md` and the ledger row.

**The generalisation, because one instance is an anecdote:** every row on the map is a capability
someone could NAME. A property *family* that is uniformly implemented has no obvious row to write, so
it gets none — and a defect in how the family **interacts with the cascade** is then invisible to a
capability-shaped map. Three rows added below; the class to keep asking about is **interactions
between two things that are each individually present.**

### ADDED to CONSTELLATION.tsv (3 rows)

```text
  css  logical properties CASCADE AS ONE GROUP with their physical counterparts   gated   G_CASCADE_LOGICAL_PHYSICAL
  css  the mapping uses the writing mode ON THE ELEMENT, not the parent's        gated   G_CASCADE_LOGICAL_PHYSICAL
  css  UA `!important` outranks author `!important`                              missing  -
```

⚠ **The second row is satisfied BY DELEGATION, and finding that out changed the fix.** css-logical-1
§Cascading: *"all properties cascade using the writing mode specified on the element, not on its
parent"* — with inheritance running logical-to-logical, so an LTR parent's `margin-inline-start`
inherits into an RTL child's `margin-inline-start` even though those are opposite physical sides. The
implementation t998 first considered was to resolve logical→physical **inside our merge**, from
`parent_cv`'s writing mode. That is not "more code for the same result" — **it is the wrong result**,
and the search is what said so. Stylo applies `writing-mode`/`direction` as *prioritary* properties
before `to_physical()`, so delegating the mapping is both smaller and correct.

⚠ **The third row is a residue named with a reason to leave it.** `revert` / `revert-layer` semantics
*with* logical properties are an **open CSSWG issue** (#7054, still open in 2026). Implementing our
guess at an unsettled spec is the kind of work that has to be undone; the row records the gap and the
reason rather than pretending it is scheduled.

### RECONCILED — Interop 2026 and Baseline, against the corpus

Interop 2026's **20 focus areas**: Anchor Positioning, advanced `attr()`, View Transitions (now
including cross-document), WebRTC (carried over from 2025), WebTransport (HTTP/3), the `zoom` CSS
property, container **style queries**; investigations cover accessibility, mobile, WebVTT, JPEG XL.
**Every one already has a row.** `zoom` is `partial` (`G_ZOOM_AND_PROBE_PINS`), `attr()` is `partial`,
container queries *including style queries* is `gated`. Audit #39's finding stands unchanged and is now
two audits old: **the vendor axis prices at ≈0 on this corpus** — and Baseline's May-2026 digest
confirms why, since container style queries only reached Baseline Newly Available in **May 2026** and
still only accept custom properties in every shipping engine.

**Ladybird**: 90% of WPT in October 2025, ~95% now; their named hard tail is WebRTC, WebAuthn, Service
Worker, WebGPU, media codecs, WASM-heavy sites, *"some modern CSS layouts"*. Three of those six are
already settled decisions here (WebRTC out of scope, WebAuthn built, EME out of scope), Service Worker
is PLATFORM MAP item 3 and unbuilt, and the sixth — *modern CSS layout* — is where this loop already
is. ⚠ **No re-rank.** An independent engine at 95% WPT naming CSS layout in its hard tail is a
confirmation of CO-#1, not a redirection of it.

### STEER

1. **Ask the map for INTERACTIONS, not only capabilities.** The next audit should try the same grep on
   other uniformly-implemented families and look for the interaction row that is missing:
   `!important` × origin (already added as `missing` this audit), shorthand × longhand ordering within
   one block, custom property × fallback × `revert`, and `all: unset` against a UA sheet. Each of those
   is *"two present things meeting"*, which is the shape that has no natural row.
2. **The 25.9% figure came from fetching stylesheets, and `CORPUS-CONSTRUCTS.md` was reading 13.5%.**
   That file declares its CSS rows to be floors; it now carries the multiplier (~3×) and the extra
   `xargs` stage that removes it. **Re-run the corpus construct ranking WITH stylesheets before the
   next time it is used to rank anything** — every CSS row in it is currently a third of its real value,
   which is enough to reorder the list.
3. **Nothing found today re-ranks the board.** Said plainly rather than padded: two audits running, the
   external axis has confirmed CO-#1 instead of moving it. The finding that mattered came from *inside*
   the work, and the audit's contribution was to explain why the map could not have produced it.

## Audit #41 — tick 1008 (2026-08-07)

**SOURCES (searched, not recalled):**

- `https://github.com/web-platform-tests/interop/blob/main/2026/README.md` — the twenty Interop 2026
  focus areas and its four investigation efforts, read as a list rather than summarised.
- `https://webkit.org/blog/17818/announcing-interop-2026/` · `https://web.dev/blog/interop-2026`
- `https://web.dev/blog/baseline-digest-may-2026` (and the Jan/Apr digests) — what became Baseline
  Newly available in 2026.

### → Interop 2026, reconciled row by row: the map already had all twenty

```text
   container style queries · anchor positioning · attr() · contrast-color() · CSS zoom ·
   custom highlights · dialogs and popovers · fetch uploads and ranges · IndexedDB · JSPI ·
   media pseudo-classes · Navigation API · scoped custom element registries ·
   scroll-driven animations · scroll snap · shape() · view transitions · web compat ·
   WebRTC · WebTransport        →  every one has at least one CONSTELLATION.tsv row
   investigations: accessibility testing · JPEG XL · mobile testing · WebVTT  →  all present
```

**That is a real result and it is the first time it has happened**: forty audits in, the vendors'
own agreed priority list contains nothing this map has never heard of. It is also exactly the
condition this instrument warns about — *"an audit that finds nothing is a suspicious audit"* — so
the search did not stop there.

### → What the Baseline digests found, and one of the two matters

⚠⚠⚠ **`hyphens: auto` HAD NO ROW, AND IT IS A LINE-COUNT PROPERTY.** `grep -ic hyphen` over 485 rows
returned **1**, and that one is a *soft-hyphen* comment inside the line breaker. `engine/css` does
not parse the property at all. This is not a paint-level nicety: `hyphens: auto` changes **where
prose breaks**, therefore how many lines a paragraph occupies, therefore the y of everything below
it — **the dy cascade the burndown names as mechanism #1**, arriving through a property the map could
not see.

Priced the same hour, HTML + linked stylesheets over the corpus that produces M1:

```text
   hyphens: auto declared        15/171   8.8%
   font-family: math             0/171    0.0%
   <math> in the served HTML     0/171    0.0%
```

Added as `unknown`, not `missing`: what our line breaker does when the property is absent may already
match Chrome on most content, and the honest next step is a fixture, not an assumption.

**`font-family: math` / MathML** added too, and **explicitly not ranked** — 0 of 171. Recorded so
the map is honest about the surface rather than about the corpus.

### → What this audit did NOT do

**No re-rank.** Twenty-for-twenty on Interop and one 8.8% property found is a confirmation of CO-#1
(the render leg), not a redirection of it. Said plainly rather than padded.

### STEER

1. **`hyphens: auto` is a fixture, and it is cheap.** One 20-row battery against Chrome: `auto` vs
   `manual` vs `none`, with and without `lang`, at widths that force a hyphenation opportunity. It
   sits directly on the mechanism the burndown ranks first, and the map could not have produced it —
   which is the entire argument for this instrument.
2. **The previous audit's steer — "ask the map for INTERACTIONS, not only capabilities" — is still
   open** and was not worked this time. Carry it.
3. ⚠ **The reconciliation method that worked here is a GREP OF THE MAP PER NAMED FEATURE**, not
   reading the map and asking what is missing. Twenty-two greps took two minutes and are falsifiable;
   "does anything look absent" is the question that has been wrong six times.

## Audit #42 — tick 1018 (2026-08-08)

**SOURCES (searched, not recalled):**

- `https://ladybird.org/newsletter/2026-07-31/` — an independent engine's own month-by-month list of
  what it just built, read as a list of NAMES rather than as a narrative.
- Carried from #41: the Interop 2026 focus areas and the 2026 Baseline digests (re-greped, no change).

### → The method from #41 held, and it found four more rows

**Grep the map once per NAMED feature.** Fourteen greps over Ladybird's July list, two minutes:

```text
   relative colors · @function · style queries · @container style() · Geolocation ·
   contenteditable · undo · system-ui · if()                        -> all PRESENT
   WebAudio · clipboard events · smooth scrolling · rich text        -> NO ROW
```

⚠⚠⚠ **`scroll-behavior: smooth` had no row and is 23.4% of the corpus.** `grep -ic "smooth scroll"`
over 487 rows returned **0**, and `engine/css` does not parse the property. It is the highest-weight
thing this audit found, and it arrived through an independent engine's release notes rather than
through anything the loop owns.

⚠⚠ **AND `scroll-margin` / `scroll-padding` came with it, at 15.2%** — the pair that keeps an anchor
target from landing underneath a sticky header. Also absent from the map and from the cascade.

Priced the same hour, HTML + linked stylesheets over the corpus that produces M1:

```text
   scroll-behavior: smooth              40/171   23.4%
   scroll-margin / scroll-padding       26/171   15.2%
   AudioContext / webkitAudioContext     0/171    0.0%
   a copy/cut/paste listener             1/171    0.6%
   navigator.clipboard                   0/171    0.0%
```

### → The steer applies t1010's rule BEFORE the work, not after

⚠⚠⚠ **`scroll-behavior: smooth` is a strong candidate to be UNMEASURABLE, and that must be probed
before a line is written.** A scroll *animation* has no steady-state geometry, and this oracle
compares a single settled snapshot — the same structural blindness that made `hyphens: auto` a tick
that must not be built (t1010). **`scroll-margin`/`scroll-padding` is the opposite**: it changes the
offset a scroll *settles* at, which is a real steady state — but only an instrument that scrolls can
see it, and this one does not. So the honest ordering is:

1. **Probe the oracle** on both, exactly as t1010 did: a fixture, Chrome, before any engine work.
2. If `scroll-margin` is measurable only by a scrolling instrument, that is an INSTRUMENT tick with
   a named consumer, not a capability tick — and it should be priced against the 15.2% it unlocks.
3. `WebAudio` and clipboard events are recorded for map honesty and **explicitly not ranked** at
   0.0% and 0.6%.

### → What this audit did NOT do

**No re-rank.** The two live rows are 23.4% and 15.2% against a render leg whose current work is at
69% (flex overflow) and 48.5% (`break-word`), so they do not displace anything. And the same
observation as #41 holds one level up: **an independent engine's release notes are a better source of
unknown-unknowns than the vendors' priority list**, because Interop names what the four vendors
already agree on — which is, by construction, the part of the platform least likely to be missing
from anyone's map.

### ⚠⚠⚠ ADDENDUM, same tick: the steer above was EXECUTED rather than filed

Both findings were probed against the oracle before anything was written, and both came back
**invisible**. Chrome's own boxes, a 200×60 scroll container with three 40px children:

```text
                                              Chrome            ours
   (no scroll property)                    200x60 / 200x40   identical
   scroll-behavior: smooth                 200x60 / 200x40   identical
   scroll-padding-top: 24px                200x60 / 200x40   identical
   scroll-margin-top: 24px (on the child)  200x60 / 200x40   identical
   scroll-snap-type + scroll-snap-align    200x60 / 200x40   identical
```

Ten of ten rows byte-identical in BOTH engines. So this instrument cannot see any of the four
properties, and we agree with the reference today for the same reason the reference agrees with
itself: nothing has scrolled.

⚠⚠⚠ **AND THAT SPLITS "UNMEASURABLE" INTO TWO KINDS, WHICH IS THE FINDING WORTH KEEPING.**

```text
   hyphens: auto     the REFERENCE IS MIS-PROVISIONED   -> building it makes us DIVERGE. Harmful.
                     (Chrome would differ if it had dictionaries; it does not have them)
   scroll-*          the PROPERTY HAS NO STEADY STATE   -> building it is INVISIBLE. Unpriceable.
                     (Chrome shows no difference either, because nothing scrolled)
```

The first cannot be fixed by instrumenting harder — the reference itself is wrong for our purposes.
The second is precisely what a **scrolling instrument** would unlock, and it now has a named consumer
and a number: 23.4% + 15.2% of the corpus. **A capability the oracle cannot see is not one fact but
two, and they lead to opposite decisions.**

## Audit #43 — tick 1028 (2026-08-08) — a SECOND independent engine, and the UA SHEET had never been audited at all

**SOURCES (searched, not recalled):**

- `https://servo.org/blog/2026/07/31/june-in-servo/` — Servo 0.4.0's own named-feature list.
  **A source this loop has never used.** #41 and #42 both used Ladybird; taking a *second*
  independent engine tests whether #42's conclusion (*"an independent engine's release notes beat
  Interop for unknown-unknowns"*) was about Ladybird or about the method. It was about the method.
- `https://ladybird.org/newsletter/2026-07-31/` — re-checked; **no August issue exists yet**, so
  #42's Ladybird pass is still the latest and was not re-greped.

### → Half one: the map, greped once per NAMED feature (the #41/#42 method, third run)

Thirty-five greps over Servo's list. Ten came back with **no row**, and pricing them on the corpus
that produces M1 (171 HTML + 416 stylesheets over 120 sites) sorts them hard:

```text
   animation-delay (incl. the 2-time shorthand)   63/171   36.8%   <- see the caveat, it is not a lever
   text-decoration-style: wavy|dotted|dashed      38/171   22.2%
   font-feature-settings                          28/171   16.4%
   overflow: clip                                 26/171   15.2%   <- THE FINDING, see half three
   input minlength / maxlength                    17/171    9.9%
   @media device-width / device-height            10/171    5.8%
   closest-corner / farthest-corner                2/171    1.2%
   FontFaceSet / document.fonts                    0/171    0.0%
   console.dir                                     0/171    0.0%
   webkitRelativePath                              0/171    0.0%
```

⚠⚠ **Two of the top three are UNPRICEABLE BY THIS ORACLE and must not be ranked on those numbers**,
by the rule audit #42 established and t1010 before it:

- **`animation-delay` has no steady state.** 36.8% is the loudest number on the page and it is the
  same class as `scroll-behavior: smooth` — a single settled snapshot cannot see a time-varying
  property. ⚠ The 36.8% is also a **soft** number: the regex admits the two-time `animation:`
  shorthand, which over-counts. Neither half of that should reach a tick without a probe first.
- **`text-decoration-style` is PAINT, not geometry.** A wavy underline moves no box, so M1 is
  structurally blind to it while a human sees it on 22.2% of the corpus — the *"the instrument
  cannot price this"* class from check #72, not the *"this bought nothing"* class.

### → Half two: THE UA STYLESHEET HAD NEVER BEEN AUDITED, AND t1027 FOUND ONE OF ITS GAPS BY ACCIDENT

Every audit so far has checked *capabilities*. Nobody checked the **UA stylesheet** — which is odd,
because it is the most enumerable document in the engine: a finite list of elements, and the
reference will recite its own copy on request. Tick 1027 found `iframe { border: 2px inset }`
missing while looking for something else, and *"found by accident"* is the signature of a surface
nothing is watching.

**The instrument, and it is one command.** Instantiate every HTML element on a page with no author
CSS, ask Chrome for the computed value of 31 UA-settable properties on each, and diff each element
against a `<span>` — so the output is exactly *"the declarations Chrome's UA sheet makes"*, 83 of 102
elements.

⚠⚠⚠ **AND THE MATCHER LIED FOR THE THIRD TIME IN THIS AUDIT'S HISTORY — CAUGHT BEFORE PUBLISHING
THIS TIME, BY THE ENGINE.** Diffing those 83 against `UA_CSS`'s selectors said **37 elements our
sheet never names**, including `div`, `li` and `section` — which obviously render as blocks, because
the corpus works. Their default `display` lives in **Rust, not in the sheet**. #33 and #34 each
published a wrong number from exactly this shape (grep the artefact, infer the engine); the fix is
not a better regex, it is **to ask the engine**. A 21-row fixture against Chrome:

```text
   CLEARED, though the grep flagged them (the sheet is not the only producer):
     em · i · cite · dfn · var  (italic)    address · figcaption · summary · output · bdi
     div · li · section · header · nav · article  (block)

   REAL, engine-verified, with their corpus weight:
     <small>    144x19 vs 120x15   font-size: smaller not applied      15/171   8.8%
     <audio>    0x19   vs 0x0      display:none not applied             2/171   1.2%
     <search>   144x19 vs 400x20   display:block not applied            1/171   0.6%
     <legend>   x=0    vs x=2      2px inline padding missing           1/171   0.6%
     <ruby>     48x19  vs 16x19    display:ruby not applied             0/171   0.0%
     <hgroup>   144x19 vs 400x20   display:block not applied            0/171   0.0%
     <nobr>     144x39 vs 299x19   white-space:nowrap not applied       0/171   0.0%
     <big>      144x19 vs 173x23   font-size: larger not applied        0/171   0.0%
```

**The honest headline is that the UA sheet is in better shape than the grep suggested and worse than
nothing**: 8 real gaps, of which exactly **one** (`<small>`, 8.8%) carries corpus weight and the rest
are ≤1.2%. ⚠ **An audit that produced a big scary list and then cleared 29 of 37 rows is the audit
working**, and it is the reason the clearing pass is not optional: acting on 37 would have meant
writing UA rules for `div` and `li`.

### → Half three: THE RANKED FINDING, and it is one predicate

`overflow: clip` is on **15.2%** of the corpus and it is in our enum, mapped out of Stylo, and
handled — so the map's *"no row"* was map-honesty, not a capability gap. It is also **wrong**, and an
8-row probe says the error is a single line:

```text
                                                     chrome        ours
   overflow:clip box containing a float             200x0        200x60   <- does NOT contain it
   overflow:clip box, child with margin-top:30px    200x10       200x40   <- margin escapes
   overflow:hidden containing a float  CONTROL      100x60       (both contain it)
   overflow:clip clipping its own height CONTROL    200x40       200x40   ok
```

**`overflow: clip` must NOT establish a block formatting context.** It is the one overflow value
defined to clip *without* becoming a scroll container, and `layout::establishes_bfc` asks
`s.overflow != Overflow::Visible` — which lumps `Clip` in with `hidden`/`scroll`. So a `clip` box
contains floats it should let escape and swallows margins that should collapse through it, and both
consequences fall out of one predicate.

⚠ **Its 15.2% is an upper bound on where it can BITE**, not a lower one: the defect only shows where
a float or a collapsing margin is actually involved. That has to be measured on the fix, not
asserted here.

### → RE-RANK, and it does displace what was queued

The tick-1027 write-up queued `iframe { border: 2px inset }` next (29.2% of sites carry an iframe).
**`overflow: clip` outranks it** and the audit is why: 15.2% with **two independent geometric
consequences from one predicate**, against 29.2% with a 4px one — and the `clip` fix needs no paired
change to avoid regressing anything, where the iframe border cannot land without `frameborder="0"`
becoming a hint in the same tick.

```text
   1. overflow: clip must not establish a BFC        15.2%   one predicate, two consequences
   2. iframe UA border + frameborder hint            29.2%   4px, and it needs the paired hint
   3. <small> font-size: smaller                      8.8%   one UA declaration
   4. font-feature-settings                          16.4%   shaping -> advances -> geometry; unprobed
   5. animation-delay / text-decoration-style        36.8% / 22.2%   PROBE FIRST; likely unpriceable
```

### → What this audit says about its own method

**#42's conclusion generalised.** A second independent engine, never used before, produced a
15.2% geometric defect that three years of Interop lists and every instrument this loop owns had not
surfaced. The claim is now about the *method* and not about Ladybird: **release notes from an engine
that is re-deriving the platform name the things a consensus list has already stopped mentioning.**

**And a new standing rule, from half two.** The reference will recite its own configuration if you
ask it — Chrome's UA sheet, its `hover`/`pointer` answer (t1020), its viewport (t1016). ⚠ **Every
element of the platform that the reference can be asked to ENUMERATE should be enumerated once,
because a gap in an enumerable surface is otherwise found only by tripping over it** — which is
exactly how `iframe`'s border was found, one tick before this audit.

## Audit #44 — tick 1039 (2026-08-08) — a THIRD engine, thinner yield, and a 9× inflation hiding in a legacy no-op

**SOURCES (searched, not recalled):**

- `https://webkit.org/blog/18178/webkit-features-for-safari-26-6/` and Safari Technology Preview
  247–249 — **a third independent engine, never used by this loop.** #42 used Ladybird, #43 used
  Servo; taking WebKit tests whether the *method* keeps producing or whether the first two sources
  were the yield.

### → The method's third run, and the honest answer is DIMINISHING

Fifteen greps over WebKit's named list. `position-area` / `anchor-name` / `anchor-center` (CSS anchor
positioning) have **no row**; `ic` unit has no row; `compileStreaming` options and
`iceTransportPolicy` have no row (WebRTC is an explicit non-goal). Priced:

```text
   CSS anchor positioning (position-area / anchor-name / anchor())    3/171   1.8%
   the `ic` length unit                                               7/171   4.1%   ⚠ grep unreliable
   Service Worker registration                                       10/171   5.8%   (XL, already deferred)
```

⚠⚠ **Nothing here outranks anything.** And the reason is worth recording as a property of the
SOURCE rather than of the platform: **Safari 26.6 and STP 247–249 are overwhelmingly BUG FIXES**
(*"CSS math functions produced an incorrect signed zero"*, *"a nested clip-path ignored css zoom"*,
*"grid items with a computed preferred size that behaves as auto did not correctly compute their
minimum content contribution"*). A fix list names things the engine **already had**; a feature list
names things it just got. **#42's and #43's yield came from release notes that enumerate FEATURES,
and this one does not** — so the rule from #42 (*"an independent engine's release notes beat Interop
for unknown-unknowns"*) needs the qualifier: **it depends on the document type, not the vendor.**

### → THE FINDING, and it is about a number the map already carries

`CSS zoom` **already has a map row**, cited as an Interop 2026 focus area. Pricing it against the
corpus produced **28.1%**, which would make it one of the highest-weight unbuilt rows on the board.
It decomposes:

```text
   `zoom` declared anywhere                     48/171   28.1%
     ├─ `zoom: 1` — the legacy IE hasLayout hack 35/171   20.5%   <- a NO-OP
     └─ `zoom` with a real value                 4/171    2.3%   <- the actual capability
```

⚠⚠⚠ **A LEGACY NO-OP INFLATED A CORPUS FREQUENCY BY NINE TIMES.** `zoom: 1` was written to trigger
IE's `hasLayout` and does nothing in any engine shipping today; it survives in resets and vendor CSS
by the ton. Ranking `zoom` on 28.1% would have bought a subsystem to serve **4 sites**.

> **This is the third distinct way a corpus grep has lied to this loop, and they now form a set:**
> an unanchored property grep matching a *class name* (`hover`, #43 — inflated by half); a
> *co-occurrence* standing in for same-element application (t1025, `42.4%` vs `49.4%`); and now a
> **legacy no-op value** standing in for the live capability. All three inflate, none deflate, and
> all three are invisible in the number itself. **A frequency is not a measurement until its
> VALUES have been looked at, not just its property name.**

### → Reconciled, applied

`CONSTELLATION.tsv` gains anchor positioning (`unknown`, 1.8%) and the `ic` unit (`unknown`, with its
grep flagged unreliable). The existing `CSS zoom` row is **corrected in place** to carry 2.3% and the
`zoom: 1` decomposition, so the next reader cannot re-derive 28.1% and rank on it.

### → Is any invariant being bent?

**No.** Twelve ticks this window, zero edits under `scripts/`. ⚠ The `WPT-AREAS.tsv` staleness named
in check #94 is unchanged and remains observer-owned — `tick.sh` printed *"the sweep is 554h old"*
above a green ratchet again this window.

## Audit #45 — tick 1050 (2026-08-08) — the map is COMPLETE against the outside world, and has a hole exactly where this session worked

**SOURCES (searched, not recalled):**

- `https://github.com/web-platform-tests/interop/blob/main/2026/README.md` — the authoritative
  Interop 2026 list: **20 focus areas + 4 investigation efforts**, agreed by Apple, Google, Igalia,
  Microsoft and Mozilla. Fifteen of the twenty are new for 2026.
- `https://webkit.org/blog/17818/announcing-interop-2026/` · `https://web.dev/blog/interop-2026`
- `https://web.dev/baseline/2026` and the 2026 monthly Baseline digests
- `https://ladybird.org/newsletter/2026-07-31/` (and 2026-01/05/06) — the independent-engine source
  audits #42–#44 established; July 2026: Ladybird moved its style system and layout engine to Rust,
  and its WPT gain has fallen to **+108 subtests in July** against +13,690 in January.

### 1 · THE RECONCILIATION CAME BACK CLEAN, AND THAT IS THE FIRST TIME

Every one of the **twenty** Interop 2026 focus areas has a named row in `CONSTELLATION.tsv`:

```text
   container style queries · anchor positioning · attr() · contrast-color() · zoom ·
   custom highlights · dialogs+popovers · fetch uploads+ranges · IndexedDB · JSPI ·
   media pseudo-classes · Navigation API · scoped custom element registries ·
   scroll-driven animations · scroll snap · shape() · view transitions · web compat ·
   WebRTC · WebTransport
```

…and so do all four investigation efforts (accessibility testing, JPEG XL, mobile testing, WebVTT),
and the 2026 Baseline additions checked (`:active-view-transition`, `zstd` Content-Encoding,
`shape()`, `contrast-color()`, Trusted Types). **Zero rows added from the world's own list.**

⚠ Audits #42–#44 already showed this method yielding less each run (#44: *"the honest answer is
DIMINISHING"*). #45 is the endpoint of that trend, and the useful reading is not *"the audit failed"*
— it is that **the outside-in method has been exhausted**, because every source the world publishes
is a list of FEATURES, and this engine's binding constraint stopped being feature coverage some time
ago. The map is 288 gated / 124 missing / 45 partial / 31 unknown over 503 rows, and Interop's list
adds nothing to any of those columns.

### 2 · ⚠⚠⚠ SO I POINTED THE AUDIT INWARD, AND FOUND A HOLE UNDER THIS SESSION'S OWN FEET

`block-in-inline` — CSS 2.1 §9.2.1.1 — has **zero rows on the map.** t1048 measured it on 30.0% of
the corpus (51/170 pages, 1,925 elements), found six defects, fixed one Chrome-exact and gated it.
The horizontal margin on an inline (§10.3.1) landed at t1050 against the corpus's **#1 cross-site
width cluster** — 21 sites, 244 hits — and has no row either.

**Both were found by a battery, not by the map, and neither could have been ranked from it.**

### 3 · THE MECHANISM, WHICH IS THE AUDIT'S REAL OUTPUT

Layout-primitive rows *do* exist — `an inline box's OWN leading` (227), `line box containing ONLY
empty inlines` (419), `margins collapse THROUGH an empty block` (483), `a float FOLLOWING inline text`
(484). Every one was added **after** a tick discovered it. Not one was on the map before the defect
was found.

> **The map is built from lists the world publishes, and nobody publishes a list of layout
> primitives — so the CSS 2.1 box model's interior is the one region the map can only RECORD, never
> RANK.** Every other class (APIs, formats, features, Interop areas) is enumerable from outside.

And that is a false constraint, because **§9 and §10 are themselves an enumeration** — a numbered
list of sections, each a primitive with a testable claim. The loop already knows this move under a
different name: t1027-1031's rule is *"ENUMERATE every surface the reference can recite"*, applied to
Chrome's UA sheet. The same move applied to the SPEC instead of the reference converts the main line
from *find a defect, then file it* into a ranked worklist with a denominator.

**THE STEER (the next non-fix tick, and it is cheap — no build):** walk CSS 2.1 §9 (visual formatting
model) and §10 (details of visual formatting model), and add one `unknown` row per subsection this
engine has never measured. The ratchet rewards this directly — the banked invariant is MEASURED, not
`unknown` — and it gives the render burndown the one thing it has never had: **a list of layout
primitives that exists before the bug does.**

### 4 · CORRECTED / ADDED

- **ADDED** `css · block-level box inside an INLINE (CSS 2.1 §9.2.1.1)` — `gated`, G_BII (t1048).
- **ADDED** `css · horizontal margin on a non-replaced inline (CSS 2.1 §10.3.1)` — `gated` (t1050).
- **CONFIRMED, not corrected**: row 419's measured table (`margin-left:10px` leaves its div 0 tall)
  is what t1050's control was copied from, and it held — the map's own measurement was reused as a
  gate two ticks later, which is the first time that has happened and is what the receipts are for.

## Audit #46 — tick 1060 (2026-08-09) — the CHECKOUT was drawn from the map and the map from the checkout, and 442 tests the runner CANNOT SCORE

### 1 · SOURCES SEARCHED (not from memory)

- Interop 2026 focus + investigation areas — fetched the **authoritative list** from
  `github.com/web-platform-tests/interop/blob/main/2026/README.md` (20 focus areas, 4 investigations),
  not a blog summary of it.
- Ladybird 2026 status (alpha targeted 2026, ~90% WPT, Rust LibJS frontend landed Feb 2026).
- `github.com/web-platform-tests/wpt/tree/master/css` — the upstream `css/` directory listing.

### 2 · THE OUTSIDE-IN AXIS IS EXHAUSTED, CONFIRMED A THIRD TIME

Every one of Interop 2026's **20 focus areas and all 4 investigation areas** already has at least one
row on `CONSTELLATION.tsv` — container style queries, anchor positioning, `attr()`, `contrast-color()`,
`zoom`, custom highlights, dialogs/popovers, fetch uploads+ranges, IndexedDB, JSPI, media
pseudo-classes, Navigation API, scoped custom element registries, scroll-driven animations, scroll
snap, `shape()`, view transitions, web compat, WebRTC, WebTransport, and accessibility testing / JPEG
XL / mobile testing / WebVTT. **Zero rows added from the outside world, for the third audit running**
(#44, #45, #46). Audit #45's conclusion stands: the yield has moved to enumerating *specs*, and t1054
–t1060 discharged the CSS 2.1 §8/§9/§10 enumeration completely — **zero unknowns and zero
receipt-only rows** in that range.

### 3 · ⚠⚠⚠ THE FINDING: THE WPT CHECKOUT IS A PARTIAL CLONE CONTAINING EXACTLY WHAT THE RATCHET ALREADY TRACKS

`RATCHET.tsv` banks 21 `WPT:` invariants and publishes `WPT:TOTAL 422865`. That total has a
denominator nobody has ever stated:

```text
   upstream wpt/css/            ~93 directories
   local    wpt/css/             16   (CSS2 + the exact 15 the ratchet tracks)
   upstream wpt/ top level      ~90 directories
   local    wpt/ top level       23
```

**The checkout contains the areas the loop decided to track, and the loop tracks the areas the
checkout contains.** That is a closed loop, and it is precisely the shape this instrument exists to
break: *"the loop becomes very good at ranking things inside a frame that may be the wrong frame."*
`css-inline`, `css-box`, `css-align`, `css-tables`, `css-writing-modes`, `css-logical`, `css-break`,
`css-multicol`, `css-lists` and `css-pseudo` are all absent from disk — and the first four are the
literal subject of the last twelve ticks.

### 4 · ⚠⚠⚠ AND 1359 TESTS ARE ALREADY ON DISK AND HAVE NEVER BEEN RUN — MEASURED THIS AUDIT

Four directories are present locally and carry **no ratchet row at all**. Run against the current
binary, this audit, for the first time:

```text
   svg        38 passed · 108 FAILED ·  623 skipped   (769)
   mathml     84 passed ·  66 FAILED ·  440 skipped   (590)
   wai-aria    0        ·   0        ·  264 skipped   (264)
   accname     0        ·   0        ·  178 skipped   (178)
```

Two separate facts, and they must not be merged:

- **174 known failures are UNBANKED.** A number the ratchet never marks cannot go backwards, so
  `svg` and `mathml` are outside the ratchet's protection entirely — 1359 tests that can silently rot.
- ⚠⚠⚠ **CORRECTED AT TICK 1064 — THIS ROW WAS WRONG AND IS WITHDRAWN.** It originally read *"442 of
  them the reftest runner CANNOT SCORE AT ALL … this is not a low score, it is NO score."* That is
  true of `manuk-wpt <dir>`, the **reftest** runner, and materially false about the loop:
  `manuk-wpt wpt <dir>` is the **testharness** runner (`tests/wpt/src/harness.rs`, 672 lines) and
  scores both directories fine — **`wai-aria` 238/434 = 54.8%, `accname` 306/481 = 63.6%, together
  544/915 = 59.5%, HANG/CRASH 0.** The audit read `SKIP — needs JS/testharness` as a property of the
  loop when it is a property of the subcommand chosen. **A SKIP IS A STATEMENT ABOUT THE RUNNER, NOT
  ABOUT THE CAPABILITY.**
- **What survives, narrower and better:** `wai-aria` and `accname` have **no `WPT:` row in
  `RATCHET.tsv`**, so 915 subtests — **371 of them failing** — sit outside the ratchet and can rot
  silently. That was the real finding; only the word *unmeasurable* was wrong.

### 5 · WHAT WE HAD BEEN WRONG ABOUT

**That a directory-count gap is a capability gap.** The first pass of this audit read "16 of 93 css
directories" and was about to add dozens of `unknown` rows. Hand-checking each one against the map —
t1054's rule, *verify every MISS before it becomes a row* — killed almost all of them: multicol,
tables, ruby, exclusions, paged media, writing-modes, counter-styles, MathML and SVG **all already
have rows**. Only **two** survived. A false gap is worse than a missed one, and this audit nearly
manufactured seventy of them.

### 6 · CORRECTED / ADDED

- **ADDED** `css · CSS fragmentation — break-before / break-inside / break-after` — `unknown`.
- **ADDED** `css · scroll anchoring` — `unknown`.
- **NOT ADDED, deliberately**: one row per absent WPT directory. The map is a map of *capabilities*,
  and the absent directories overwhelmingly describe capabilities it already carries. The finding is
  about the **instrument**, not the map, and is recorded as such.

### 7 · THE STEER

1. **The WPT denominator must be stated wherever the total is published.** `WPT:TOTAL 422865` reads
   as a total over WPT and is a total over 23 hand-picked directories. This is harness territory
   (`scripts/wpt-expand.sh`, `RATCHET.tsv` generation) — **flagged for the observer, not touched.**
2. **`svg` and `mathml` are 1359 tests, on disk, unbanked, with 174 known failures.** Banking them
   costs nothing to discover and immediately extends the ratchet's protection.
3. ⚠ **CORRECTED AT TICK 1064.** This item read *"the a11y conformance surface is STRUCTURALLY
   invisible to the reftest runner … the largest unmeasured surface the audit found."* The
   testharness runner already exists and scores it: **544/915 = 59.5%**. The accurate item is
   **BANK IT** — `wai-aria` and `accname` carry no ratchet row, so 371 failing subtests on the I3
   moat cannot regress because nothing marks them. Build no runner; add the rows.

## Audit #47 — tick 1069 (2026-08-09) — the loop hand-built an oracle for an area that already had a 1,140-test one on disk

### 1 · WHAT WAS MEASURED (every number below was run this audit, not recalled)

The last five ticks (1065–1068) were all found by **one instrument**: a ~30-row HTML fixture, authored
by the loop, diffed against headless Chrome on `(x, y, w, h)` through `manuk-wpt boxes --html`. It
found nine defects and it is the most productive thing the loop owns. This audit asks the question
that instrument cannot ask about itself: **what is it not looking at, and what is already on disk?**

### 2 · ⚠⚠⚠ FINDING 1 — `css/CSS2` IS 9,221 TESTS ON DISK, HAS NEVER BEEN RUN, AND HAS NO RATCHET ROW

```text
   manuk-wpt ~/wpt/css/CSS2        1606 passed · 4040 FAILED · 3575 skipped   (9221 total)
     …of which CSS2/tables           68 passed ·  175 FAILED ·  897 skipped   (1140 total)
```

**`CSS2/tables` is the literal subject of ticks 1065 and 1066**, where the loop hand-authored a
41-case §17 fixture, discovered five defects and fixed four. A 1,140-test §17 conformance suite was
sitting in the checkout the whole time and **was never consulted** — not to rank the work, not to
check it afterwards. The 43 subdirectories are a §-by-§ map of CSS 2.1: `abspos`, `floats`,
`normal-flow`, `visudet`, `visuren`, `tables`, `zindex`, `stacking-context`, `linebox`, `margin-padding-clear`.
That is the enumeration t1054 spent a tick constructing by hand, as a directory listing.

> **THE BATTERY AND THE SUITE ANSWER DIFFERENT QUESTIONS, AND THAT IS NOT A DEFENCE.** A suite gives
> a NUMBER and a regression guard; a battery gives a MECHANISM. None of the five §17 defects would
> have been *localised* by a pass count, so the battery was the right tool — **the finding is that
> the loop never LOOKED**, and a 175-failure work-list for the exact area it was working sat unopened.

### 3 · ⚠⚠⚠ FINDING 2 — EVERY BANKED `WPT:` NUMBER IS ONE LANE OF TWO, AND NOTHING SAYS WHICH

The loop has two runners. `manuk-wpt <dir>` is the **reftest** (paint-diffing) runner; `manuk-wpt wpt
<dir>` is the **testharness** runner — the distinction t1064 published as a correction. Run both
against three directories the ratchet already tracks:

```text
                        RATCHET.tsv     testharness lane        reftest lane
   css/css-backgrounds        3         27/86   = 31.4%      173 passed / 561 scored
   css/css-position          63         99/311  = 31.8%        9 passed /  83 scored
   css/css-display           10        124/151  = 82.1%       14 passed /  98 scored
```

**The banked number matches neither lane, in any of the three rows.** It may well come from a
different invocation inside `scripts/wpt-sweep.sh` (harness-owned, not inspected here) — the point is
that `RATCHET.tsv` records a bare integer with **no lane, no subset and no denominator**, so a reader
cannot tell what moved when it moves. `WPT:TOTAL 422865` inherits all of it. This is #46's *"a total
over WPT that is a total over a hand-picked subset"* one level deeper: not just which directories, but
**which runner**.

### 4 · ⚠⚠⚠ FINDING 3 — `TH_TIMEOUT` IS A SILENT DENOMINATOR CUT, WHICH IS THE ONE THING THE CERTIFICATION REDESIGN FORBIDS

```text
   css/css-backgrounds   FILES 121   TH_TIMEOUT 104      subtests 27/86
   css/css-position      FILES 110   TH_TIMEOUT  39      subtests 99/311
   css/css-display       FILES  31   TH_TIMEOUT  14      subtests 124/151
```

**104 of 121 files in `css-backgrounds` never completed**, and the 86-subtest denominator is what the
remaining **17** reported — 121 files averaging five subtests would be ~600, so a timed-out file
contributes zero to the numerator *and* zero to the denominator. `DAILY-DRIVER-CERTIFICATION.md` §2
is explicit that *"a timeout/crash/bot-wall is a COUNTED FAIL … NEVER a silent drop (dropping the
hard sites is what made every past reading optimistic)"*. That rule was written for the fidelity
corpus and **the WPT lane does the exact thing it forbids**: `31.4%` is a score over the tests that
finished, published as a score over the directory.

### 5 · ⚠⚠ FINDING 4 — THE ONLY DISCOVERY INSTRUMENT THE LOOP USED FOR FIVE TICKS IS STRUCTURALLY BLIND TO PAINT

`boxes --html` reports `(x, y, w, h)`. It cannot see colour, background rendering, border style,
stacking order, or anything that requires a scroll or an interaction — and **`css/css-backgrounds` is
the lowest-scoring directory on the whole lever board (3.5%)**. The loop has a paint-diffing
instrument, it is the reftest runner, it is on disk, and **not one tick this session used it.** Five
consecutive ticks in a row is exactly the clustering this instrument exists to notice: not that the
work was wrong, but that the *method* selected the findings.

⚠ And the same blindness explains a fixture failure this session: t1067's grid battery could not
express `position: sticky` at all (no scroll), which is why sticky remains `gated` with no measurement
behind it after four batteries in adjacent areas.

### 6 · WHAT SURVIVES FROM #46, RE-CHECKED

`svg` (108 failed), `mathml` (66 failed), `wai-aria` and `accname` (371 failed) remain unbanked. Add
to them, measured this audit: **`html-aam` — 15 files, `253/335 = 75.5%`, 82 failures, no ratchet
row, never mentioned in any prior audit.** And `css/CSS2`'s 4,040. **Total known-failing subtests
outside the ratchet's protection: ~4,667.**

### 7 · WHAT WE HAD BEEN WRONG ABOUT

**That "the WPT checkout contains what the ratchet tracks" (#46) was the whole shape.** It is not: the
checkout contains **six directories the ratchet does not track**, one of them larger than every css
directory the ratchet does track, and it is the CSS 2.1 core suite. #46 counted directories against
upstream and concluded the frame was closed; it never counted the directories *inside the frame that
carry no row*. A closed loop was diagnosed correctly and its largest instance was missed.

### THE STEER, in order

1. ⚠⚠⚠ **Open `CSS2/tables`'s 175 failures against the t1065/t1066 tree.** It is a ready-made,
   already-paid-for work-list for the exact area two ticks just landed in, and it will say which of
   the four fixes moved conformance and which did not — the attribution neither tick could get.
2. **State the LANE wherever a WPT number is published**, and give `RATCHET.tsv`'s `WPT:` rows a
   denominator. Harness-owned (`wpt-sweep.sh`, `status-update.sh`) — flagged, not touched.
3. **`TH_TIMEOUT` must be a counted FAIL**, not a silent drop, or every published WPT percentage is
   optimistic by the fraction of the directory that hangs — 86% of it, in `css-backgrounds`.
4. **Take one PAINT tick.** The reftest runner is a reference-diffing instrument the loop already
   owns and has never used for discovery; `css/CSS2/backgrounds`, `zindex` and `stacking-context` are
   its ranked entry points, and paint is where the geometry battery cannot follow.

**Next audit due: tick 1079.**

---

## Audit #48 — tick 1079 (2026-08-09). The world's list is fully covered, and that is the finding.

**Sources read this audit (web, not memory):**

- `https://github.com/web-platform-tests/interop/blob/main/2026/README.md` — the canonical list
- `https://webkit.org/blog/17818/announcing-interop-2026/` · `https://web.dev/blog/interop-2026`
- `https://web.dev/baseline/2026` and the 2026 Baseline monthly digests
- `https://ladybird.org/newsletter/2026-06-30/` — the independent-engine reference point

### 1 · RECONCILED: Interop 2026's twenty focus areas and four investigations, one by one

Container style queries · CSS anchor positioning · CSS `attr()` · `contrast-color()` · CSS zoom ·
custom highlights · dialogs and popovers · fetch uploads and ranges · IndexedDB · JSPI for Wasm ·
media pseudo-classes · Navigation API · scoped custom element registries · scroll-driven animations ·
scroll snap · CSS `shape()` · view transitions · web compat · WebRTC · WebTransport. Investigations:
accessibility testing · JPEG XL · mobile testing · WebVTT. Baseline 2026 additions checked as well:
`:active-view-transition`, `shape()`, `contrast-color()`, Trusted Types, zstd `Content-Encoding`.

**Every one already has a row in `CONSTELLATION.tsv`. Zero rows added from the external lists.**

### 2 · ⚠⚠⚠ FINDING 1 — THE EXTERNAL LISTS HAVE NOW ADDED NOTHING TWICE, WHILE THE LAST TWO TICKS BOTH FOUND TOTAL ABSENCES

t1048-1055's audit recorded that Interop 2026 added 0 rows while `block-in-inline` — 30% of the
corpus — had none. That was read then as one miss. It is a **pattern**, and this audit is where it
gets named: the two capability ticks immediately preceding it each found a feature that was **not
implemented at any layer**, and *neither could ever appear on an Interop, Baseline or "what's new"
list*, because both are twenty-seven years old:

```text
   t1078   ::first-letter        CSS 2.1 §5.12.1   absent   10.5% of the CSS 2.1 suite's failures
   t1079   border-*-color/style  CSS 2.1 §8.5      uniform  16% of them (via *-applies-to-NNN)
```

**A list of what is NEW cannot rank what is OLD AND MISSING.** The vendors' agenda is, by
construction, a list of things all four engines already ship and disagree about at the edges. Ours is
an engine that does not ship some of the middle. So the surface audit's Interop pass is now a
*negative control* — useful for proving the map is not behind the world — and the discovery
instrument is `~/wpt/css/CSS2`, the 9,221-test CSS 2.1 core suite.

### 3 · THE HONEST CSS 2.1 NUMBER, which belongs on the map

```text
   css/CSS2   3006 passed · 2640 failed · 3575 skipped   (9,221 on disk)
              32.6% of the directory · 53.2% of what the runner can run
```

⚠ The lane matters (audit #47's rule): this is the **reftest** runner, and 3,575 files are skipped —
so `53.2%` is the score over what ran and `32.6%` is the score over the directory. Both are stated;
neither is published alone.

### 4 · FINDING 2 — THE WORST CHAPTER BY RATIO IS `visufx`, AND A PROBE NAMES ITS CAUSE

Ranked by pass rate rather than by failure count, which is what the last three ticks' rankings were
missing:

```text
   visufx              1 / 48     2.1%     <- worst
   linebox            14 / 190    7.4%
   bidi-text          17 / 105   16.2%
   generated-content  45 / 190   23.7%
   lists              37 / 82    45.1%
   text              161 / 354   45.5%
```

A ranking is a place to look, not a diagnosis, so `visufx`'s three primitives were probed directly
rather than inferred:

```text
   visibility: hidden / visible    correct
   overflow: hidden                correct (clips 80px of content to 20px)
   clip: rect(0,50px,20px,0)       NOT APPLIED — the full 100x40 box paints
```

**`clip` is unimplemented and has no row on the map.** Added as `missing`.

### 5 · FINDING 3 — FOUR MORE CSS 2.1 SURFACES WITH NO ROW AT ALL

Checked by grep against both the map and `engine/`:

```text
   empty-cells       no map row · ZERO occurrences in engine/       — a CSS 2.1 table property
   caption-side      no map row · implemented in engine/layout      — claimed by nothing
   quotes / open-quote / close-quote   no map row · no engine hits
   the `ex` unit     no map row · ~10% short of Chrome (t1079)      — `em` is exact
```

`empty-cells` and `quotes` are the same shape as t1078 and t1079: **absent, old, and unnameable by
any external list.**

### 6 · WHAT WE HAD BEEN WRONG ABOUT

**That the surface audit's job is to reconcile against the world's list.** It is not, or not only.
Reconciling against Interop/Baseline is *cheap and has now returned nothing twice*, while the two
ticks either side of this audit found absences worth 26% of a 9,221-test suite between them. The
audit's real question is the one the loop's own instruments cannot ask: **which sections of the specs
we claim to implement have never had a single test run against them?** For CSS 2.1 the answer is on
disk and enumerable — 43 chapter directories — and it is the reason this arc keeps paying.

### THE STEER, in order

1. ⚠⚠⚠ **Rank the CSS 2.1 chapters by PASS RATE, not by failure count.** `visufx` (2.1%) and
   `linebox` (7.4%) never appeared in any of the last three ticks' rankings because they are small;
   a chapter at 2% is a missing primitive, a chapter at 45% is a tail.
2. **Take `clip: rect()`** — one CSS 2.1 §11.1.2 primitive, probe-confirmed absent, and `visufx`'s
   47 failures are its work-list.
3. **`linebox` at 7.4% is the largest low-rate chapter (176 failures)** and sits directly on the
   `vertical-align`/line-box arc t913–t935 built. Rank its failure names before assuming.
4. Keep the Interop pass, but run it as the **negative control it has become** — a few minutes to
   prove the map is not behind the world, not the audit's main body.

**Next audit due: tick 1089.**

## Audit #49 — tick 1089 (2026-08-10). The map has 545 rows about the ENGINE and none about the INSTRUMENTS

SOURCES (searched, not recalled):
- `https://web-platform-tests.org/writing-tests/reftests.html` — WPT's own definition of what a
  reftest runner must do before it screenshots.
- `https://webkit.org/blog/17818/announcing-interop-2026/` · `https://web.dev/blog/interop-2026` ·
  `https://www.igalia.com/news/interop-2026.html` — Interop 2026: 19 focus areas, 3 cleanup, 4
  investigation; the four carrying 20% of the score are **advanced `attr()`**, **IndexedDB
  `getAllRecords()`**, **WebTransport** and **Wasm JSPI**; Anchor Positioning and **cross-document**
  View Transitions are the headline additions.
- `https://ladybird.org/newsletter/2026-05-31/` — Ladybird 2,067,263 → 2,075,546 WPT subtests in May
  2026 (+8,283/month), the independent-engine reference rate.

### 1 · Interop 2026 returns ZERO new rows for the THIRD audit running, and that is now a result

Every one of the six named areas above already has a row in `CONSTELLATION.tsv` (anchor 12 matches,
`attr(` 4, `getAllRecords` 1, WebTransport 1, JSPI 1, view transitions 3). Audits #47 and #48 said the
same. **A list of what is NEW cannot rank what is OLD AND MISSING** — recorded again, and this audit
stops treating Interop as a discovery source. It is a *coverage check*, and it passes.

### 2 · ⚠⚠⚠ WHAT WE HAD BEEN WRONG ABOUT: the map describes the ENGINE and nothing describes the INSTRUMENTS

`CONSTELLATION.tsv` has **545 rows**, every one a browser capability. t1088 found that the reftest
runner had been rendering **19.6% of `css/CSS2`'s references undressed** — 1,230 of 6,263 references
are drawn out of `<img>` swatches and it fetched no subresources — and **no row in the map could ever
have surfaced that**, because the map has no notion of an instrument having capabilities. The audit
that would have caught it is the one nobody runs: *point the audit at the measuring apparatus.*

So this audit does that, against WPT's own documented contract for a reftest runner, and prices each
gap against the corpus instead of asserting it:

```text
   WPT says a reftest screenshots after…        ours                         css/CSS2 weight
   the `load` event has fired                   no JS at all (skipped)       49 files use reftest-wait
   WEB FONTS are loaded                         not fetched                  10 files (0.2%)  ← THIN
   pending paints complete                      synchronous raster           n/a
   …in an 800×600 window incl. scrollbars       VW=800 VH=600                ✅ matches
   (implied) the document's subresources        <img> + background-image ✅   1,230 refs (19.6%)  ← FIXED t1088
                                                external stylesheets ✗       1,231 (19.7%)  ← THE NEXT ONE
```

⚠⚠ **The `@font-face` row is the reason to price before building.** WPT's documentation names web
fonts as an explicit screenshot precondition, so a reader of the spec would rank it high; in this
chapter it is **10 files of 6,263** and ranks dead last. The externally-sourced requirement and the
corpus disagree, and the corpus wins (VI.3). ⚠ It will NOT rank last in `css/css-fonts`, which is
unmeasured — that is a separate question and is flagged, not answered.

⚠⚠⚠ **AND THE NEXT LEVER IS ALREADY SIZED: external stylesheets, 1,231 of 6,263 (19.7%)** — the same
order of magnitude as the `<img>` blind spot, the same shape (a document whose meaning is in a file
the runner never fetched), and t1088 deliberately did not bundle it so that its own +429 would stay
attributable.

### 3 · ADDED to the map

`CONSTELLATION.tsv` is a browser-capability ledger and these are not browser capabilities, so they are
NOT forced into it. They belong to the conformance runner and are recorded here as its work-list,
with the numbers above:

1. `reftest: external stylesheets` — status `missing`, 19.7% of `css/CSS2` reftests.
2. `reftest: web fonts` — status `missing`, 0.2% here, unmeasured in `css/css-fonts`.
3. `reftest: reftest-wait / load event` — status `missing`, 49 files; needs the JS path the runner
   currently uses to SKIP a test outright.

⚠ **The standing correction this audit leaves behind:** every future audit must spend one of its
questions on the instruments, not only on the platform. Three of the loop's largest single findings —
the mis-provisioned reference (#93), the unrun subcommand (t1057), and t1088's undressed reference —
were defects in apparatus that a capability map is structurally unable to represent.

## Audit #50 — tick 1099 (2026-08-10). THE METRIC CANNOT SEE A THIRD OF THE WINDOW'S WORK, BY CONSTRUCTION

This audit had an obvious subject handed to it: the window closed with **`css/CSS2` 3,029 → 3,854
(+825, the largest suite movement in the loop's history)** and **M1 flat at 23 sites for the fourth
sweep running**. Either the fixes were worthless or the map is wrong about what M1 measures. The
map, as usual.

### The finding: the fidelity probe enumerates `querySelectorAll('*')`

```js
   // tests/wpt/src/chrome.rs — the structural probe, run identically on BOTH engines
   var all = document.querySelectorAll('*');
   …
   out[pathOf(e)] = [t, cs.display, x, y, w, h, '', cs.position];
```

**A pseudo-element has no DOM node.** `::before` and `::after` are not in `querySelectorAll('*')`,
they have no `pathOf`, and they are therefore absent from *both* sides of the diff. So for
generated content the probe cannot report:

- whether the box exists at all,
- where it is or how big it is,
- what it says.

⚠⚠⚠ **THREE OF THIS WINDOW'S FOUR CAPABILITY FIXES ARE GENERATED CONTENT** — t1092 (block-level
`display`), t1093 (`display:none`), t1096 (counters). Every one is Chrome-exact on its own probe and
gained suite tests with zero losses, and **M1 has no term in which any of them can appear.** That is
not a flat metric; it is a metric that was never pointed at them.

### What it CAN see, and why that is weak rather than zero

The second-order effect survives: a block-level `::before` makes its owner a line taller, which
pushes DOM siblings down, and siblings *are* enumerated. But `shape` scores each element within 8px
**against a shared ancestor**, so a uniform downward shift of a subtree is substantially normalised
away, and what remains often sits inside tolerance. The direct effect is unscoreable and the
indirect one is attenuated by the metric's own design. **A fix can be right, Chrome-exact, and
worth 825 suite tests while M1 is entitled to read exactly zero.**

### Corrections to the map

1. **M1's blind spots are now THREE, and the map records one.** The known one is paint-only
   properties (`clip`, filters, blend modes — audit #49's `clip` row, 36% of pages). Add:
   **(2) generated content, in its entirety**, and **(3) TEXT — the probe records `tag, display, x,
   y, w, h, position` and never a string**, which is why t1096 could ship counters that rendered
   `S0.` where Chrome renders `S1.` and pass a "Chrome-exact" width check.
2. **The suite and the corpus metric measure disjoint things more than the loop assumed.** They
   have now disagreed by 825 tests in one window. Neither is wrong; the map was wrong to treat M1 as
   a superset.
3. ⚠ **This is the same structural fact that made the ACCESSIBILITY tree blind** (t1097: generated
   content is not in the DOM, and the AX tree is built from the DOM). One fact, three consumers —
   the AX tree, the fidelity probe, and the oracle's cluster ranking — and it was only ever noticed
   in the first. *When a structural absence bites one consumer, enumerate the others before
   assuming it is local.*

### The re-rank this forces

**Do not read "M1 flat" as "generated content was not worth doing"** — the corpus says 68% of pages
carry a block-level pseudo and 47% a `display:none` one, and the metric cannot price either. The
honest options, in order:

1. **Teach the probe pseudo-elements.** `getComputedStyle(e, '::before').content` is available on
   both engines and the probe already calls `getComputedStyle(e)` for every node. This is a small
   change to an instrument the loop owns, and it converts an entire invisible class into a scored
   one — the same shape as t1088 and t1090, which each bought hundreds of tests by making the
   instrument able to see its own corpus.
2. Record TEXT in the probe, which closes blind spot (3) and would have caught the counter defect.
3. Keep the CSS 2.1 suite as the ranking instrument for anything M1 cannot express, and stop
   treating a flat M1 as a verdict on work M1 has no term for.

## Audit #51 — tick 1109 (2026-08-10) — the arc the loop has been working for four ticks had NO MAP ROW

**Sources, read this session, not from memory:**

- `https://github.com/web-platform-tests/interop/blob/main/2026/README.md` — the 20 focus areas and
  4 investigation efforts, verbatim
- `https://ladybird.org/newsletter/2026-07-31/` and `/2026-02-28/` — what an independent engine
  found hard in 2026, and in what order
- `https://www.unicode.org/reports/tr14/` (UAX #14) and
  `https://www.unicode.org/L2/L2019/19041-linebrk-soft-hyphen.html` — the soft-hyphen tailoring

### What the world named that we did not

**Interop 2026 added ZERO rows again** — all 20 focus areas (container style queries, anchor
positioning, `attr()`, `contrast-color()`, `zoom`, custom highlights, dialogs/popovers, fetch
uploads+ranges, IndexedDB, JSPI, media pseudo-classes, Navigation API, scoped custom element
registries, scroll-driven animations, scroll snap, `shape()`, view transitions, web compat, WebRTC,
WebTransport) already have rows. That is the second consecutive audit where the reference list is
fully covered, which is itself the warning: **the reference list is not where our gaps are.**

⚠⚠⚠ **THE FINDING, AND IT IS THE EMBARRASSING KIND. Ticks 1105–1108 were spent entirely on the
soft-wrap-opportunity rule — the largest single shape movement the anchor site has ever had — and the
capability map had NO ROW for any of it.** `white-space`: zero rows. `wrap opportunity`: zero rows.
`soft hyphen`: zero rows. `UAX`: one row, and it was `CJK line breaking` from tick 225. The map had
`hyphens: auto` (unmeasurable) and `CJK line breaking` (gated) and nothing between them, so the
mechanism underneath both was invisible to every instrument that reads the map.

**This is the map-drawn-from-memory failure running in the OTHER direction.** The audit exists to
catch capabilities the world has and we lack; this time it caught **three capabilities we BUILT and
never claimed**, sitting beside two we lack and could not see. A map that cannot see the work in
flight ranks the next tick against a frame that is missing its own main line.

### Added (7 rows)

```text
   gated    a soft wrap opportunity belongs to the element CONTAINING the space (CSS Text §3)
   gated    NO break opportunity where there is no white space (adjacent inline siblings)
   gated    a greedy line breaker must REWIND to the last break opportunity
   gated    `::before`/`::after` on a NESTED INLINE element
   partial  UAX #14 line breaking — the Unicode algorithm rather than a hand-rolled split
   missing  soft hyphen U+00AD is a HYPHENATION opportunity (break AND render a visible hyphen)
   missing  a collapsible space at the end of a line is removed — for GENERATED CONTENT
```

Map drift is back at its pre-existing 7 (the new `partial` row was re-stated as `missing` rather than
left as a bare assertion). MEASURED 515 of 551; `unknown` still 36.

### What we had been wrong about

**`break_segments` is a hand-rolled line-break opportunity finder and the map called the result
"CJK line breaking, gated".** Ladybird shipped the real UAX #14 in 2026 and named CJK and long words
as what it fixed — the same two things our tick-225 probe checked. Four of the five classes we can
currently measure are Chrome-exact (CJK, hyphen, `<wbr>`, `overflow-wrap:break-word`); the fifth,
soft hyphen, is not, and we only know that because t1108's battery happened to include a row for it.
**The classes a hand-rolled finder does not know are invisible until a site hits one**, which is
exactly the shape of an unknown unknown, and the row now says so instead of claiming a gate.

### The re-rank

It does not displace the t1109 sweep's work-list — three sites are one defect from M1 and that is
still the cheapest tier. But the two new `missing` rows are both **line-count** defects, and a wrong
line count is the burndown's own named mechanism #1 (a whole-subtree `dy` cascade below it), so they
belong in the same queue rather than in a text-rendering backlog: the generated-content one is
already the named next brick, and it is now on the map where the next audit can see it.

## Audit #52 — tick 1119 (2026-08-10) — the map enumerates FEATURES, and the defect lived at an INTERSECTION

**Sources, read this session, not from memory:**

- `~/wpt/css/css-flexbox/abspos/` and `~/wpt/css/css-grid/abspos/` — the two directories the suite
  devotes to this intersection, read as a work-list and then RUN (the pass-sets, not the totals)
- CSS Flexbox §4.1 *"absolutely-positioned children"* and CSS Grid §9 *"absolute positioning"*, via
  the `rel=help` links the failing tests carry
- The map itself, queried rather than recalled: `awk -F'\t'` over `docs/loop/CONSTELLATION.tsv`

### The finding: 8 abspos rows, 34 flex/grid rows, and ZERO at their intersection

```text
   rows matching  absolut|abspos|out-of-flow|static position  .....  8
   rows matching  flex|grid  ..........................................  34
   rows matching  BOTH  ................................................  0
```

⚠⚠⚠ **THE MAP IS A LIST OF FEATURES AND THE WEB IS A LIST OF COMBINATIONS.** Tick 1119's defect —
an out-of-flow child of a flex container emitted TWICE, reported as one box 499,432px wide — is not
a missing feature. Both features are present, gated, and correct on their own; the *interaction*
between them had never been named, so no instrument that reads this map could rank it, and the two
ticks before it (1111, 1112) searched inside the wrong frame because the frame had no cell for it.

This is the same failure mode as audit #51 in a new direction. #51 caught three capabilities we had
BUILT and never claimed. This one catches a capability nobody could have claimed or disclaimed,
because the map has no way to express *"A inside B"*. Every row is a noun.

**The cheap test, and it should become part of this audit's procedure:** take the two highest-mass
rows the loop is actively working (`CSS Flexbox layout`, and this window the abspos family) and grep
for rows matching both. A zero is not proof of a defect — but it is proof that if there is one, the
map cannot see it. Three of the last four layout ticks have been at such an intersection
(`transform` × table cell, `direction:rtl` × grid axis, `position:absolute` × flex).

### Added (5 rows)

```text
   gated    an out-of-flow child of a FLEX container takes no part in flex layout, and its insets
            resolve against the container's PADDING box (Flexbox §4.1)
   partial  the STATIC POSITION of such a child is "as if it were the sole item" — the container's
            alignment decides it (partial: a container at a PROVISIONAL origin records inner space)
   missing  an out-of-flow child with DEFINITE GRID PLACEMENT is positioned against its GRID AREA
            (Grid §9) — the named reason tick 1119's fix stops at flex
   works    `LayoutBox::node_rects` reports the UNION of an element's boxes, so a DOUBLE-EMITTED
            element reports geometry no code ever computed
   missing  `pre_transform_rect` is a FIRST-WRITE-WINS cache that an intrinsic MEASURING pass reaches
```

MEASURED 523 of 558; `unknown` still 36 (no row was resolved *away* this audit — two were added
already-`missing`, which is the honest direction).

### What we had been wrong about

**The `instrument` class had ONE row, ten ticks after audit #49 named its absence.** #49's finding
was *"545 rows about the ENGINE and none about the INSTRUMENTS"*; one row was added and the class
stopped growing, while the loop went on to spend t1112 building `MANUK_HOVF_TRACE` and t1119
discovering that `node_rects` unions duplicates — both facts about instruments, neither on the map.
Two are added here. **A class with one row is not a covered class; it is a closed ticket.**

**And the taxonomy itself has drifted:** `layout` holds 3 rows while `css` (125) and `doc` (104) carry
the layout content, so `class` no longer partitions anything and a per-class count says nothing about
coverage. Recorded, not fixed — renaming 230 rows is its own tick and would rewrite history that the
receipts still refer to.

## Audit #53 — tick 1129 (2026-08-11) — the map's grammar is NOUNS, and two of this arc's three defect classes are VERBS

**Sources, read this session, not from memory:**

- The map itself, queried rather than recalled: `awk -F'\t'` over `docs/loop/CONSTELLATION.tsv` for
  the three classes ticks 1119–1128 actually worked
- `CONSTITUTION.MD` PART VI as amended by checks #105 and #106 — both added clauses this window, and
  neither had a map row to point at
- The t1127 sweep's own root-cause output and the `MANUK_RO_PARTITION` runs behind check #106's steer

### The finding: audit #52 found the map cannot say "A inside B"; it also cannot say "during pass P"

#52 caught that every row is a noun, so an INTERACTION between two present features (abspos × flex)
had no cell. This window produced two more classes, and neither is an interaction — they are **phases
of the layout algorithm**:

```text
   rows matching  provisional|inner layout|re-origin  .............  0
   rows matching  intrinsic  ...............................  8, and every one is a FEATURE
                  (contain-intrinsic-size, replaced intrinsic sizing, <select>'s arrow, …)
   the one exception, added by check #105 four ticks ago:
     instrument   `pre_transform_rect` is a FIRST-WRITE-WINS cache an intrinsic MEASURING pass can reach
```

⚠⚠⚠ **THE TWO HIGHEST-YIELD FIXES OF THE LAST TEN TICKS WERE BOTH VERBS.** t1124 —
*"a box laid out at a PROVISIONAL origin must re-origin all THREE of its outputs"* — bought two M1
crossings, and the class had **zero** rows. t1120 — *"an intrinsic measurement must not write to
anything the real layout reads"* — was six ticks of one class, and the only row it had was the one an
audit wrote after the fact. A map that can only name features ranks the next tick against a frame in
which the last two winners were invisible.

**The cheap generalisation, and it is testable:** the map's `class` column has 18 values and every one
is a SUBSYSTEM (`css`, `layout`, `dom`, `render`, …). Nothing in the schema forbids a row whose
subject is a pass, a cache, or an invariant of the pipeline — #52's `instrument` rows proved that by
existing. The gap is habit, not structure.

### Added (3 rows)

```text
   gated    a box laid out at a PROVISIONAL origin must re-origin ALL THREE of its outputs —
            boxes, fragments, and static positions          (t1124, two M1 crossings)
   partial  an INTRINSIC MEASUREMENT is a throwaway pass and must not write to anything the
            real layout reads                                (t1120; 3 of 5 side-tables unaudited)
   works    a `--jobs 2` sweep row is bankable for the DENOMINATOR and is NOT evidence about
            any single site                                  (check #106; five refuted readings)
```

MEASURED 525 of 561; `unknown` still 36 — none resolved this audit, and the `partial` row is stated
as partial rather than claimed, because three of the five writable side-tables have not been audited
against its own question.

### What we had been wrong about

**`text shaping (swash)` is one row, `gated`, and its `what_breaks_without_it` is "all text".** The
current work-list's dominant root cause on `www.jatekshop.eu` is `mis-sized: width ~8px (<a>)` at 50
hits with a median of 10px — link text ~10px too narrow, fifty times on one page. That is not a
refutation of the row (the shaper runs, and one site is not the corpus), but it is the map at its
least useful: a one-line `gated` claim over the single largest surface in the engine, with no
sub-rows for advance accuracy, letter/word-spacing, or the `font-feature-settings` interaction the
map ALREADY carries as a separate row saying *"shaping changes advances, which changes every width
downstream."* **Two rows contradict each other in tone and neither is wrong; the honest fix is to
decompose the shaping row**, which is its own tick and is filed here rather than done in an audit.

⚠ **NOT claimed, and the discipline is the point:** I counted mis-sized-width causes in the t1127
sweep log and got 13 lines, then checked what that log actually contains — root causes are printed
per-site for a handful of sites, not corpus-wide. **A count off an instrument's log is a count of the
LOG.** The corpus-wide frequency of this class is unmeasured, so the row above is not added.

## Audit #54 — tick 1139 (2026-08-11) — the map HAD both of this window's defects: one unread for 378 ticks, one marked `gated` and WRONG

**Sources, queried this session, not recalled:**

- `docs/loop/CONSTELLATION.tsv`, queried by `awk -F'\t'` for the two subjects ticks 1137–1138 actually
  fixed, plus a term census (`line-height` 8 rows · `line box` 13 · `rounding` 3 · `strut` 1 ·
  `half-leading` 2 · `content area` 1 · `font metric` 1 · **`forced break` 0**)
- `engine/page/tests/g_line_box_rounding.rs` read in full — the gate the map cites, not the claim
- The t1137 / t1138 batteries (256 rows against live Chrome) and their same-hour pass-SET diffs

### The finding, and it is the INVERSE of audit #53's

#53 found the map's grammar could not NAME two of that window's defect classes. **This window it
could name both — and naming them changed nothing, because one row was never read and the other was
a false green.**

```text
  row 421  doc   `<br>` line-box height          partial   gate: -
           "a <div><br></div> is 18px in Chrome and 19px here — 1px per <br>,
            which accumulates down a document that uses them"      ← TICK 759

  row 301  text  line-box height rounding (`line-height: normal`)  gated  G_LINE_BOX_ROUNDING
           receipt: "the rule is round(ascent+descent+lineGap), verified on THREE faces
            because round-each-term agrees on two and is wrong on Liberation Sans"  ← TICK 581
```

⚠⚠⚠ **ROW 421 CARRIED t1137's DEFECT, WITH THE EXACT NUMBER, FOR 378 TICKS.** *"18px in Chrome and
19px here"* is precisely what the t1137 battery re-measured from scratch. `partial`, gate `-`, and
nobody read it. The map was **right, specific, and inert.** It also **understated** the defect by a
factor of thirty: the cause is that a `<br>` was an inline BOX on the line it ends, so a `<br>`
carrying `line-height:40px` inflated its line by 22px and `font-size:40px` by 30px — a 1px row
describing a 30px rule.

⚠⚠⚠ **ROW 301 SAID `gated` AND THE GATE PINNED THE WRONG RULE.** The receipt's own reasoning is the
defect: *"round-each-term … is wrong on Liberation Sans"* is true **only if the gap term is
dropped** — `14 + 3 = 17`, but `14 + 3 + round(0.523) = 18`, which is Chrome's answer. All three
faces were measured at **16px**, the one size where the two rules cannot be told apart, and
`G_LINE_BOX_ROUNDING`'s whole fixture is `font: 16px sans-serif` with the missing-gap counter-example
written into its RED-proof list. **A `gated` row was a FALSE GREEN for 557 ticks, and the gate was
what kept it green.**

### The two consequences, both cheap and both testable

**1. GREP THE MAP BEFORE A CAPABILITY TICK.** Three minutes, no build — the discipline check #105 §4
already imposes on the CORPUS, applied to the MAP. On this window it would have found row 421 and
saved 378 ticks of not-knowing. The population it would surface is **49 `partial` rows, 8 of them
with no gate at all** — a work-list the loop already owns and has never read as one.

**2. `gated` IS NOT A STATUS, IT IS A CLAIM ABOUT WHAT THE GATE VARIED.** 322 of 562 rows are
`gated`, and each is only as true as the parameter its gate held fixed. `G_LINE_BOX_ROUNDING` varied
the FACE (Liberation / DejaVu / Noto) and fixed the SIZE; the defect lived in the size. t1138's
replacement gate carries an explicit `separated >= 10` assertion — *at least ten of its rows must
actively distinguish the two rounding rules* — so narrowing it back toward the agreeing sizes FAILS
rather than silently passing. **That assertion is the pattern this column needs**, and it is filed
here as a proposal rather than applied to 322 rows in an audit.

### Edited (2 rows, no new rows)

```text
   421  partial → gated (`a_br_does_not_grow_the_line_it_ends`); receipt records the 378-tick
        latency and corrects the magnitude from 1px to 30px
   301  receipt CORRECTED — the banked t581 rule was wrong and its own justification says why;
        the new rule and the anti-re-fitting assertion recorded
```

MEASURED 525 of 561; `unknown` still 36 — none resolved this audit. **No row was added**, which is
itself the finding: nothing this window was missing from the map.

### What we had been wrong about

Every audit since #31 has treated a `gated` row as settled and spent its attention on `unknown` and
`missing`. **The two defects this window fixed were both inside the `gated`/`partial` population**,
and audit #53's own filed debt — *"decompose the shaping row"* — is the same shape a third time: one
`gated` line over the largest surface in the engine. The map's failure mode has moved from *"the
frame is missing a cell"* to *"the cell exists and says something untrue with confidence."*

## Audit #55 — tick 1150 (2026-08-11) — the world's own top-20 list finds NO hole in the map, and one row overclaims on an item that IS on it

### Sources (read this audit, not recalled)

- `github.com/web-platform-tests/interop` → `2026/README.md` — the authoritative Interop 2026 list:
  **20 focus areas + 4 investigation efforts.**
- webkit.org/blog/17818, web.dev/blog/interop-2026, hacks.mozilla.org (Launching Interop 2026),
  igalia.com/news/interop-2026 — the four vendors' own framing.
- ladybird.org newsletters, Jan–Jul 2026 — an independent engine's curve at this stage.

### The reconciliation: 20 of 20, all with explicit verdicts

Every Interop 2026 focus area already has a row, and none is `unknown`:

```text
  gated    IndexedDB · Navigation API · scroll snap · view transitions · WebVTT · dialogs+popovers
  partial  CSS attr() · CSS zoom · media pseudo-classes · fetch uploads+ranges ·
           cross-document view transitions
  missing  anchor positioning · custom highlights · JSPI · scoped custom element registries ·
           scroll-driven animations · CSS shape() · WebRTC · WebTransport · JPEG XL ·
           container STYLE queries
```

**No row was added, and that is a real result rather than a shrug**: the loop's frame is not behind
the world's own priority list, which is the specific failure this instrument exists to catch. It
also re-confirms the standing scope calls — WebRTC, WebTransport/HTTP-3, JSPI and JPEG XL are all
Interop 2026 focus areas and all four are *deliberately* on `docs/loop/DECISIONS.md`'s cut line, so
their `missing` is a decision, not a blind spot.

### ⚠⚠⚠ What we had been wrong about — and it is audit #54's finding, one row over

`container queries (incl. style queries)` was marked **`gated`**, and its backing gate was
`G_PROBE_CAPABILITIES` — a **presence probe**. Its own receipt contradicts its title:

```text
   RESIDUE: style()/scroll-state() queries follow size machinery; …
```

and a second row, `container STYLE queries (@container style(--x: y))`, says `missing`. Confirmed
against source: `@container` **size** conditions are real (Stylo's `ContainerCondition::matches`
driven through our `query_container_size`, `stylo_engine.rs:637`); there is **no `style()` support
anywhere in `engine/`**. So the map carried two rows for one capability with opposite verdicts, and
the one a reader hits first said `gated`. **Container style queries is focus area #1 of Interop
2026** — the single item most likely to be looked up in this map this year.

Row 98 retitled to `container SIZE queries (@container (min-width:...)) — style() NOT included, see
its own row`. Status and receipt unchanged: they were already correct about what was built.

### The debt this names, measured rather than asserted

**Twenty-three rows cite `G_PROBE_CAPABILITIES` as their gate, and eleven of those are marked
`gated` on it alone** — `WebAssembly`, `WebAssembly GC`, `Temporal`, `Intl`, `CJK line breaking`,
`field-sizing`, `print / media queries`, `name-only container queries`, and the row corrected above.
`G_PROBE_CAPABILITIES` answers *"is the symbol present / does it parse"*, not *"does it behave"*, so
`gated` on it alone is a **stronger verdict than its receipt supports** — the same class audit #54
named (*"`gated` is not a status, it is a claim about what the gate VARIED"*), now with a count.
Filed as a work-list, not fixed in an audit: re-verdicting eleven rows is eleven probes.

### The external calibration, and it is the one worth carrying

Ladybird's WPT curve, month by month in 2026: **+63,726 (April) → +8,283 (May) → +3,366 (June) →
+108 (July)** — and the April figure is mostly an *import* (test262 landed upstream, ~52k of the
63.7k). An independent engine of comparable maturity has a subtest curve that has gone
**essentially flat inside four months**. That is the strongest outside evidence yet for the pivot
this loop already made: **a WPT subtest count stops being a progress signal at exactly this stage**,
which is why the board ranks on corpus fidelity and not on flips. Recorded because it is the kind of
number that would otherwise be argued about from memory.

MEASURED 525 of 561; `unknown` still 36 — none resolved. Rows added: 0. Rows corrected: 1.

## Audit #56 — tick 1161 (2026-08-11) — the ROWS were right and their MEASUREMENTS were 26 days old, and an independent engine hit the same plateau in the same month

### Sources (read this audit, not recalled)

- `github.com/web-platform-tests/interop` → `2026/README.md` — re-read: **20 focus areas + 4
  investigation efforts**, unchanged since audit #55 read it eleven ticks ago.
- ladybird.org newsletters, Jan–Jul 2026 — an independent engine's curve, and the one external
  series that is directly comparable to ours.
- webkit.org/blog/17818, web.dev/blog/interop-2026, igalia.com/news/interop-2026.

### The reconciliation found NOTHING NEW, and that is the correct result twice running

All 20 focus areas still have rows; all still carry explicit verdicts; nothing was added. Audit #55
established this and #56 confirms it did not rot in eleven ticks. **The frame is not behind the
world's list.** Repeating that check was cheap; what it freed the audit to look at is the layer
below, which is where this one found its finding.

### ⚠⚠⚠ What we had been wrong about — a `gated` row whose MEASUREMENT is a month old is also an unmeasured claim

Audit #54 established *"a `partial` row with no gate is an UNMEASURED CLAIM."* This audit found the
next layer of the same defect, and it was live: **`docs/loop/WPT-AREAS.tsv` — the source the PRIMARY
per-tick progress metric is computed from — had not been re-run since Jul 16.** Twenty-six days,
roughly a hundred ticks, while the loop steered by the total it produces. Refreshing it costs
**~15 seconds per area**.

Every row had moved, and one came back carrying a **Bar 0 crash** (`css/selectors`, `HANG/CRASH 1` —
`has-complexity.html`, the `:has()` quadratic; see tick 1161). The map said the right things about
the world and the numbers under it were a month stale.

> **A frozen metric is not a slow metric. It is a metric that cannot report a crash.**

### ⚠⚠ And a percentage can be FLAT while the engine got 3.7× better — the suite grows underneath us

```text
                        STALE (Jul 16)        FRESH (Aug 11)       passes    pct
   css/css-grid          150/2841   5.3%       558/9281   6.0%      x3.7    +0.7pt
   css/css-sizing        191/1588  12.0%       576/2084  27.6%      x3.0   +15.6pt
   css/css-flexbox       223/3594   6.2%      1329/3660  36.3%      x6.0   +30.1pt
```

`css-grid`'s denominator **tripled** (2841 → 9281) while its passes nearly quadrupled, so the
percentage moved 0.7 points. **A ratio against a moving denominator is not a progress metric**, and
anyone reading the `pct` column would have concluded grid was stuck. Read `pass` as the monotone
series and `pct` only as a coverage share.

### ⚠⚠ The external calibration, which is new and which the board should have

Ladybird — independent, non-Chromium, and as of 2026 running **its style and layout pipeline in
Rust** (selector matching first, then computed style, cascade, animations, `calc()`, layout-tree
construction, and every formatting context from block through SVG):

```text
   passing WPT subtests   Apr 2,067,263 → May 2,075,546 → Jun 2,078,912 → Jul 2,079,020
   monthly gain                          +8,283          +3,366          +108
   ours (tick 1161)       430,742 of 1,225,493
```

Two things follow, and both are steering-relevant:

1. **The plateau is not local.** An independent engine, further along and on the same kind of stack,
   saw its monthly gain collapse from 8,283 to 108 in three months. The board's read that layout
   reftests are byte-exact CONJUNCTIONS with low flip is corroborated from outside this repo — our
   flat headline is the shape of the terrain, not evidence the loop stopped working.
2. **Their ORDER is a ranked external path** for the board's *"PORT, don't reverse-engineer"* steer:
   selector matching → computed style → cascade → animations → `calc()` → layout-tree construction →
   formatting contexts. That is an engine that walked this road choosing to do the matcher first.

### The steer — binding on the next ticks

1. ⚠⚠⚠ **CLOSE THE `:has()` BAR 0's SECOND HALF.** `Page::relayout` recascades whenever the node
   count grows (`engine/page/src/lib.rs:6167`), so 75,000 `appendChild` calls are 75,000 full
   cascades. Tick 1161 made each cascade linear; **incremental style invalidation** is what makes
   there be fewer of them, and the WPT test still crashes until it exists.
2. **Re-run `WPT-AREAS.tsv` on a cadence, not on an inspiration** — it is 15s an area and it is the
   primary metric's source. `scripts/` is observer-owned, so this is filed as a request: wire the
   refresh into the wall, or into the sweep, so a Bar 0 cannot hide in a stale row for a month again.
3. **`domparsing` fell 188 → 149 on an unchanged denominator and cannot be attributed** — the old
   binary no longer exists, so no same-hour control is possible. Open question, deliberately not
   explained away.
4. **The total is 92% `encoding` by test count** (1,127,434 of 1,225,493). A +1,300-subtest gain
   across the whole CSS surface moves the headline ~0.1pt. Good monotonicity check, poor sensitivity
   one — do not read a flat total as a flat engine.

## Audit #57 — tick 1171 (2026-08-12) — HALF of Interop 2026 is on our own DEATH-TAIL, and my reconciliation script lied twice

**SOURCES (fetched, not recalled):**
- https://web.dev/blog/interop-2026 — the authoritative list of the 20 focus areas + 4 investigations
- https://webkit.org/blog/17818/announcing-interop-2026/ , https://hacks.mozilla.org/2026/02/launching-interop-2026/ , https://www.igalia.com/news/interop-2026.html — the same list from the other three participants

### What was ADDED to the map: NOTHING, and that is the second consecutive clean result

Every one of Interop 2026's twenty focus areas and four investigations already has a row in
`docs/loop/CONSTELLATION.tsv` (562 rows). Audit #55 found the same for the 2025 list. **The map is not
missing capabilities the world names** — it has now survived that test twice against a list assembled
independently by Apple, Google, Igalia, Microsoft and Mozilla.

### What we had been WRONG about — my own reconciliation, twice, in the same pass

⚠⚠⚠ **THE FIRST RUN OF THIS AUDIT PRODUCED TWO FALSE `missing` VERDICTS, AND THEY WERE MINE.** I
matched each Interop area against the constellation with a regex over BOTH the `capability` and the
`what_breaks_without_it` columns, then reported the weakest-status hit. Two areas were mis-attributed:

```text
   Interop area          my first pass     the truth
   dialogs and popovers   missing          GATED  (G_DIALOG + G_POPOVER, plus G_DIALOG_RENDER,
                                                   G_DIALOG_REQUEST_CLOSE, G_TOGGLE_EVENT_SOURCE)
   view transitions       missing          GATED  (G_VIEW_TRANSITION); the cross-document form
                                                   Interop 2026 expands to is `partial`
```

Both were matched by the PROSE of an unrelated row (the anchor-positioning row's
`what_breaks_without_it` mentions popovers). **A reconciliation that reads the description column is
matching a sentence, not a capability** — and it produced exactly the kind of confident false
negative this instrument exists to catch, one level up from where it was looking. Corrected by
matching the `capability` column only, and the corrected table is below.

### The corrected reconciliation, and the finding

```text
   gated / works (7)   dialogs+popovers · contrast-color() · IndexedDB getAllRecords ·
                       Navigation API · scroll snap · WebVTT · View Transitions (base)
   partial (4)         attr() · fetch uploads+ranges · media pseudo-classes · zoom
                       (+ cross-document view transitions)
   missing (10)        anchor positioning · container style queries · scroll-driven animations ·
                       custom highlights · JSPI · scoped custom element registries · WebRTC ·
                       WebTransport · JPEG XL · shape() · view-transition pseudo-classes
```

⚠⚠⚠ **EIGHT OF THOSE TEN ARE ON THIS PROJECT'S OWN EXPLICIT DEATH-TAIL / SKIP-v1 LIST** — anchor
positioning, scroll-driven animations, custom highlights, JSPI, scoped registries, JPEG XL, WebRTC
and WebTransport-over-HTTP/3 are all named there by the tick-543 orders. **So the world's top-20 for
2026 and our v1 scope disagree on half the list, and the disagreement is DELIBERATE and already
written down.** That is not a defect — our exit is *"runs almost every website"*, not *"wins
Interop"* — but it had never been stated as a ratio, and a 10-of-20 divergence is large enough that
it should be a decision the owner re-affirms rather than a fact that accumulates quietly. **Recorded
here as a standing disclosure, not as a steer to go build them.**

⚠ **THE TWO THAT ARE *NOT* ON THE DEATH-TAIL are the only candidates this audit surfaces:**
**container style queries** (`@container style(--x: y)`) and **`shape()`** — both `missing`, both CSS,
neither deferred anywhere. They are named for pricing (corpus frequency first, per the standing rule),
not scheduled.

### Re-rank?

**No.** Nothing discovered here outranks the current CO-#1. The binding constraint remains M1 at
**18.8%** against a 95% bar (measured this session, t1170), and the refreshed WPT board puts
`css/css-grid` at **8,723 failing** — the largest CSS surface and the M1 body. Interop 2026's list
does not touch that ranking: its CSS items are either already gated or on the death-tail.

---

## AUDIT #45 — tick 1182 (2026-08-12): the map is COMPLETE against the world's list, and my reconciliation instrument was not

**Sources read (live, not from memory):**

- `github.com/web-platform-tests/interop/blob/main/2026/README.md` — the canonical 2026 list
- `webkit.org/blog/17818/announcing-interop-2026/` — the 20 focus areas with descriptions
- `web.dev/baseline/2026` + the 2026 monthly Baseline digests — Newly/Widely Available this year

**Reconciled: 31 externally-named priorities** — 20 Interop 2026 focus areas, 4 investigation areas,
and 7 Baseline-2026 items (`:active-view-transition`, Trusted Types, Navigation API, `lh`, `rlh`,
`contain-intrinsic-size`, view transitions).

### The result, and it is the opposite of what this instrument was built expecting

⚠⚠⚠ **THIRTY OF THIRTY-ONE ALREADY HAVE A CAPABILITY ROW.** Container style queries (row 370),
`shape()`, anchor positioning (99/502), scroll-driven animations (92), scoped custom element
registries (96), JSPI (97), custom highlights (103), WebTransport (106), `getAllRecords()` (178),
`contrast-color()` (192), `:open` (182), `attr()` (100), CSS `zoom` (101), **fetch uploads + ranges
(105)**, WebVTT (79), Trusted Types (257), `lh`/`rlh` (206), `:active-view-transition` (199) — all
present, each with a status and, where claimed, a named gate.

This file's own standing prior is *"an audit that finds nothing is a suspicious audit; six phantoms
say the map is never clean."* That prior was written in the six-phantoms era and **it no longer
describes this map.** Audits #31/#34/#42 added most of these rows; the map has caught up with the
world's list. Recorded so the next audit does not manufacture a finding to satisfy the prior.

### The ONE addition

**`<dialog closedby>` + `popover="hint"`** — the Interop 2026 *Dialog and Popover Additions*. Added
as `unknown`. Rows 90/208/222/334 cover `<dialog>`+popover, `ToggleEvent.source`, `CloseWatcher` and
top-layer rendering, so the **feature** is mapped and **this year's additions to it** are not.

> ⚠⚠ **THAT IS THE SHAPE OF THE ONLY MISS, AND IT GENERALISES: our map tracks FEATURES; Interop
> tracks the year's ADDITIONS to features.** A row can be `gated`, accurate, and three years old at
> the sub-feature level while the audit's own reconciliation — which matches on capability NAMES —
> reports it covered. The next audit should reconcile at the *sub-feature* grain for every row whose
> feature appears on an Interop list, not at the row grain.

### ⚠⚠⚠ What I was wrong about: the RECONCILIATION INSTRUMENT, three times in one audit

I filed four candidate gaps. **Three evaporated on inspection**, each from a different defect in how
the reconciliation was run:

| candidate | why it looked missing | what it actually was |
|---|---|---|
| fetch uploads + ranges | grepped for `"Range header"`, the spec's wording | row 105, worded `fetch uploads + ranges (streaming)`, `partial` |
| container style queries | `awk $2 ~ /container style quer/` | row 370, worded `container STYLE queries` — **case** |
| Trusted Types | same awk, `IGNORECASE` silently inert under this `awk` | row 257, `missing`, since audit #31 |

**A reconciliation that matches on my phrasing of a capability measures my vocabulary, not the map.**
The standing rule *"match the `capability` column, never the prose column"* is correct and is only
half of it: grepping the whole line **over**-reports coverage (a prose mention reads as a row), and
grepping column 2 with a brittle matcher **under**-reports it. Both failed here, in opposite
directions, in the same twenty minutes — which is why every candidate gap in this audit was opened
and read before it was filed, and why 3 of 4 were withdrawn.

### Re-rank?

**No — but for a different reason than last time.** Nothing external outranks the current CO-#1. The
re-rank that IS due is *internal*: this session landed **+3,121 WPT subtests** (t1179 +406,
t1180 +1, t1181 +2,714) and `WPT-AREAS.tsv` predates all of it, so the lever board is ranking off
numbers that are three thousand subtests stale — `css/css-values` is listed at 20.9% and measured
**40.4%**. The full sweep is running in this same tick. **A moving denominator is the tell**
(t1163), and the board must not pick the next lever until it is refreshed.
