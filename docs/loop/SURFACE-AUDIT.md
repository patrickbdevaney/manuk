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
