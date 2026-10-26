# Manuk v1.0.0 — first public milestone

**Manuk is a from-scratch web browser engine written in Rust, built to render the real internet and be driven by an agent.** This is our first tagged release: a working binary and an honest marker of where the project stands and where it's going. It is a **milestone, not a finish line** — see "Honest state" below before you judge it against Chrome.

## What it is

A real rendering + scripting engine assembled from a Rust stack:

- **HTML/DOM** — html5ever parsing into a live DOM with CSSOM.
- **CSS cascade** — Servo's **Stylo** (the same cascade engine Firefox ships).
- **Layout** — **Taffy** flexbox/grid + block/inline geometry.
- **JavaScript** — **SpiderMonkey** (mozjs) via FFI, with a growing Web API surface (DOM bindings, `getComputedStyle`, events, timers, fetch, canvas, media source).
- **Text & paint** — parley/swash shaping, tiny-skia rasterization.
- **Agentic surface** — the engine is scriptable/observable so an agent can load, read, and drive pages.

The browser binary is `manuk` (the `shell` crate).

## Honest state (read this)

We are **not claiming a Chrome-equivalent daily driver yet.** Measured against Chromium on a 200-site representative CrUX corpus and the Web Platform Tests:

- **Web-platform conformance (WPT):** **78.2%** of active-area subtests pass (layout + DOM + CSS; 116,846 / 149,430). *(This is a monotonic lower-bound ledger, not a marketing figure.)*
- **Full-page visual match to Chrome (M1):** **~36%** of corpus sites reach a ≥0.75 structural-match score.
- **Content coverage is much higher than that 36% suggests:** the engine draws the *right content* on a broad majority of sites (~87% mean element coverage) — **the gap is placement/layout geometry, not missing content.** In plain terms: *it draws the right boxes, and increasingly in the right places, but not yet pixel-faithful to Chrome across the whole web.*
- **Interactivity (M2):** broad JS/Web-API primitives are present and pass on fixtures, but full real-site interactive parity with Chrome is **not yet certified**.

**So:** Manuk renders a broad swathe of real websites recognizably and runs their scripts, but it does **not** guarantee a Chrome-identical result on every site, and interactive/visual compatibility is still maturing. Treat v1.0.0 as a capable preview you can run and evaluate — not a drop-in Chrome replacement.

## Roadmap

The project is executed as a continuous, gated improvement loop (Rust engine work only; every change is ratcheted so a regression is never traded for a feature). The path:

1. **M1 — RENDER** *(in progress, ~36%)*: per-site structural match ≥0.75 to Chrome across the CrUX corpus. The binding constraint is placement/layout-geometry math (flex/grid sizing, containing-block/viewport, table/column layout, font-metric-driven heights).
2. **M2 — FUNCTION**: certified real-site interactivity — a stratified smoke suite A/B-diffed against Chromium via the agent's `script.evaluate`, gated on render scorability.
3. **Daily-driver certification**: ≥95% of representative CrUX sites both **render** and **function**, per `docs/loop/DAILY-DRIVER-CERTIFICATION.md` (the authority). v1.0.0's *original* trigger was cert-pass; we are shipping this milestone ahead of that, deliberately and transparently.
4. **Post-daily-driver — the agentic browser**: the engine's real purpose — a browser an agent drives natively (load, read the accessibility/DOM tree, act, verify), not just a human GUI.

## Build & run

```
cargo build --release --bin manuk
./target/release/manuk <url>
```

Requires a Rust toolchain and the SpiderMonkey build prerequisites.

## Provenance

1,834 commits since 2026-07-09, developed via an autonomous, test-ratcheted engineering loop. Full methodology, scope, and constitution live in `docs/loop/` and `CONSTITUTION.MD`.

---

*v1.0.0 is an honest milestone: a running browser, a truthful measurement of its compatibility, and a clear roadmap to a daily driver and an agent-native web.*
