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

## Tick 16 — L45: block-in-inline (2026-07-11)
- Selected: top traversal-blocking item (found while VISUAL-verifying Tick 15). A block box inside
  an inline lost its box entirely — text flowed, background/padding/border vanished. Ubiquitous in
  real markup (`<div>` inside `<a>`/`<span>`/a custom element).
- Cause: `layout_children` decides `has_block` from DIRECT children only, so an inline wrapping a
  block sent the parent down the pure-inline path, where `collect_inline_node` harvested the
  block's TEXT as inline words and discarded its box.
- Fix (CSS2 §9.2.1.1): **blockify** an inline that contains a block-level box (`is_block_level` +
  new `inline_contains_block`, recursing through inline-only descent; inline-block/flex/table are
  atomic and don't propagate). The parent opens a BFC and the inline's children split into
  anonymous blocks + the block child — the spec's resulting box structure. Documented deviation:
  the inline's OWN background paints behind the blockified box, not per split fragment.
- Verified: VISUAL — repro now matches Chrome (yellow padded block); the previously-blank Tick-15
  inline-shadow-host page renders fully. HEADLESS — `block_inside_an_inline_keeps_its_box`.
  **Parity 72/72** (core inline/block seam — the gate that matters most here); layout 30,
  workspace, page interactive all green. Commit `e7cd623`.
- Reflect: two ticks running, the screenshot discipline has found the bug the tick then fixed.
  Next: Tick 17 = **empirical real-page visual audit vs Chrome** — stop guessing which fidelity
  gap matters; render real pages side-by-side and let the diff pick the work.

## Ticks 18–22 (user-feedback arc + EPOCH-1 fallout) (2026-07-11)
- **T18 — CRITICAL dead affordances.** User ran the binary: "bookmark and find in page dont work".
  Both were dead affordances (§1.8, ratcheted by EPOCH-1 days earlier). Find set a flag and **drew
  no UI at all**; bookmark toggled state and logged. Added a real find bar (live match count), a
  ★/☆ star + toast, Chromium-style zoom −[%]+, and the missing standard keybindings.
- **T19 — STANDING GATES (ADR-010).** JS interactivity + Chromium CSS/HTML fidelity are continuous
  obligations, not features; they were only checked *opportunistically*. Now G1/G2/G3/G4 run every
  tick via `scripts/verify.sh`. G3 makes §1.8 machine-checked (a menu item cannot ship without
  declaring its observable effect).
- **T20 — DEBT-1 paid.** The UI thread `block_on`'d the network 4× while building a page (scripts,
  CSS, images, page fetch). Root blocker: `fetch_images` held an `Rc` across an `.await`, making the
  future `!Send` — that one detail pinned the whole build to the UI thread. Now everything is
  prefetched off-thread and the UI thread builds with ZERO network. **4 → 1 blocks.** (User's
  "refresh lag" — gone.)
- **T21 — G1 + ADR-011.** Built real-site visual fidelity (render real URL → Chromium screenshot →
  block-grid compare → side-by-side composite). It immediately exposed that **the gates were testing
  a cascade no user ever sees**: `manuk-wpt` defaulted to MinimalCascade, the shell ships Stylo.
  Fixed (stylo now default; parity 72/72 under both, so it cost nothing).
- **T22 — W1.** Screenshot showed Wikipedia's language dropdown painting over the infobox. Root
  cause: **`visibility` and `opacity` were not supported at all.** The modern web hides dropdowns/
  modals/tooltips with `visibility:hidden`+`opacity:0` (animatable, unlike `display:none`) — so
  every one of them painted on top of the page. Implemented both (visibility inherited; opacity
  folded to an effective subtree value). Dropdown/Tools/Main-menu overlaps gone.
- **Recurring lesson, now 3×:** Wikipedia's score moved 81.0→81.7% for a *massive* structural
  repair. **The score gates; the eyeball diagnoses.**
- Remaining from the Wikipedia screenshot (next): missing left TOC sidebar + right Appearance panel
  (page-level CSS Grid), broken infobox table layout, unrendered icon squares.

## Tick 25–26 (2026-07-11) — the A/B screenshot found four class bugs, then a fifth that wasn't a layout bug at all

**COVERAGE 77.0% → 99.7%. Wikipedia's TOC 1,949px → 374px (Chrome: 364px).**

Five findings, in the order they fell:

1. **mask-image.** The modern web draws an icon as an *empty element* with a `background-color`
   shaped by a mask. We painted the background and ignored the mask: a black square where every
   icon should be.
