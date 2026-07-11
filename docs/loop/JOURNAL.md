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

## Tick 5 — L32: predictive prerender of the likely-next navigation (2026-07-11)
- Selected: the forced-highest-U tick (§5). U8 tied three ways (L31 llama, L32 prerender, L34
  service worker); L31 (needs a GGUF model → EXTERNAL) and L34 (C9) can't be honestly
  HEADLESS-verified in one tick, so picked the highest-U item that keeps the verification
  invariant: **L32** (PERF, HEADLESS).
- Implemented: `shell/prerender.rs` (pure predictor: hovered link, else first same-origin content
  link; gated by `is_prewarmable` = same-origin http(s) GET only). On hover the shell prewarms
  the predicted URL off-thread (bounded to 1 in-flight), builds it into the bfcache keyed by the
  requested URL without disturbing the current page (`finish_prewarm`), and `goto` checks the
  bfcache first so a prewarmed click is an instant swap. `build_page` extracted + shared with
  `finish_load`.
- Verified: HEADLESS — 5 predictor unit tests (hover wins; cross-origin hover → same-origin
  content fallback; non-http never prewarmed; no same-origin → None; same_origin gate). Parity
  72/72; workspace builds; page interactive test green. MEASURE: prewarm/hit logged. Commit
  `c6925f7`.
- Reflect: idle (non-hover) prerender from ranked content links, a surfaced hit-rate metric, and
  never-cross-origin speculation rules are follow-ons (logged). Next: Tick 6 (normal UCB; Tick 10
  is the next forced-highest-U).

## Tick 6 — L03: cross-window postMessage + window.opener (2026-07-11)
- Selected: top UCB score (~4.4); completes the OAuth-popup round-trip window.open began (an
  explicit needs-list item) and is HEADLESS-verifiable.
- Implemented: reused the host-queue pattern (no parallel queue). window handles
  (`__makeWindowRef`) carry a target window id; `postMessage` routes a JSON payload via native
  `__postMessage`; `__deliverMessage` fires a `message` MessageEvent through the window event
  registry (built in Tick 3). `window.open` now allocates a window id + returns a real handle;
  a shared `next_window_id` counter keeps open()-ids and host tab-ids from colliding.
  `PageContext::{set_identity, take_messages, deliver_message}` + Page/lib wrappers. Shell keeps
  win_id↔tab maps + per-tab opener, seeds identity on load, and `pump_messages` routes each send
  to the target tab's page (active or background via `Browser::page_mut`).
- Verified: HEADLESS — page interactive scenarios (9) window.open().postMessage queues with the
  popup id + targetOrigin, (10) deliver_message fires onmessage with data/origin/source.__winId.
  Parity 72/72; `_sm` + JS-less builds green. Commit `7c4a1f6`.
- Reflect: BroadcastChannel, MessageChannel/MessagePort, full structured clone (Blob/Map/Set),
  and window.name targeting are follow-ons (logged). Next: Tick 7 (normal UCB).

## Tick 7 — L02: MutationObserver (2026-07-11)
- Selected: top UCB score (~4.4); the next SPA-compat lever — frameworks mutate the DOM after a
  fetch and observe it; absent the API their code throws at construction.
- Implemented: a real `MutationObserver` in the window prelude (observe with attributes/childList/
  characterData/subtree/attributeOldValue/attributeFilter, disconnect, takeRecords). The native
  DOM-mutating methods (setAttribute/removeAttribute/appendChild/insertBefore/removeChild/remove/
  textContent/innerHTML) emit records via a `record_mutation` helper; delivery is a microtask
  (queueMicrotask) so a handler's mutations surface after the current script but before the
  dispatch/load/fetch call returns (all drain microtasks via `run_deferred`). Subtree matching
  walks parentNode over live reflectors. No host round-trip — entirely in engine/js.
- Verified: HEADLESS — page interactive scenario (11): a click sets an attribute on the target,
  an attribute on a descendant (subtree), and appends a child → observer fires with the batched,
  correctly-typed records. Parity 72/72; workspace + js + page tests green (ignored js tests'
  exit-SIGSEGV is the pre-existing leaked-runtime teardown; assertions pass first, confirmed via
  git-stash). Commit `861a66c`.
