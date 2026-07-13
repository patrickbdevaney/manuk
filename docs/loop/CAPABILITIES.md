# CAPABILITIES — what the web actually uses, and what we actually support

> **This file is the ONLY axis on which Chromium is the target.**
>
> Chromium is the **ceiling on capability** and the **floor on everything else**. Whatever a page can do
> in Chrome it must be able to do here — the scripts run, the layout resolves, the forms submit, the
> embeds render. That is what this file tracks, and on this axis we are behind and are catching up.
>
> On **speed, stability, resource use and honesty of failure**, Chromium is the baseline to *beat*, not
> the number to converge on. Being faster is not a divergence. (See THE NORTH STAR in `STATUS.md`.)
>
> **The trap:** a speed win is only real if it comes from doing the same work *better* and not from **not
> doing the work**. Every ❌ in this file is a place where "we're fast" might just mean "we skipped it".

**This file is measured, not imagined.** Two numbers per row, and neither is a judgement:

- **Usage** — how many of **237 real site snapshots** (the oracle corpus: news, ecommerce, social, docs,
  SaaS, gov, media, tools) contain the feature. Counted as **distinct sites**, never as hits — one site
  with 500 `<div>`s must not outvote 200 sites with one `<iframe>`.
- **Support** — what our engine *answers when asked*, from a feature-detection probe run through the
  real pipeline. Not what the code looks like it does.

The priority order is what falls out of putting those two columns next to each other. It is not a
roadmap someone wrote; it is a subtraction.

> **A capability that THROWS is worse than one that is missing.** A missing feature degrades a page. A
> thrown `TypeError` at the top of a bundle kills every line of script after it — so a 3%-of-the-web
> feature that throws (canvas) outranks a 34%-of-the-web feature that degrades (srcset).
>
> **USAGE IS NOT DAMAGE.** They are different columns and only one of them is a priority. `srcset` is
> used by 34% of sites and *breaks* ~1% of them, because `src` is a fallback and it works. `<canvas>` is
> used by 3% and *broke* all 3%, because it threw. **Rank by what happens to the user, not by what
> appears in the markup** — otherwise you build the popular thing instead of the load-bearing one.

## ⚠️ How this file was nearly a lie, on its first day

The first version of this table opened with:

> **1. `localStorage` — 27% of the web — ❌ THROWS. Not a gap, an outage.**

**It was false.** A real, persisted, per-origin `localStorage` has existed for ages (`manuk_net::webstorage`,
behind the `__storage` native seam). It threw in my probe because **I ran the probe from a `file://` URL**
— an opaque origin, which gets no storage *in every browser*, and correctly answers with a
`QuotaExceededError`.

I had already written a replacement shim. Had I not stopped to ask *why* it threw, I would have shipped a
worse duplicate of a working feature and reported a 27%-of-the-web win that did not exist.

> **The instrument is part of the experiment.** A capability audit run from the wrong origin reports the
> browser as broken. **Support numbers must be measured from a real origin, over real HTTP** — which is
> what the probe now does, and why `docs/loop/capability-probe.html` is served rather than opened.

## The blockers, ranked by (sites that use it) × (how badly we fail)

Measured from a real HTTP origin. Every "we do" below is what the engine *answered when asked*.

| # | Capability | Usage | We do | Consequence |
|---|---|---|---|---|
| ✅ | ~~`<form>` submit~~ | 50% | ✅ **DONE (tick 34)** | The `submit` event is dispatched and `preventDefault()` honoured, so an AJAX form no longer performs the navigation its author cancelled. `method=POST` still open. |
| — | ~~`<picture>` / `srcset`~~ | 34% used | ❌ unparsed | **DOWNGRADED — measured.** Usage 34%, **damage ~1%**. Only **2 of 237** sites have an `<img srcset>` with no `src`; everyone else ships a working `src` fallback, which is what `src` is *for*. We load a possibly-wrong-*resolution* image, not a missing one. Worth doing; not worth doing next. |
| **1** | **`<iframe>`** | **23%** | ❌ 0-height box, no `contentWindow` | Embeds, maps, video players, payment frames, comment widgets. A nested browsing context is real work and it is unavoidable. |
| ✅ | ~~CSS animation reveal~~ | **21% pair `opacity:0` with an animation** | ✅ **DONE (tick 36)** | A fifth of the web had **invisible content**: the base rule says `opacity:0`, the keyframes reveal it, and we rendered the first frame literally. An animated element now renders its **end state**. Deliberately-hidden elements stay hidden. |
| ✅ | ~~`position: sticky`~~ | 14% | ✅ **ALREADY WORKED** | The ledger said "does not stick". **It was never tested.** Verified: a sticky header pins to the viewport top at scroll 500. Third untested assumption in three ticks — see PROCESS #20. |
| 2 | CSS transition **motion** | 13% (transition-only) | ⚠️ end state renders, no tween | Hover/transition effects jump instead of animating. Legible, and low damage: the end state IS the content. |
| 4 | **WebSocket** | 5% | ❌ missing | Live feeds, chat, collaboration. **Social platforms live here.** |
| 5 | **Service Worker** | 5% | ❌ missing | Offline, PWA install, push. Usually feature-guarded. |
| 6 | **`<dialog>` / `showModal`** | 3% | ❌ missing | Modals. Growing fast; will not stay at 3%. |
| 7 | **Web Worker** | 2% | ❌ missing | Heavy apps move work off the main thread. |
| 8 | **IndexedDB** | 1% | ❌ missing | Offline-first apps. Low usage, high cost. |