2. **Inline elements had no geometry.** An empty `<span id=…>` anchor produced no box and no
   fragment, so it did not exist. Chrome gives it width 0 and a line-height-tall rect, and pages
   depend on that. 1,079 spans + 298 anchors on ONE article — 98% of everything we were missing.
3. **`inline-flex` did not exist.** Stylo mapped it to block-level flex, so every icon button
   filled its container.
4. **Flex max-content was measured by laying the container out at a 1e6 width and reading the
   right edge.** `max-width` clamps the container back down and `justify-content:center` centres
   the content inside *that* — a 32px button measured 234px, auto margins measured 500,532px.
   Ask taffy for max-content instead.
5. **`'localStorage' in window` — the gate.** The web feature-detects and *grades* the browser.
   MediaWiki reverts `client-js` → `client-nojs` and ships its no-script fallback. That, not
   layout, was the ~5,000px of vertical drift.

### The lesson, sharper than before

We have said "THE SCORE GATES; THE EYEBALL DIAGNOSES" for four ticks. Tick 26 adds the next layer:
**the eyeball diagnoses, but only a MEASUREMENT tells you which box is wrong.** Three hours went
into staring at a stacked header. Ten minutes went into `boxes --tree`, which printed
`label.cdx-button <InlineFlex> [44 17 236×32]` and ended the argument. Build the probe *first*.

And: **when a metric will not move, suspect the metric.** Wikipedia's median dy sat at exactly
5,122px across four real fixes. It was not stubborn — Chrome's screenshot and Chrome's box probe
were rendering different pages, `node_rects` was unioning overflow into every ancestor, and the
site was serving us a degraded document. None of those were the engine.

### The crash we had been hiding

The shell called `libc::_exit()` to skip SpiderMonkey's atexit crash. That is not a fix: `_exit`
skips *every* exit handler, and in a browser those handlers flush the user's profile. The crash was
real (exit code 139, after `main` returned, with perfect output). `JS_ShutDown()` now runs in order.
**A workaround that hides a crash is a data-loss bug wearing a disguise.**

## Reassessment (2026-07-11) — the bottleneck was never verification

The directive assumed parallel verification throughput was the constraint, and asked for a
re-derived timeline once the harness, the three passes and reference-source discovery had run for a
real stretch. They have. The honest finding is that **the constraint was somewhere else entirely.**

### What actually compressed the work

Every one of the ~26 class bugs closed today was found by one of three things, and **none of them is
parallelism**:

1. **Corpus breadth.** Three sites said COVERAGE 99.7% and everything was fine. Twenty sites said a
   page was printing its own JavaScript, `:checked` never matched anywhere, checkboxes were
   invisible, and docs.python.org rendered entirely dark. A three-site sample is not a benchmark; it
   is an anecdote that confidently reports that a bug on one of those three is the most important bug
   on the web.
2. **Better probes.** `boxes --tree` printing a box's COMPUTED display ended a multi-hour argument
   in ten minutes. `MANUK_TRACE_INTRINSIC` printed the number flex wrapping is decided by, which is
   otherwise invisible.
3. **Printing the exception.** `"a page <script> threw; continuing"` is a shrug. Printing the message
   turned it into two exact TypeErrors and replaced an hour of bisecting. **The browser was naming
   its own bugs out loud and we were discarding the message.**

### What did NOT compress it

* **The verification harness.** Built, validated (13 PASS / 13 FAIL / 0 escalate against ground
  truth; 429s and dead keys fail over cleanly; nothing is ever dropped). It has not yet been the
  bottleneck-breaker, because **verification was never the bottleneck** — deciding *what is wrong* was,
  and that is the one thing the harness is explicitly forbidden from doing. It is the right tool and
  it is correctly scoped; its value will arrive when the work becomes wide and shallow (Pass 3's long
  tail), not while it is narrow and deep.
* **Reference-source discovery.** Not yet used. The bugs found were not ambiguous-algorithm bugs
  (margin collapsing, line breaking); they were *absent-feature* bugs — a stub returning `false`, a
  missing property, a filter that omitted a condition. Reading Blink would not have found a single
  one of them faster than rendering the page and looking at it. It will earn its keep on the
  edge-case-heavy items (float/BFC interaction, event ordering), which is exactly where the ADR says
  to reach for it — and nowhere else.

### The re-derived numbers, and their ceiling

**Bar 1 (functional breadth): 17/20 sites usable, from a standing start today.** The remaining three
are named with what each is waiting on. Bar 1 across the corpus is close.

**Bar 2 (pixel precision): 7/20.** This is a different problem and it will not go at the same rate.
The residual is dominated by **font metrics** — our glyph advances differ from Chrome's, so text
wraps at different points, so every box below it moves. That is not twenty-six small bugs; it is one
deep one, and no amount of parallel checking or algorithm-reading shortens it.