- Reflect: characterData oldValue nuance, observer GC lifetime, and IntersectionObserver/
  ResizeObserver are follow-ons (logged). Next: Tick 8 (normal UCB).

## Tick 8 — L11: responsive @media (2026-07-11)
- Selected: human-table-stakes item (UCB near-tie with agentic L17, broken by the user's
  human-first ordering); a known weak frontier for "look like Chromium/Gecko".
- Discovery: the Stylo cascade matched only top-level `CssRule::Style` and never descended into
  `@media` blocks — so responsive rules never applied at ANY width (the viewport was already
  threaded into the Device; only the walk was missing).
- Implemented: `match_rules_recursive` descends into `CssRule::Media`, evaluating each query
  (`MediaList::evaluate` + `CustomMediaEvaluator::none`) against the Stylist's real-viewport
  Device and including inner rules only when it matches (nested @media recurse). `matchMedia`
  upgraded from a no-match stub to a real min/max-width/height/orientation/prefers-* evaluator
  (comma=OR, ` and `=AND) so JS branches agree with the CSS cascade.
- Verified: HEADLESS — css `media_query_applies_by_viewport_width` (max-width applies at 400,
  none at 800, min-width at 1200); page interactive (12) matchMedia not-narrow/is-wide/in-range.
  Parity 72/72; css 27+2, page 12-scenario, workspace all green. Commit `02595bc`.
- Reflect: container queries, matchMedia resize-listeners, full media-feature set, @supports are
  follow-ons (logged). Table stakes now solid → un-defer the agentic L17 for Tick 9.
  Next: Tick 9 (L17). NOTE: Tick 10 is the forced-highest-U tick.

## Tick 9 — L17: agent-native targeting (AG2 pruning + AG3 dual targeting) (2026-07-11)
- Selected: top UCB (~4.3), un-deferred now that human table stakes are solid — the agent-native
  differentiator, advancing the under-developed AGENT-EXT axis.
- Implemented: `agent/src/targeting.rs`, pure functions over `engine/a11y`. AG2
  `prune_for_task(tree, task)` keeps interactive + name-matching nodes + ancestor chains, drops
  decorative/off-task subtrees. AG3 `resolve_target(tree, intent, viewport)` scores candidates by
  semantic (keyword overlap + exact-label + action-role bonuses) and visual (in-viewport, larger,
  central) signals, weighted 0.72/0.28; returns node + click point + confidence margin.
- Verified: HEADLESS — 4 unit tests over a synthetic page tree (prune shrinks + keeps the right
  nodes; resolve picks the prominent nav button over the same-text footer link; identical buttons
  → low confidence; no candidate → None). Parity 72/72; agent 109 + workspace green. Commit
  `6524b11`.
- Reflect: wire AG3 into the shell/BrowserAction path, a learned scorer, and OCR fallback are
  follow-ons. Next: Tick 10 (FORCED-HIGHEST-U).

