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

## Tick 3 — L10: history.pushState/replaceState + popstate + location (2026-07-11)
- Selected: top UCB score (V/C=2.0); the natural complement to Tick 2 fetch — SPAs pair
  data-load with client-side routing, so this unblocks the same site class.
- Implemented: real `location` (parsed from the doc URL, threaded through `install` as `%URL%`);
  `history` (pushState/replaceState/state/length/back/forward/go) that updates location + queues
  a host op via native `__historyPush` (window.open-style thread-local); a window-level event
  registry so `popstate` fires (it's a window event, not a node event). `PageContext` +
  `Page::{take_history_ops, fire_popstate}`; shell `handle_history_ops` reflects pushState/
  replaceState into the omnibox URL + back/forward stack with NO navigation.
- Verified: HEADLESS — page interactive test scenarios (7) click→pushState updates
  `location.pathname` + queues the op with serialized state, (8) `fire_popstate` runs
  `onpopstate` with restored state. Parity 72/72; `_sm` + JS-less green.
- Disk: pruned target incremental dirs (42G free, 86%; > 25G, no nuke). Commit `7f1b35d`.
- Reflect: same-document Back/Forward *button* → popstate (with per-entry state restore, needs
  a same-doc flag on SessionHistory) is a follow-on (L10b); `document.location`/`document.URL`
  mirror pending. Next: Tick 4.

## Tick 4 — L04: downloads to disk (2026-07-11)
- Selected: V/C=2.0; self-contained, low-risk, a concrete item from the original diligence needs
  list, cleanly HEADLESS-verifiable (chosen over the higher-V but heavier/observers ticks to
  land a complete verified feature).
- Implemented: `engine/net/downloads.rs` (pure policy+FS tail: is_attachment, suggested_filename
  w/ RFC 6266 + path-traversal sanitize, download_dir, write_download w/ de-dupe); `manuk_page::
  fetch_document -> Loaded::{Document,Download}` branching on response headers; shell
  `finish_download` (write + record + restore prior page) + a Downloads hamburger entry.
- Verified: HEADLESS — 4 net unit tests (attachment detection; filename from CD/URL incl.
  RFC5987 + traversal safety; write+dedupe). Parity 72/72; workspace builds; page interactive
  test still green. Commit `d6022ff`.
- Reflect: the download rides in on a navigation then restores the prior page (re-fetch from
  cache) — fine, but a `<a download>` attribute trigger + a streaming-to-disk progress shelf are
  follow-ons (logged). Next: Tick 5 (forced-highest-U).