**Do not extrapolate Tier 2/3 from Tier 1's pace.** Tier 1 went fast because it was made of absent
features, and absent features are cheap once you can SEE that they are absent — which is what the
corpus and the probes bought. Tier 3's real blockers (codec licensing, GPU driver integration, DRM)
are external-integration problems. Reading Chromium's approach to them substitutes for none of the
licensing or certification work, and no verification fan-out touches them at all.

## Tick 38 (2026-07-12) — the differential oracle is live, and it changed what "next" means

**Discovery method: THE ORACLE** (first tick where that is true).

Built METHODOLOGY Part 2: one snapshot, fed identically to both engines; Chromium probed for every
`[id]` element's tag, computed `display` and box; diff; **cluster by root cause**; rank by distinct
sites explained. Two hygiene rules are enforced in code, because both have already burned us:

* **One snapshot, both engines.** Fetching per-engine compares two different documents and calls the
  difference a bug — that is what pinned a metric at 5,122px across four *correct* fixes.
* **Never diff against a degraded oracle.** The health check asks what Chromium actually DREW
  (element count, visible text length), not how many elements carried an id. A bot wall is discarded,
  not scored as our bug.

### What it found in its first minutes

1. **Monospace was 23% too large.** Chrome's default monospace size is 13px, not 16px — which is why
   `<code>` famously looks smaller than its prose. Every code block and every inline `<code>` on the
   web was oversized, and every documentation site's layout was pushed down by it. `<pre>` 58px → 47px
   (Chromium: 45px). **Fixed.**
2. **Generic font families were never resolved.** `fontdb`'s defaults are `Arial`/`Times New Roman` —
   Windows names, usually absent on Linux — so `font-family: sans-serif` landed on an arbitrary
   fallback. Chromium asks fontconfig and gets Noto Sans. **We were measuring a different font's
   widths for every string on every page**: the same sentence came out 305px for us, 317px for
   Chromium, so every line wrapped at a different word and every box below it moved.

### The finding that mattered more than either fix

Fixing the selection **turned the wall red** (72/72 → 69/72) on `valign` and `white-space-nowrap` —
both LINE-HEIGHT and ADVANCE probes. The wall is right. The selection is fixed; the metrics computed
on top of it are not:

* `swash` advances disagree with Chromium by **~11% on monospace** (6.9px/char vs 7.83).
* Our `normal` line-height is a **1.2× guess**, not the font's own ascent/descent/lineGap.

So the fix is **held behind `MANUK_FONT_SYSTEM=1`**, not landed. Shipping a measured regression to buy
an unmeasured improvement is exactly the trade the wall exists to refuse (METHODOLOGY Part 18: *no
gate is ever loosened to make a feature land*).

**Next tick is therefore chosen for me, not by me:** adopt **Skrifa** for metrics/outlines/hinting
(METHODOLOGY Part 15 — the library Chromium itself ships) rather than re-deriving advance math, then
un-flag the font selection and expect both the wall and Bar 2 to move together.

That is the oracle working as designed: it did not just find bugs, it **re-ordered the queue**, and
it produced a number (11% advance error) that turns "fonts feel off" into a bounded integration task.

### Also this tick
* **Every swallowed exception now reports its message.** Six catch sites were shrugging. This is the
  whole mechanism of the Framework Exception Miner (Part 9) — it only works if nothing is discarded.
* METHODOLOGY Parts 15–20 folded in; §4.1 **saltatory breadth first** made an explicit selection rule:
  an item that unlocks a whole *class* of design pattern outranks everything narrow or deep, always.

## Tick 14 — the oracle pays for itself: fonts, flex, and the frozen tab (2026-07-12)

Six landings, all chosen by measurement rather than by plan, and all Pass-1 breadth (METHODOLOGY
4.1). Every one of them was a *whole class* of page failing, not a pixel gap.

- **`font-family` was never mapped from the cascade.** Not partially — at all. Every page on the
  web rendered in one fallback face regardless of its CSS. Every "font metrics" divergence the
  oracle had ever reported was this bug in a costume: we were not mismeasuring the font, we were
  not *using* it. Ten `i` at 100px now match Chromium exactly across default/serif/sans/mono
  (278/278/222/602 vs 277.84/277.84/222.17/602.06) — including the fact, which I did not believe
  until I measured it, that **Chrome's default font is a serif**.
