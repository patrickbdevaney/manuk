# Manuk — JOURNAL (append-only, one block per tick)

_Minimal history for audit + resume. See [[CONSTITUTION]] §4/§6, [[RESUME]] for the live pointer._

## Tick 0 — bootstrap (2026-07-11)
- Established the loop: CONSTITUTION, STATE, LEDGER, RESEARCH, DECISIONS, JOURNAL, RESUME.
- Absorbed outstanding beneficial work into LEDGER Tier A; seeded Tier B innovation.

## Tick 1 — L14 slice: boot-time window/screen metrics (2026-07-11)
- Selected: front-loaded a low-cost, high-leverage COMPAT/FINGERPRINT slice (SPA frameworks
  read window/screen metrics at boot; absence = instant ReferenceError abort).
- Implemented: window prelude defines innerWidth/innerHeight/outer*, screen.*, devicePixelRatio,
  screenX/Y, a no-match matchMedia, requestAnimationFrame (via setTimeout). Honest desktop values.
- Verified: HEADLESS — interactive test scenario 4 (`1280x720x1:function:function`). Parity 72/72.
- Disk: pruned target/debug/incremental (−10G → 47G target; 42G free, 86% full). Policy: when
  free < 25G, `rm -rf target/debug` (accepts a one-time mozjs debug rebuild); avoid otherwise.
- Reflect: L14 partial (T=1); full fingerprint surface (fonts, timezone, canvas/WebGL
  consistency, true window size) remains. Commit `1a717d0`.
- Next: Tick 2 — L01 fetch()/XHR (highest exploit, V/C=2.0, the biggest COMPAT lever).

## Tick 2 — L01: real fetch() + XMLHttpRequest for page JS (2026-07-11)
- Selected: highest-exploit Tier-A item (V/C=2.0); SPAs load their data via fetch/XHR, so
  without it the whole SPA class stays blank.
- Discovery mid-tick: a half-built fetch already lived in `event_loop.rs` — a fake thenable
  that broke `.then().then()` chaining, and `PageContext` ran `event_loop::run` which
  auto-resolved every request with status 0 at load. My first pass added a *parallel* system
  (native `__fetch` + Rust queue + prelude shims); it collided (the event-loop prelude
  overwrites `globalThis.fetch`), surfacing as `[object Object]`. Reflected → **deduped**:
  deleted the parallel path, unified on the existing plumbing, and fixed it properly.
- Implemented: fetch() returns a REAL Promise (native jobs routed via job_queue → chaining +
  await work); spec-shaped Response (ok/status/text()/json()/clone()/headers); richer XHR;
  bodies threaded; kind-agnostic `__deliver`. New `run_deferred` (leaves fetch/XHR queued for
  the host) + `drain_pending` + `deliver`. `PageContext::{take_fetches,resolve_fetch}`;
  `Page::{take_fetches,resolve_fetch,base_url}`; shell `pump_fetches()` (block_on manuk-net,
  relayout on mutation). `manuk-net` pub-exports `Bytes`.
- Verified: HEADLESS — page interactive test scenarios (5) fetch `.then` chain mutates the DOM
  with the body, (6) XHR `onload` sees status+body; `event_loop::fetch_and_xhr` updated to the
  spec Response + passes. Parity 72/72; `_sm` and JS-less builds green.
- Disk: pruned target/{debug,release}/incremental (42G free, 86%; > 25G, no nuke). Commit
  `91a22bb`.
- Reflect: async non-blocking fetch (don't stall the UI thread on block_on), real request
  headers/Request/Response fidelity, and `AbortController` are logged follow-ons (new ledger
  items). Next: Tick 3.
