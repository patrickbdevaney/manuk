# THE IN-BROWSER DEMO — plan, and the feasibility proof (tick 50)

**Purpose, stated plainly so downstream scope decisions can be checked against it:** a visitor opens a URL
and gets, **inside their own browser**, a live running instance of our **actual** rendering/layout/paint
pipeline — not a screenshot, not a video, not a description. They scroll, click, and hover on real curated
pages and watch our engine do the work, in real time. **This is the proof-of-realness the whole effort is
for; do not let scope-narrowing erode that core experience.**

## FEASIBILITY: PROVEN (tick 50)

The load-bearing question was *"does the engine even compile to `wasm32-unknown-unknown`?"* — probed first,
before any scaffolding, exactly as the WPT runner probed `testharness.js`. **Answer: yes, the entire render
pipeline minus JS.** `./scripts/wasm-check.sh` proves it repeatably:

```
manuk-dom ✓  manuk-html ✓  manuk-text ✓  manuk-layout ✓  manuk-paint ✓  manuk-css+stylo ✓
```

**The one real blocker, found and fixed:** `NodeId` packed `generation << 32 | index` into a **`usize`**,
and `wasm32`'s `usize` is **32 bits**, so the shift overflowed and `manuk-dom` **did not compile**. The fix
is a **`u64`** backing — identical to `usize` on 64-bit, correct on 32-bit — so the arena is now
pointer-width-independent (`G_ARENA_U64` pins it). *This also hardens the ARM/cross-platform target.* Native
is unregressed (full wall green).

## What is real, and what is not — both stated IN-PRODUCT, not just here

- **Real:** Stylo (the actual cascade), Taffy (the actual flex/grid), tiny-skia (the actual rasterizer),
  and this engine's own DOM/layout/paint — compiled to wasm, **genuinely executing in the visitor's
  browser.** Scroll, click, hover, and CSS-driven state (`:checked`, `:hover`, transitions/animations) are
  fully live.
- **Not real, stated honestly:** **no JS execution** (SpiderMonkey is C++, not in the wasm build — which is
  also *why* the demo is JS-free), and **no live/arbitrary URL fetching** — a **curated, bundled set of real
  page snapshots**, re-derived from the oracle's live cluster registry at build time so the set stays
  representative of what is actually covered, **not hand-picked to look good.**

## The checkable feature: side-by-side vs Chromium

Per curated page, a toggle / side-by-side showing **our live render against the reference Chromium render**
of the same page. **A visitor should not have to trust a claim — they look at both and see for themselves.**

## Threading

**Deliberately single-threaded** — a scoping choice matched to the workload (one page under a cursor does
not need Rayon's multi-tab parallelism), *not a limitation worked around.* It is also what keeps GitHub
Pages hosting clean: **no `SharedArrayBuffer`/COOP-COEP header requirement, no service-worker hacks.**

## Build & deploy — non-blocking

Its **own CI lane**, separate from the verify wall, triggered on **push to a stable branch** (not every
commit) so it **never competes with tick velocity or gates a merge**. A `wasm32-unknown-unknown` build job,
static deploy to **GitHub Pages**.

## Standing maintenance rule — it is a living demo of a moving engine

- **If the single-threaded demo path's rendered output ever diverges from the native parallel path's output
  for the same page, that is a REAL REGRESSION** — track and fix it; do not let the two paths silently drift
  into showing different things.
- **Re-derive the curated page set from the oracle's cluster registry on the audit cadence**, so the demo
  ages forward with the project instead of going stale as a fixed snapshot.

## Remaining work (the demo is feasible; it is not yet built)

1. A `demo/` wasm crate: `parse → Stylo cascade → Taffy layout → tiny-skia paint → canvas ImageData`, with
   scroll/hover/click routed through the real hit-test. **No new engine code** — it wires the crates the
   `wasm-check` already builds.
2. A build step that bakes curated snapshots (from the cluster registry) + their Chromium references.
3. The HTML/JS-glue shell (canvas + the side-by-side toggle + the in-product real/not-real statement).
4. The separate GitHub Pages CI lane.
5. A parity check: demo-path output == native-path output per page (the maintenance rule, as a gate).