- **The generic-family flag was right to be red.** Held behind `MANUK_FONT_SYSTEM` because it took
  the wall 72/72 → 69/72. The reading was "adopt Skrifa (Part 15)". Wrong: the preference lists came
  from `fc-match <generic>`, and Chrome never asks fontconfig for the bare generic — it asks for
  *Arial* / *Times New Roman*, which resolve to the Liberation faces. Noto's line box is 1.362em
  against Liberation's 1.150em, so every line on every page was 18% too tall. Skrifa would have
  replaced a working metrics engine and left the real bug in place. **The wall was not an obstacle
  to route around; the wall was the finding.**
- **The network layer had no timeout of any kind.** One blackholed tracker stalled the page until
  the kernel gave up. w3schools: 37.8s → 15.0s (Chromium: 15.2s), and coverage went *up*, 95.7% →
  100%, because the stalls were losing elements too. New gate **G_LOAD**.
- **Flex items could never shrink.** Asked "how narrow can you get?", we answered with the
  max-content width, so taffy took that as the item's minimum. Three cards in a row each demanded
  their full `width:100%` and overflowed to x=2388 — off-screen. We had no min-content computation
  anywhere in the engine.
- **A percentage width on a flex item resolved TWICE** — used width came out squared (30% of a
  1000px row → 90px, not 300). Survived because `auto` and `100%` are the two values immune to it.
- **Every responsive image rendered stretched**: a replaced element's auto height came from the
  image's natural pixels instead of its used width × intrinsic ratio, so `img{max-width:100%}` — the
  most common reset on the web — narrowed the box and left the height alone.

**Corpus (19 sites): MEAN VISUAL 78.2% → 79.8%, MEAN COVERAGE 97.8%, and every site now renders.**
rust-lang.org 45.2 → 68.6 · blog.rust-lang.org 61.0 → 71.2 · old.reddit 36.3 → 45.8 · w3schools was
a hang and is now a page.

**Two lessons, both about instruments rather than code.**

1. *The symptom names the wrong organ.* rust-lang.org's columns **looked stacked**, so I chased media
   queries, `em` breakpoints, external-vs-inline stylesheets and the re-cascade path. The boxes said
   they were in a perfect row, overflowing off-screen. Measure the boxes before theorising from a
   screenshot.
2. *An oracle must never be able to charge its own slowness to your account.* The sweep reported
   w3schools and go.dev as HANG/FAIL, and a local-snapshot bisect said "Chromium is the slow one, the
   bug does not exist" — because the snapshot had no network and skipped the very fetches that were
   hanging. Timing the two engines *separately on the live URL* found our real 37.8s. `fidelity` now
   attributes load time to whichever engine spent it, in code. Same hazard as `oracle_is_healthy`.

**And two gates that were not gates.** F1-F3 had been *silently skipping* for their whole existence —
the bench corpus they named did not exist, so `bench` printed empty tables and `verify.sh` printed a
yellow dash. The corpus is now committed and the floors are asserted. Separately, `disk-hygiene`'s
flush left a dangling symlink into tmpfs and broke the next build — in a commit whose message claimed
that failure was "designed out". It is now, actually.

Banked: `manuk-readable-web-2026-07-12`.

## Tick 15 — the invisible-content class (2026-07-12)

Four bugs, one family: **content that was laid out perfectly and could not be seen.** Every geometry
probe in the codebase called these pages correct. The boxes were right, the colours were right, the
text was in the display list at full alpha. The user saw a blank space.

- **`font-size: 0` painted glyph-shaped CONTINENTS.** At 0px, swash falls back to the face's
  *unscaled* outline and returns a 1,227×1,450 bitmap per glyph, which `blit_glyph` floods with the
  run's text colour. One zeroed word buried old.reddit's post titles under 27,000px of #888888. And
  `font-size:0` is not exotic — it is the inline-block whitespace killer and half of the
  image-replacement recipe. Underneath it, a second bug: **MinimalCascade could not parse a unitless
  zero at all**, so the size stayed *inherited* and the text rendered at full size. Stylo was right;
  the two cascades disagreed about whether text is visible. That is what ADR-011 exists for.
- **Anonymous boxes were stranded in stacking layer 0.** `z` and `clip` are keyed by NodeId and a
  synthesised box has no node — so the box holding a `z-index`'d element's TEXT sorted below that
  element's own background and got painted over.
- **Every `position:absolute` element with no insets was DELETED.** Its static position needs flow's
  cursor, flow discarded it, so the abs pass had nothing to place against and dropped the box. That
  is every React portal root, every JS-positioned dropdown, and every `.sr-only` node on the web.
  github: coverage 91.4% → 97.8%.