**Fixed this tick** (they were genuinely throwing, and a throw takes the page with it):

| Capability | Usage | Was | Now |
|---|---|---|---|
| `canvas.getContext('2d')` | 3% | ❌ **THREW** — `ctx.fillRect` was a `TypeError` and a charting library took the whole bundle down | ✅ a real context, drawing ops are no-ops, `measureText` returns a real shape. **A blank chart on a working page.** `getContext('webgl')` → `null`, the spec's "cannot". |
| `Notification` | 14% | ❌ missing | ✅ honest: `permission === 'denied'`, `requestPermission()` resolves `'denied'`. The site asked and was told no. |

## What we already have (and the corpus confirms we need)

| Capability | Usage | Status |
|---|---|---|
| inline `<svg>` | **72%** | ✅ renders |
| CSS custom properties | **53%** | ✅ (Stylo) |
| CSS transform | 45% | ✅ mapped, moves boxes |
| `@media` | 42% | ✅ `matchMedia` + cascade |
| CSS flex | 41% | ✅ Taffy |
| CSS transition / `@keyframes` | 38% | ⚠️ parsed; **not animated** — static end-state |
| `<script type=module>` | **31%** | ✅ + deferred by default (tick 32) |
| CSS grid | 28% | ✅ Taffy |
| custom elements / shadow DOM | 19% | ✅ upgrade, shadow root, styles (ticks 26, 25) |
| `@font-face` | 19% | ✅ |
| `matchMedia` | 17% | ✅ |
| `<video>` | 16% | ✅ **degrades honestly** — poster + an honest "cannot play" (tick 28) |
| CSS `:has()` | 15% | ✅ (Stylo) |
| `<template>` | 14% | ✅ (tick 26 — Lit needed it) |
| `position: sticky` | 14% | ⚠️ box laid out; **does not stick on scroll** |
| MutationObserver | 13% | ✅ |
| `@supports` | 13% | ✅ (Stylo) |
| `@container` | 11% | ✅ (Stylo) |
| `fetch()` / XHR | 12% / 10% | ✅ |
| IntersectionObserver | 9% | ✅ — **and this is why images lazy-load at all** |
| ResizeObserver | 4% | ✅ |
| `history.pushState` | 5% | ✅ |

## The shape of the remaining work

Read down the two tables and the shape is clear, and it is **not** "make the pixels match Chrome":

1. ~~**Make the browser writable.**~~ ✅ **Done, tick 34.** The `submit` event fires and is cancellable.
2. **Nested browsing contexts (`<iframe>`).** 23% of sites, and we render a **0-height box with nothing
   in it** — the damage number and the usage number are the same here, which is what makes it the real #2.
3. **Motion.** `transition`/`@keyframes` is 38% and `position:sticky` is 14%.
4. **Responsive images.** `<picture>`/`srcset`, once the above are done. Real, but a *quality* bug, not
   an *outage* — the fallback `src` is doing its job on 233 of 237 sites.
5. **Then** the long tail: WebSocket, Workers, Service Workers, IndexedDB — the substrate of social
   platforms and offline apps, and the right thing to do *after* the above, not before.

## How this file is maintained

Re-run the two probes and paste the numbers. Both are cheap and both are in the repo:

- **Usage:** scan the cached corpus snapshots (`/tmp/manuk-oracle-snapshots`) for each feature, counting
  distinct sites.
- **Support:** run `docs/loop/capability-probe.html` through the engine and read what it answers.

**Never edit a number by hand.** The entire value of this file is that neither column is anybody's
opinion — the moment one of them becomes a claim rather than a measurement, this is just a roadmap with
extra steps, and roadmaps are how a project ends up pixel-tuning one site while a quarter of the web
throws on line one.
