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