- **Every CSS background image was stretched to its element** — backgrounds share the `<img>` bitmap
  map, so the replaced-element blit painted a scaled copy over the correctly-tiled one.

**The method that finally worked, after four rounds of reasoning failed.** For the reddit grey I had
proof it was impossible: the display list was *correct*, no decoded image was flat, no Rect/RoundRect/
MaskedRect/Shadow carried the colour. Every instrument agreed the pixels could not exist. What found
it was bisecting the RASTERIZER — disable one display-item type at a time and re-render. Rects off:
still there. Backgrounds off: still there. **Text off: gone.** Then one `eprintln!` on any glyph
bitmap bigger than 3× its font size.

> When every instrument says the bug is impossible, they are all looking at the same layer, and the
> bug is one layer down.

**New gate G_INTERACT** (METHODOLOGY 5.2's lesson, again): UI-thread cost of tab open/switch/close,
with REAL pages in thirty tabs — an empty `Browser` measures a `Vec` and proves nothing. open 0.94ms,
switch 0.02ms, close 0.01ms, all far under a frame. It asserts the SHAPE too: closing the thirtieth
tab must not cost more than the first. Audited clean alongside: the hamburger is a flag flip, scroll
is 0.01ms, click 0.27ms, document fetch is off-thread. The only UI-thread cost a person can still
feel is the page *build* on navigation (~100ms on a large document) — that is the next target.

Corpus (18 sites): MEAN COVERAGE **99.0%**, MEAN VISUAL **81.1%**.
old.reddit 45.7 → 56.9 · github coverage 91.4 → 97.8 · rust-lang 68.6 · users.rust-lang 98.8.

Banked: `manuk-legible-web-2026-07-12`.

## Tick 16 — compliance becomes mechanical (2026-07-12)

Written at the START of the tick, which is now enforced: the pre-commit hook refuses a commit for a
tick with no journal entry. Stating the hypothesis while you can still be wrong about it is the
point; narrating a success afterwards is not journaling.

**The problem this closes.** "Did the gates run?" was a claim the user had to trust, and the
methodology had already drifted out of sync with reality once — verify-wall compression and oracle
breadth were prescribed in Parts 2 and 10 for many ticks and simply had not been done, while backlog
work carried on, and nobody noticed until it was asked about directly. Remembering the methodology is
not a mechanism.

**Four mechanisms, all live and all tested by trying to defeat them:**

- **The gate receipt.** `verify.sh` writes `.git/manuk-verify-receipt` naming the *exact tree* it
  verified (`sha256(git diff HEAD)`). `scripts/hooks/pre-commit` recomputes that name from what is
  being **staged** and refuses the commit if they differ. Verifying one version of a diff and
  committing another is now impossible rather than merely discouraged. Verified by trying it: no
  receipt → blocked; **stale** receipt (verified a different tree) → blocked.
- **Journal enforcement.** The same hook refuses any commit unless `JOURNAL.md` has a `## Tick <N>`
  entry for the `TICK:` in `STATUS.md`. Verified: bumping the tick to one with no entry → blocked.
- **`STATUS.md`** — checkable facts, not narrative, updated every tick. Current tier, measured wall
  time, oracle size, SPA-miner status, which gates are actually standing vs. pending, last five
  journal lines. Five seconds to know whether we are compliant, with no interrogation.
- **`scripts/self-audit.sh`** — every 10 ticks, and it *exits non-zero*. It checks the filesystem and
  the corpus for the artifacts each prescription would have produced if it had actually been
  executed. An audit you can pass by remembering things is not an audit. It currently reports **8
  real failures**, which is exactly what it is for.

**Tier 0, measured honestly rather than assumed:**

- **Item 1 (verify wall < 5 min): ALREADY MET.** 181s (3m 01s) on the worst realistic tick — touching
  `engine/css`, the shared-type edit that cascades furthest — and 57s warm. So mold/lld,
  cargo-nextest and workspace-hack are **not needed**, and doing them anyway would be infrastructure
  theatre performed against a target that is already satisfied. Measured, not assumed; the self-audit
  re-checks the number every 10 ticks and fails if it ever crosses 300s.
- **Item 2 (oracle at 200–500 sites): OPEN.** 20 sites today. That is an anecdote about the web, not
  a measurement of it. This is next.
- **Item 3 (ten SPA apps through the Framework Exception Miner): OPEN.** Zero run. The largest
  *unmeasured* unknown in the schedule, and cheap to measure.

Also landed this tick, before the redirect: the cascade tested **every rule against every element**
(the `Stylist` was built and then never used for matching) — 339ms → 199ms on Wikipedia; and a page
load ran the full cascade **four** times, twice with byte-identical inputs.