## Tick 10 — L31 slice: action grounding (FORCED-HIGHEST-U) (2026-07-11)
- Selected: §5 forces the highest-U item; L31 (llama grounding, U8) is highest. Its model
  inference is EXTERNAL (can't cleanly HEADLESS-verify), but the **grounding half** — model
  Action → concrete on-page target — is pure + verifiable, and composes Tick 9's scorer. Landed
  that half (honors both forced-highest-U and the verification invariant).
- Implemented: `agent/src/grounding.rs` `ground_action(action, tree, viewport, min_confidence)`
  → `Grounded::{Direct, Ready, Ambiguous, Unresolved}`; text-targeting actions resolve via
  `targeting::resolve_target`, below-margin targets flagged Ambiguous. Fixed a targeting
  false-positive: with an intent, a candidate must match it by name (role/visual bonuses only
  break ties among real matches) — "Checkout" no longer resolves to an unrelated button.
- Verified: HEADLESS — 5 grounding tests + corrected targeting tests. Parity 72/72; agent 114 +
  workspace green. Commit `2db6920`.
- Reflect: inject a real backend (external) to produce the Action; a disambiguation prompt on
  Ambiguous; wire Grounded into the shell BrowserAction executor. Next: Tick 11 (normal UCB;
  Tick 15 is the next forced-highest-U).

## Tick 11 — L05: file uploads (multipart) (2026-07-11)
- Selected: rotated back to human table stakes after two agentic ticks (9,10); top human UCB item,
  a concrete "run any website" gap (forms were GET-only, skipping file inputs).
- Implemented: `engine/net/multipart.rs` (pure RFC-7578 encoder: `Part::{field,file}`, `encode`
  to exact bytes, header-injection escape, deterministic `boundary_from_seed`); `agent/forms.rs`
  `file_inputs` + `multipart_submission` (text-field parts + file parts → POST `MultipartPost`).
- Verified: HEADLESS — net multipart exact-byte tests + agent forms multipart POST assembly.
  Parity 72/72; net+agent 117 + workspace green. Commit `fc41bc9`.
- Reflect: shell OS file-picker on `<input type=file>` click, multi-file, drag-drop, streaming,
  progress are follow-ons (the GUI picker is the remaining thin integration). Next: Tick 12.

## Tick 12 — L30: in-process automation surface (2026-07-11)
- Selected: top raw-UCB (~4.6); the agent-native differentiator composing Ticks 9-10; user's
  latest directive explicitly invited "innovations".
- Implemented: `agent/src/automation.rs` — `Selector{role,name,nth}` durable element reference
  (resolves by semantics, survives sibling mutations); `Condition{Visible,Gone,TextPresent,
  UrlMatches,CountAtLeast}` with `evaluate`; `wait(cond, snapshots)->Outcome::{Met{at},Timeout}`
  over a caller-driven snapshot stream (no timers); `assert_that(...)->AssertResult`.
- Verified: HEADLESS — 5 unit tests (selector stable across sibling insert; nth among
  duplicates; each Condition; wait Met/Timeout; assert failure detail). Parity 72/72; agent 122 +
  workspace green. Commit `034c275`.
- Reflect: expose as a scriptable session/BiDi command; retries w/ backoff; data-testid
  selectors; an act→wait→assert step helper. Next: Tick 13.

## Tick 13 — headless screenshot discipline + flex-block-child fix (2026-07-11)
- Trigger: user unblocked headful verification (screenshots) + llama.cpp. Reprioritized ahead of
  L18 (re-queued) because visual verification is the force-multiplier the user asked for.
- Built: `manuk-wpt render` — CPU-painter PNG of any HTML (+ optional headless-Chrome reference),
  readable back for eyeballing. NO window/GPU. Proven working.
- First fruit: the first screenshot caught a major bug — a flex card row rendered as ONE
  full-width card. Root cause: `content_right_extent` counted a block child's container-filling
  width (≈1e6 at the max-content probe), so the first flex item measured to the whole container.
  Fixed by ignoring a box's own edge when it filled the measuring width. Affects most real flex/
  grid layouts (cards/columns nest block content).
- EXTERNAL proven: `llama-server` + `Qwen3.5-4B.Q4_K_M.gguf` → prompt yields
  `{"Type":{"field":"Email","text":"alice@example.com"}}` (with `/no_think`), the exact Action
  the Tick-10 `ground_action` resolves. Server stopped after (restartable).
- Constitution §7 rewritten: added `VISUAL` (render+Read PNG) + made `EXTERNAL` (llama) runnable;
  documented both disciplines + the mmproj multimodal lever.
- Verified: VISUAL — before/after screenshots (1→3 cards, matches Chrome); layout regression test;
  parity 72/72; layout 28+1, workspace green. Commit `64ba73a` (docs to follow).
- Reflect: border-radius + box-shadow paint are the next visible gaps (VISUAL-verifiable now);
  shell-chrome headless paint for tab-strip pixels. Next: Tick 14.

## Tick 14 — L43: border-radius + box-shadow paint (2026-07-11)
- Selected: the two visible "look like Chromium" gaps the Tick-13 screenshot exposed (square
  corners, no shadows) — now VISUAL-verifiable.
- Implemented: `border_radius` (uniform) + `box_shadow` (first outer) on ComputedStyle, parsed in
  MinimalCascade (paren-aware tokenizer for `rgba(...)`; inset/multiple/spread → None, never a
  wrong shadow) and mapped from Stylo. Threaded through LayoutBox. New paint items RoundRect +
  Shadow: rounded rects as a tiny-skia Bézier path (k=0.5523); the shadow's soft edge is stacked
  concentric rounded rects with quadratic alpha falloff (tiny-skia has no Gaussian blur). Damage
  boxes grow by `blur`.
- Verified: VISUAL — radius-16 / pill-45 / square+shadow / radius-no-shadow render correctly and
  match Chrome's shapes; the card sample now has Chrome-like rounded corners + shadows. HEADLESS —
  paint pixel tests (corner cut away, centre + straight edges filled; shadow bleeds outside the
  box but not across the canvas). Parity 72/72; css 21, layout 29, paint 6, workspace green.
  Commit `e441564`.
- Governance: **ADR-004 mission amendment** recorded (maximal traversal earned by CAPABILITY — a
  fifth real browser with its own genuine fingerprint, impersonation is off-strategy; sites are
  representative points, not a checklist; ambidextrous spine — one engine, no forked page
  pipeline). Constitution §0 rewritten to match, incl. the traversal-blocking prioritization rule.
- Reflect: per-corner/elliptical radii, radius-clipping for borders/images, inset + multiple
  shadows are follow-ons. Next: Tick 15 (FORCED-HIGHEST-U, now filtered by traversal impact).

## Tick 15 — L16: Custom Elements + Shadow DOM (FORCED-HIGHEST-U) (2026-07-11)
- Selected: §5 forces highest-U; **ADR-004's traversal-blocking rule** then vetoed the nominal
  winner (L34 service worker, U8/C9 — sites *degrade* without it) in favour of **L16** (U7):
  unsupported web components make content **simply not appear**. First decision the amendment
  changed.
- Discovery: the DOM + layout ALREADY modelled shadow roots + the flat tree (slots, declarative
  `<template shadowrootmode>`). But the path was broken and there was no JS API.
- **Two real bugs (both surfaced by the screenshot discipline):**
  1. **CRASH** — layout's `collect_positioned` walked the *node* tree while all other layout walks
     the *flat* tree, so it reached unslotted light-DOM children of a shadow host (never rendered →
     never styled) and panicked indexing styles. **Any declarative-shadow-DOM page crashed layout.**
     Now flat-tree + non-indexing lookup (a missing style can never crash layout).
  2. **Stylo styled no shadow content** (the shell's default cascade walks the node tree) → blank.
     `cascade_via_stylo` now adopts MinimalCascade's N4 flat-tree scoped result for nodes it missed.
- Implemented: `attachShadow({mode})` + `shadowRoot`; `customElements.define/get/whenDefined` with
  real upgrade — `HTMLElement`'s constructor RETURNS the element under upgrade, so (per ES) the
  derived ctor's `this` becomes it and `constructor(){super(); this.attachShadow(...)}` runs on the
  real element, as the spec's upgrade does. connectedCallback + attributeChangedCallback +
  observedAttributes. Upgrade sweep in the MutationObserver microtask catches later inserts.
  `tests/wpt` gains an optional `spidermonkey` feature so `render` can screenshot JS-built pages.
- Verified: HEADLESS — scenario (13). VISUAL — declarative shadow DOM (block + inline hosts, slot
  assignment) and a JS-defined custom element both render end-to-end. Parity 72/72; css 21,
  layout 29, paint 6, dom 9, workspace green. Commit `8f76665`.
- Reflect: **new bug L45 — block-in-inline** (a block box inside an inline loses its box;
  pre-existing, not shadow-specific — it's why an inline host with block shadow content renders
  bare text). High traversal value. Next: Tick 16 = L45.
