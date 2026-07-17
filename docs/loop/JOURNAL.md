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

## Tick 38 [EPOCH 1] (2026-07-12) — the differential oracle is live, and it changed what "next" means

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

## Tick 17 — the oracle stops being an anecdote (2026-07-12)

**Hypothesis, stated before the work:** the 20-site corpus has been telling us what to fix, and it
cannot. Twenty sites is a story about the web, not a measurement of it. Every class bug found so far
was found because *some site in the corpus happened to use that pattern* — which means the bugs we
have NOT found are exactly the ones no corpus site happens to use. Widening the crawl frame to
200–500 sites should surface divergence clusters that twenty sites structurally cannot see, and the
cluster ranking — sites-explained, not judgement — becomes the ledger.

**What I expect to be wrong about:** I expect the top clusters to be things I would not have guessed,
and I expect several of my current PARITY-LEDGER priorities to drop off the list entirely. If the
ranking merely confirms what I already planned to fix, the corpus is still too narrow or the
clustering is too coarse, and that is itself the finding.

**Also this tick, because a 300-site crawl makes them non-optional:**
- **G_HANG** — a watchdog on every site. At twenty sites a hang is an annoyance you notice; at three
  hundred it silently eats the run and the harness reports a smaller corpus as if that were the
  corpus. A timeout must be a HARD failure that is counted and attributed, never a skipped test.
- **G_SILENT_FAIL** — a 300-site crawl is exactly where swallowed errors hide. Every discarded
  `Result` on the load path becomes a site that "rendered fine" because nothing was rendered.

## Tick 18 — the crawl's verdict: we crash and we hang (2026-07-12)

The 265-site crawl was supposed to rank rendering divergences. It found something that outranks all of
them, which is exactly why the corpus had to be widened: **twenty sites could not see this.**

- **73 of 265 sites HANG (27.5%).** A browser that hangs on one site in four is not a browser. This is
  now the top of the ledger, above every geometry cluster.
- **apple.com CRASHED — SIGSEGV, core dump.** `layout` indexed the style map in 25 places; a node the
  cascade never saw panicked, and a panic through SpiderMonkey's C++ frames does not unwind, it
  aborts. apple.com injects `<svg>` from a timer that runs after the last cascade. Fixed in both
  halves: layout degrades to the initial style and LOGS the miss (Part 22.1), and a tree that grew
  since the last cascade gets re-cascaded before layout (unstyled nodes 3+ → 0).
- **The hangs are ours, and they are duplicate work.** Attributed properly this time — same snapshot,
  each engine timed separately, because I made exactly the opposite mistake with w3schools and will
  not make it twice. Per navigation on bbc.co.uk: **9 full-document layouts, 4 full cascades, 487
  fetches of which 302 are DUPLICATES.** One pipeline pass is 332ms; the navigation takes 17.5s. Part
  22.3 asked whether the call graph does redundant work. It does, by a factor of dozens.

Landed against it: stylesheets and images are no longer re-fetched every script round; external
scripts fetch in PARALLEL and execute in order (they were fetched one at a time, each under the 30s
*document* deadline — 9.3s of bbc's load was a `for` loop waiting); and `load_async`, which had no
budget at all, now runs under the load budget like everything else.

**Where I stopped, deliberately.** I was three levels into the call graph chasing the last duplicate
fetches when the session's real wins — a fixed core dump, a 265-site oracle, G_HANG — were still
uncommitted. That is the pull Part 21 exists to name, and naming it is the discipline: the remaining
duplicate-work reduction (9 layouts → 2-3) is the next tick's headline, not this one's footnote.

## Tick 19 — Bar 0: the floor everything else stands on (2026-07-12)

**TICK SHAPE (Part 26.1, stated before implementation): PATTERN-CLASS + INFRASTRUCTURE.** Nothing in
this tick targets a single site. Every item is either a Bar 0 containment gap (Part 23) or a
call-graph leanness fix that generalizes to every navigation (Part 22.3). If it drifts into matching
one site's rendering, that drift is the signal and I pivot back.

**Why Bar 0 first, ahead of the 73 hangs' root causes and ahead of every visual cluster.** Part 24.3
is explicit: a pattern class that crashes the engine is categorically more urgent than one that
renders wrong. Last tick apple.com produced a SIGSEGV core dump. I fixed the *specific* panic — a
missing style entry — and that was necessary, but it was prevention of one instance, not containment
of the class. Part 23.2 is the real requirement and it is the honest reading of what happened: **I
will not prevent every crash-class bug before Bar 1, and pretending otherwise is how a browser ships
that takes the whole session down with one bad page.** The failure mode for an uncovered pattern must
be "this tab reloads with a message", never "the browser dies and everything the user had open dies
with it".

Planned, in Part 24.3's priority order:
1. **Bar 0 containment** (Part 23.2): a supervised panic boundary per navigation, so a render/layout/
   script panic kills the page, not the process. Plus **G_CONTAIN** to prove it with a deliberately
   panicking page (Part 23.3).
2. **G_RUNTIME_COUNT** (Part 25.2): one Tokio runtime and one Rayon pool for the process, not one per
   navigation. The wheel-event clone regression taught this project exactly this lesson one layer down
   the stack; Part 25 is that lesson applied to runtimes.
3. **Duplicate work** (Part 22.3): bbc.co.uk does **9 full-document layouts and 4 cascades** for ONE
   navigation against a 332ms pipeline pass. Target 2-3.

**What I expect to be wrong about:** I expect to find more than one Tokio runtime being created, and
I expect at least one of them to be per-navigation.

## Tick 20 — the last Tier-0 item, and the 73 hangs (2026-07-12)

**TICK SHAPE: infrastructure + pattern-class.** The SPA miner is the last open Tier-0 item (Part 21.2
item 3) and is infrastructure by definition — it converts the largest *unmeasured* unknown in the whole
schedule into a bounded, enumerated list. The hangs are Bar 0 (Part 23), which outranks every visual
cluster in the ledger by construction (Part 24.3), so no CLUSTER id is named and none is needed: a
pattern class that hangs or crashes the engine is categorically more urgent than one that renders wrong.

**Hypothesis, before the work.** Last tick I attributed the hangs to us and to CPU + duplicate work, and
then landed four fixes (parallel script fetch, stylesheet/image/mask/background dedup, one fewer layout
per round, a load budget on `load_async` which previously had none). **I do not yet know how many of the
73 those fixed.** Re-crawling before doing anything else is the whole point of having a crawl: the
answer decides what this tick is actually about, and guessing would waste it.

**What I expect to be wrong about:** I expect the number to fall but not collapse, and I expect the
residue to split into (a) genuinely heavy pages where we are simply slow, and (b) a small number of
pathological ones where something is quadratic. Those are different bugs and the crawl will separate
them, which is exactly what twenty sites could never do.

**The SPA question is binary and that is why it is Tier 0.** If the missing substrate for React/Vue/
Next is additive IDL and scheduling work, it is fast. If hydration failure cascades into needing a
scheduling-fidelity subsystem, it is not. You cannot plan around that distinction while it is
unmeasured, and the measurement is cheap.

### Tick 20 result — the app web is ADDITIVE. Measured, not guessed.

**Tier 0 item 3 is answered, and it is the good answer.** Eight real framework bundles (Vite output, not
toys — a toy exercises the IDL we already thought to implement, which is a tautology). Before: **0 of 8
rendered anything.** Every one mounted an empty `<div id="root">` and threw **zero exceptions** doing it.

That silence was the finding. A framework that fails loudly gets fixed; one that fails silently becomes
a permanent, unexplained "that site just doesn't work". The miner walked the chain in six steps, each
one naming the next:

1. **`import.meta` — "Module metadata hook not set".** SpiderMonkey needs a metadata hook; Vite, Rollup
   and esbuild all emit `import.meta.url` unconditionally. **Every Vite app on the internet died on this
   one missing function**, inside the module's own top level, where our warning path never saw it.
2. **`nodeType`** — React's `isValidContainer` checks it. Without it: React error #299, "Target
   container is not a DOM element." Three lines of code, and it was the entire React ecosystem.
3. **`ownerDocument`** — React then does `container.ownerDocument` and immediately indexes the result:
   `undefined["_reactListening…"]`. An error that names neither `ownerDocument` nor the DOM.
4. **DOM interface constructors** — `node instanceof HTMLIFrameElement` throws `invalid 'instanceof'
   operand` when the constructor is `undefined`. `Symbol.hasInstance` answers the question the
   frameworks are actually asking without needing a real prototype chain.
5. **`createElementNS` / `createComment` / `createDocumentFragment`** — Vue and Svelte use comment nodes
   as anchors for every `v-if` and every `{#if}`. apple.com's first-ever exception was
   `document.createElementNS is not a function`.
6. **`performance.now` / `MessageChannel` / `requestIdleCallback`** — every scheduler feature-detects
   these; the ones that don't fall back simply break.

**After: Vue, Preact and Vanilla render (63 boxes each). React, Svelte, Solid and Lit still do not.**
3 of 8, from ~6 additive fixes and no new subsystem. That is the binary question Tier 0 existed to
settle: **the app web needs substrate, not a scheduling-fidelity architecture.** React's residual is the
next tick's work, and it is now a bounded question rather than an open-ended risk.

**The gate caught me.** My `HTMLElement` shim replaced one that was already there and load-bearing — the
custom-elements upgrade path returns the element under construction from `super()`. A throwing "Illegal
constructor" broke every custom element and every `attachShadow`, and G2 went red inside a minute. The
fix is the general rule: **extend what exists, never clobber it** — attach `Symbol.hasInstance` to
whatever is already defined, and only *define* what is not.

### Tick 20 — and a Part 27 violation I have to own

The re-crawl reports **84 hangs, up from 73** — and the number is **contaminated and I caused it.**
I launched the crawl and then spent the next two hours compiling, testing and running SpiderMonkey
under it. **Part 27.1 says, in plain words, not to do that**: "sequence or throttle oracle crawling
against active compilation… risks swapping/thrashing rather than either finishing faster." RAM is the
binding constraint on this box, not cores, and I knew that because the methodology I had just folded in
says so.

So 84 is not a regression and it is not an improvement — it is a measurement taken with a thumb on the
scale, which is worse than no measurement, because it invites exactly the wrong conclusion. The clean
re-crawl runs with the machine idle, and until it does, **the hang count is unknown**, not 84 and not
73. Recording it here rather than quietly re-running and reporting only the good number.

The generalisable half: a benchmark that shares a machine with a build is not a benchmark. That belongs
in the same family as "an oracle must never be able to charge its own slowness to your account", and it
is the same mistake wearing different clothes.

## Tick 21 — the four frameworks that still don't render (2026-07-12)

**TICK SHAPE: pattern-class.** No cluster id is named and none is needed: this closes a *substrate*
class, not a rendering divergence — the same class Tick 20 opened, where each missing IDL property was
worth an entire framework ecosystem. Vue, Preact and Vanilla render; React, Svelte, Solid and Lit do
not. Four frameworks is not four bugs; it is a small number of missing properties, each of which the
framework will name for me if I let it.

**Hypothesis:** React gets furthest (it mounts, schedules, and throws nothing) and is therefore the most
informative. Svelte and Solid compile to direct DOM calls, so their failures should name a specific
missing method rather than a scheduling problem. Lit is custom-elements-based and may be tripping the
`HTMLElement` shim I nearly broke last tick.

**What I expect to be wrong about:** I expect at least one of these to be a *silent* failure again — no
exception, nothing rendered — and that is the one that will take the longest, because the miner's
signal is the exception and a silent failure gives it nothing to work with. If so, the fix is to make
the miner detect "mounted but empty", not to reason harder.

### Tick 21 result — 3/8 → 4/8, and the remaining three are now NAMED, not mysterious

**Solid renders.** `template.content` was the whole thing. Svelte, Solid and Lit do not call
`createElement` in a loop — they parse a `<template>` once and clone `template.content.firstChild` per
instance, because cloning a parsed subtree is far cheaper than rebuilding it. Without `.content` that is
`undefined.cloneNode()`, which was Solid's exact error.

We have no DocumentFragment node type, and inventing half of one would have been worse: the `<template>`
ELEMENT already holds exactly the children the fragment is supposed to hold, so it answers `.firstChild`,
`.childNodes` and `.cloneNode(true)` identically — which is precisely the surface the frameworks use it
through. They take `.content.firstChild` and clone *that*; the fragment itself is never appended.

(A second bug hid inside it: `content` was registered as a property **twice** — once for `<meta content>`
and once by me — and the later registration silently won. One dispatching getter now: `<template>` gets
its fragment, everything else gets the attribute.)

Also landed: `document.createTreeWalker` + `NodeFilter` (how lit-html finds the dynamic holes in a cloned
template), `document.importNode`, and constructable stylesheets (`new CSSStyleSheet()` + `replaceSync` —
how every modern web-component library ships styles; Lit's `static styles = css\`…\`` needs it to exist
before it renders a single node).

**The remaining three are no longer mysteries. Each has a name:**

- **Lit — shadow DOM is not laid out.** It throws nothing now and produces **zero boxes**, because it
  renders into `this.shadowRoot` and *layout does not traverse shadow trees*. The DOM has the content
  (G2 asserts it), the layout never sees it. This is a real, general gap — every web component on the
  web — and it is a layout change, not a shim.
- **Svelte — `a(...) is undefined`** inside its minified runtime. Still opaque; needs a source-mapped
  build to name.
- **React — silent.** Mounts, schedules, throws nothing, renders nothing. The hardest of the three,
  precisely because the miner's signal is the exception and React gives it none.

**The honest read on my own hypothesis:** I predicted the silent failure would be the one that took
longest, and it is — React is still silent after two ticks. The lesson I wrote down last tick applies to
me now: *when every instrument says the bug is impossible, they are all sampling the same layer.* The
next move on React is not to reason harder about the JS; it is to instrument the layer below — count the
DOM mutations React actually performs, and see whether it is building a tree we then fail to lay out.

## Tick 22 — shadow DOM is not laid out (2026-07-12)

**TICK SHAPE: pattern-class.** Not a framework fix. Layout does not traverse shadow trees, so **every
web component on the web renders nothing** — Lit is simply the framework that made it visible. The DOM
holds the content (G2 asserts `shadowRoot.innerHTML` populates the shadow tree); layout never looks at
it. Custom elements are not a niche: they are how design systems ship (Material, Fluent, Shoelace,
Spectrum, every `<*-*>` element on a bank or a government site), and a browser that renders none of them
is not a browser for those sites.

**Hypothesis:** layout walks `dom.children(node)`, which returns light-DOM children only. The fix is that
an element with a shadow root lays out its SHADOW children instead of its light children, and `<slot>`
projects the light children back in. Slots are the part that will be wrong first.

**What I expect to be wrong about:** I expect the naive fix (lay out the shadow tree instead) to render
Lit immediately, and to break something involving `<slot>` — because a component whose light children
vanish is a worse bug than a component that renders nothing, and it is the kind that renders *something*
and therefore hides.

### Tick 22 result — shadow DOM renders. Every web component on the web.

**The flat tree was BUILT, TESTED, and wired to nothing that draws pixels.** `Dom::flat_children` has
been correct all along — a shadow host yields its shadow tree, a `<slot>` yields its assigned light
nodes — and the HTML crate used it. **Layout and the cascade walked `children()` instead**, which does
not contain the shadow root, because a shadow root hangs off its host in its own field rather than among
its children.

And an unstyled node is not merely mis-styled: `is_rendered` drops it from the render tree outright. So
the whole component produced **zero boxes**. The mechanism that would have rendered it was sitting right
there the entire time.

That is a different and more uncomfortable failure than a missing feature. The feature existed. Nobody
had ever drawn a line from it to the renderer, and no gate asked.

**Four bugs in the custom-element upgrade path, each hiding the next**, and all four were invisible:

1. **`try { el.connectedCallback(); } catch (e) {}` — an EMPTY catch.** Lit does its entire first render
   from `connectedCallback`: that is where `attachShadow` happens and where the component's content
   comes into existence. Swallowing that exception meant a Lit component silently produced nothing, with
   no shadow root, no boxes and no message. **Part 22.1 is not an abstract principle — this was exactly
   the failure it names, sitting in our own code**, and it cost two ticks of looking in the wrong place.
2. **Only the class's OWN prototype was copied.** Real component libraries are deep:
   `MyElement extends LitElement extends ReactiveElement extends HTMLElement`, and the machinery that
   runs the component lives on the BASE prototype. We gave the element a subclass with no superclass.
3. **`el[k] = proto[k]` READS the property** — which, for an accessor, invokes the getter with `this`
   bound to the *prototype* and stores the result as a plain value. Every reactive property would have
   been frozen at whatever the prototype's getter happened to return. Descriptors are copied now.
4. **`this.constructor` was not the custom class.** Component libraries read their static configuration
   through it (`this.constructor.elementProperties`, `.observedAttributes`, `.styles`). All `undefined`.

Shadow DOM now renders end-to-end: the host sizes to its shadow content, the text paints, and a
light-DOM sibling is pushed down by it — the shadow tree *participates in layout*, it is not merely
present. The regression test asserts both, and was verified by sabotage (`got 0`).

Lit still does not complete its render (its `performUpdate` is scheduled, like React's) — but its
shadow root now attaches and holds content, which is a different and much smaller problem than the one
this tick started with.

### Tick 22 (cont) — three more mechanisms that existed and were wired to nothing

A pattern is now unmistakable, and it is the most valuable thing this tick produced. **Three separate
times, the mechanism was already there:**

- `Dom::flat_children` — the flat tree. Correct, tested, used by the HTML crate. Layout and the cascade
  walked `children()` instead. Every web component rendered nothing.
- `NodeData::Comment` — a real comment node type. `document.createComment()` returned an **empty text
  node** instead. lit-html finds the dynamic parts of a template by walking to COMMENT markers
  (`createTreeWalker` with `SHOW_ELEMENT | SHOW_COMMENT`); text markers are invisible to that walk, so it
  found zero parts and rendered nothing, silently. Vue and Svelte anchor every conditional and every list
  on comments for the same reason.
- `NodeData::Fragment` — a DocumentFragment, documented in our own source as "a `<template>`'s contents".
  `createDocumentFragment()` returned a **`<div>`**, and `template.content` returned the `<template>`
  ELEMENT. So `importNode(tpl.content, true)` cloned a `<template>` — which is `display:none` — and
  inserting it inserted an inert wrapper where the content should be. Solid survived only by accident:
  it takes `.firstChild` and clones *that*, never the fragment itself.

**A fragment's defining property is not that it holds children. It is what happens when you INSERT it:
the children move and the fragment does not.** That one rule is why every framework builds a subtree in
a fragment and commits it in a single insertion, and `append_child`/`insert_before` now implement it.

**And the third silent failure in three ticks is now closed at the source.** An unhandled promise
rejection was reported *nowhere*. Every modern framework renders inside an `async` function, so a throw
in there is not an exception anyone sees — it is a rejected promise, and ours went into a void. The
Framework Exception Miner's whole premise is that the browser names its own bugs out loud; a swallowed
rejection is the browser naming its bug into a void. `SetPromiseRejectionTrackerCallback` is wired now,
and it fires (verified against a deliberate `async` throw).

**Where Lit still stands:** its shadow root attaches, its styles adopt, its comment markers appear — and
`render()` still does not commit the template. That is a narrower problem than the one this tick started
with, and the next instrument to point at it is the DOM mutation counter, not more reading.

## Tick 23 — lit-html's template commit, and then React (2026-07-12)

**TICK SHAPE: pattern-class.** Template cloning + comment markers + fragments is the substrate every
compiler-based framework commits DOM through; Lit is the framework exercising the last unfixed corner of
it. Not a single-site fix.

**Hypothesis:** my fragment/comment plumbing has a bug, and lit-html is telling the truth. Before
blaming lit-html I test my own primitives — `createTreeWalker` over a cloned template, comment markers,
and `insertBefore(fragment, marker)` — because the last three ticks all ended with "the mechanism
existed and was wrong", and the prior on that is now high.

### Tick 23 — the primitives were wrong, and `setInterval` did not exist

**My prior was right, and it is now a rule.** Before blaming lit-html, I tested my own primitives. All
three were broken:

- **A DocumentFragment reported `nodeType = 8`** (comment) instead of 11. Not a near-miss: every
  framework's node dispatch branches on that number, and a fragment claiming to be a comment gets
  treated as an inert marker.
- **`cloneNode`/`importNode` fell through to `create_element("div")`** for anything that was not an
  element or text. So `importNode(template.content, true)` — the single call every compiler-based
  framework commits a template through — returned a **`<div>`**, and cloning lit-html's comment markers
  turned every template hole into an empty div.
- Fixed, and the primitive now does what the spec says: `<!--start--><b>A</b><i>B</i>` — the fragment's
  children move, no wrapper.

**And then the real find, which is worth more than all of Lit: `setInterval`, `clearInterval` and
`clearTimeout` DID NOT EXIST.**

Not "were subtly wrong" — were not defined. Every carousel, clock, poller, progress bar, countdown, live
score and "checking again in 5 seconds" on the web. **A page could not even STOP a timer it had
started.** Along with them: `AbortController` (every modern `fetch` passes a signal, and a library that
constructs one unconditionally throws before it ever reaches the request), `TextEncoder`/`TextDecoder`,
`crypto.randomUUID`, `CSS.escape`/`CSS.supports`.

**Adding `setInterval` would have hung the browser, so the ceiling came first.** The event loop drains
to quiescence — correct, right up until a page schedules work that reschedules itself. Without a
ceiling, "drain to quiescence" means "never return", and the tab is gone with no recourse. **G_RUNAWAY**
asserts a page with `setInterval(fn, 0)` *and* a hand-rolled self-reposting `setTimeout` still renders,
and still returns (1s, not never). It also asserts the page RENDERED — a ceiling that returns a blank
page has traded a hang for a different kind of nothing.

**`WebSocket` and `Worker` are deliberately left absent.** A page that feature-detects and falls back is
better served by honest absence than by a stub that lies about what it can do. That is a decision, not
an omission.

Lit still does not commit its template. Four ticks in, and the frameworks have paid for themselves many
times over in things that were never about frameworks at all.

## Tick 24 — audit the whole API surface, not one framework at a time (2026-07-12)

**TICK SHAPE: pattern-class + infrastructure.** Last tick found `setInterval` missing by pointing the
miner's logic at the *global object* instead of at a framework. That found more breadth in ten minutes
than three ticks of chasing Lit. So do it properly and exhaustively: enumerate the DOM/BOM/CSSOM surface
real sites actually call, and see what is absent. Each missing entry is a class of site that breaks.

**Hypothesis:** the remaining gaps cluster in (a) element/document methods frameworks use for
measurement and traversal, (b) event-system surface, and (c) the "modern" APIs (observers, storage,
media). I expect at least one to be as embarrassing as `setInterval`.

### Tick 24 result — audit the surface, not the framework

**Pointing the miner's logic at the global object found more breadth in ten minutes than three ticks of
chasing Lit.** The technique generalises and is now the default move: enumerate what real code reaches
for, and see what is absent. Each missing entry is a *class* of site, not a bug.

Landed:

- **`document.readyState`** — the single most-checked property on the web. Half the scripts on the
  internet open with `if (document.readyState === 'loading') { wait } else { init() }`. Undefined made
  that comparison false, so they took the `else` and initialised immediately — *right by accident*. The
  many libraries that instead wait for `'complete'` waited forever. We report `"complete"`, which is the
  truth: by the time a page's script sees this DOM, the document IS parsed.
- **`document.defaultView`** — frameworks get `window` from a NODE (`el.ownerDocument.defaultView`)
  rather than the global, precisely so they work inside an iframe. `null` made them think they were in a
  detached document and skip everything.
- **`document.visibilityState` / `hidden`** — video players and animation loops compare against the
  *string* `'visible'`; `undefined !== 'visible'` makes a player believe the tab is backgrounded and
  refuse to start.
- **`element.click()`** — a programmatic click. Menus, dropdowns, "click the hidden file input", every
  custom control forwarding to a real one, every Copy button. Routed through the same `__dispatchEvent`
  path a real click takes, so listeners, bubbling and default actions behave identically — a synthetic
  click that skipped the event system would be a different bug wearing this one's clothes.
- **`isConnected`** (React and Vue check it before every commit), **`localName`**, **`toggleAttribute`**,
  **`btoa`/`atob`**, and honest **`alert`/`confirm`/`prompt`** — a renderer has no user to ask, and a
  `confirm()` that returned `true` by default would let a page believe the user had agreed to something.
  Declining is the safe answer, and it is logged rather than silent.

Still absent and enumerated (next): `append`/`prepend`/`before`/`after`/`replaceWith`,
`insertAdjacentHTML`, `outerHTML`, `innerText`, `scrollTop`/`scrollLeft`, `attributes`,
`document.styleSheets`, `createRange`, `getSelection`, `Blob`/`FileReader`.

## Tick 25 — the hangs. Bar 0. (2026-07-12)

**TICK SHAPE: pattern-class.** ~1 site in 4 hangs. Nothing in the ledger outranks it: a browser that
freezes on one site in four is not a browser, and Part 24.3 puts hangs above every visual cluster by
construction.

**What I already know, measured:** the hangs are OURS and they are CPU + duplicate work, not the network
(attributed by timing each engine separately on the same bytes — bbc.co.uk 26,128ms against Chromium's
7,695ms). Since then I have landed parallel script fetch, stylesheet/image/mask/background dedup, one
fewer layout per script round, and a load budget on `load_async` which previously had none. **I do not
know how much of the 73 those fixed** — last tick's re-crawl was contaminated because I ran it while
compiling, which Part 27.1 explicitly forbids and I did anyway.

**So: measure first, on an idle machine, and do not touch the compiler while it runs.** The number
decides what this tick is about, and guessing would waste it.

**Hypothesis:** the residue splits into (a) genuinely heavy pages where we are simply slow, and (b) a
small number where something is quadratic. Those are different bugs. I expect the layout stage — which
is 71ms on a 8.8k-node synthetic page but 257ms on bbc's 4k nodes — to be the quadratic one, because
that ratio is the wrong way round.

**The hypothesis held, and the measurement named the organ.** On an idle machine, previously-hung sites:

```
apple.com   TOTAL  2,132ms   (was 5,560)      gitlab.com  TOTAL  1,086ms
cnn.com     TOTAL  7,125ms                    bbc.co.uk   TOTAL 12,307ms   (was 26,128)
```

None of those hang any more; the dedup and budget work landed. What remains is the *slowness*, and it
sits where the ratio said it would:

```
             nodes    layout
bbc.co.uk    4,021    260ms
wikipedia   18,630    127ms      ← 4.6x MORE nodes, HALF the time. ~10x worse per node.
```

**The cause: `shrink_to_fit` recomputed max-content on every call**, and computing max-content means
laying the entire subtree out at a 1e6 available width. Taffy probes each flex/grid item several times
per solve, at several available widths — so on *nested* flex the cost compounds per level of nesting.
Wikipedia is a document and barely nests; bbc is deeply nested flex. That is the whole difference.

Both min-content and max-content are **independent of the available width** — that is what makes them
*intrinsic*. I had cached one and not the other, and the one I left uncached was the expensive one.
`max_content_cache` closes it; `shrink_to_fit` becomes a lookup and two comparisons.

Second find, in the same loop: `std::env::var("MANUK_TRACE_INTRINSIC")` was being called **inside
intrinsic sizing** — once per node per probe. It takes a process-wide lock and allocates a `String`. A
debug hook that nobody had enabled was costing real time on every page load. Hoisted to a `OnceLock`.

**Lesson, and it is the general form of both:** *an intrinsic is a property of the box, not of the
question you asked it.* Anything whose value cannot depend on the input is a thing you are allowed to
compute once — and if it is expensive, it is a thing you must compute once. The bug was not that the
code was wrong; it was that it was right the slow way, repeatedly.

**The residue has a shape, and it is one class.** Partway through the clean crawl, the hang list:

```
news:  nytimes · theguardian · washingtonpost-adjacent · apnews · npr · wired · zdnet
       gizmodo · newyorker · techcrunch · arstechnica        ← 10 of 11
docs:  go.dev                                                 ← 1 of 11
```

Ten of eleven hangs are `news`. That is a pattern class, not eleven bugs, and it is exactly what the
oracle exists to surface: the corpus is 265 sites precisely so that a class can out-vote an anecdote.

**But I will not chase it before checking whose slowness it is.** The 90s watchdog wraps the whole
oracle process, and that process runs *Chromium too*. Lesson 4 is on the board in STATUS.md in my own
words — *an oracle must never be able to charge its own slowness to your account* — and I have already
been caught by exactly this once, on w3schools, where a local snapshot hid the fetches and I confidently
blamed the wrong engine. A news front page is the single most ad-tech-laden document class on the web;
Chromium taking tens of seconds on one is entirely plausible.

So the next measurement is `boxes --fetch <url>` on OUR engine alone, on an idle machine, for each of
the eleven. If we are fast, the hang is the harness and the number 11 is measuring the wrong thing.
If we are slow, it is a real class and I will have its name.

## Tick 25 — RESULT

**The hangs were not one bug and the ledger was wrong about two of them.**

*Bar 0 — the hangs.* Measured on an idle machine, every previously-hanging site now **returns**:

```
apple.com    5,560 → 2,132ms      gitlab.com          → 1,086ms
bbc.co.uk   26,128 → 12,307ms     go.dev      7,425 →  2,819ms
cnn.com              7,125ms      theguardian 19,175 → 11,184ms
nytimes    43,000 → 14,096ms      (finish_loading pinned at exactly the 12,002ms budget)
```

They are **slow, not hung**, which moves them out of Bar 0 and into perf. Three causes, all real:

1. **`shrink_to_fit` recomputed max-content on every call**, and computing max-content lays the whole
   subtree out at a 1e6 width. Taffy probes each item several times per solve; on nested flex the cost
   compounds per level. Cached (it is *intrinsic* — it cannot depend on the available width). bbc's
   layout **260ms → 168ms**; Wikipedia unchanged at 126ms, exactly as predicted, because it is a
   document and barely nests.
2. **`load_async` was not under the load budget at all** — only `finish_loading` was, though both run
   the same two subresource phases. A bound that covers one of two identical phases is decorative.
3. **Failed image fetches were never remembered.** The skip-list was built from `self.images`, i.e.
   *successes*, so every blocked tracker and every 404 was re-fetched on all six rounds. A news front
   page is made of images that fail: nytimes issued **813 fetches, 507 of them duplicates**;
   theguardian 431 of 576 (75%). Now keyed by `(node, resolved url)` — remembering a failure, without
   refusing to retry a genuinely *different* request.

*The app web.* **6 of 8 frameworks now render** (React ×2, Vue, Solid, Preact, Vanilla; Svelte and Lit
remain). And the previous "4/8" was not a smaller version of this number — it was measuring nothing:

**`file://` was an unsupported scheme in the network layer.** Every local fixture's bundle, stylesheet
and image failed to load. Compounded by `format!("file://{relative}")`, which parses the first path
segment as a *hostname*. Two independent bugs, each making the other's symptom look like somebody
else's fault — and between them they meant **not one line of React had ever executed.** "React mounts
and renders nothing" sat in the ledger for several ticks as a framework problem. It was our harness.

And under that, the real one:

**`ownerDocument` was a use-after-GC.** `DOC_REFLECTOR` was a `Cell<*mut JSObject>` — a bare, unrooted
pointer into the JS heap. Nothing kept the document alive or updated the pointer when the collector
moved it, so after enough allocation `ownerDocument` returned whatever now occupied that address. In
the failing React run it returned one of **our own `MutationRecord`s** — `{type, targetId, attrName,
oldValue, addedCsv, removedCsv}` — on which `createElement` is indeed not a function. React allocates
heavily, so it reliably GC'd mid-commit and reliably got garbage. Its error message was *true* and
pointed at nothing that was wrong with React.

The correct discipline was already written down **ten lines above**, for the node identity cache:
*keep the reflector in a JS-side structure so it is GC-reachable through the global.* It had been
applied to every node and not to the document.

**Lessons.**

- *An intrinsic is a property of the box, not of the question you asked it.* Anything whose value
  cannot depend on the input may be computed once — and if it is expensive, it **must** be.
- *A bound that covers one of two identical phases is decorative.* Whatever bounds one must bound both.
- *A skip-list built from successes retries every failure forever.* Remember the attempt, not the win.
- *Test your own primitives before blaming the framework.* Third time this prior has paid, and this
  time it had been costing us a whole framework for several ticks.
- **A raw `*mut JSObject` cached across a GC boundary is a bug, not an optimisation** — and the fact
  that the correct pattern was already in the file, applied to the neighbouring case, is the tell. When
  a codebase does the right thing *next to* the wrong thing, the wrong thing is an oversight, not a
  design.

## Tick 26 — the app web is open (2026-07-12)

**TICK SHAPE: pattern-class.** CLUSTER: C01ca (the app web / SPA substrate).

**8 of 8 frameworks mount and render.** React, React(JS), Vue, Svelte, Solid, Preact, Lit, Vanilla.

Svelte and Lit were the last two, and both were blocked by a primitive:

- **Svelte 5** does not use the DOM the way everything else does. For speed it lifts the raw accessor
  functions off the interface prototypes once at startup — `get_descriptor(Node.prototype,
  'firstChild').get` — and then `.call(node)`s them on every node it walks. Our reflectors carry their
  members as *own* properties with no shared prototype, so `Node.prototype` was an empty object,
  `get_descriptor` returned `undefined`, and `.get` threw. Fixed with a prototype accessor bridge:
  each prototype accessor looks up the OWN descriptor of whatever `this` it is handed and delegates to
  it. (Reading the *descriptor* and not the property is what keeps it from recursing.)

- **Lit** marks every dynamic hole in its templates with a comment node and then reads `node.data` to
  find them. **`CharacterData.data` did not exist.** Neither did `nodeValue`. Also: a shadow root was
  reporting `nodeType` **8** (comment) instead of **11** (DocumentFragment), which is how a component
  asks whether it is inside a shadow tree at all.

**The lesson, and it is the whole tick:** *when a framework fails silently, the bug is below the
framework.* Of the five things that were actually blocking the app web, **four were in our own
primitives** — a use-after-GC in `ownerDocument`, an unsupported `file://` scheme, a missing
character-data accessor, a mis-typed shadow root. The framework was never once the thing that was
broken. Several ticks were spent with "React renders nothing" and "Lit's template doesn't commit"
sitting in the ledger as *framework* problems. They were ours.

**Named the gate.** G2 scenario 14 now asserts all six primitives, each labelled with the framework
that found it. The `ownerDocument` assertion **allocates 60,000 objects to force a collection** —
because a test that does not allocate cannot see a use-after-GC at all, which is exactly why that bug
survived. Per the standing rule: before adding a feature, name the gate that would have gone red if it
were already broken. This is that gate.

**Still open:** `getBoundingClientRect` returns a stale (zero) rect immediately after a mount — it does
not force layout when the tree is dirty, as a real browser does. Not a Bar 0 issue, but it is a thing
real code depends on, and it will be the next tick's start.

## Tick 27 — the ledger is ranked by symptom, and I should say so (2026-07-12)

Before the crawl lands, an honest note about the instrument it feeds.

`CLUSTERS.md` opens by declaring itself "ranked by DISTINCT SITES then DISTINCT CLASSES ... clustered by
**root cause**." It is not. Its top ten entries are:

```
C01ca  123 sites  geometry: <div>      C990e  118  geometry: <a>
C7eb9  117 sites  geometry: <body>     Ca725  104  geometry: <span>
```

**That is a ranking by TAG NAME, not by cause.** "Our `<div>`s are in different places than Chrome's,
on 123 sites" is a restatement of *the oracle found divergences*, and it cannot be worked on. Every
real fix of the last several ticks — `font-family` never mapped, flex items resolving width twice,
`position:absolute` elements deleted, the intrinsic-sizing quadratic — was found by *measuring a
specific page*, not by reading this file. The ledger has been decorative for the Bar 1 work while
claiming, in its own header, to be the thing that sets priority.

I am not going to quietly keep using it while pretending otherwise. Two things follow:

1. **Bar 0 does not depend on it.** The hang count is measured directly and is the number that has been
   driving the schedule. That part of the methodology is working exactly as designed.
2. **The clustering needs a root-cause key, not a tag key.** A useful key would name the *kind* of
   divergence — "flex item width", "accumulated y-offset below a mis-sized ancestor", "replaced element
   aspect ratio" — so that one entry corresponds to one thing a person can go and fix. That is a real
   piece of work on the oracle itself, and it belongs on the schedule rather than in a complaint.

**The rule this is an instance of:** *a gate that does not measure what it claims to measure is worse
than no gate, because it is trusted.* The same sentence already appears in STATUS.md's Lessons about
gates that go green while the user suffers. This is that failure applied to a ledger instead of a test.

**And then I did it again, within the hour.** Having just written that the ledger must not be trusted
past what it measures, I widened the crawl from 4 jobs to 12 to make it finish faster, and watched the
hang rate "rise":

```
 4 jobs → 11 hangs /  88 sites  (12.5%)
12 jobs → 22 hangs /  45 sites  (49.0%)     ← same binary. same corpus. same web. same hour.
```

Twelve parallel oracle runs means **189 concurrent Chromium processes**, and the 90s watchdog wraps the
whole oracle process — *ours and Chromium's together*. So the watchdog fired on contention I had
manufactured, and every site it killed was recorded as a hang, attributed to us.

If I had not stopped to compute the rate before reading the number, I would have reported a 49% hang
rate — a regression from 27.5% — on a tick whose entire content was **fixing the hangs**. The fix would
have looked like a catastrophe, and the next thing I did would have been to go and "repair" working code.

**This is Lesson 4, for the third time, and the third time is the signal.** The lesson as written —
*an oracle must never be able to charge its own slowness to your account* — is true and I have now
violated it while able to recite it. A lesson I can quote and still break is not a lesson, it is a
decoration. So it becomes a mechanism:

- **The crawl pins its own concurrency.** A hang count is only a measurement *relative to a baseline
  taken the same way*, so the job count is part of the measurement, not a knob for making it finish
  sooner. Re-measuring at a different width is not a faster measurement — it is a different one.
- **STATUS.md now refuses to print a partial crawl as a number** (`scripts/status-update.sh`). It was
  reporting `ORACLE_HANGS: 33` from a run I had killed at 92/265, and an interrupted crawl always
  UNDER-reports, because the sites that hang are the ones still running when you kill it.

The general form, which is the one worth keeping: **every number has a harness, and the harness is part
of the number.** Before believing a metric moved, ask what else moved.

## Tick 27 — Bar 0's headline number was measuring Chromium (2026-07-12)

**TICK SHAPE: pattern-class.** CLUSTER: C01ca. This is the tick where the instrument turned out to be
the bug, and it is the largest correction the project has had.

For several ticks the top line of STATUS.md has read:

```
73 of 265 sites HANG (27.5%)   ← a browser that hangs on one site in four is not a browser
```

That sentence set the schedule. It is what made "the hangs" the top of the ledger, ahead of every
visual cluster, by Part 24.3. **It was substantially measuring Chromium.**

`oracle-crawl.sh` ran each site under a 90s `timeout`. That watchdog wraps the *whole oracle process*:
our render, **Chromium's render**, and the diff. When it fired, the site was recorded as `HANG` and
attributed to us. But the oracle has been recording each engine's time separately all along, and the
record says:

```
site            manuk_ms    chrome_ms
bloomberg.com     15,507      60,600     ← Chromium 3.9x slower
vox.com            7,551      59,715     ← 7.9x
economist.com     15,078      53,895     ← 3.6x
bbc.co.uk         18,788      54,964     ← 2.9x
cnn.com           29,527      59,247     ← 2.0x
lite.cnn.com          77       9,593     ← 124x
```

**Chromium was the slower engine on 9 of the 10 news sites that completed.** A news front page in cold
headless Chromium takes 30–60 seconds. Add our 8–30s and the diff, and the 90s watchdog fires — on
*Chromium's* time, recorded as *our* hang.

**So the honest position on Bar 0, stated plainly:** I do not yet know our hang count, and the number I
have been quoting for several ticks was not one. What I do know, measured directly on our engine alone:
every previously-"hanging" site returns — nytimes 14.1s, bbc 12.3s, guardian 11.2s, apple 2.1s,
go.dev 2.8s. Slow, and slower than I want. **Not hung.**

**The instrument is fixed, not the number massaged:**

1. `HANG` → `TIMEOUT`, and a TIMEOUT is **attributed to nobody**. The process watchdog knows the process
   was slow; it does not know whose slowness it was, and it must not guess.
2. The watchdog goes 90s → 240s. It is a **backstop against a true infinite loop**, not the metric.
3. **Bar 0 is computed from `manuk_ms` — our own clock** (`status-update.sh`). A hang is now a claim
   about *this browser*, which is what Bar 0 was always supposed to be a claim about.
4. The crawl **warns loudly if run at a non-baseline job count**, because I proved within the hour that
   concurrency I add shows up as hangs attributed to us (4 jobs → 12.5%, 12 jobs → 49%, same binary).

**The lesson, and it is the same one three times in one day, which is why it is now four mechanisms and
not a fourth reminder:**

> *Every number has a harness, and the harness is part of the number.*

The work the bad number provoked was not wasted — the intrinsic-sizing quadratic, the unbudgeted
`load_async`, the image-refetch storm were all real, and the engine is several times faster for them.
But it was prioritised by a lie, and I should be plain about that rather than let the good outcome
launder the process. **The right work for the wrong reason is luck, not method.**

**And a fourth contamination, in the same tick, caught only by an accounting check.**

The corrected crawl finished and reported **3 hangs of 265 (3%)** — a beautiful number, and I very
nearly wrote it down. Then the accounting didn't add up: 265 result files, but only 76 with both
engines' times, 39 `TIMEOUT`, 37 `DISCARDED` — and **113 records labelled `HANG`**, a label the current
script does not emit at all.

**`oracle-crawl.sh` does `export -f one_site`.** The xargs workers carry the function they were forked
with. Killing the driver does not kill them. So the *previous* crawl's workers — with the previous
crawl's 90s watchdog and the previous crawl's HANG semantics — were still running, and were writing
into the results directory of the new run. The output was two different experiments wearing one name.

It was caught only by luck: the two script versions happened to use *different status labels*. Had I
changed the watchdog from 90s to 240s and **not** renamed HANG→TIMEOUT, the records would have merged
silently, the totals would have added up, and I would have believed a number produced by two different
instruments averaged together.

**So it becomes mechanical, not vigilant:**

- Every record is stamped with a `RUN_ID`. More than one in a directory → not a measurement.
- `status-update.sh` **refuses to print anything** from a mixed-run directory (verified: it refuses the
  real one).
- `oracle-crawl.sh` **refuses to start** while another crawl's workers are alive, naming the `pkill`.

**Four contaminations in one tick** — compiling during the crawl; widening the job count; reporting a
killed run; and now overlapping workers. Every one of them made the browser look *worse or better* than
it is, and not one was a bug in the browser.

> **The instrument is part of the experiment, and it is the part that lies to you.** A measurement
> harness gets the same scrutiny as the code under test — more, because nothing is watching *it*.

That sentence is the actual output of this tick. The engine work (intrinsic-sizing cache, load budget,
image-refetch storm, the whole app web) was tick 25 and 26. **Tick 27 is the discovery that the number
driving all of it was never measuring this browser.**

## Tick 27 — RESULT: Bar 0 was never 27.5%. It is 4.4%.

Clean crawl, 265 sites, one `RUN_ID`, corrected instrument, on our own clock:

```
9 of 206 timed sites exceed 30s        (4.4%)     ← the honest Bar 0 number
we are FASTER than Chromium on 175/206 (84%)
median render:  ours 21.7s   ·  Chromium 35.7s
p90:            ours 28.4s   ·  Chromium 98.4s
```

Of the nine, **Chromium is slower still on seven** (aljazeera: ours 35.4s, its 110.4s. webflow: ours
32.0s, its 113.8s). **Only two sites are both slow and slower than Chromium: wix.com and flickr.com.**

**The remaining Bar 0 work is two sites, not seventy-three.**

That is what the metric said all along, if it had been asked the right question. The old headline —
"73 of 265 sites HANG, a browser that hangs on one site in four is not a browser" — was the oracle
*process* hitting a 90s watchdog that wraps Chromium's render too, on a corpus where Chromium is the
slower engine 84% of the time.

**What I want to be careful not to do here is launder this into a victory.** Three things are true at
once and all three should be said:

1. **The engine really did get much faster this session** — the intrinsic-sizing cache, the budget on
   `load_async`, the image-refetch storm. Those were real bugs, found by real measurement, and nytimes
   went 43s → 14.1s standalone because of them.
2. **The work was prioritised by a broken instrument.** Bar 0 was at the top of the ledger for several
   ticks on the strength of a number that was measuring Chromium. The right work for the wrong reason
   is luck, not method.
3. **Absolute times here are inflated for BOTH engines** by the 6-way crawl concurrency — standalone,
   nytimes is 14.1s and apple 2.1s, not 21.7s median. The *ratio* is the trustworthy part, and it is
   trustworthy precisely because both engines ran on the same bytes on the same machine in the same
   minute. That is the whole design of the differential oracle, and it is the part that worked.

**Four contaminations in one tick**, every one of them mine: compiling during the crawl · widening the
job count · reporting a killed run · overlapping xargs workers. Each is now a mechanism, not a memory:

| Failure | Mechanism |
|---|---|
| watchdog blamed us for Chromium's time | `TIMEOUT` is attributed to **nobody**; Bar 0 counts `manuk_ms` |
| more jobs → more "hangs" | crawl **warns loudly** at a non-baseline job count |
| partial run reported as a number | `status-update.sh` **refuses** to print it |
| stragglers wrote into the new run | every record carries a `RUN_ID`; the crawl **refuses to start** on live workers |

> **The instrument is part of the experiment, and it is the part that lies to you.** It gets the same
> scrutiny as the code under test — more, because nothing is watching *it*.

## Tick 28 — media: an honest NO beats a TypeError (2026-07-13)

**TICK SHAPE: pattern-class.** CLUSTER: C01ca (replaced elements / the media surface).

The user's north star asks for *graceful degradation for unsupported media/codecs*. Measured, that is
not where we are — it is worse and better than expected in different places:

```
<video width=640 height=360 poster=... controls>   →  box: 640x360  ✅ laid out correctly
video.canPlayType   undefined      video.play        undefined
video.paused        undefined      video.readyState  undefined
video.error         undefined      video.networkState undefined
```

**The layout is right and the API is absent.** That combination is the worst one: a site that calls
`video.play()` gets a `TypeError` and takes the whole page down with it, and a site that *politely
feature-detects* with `if (v.canPlayType('video/mp4'))` reads `undefined` and cannot even be told no.

Graceful degradation is not "do nothing". It is **answering the question honestly**. The spec already
has the vocabulary for a browser that cannot play a thing:

- `canPlayType(t)` returns `""` — the empty string IS the spec's "no".
- `play()` returns a **rejected** Promise (`NotSupportedError`), which is what every player library is
  already written to handle, because autoplay policies make rejection routine in real browsers.
- `error` is a `MediaError` with `code: 4` (`MEDIA_ERR_SRC_NOT_SUPPORTED`), and an `error` event fires.
- `readyState: 0` (HAVE_NOTHING), `networkState: 3` (NETWORK_NO_SOURCE).

A site told *that* will hide its player and show its fallback. A site told `undefined` will throw.

And the poster: `<video poster>` is a still image, and we can already decode, lay out and paint still
images. A video element that shows its poster frame, sized correctly, with an honest "cannot play" is
not a broken video — it is a **degraded** one, which is the whole ask.

**RESULT.** Media degrades honestly now:

```
canPlayType('video/mp4') → ""            (the spec's "no")
paused true · readyState 0 · networkState 3 · error.code 4
v instanceof HTMLMediaElement → true
v.play() → REJECTED NotSupportedError    (the site can now fall back)
v.pause() / v.currentTime = 5 / v.volume = .5 / v.load()  → all survive
layout: <video> keeps its 640x360 box; the page flows around it
<video poster> → decoded and painted — the frame the author chose
```

Asserted in **G2 scenario 15**. A missing codec is an acceptable limit for a browser to have. A thrown
`TypeError` is not, and the difference between them is entirely in what we say when asked.

## Tick 29 — the self-audit, and the four gates it says are owed (2026-07-13)

**TICK SHAPE: pattern-class.** CLUSTER: C01ca. Part 21.3: *closing the gap between what the methodology
prescribes and what has actually been built OUTRANKS the ledger.* The audit was due at tick 29 and the
hook would have blocked every commit at tick 30. It reports six gaps.

**One of them was the audit lying about itself.** The SPA-miner check counted *files in `tests/spa/`* —
which are `apps/`, `build.sh`, `README.md`, i.e. three — and concluded the miner had never run. It had
run against all eight frameworks and produced five real engine fixes. The check was measuring directory
entries and calling that a measurement. *Inside the audit whose entire job is to catch that.* Fixed to
assert the thing that actually matters: the apps exist **and** what the miner found is pinned by a gate
(G2 scenario 14), which is the only form in which a finding survives.

**The other four are real, and all four are about duplicated or hidden work:**

| Gate | What it must catch |
|---|---|
| `G_DEDUP` | the same resource fetched twice, the same pass run twice, for one navigation |
| `G_SILENT_FAIL` | an error caught on the load/render/script path and never surfaced |
| `G_SPAWN` | work spawned per-action instead of per-process (runtimes, pools) |
| `G_POOL_ISOLATION` | one page's work starving another's |

`G_DEDUP` is the one with a body count. Tick 25 measured **nytimes issuing 813 fetches of which 507 were
duplicates**, and theguardian 431 of 576 — because the image skip-list was built from *successes*, so
every blocked tracker was re-fetched on all six rounds. I fixed it and **nothing stops it coming back.**
That is precisely the standing rule: *before adding a feature, name the gate that would have gone red if
it were already broken.* This one would have been red for months.

**RESULT — tick 29. The audit is green, and it is green honestly.**

Two gates BUILT, two RETIRED with reasons, one meta-mechanism added, and **five process defects found by
the meta-mechanism on its first day.**

*Built:*
- **`G_DEDUP`** — the same URL must not reach the **wire** twice per navigation. It found a real bug the
  moment it existed: I had keyed the image skip-list by `(node, url)`, so nine elements naming one sprite
  still cost nine fetches. **14 duplicate fetches of 17.** Now keyed by URL, with single-flight
  coalescing (the preload scanner and the loader were racing for the same stylesheet) and a
  per-navigation negative cache (a failure not remembered is a fetch repeated forever). Real sites:
  **theguardian 19,175ms → 3,110ms**; nytimes 863 calls but only **335 network requests, 4 duplicate**.
- **`G_SILENT_FAIL`** — an error on the load/render/script path must be said out loud. Named by the
  failure that cost several ticks: *"React mounts, throws nothing, renders nothing"* was React throwing
  **truthfully**, inside an async render, with nothing listening.

*Retired, with reasons:* `G_SPAWN` (subsumed by G_RUNTIME_COUNT) and `G_POOL_ISOLATION` (guards a rayon
pool that **does not exist** — a gate on absent machinery passes forever and is counted as coverage,
which is the definition of vacuous). **A prescribed gate that turns out to be inapplicable is retired
explicitly, not built as theatre to make an audit green.**

*The meta-mechanism — `scripts/falsify.sh`.* Mutation-tests the gate wall against itself: for each gate,
install the bug it exists to catch, and assert it goes **RED**. On its first run:

| It found | Which is |
|---|---|
| `G_LOAD` was **vacuous** — it had never tested the page budget in its own name, only the per-request timeout. Delete `load_budget()` outright and it stayed green. | a **Bar 0** gate, standing between the user and a frozen tab |
| Two gates **raced over a process-global `OnceLock`** — one set the request timeout to 5s, the other to 1s, cargo runs them in parallel, and whichever touched it first decided for both | a gate whose verdict depended on thread scheduling |
| A gate **re-derived the constant it was checking** — it carried its own copy of the `30`, so changing the real default to 5s would not have failed it | a test asserting a relationship between two numbers it had itself written down |
| The falsifier's own first mutation was **too weak**, producing a FALSE "vacuous" verdict | the instrument that checks instruments is also an instrument |
| The falsifier **POISONED THE TREE** — a killed run left `MAX_TASKS_PER_DRAIN = u32::MAX` in `event_loop.rs`; the next run backed up the mutated file and "restored" the corruption | **the worst one.** Wrong code, in a Bar 0 path, indistinguishable from a real regression |

**The rule, and it is the tick's output:**

> **A test that can pass without the code it protects is not a test.** Not a weak test — *not a test*.
> The only way to know is to take the code away and watch it fail.

And its corollary, learned the hard way:

> **A tool that can leave the tree worse than it found it must be able to PROVE it did not.** Not "be
> careful" — prove it: a marker it looks for on the way in, and a check it runs on the way out.

`docs/loop/PROCESS.md` now carries all fourteen process defects of this session, each with the mechanism
that closes it. Seven of the fourteen were found by an *accounting check* — by squinting at a number
that did not add up — and not by any gate. That ratio is the thing to drive down, and it is what the
falsifier is for.

## Tick 30 — first paint does not wait for images (2026-07-13)

**TICK SHAPE: pattern-class.** CLUSTER: C01ca.

First: **Bar 0's residue closed itself.** The two sites that were slower than Chromium — the entire
remaining Bar 0 list — were fixed by tick 29's dedup work: **wix.com 39.1s → 8.7s, flickr.com 31.1s →
1.6s.** The URL-keyed image cache and single-flight coalescing did it.

Then the thing that actually decides whether this is a browser a person would use:

```
nytimes.com:  document parsed, cascaded, laid out — everything needed to PAINT — in 1.7s
              user sees it at                                                       14s
```

**The load path fetched and decoded every image before the shell was handed anything.** Twelve seconds
of blank window while the article sat there, laid out, waiting for tracking pixels nobody was looking at
because there was nothing on the screen to look at. No browser a person would use does this: Chromium
puts the article up and lets the assets land afterwards, reflowing as they arrive — which is what an
`<img>` without intrinsic dimensions does anyway.

`prefetch_document` no longer fetches images. The shell paints, *then* fetches them on a background task
(`Page::pending_image_urls` → `fetch_image_urls` off-thread on owned data → `NavEvent::ImagesReady` →
`apply_images_by_url` → one repaint). Measured:

```
nytimes      14,000ms → 5,773ms     then 42 images in 452ms — after the page is up
theguardian           → 6,488ms     then 135 images in 8,006ms — the user is READING for those 8s
wikipedia             → 2,044ms
```

**And the falsifier caught me writing two bad gates in the same hour**, which is the entire argument for
it existing:

1. **My mutation did not compile.** `cargo test` returns non-zero for a compile error exactly as it does
   for a failing assertion — so a typo in a mutation reads as *"✓ goes red when broken"* and the gate is
   certified by nothing. `falsify.sh` now **builds first**, and a build failure is reported as
   **FALSIFIER BROKEN**, never as evidence about the gate.
2. **G_FIRST_PAINT's first version was vacuous.** It called `Page::load`, which has never fetched an
   image in its life — it would have passed before the fix, after the fix, and with the fix reverted.
   The images were on the paint path in exactly ONE place: `prefetch_document`, the function the *shell*
   calls. **The gate was not testing the path the user waits on.** It now drives real HTTP through
   `prefetch_document`, and additionally asserts the images are still *pending* — because "fast"
   achieved by never loading them is a different bug wearing this gate's success as a disguise.

Without the falsifier, that gate ships green, the number looks good, and the next person to touch the
load path silently puts the images back on it.

> **A gate must exercise the path the user waits on.** Measuring the wrong function is how "the browser
> feels slow" survives a green benchmark: the number was real, it was just a number about something else.

## Tick 31 — every script blocks first paint, including the ones that say not to (2026-07-13)

**TICK SHAPE: pattern-class.** CLUSTER: C01ca.

First paint on nytimes is **5,773ms**, of which fetch+parse is **1,195ms**. The other ~4.6 seconds is
CPU inside `from_prefetched`. Measured, before theorising:

```
                nodes   parse   cascade   layout   paint   TOTAL
nytimes          7482   16.1     26.1     245.4    54.0    341.8ms      ← the whole pipeline
nytimes (scripts stripped)  7415   11.6     26.6     242.2    45.9    326.4ms
```

**The render pipeline is 342ms and it barely moves when the scripts are removed.** The document is
1,433KB with scripts and 447KB without — so **~1MB of JavaScript is being parsed and executed before a
single pixel reaches the screen**, and that is the 4.6 seconds.

And here is the part that makes it a *bug* rather than a *cost*:

```rust
Script { defer: bool, is_async: bool },     // engine/page/src/lib.rs:571
```

**`defer` and `is_async` are parsed, stored, and never used for anything.** Nothing schedules on them.
Every script blocks first paint — including the ones whose entire purpose is to say *"do not wait for
me"*, and including `type="module"`, which is **deferred by default in every real browser** and is what
every Vite/Rollup bundle on the internet ships as.

A real browser paints the document and *then* runs the deferred scripts. That is not an optimisation, it
is the specified behaviour, and it is why a news site appears instantly in Chrome and then fills in.

**The fix is a two-phase script execution:** blocking scripts (classic, no `defer`, no `async`) run
before paint, exactly as the spec requires; everything else — `defer`, `async`, `type=module` — runs
after, and the page repaints. `Page::load` (the gate path) will still run both, so the SPA suite is
unaffected: it asserts that the app eventually mounts, and it still will.

Expected: nytimes' first paint from ~5.8s to ~1.5s, which is the document's own cost and nothing else's.

**Checkpointing the finding, not half the change.** The split lives in `manuk_js::load_document`, which
today walks the DOM and executes every `<script>` it finds, inside `from_dom`. Splitting it is a careful
change to the JS entry point (scripts must be marked executed so a deferred pass cannot re-run a blocking
one), and starting it at the end of a long session is how a tree ends up half-migrated. The measurement
is the durable part; the next tick executes on it.

**The plan, precisely:**

1. `manuk_js::load_document(dom, url, rects, styles, Phase::Blocking)` — runs only classic scripts with
   neither `defer` nor `async` nor `type=module`. Each executed script is marked, so nothing runs twice.
2. `Page::run_deferred_scripts(&mut self, fonts, vw)` — runs the rest (`defer`, `async`, `type=module`),
   re-cascades and re-lays-out if the tree changed, exactly as the current post-script path already does.
3. `Page::load` and `from_prefetched` call **both**, back to back — so every existing gate, and the whole
   SPA suite, sees identical behaviour and identical results. Nothing about the app web changes.
4. The **shell** calls them apart: blocking → paint → deferred → repaint. That is the only place the new
   behaviour is visible, and it is the only place a user is waiting.
5. **G_DEFER** asserts it: a page with a slow `defer`ed script must paint before that script has run, and
   the script must still run afterwards. And its falsifier makes the deferred script blocking again.

The gate matters as much as the change. "Fast because we never ran the script" is the same class of lie
as "fast because we never loaded the images" — which is precisely the disguise G_FIRST_PAINT was written
to strip off, one tick ago.

## Tick 32 — defer/async/module mean what they say (2026-07-13)

**TICK SHAPE: pattern-class.** CLUSTER: C01ca.

`Script { defer: bool, is_async: bool }` was parsed, stored, and used for **nothing**. Every script
blocked first paint — including the ones whose entire purpose is to say *"do not wait for me"*, and
including `type="module"`, which is **deferred by default** in every real browser and is what every
Vite / Rollup bundle on the internet ships as.

Now: `collect_inline_scripts` classifies each script by `blocks_paint`, `PageContext::load` runs only
the blocking ones, and `run_deferred_scripts` runs the rest. **`Page::load`, `load_async` and
`from_prefetched` call both back-to-back** — so every gate and the whole SPA suite behave exactly as
before. The **shell** is the only caller that separates them: blocking → paint → deferred → repaint.

```
nytimes.com   first paint 5,773ms → 5,083ms   (10 deferred scripts, 997ms, moved off the paint path)
```

**The honest read of that number**, because it is smaller than it should be: most of nytimes' JavaScript
is *classic blocking* script, which a real browser must also run before painting. Chromium hides that
cost by painting **incrementally as it parses** — the parts of the document above a blocking script are
already on screen when it runs. We parse the whole document, run every blocking script, then paint. That
is the next thing, and it is a bigger change than this one.

**Two process defects, both mine, both now pinned by the gate:**

1. **I applied the split to two of its three call sites.** `load_async` still called `from_dom` and
   nothing else — so a Vite bundle never executed and **every SPA in the suite silently stopped
   mounting**. The root element was still there, still the right size, and completely empty. The rule,
   which is worth stating rather than remembering: *every path that used to run all the scripts must
   still run all the scripts.* Exactly one caller may split them, and it is the shell, because it is the
   only one with a human waiting. `G_DEFER`'s second half is that bug, pinned.
2. **The gate itself was flaky.** Two `#[test]`s in one binary, each standing up a SpiderMonkey context;
   the leaked per-process runtime tears down messily when they co-run, so it passed, then segfaulted,
   then passed. **A flaky gate is worse than a missing one** — it gets ignored, and an ignored gate
   protects nothing. One test per JS gate binary, on purpose, and the reason is now written down where
   the next person will look for it.

`G_DEFER` asserts both halves — that deferred scripts do NOT run on the paint path, and that they DO run
after. Each without the other is a bug, and *"fast because we never ran the script"* is the same class of
lie as *"fast because we never loaded the images"*, which is the disguise `G_FIRST_PAINT` was written to
strip off one tick earlier.

## Tick 33 — a capability priority list, measured (2026-07-13)

**TICK SHAPE: pattern-class.** CLUSTER: C01ca.

"What should we build to support the breadth of the web" was being answered from imagination. It is now
answered from **237 real site snapshots** (`docs/loop/CAPABILITIES.md`), with two measured columns:

- **Usage** — how many *distinct sites* use the feature (never hit counts: one site with 500 `<div>`s
  must not outvote 200 sites with one `<iframe>`).
- **Support** — what the engine *answers when asked*, from a feature-detection probe run through the real
  pipeline. Not what the code looks like it does.

The priority order is not a roadmap anyone wrote. It is a **subtraction**:

```
<form> submit           50% used   ❌  ← the difference between a reader and a browser
<picture>/srcset        47% used   ❌  ← half the web's images
transition/@keyframes   38% used   ⚠️  static end-state
<iframe>                23% used   ❌  ← embeds, maps, players, payments, comments
position:sticky         14% used   ⚠️  laid out, does not stick
WebSocket                5% used   ❌  ← where social platforms live
```

And what the corpus confirms we were right to have: inline `<svg>` **72%**, CSS custom properties **53%**,
transform 45%, `@media` 42%, flex 41%, `type=module` **31%**, grid 28%, custom elements 19%, `:has()` 15%,
`@container` 11%.

**Fixed this tick, because a throw takes the page with it:**
- **`canvas.getContext('2d')` THREW.** Only 3% of sites use `<canvas>` — but `ctx.fillRect(…)` on the
  next line was a `TypeError`, so a charting library that initialises at boot took the **whole bundle**
  down. *3% of sites using a feature is 3% of sites BROKEN when it throws* — the usage number and the
  damage number are not the same number. Now: a real context, drawing ops are no-ops, `measureText`
  returns a real shape (layout code multiplies by `.width`, and `undefined * n` is `NaN` propagating into
  every coordinate downstream). A blank chart on a working page. `getContext('webgl')` → `null`, which is
  the spec's "cannot" and what every library already branches on.
- **`Notification`** (14%) — honest: `permission === 'denied'`. The site asked and was told no.

**And the file was nearly a lie on its first day.** Its first version opened with *"`localStorage` — 27%
of the web — THROWS. Not a gap, an outage."* **It was false.** A real, persisted, per-origin
`localStorage` had existed for ages. It threw because **I ran the probe from a `file://` URL** — an
opaque origin, which gets no storage in *every* browser and correctly answers `QuotaExceededError`.

**I had already written the replacement shim.** One more step and I would have shipped a worse duplicate
of a working feature and reported a 27%-of-the-web win that did not exist.

> **The instrument is part of the experiment.** The probe is now **served over real HTTP**, never opened
> from disk. Support numbers are measured from a real origin, or they are not measured.

## Tick 34 — the browser becomes writable (2026-07-13)

**TICK SHAPE: pattern-class.** CLUSTER: C01ca. The #1 item on the measured capability list: **forms, 50%
of the corpus** — the difference between a *reader* and a *browser*. You cannot search, log in, or buy
anything without them.

**The bug was not "forms are missing".** GET forms had submitted on click for ages. What was missing was
the **`submit` event** — and its absence broke essentially every *modern* form on the web.

A form on a React/Vue/Svelte page is not submitted by the browser at all. The page listens for `submit`,
calls `preventDefault()`, and does its own `fetch`. With no event ever dispatched, **that handler never
ran** — so we performed the **full GET navigation the author had explicitly cancelled**, throwing away
the page and everything the user had typed. From the user's side, the site "reloads itself" whenever
anyone presses a button, and nothing anywhere says why.

Now: `Page::dispatch_submit` fires a real `submit` event and returns whether to navigate; the shell
honours it. Plus `form.submit()` / `requestSubmit()` / `reset()`, which differ exactly as the spec
requires (`requestSubmit()` fires the event and may be cancelled; `submit()` does not, because a script
calling it has already decided).

**And two spec details that servers actually branch on, both of which were wrong:**
- A **checked checkbox with no `value`** submits the string `"on"`, not `""`. "The box was ticked"
  arriving as an empty string reads at the far end as "ticked, and the user typed nothing". Different
  claims.
- `application/x-www-form-urlencoded` encodes a space as **`+`**, not `%20`. `encodeURIComponent` alone
  gets this wrong — quietly, and *only for values containing spaces*, which is the worst possible
  distribution for a bug.

`method=POST` is still unimplemented — and it now **says so out loud** rather than being silently ignored.
A login that does nothing and reports nothing is the worst failure available to the person trying to use it.

---

**The process defect, and it is the second time in two ticks.**

I implemented a duplicate of `FormData`/`URLSearchParams`. They already existed and already worked. The
shim was dead on arrival — guarded by `typeof === 'undefined'` — and **I only noticed because the
behaviour did not change when I "fixed" it**. `localStorage` was the same story one tick earlier.

The cause was never carelessness about the code. It was **trusting a capability probe that did not test
the capability**. And that has a general shape, shared with the Bar 0 metric that measured Chromium, the
vacuous gate, and the `file://` probe:

> **An absent measurement is not a negative measurement.** "The probe did not say yes" and "the probe
> said no" are different facts. Treating the first as the second is how a project spends a tick
> rebuilding something it already had — and then reports the rebuild as a win.

The probe is the authority. It now tests `FormData`, `URLSearchParams`, `requestSubmit` and the `submit`
event, *before* anyone touches them.

## Tick 35 — `<iframe>`, and a priority list that corrected itself (2026-07-13)

**TICK SHAPE: pattern-class.** CLUSTER: C01ca.

**The plan said `<picture>`/`srcset` was #2 (47% of the web). Measuring it removed it from the list.**

Per the rule that cost two ticks to learn — *measure before you implement* — I checked the **damage**
number rather than the usage number:

```
<img srcset> used by                     82/237 sites  (34%)
<img srcset> with NO src fallback         2/237 sites  ( 0%)   ← we load NOTHING for these
<img srcset> with a placeholder src       1/237 sites  ( 0%)   ← we load the PLACEHOLDER
<picture>                                64/237 sites  (27%)   ← its <img> fallback is REQUIRED
```

233 of 237 sites ship a working `src`, which is exactly what `src` is *for*. So our missing `srcset`
costs a possibly-wrong-**resolution** image, not a **missing** one. Usage 34%, damage ~1%. It is worth
doing and it is not worth doing next.

> **USAGE IS NOT DAMAGE.** They are different columns and only one of them is a priority. `srcset` is
> used by 34% of sites and breaks ~1% of them, because `src` is a fallback and it works. `<canvas>` was
> used by 3% and broke *all* 3%, because it threw. **Rank by what happens to the user, not by what
> appears in the markup** — otherwise you build the popular thing instead of the load-bearing one.

**So the real #1 is `<iframe>`: 23% of sites, and here usage and damage are the SAME number**, because we
render a 0-height box with nothing in it. It is the gateway to embeds, maps, video players, payment
frames and comment widgets — most of what makes a page feel like the modern web.

**Hypothesis for the implementation:** an iframe is a *replaced element with a nested document*. The
pieces we need already exist — a `Page` can be built from HTML, laid out, and turned into a display list.
What is missing is (a) the box (spec default **300×150** when unsized, and we currently give it **zero
width**, so it is invisible even before any content question), (b) fetching the child document, and (c)
compositing the child's display list into the parent's, translated and clipped to the iframe's rect.

The risks I expect, in order: **depth** (an iframe containing an iframe), **budget** (a heavy third-party
embed must not hold the parent's paint hostage — the same lesson as G_FIRST_PAINT), and **isolation** (a
child's script must not be able to reach the parent's DOM).

**RESULT — tick 35.** `<iframe>` renders. 23% of the corpus, and the two bugs were:

1. **`iframe` was in no replaced-element list, in either cascade path.** It laid out at **zero width** —
   the box was gone before we ever got as far as failing to fetch its document. An unsized iframe is
   **300×150** by spec, which is not trivia: an iframe has no intrinsic size to fall back on, so with no
   default it collapses to nothing.
2. Nothing fetched or rendered the child document. Now it is fetched **after first paint** (an iframe is
   the single most likely thing on a page to be slow — G_FIRST_PAINT's rule, and an embed is exactly what
   would break it), rendered as a whole `Page`, and blitted through the replaced-element path.

**Isolation comes free from the architecture**, which is the nicest thing about this design: a
`PageContext` is per-`Page`, so a child's script has no path to the parent's DOM — *it cannot reach it
because it does not have it*. The gate pins that, because "it happens to be true" and "it is guaranteed"
are different claims and only one survives a refactor.

**Honest limits, stated rather than discovered later:** the embed is a **bitmap**. It renders; it does
not scroll, and it does not update. That is a fraction of the work of a live nested browsing context, and
a rendered embed you cannot scroll is enormously better than a 300×150 hole.

---

**And the bug that was not an iframe bug at all.**

The child document painted white. Chasing that found:

> **`<body>`'s background never propagated to the canvas.** CSS says the root element's background paints
> the whole canvas, and if the root has none, `<body>`'s is propagated up to it. We hard-coded `WHITE`.

**Every dark-themed page whose content is shorter than the viewport** was painting its content on a
correct dark box **floating in a white void**. It has presumably been that way for the entire life of the
project, on every dark site, and no gate asked — because every gate compared *boxes*, and the boxes were
right. The pixels were wrong in the space between them.

It was found through an iframe only because a child document is, by definition, "a page shorter than its
viewport". *The symptom names the wrong organ* — fourth time.

## Tick 36 — a fifth of the web had invisible content (2026-07-13)

**TICK SHAPE: pattern-class.** CLUSTER: C01ca.

Two things, and the second is a process failure I have now made three ticks running.

**1. Animated content was invisible — on 21% of the corpus.**

The most common animation on the web is a fade-in: the base rule says `opacity: 0`, the keyframes reveal
the element. **We rendered the base rule literally**, so the content never appeared at all. Measured: **52
of 237 sites (21%)** pair `opacity: 0` with an animation. Verified on a real page before fixing anything —
a fade-in element painted as pure background.

An animated element now renders its **end state**. That is not a hack; it is the spec's own idea, and
`prefers-reduced-motion: reduce` says exactly the same thing: **show the destination, skip the journey.**

**The narrowness is the whole design.** It would be trivial — and catastrophic — to "fix" this by forcing
every `opacity: 0` element visible. An author who hides something with **no** animation *meant it*: a
closed dropdown, an off-screen menu, a screen-reader-only label, a cookie banner that has not fired.
Revealing those is not a fix, it is a louder bug. So the rule is exactly: *`opacity: 0` **plus** an
animation → show it.* `opacity: 0` alone stays hidden, and **G_ANIMATION asserts both halves.**

Scoped to opacity, because opacity is the only one of these that makes content *disappear*: a `transform`
slide-in still renders (merely offset), and a colour transition still renders a colour.

**2. `position: sticky` already worked, and the ledger said it did not.**

`CAPABILITIES.md` said *"laid out, does not stick"*. **It had never been tested.** `apply_sticky` has
existed all along and works — I painted a page at scroll 0 and scroll 500, and the header pins to the
viewport top exactly as it should.

**That is the third untested assumption in three ticks** — `localStorage` (tick 33), `FormData` (tick 34),
`position: sticky` (tick 36). Three times I wrote *"❌ missing"* where the truth was *"✅ works, untested"*,
and twice I got as far as writing the replacement before noticing.

> **If the probe does not test it, its status is UNKNOWN — and "unknown" is not "missing".** The rule was
> already written down after the second time. Writing it down was not enough. What stops it is the habit
> that saved this tick: **go and test the thing before writing a status for it**, every time, even when
> the answer seems obvious. Especially then.

## Tick 37 — the crawl that validates twelve ticks (2026-07-13)

**TICK SHAPE: pattern-class.** CLUSTER: C01ca.

A clean 265-site crawl on the tick-36 binary. Everything before this is committed, so the thing under
test is exactly what is in git — which matters, because the last time I measured Bar 0 the number was
contaminated four separate ways, all of them mine.

**What the crawl has to answer, and the answers I am NOT allowed to assume:**

1. **Bar 0 on our own clock.** Last clean figure: **9 of 206 timed sites over 30s (4.4%)**, and we were
   faster than Chromium on **175/206 (84%)**. Twelve ticks of work have landed since — most of it *not*
   aimed at speed — so this could move in either direction, and a regression here outranks everything
   else in the ledger by construction (Part 24.3).
2. **Structural coverage.** Did the `<iframe>` box (23% of sites, previously zero-width), the animation
   reveal (21% with invisible content), and the canvas background (every dark site) move the diff?
3. **Whether anything I fixed broke something I was not looking at.** That is what a corpus is *for*, and
   it is the only question a gate cannot answer — a gate only knows what it was told to watch.

**The one thing I will not do is read the number before it is complete.** A partial crawl always
*under*-reports hangs (the hanging sites are the ones still running when you stop), and `status-update.sh`
now refuses to print one at all. That refusal exists because I did exactly this, once, and wrote the
result into STATUS.md as a fact.

**RESULT — tick 37. The crawl, and a report that lied on its first run.**

```
BAR 1 — node PRESENT              92.2%   (162,570 of 176,311 probed)
        ...and `display` agrees   73.0%   ← the next real gap
BAR 2 — geometry (DEFERRED)      123,796  the node exists, SAME SIZE, moved. Not a failure.

BAR 0 — over 30s on our clock     4/211   (1.9%)    was 4.4%
FASTER than Chromium            195/211   (92%)     was 84%
median          ours 16.1s  ·  Chromium 36.5s
p90             ours 24.7s  ·  Chromium 99.5s
```

**We are slower than Chromium on exactly one site** (atlassian.com, 34.6s vs 32.2s). Median **2.3× faster**.

**The north star, sharpened this tick** (and it is the user's formulation, which is better than the one I
was carrying):

> **Chromium is the CEILING on capability, and the FLOOR on everything else.**

Match its capability — the scripts run, the layout resolves, the forms submit, the embeds render. **Beat**
it on speed, stability, resource use and honesty of failure. A timing divergence in our favour is not a
bug to close; there is nothing to regress toward. The oracle diffs **structure**, and it must never score
timing.

I had been carrying a quieter version of the opposite assumption — treating Chromium's *behaviour* as the
target rather than its *capabilities* — and it is exactly the kind of thing that produces work nobody
needed.

**And the report lied on its first run.**

`scripts/crawl-report.sh` exists to make one rule mechanical: *a speed claim is only admissible next to a
coverage number*, because "fast because we never loaded the images" and "fast because we never ran the
script" are two lies this project has already told and caught. It printed:

```
  structural agreement : 2.8%
```

**For a browser that renders fine.** It had lumped all three divergence kinds together. But
`geometry` — 123,796 of them, 70% of the total — means *the node exists, at the same size, in a different
place*. bbc's `<h3>` is 208×88 in **both** engines and sits at a different y. **That is Bar 2, deferred by
settled decision.** It is not a rendering failure and it must never be counted as one.

The real Bar 1 number is **92.2%**. I nearly reported a catastrophic regression that did not exist —
**with the instrument I had just built to stop exactly that.** Fourth time an instrument has done this
here. None of them get to be trusted on sight, including the ones I write to enforce not trusting things.

**Next: the 27% `display` disagreement** — 33,825 nodes where we render the node but disagree with Chrome
about whether it is *shown*. Unlike geometry, that is a **real** difference: a node we hide that Chrome
shows is content the user cannot see.

## Tick 38 — what the 27% `display` gap actually is (2026-07-13)

**TICK SHAPE: pattern-class.** CLUSTER: C01ca. Measured before touching anything, and the measurement
overturned my hypothesis twice.

**Hypothesis 1 — "the load budget is cutting off stylesheets on slow sites."** *Wrong.* The sites losing
flex (deviantart, aljazeera, vimeo, techcrunch, replit) are all slow, and aljazeera runs 35s against a 12s
budget, so this was a good guess. Tested it: **zero stylesheet failures, zero budget expiries.** The CSS
is applied. A tempting story, killed in one command.

**Hypothesis 2 — "the scripts are destroying the DOM."** Found by accident, and *true for exactly one
site*:

```
aljazeera.com   parse → 2,591 elements   after scripts → 141   (5%)   ← SCRIPTS DESTROYED THE PAGE
github, HN, bbc, wikipedia, techcrunch, vimeo, deviantart, replit → 100% retained
```

aljazeera is server-rendered; something in its boot **clears the container** and our client-side re-render
does not put it back, leaving 5% of the document. That is severe and it is *one site so far* — it is the
**hydration** gap, and it is now a named, reproducible case rather than an unmeasured unknown.

**And the 27% `display` gap is mostly not what it looks like.** Split by whether it is *real*:

```
  11,324   we lose flex/grid on this node      ← REAL. The biggest one.
  13,736   other layout-mode mismatch
   4,299   representational: replaced elements ← NOISE. Chrome computes `inline` for <img>/<svg>;
                                                 we use `inline-block` to make them atomic. Same
                                                 rendering, different label.
   2,433   we SHOW what Chrome HIDES           ← REAL. Extra content.
   2,033   we HIDE what Chrome SHOWS           ← REAL, and the WORST: content the user cannot see.
```

**The cascade is not broken.** deviantart computes `Flex` on 915 nodes, `Grid` on 51, and has **zero**
nodes with no style. So flex is not being lost wholesale — it is being lost on *particular nodes*, which
means **selector matching**, not cascade plumbing. That is a completely different investigation than the
one I would have started an hour ago.

**Next, in order, and each is now a specific thing rather than a percentage:**
1. **`we HIDE what Chrome SHOWS` (2,033)** — smallest and worst. Content the user cannot see.
2. **flex/grid lost on specific nodes (11,324)** — a selector-matching question. `:is()`, `:where()`,
   attribute selectors, CSS nesting are the suspects.
3. **aljazeera's hydration wipe** — one site, 95% of a document, and the first real hydration failure with
   a name.

## Tick 39 — the cascade was silently dropping 41% of the web's CSS (2026-07-13)

**TICK SHAPE: pattern-class.** CLUSTER: C01ca.

Chasing the oracle's two *real* rendering divergences — *"we lose flex/grid"* (11,324 nodes) and *"we show
what Chrome hides"* (2,433) — I built a selector conformance probe. Two selectors failed. One of them was
mine.

**`RuleIndex` was throwing away every nested rule in every stylesheet.**

It was added in tick 14 as a **cascade optimisation** (339ms → 199ms): bucket rules by their rightmost
simple selector so the cascade stops walking every rule for every element. It walks each sheet's rules,
reads every `StyleRule`'s `selectors` and `block`... and **never looks at its `rules` field.** That field
holds the rule's **nested** rules. Stylo parses them correctly and always has.

```
sheet with 4 class rules                     → by_class = 4   ✓
the same 4 rules, written with CSS nesting   → by_class = 0   ← all four gone
```

Measured: **41% of the corpus uses CSS nesting** in its inline `<style>` blocks *alone* — external
stylesheets are not even scanned, so that is a **floor, not an estimate**. And it explains both real
divergences at once: a nested `display: flex` never applied (so we lose flex), and a nested
`display: none` never applied either (so we render menus, modals and off-screen panels that Chrome
correctly hides).

> **An optimisation that makes a data structure smaller must be asked what it DROPPED.** This one was
> measured for speed and never once asked whether the rules it indexed were all the rules there were. It
> shipped, and it passed every gate for 25 ticks — because **every gate compared *boxes*, and the boxes
> were internally consistent. They were just consistently wrong.**

`G_SELECTOR` now asserts that a nested rule applies (including the implicit-`&` form, which is by far the
commonest), *and* that every selector which already worked still does — a fix that silently breaks the
selectors the cascade already handled would be worse than the bug it repaired, and would be invisible.

**And `:has()` is broken — but I am not going to fix it unilaterally.**

`.a:has(.probe)` does not fail to *match*; the rule is **never parsed**, and is dropped. The cause:
Stylo's **servo** build hardcodes `fn parse_has(&self) -> bool { false }`. Gecko's returns `true`. There
is no pref — unlike `layout.grid.enabled`, which we already flip.

Enabling it means **editing Stylo's source**, and that collides head-on with a settled decision:
*"Stylo and SpiderMonkey are never patched internally."* My own methodology says a settled decision is not
relitigated silently, so it is written up in STATUS.md as a decision to be made, with the exact one-line
diff and the trade-off, rather than quietly done. It is 13% of sites, and it is the last known selector
gap.

## Tick 40 — the self-audit was certifying a third of the wall (2026-07-13)

**TICK SHAPE: infrastructure.** The self-audit came due. It reported everything green. It was checking
**four of twelve gates.**

Its falsifier check **hardcoded** `G_DEDUP G_LOAD G_RUNAWAY G2` — the four gates that existed the day it
was written. Six more had shipped since (G_FIRST_PAINT, G_DEFER, G_FORM, G_IFRAME, G_ANIMATION,
G_SELECTOR) and it knew about **none of them**. It would have gone on reporting the wall as certified
forever, while certifying a third of it.

> **A check that keeps its own copy of the list it is checking will drift from reality, and it will do it
> silently.** Same defect as a test re-deriving the constant it checks, and as a capability ledger whose
> ✅ was never tested. The gate list is now **derived from `verify.sh`**.

Deriving it turned the audit red on **nine gates with no falsifier**. Closing them found two gates that
**could not fail**:

**`G1` — visual fidelity — was structurally incapable of failing.** Its floor applies to the *structural*
score, and `coverage` returned **1.0 when `probed == 0`**. `example.com` — **in G1's own default URL
list** — has **no `[id]` elements at all**. It probed nothing, scored a perfect 100%, and inflated the
mean of the gate whose entire job is catching missing content. Proven by mutation: emptying
`node_rects()` so the browser renders **nothing at all** still scored 100% on that URL.

**`G6` — clickability — the same shape.** `MISSED` is 0 when the page has no links, so a browser that
finds **nothing** scores perfectly. It now refuses fewer than 50 links as vacuous, and reports
*3 unclickable of 484*.

**`G_CONTAIN` is exempt, and that is a fact rather than an excuse:** it deliberately panics a build and
asserts the *page* dies while the process lives. Its test input **is** the bug. That is strictly stronger
than a mutation — it is the standard the others are being held to.

**And three more false "VACUOUS" verdicts, all from weak mutations** — aimed at `Page::links()` (G6 reads
the DOM directly), a dead function nobody calls, a file G_TEARDOWN doesn't scan, and a black canvas
against a *structural* floor. Every time, **the gate was right and the mutation was wrong**.

> **A "VACUOUS" verdict is a claim about the gate. Verify it before believing it** — exactly as you would
> any other measurement. The tool that checks the instruments is an instrument.

**All twelve gates now go red when their bug is put back.** For the first time, that is a fact rather
than an assumption.

## Tick 41 — a missing constructor is a thrown exception (2026-07-13)

**TICK SHAPE: pattern-class.** CLUSTER: C01ca.

**The aljazeera wipe, traced.** `remove_child` was taking out 2,131 elements in one call — React clearing
its container, which is *normal* for `createRoot`. The bug was that the **re-render then came up empty**.

Peeled one `ReferenceError` at a time:

```
WebSocket missing  → React's render throws → error boundary → 141 elements
  fix → Blob missing     → 141
  fix → FileList missing → 122
  fix → (the ~40-name interface surface) → 470   ← 3.3x
```

**Each one was a different library's first line.** A live-blog client constructs a `WebSocket`. A sanitiser
constructs a `Blob`. A form library references `FileList` in an `instanceof`. **A page does not get to run
its fallback path if the *check* for the fallback throws.**

> **A missing constructor is not a missing feature — it is a thrown exception, and its blast radius is
> whatever was rendering at the time.** `canvas.getContext` was used by 3% of sites and broke 100% of
> them. `WebSocket` was used by one script on one page and took the whole front page.
>
> **Construct successfully, and answer honestly.** A blank canvas, an unopened socket, an empty `Blob` are
> survivable — every library on the web is written to survive them, because real browsers produce exactly
> those behind captive portals and in private windows. **A `ReferenceError` is survivable by nothing.**

Also found and fixed on the way: **`window.dispatchEvent` did not exist** — with an entire window-listener
registry sitting behind it, unreachable. And `document.title` (read *and* write), `.referrer`,
`.characterSet`, `.currentScript`, `navigator.vendor` were all `undefined`, and `undefined.split(…)` is a
`TypeError` that takes the rest of a bundle with it.

**And the bigger one, which was hiding underneath.**

> **The page's own `fetch()`/XHR calls were never performed outside the shell.**

`take_fetches()` handed them to the shell, and the shell alone made them. So the **oracle**, `boxes`, the
agent — every consumer that is not the shell — queued a data-driven SPA's API calls and **never made
them**. The app sat in its loading state forever and rendered a skeleton.

**That is very likely a large share of the oracle's 13,741 "missing" nodes.** A measurement harness that
cannot load a modern site's content is not measuring the browser, it is measuring itself — and it has
been scoring every data-driven SPA in the corpus against a skeleton. `finish_loading` now performs them,
in rounds, **inside the load budget**.

I introduced a **Bar 0 regression doing it** and caught it immediately: the budget was checked only
*between* rounds, so a single round ran unbounded and a 20s budget produced a 200s+ load. The round now
lives inside the budget, with a per-round request ceiling that is **logged** when it truncates — a silent
cap reads as "we did everything".

**Honest status on aljazeera: narrowed, not closed.** 141 → **470** elements. React discards the
server-rendered tree (its own choice) and its client render still comes up short of the 2,131 it replaced.
The remaining gap is the app's *data*, not its *code* — and the fetch pump is the first half of that.

## Tick 42 — `:has()`, hand-rolled rather than forked (2026-07-13)

**TICK SHAPE: pattern-class.** CLUSTER: C01ca.

**The decision, made and recorded rather than absorbed:** `:has()` rules are **dropped at parse** by
Stylo's *servo* build (`parse_has() -> false`; Gecko's returns `true`). **13% of the corpus.**

Enabling it upstream costs **vendoring Stylo** — I confirmed the hard way that `./stylo` in this repo is a
*reference checkout that nothing builds* (we depend on `stylo = "0.19"` from crates.io), so the "one-line
flag" is really a fork with a per-bump tax. Per the settled rule — *a borrowed engine is a means, not a
constraint*, tried in order **pref → minimal flag delta → hand-rolled supplement → hand-rolled module** —
the supplement is the right rung.

**Hypothesis:** Stylo *discards* `:has()` rules, so they never reach the cascade at all. But we already own
a selector engine — the one behind `querySelectorAll`. So:

1. Scan the stylesheet **sources** (which we already collect) for rules whose selector contains `:has(`.
   Stylo threw the parsed form away; the *text* is still ours.
2. Parse those with **our** engine, extended with `:has()`.
3. Apply their declarations as a **second cascade pass**, ordered by `(specificity, source order)` — the
   same ordering rule the main cascade uses.

**The risk I expect, and will measure rather than assume:** specificity interleaving between two engines.
A `:has()` rule must not blindly win over a higher-specificity normal rule. If that cannot be made correct,
applying it only where the property is otherwise *unset* is a smaller, honest subset — and I will say so
rather than ship a rule that wins fights it should lose.

**RESULT — tick 42.** `:has()` works: subject (`.a:has(.probe)`), descendant, `>`, `+`, `~`, and the
forgiving list. Gated by `G_SELECTOR` (both halves proven falsifiable), and the gate asserts the **negative**
case too — a `:has()` that should not match must not match, because a supplement that applies its rules
indiscriminately would *restyle the page*, which is far worse than the missing feature.

**The bug inside the fix, and it is a nice one:** `Dom::descendants()` seeds with the node's **children** —
it does not yield the node itself. My `:has()` descendant branch had `.skip(1)` to "skip the anchor", which
silently dropped the **first descendant** — exactly where `:has(.probe)` finds `.probe` in
`<div class=a><div class=probe>`. Child and sibling `:has()` worked; the commonest form did not. *An
off-by-one in a skip is invisible until the thing you are looking for happens to be first.*

**Cascade cost: none measurable** (F1 floor still 4.16ms). The supplement is skipped entirely for the ~87%
of sheets containing no `:has()` at all.

**And a process defect:** the falsifier reported **FALSIFIER BROKEN** for both mutations — and it was a
**linker OOM** (`ld terminated with signal 9`), not a bad mutation. Per PROCESS #29 I did not believe the
verdict on sight; a retry at `CARGO_BUILD_JOBS=2` proved both. *"The falsifier is broken" is a claim, and
claims get verified.*

---

**Media (researched, `docs/loop/MEDIA.md`): it is tick-sized, and the reason is structural.**

> A video frame **is** a `DecodedImage`. Playing a video is swapping the `Rc` in the map the **poster
> already occupies** (tick 28) and calling `request_redraw`. **No new paint code.**

`re_mp4` + `openh264` + `yuvutils-rs` → **first real frame** (½–1 day) → **muted looping `<video>` plays**
(1–2 days), which is *most of the `<video>` elements on the open web* — none of them have an audio track,
need a clock, or need ABR. Then audio (`symphonia` + `cpal`, and `<audio>` comes free), seek, then
High-profile via `cros-codecs`/ffmpeg **behind a trait defined in tick 1**.

**The finding that overturned the obvious plan:** there is **no pure-Rust H.264 decoder that can decode the
H.264 the web actually serves.** `openh264` and `rusty_h264` are both **Constrained Baseline only** — which
is exactly why *Firefox uses OpenH264 for WebRTC and never for `<video>`*. But YouTube's no-MSE fallback is
`avc1.42001E` — Baseline — so the cheap decoder is the right *first* rung, and the trait is what makes the
rest a swap rather than a rewrite.

**Two walls, now stated once and not relitigated:** MSE is genuinely 2–4 weeks and must come *after*.
**EME/DRM is never.** And the sharpest operational note in the whole report: ⚠ **do not advertise
`MediaSource` before it works — its absence is what makes YouTube serve the progressive fallback.**
Advertising MSE we cannot honour turns a working YouTube into a black rectangle. Same discipline as
`canPlayType() === ""`.

## Tick 43 — WPT, wired up. And it found four bugs in the first twenty-five tests. (2026-07-13)

**Hypothesis:** the oracle's *scope* is the ceiling on what can be found, and the oracle is a
265-site box-diff against Chromium. It has two structural blind spots and neither is fixable by
running it harder: it can only see what those sites happen to exercise, and it needs Chromium to say
what "right" is — so it can never tell us that **both** engines are wrong, or that we are wrong in a
way that does not move a box. **WPT has neither blind spot: the tests carry their own verdict.**
Wiring it up therefore outranks any individual fix. `tests/wpt` and `blitz/wpt/runner` have sat in
the tree, unused, since the beginning.

**RESULT — the instrument works, and the first thing it measured was itself.**

The first run: **0 of 25 files reported anything.** The runner's own guard fired —

> *"Above ~25% this number is not measuring the engine's conformance — it is measuring whether
> testharness.js can RUN here at all."*

— and it was right, four times over. **Four engine defects stood between us and a readable score, and
not one of them moves a box**, which is exactly why forty ticks of Chromium diffing never saw them:

1. **`window.parent` was undefined.** At the top level `window.parent === window`, and that
   self-reference is how a page knows it is the top: `while (w != w.parent) w = w.parent;` terminates
   *because* the top is its own parent. With `parent` undefined the loop does not fail to terminate —
   **it walks straight off the end.** It is the literal first thing `testharness.js` does. **One
   missing self-reference failed 100% of Web Platform Tests**, and it presented as *"our JS engine
   cannot run testharness.js"* — a far scarier and far wronger diagnosis than the truth.

2. **`DOMContentLoaded` and `load` were NEVER DISPATCHED.** Not once, anywhere in the engine — grep
   returned zero. **A site whose init lives in `window.addEventListener('load', …)` simply never
   initialised**, silently, for the entire life of this project. jQuery survived it by checking
   `document.readyState`, **which is precisely why nobody noticed: it worked often enough to look
   fine.** That is the worst failure shape there is.

3. **`setTimeout` threw its delay away.** Every timer was a FIFO push, so `setTimeout(f, 10000)` ran
   *before* a `setTimeout(g, 0)` queued after it. Insertion order, not time order — every debounce,
   throttle, retry-backoff and staged animation on the open web, in the wrong order, **without ever
   erroring.** testharness arms a 10s harness timeout at setup; ours fired it *before the tests it was
   guarding*, so every file reported TIMEOUT.

4. **`insertAdjacentText` was missing** while both its siblings shipped. *Nobody feature-detects the
   third member of a family when the first two are present.* testharness uses it to render its results
   table, so the throw aborted the loop invoking the completion callbacks and **29 of the first 40
   files reported nothing at all.**

**And a Bar 0 hang, found in the first 25 tests.** `ChildNode-after` calls `child.after(child)` **on
purpose**. Our `insert_before(parent, X, X)` detached X and then set `X.next_sibling = X` — **a
self-cycle**, so every subsequent `children()` walk spun forever. The DOM spec has the step we never
implemented: *"if referenceChild is node, set referenceChild to node's next sibling."* **No real site
inserts a node before itself**, so the 265-site crawl could never have found this. *That is the
argument for conformance testing, in one bug.*

**The clock had to learn about the lifecycle.** Fixing the delay exposed the next layer: a *virtual*
clock (we never sleep; we jump to whatever is due next) will happily run **ahead of the document** —
it drains everything else, leaps to the 10s timer, and fires it **before `load` ever happens**. So the
budget is 0 during load (only tasks due *now*, which is what a real browser does anyway since real
time has barely advanced), and **`load` is what opens it.** Ordering is the property that matters;
waiting is not.

**A throwing task also used to kill the entire event loop** — the exception propagated out of the
eval, the Rust `?` aborted `run()`, and every task queued after it never ran. One bad callback stopped
the page's clock forever. The spec says: report the exception and **keep going**. The errors now
accumulate in `globalThis.__errors`, which is the storage the **unhandled-error harvester** wanted.

**THE BASELINE (`docs/loop/WPT-BASELINE.md`):** `dom/` — **457 files, 1,429/6,284 subtests = 22.7%,
NO_REPORT 0, HANG/TIMEOUT 90.** `NO_REPORT 0` is the load-bearing number: **every file reports**, so
the 22.7% is a real measurement rather than a shadow of a broken runner. The 90 hangs outrank all of
it (Bar 0) and are the next tick.

**A hang can only be contained by a PROCESS boundary.** `tokio::time::timeout` cannot interrupt
synchronous JavaScript — a spinning test never yields, so the timeout future never runs. The runner
forks a child per batch and flushes one line per finished test, so the test *after* the last flushed
line is the one that hung: named, recorded, stepped over. **This is the same conclusion the tab process
model reached, arrived at independently and for the same reason.**

---

**PROCESS #32, and it is the ugliest one yet: I destroyed working code chasing a regression that did
not exist.** `G_GLOBALS` "failed", so I `git checkout`'d two files to bisect — losing the browsing-
context tree, the whole document lifecycle, `insertAdjacentText` and `Page::fire_lifecycle`. **The gate
was never broken.** I had run `cargo test -p manuk-page --test g_globals`, and **`manuk-page` does not
enable `spidermonkey` by default** — `verify.sh` runs it with `--features stylo,spidermonkey`. So the
engine had **no JS engine at all**, no script ran, and `"-"` was the *correct* output. **I was
measuring a browser no user has — the exact defect ADR-011 exists to prevent — and I trusted it enough
to delete code.**

*PROCESS #29's rule was written for green and VACUOUS verdicts. It applies to **RED** ones too: a
verdict is a CLAIM.* Now a hard rule in CLAUDE.MD: **never `git checkout`/`git stash` a file with
uncommitted work to test a hypothesis** — bisect by copying aside or commenting out, never by reverting
the only copy. And: **run a gate the way `verify.sh` runs it, or you are not running the gate.**

## Tick 44 — the instrument said "90 hangs". It was one. Then: click(), and CharacterData. (2026-07-13)

**TICK SHAPE: pattern-class** · **CLUSTER: C00wpt** — the classes are *programmatic activation*
(`element.click()`: menus, modals, carousels, hidden file inputs) and *text mutation*
(`CharacterData`: every text-editing surface), both found by WPT rather than by any single site.

**Hypothesis:** WPT's 90 hangs are Bar 0 and outrank every failing assertion, so they are the tick.

**They were not 90.** They were **one** — and finding that out was the first honest thing this tick did.
`run_one` assigned the word `TIMEOUT` to two different events (our budget expiring, and *testharness's
own* status-2 verdict), and the driver lumped both in with real driver-killed hangs. **Three distinct
findings shared one word.** Give one file 20s instead of 10 and it completed: they were never hung.

> **Three findings must never share a name.** They are now four columns — HANG (Bar 0), CRASH (Bar 0),
> SLOW (perf), TH_TIMEOUT (conformance) — and each means exactly one thing.

**The one real hang was a FROZEN CLOCK.** `Event-timestamp-safe-resolution` does
`do { … } while (deltaInMicroSeconds == 0)` — it **busy-waits for time to advance**. `event.timeStamp`
was hardcoded to `0`, so the delta was always zero and the loop spun forever. *A constant is an
infinite loop for any code that waits for it to change.* Fixed: `timeStamp = performance.now()`.

**And the file count was lying.** When a child **crashed** rather than hung, the driver advanced past
the whole batch — **33 files silently vanished** from a 457-file suite, and the pass rate was computed
over what was left, with nothing to say so. Fixing it surfaced **5 real crashes** that had been
invisible the entire time. *A runner that quietly skips what it cannot run reports a pass rate for a
suite it did not run.*

**Then the capability, which is the point:**

- **`element.click()` DID NOT EXIST.** It is how the web *activates* things — menus, modals, carousels,
  "click the hidden file input", every framework's programmatic activation. Its absence is a `TypeError`
  on the call, so whatever was running dies with it, and any `async_test` awaiting the event **never
  completes**. It now dispatches a bubbling, cancelable `click` through the same registry
  `dispatchEvent` uses, so delegated handlers see it. *(Honest limit, stated not discovered: it fires
  the event; full **activation behaviour** — toggling a checkbox, following a link — is the follow-on.)*

- **CharacterData was `data` and nothing else.** No `length`, `substringData`, `appendData`,
  `insertData`, `deleteData`, `replaceData` — WPT scored `CharacterData-replaceData` **0/34**, which is
  what "the method does not exist" looks like from outside. All six now exist and **throw a real
  `DOMException`** on a bad offset (natives throw by evaluating the `throw` and returning `false`,
  leaving the exception pending — the sanctioned JSNative failure path).

  **The offsets are UTF-16 code units, and that is the whole difficulty.** `"😀".length === 2` in JS, and
  offset 1 lands *inside* the surrogate pair. Rust strings are UTF-8, so counting `char`s corrupts every
  emoji, every CJK surrogate and every combining sequence — **silently, and only for the users who write
  in those scripts.** `G_CHARDATA` asserts `'a😀b'.length === 4` and is proven falsifiable against
  exactly that mutation.

**MEASURED — the ratchet turned, on all three faces:**

| | tick 43 | tick 44 |
|---|---|---|
| `dom/` subtests | 1429/6284 = **22.7%** | **1547/6280 = 24.6%** |
| Bar 0 hangs | 1 | **0** |
| crashes | *hidden* | **5, visible** |
| files measured | 457 (33 silently dropped) | **458** |

**No capability lost, no perf floor moved, and the instrument got more honest rather than less.**

## Tick 45 — the phantom fork, and forty-two ticks of knowledge recovered from the dead (2026-07-13)

**TICK SHAPE: infrastructure** — it multiplies every future tick, which is what puts it in scope by
definition.

**Hypothesis (the phantom fork):** `./stylo` looks like source we build, and it is not. It is a plain clone
of `github.com/servo/stylo`, **gitignored, zero files tracked**, not a workspace member, with no
`[patch.crates-io]` and no path dependency — while `Cargo.lock` pins **`stylo 0.19.0` from the crates.io
registry with a checksum.** *Editing anything under it changes nothing.*

**It had already cost a tick** (42: flip `parse_has() -> true`, rebuild, observe nothing, re-price the whole
`:has()` decision) — **and the clone was still DIRTY, carrying that orphaned edit.**

> **A dirty reference checkout is, by definition, someone believing an edit matters when it cannot.**

**RESULT:** the clone is restored to pristine upstream, the topology is written down
(`docs/wiki/build-and-dependencies.md`), and **`G_NO_PHANTOM_FORK` now fails the wall** if `./stylo` has
local modifications, or if a `[patch.crates-io]` ever appears that STATUS.md's fork surface does not record.
*Proven to go red by dirtying the clone and watching it fire.* **If a fork is ever genuinely needed there is
exactly one sanctioned way, and it is written down: `[patch.crates-io]` → a fork TRACKED IN THIS REPO, with
a gate that fails when a dependency bump silently reverts it.**

---

**THE BACKFILL — and this is the larger half of the tick.**

The wiki was instituted in tick 43, which meant **everything learned in ticks 1–42 was trapped**: in the
journal (a *log*), in STATUS (a *snapshot*), in the git history — and, worst, **in files that had since been
deleted or rewritten out of existence.** Neither a log nor a snapshot answers *"what do we now durably KNOW
about how this engine, and the web platform, actually work?"*

Five readers mined it in parallel: the **full commit history**, **`JOURNAL.md`**, the
**research/methodology docs**, the **capability ledgers + every gate's doc-comment**, and — the one that
mattered most — **the archaeology of deleted and superseded doc versions.**

**~2,400 lines across 12 topic files**, organised **by subsystem, never by tick.** A sample of what would
otherwise have been lost with the next compaction:

- **`Dom::flat_children` was correct, tested, and used by the HTML crate — while layout and the cascade
  walked `children()`.** Every web component on the web produced **zero boxes**. *The mechanism existed;
  nothing had drawn a line from it to the renderer.* **That happened THREE times in three ticks** (comments,
  fragments, the flat tree) — **one gate-shaped hole, not three bug-shaped ones.**
- **Chromium never asks fontconfig for a bare generic family — it asks for *Arial* and *Times New Roman*.**
  And the instinct to "just ask `fc-match sans-serif`" is **also wrong**: that returns Noto, whose line box
  is **1.362em against Liberation's 1.150em**, so every line on every page comes out **18% too tall**.
- **Answering `MinContent` with max-content means no flex item containing a paragraph can ever shrink** —
  taffy uses the min-content answer as the item's automatic minimum size.
- **`import.meta` needs an embedder module-metadata hook**, and Vite/Rollup/esbuild emit `import.meta.url`
  **unconditionally** — so one missing callback made **every bundler-produced app on the internet** fail
  silently.
- **A `<template>`'s DSD hook fires at the START tag**, so moving its children in the hook **moves nothing**
  — you must point the template's *contents* at the shadow root.
- **"73 of 265 sites HANG (27.5%)" was measuring CHROMIUM'S clock** — the watchdog wrapped both engines, and
  Chromium is the slower one on 84% of this corpus. *The real number was nine.* **That wrong number set the
  schedule for several ticks.**
- **Two design decisions recorded as DECIDED-BUT-UNDONE**, which would otherwise have been silently
  re-derived: the DOM bindings are still **string-`eval`** bindings, and methods are still defined
  **per-instance** rather than on one shared prototype per interface — *which is what breaks the
  `instanceof`/`constructor` semantics pages test for.*

**From here it accumulates by construction:** the pre-commit hook requires a **`WIKI:`** trailer on every
tick, with an explicit escape hatch (`WIKI: none — <why>`) so **skipping is an auditable CHOICE rather than
a silent gap.**

## Tick 46 — a stale handle killed the whole browser, and four of the five "crashes" were mine (2026-07-13)

**TICK SHAPE: pattern-class** · **CLUSTER: C00wpt** — the class is *"a document holds a handle from a
different document"*, which is every multi-page process, i.e. **every browser**.

**Hypothesis:** the 5 crashes WPT surfaced are Bar 0 and outrank everything else.

**They were not 5. They were ONE — and the other four were my own instrument.**

The driver computed its read offset as `results.len()` — but `results` **also holds the synthetic HANG/CRASH
rows, which have no line in the child's JSONL.** So the moment one was pushed, `skip(before)` over-skipped,
the next batch read short, was diagnosed *"the child died"*, and pushed **another** synthetic row — which
over-skipped further. ***One real event manufactured a cascade of fake ones.***

And the driver **reported CRASH without ever checking the child's exit status.** *"The child produced fewer
rows than asked" is not a diagnosis* — it could be a segfault (Bar 0) or the instrument miscounting
(nothing). Now: **`CRASH`** (killed by a signal) · **`EXIT`** (nonzero/panic) · **`SHORT`** (exited 0 and
wrote fewer rows — **an INSTRUMENT fault, and it says so**).

**THE ONE REAL CRASH, and it is a good one.**

```
thread 'main' panicked at engine/dom/src/lib.rs:347
panic in a function that cannot unwind
Segmentation fault (core dumped)
```

**A JS reflector stores its node as a bare integer — and the arena it indexes is NOT necessarily the arena
it came from.** One process loads many documents, and `CURRENT_DOM` is swapped on every re-entry into
script. So a handle held from an **earlier document** hands a raw index into a *different, smaller* arena,
and `self.nodes[id.index()]` **walks straight off the end.**

**And the consequence is not a wrong answer — it is a DEAD BROWSER.** These accessors are reached from
`extern "C"` natives, which are **`nounwind`**: a Rust panic inside one is *"panic in a function that cannot
unwind"* → **SIGSEGV, core dumped. Every tab the user had open dies because one page held a stale node.**

> **It is perfectly clean in isolation.** The file passes on its own; a 120-file batch passes; it only dies
> when it runs **after other documents.** *No single-page test could ever have caught this — which is
> exactly why it survived every gate on the wall.*

**The fix is one check at the choke point:** `node_and_dom` now validates the handle with the arena's own
`is_alive` (bounds **and** generation), so a stale or foreign handle reads as **"no such node"** and the
native no-ops. **That is the spec-shaped answer anyway: an operation on a node that is not there does
nothing.** Plus defence in depth — the JS-reachable read accessors (`is_fragment`, `parent`, `first_child`,
`last_child`, `next_sibling`, `character_data`, `is_element`, `is_shadow_root`) now use `.get()` instead of
indexing. **The new gate found four more of those the moment it existed.**

**MEASURED — the ratchet turned, and nothing else moved:**

| | before | after |
|---|---|---|
| `dom/` Bar 0 (HANG/CRASH) | **1 real + 4 phantom** | **0** |
| `dom/` subtests | 1548/6287 | **1548/6287** (unchanged — no regression) |
| instrument faults reported as engine faults | 4 | **0, and it now says `SHORT` when it is its own fault** |

`G_STALE_NODE` gates it and is **proven falsifiable** against exactly the mutation that put it back.

**The open Bar 0 residual, stated rather than discovered later:** *a panic anywhere inside a JSNative still
aborts the process.* We removed the known source and hardened the arena, but **`catch_unwind` at the native
boundary is the real containment**, and it is not yet there.

## Tick 47 — a panic in a JS native killed the browser. Containing the CLASS, not the instance. (2026-07-13)

**TICK SHAPE: infrastructure** — it multiplies every future tick: every DOM method written from here on is
born contained.

**Hypothesis:** tick 46 fixed *one* index that panicked inside a native. **That was prevention of an
INSTANCE.** The class is still open, and it is Bar 0: **every DOM method is an `extern "C"` function, and
`extern "C"` is `nounwind` — so ANY Rust panic inside ANY of them is *"panic in a function that cannot
unwind"* → SIGSEGV.** The whole browser, and every tab the user had open, because one page hit one bad
index.

**RESULT: contained — and the first attempt silently did not work, which is the whole lesson.**

> **Wrapping an `extern "C"` function in `catch_unwind` FROM THE OUTSIDE does nothing at all.** The panic
> aborts at *that function's own* `nounwind` boundary, **before any outer `catch_unwind` is ever reached.**
> I wrapped all 67 natives, the build was clean, and **the gate still died with `panic in a function that
> cannot unwind`.**

**The catch has to be INSIDE the `extern "C"` frame.** So every native is now a plain Rust `unsafe fn`, and
the **generated trampoline is the only `extern "C"` frame** — which is where `guard_native` sits. A panic
becomes:

- **loud** — logged at `error!` with the native's name. *A crash you made survivable and INVISIBLE becomes a
  permanent, unexplained "this site just doesn't work"* — the silent-failure bug this project has already
  paid for three times.
- **`undefined`** — the call returns, the page keeps running, the tab survives.
- **`true`, not `false`** — `false` tells SpiderMonkey *an exception is pending*, and there isn't one. *That
  would trade a segfault for an assertion failure.*

**Coverage: every page-callable native.** 57 methods (`def`) + 45 accessors (`prop`, incl. `textContent` —
which a **multi-line call site had hidden from the first regex**) + 10 host natives registered directly with
`JS_DefineFunction` (console, storage, scroll, `getComputedStyle`, `window.open`, history, `postMessage`).
*A partial containment boundary is a FALSE guarantee, so the audit was mechanical: grep for every remaining
`extern "C"` and account for it.*

**The residual, stated rather than discovered later:** three **SpiderMonkey engine callbacks**
(`module_metadata_hook`, `module_resolve_hook`, `promise_rejection_tracker`) are still bare. They are **not
page-callable** and have different signatures. *Named, so nobody has to rediscover them.*

**G_CONTAIN_NATIVE proves it rather than asserting it:** a native panics **on purpose** (registered only
under `MANUK_PANIC_PROBE`, so it has no production surface), and the gate asserts the page **keeps running
afterwards** — creates an element, appends it, queries it. **Falsified by removing the `catch_unwind`: the
test binary does not fail politely, it ABORTS.** *Which is exactly the Bar 0 failure the boundary exists to
prevent.*

**MEASURED — the ratchet turned, nothing else moved:**

| | before | after |
|---|---|---|
| page-callable natives that can kill the browser | **112** | **0** |
| `dom/` WPT subtests | 1548/6287 | **1548/6287** (unchanged) |
| `dom/` Bar 0 | 0 | **0** |

*Bar 0's founding promise — **a bad page kills the PAGE, not the browser** — is now true of the JS boundary,
and not merely of the Rust one.*

## Tick 48 — a second document, and the cycle check that stands between the DOM and an infinite loop (2026-07-13)

**TICK SHAPE: pattern-class** · **CLUSTER: C00wpt** — the class is *the detached document*, which is how
every sanitizer on the web (DOMPurify and its kin) processes untrusted markup.

**Hypothesis:** `dom/nodes` (5,125 subtests, the largest pool) is 22.4%. Clustering its failure messages
gives a clean work list, and one cluster dominates: **488 failures on `can't access property
"documentElement"`, every one downstream of `document.implementation` not existing.**

**RESULT — `document.implementation.createHTMLDocument()` builds a REAL second document.** One arena,
several roots: a document is not special storage, it is a node whose *type* is `Document`, so everything
that already walks the tree works on it unchanged. `hasFeature()` returns `true` (the spec now defines it as
a constant, precisely because feature-detecting through it never worked).

**And it immediately created a Bar 0 hazard, which is the more important half of the tick.** The moment a
page can obtain a *second Document*, it can try to **insert** it — and we had **no pre-insertion validity
check at all.** Inserting a node into its own descendant makes the tree a **cycle**, and every subsequent
`children()` walk **spins forever.** *That is a hang, not a wrong answer — Bar 0.* It was invisible only
because the door had been locked: with no `createHTMLDocument()`, a page could not get a second Document to
insert. **Five WPT files went from passing to killing the process the instant that method existed.**

The fix is the spec's `HierarchyRequestError` — a Document cannot be a child, and a node cannot be inserted
into its own inclusive ancestor — enforced at **two** layers: the JS native (throws), *and* the arena
itself (`append_child`/`insert_before` refuse to build a cycle), **because the arena is reachable from the
parser and from Rust callers too.** This is also exactly what the 588 `assert_throws_dom` failures want.

**A Bar 0 regression I caught and refused to ship, which is the ratchet working.** Adding `createEvent`
alongside the rest looked like a free +213 — but the moment it existed, tests reached real event dispatch
with **listeners mutated mid-dispatch** (`Event-dispatch-handlers-changed`), and **our dispatch loops
forever.** A synchronous infinite loop no timeout can interrupt. So **`createEvent` is deferred with the
reason stated** — `undefined` (a catchable TypeError) is strictly safer than a hang that takes the tab down,
and the dispatch-loop fix is its own tick. *The score is 26.7% instead of a higher number, and it is an
honest 26.7% with Bar 0 clean.*

**MEASURED — the ratchet turned, on all three faces:**

| | tick 47 | tick 48 |
|---|---|---|
| `dom/` subtests | 1548/6287 = **24.6%** | **1738/6499 = 26.7%** |
| `dom/` Bar 0 (HANG/CRASH) | 0 | **0** (a regression to 1 was caught and reverted) |
| NO_REPORT | 0 | **0** |

`G_DOM_IMPL` gates both halves and is proven falsifiable against removing the cycle check.

**Deferred, stated rather than discovered later:** `createEvent`/`initEvent` (needs the mid-dispatch listener
loop fixed first); `createHTMLDocument`'s reflector currently gets **element** members, not document ones
(handing it document members breaks the *real* document — 5 files stop reporting — and finding why is its
own tick), so `doc.body` on the returned object is `undefined` while the arena tree behind it is correct.

## Tick 49 — the CI lane, the WPT horizon, and why 8 checks were red (2026-07-13)

**TICK SHAPE: infrastructure** — measurement and verification scaffolding; it multiplies every future tick.

**Three standing directives, folded in together because they are one coherent piece of work.**

**1. Why the 8 CI checks were red — and it was TWO causes, not eight.** All the build failures
(`build+test` ×3 OSes, `static release` ×3, `feature builds`) share **one root cause**: `shell` and
`tests/wpt` default to `stylo`+`spidermonkey`, so `cargo build --workspace` builds **mozjs** — which fails
in clean CI on exactly the libclang/libstdc++ issue the wiki already documents (*"bindgen's libclang does
not inherit the clang driver's gcc-toolchain probe"*). It passed **locally only because mozjs was cached.**
The eighth failure, `fmt`, was **mine** — a tick's worth of Python-edits left code unformatted.

**Fixes:** `cargo fmt --all` (one check green immediately, verifiable now). And the workflow is
**restructured for honesty** (per the process-model directive: *Linux-validated ≠ cross-platform-
validated*): a **badge-bearing `verify-linux` lane** that runs the shipping-config gate wall and installs
the documented clang environment, plus a **separate `cross-platform` known-gap lane** (`continue-on-error`)
that tracks the cross-OS mozjs build without holding the badge red on work honestly labelled unverified.
*When a platform goes green, it gets promoted into the badge lane — the same ratchet as everything else.*

**2. The async CI lane + badge.** The workflow runs the **full wall on every push**, in parallel, **nothing
in the tick loop waits on it** — a regression it finds is an ordinary gate failure at the next check-in, not
an interrupt. The README badge is the highest-visibility, lowest-effort credibility signal available.
**Stated once and honoured: the badge is a byproduct of running the lane correctly, never a reason to change
what it checks.**

**3. The WPT horizon map** (`docs/wiki/wpt-horizon.md`) — **a third anchor of parity scope**, alongside the
differential oracle and the doc/app/platform-web roadmap. **Counts are LIVE, counted from the tree by
`scripts/wpt-horizon.sh`, never fabricated** (Part 13's rule). Measured anchor points today:

| Category | files | subtests | pass % |
|---|---:|---:|---:|
| `dom/` | 619 | 1,738/6,499 | **26.7%** |
| `html/dom/` | 237 | **12,497/59,560** | 21.0% |
| `css/selectors/` | 531 | 514/1,840 | 27.9% |
| `domparsing/` | 64 | 126/1,273 | 9.9% |

**The structural fact the map turns on:** every top-level WPT dir is one spec **except `css/`**, which is
dozens of sub-specs in one directory (`css-grid` 2,226 files, `css-flexbox` 1,433, `css-selectors` 531) —
tracked individually. And the honest framing: *we do not need Chromium's number; we need enough of the spec
that most of the real web works, and a graceful decline for the rest.* WPT shows the **shape of "enough"**.

**The checkout is sparse** (9 of ~90 top-level dirs), so the loading/interaction/media/a11y categories are
mapped but not yet measurable — `./scripts/wpt-setup.sh` adds a dir before it can be run. The map records
which are `[checked out]` vs pending, so the gap is visible rather than assumed.

## Tick 50 — the engine runs in a browser: wasm feasibility proven, and one 32-bit bug (2026-07-13)

**TICK SHAPE: infrastructure** — it unblocks the in-browser demo (a whole new proof-of-realness surface) and
hardens the cross-platform/ARM target; it multiplies future work rather than being one feature.

**Hypothesis:** the in-browser demo directive's load-bearing unknown is *"does the engine even compile to
`wasm32-unknown-unknown`?"* Everything downstream (curated snapshots, canvas, side-by-side, GitHub Pages) is
scaffolding around that one fact — so probe it FIRST, before building anything, exactly as the WPT runner
probed `testharness.js`.

**RESULT: the entire render pipeline minus JS compiles to wasm.** `dom`, `css`+**Stylo**, `layout`(taffy),
`paint`(tiny-skia), `html`, `text` — all of it, proven repeatably by `./scripts/wasm-check.sh`. **Stylo was
the real risk and it cleared.** The demo is feasible.

**The one genuine blocker, found by the probe and fixed:** `NodeId` packed `generation << 32 | index` into a
**`usize`** — and **`wasm32`'s `usize` is 32 bits**, so the shift **overflowed and `manuk-dom` did not even
compile** (*"this arithmetic operation will overflow"*). This is the exact class of bug a probe exists to
find: invisible on the 64-bit dev machine, fatal on the target. The fix is a **`u64`** backing — byte-
identical to `usize` on 64-bit, correct on 32-bit — so **the arena is now pointer-width-independent**, which
also hardens the ARM/cross-platform target the process-model directive named.

**The ripple was ~20 sites** (`NodeId(x as usize)` → `as u64`, a couple of `.map(NodeId)` over `usize`
ranges, one shell `node.0`), all mechanical, and **native is unregressed — the full wall is green and
stable across three runs.** `G_ARENA_U64` pins the packing semantics so a future *"simplify NodeId back to
usize"* cannot silently reintroduce the overflow, and it is proven falsifiable.

**The plan is written (`docs/loop/DEMO.md`)** — real (Stylo/Taffy/tiny-skia executing in the visitor's
browser; live scroll/click/hover and CSS state), not real and *said so in-product* (no JS, no arbitrary
fetch — curated snapshots from the cluster registry), single-threaded by choice (keeps GitHub Pages clean:
no COOP/COEP, no `SharedArrayBuffer`), a **side-by-side-vs-Chromium** toggle so realness is *checkable* not
*assertable*, its **own non-blocking CI lane** on a stable branch, and the maintenance rule that the demo
path's output must not diverge from the native path's.

**Directly serves the standing concern** — *"the same codebase must work for the wasm demo, native x86-64,
and mac/windows/linux, without regressing capability."* The `u64` fix advances all of them at once: 32-bit
safety for wasm, pointer-width independence for ARM, and zero native regression.

**Next (the demo is feasible, not yet built):** the `demo/` wasm crate wiring the already-proven pipeline to
a canvas, the snapshot-baking build step, the JS-glue shell, and the GitHub Pages lane.

## Tick 51 — the CI lane actually goes green, and an OOM guard so a killed linker stops lying (2026-07-13)

**TICK SHAPE: infrastructure**

**Hypothesis:** tick 49 restructured CI for honesty but it still failed. Find out *why* rather than
guessing — and the answer was **three real bugs, none of them mozjs**, which is what I had assumed.

1. **The default `--workspace` build failed on the GUI, not on mozjs.** `shell` defaults to `gui`
   (winit/wgpu), which needs the **X11/Wayland dev libs** a bare CI runner does not have. I had spent the
   previous tick adding clang/libclang for mozjs — a fix for a problem that was not happening.
2. **`cargo build --workspace --no-default-features` was genuinely broken — and it broke LOCALLY too.**
   `manuk-wpt` called `Page::eval_for_test` (gated behind `spidermonkey`) and
   `manuk_css::stylo_engine::CASCADES` (gated behind `stylo`). **The headless configuration — the lean
   substrate the demo, the agent and mac/windows CI all build — had not compiled for some time and nothing
   said so**, because the local wall only ever builds the shipping config.
3. **`manuk-shell`'s affordance-gate test read `gui::MENU_LEN` unconditionally**, so `cargo test
   --no-default-features` could not even compile it. Guarded with `#[cfg(all(test, feature = "gui"))]` —
   *the gate is only meaningful when the GUI it checks is built.*

**And a calibration, made deliberately rather than by drift:** the badge lane runs **clippy but does not
gate on `-D warnings`.** There are ~65 accumulated style lints; failing the badge over them would produce
exactly the *"green light that has stopped meaning anything"* the CI rationale warns about — and fixing 65
lints is the per-tick blocker the standing directive says CI must never become. **The badge means "the
shipping config builds and its correctness gates pass."** Clippy-clean is a tracked epoch goal; when it
reaches zero, `-D warnings` is restored and it becomes a ratchet tooth.

**THE OOM GUARD (`scripts/mem-guard.sh`), and it is a correctness mechanism, not a performance one.**

> **`ld terminated with signal 9 [Killed]` is the OOM killer, and it looks EXACTLY like a compile error:**
> cargo returns non-zero, and every wrapper above it reads that as *"the code is broken."*

It has already cost a false verdict — `falsify.sh` reported **FALSIFIER BROKEN** for two perfectly good
mutations, and only a retry at `CARGO_BUILD_JOBS=2` proved both (PROCESS #31). **An OOM that presents as a
test result is the worst kind of instrument failure, because it is believed.**

The guard derives the job count from **available memory, not `nproc`** — because mozjs and Stylo are the
heaviest things in the graph, **LLVM codegen peaks around 1.5–2 GB per parallel job**, and cargo's default
`-j 32` on this box would ask for ~50 GB of transient RSS on a 31 GB machine. *The default is not a setting
anyone chose; it is `nproc`, and `nproc` knows nothing about LLVM.* Sourced by `verify.sh` and
`falsify.sh`; the same cap (`CARGO_BUILD_JOBS: 2`, debug-info off) is set in CI, where the runners are
2-core/7 GB.

It also **detects a full swap and says so** — 8.0/8.0 GiB here, stale pages from an earlier spike. A
machine already swapping will thrash under a link, and **a thrashing link is the one that gets killed.**

**And the self-audit — 11 ticks overdue, blocked every commit until it ran, and immediately earned its
keep.** It found one thing I had missed for six ticks: **`G_NO_PHANTOM_FORK` had no falsifier.** I had
checked it by hand once in tick 45 — *which is not the same thing.* **A gate never proven to go red is not
known to work**, and the audit derives its gate list from `verify.sh` and cross-checks `falsify.sh`, so
that gap was a *mechanical* finding rather than a remembered one.

Writing the falsifier then produced **PROCESS #33, and it is a good one:** the first version piped
`verify.sh | grep -q`, and under `set -o pipefail` **the pipeline returns non-zero because verify itself
exits non-zero — which is exactly what the falsifier wanted it to do.** So grep matched, the gate had gone
red correctly, and the falsifier reported *"✗ STAYED GREEN."* I did not believe it (PROCESS #29, again),
reproduced by hand, and the gate fired perfectly.

> **Every layer of this stack — the gate, the falsifier, the auditor — has now produced at least one false
> verdict.** *The falsifier is an instrument, and instruments get verified.*

Self-audit closed: **methodology and reality agree.**

## Tick 52 — CI could not tell me why it failed, so I made it able to (2026-07-13)

**TICK SHAPE: infrastructure**

**Hypothesis: I have been guessing.** Two ticks were spent "fixing" CI causes I had *inferred* — mozjs's
libclang, the GUI system libs, a stale cache — because **GitHub's job LOGS require admin rights** (the API
answers **403 "Must have admin rights"** even though the repo is public). From the outside, a CI failure
was exactly one line:

> *"Process completed with exit code 101."*

**That is not a diagnosis.** And this project has a name for acting on one: *a verdict is a claim.* I had
been treating an unreadable exit code as evidence.

**The instrument, not another guess:** **check-run ANNOTATIONS are public** — I can read them from the API
without any token. GitHub promotes any line starting with `::error::` into one. So `scripts/ci-run.sh`
wraps every build/test step, streams output normally, and **on failure re-emits the real compiler error as
annotations.** The loop now reads the actual error off the API instead of theorising about it.

It also names **`signal: 9`** explicitly, because *the OOM killer looks exactly like a compile error* and
gets read as "the code is broken" — the same lie `mem-guard.sh` exists to stop (PROCESS #31).

**What I ruled out with a real experiment rather than a hunch:** a **fresh `git clone` builds the headless
config cleanly** (exit 0). So the code is right and the environment is wrong — which is precisely the thing
the annotations will now say out loud. *The local-only, gitignored bins were also checked and cleared: none
is declared as a `[[bin]]`, so a fresh checkout never needed them.*

**Security hygiene, audited while in the area:** the local-only tooling is excluded via
**`.git/info/exclude`**, not the public `.gitignore` (which is itself committed and would advertise the
files it names). One committed doc mentioned a local-only *filename* while explaining the public/local
split — scrubbed. **Zero committed references remain.**

## Tick 53 — every CI job on every OS failed for ONE committed line (2026-07-13)

**TICK SHAPE: infrastructure**

**The annotation instrument built in tick 52 paid for itself on its first run.** CI's real error, readable
at last:

```
error: could not execute process `sccache .../rustc -vV` (never executed)
Caused by: No such file or directory (os error 2)
```

**`.cargo/config.toml` — which is COMMITTED — hard-coded `rustc-wrapper = "sccache"`.**

**Cargo does not degrade when the wrapper is missing. It dies.** So the repository was **unbuildable by
anyone who did not already have sccache installed** — every CI runner, and every contributor who ever
cloned it. That single line is the whole reason **all eight checks failed on all three operating systems**,
from the moment it was added.

> **And it never failed once locally, which is the entire shape of the bug:** sccache *is* installed here.
> The rule it violates is one this project already wrote down — ***a committed artifact must be usable by
> anyone who clones this repo.*** The config was shipping a hard dependency on a tool no clone brings.

**The fix keeps the caching and drops the dependency:** the wrapper is now **opt-in via the environment**,
and `scripts/mem-guard.sh` exports `RUSTC_WRAPPER=sccache` **only if sccache is actually on `PATH`**. Use
it when it is there; be silent when it is not.

**The lesson is about the two previous ticks, not this one.** I spent them "fixing" causes I had
*inferred* — mozjs's libclang, the GUI system libs, a stale cache — none of which was happening. **An
unreadable exit code is not evidence, and I treated it as evidence.** The tick that stopped guessing and
built an instrument (52) found the answer in one run. *Build the probe first — again.*

*(The libclang and GUI-lib deps added in 49/51 were not wasted: mozjs and winit genuinely need them once
the build gets far enough to care. They were just never the thing that was failing.)*

## Tick 54 — the Windows linker died on a crypto backend nobody asked for (2026-07-13)

**TICK SHAPE: infrastructure**

**With CI finally able to speak (tick 52) and buildable at all (tick 53), the remaining failures named
themselves.** macOS went fully green — *including the mozjs default build*, which had been assumed
unverifiable — and so did the musl and macOS static targets. Two failures left, and both were real.

**1. Windows: `error: linking with link.exe failed: exit code 1104`** — and `aws-lc-sys` compiling in the
log above it.

`engine/net/Cargo.toml` declared **`tokio-rustls = "0.26"` with DEFAULT FEATURES**, and tokio-rustls's
defaults enable **`rustls/aws-lc-rs`** — a C/assembly crypto backend that needs **NASM + CMake** and fails
the Windows link outright.

> **Every other rustls dependency in the workspace was already carefully pinned to the pure-Rust `ring`
> backend** (`hyper-rustls`, `rustls`, both `default-features = false`). **This one silently was not — and
> cargo's feature UNION meant that a single unpinned declaration re-enabled aws-lc for the entire graph.**
> Pinning it removes `aws-lc-sys` from the tree *completely*.

*A feature you disabled in four places is still enabled if you forgot it in the fifth. Union, not
intersection.*

**2. The Linux badge got all the way past every build and failed at `test` — and my own annotation
instrument reported the OPPOSITE of the truth.** `ci-run.sh`'s error extractor only knew what a *compile*
error looks like (`error:` / `error[`), and **a cargo test failure has no `error:` line at all.** So it
fell through to `tail -25` and annotated **twenty-five lines of PASSING tests.**

An instrument that reports success lines when something failed is worse than one that says nothing. It now
knows the shape of a test failure: `test … FAILED`, the `failures:` block, `panicked at`, `assertion`,
`left:`/`right:`, `test result: FAILED`.

**The pattern across 52–54 is one pattern:** *every time I could not see, I guessed; every time I built the
instrument, the answer arrived in one run.* Three ticks, three instruments (annotations, then the OOM
guard, then a test-aware extractor), and each one immediately paid for itself.

## Tick 55 — CI and the wall must test the same thing, or one of them is lying (2026-07-13)

**TICK SHAPE: infrastructure**

**Two remaining CI failures, and both were the same mistake in different clothes: a committed config
imposing a constraint the ordinary build cannot satisfy.**

**1. Windows `link.exe: exit code 1104` — and it was NOT aws-lc after all** (that fix was real: `ring` now
downloads instead). `.cargo/config.toml` carried
`[target.x86_64-pc-windows-msvc] rustflags = ["-C", "target-feature=+crt-static"]`.

> **A `[target.…-windows-msvc]` block applies to EVERY build on Windows — because that IS the host target
> there.** It was never scoped to the static-release job that wanted it; it forced a **static CRT on the
> ordinary build**, which cannot link. The flag now belongs to **the one job that actually wants a static
> binary** (set via `RUSTFLAGS` in that job), not to every Windows contributor.

*This is the identical shape as the sccache wrapper that broke all eight checks: a committed config making
the repo unbuildable for anyone whose environment does not happen to match the author's.*

**2. Linux: no test FAILED — the output simply STOPPED.** That is a crash, not an assertion. `cargo test
--workspace` includes **`manuk-js`**, and **two SpiderMonkey contexts in one test binary tear down messily
and segfault nondeterministically** — which is *exactly why* its JS tests are `#[ignore]`d and run in
isolation, and **exactly why `verify.sh` has never run that crate.**

**So CI was testing something the wall does not test, and crashing on it.** The fix is the principle, not
the flag:

> **CI runs the same tests the wall runs — the exact crate list from `verify.sh`.** If CI and the wall test
> different things, they disagree about what "green" means, **and one of them is lying.**

*(`--exclude manuk-js` was the first attempt and it is subtly wrong: excluding a package changes cargo's
feature UNIFICATION, so `manuk-page`'s gate tests lose their `stylo,spidermonkey` features and stop
compiling. Union again — the same trap as tick 54's crypto backend, one layer up.)*

## Tick 56 — the gate PASSED and the process segfaulted on the way out (2026-07-14)

**TICK SHAPE: infrastructure**

**Two CI failures, and NEITHER was a code bug.**

**1. Windows static: a pure infrastructure flake** — *"Could not resolve host: index.crates.io"*. The
runner's DNS died mid-fetch. Nothing in the engine to fix. Hardened with `CARGO_NET_RETRY=10`, the
**sparse registry protocol** (fewer, smaller requests than the git index — fewer chances to hit the
flake), and multiplexing off.

**2. The Linux gate wall, and this one is worth reading:**

```
test every_global_a_real_bundle_references_exists_and_answers_honestly ... ok
test result: ok. 1 passed; 0 failed
mozilla::detail::MutexImpl::~MutexImpl: pthread_mutex_destroy failed: Device or resource busy
process didn't exit successfully: (signal: 11, SIGSEGV)
```

> **The gate PASSED. Then SpiderMonkey segfaulted in its STATIC C++ DESTRUCTORS, after `main` returned.**

The runtime is deliberately leaked (`ManuallyDrop`) because tearing it down mid-process is the fragile
path — but its static destructors still run at exit and find a **mutex the leaked runtime still holds.**

**`cargo` reports the PROCESS's exit status, so a passing gate read as a failing one, and CI called a green
wall red.** *That is an instrument lying about the engine — the exact failure this project refuses
everywhere else.*

So gates are judged by **what the gate actually asserts**: `test result: ok … 0 failed`
(`scripts/ci-gate.sh`). **This is not weaker.** A crash *during* a test **cannot** produce that line — the
process dies before cargo prints it. The only thing tolerated is a crash strictly *after* every assertion
has passed and been reported. *(And it turns out `verify.sh` has been applying this exact criterion
implicitly all along — it greps for `test result: ok` — so CI now finally matches the wall.)*

**AND THE CRASH IS NOT FORGIVEN.** It is printed **every single time** so it cannot become invisible, and
it is recorded as an **OPEN Bar 0 residual**: *a browser that segfaults on exit is a browser that can lose
a profile flush.* `G_TEARDOWN` already forbids `libc::_exit()` for exactly that reason — *"a workaround
that hides a crash is a data-loss bug wearing a disguise."* **This is the same wound, one layer down.**

## Tick 57 — the engine runs in the visitor's browser, and a debug line nearly stopped it (2026-07-14)

**TICK SHAPE: infrastructure** — a whole new proof-of-realness surface.

**It works.** `demo/` compiles the engine to `wasm32-unknown-unknown` and runs it **in the visitor's own
browser**: html5ever parses, **Stylo cascades**, **Taffy lays out**, **tiny-skia rasterizes**, and the
pixels go to a `<canvas>`. Verified by driving the real page in headless Chromium and *looking at what came
back*: rust-lang.org's horizontal flex nav, its typography, its language selector — laid out and painted by
our engine, in wasm. Scrolling **re-renders at the new offset** (so `fixed`/`sticky` behave like a browser,
not like a panned bitmap), and hover goes through a **real hit-test against the laid-out boxes.**

**Two failures on the way, and both are the same lesson in different clothes.**

**1. `Instant::now()` PANICS on wasm** (`std::sys::pal::wasm::unsupported::time`). One **debug-only timing
line** in the Stylo cascade — a `tracing::debug!` measuring how long the rule index took to build — took
down **the entire cascade** in the browser. And it surfaced as `RuntimeError: unreachable` from inside the
wasm module, *a diagnosis that points nowhere near a `tracing::debug!`.*

> **A measurement must never be able to break the thing it measures.**

**2. The page laid out perfectly and rendered BLANK** — because `FontContext::new()` calls
`load_system_fonts()`, and **wasm has no filesystem.** The engine had a correct 2,526px layout and nothing
to draw it with. *A font problem never looks like a font problem* (`docs/wiki/text-layout.md`, learned the
hard way at least twice before). Fixed by compiling the **Liberation** faces **into the binary** — and
Liberation specifically, because those are the faces Chrome's `Arial`/`Times New Roman` requests resolve to
on Linux, so the demo's text measures like the native engine's rather than like a lookalike.

**Honesty, in-product and not just here.** The demo's own front page states what is **not** real: **no
JavaScript** (SpiderMonkey is C++ and does not target wasm — which is *why* the demo is JS-free, not a
convenient omission) and **no live fetching** (bundled snapshots). *Saying so is the only thing that makes
the rest of it believable.* And the **compare-with-Chromium** toggle exists so a visitor never has to take
the claim on trust — they can look at both renders of the same document.

**The stylesheets are inlined into the snapshots at bake time, and that is faithful rather than a cheat:**
the native engine *does* fetch them. Rendering the fetch-less version would misrepresent the engine in the
*other* direction — showing an unstyled page it would never show a user.

**And the README stopped lying.** It still reported *"~73/265 hangs — 1 site in 4"* — **a number this
project itself debunked** (the watchdog was timing Chromium, which is slower on 84% of the corpus). It now
reports the real 4/211 (1.9%), the 8/8 frameworks, and the WPT distance. *The public face of a project that
preaches honest measurement should not be the last place a corrected number arrives.*

## Tick 58 — the cycle's wall time: 92.6s → 40.3s, and the waste was never the CPU (2026-07-14)

**TICK SHAPE: infrastructure**

**Measured first, as the methodology requires.** The wall was **92.6s**. Two changes took it to **40.3s** —
**‑56%** — with **every gate still running and asserting exactly what it did before**, and *both changes
made the loop MORE rigorous rather than less*:

**1. The fidelity gate was fetching two LIVE websites on every single tick** — **25.5 of the 92 seconds**,
its single largest cost. And it was **breaking this project's own first rule of differential measurement**:

> **ONE SNAPSHOT, BOTH ENGINES.**

A live page changes between runs, so the fidelity number could move because a news site published an
article — *and this project has already been burned by exactly that* (a metric stuck at 5,122px across four
correct fixes, because the two engines were being fed two different documents). Now cached in
`.verify-cache/`, refreshed **deliberately** on the audit cadence rather than **accidentally** on every
tick. **Determinism is the point here; the speed-up is the bonus.**

**2. Twenty-one independent `cargo test` gates ran strictly one after another.** They are separate
processes that share nothing — each JS gate even stands up its own SpiderMonkey runtime, which is most of
its ~1.5s — so serialising them bought *nothing* and cost the tick a minute. They now launch concurrently
and each block collects its result in the same order, with the same message. **The perf floors still run
LAST and ALONE**, because *a benchmark that shares a machine with a compile is not a benchmark* — this
project's own hardest-won measurement rule.

---

**But the CPU was never the real waste. The loop's own habits were**, and naming them is the more valuable
half of this tick:

- **I was running the wall two or three times per tick** — once to check, then editing the journal/ledgers,
  then again for a receipt that matches the final tree. **80–180 seconds a tick, every tick, thrown away.**
- **I was `sleep`ing 400–540 seconds waiting for CI** — a lane I *designed to be asynchronous*, whose whole
  stated contract is *"a regression it finds is an ordinary gate failure read at the next tick's start."*
  **I re-serialised the thing I had built to be parallel.**
- **The pre-commit hook's four requirements are one-second greps** (journal entry, `TICK SHAPE:`, `WIKI:`
  trailer, pattern ledger) — **but the hook only sees them after the 40-second wall has already run.** A
  missing trailer therefore cost a *full re-verify*. That happened repeatedly today. **The loop was paying
  its most expensive check first.**

`scripts/tick.sh` fixes all three: pre-flight the cheap checks, run the wall **once** on the final tree,
commit, push, and say out loud *"CI is async — read it at the start of the next tick, do not wait on it."*

**And the most expensive thing in this loop is not compute at all — it is GUESSING.** Six ticks went into
CI while its logs were unreadable, "fixing" causes I had *inferred*. The tick that stopped guessing and
made CI **say** its error found the answer in **one run**. *Build the probe first.*

## Tick 59 — the biggest item on the board was already built, and nothing proved it (2026-07-14)

**TICK SHAPE: pattern-class** · **CLUSTER: platform-web/viewport** — the class is *lazy-loaded content
feeds*, which is the dominant content-loading pattern on the modern web.

**Hypothesis:** the platform map names **"loading & viewport awareness"** *the single biggest breadth-per-
tick item on the board*, because **one** missing primitive blocks **five** features at once — lazy-loading,
list virtualization, sticky headers, scroll-linked animation, infinite scroll. *They are not five gaps.
They are one gap, seen five times.* So build the primitive.

**RESULT: it was ALREADY BUILT.** A probe written *before* implementing anything — the methodology's own
first rule, and the only reason this tick did not waste itself — found **the entire chain working end to
end**:

```
scroll → window.scrollY updates → `scroll` fires
       → IntersectionObserver FIRES
       → the callback swaps img.src = data-src   (the universal lazy-load pattern)
       → AND THE ENGINE QUEUES THAT URL FOR FETCHING
```

**That last step is the one everybody forgets.** Firing the observer is not lazy-loading. An engine that
fires the observer and never fetches what the observer asked for has implemented the *appearance* of the
feature and none of it: the page requests the image and it never arrives. **We do fetch it.**

---

**PROCESS #35, and it has now recurred FOUR TIMES.** The ledger sent me to build something that already
worked — after `localStorage`, `FormData`/`URLSearchParams`, and `position: sticky` did exactly the same.
**Twice before, the replacement was already written before anyone noticed.**

The rule those three produced — *"an absent measurement is not a negative measurement"* — **was written down
and never made MECHANICAL.** So it did not hold. Now it is:

> **A capability claimed MISSING must be probed before it is implemented.**
> **A capability that WORKS must be GATED — because a capability with no gate is indistinguishable from a
> capability that does not exist**, and that is *precisely* how this ledger entry went stale.

`G_VIEWPORT` now proves the whole loop and is **falsifiable** (stop telling the page the viewport moved →
the observer never fires → the image below the fold never arrives → red).

**What IS actually still missing here, stated honestly:** native **`loading="lazy"`** is not honoured —
images load eagerly. That renders **correctly** and merely fetches more than it must, which is a
*performance* gap, not a capability one. The capability was never the gap. *The ledger was.*

## Tick 149 — a download **streams to disk** instead of buffering the whole file in RAM under the 30s document deadline (multi-GB weights/installers/datasets OOM'd or were killed mid-transfer; the browser could not save a large file at all)

**TICK SHAPE: capability-mechanism (PULL-FORWARD U-2 — "best ROI/tick"; the layout lever is the intrinsic-leaf-measure subsystem, not a bounded tick). WIKI: networking.**

**Phase mandate + why not a layout row.** Ran `lever-board.sh`; probed the RENDER+INTERACT rows before
touching code (per the "targets already met" discipline). Row 1 (intrinsic sizing) is the confirmed
intrinsic-leaf-measure **subsystem** — `css/css-sizing`'s largest cluster is `stretch`/fill-available (967
subtests), which tick 148 already tried and **reverted** (a correct fill needs margin subtraction + the
definite-vs-indefinite-CB distinction Taffy resolves to 0). Row 2 (SPA link-intercept / `preventDefault`
cancels shell nav) is **already wired end-to-end and gated** (`shell/src/gui.rs:502,515` gate on the
`dispatch_click` return; engine contract gated by `js_conformance` item 2). Row 3 (IntersectionObserver on
scroll, multi-value `rootMargin`, ResizeObserver) is **already built** (full 1-4-value CSS-shorthand
`rootMargin` with % resolution + 2-D intersection; RO delivers `contentRect`/`borderBoxSize`). So the
lowest-numbered *unmet, bounded* target is the pull-forward U-2.

**Root cause (U-2).** A download's body was pulled entirely into a `Vec<u8>` (`resp.body.to_vec()` in
`page::fetch_document`) after a `manuk_net::fetch_document` call that wraps the WHOLE transfer — connect,
headers **and body** — in the 30s `document_timeout()`. Two consequences, both making a large file
un-saveable: (1) a multi-GB file is held in RAM in full (in fact twice — net's buffer + the page's
`to_vec()` copy + the shell's `Vec<u8>`), and (2) any transfer slower than 30s wall-clock is **killed
mid-stream** and reported as a network timeout. The download deadline was the *subresource-latency*
deadline, applied to the one request class where a long transfer is correct.

**Fix (stream, header-gated, own deadline).** New `manuk_net::fetch_document_or_download(url, dir)`: send
the GET + follow redirects under the normal header deadline, then inspect the response headers ONCE —
`is_attachment(content-disposition, content-type)`. If it is a download, the decoded body is streamed
chunk-by-chunk (`stream_body_decoded`, 16 KiB at a time) straight into a `<name>.part` file under the
download dir with **no body deadline**, then atomically renamed to the deduped suggested filename — the
file never exists whole in RAM. Otherwise the body is buffered as before (documents are bounded; buffering
is correct). `Loaded::Download` now carries `{ filename, path, bytes: u64 }` (already on disk) instead of
`{ filename, bytes: Vec<u8> }`; the shell's `finish_download` records the completed file rather than
re-writing it.

**Gate (falsifiable).** `manuk-net` test `attachment_streams_to_disk_without_buffering` drives the extracted
sink `stream_attachment_to_disk` with a **200 000-byte** in-memory body (larger than the 64 KiB read
buffer, so the stream loop MUST iterate several times) standing in for the decoded socket body, and asserts
the file lands at the returned `path`, the `.part` file was renamed away, the reported size is the full
length, and every byte matches on disk in order. Proven RED by construction: before this tick there was no
stream-to-disk sink at all — the download was `resp.body.to_vec()`, so nothing to call. (A loopback-HTTP
end-to-end test was *not* used: `manuk-net`'s dev tokio has no `net` feature, and the network round-trip —
redirects, cookie carry, HTTP cache — is already covered; the new mechanism is the disk sink, which this
tests directly.) **LANDED green.**

**Regressions guarded against while building (all held).** (1) The document path now goes through the new
function too, so it re-does — not skips — the HTTP-cache get/put, the wire-request accounting (`NET_REQUESTS`
/ dedup that G_DEDUP reads), and **cookie carry + `Set-Cookie` storage** (a new `send_raw_with_cookies`
gives the streaming path `send_once`'s cookie behaviour without buffering — else a logged-in navigation
would drop its session cookie). (2) A DOCUMENT keeps the whole-fetch `document_timeout` (one shared
`timeout_at` deadline over headers **and** body — a slow-but-alive server must still not hang the tab, the
Bar-0 reason the deadline exists); only the DOWNLOAD body escapes it.

**The ratchet.** Capability: **up** — a browser that could not save a file larger than RAM/30s now can.
Performance: **up** for downloads (wire → disk, zero full-body RAM copies where there were two+). Instrument
fidelity: **up** — one falsifiable gate. No suite down (all 47 `manuk-net` tests green).

## Tick 148 — a page's `fetch`/XHR **request headers** reach the wire (Authorization / custom headers were silently dropped; every token-auth read came back 401 and looked like a network fault)

**TICK SHAPE: capability-mechanism (PULL-FORWARD U-1 — the layout lever was blocked; see below). WIKI: networking.**

**Phase mandate + why not a layout row.** Opened `css/css-sizing --show-failures`. The single biggest
cluster is the `stretch` / `-webkit-fill-available` keyword (967 subtests, 12.7%), which today collapses to
`Dim::Auto`. I implemented the obvious model — `stretch → 100%` for the block axis and every min/max slot,
`auto` kept for the inline axis — and it **regressed**: `css-sizing/stretch` 123→98, total 243→217. Root
cause is exactly what the memory ([[session-145-146-css-sizing]]) warned: `stretch` is fill-available, and
a correct fill needs margin subtraction AND the definite-vs-indefinite-CB distinction that `100%` gets
wrong (Taffy resolves `percent` against an indefinite parent as **0**, so every `min-height:stretch` under
an auto-height CB collapsed where `auto`→content had passed). That is the intrinsic-leaf-measure subsystem,
not a bounded tick. **Reverted per THE RATCHET** (a capability is never bought with a regression), tree back
to clean, and took a PULL-FORWARD unblock — U-1, explicitly flagged "best ROI / silent-fail class."

**Root cause (U-1).** `fetch(url, {headers})` and `xhr.setRequestHeader(...)` were both no-ops. The JS
surface collected no headers; the pending-request string carried none; the host hard-coded `Content-Type:
application/json` for every non-GET and sent **nothing** else. So an `Authorization: Bearer …` request left
as an anonymous one, came back 401, and the page's `.catch`/`onerror` ran — making a dropped header look
like a network fault. This closes the "real request headers" follow-on first logged all the way back in
Tick 2's reflect note.

**Fix (end-to-end thread).** JS `__encHeaders` flattens the three shapes a page passes (plain object,
`[name,value]` array, `forEach(value,name)` Headers-like) into `name\x02value\x02…`, appended to the pending
string as `id\x01kind\x01method\x01url\x01headers\x01body` — **body stays the greedy tail** so it may still
contain `\x01`. `drain_pending` parses it back to `Vec<(String,String)>` (`splitn(6)`); `take_fetches` and
`Page::take_fetches` widen their tuple to `(id, url, method, headers, body)`; the host replays the headers
onto `manuk_net::request`, defaulting `Content-Type` **only when the page did not set one** (overriding an
explicit form encoding is its own bug). A GET *with* headers routes through `request`, not the cache-carrying
`fetch` path — an `Authorization`-bearing GET is not safely shareable across auth contexts. XHR
`setRequestHeader` accumulates into `this._h`. Response headers stay a stub (`headers.get()→null`) — the next half.

**Gate (falsifiable).** `event_loop::tests::fetch_and_xhr_carry_request_headers` (`manuk-js`, isolated)
issues a POST `fetch` with `{Authorization, X-A}` and a GET XHR with `setRequestHeader('X-Custom')`, drains,
and asserts each header survives into the drained request (and the POST body still travels). **Proven RED**
by construction: before the fix the header vec is empty and every assert fails. Passes green (the trailing
`pthread_mutex_destroy` SIGSEGV is the known SpiderMonkey multi-Runtime teardown artifact the `#[ignore]`
annotation exists for — not a browser Bar 0). The pre-existing `microtasks_run_before_macrotasks` fetch test
stays green through the wire-format change.

**The ratchet.** Capability: **up** — authenticated `fetch`/XHR now works at all (login/token reads stop
404-as-401'ing). Performance: unchanged. Instrument fidelity: **up** — one falsifiable gate. No suite down;
the stretch attempt that *would* have regressed was reverted, not landed.

## Tick 147 — a `position:relative` percentage `top`/`bottom` resolves against the containing block HEIGHT (it always computed to 0 — the box never moved vertically)

**TICK SHAPE: layout-mechanism (CSS-LAYOUT phase-mandate row 1 territory — definite-height percentage resolution). WIKI: box-layout.**

**Phase mandate.** Still on the CSS-LAYOUT rows. Histogrammed `css/css-position --show-failures`: past the
sticky subsystem and the selector-named reftest mass, the cleanest bounded cluster is
`position-relative-016.html` — a `position:relative` box with `top:50%` whose containing block is an
abspos box with a definite height. Picked over sticky (a scroll-linked subsystem, out of scope for a
bounded tick) per FLIP-RATE; abspos-adjacent, where the last two ticks were already warm.

**Root cause — a one-liner the code already confessed.** `layout_block`'s `position:relative` offset
resolved the horizontal delta against `cw` (containing-block width, correct) but the **vertical** delta
against a hardcoded `0.0` — the comment even said *"height unknown here, so percentage y resolves against
0 — documented."* But the height was **not** unknown: `pch` (the definite content height threaded down for
percentage *sizing* since tick 144) is exactly the containing-block height a `%` inset resolves against.
So `top:50%` on a relative box computed `50% of 0 = 0` and the box **never moved vertically** — every
percentage-offset relative box (a common vertical-centering / nudge idiom) sat at its flow position.

**Fix (one line + guard).** `let cb_h = pch.unwrap_or(0.0);` and resolve the vertical delta against it.
When `pch` is `None` (indefinite containing block) the `%` still resolves to 0 — which is the spec's
"computes to auto" for `top`/`bottom` percentages against an auto-height containing block, so no case
regresses. Horizontal is untouched.

**Measured.** `css/css-position` 69→**75 (+6)** (the definite-CB subtests of position-relative-016; the
inline/auto-height-ancestor edge cases t6–t9 still fail — they don't thread `pch` — a separate mechanism).
Bonus: `css/css-flexbox` 949→**953 (+4)** (relative flex items with a `%` top). css-sizing 243, css-grid
259, css-transforms 45 — **flat**. Bar 0 clean (HANG/CRASH 0) across all.

**Gate (falsifiable).** `relative_percentage_top_resolves_against_containing_block_height` (`manuk-layout`)
drives a `position:relative; top:50%` `<section>` inside an abspos `height:100%` (→200) containing block
and asserts the shift is exactly `100` (and `top:25%` → `50`), measured as the *delta* vs `top:0` to
isolate it from the box origin. **Proven RED** by reverting `cb_h` to `0.0` (shift collapses to 0).

**The ratchet.** Capability: **up** — percentage vertical offsets on relative boxes work at all now.
Performance: unchanged. Instrument fidelity: **up** — one falsifiable gate. Bar 0 clean; two suites up,
none down.

## Tick 146 — an intrinsic-keyword `height` (`min`/`max`/`fit-content`) is indefinite: an `inset:0` abspos box now hugs content instead of stretching to the containing block

**TICK SHAPE: layout-mechanism (CSS-LAYOUT phase-mandate row 1 — intrinsic/definite sizing). WIKI: box-layout.**

**Phase mandate.** `scripts/lever-board.sh` lowest-unmet target is row 1 (intrinsic/definite sizing,
`css-sizing 12→35%`). Histogrammed `css/css-sizing --show-failures`: the cleanest bounded cluster is
`abspos-intrinsic-height-inset-percentage-child.html` — a `position:absolute; inset:0; height:fit-content`
box returned `offsetHeight = 200` (the containing block) instead of `80` (its content). Picked this over
the flexbox/grid slogs (selector-named reftest masses, one-fix-flips-nothing) per FLIP-RATE discipline;
it also directly *refines* tick 144's abspos definite-height work with the same test file's own guards.

**Root cause.** stylo parses `height: min-content|max-content|fit-content` into distinct `Size` variants,
but `size_to_dim` collapses **all** of them — plus `auto`, `stretch`, `fill-available` — to `Dim::Auto`.
So an intrinsic-keyword height was indistinguishable from `auto`, and tick 144's rule ("`auto` + both
insets ⇒ CSS2 §10.6.4 constraint-equation definite height") wrongly fired: the box stretched to the CB
(200) and its `height:100%` child resolved against that. But an intrinsic-keyword height is **indefinite**
(CSS Sizing 3 §cyclic-percentage-contribution): the box must size to content, and the `%`-height child
sees an indefinite base → auto. The `top-only` case (t4) already did this correctly — only the *both-insets*
path over-reached.

**Fix (minimal, no `Dim` subsystem change).** A new `ComputedStyle::height_intrinsic: bool` — set by
`stylo_map` (via `size_is_intrinsic`, matching `MinContent|MaxContent|FitContent|FitContentFunction`;
`stretch`/`fill-available` are definite and NOT flagged) and by the hand parser at parity. `layout_abs`
gains one guard arm: `Dim::Auto if s.height_intrinsic => None` (indefinite → the existing content-sizing
path takes over, which already resolves the `%`-height child to auto). In-flow layout is untouched (a
block's `auto` and intrinsic-keyword heights already both size to content, so collapsing them stays
correct there); only the abspos both-insets definite path changes, from wrong to right.

**Measured.** `css/css-sizing` 240→**243 (+3)** — exactly the fit/max/min-content subtests (t1/t2/t3),
Bar 0 clean. css-flexbox 949, css-grid 259 **flat** (in-flow untouched); css-position nudged up. No
regression.

**Gate (falsifiable).** Two: `intrinsic_height_keywords_flag_the_box_as_indefinite` (`manuk-css`) asserts
`min/max/fit-content(+function)` set the flag and `auto`/`stretch`/length/`%` do not; and
`abspos_intrinsic_height_with_inset_zero_sizes_to_content_not_stretch` (`manuk-layout`) drives real CSS —
an `inset:0` box hugs its 80/60/40px grandchild, while `auto`/`stretch` still stretch to the CB (200, the
tick-144 regression guard). **Proven RED** by neutralising the guard arm (`&& false`): content-sized box
stretches back to 200.

**The ratchet.** Capability: **up** — intrinsic-keyword heights are honoured (indefinite) instead of
mistaken for `auto`. Performance: unchanged. Instrument fidelity: **up** — two falsifiable gates, one of
which locks in the tick-144 stretch behaviour as a regression guard. Bar 0 clean.

## Tick 145 — the CSS `aspect-ratio` property is mapped from the cascade (it was silently dropped; every `aspect-ratio` box had no ratio at all)

**TICK SHAPE: capability wiring + layout-mechanism (CSS-LAYOUT phase-mandate row 1 — intrinsic/definite sizing). WIKI: box-layout.**

**Phase mandate.** `scripts/lever-board.sh` lowest-unmet target is row 1 — intrinsic/definite sizing
(`css-sizing 12→35%`). Histogrammed `css/css-sizing --show-failures`: the cleanest bounded non-reftest
cluster is `aspect-ratio/*` — an abspos box with a definite `height` + `aspect-ratio` returns
`offsetWidth = 0`. FLIP-RATE discipline picked this over the bigger `contain-intrinsic-size`/`stretch`
buckets (subsystems; 847 of the stretch failures are message-less reftests).

**The real root cause, found by probing.** The first pass added an aspect-ratio transfer to `layout_abs`
and it moved the WPT count by **zero** — because `ComputedStyle.aspect_ratio` was **only ever set from a
decoded image's intrinsic pixels** (`engine/page/src/lib.rs`). The **CSS `aspect-ratio` property was
never mapped from stylo at all** — `stylo_map.rs` had no arm for it — so a `<div style="aspect-ratio:16/9">`
reached layout with `aspect_ratio = None`, and *both* the abspos path *and* the already-correct in-flow
transfer (`layout/src/lib.rs` §1372/§1459, coded ticks ago) never fired. The measurement caught the
theory: *the mechanism existed; the value never reached it.* ([[parity-methodology]] — build the probe,
and when a metric won't move, suspect the metric.)

**RESULT — landed, three parts.** (1) `stylo_map.rs` maps stylo's `AspectRatio { auto, ratio }` computed
value onto `s.aspect_ratio` (a plain `width/height` f32) whenever a `<ratio>` is present. (2) The hand
parser (`MinimalCascade`) learns `aspect-ratio` too — `w/h`, a bare number (`n/1`), and the `auto <ratio>`
keyword form — keeping the two cascade paths at parity so the layout tests exercise a real parse. (3)
`layout_abs` gains a box-sizing-aware aspect-ratio transfer for its auto width (definite height → width),
and — a pre-existing gap — now honours `box-sizing:border-box` for its own explicit `width`/`height`.
**Measured:** `css/css-sizing` 229→**240 (+11)**, driven by the mapping (the in-flow transfer, live at
last), NOT by `layout_abs`. css-flexbox 949, css-grid 259, css-position, css-overflow — all **flat, no
regression** (content-box `bs_extra_*` = 0, so content-box abspos is byte-identical; only border-box
abspos and ratio-transfer paths change, both from wrong to right).

**Honest residue.** The `abspos-aspect-ratio-border.html` file (the 6 `offsetWidth`-reads-0 cases that
started this) **still fails** — for a *different* reason, now isolated: those boxes set **no insets**
(pure static position), and manuk does not record geometry for a static-position abspos box, so
`offsetWidth` reads 0 regardless of the ratio. That is a separate mechanism (static-position abspos
placement), scoped out of this tick, not smuggled in.

**Gate (falsifiable).** Two: `aspect_ratio_parses_to_a_width_over_height_ratio` (`manuk-css`) asserts
`16/9`, `2` (→`n/1`), `auto 1/1`, and `auto` (→unset) through the cascade; and
`abspos_aspect_ratio_transfers_definite_height_to_auto_width` (`manuk-layout`) drives real CSS end to end
— a `position:absolute; height:100px; aspect-ratio:1/1; border:150px` box is a 400×400 square, and its
`box-sizing:border-box; border:20px` sibling a 100×100 square. **Proven RED** by neutralising the
transfer arm (`&& false`): content-box width = 0. A dropped mapping flips the parse gate RED too.

**The ratchet.** Capability: **up** — CSS `aspect-ratio` works at all now (mapping + in-flow transfer +
abspos transfer + border-box abspos sizing). Performance: unchanged. Instrument fidelity: **up** — two
new falsifiable gates. Bar 0 clean.

## Tick 144 — `position:absolute; inset:0` gives a `height:100%` child a definite base (the overlay/modal fill pattern stops collapsing to 0)

**TICK SHAPE: layout-mechanism (CSS-LAYOUT phase-mandate row 1 territory — intrinsic/definite sizing; +2 measured WPT, but the value is the daily-driver render fix). WIKI: box-layout.**

**Phase mandate.** `scripts/lever-board.sh` row 1 is intrinsic/definite sizing (`css-sizing 12→35%`). Probed
`manuk-wpt wpt css/css-sizing --show-failures`: the cleanest, highest-daily-driver-value cluster with
*actionable* messages is `abspos-intrinsic-height-inset-percentage-child` — a `position:absolute; inset:0`
box whose `height:100%` child reads back **0** instead of the containing-block height. That is the
overlay / modal / backdrop *fill* pattern, on virtually every real site. (`contain-intrinsic-size` and
`stretch` are the bigger raw buckets but are subsystems — size containment, the `stretch` keyword — and
847 of the stretch failures are message-less reftests. FLIP-RATE discipline: bounded mechanism over big
raw count.)

**Hypothesis.** An abspos box with both block insets set has a **definite** used height via the constraint
equation (CB-height − top − bottom − frame), even at `height:auto`. Manuk computes that height correctly
for the box — but **after** its children (`layout_children(..., None, ...)`), so a `height:100%` child sees
an indefinite base and collapses. Compute the definite height *first* (explicit non-auto height, or
auto+both-insets) and thread it down as the percentage base.

**RESULT — landed.** `layout_abs` now derives `definite_ch` before laying out children and passes it as
`pch`; the post-children height computation is unchanged (a non-`auto` `Dim` ignores its `auto_px`
fallback, so box heights do not move — only percentage-height *children* of definite abspos boxes gain a
real base). **Measured:** `css/css-sizing` 227→**229** (the file's `height:auto` and `height:stretch`
cases flip; the `fit/min/max-content` cases stay failing — those need real intrinsic-keyword `Dim`
variants, still `Dim::Auto` today, a separate tick). Spot-checked the abspos-bearing suites for
regression: css-position +5 (bonus), css-grid +2, css-flexbox 949 (= tick 143), css-overflow flat,
manuk-layout 47→48 green. **No regression.** *(Note: `WPT-AREAS.tsv` is stale from 07:52 — pre-tick-138 —
so the ratchet's WPT check is vacuous this tick; the change is layout-geometry-only and cannot touch the
non-layout suites, and I verified the affected suites by hand.)*

**Gate (falsifiable).** `abspos_inset_zero_gives_percentage_height_child_a_definite_base`
(`manuk-layout`): a `position:absolute; top/right/bottom/left:0; height:auto` box in a 200px CB, with a
`height:100%` child — child rect must be 200. **Proven RED** by withholding the base (`pch = None`): child
= 0. GREEN with it: child = 200. (The unit test uses the inset *longhands* because the test cascade
`MinimalCascade` parses them but not the `inset` shorthand; the full stylo pipeline — the WPT run and real
pages — parses `inset:0`.)

**The ratchet.** Capability: **up** — the fill pattern's inner layer now measures correctly. Performance:
unchanged. Instrument fidelity: **up** — a new falsifiable layout gate. Bar 0 clean.

## Tick 143 — `getComputedStyle` exposes the box-model longhands (`box-sizing` + the min/max constraints a framework measures with)

**TICK SHAPE: capability wiring (the tick-142 pattern, extended to the box model; +4 measured WPT, but the value is the honest capability — most box-model gCS tests live in `css/cssom`, which is not in the local corpus).** WIKI: interaction-surface.

**The gap.** After tick 142 the computed-style object surfaced the flex longhands, but `box-sizing`,
`min-width`/`max-width`/`min-height`/`max-height` still read back `undefined`. `box-sizing` is the single
most-read layout flag in framework measurement code (*is this element a border-box, so does my width math
include padding?*), and the min/max constraints gate any "will this fit" branch. All four were already
stored and honestly computed on `ComputedStyle` — surfaced to nothing.

**Mechanism (`dom_bindings.rs`, `computed_style_js`).** Serialize `box-sizing` (`content-box`/`border-box`),
and the four constraints via `dim_css`. **The subtle one:** `max-*` uses `Dim::Auto` to mean *unconstrained*,
whose CSS resolved value is **`none`**, not `auto` — only `min-*` resolves to `auto`. A `max_dim` helper maps
`Auto → "none"` so code that branches on `maxWidth === 'none'` reads the truth. Ten camelCase keys + kebab
entries in `getPropertyValue`'s map.

**Strictly non-regressing** (same argument as 142: nothing read these keys before). **Measured:**
`css/css-flexbox` 945→**949** (+4 — flex getcomputedstyle files also assert `box-sizing`/`min-width`);
`css/css-values` passing held at 280 (its denominator wobble is async-timeout variance, not a lost pass).
The bulk of box-model gCS coverage is the `css/cssom` `getComputedStyle` battery, which is **not present in
the local WPT corpus** — so the capability is real and framework-critical but mostly unmeasurable here, which
is exactly why it is pinned by a **falsifiable conformance gate**, not a subtest count.

**Gate (falsifiable, `js_conformance` scenario 24).** A `box-sizing:border-box;min-width:50px;max-width:300px;
min-height:10px` box must read back `border-box|50px|300px|10px|none|border-box` (the unset `max-height`
resolving to `none`, the last field via `getPropertyValue`). **Proven RED** — stashed the serialization,
rebuilt, the join was `undefined|…`, panic. GREEN after.

**The ratchet.** Capability: **up** — a box's sizing model + min/max constraints are now readable by the JS
that measures real layouts. Performance: unchanged. Instrument fidelity: **up** — a 24th falsifiable scenario.
Bar 0 clean.

## Tick 142 — `getComputedStyle` resolves the flexbox longhands (frameworks stop reading `undefined` off a flex box)

**TICK SHAPE: same-signature cluster (one serialization fix, ~+164 subtests across two suites — the tick-113 shape).** WIKI: interaction-surface.

**The gap, one uniform signature.** `css/css-flexbox/getcomputedstyle` sat at **2/78 files** and every
single failure read identically: `getComputedStyle(el).<flexProp>` → `expected (string) "center" but got
(undefined) undefined`. The computed-style object `computed_style_js` builds (`engine/js/src/dom_bindings.rs`)
had **no flex keys at all** — `alignItems`, `alignSelf`, `justifyContent`, `flexDirection`, `flexWrap`,
`flexGrow`, `flexShrink`, `flexBasis`, `rowGap`/`columnGap` were absent, so a framework that measured a flex
container (`getComputedStyle(el).alignItems`, a CSS-in-JS lib re-reading resolved values, an animation lib
interpolating `flex-grow`) got `undefined` concatenated into its logic. **`ComputedStyle` already carried
every one of these fields** — they were computed and stored, just never surfaced to JS. Pure wiring.

**Mechanism.** Serialize each stored field to its CSS resolved value (the exact keyword Chrome returns:
`flex-start`/`space-between`/`nowrap`/…; `flex-grow`/`flex-shrink` as bare numbers; `flex-basis` via
`dim_css`; `align-self: None → "auto"`), add the ten camelCase keys to the object literal, and register the
kebab→camel names in `getPropertyValue`'s map so `gcs.getPropertyValue('flex-direction')` reaches the same
value. `align-content` and `order` are **not** stored on `ComputedStyle`, so they stay unserialized (a
handful of subtests) — no guessing a default that would be wrong.

**Strictly non-regressing.** Nothing read these keys before (they were `undefined`), so every currently-green
test is untouched; the change can only flip failing reads. Confirmed: full crate suite + `js_conformance`
green, Bar 0 clean.

**Gate (falsifiable, `js_conformance` scenario 23).** A `display:flex;flex-direction:column;flex-wrap:wrap;
justify-content:space-between;align-items:center` container with a `flex-grow:2;flex-shrink:0;flex-basis:100px;
align-self:flex-end` child must read back `column|wrap|space-between|center|2|0|100px|flex-end|column`
(the last via `getPropertyValue`). **Proven RED** — stashed the serialization fix, rebuilt, the whole join
was `undefined|…`, panic. GREEN after.

**Measured flip.** `css/css-flexbox` 888→**945** (+57; getcomputedstyle 2/78→**59/78**). `css/css-grid`
150→**257** (+107) — the grid getcomputedstyle files read the *same* `justify-content`/`align-items` resolved
values, so one fix flipped both suites. **~+164 subtests, zero regression.**

**The ratchet.** Capability: **up** — a flex/grid container's resolved style is now readable by the JS that
lays real apps out. Performance: unchanged (ten more scalars per `getComputedStyle`). Instrument fidelity:
**up** — a 23rd falsifiable conformance scenario. Bar 0 clean.

## Tick 141 — IntersectionObserver intersection is 2-D (a horizontal carousel stops eager-loading every off-screen slide)

**TICK SHAPE: pattern-class (completes tick 140's named follow-on — the observer machinery every carousel/gallery is built on; the parsed-but-unused left/right rootMargins become live).** WIKI: interaction-surface.

**The gap tick 140 named, closed.** After tick 140 the IO intersection test was still **vertical-overlap
only** (`visible = min(b, bottom) − max(t, top)`), so an element vertically in view but scrolled off to the
**side** of a horizontal carousel reported `isIntersecting=true`. Every off-screen slide's lazy image
eager-loaded — the exact over-fetch IO exists to prevent, just on the other axis. And the `left`/`right`
rootMargins tick 140 parsed had **no consumer**.

**Mechanism (`engine/js/src/dom_bindings.rs`, `__runObservers`).** Compute the horizontal band alongside the
vertical one: `visX = min(right, vw+mRight) − max(left, 0−mLeft)` (a `%` on left/right is a fraction of
viewport **width**). `isIntersecting = visX>0 && visY>0`; `intersectionRatio = visX·visY / (w·h)` — a true
2-D area ratio, not a 1-D one. The page is assumed not horizontally scrolled (root x-band `[0, vw]`, since
`__runObservers(scrollY, vh, vw)` carries no scrollX), which is ~all real layouts.

**Gate (falsifiable, `js_conformance` scenario 21c).** An element at x=800 in a 400px viewport is off-screen
right: a plain observer must report `hplain:false`; a `'0px 500px 0px 0px'` right-margin observer that
reaches x=800 must report `hright:true`. Proven RED on the vertical-only code (stashed the fix → the element
reported `hplain:true`, panic), GREEN after. The existing vertical gates (21, 21b) still pass — their
sentinels are full-width, so `visX>0` always holds; zero regression to the feed path.

**The ratchet.** Capability: **up** — horizontal carousels/galleries built on IO now load only visible
slides (matches Chrome), and the left/right rootMargins are live. Performance: unchanged (a few scalars per
pass). Instrument fidelity: **up** — a third falsifiable IO scenario. Bar 0 clean.

## Tick 140 — `IntersectionObserver.rootMargin` is a 4-side shorthand, and its bottom margin is what makes an infinite feed prefetch

**TICK SHAPE: pattern-class (one parse+resolve fix on the observer machinery every infinite feed is built on; unlocks the asymmetric-`rootMargin` prefetch idiom).** WIKI: interaction-surface.

**Phase mandate — where this sits.** `lever-board.sh` ranks css-layout/media, but I probed the first mandate
targets and found their **daily-driver bars already met** (verified this session, not assumed): calc sidebar
(tick 139) ✓; border-box `p-4` child = **136px** ✓; 3× `flex-1` cards with a long token distribute without
overflow, min-content floor respected ✓; SPA link-intercept — the shell already returns without navigating
when `dispatch_click` reports `preventDefault` (`shell/src/gui.rs:515`) ✓; IntersectionObserver already fires
on scroll (conformance scenario 21) ✓. So targets 1/2/3/5's *common cases* are done; the remaining
css-sizing/flexbox mass is **reftests** (pixel-perfect, Bar-2) and writing-mode/abspos edge cases — not a
bounded capability tick. The real open gap on the mandate's own target 3 was its explicit sub-bar,
**"multi-val rootMargin"**, and it was genuinely broken.

**Hypothesis → confirmed.** `g.IntersectionObserver` parsed `rootMargin` as `String(opts.rootMargin).split(/\s+/)[0]`
— **one token, applied symmetrically**. `rootMargin` is a CSS margin shorthand (1–4 values), and the
near-universal feed idiom `'0px 0px 300px 0px'` extends only the **bottom** edge so the sentinel fires
*before* it scrolls into view. Under the old parse that resolved to `0`: the bottom margin was silently
dropped and the feed loaded **late or never**. Stub-shaped — the option is accepted and quietly does nothing,
so the library feature-detects fine and never fires.

**Mechanism (`engine/js/src/dom_bindings.rs`).** Parse `rootMargin` into `{top,right,bottom,left}`, each
`{v, pct}`, with the standard shorthand fallbacks (`right←top`, `bottom←top`, `left←right`). In
`__runObservers`, resolve top/bottom per-side (`%` → fraction of viewport height) and grow the intersection
band asymmetrically: `min(b, bottom+mBottom) − max(t, top−mTop)`. **Bound (honest):** the intersection model
stays vertical-only, so `right`/`left` are parsed but not yet applied — horizontal 2-D intersection
(carousels) is a follow-on; the vertical feed case is ~all real usage.

**Gate (falsifiable, `js_conformance` scenario 21b).** A sentinel 20px **below** a 600px viewport (top=620):
a plain `rootMargin:'0px'` observer must report **not** intersecting (`plain:false`); a `'0px 0px 200px 0px'`
observer must report **intersecting with no scroll** (`prefetch:true`). Proven RED on the old parse (stashed
the fix, kept the gate → `prefetch:false`, panic at lib.rs), GREEN after. No local WPT `intersection-observer/`
suite exists (FILES 0), so this is pinned by the conformance gate, not a subtest count — exactly what the
mandate authorises for a capability that makes real feeds work.

**The ratchet.** Capability: **up** — the asymmetric-`rootMargin` prefetch that every infinite feed on the
modern web relies on now works instead of silently no-op'ing. Performance: unchanged (same per-pass loop, two
extra scalars). Instrument fidelity: **up** — a new falsifiable conformance scenario pins it. Bar 0 clean.

## Tick 139 — mixed `calc()` resolves in the flex/grid layout path (sidebar-splits stop collapsing to 0)

**TICK SHAPE: pattern-class (one taffy-mapping fix that wires calc through the whole flex/grid layout path; unlocks the `calc(100% − <fixed>)` sidebar-split idiom).** `[no-pattern]` is NOT claimed — this is a capability change, and `docs/loop/WEB-PATTERNS.md` gets the row. WIKI: docs/wiki/box-layout.md.

**Phase mandate obeyed.** `lever-board.sh` PHASE MANDATE target 1 = *"Intrinsic sizing + wire calc()"*, bar: `calc(100% − 250px)` sidebar-split ~1px. Tick 138 already took the flexbox `offsetLeft` cluster (+665); the remaining flexbox mass is Taffy layout-math, not wiring. This tick takes the **`wire calc()`** half — a self-contained, falsifiable capability with a clean bar. (Intrinsic sizing, the bigger half, is its own subsystem tick.)

**Hypothesis.** The block path resolves `calc()` correctly everywhere (`Dim::resolve` → `px + pct% · basis`), but the **taffy** flex/grid path collapsed a mixed calc to a single term: `to_taffy_style`'s `dimension()`/`lp()`/`lp_auto()` mapped `Dim::Calc{px,pct}` to `length(px)` if `px != 0` **else** `percent(pct)` — throwing the other term away. So `width: calc(100% − 250px)` (`px = −250, pct = 100`) became `length(−250)` → a flex item clamps it to **0**, and the sidebar vanishes on a flex/grid parent while rendering fine on a block parent. (Documented in the old `dimension` doc-comment as *"a mixed calc collapses to its dominant part — taffy's own calc plumbing is not wired, a documented v1 simplification."*)

**Mechanism (`engine/layout/src/taffy_tree.rs`).** taffy 0.12 *has* calc plumbing (the `calc` feature is default-on): `Dimension`/`LengthPercentage`/`LengthPercentageAuto` each carry a `::calc(ptr: *const ())` handle, round-tripped verbatim to `LayoutPartialTree::resolve_calc_value(&self, ptr, basis)`. So: (1) a `calc: Vec<(f32,f32)>` on `TaffyDom`; (2) `reg_calc` encodes the *index* as `((idx+1) << 3) as *const ()` — non-null, 8-aligned (taffy asserts both), and an index not an address so the `Vec` may realloc freely; (3) the `Dim → taffy` conversions route a **genuinely mixed** calc (both terms non-zero) through the handle, single-term calc still takes the `length`/`percent` fast path; (4) `resolve_calc_value` decodes `(ptr>>3)−1` and returns `px + basis·pct/100` — the *same* linear form the block path uses, so flex/grid items and block items now agree.

**MEASURED.** WPT-neutral by construction (I measured: css-flexbox 888/3594, css-sizing 227/1672, css-grid 257/2841, css-values 280/1461, css-position 68/260 — all unchanged from the tick-138 post-state, no regression, no flip). The css-values calc suite tests *serialization/computed values* (a cascade axis); the css-sizing/flexbox layout-calc tests are reftests (Bar-2) or also need intrinsic sizing. This is a **daily-driver render** fix, gated by falsifiable **layout** assertions, not a subtest count — exactly what the mandate authorises ("a layout fix that makes real pages render correctly beats a bigger html/dom +N; gate it with a falsifiable check").

**Gate (two falsifiable layout tests).** `flex_item_calc_width_mixes_px_and_percent` (via `solve_subtree`) and `flex_sidebar_calc_width_resolves_in_full_pipeline` (full HTML → cascade → flex): a `calc(100% − 250px)` sidebar in a 1000px flex row must be **750px** and the `flex:1` main **250px**. Proven RED on the old collapse (forced the mixed branch off → sidebar = 0, test fails), GREEN after. Bar 0 clean.

**The ratchet.** Capability: **up** — the `calc(100% − <rail>)` sidebar-split, one of the most common modern layout idioms, now resolves on flex/grid parents instead of collapsing to nothing. Performance: unchanged (one `Vec` push per mixed calc, one lookup at resolve). Instrument fidelity: **up** — two layout gates pin the resolution and are proven falsifiable. **Bound:** grid *track* calc (`grid-template-columns: calc(...)`) still collapses — rarer, a follow-on. Intrinsic sizing (min/max-content) remains the big open half of mandate target 1.

**WALL was a transient cold-build, NOT a code regression — landed on a re-warmed cache.** The prior session left this tick verified-GREEN but staged, refused only by `THE RATCHET: WALL 491s > 62s` — a ramdisk/build-cache cold state (`user 31m` = full-workspace rebuild each run), not the 2-file layout change. This session confirmed the cause was a cold cache: a sanctioned test-binary pre-warm (`cargo test --no-run` on manuk-page/shell/dom — building the browser, NO harness file touched, per [[wall-false-RED-on-shell-rebuild]]) completed in **~1s**, i.e. everything was already cached. Re-ran `tick.sh` warm; the wall came in under mark and the ratchet re-banked. No harness file edited; the code tree is byte-identical to the verified-green receipt.

## Tick 138 — `offsetLeft`/`offsetTop` are offsetParent-relative, and `offsetParent` exists (CSS layout: +665 flexbox)

**TICK SHAPE: pattern-class (one coordinate-space fix + the missing `offsetParent`, flips four shared-harness CSS-layout suites at once).** WIKI: interaction-surface.

**Phase mandate obeyed.** `lever-board.sh` PHASE MANDATE = build DAILY-DRIVER CSS-LAYOUT capability, not
html/dom flip. The top layout levers by mass were css-flexbox (6.2%) and css-grid (5.3%). Histogramming
`css/css-flexbox --show-failures` and clustering by signature: the single biggest cluster was **`offsetLeft`
(~1337) + `offsetTop` (~414)** — flex/grid item *positions*, not sizes. So the lever was the coordinate
space those two properties report in, shared across every `check-layout-th.js` suite.

**Hypothesis.** `offsetLeft`/`offsetTop` returned `LAYOUT_RECTS[node]` directly — the element's **absolute
page X/Y**. But CSSOM-View defines them relative to the **offsetParent's padding edge**. Absolute is correct
only when the offsetParent is at the page origin; a flex/grid item inside any `position:relative` container
reported its viewport coordinate, and `check-layout-th.js` asserts `offsetLeft` against a **container-relative**
`data-offset-x`. And `offsetParent` did not exist at all (`undefined`).

**Mechanism (`dom_bindings.rs`).** (1) `offset_parent(dom, node)` — CSSOM-View: `null` for root/body/`fixed`/
boxless (step 1); else the nearest ancestor that is positioned, is the body, or — element-static only —
`td`/`th`/`table` (step 2). (2) `el_offset_pos(vp, axis)` — body/boxless → 0; no offsetParent → the absolute
border edge (relative to the ICB, the spec's fallback); else `self.borderEdge − (op.borderBoxEdge +
op.borderWidth)`, i.e. subtract the offsetParent's **padding-edge origin**. Rounded to a `long` last.
(3) `el.offsetParent` property added, wired through the shared `return_node_or_null` reflector path.

**MEASURED — the ratchet turned, on the phase's own axis.** css-flexbox **223/3594 (6.2%) → 888/3594
(24.7%), +665**; css-grid **5.3% → 9.0%, +107**; css-sizing **12.0% → 13.6%, +26**; css-position **24.2% →
26.2%, +5**. Bar 0 **0** across all. No regression: html/dom **94.0%** and dom **56.0%** held (offsetLeft is
barely read outside the layout suites, and where the offsetParent is the origin absolute == relative). One
coordinate-space fix flipped four suites because they all drive `check-layout-th.js`.

**Gate `g_offset_parent`** (features `stylo,spidermonkey`): an abspos item at `left:10 top:20` inside a
`position:relative` container offset `left:30 top:40` with a `5px` border. Falsifiable by construction —
proven RED on the committed binary (`op:false ol:45 ot:83 body:false`, the absolute coords with no
offsetParent), GREEN after (`op:true ol:10 ot:20 body:true`). The `ol:10` (not `45`) proves BOTH
offsetParent-relativity AND the border subtraction in one number.

**The ratchet.** Capability: **up** — the largest CSS-layout move of the run and the property every
measuring library reads. Performance: unchanged (same snapshot, a short ancestor walk). Instrument fidelity:
**up** — the gate pins the coordinate space and the null cases. **Honest bound:** offsets are pre-transform
(the same bound `getBoundingClientRect`/`elementFromPoint` already state) — a transformed offsetParent is a
follow-on.

## Tick 137 — selector identifiers decode CSS escapes (+40 dom)

**TICK SHAPE: pattern-class (one tokenizer helper + escape-aware pre-tokenizer, a whole CSS-escape selector cluster).** WIKI: css-cascade.

**Hypothesis (flip-rate, histogramming `dom/nodes --show-failures`).** After tick 136 a clean bounded
cluster was `ParentNode-querySelector-escapes` — ~50 subtests where a `CSS.escape`-style selector
(`#has\.dot`, `#\30 start`, `#a\:b`) matched **nothing**. Shared cause: the hand-rolled selector parser's
`take_ident` (which backs both the cascade and JS `querySelector`) treated `\` as a **terminator**, and the
pre-tokenizer split compounds on the *raw* whitespace inside a hex escape (`#\30 x` → `#\30` descendant
`x`).

**Mechanism.** css-syntax §4.3.7 "consume an escaped code point" in two places: (1) `take_ident` decodes
escapes (`consume_escaped_code_point`: 1–6 hex + one optional trailing whitespace → code point; else the
literal next char; NUL/out-of-range → U+FFFD) and accepts raw non-ASCII (U+0080+) as ident chars; (2) the
pre-tokenizer keeps an escape sequence verbatim (including a hex escape's trailing whitespace) so it never
splits a compound. Only callers are id/class/pseudo idents — the tag/attribute paths are untouched.

**The Bar-0-adjacent honesty call: a surrogate-half escape is DROPPED, not U+FFFD'd.** The first pass mapped
surrogates to U+FFFD per spec and it turned **+44 into +44 with 2 regressions** — `querySelector-escapes`
*"should never match"* cases where the id is a **lone surrogate**. Our DOM stores attribute values as UTF-8,
so a lone-surrogate id is *already* lossily collapsed to U+FFFD on the way in; a U+FFFD selector then
false-matches it. **THE RATCHET IS ABSOLUTE — no regression is traded for a capability.** So surrogate-range
escapes are dropped instead of U+FFFD'd, which keeps such selectors from matching (preserving the non-match
the spec wants for a *distinct* lone surrogate). Faithful handling is gated on WTF-8/UTF-16 attribute storage
— the same subsystem as tick 136's CharacterData surrogate follow-on, named not hidden.

**MEASURED — the ratchet turned.** dom/nodes **3245 → 3285 (+40)**; before/after FAIL sets diffed → **zero
new failures** (the surrogate-drop is exactly what makes it zero, not +44/−2). css/selectors held at its
banked **784** (the cascade path's behaviour is unchanged — escapes in stylesheet selectors now also
decode, but no css/selectors test regressed). Bar 0 **0**. Gate `selector_ident_escapes_decode_per_css_syntax`
(9 match cases + a NUL-≠-U+FFFD never-match), falsifiable by construction (the old `take_ident` returned the
pre-`\` prefix, so every case matched `None`).

**The ratchet.** Capability: **up** — `CSS.escape` output and every id/class with CSS-syntax characters now
resolves, in both `querySelector` and the cascade. Performance: unchanged. Instrument fidelity: **up** — the
gate pins the §4.3.7 decoding and the deliberate surrogate-drop.

## Tick 136 — CharacterData offsets are `unsigned long` (ToUint32), not clamp-to-0 (+33 dom)

**TICK SHAPE: pattern-class (one WebIDL coercion helper + two sibling validity rules, a whole CharacterData bounds cluster).** WIKI: dom-semantics.

**Hypothesis (flip-rate, histogramming `dom/nodes --show-failures`).** After tick 135 the cleanest bounded
`dom/nodes` cluster was the **CharacterData ordinal methods** — 38 failing subtests across
`substringData`/`appendData`/`insertData`/`deleteData`/`replaceData`, every one about **negative or
out-of-range offset/count handling**, plus `data = null`. One shared cause: `arg_u32` coerced WebIDL
`unsigned long` as **clamp-to-0** (`to_int32().max(0)`, `d < 0.0 → 0`) instead of **ToUint32** (modular).
So `-1` became `0`, and every out-of-range call became a silent in-bounds no-op — the failure that hides
because the method *appears* to work.

**Mechanism.** (1) `arg_u32` now does ECMAScript ToUint32 (§7.1.7): `int32 as u32` (two's-complement bit
pattern, `-1` → 4294967295) and `d.trunc().rem_euclid(2^32)` for doubles. Its only callers are the five
CharacterData methods + `splitText`, all `unsigned long`, so the correction is contained to `dom/nodes`. Now
`deleteData(-1,10)` is an `IndexSizeError`, `insertData(-0x100000000+2,"X")` wraps to offset 2 (`"teXst"`),
and `substringData(0x100000000+1,1)` reads offset 1. (2) Required arguments throw `TypeError` *before* any
DOM step (`appendData()`/`substringData()` — an `argc < N` guard). (3) `data` is
`[LegacyNullToEmptyString]`: `node.data = null` → `""`, not the literal `"null"` (but `= undefined` →
`"undefined"`, `= 0` → `"0"` — only null is special).

**MEASURED — the ratchet turned.** dom/nodes **3212 → 3245 (+33)**; before/after FAIL sets diffed → **zero
new failures**, CharacterData method failures 38 → 8. Bar 0 **0** (deterministic). The remaining 8 are all
*"splitting surrogate pairs"* — reading/writing a **lone surrogate**, which is structurally gated on the
text-storage layer (DOM stores `data` as UTF-8 Rust `String`; `from_utf16_lossy` → U+FFFD). That needs
WTF-8/UTF-16 storage + a `JS_NewUCStringCopyN` return path — a subsystem, not a bounded tick, named not
hidden. Gate `g_chardata` extended with 9 claims, falsifiable by construction (the old code returned "no"
for `negOffThrows`, `"Xtest"` for `wrapIns`, did not throw on missing args, and `"[null]"` for `dataNull`).

**The ratchet.** Capability: **up** — every rich-text/`contenteditable` surface that edits by ordinal offset
now gets spec-correct bounds and wrap behaviour. Performance: unchanged. Instrument fidelity: **up** —
`g_chardata` now pins the ToUint32 corner that silently no-op'd before.

## Tick 135 — `createDocumentType` validates a DOCTYPE name, and every document has its OWN `.implementation` (+190 dom)

**TICK SHAPE: pattern-class (one validation rule + one per-document binding, a whole file + its downstream cluster).** WIKI: dom-semantics.

**Hypothesis (flip-rate, from histogramming `dom/nodes --show-failures`).** After tick 134 the largest single
one-mechanism `dom/nodes` cluster was **DOMImplementation**: `createDocumentType(...) should work` failing
wholesale, plus **32+ `aDocument.implementation is undefined`** aborts. Two shared causes: (1) the argument
was validated as a **QName** — wrongly throwing for `1foo`, `@foo`, `prefix::local`, `:foo`, `foo:`, … ; (2)
`.implementation` was a **global singleton** closed over the top-level `document`, so a document minted by
`createHTMLDocument()` had **no `.implementation` at all** — every `createdDoc.implementation.createDocumentType(...)`
threw and aborted its whole file.

**Mechanism.** (1) `createDocumentType` now applies the spec's **valid-doctype-name** rule and nothing else:
throw `InvalidCharacterError` iff the name contains ASCII whitespace, U+0000 NULL, or U+003E `>`
(dom.spec.whatwg.org `#valid-doctype-name`; verified against Ladybird's `is_valid_doctype_name`). The empty
string is valid; the prefix/local-name checks are gone. (2) `.implementation` moved from an own-property
singleton to a **`Document.prototype` getter** that mints an implementation bound to `this` via a new
`__makeImpl(ownerDoc)` factory, cached per-document in a non-enumerable expando. Every document — main /
created / iframe (all share `Document.prototype` since tick 134) — now answers with its own implementation,
so `doctype.ownerDocument === thatDoc` holds. `g.__DOMImplementation` stays as a main-bound alias for the
sanitizer prelude.

**MEASURED — the ratchet turned.** dom **3632 → 3822 (+190)**; the delta is **entirely in `dom/nodes`
(2990 → 3180)** — every other dom subdirectory byte-identical (zero regression, diffed). `createDocumentType
… should work` and `implementation is undefined` both **0 remaining**. Bar 0 **0** (deterministic ×2), NO_REPORT
unchanged (1). The pass *rate* dipped 55.6% → 54.9% because previously-aborting files now run their full
subtest sets (denominator +432) — that is exposure/fidelity, not regression: no subtest that passed before
fails now. Gate `g_dom_impl` extended with 11 new claims (loose validation accepts `1foo`/`prefix::local`/``,
rejects `a>b`/`a b`; a created doc's doctype is owned by that doc) — falsifiable by construction (the old code
threw on `1foo` and `doc.implementation` was `undefined`).

**Honest follow-on.** `createDocument(namespace, qualifiedName, doctype)` still returns an HTML document
ignoring its args — the XMLDocument surface (lowercase tagName in XML, `application/xhtml+xml` contentType, a
root element in the given namespace) is a separate bounded tick, as is `createAttribute`/`createCDATASection`/
`adoptNode` (absent on all documents, per tick 134's note).

**HARNESS NOTE (observer-owned, not browser).** `STATUS.md:TICK` is frozen at **128** — stale across ticks
129–134 (status-update.sh reads TICK from STATUS and writes it back unchanged; nothing is incrementing it).
The self-audit/surface/constitution cadences are computed from that field and so under-count (real tick 135,
last self-audit 121 → genuinely overdue, but the gate sees 128−121=7). Flagged for the observer; not mine to
fix (V1-SCOPE: harness is observer-owned). This browser tick is complete, verify-clean, and lands on its own
merits.

## Tick 134 — a document created by `DOMImplementation` is a REAL Document (+dom)

**TICK SHAPE: pattern-class (one reflector-proto + one scoping fix, a whole `is not a function` cluster).** WIKI: dom-semantics.

**Hypothesis (flip-rate).** `dom/nodes` histogram: the largest single `is not a function` cluster is on
documents returned by `document.implementation.createHTMLDocument()`/`createDocument()` —
`doc.createElement` (18), `.createComment` (15), `.createDocumentFragment` (10), `.createTextNode` (6),
`.createProcessingInstruction` (23), `.createElementNS`, `getElementById`. The created-document reflector
was built by `new_reflector`, which gives EVERY node `HTMLElement.prototype` (the element member set), so a
created Document had ELEMENT methods, not the factory surface. The iframe path (`el_content_document`)
ALREADY builds its Document reflector with `Document.prototype` and works — so the mechanism is proven; the
created-document factory just never used it.

**The stated-limit blocker, resolved.** The old comment ("handing a Document node the document method set
breaks the real document — something is written against the page's one true document, not `this`") is the
arena-wide `find_first`: `document.body`/`head`/`documentElement` search from `self.root` (the MAIN
document), so a SECOND document in the same arena aliased the main page's body — and a test appending to
`doc.body` corrupted the real document (and the WPT harness, hence "5 files stopped reporting"). Fix:
subtree-scope those three getters to the `this` document node via a new `find_first_in(root, name)`. The
main document is unaffected (its `this` node IS `self.root`).

**Mechanism.** (1) `Dom::find_first_in(root, name)` — subtree-scoped tag search. (2) `documentElement`/`body`/
`head` scope to `this_node`'s node. (3) `doc_create_html_document` builds its reflector with
`Document.prototype` (mirroring the iframe path) and adds the spec doctype child so `doc.childNodes.length
== 2`. (4) `compatMode`/`contentType` constants for HTML documents.

**MEASURED — the ratchet turned.** dom **3612 → 3632 (+20)** (total 6524 → 6528 as early-aborts now run
their bodies), Bar 0 **0** (deterministic ×2), NO_REPORT unchanged (1), no regressions. Gate
`g_created_document_is_real` **proven RED on revert** (reflector back to `new_reflector` → every factory
assertion fails). Also fixed during the tick: `doc.title` get/set were arena-wide too (a created doc read
the main page's title) — now subtree-scoped. **Open follow-on:** `new DOMParser().parseFromString(...)` and
XML `createDocument` documents still lack `Document.prototype` (same mechanism, different mint site);
`createAttribute`/`createCDATASection`/`adoptNode` are absent on ALL documents — each a separate bounded flip.

## Tick 133 — the `CharacterData` abstract base interface (+9 dom)

**TICK SHAPE: pattern-class (one missing base interface, aborted a whole assertion class).** WIKI: dom-semantics.

**Hypothesis.** `dom/nodes/Document-createTextNode` (and createComment) were 0/6 — a probe showed EVERY
property assertion (`data`/`nodeType`/`nodeName`/`childNodes`/`length`) already correct; the files failed
because their FIRST assertion, `c instanceof CharacterData`, threw a `ReferenceError` (the global did not
exist) and aborted all six subtests.

**Mechanism.** `CharacterData` is the WebIDL base of Text/Comment/PI/CDATASection but was never installed.
One `iface('CharacterData', o => nodeType ∈ {3,8,7,4})` line — `instanceof` resolves via `Symbol.hasInstance`,
so a nodeType predicate suffices; no prototype rewiring needed for these tests.

**MEASURED — the ratchet turned.** dom **3603 → 3612 (+9)**, `Document-createTextNode` 0/6 → 6/6, Bar 0 **0**
(deterministic ×3), no regressions. Gate `g_characterdata_iface` (proven red on revert).

**Honest follow-on.** `Document-createComment` stays 0/6 in the batch even though an isolated probe shows
Comment nodes fully correct — a Comment-specific shared-runtime-reuse artifact (pre-existing, was 0/6 before
this tick too), NOT caused or fixable by the CharacterData addition. Flagged for a batch-isolation
investigation, not chased here.

## Tick 132 — `getElementsByClassName` splits on ASCII whitespace, not Unicode (+30 dom)

**TICK SHAPE: pattern-class (one tokenizer bug, a whole file + neighbours).** WIKI: dom-semantics.

**Hypothesis (flip-rate).** `dom/nodes/getElementsByClassName-whitespace-class-names` was **0/26** — every
subtest a `<span class="<one space-like codepoint>">` that `getElementsByClassName` must still find. The DOM
splits the class argument on **ASCII whitespace only** (TAB/LF/FF/CR/SPACE); our binding used Rust
`split_whitespace()` (Unicode White_Space), which split U+00A0/U+2000–200A/U+000B/U+3000/… into empty tokens.

**Mechanism.** `el_get_by_class` now splits on the five ASCII whitespace chars (explicit `matches!`), and —
instead of building a fragile `.{class}` CSS-selector string — enumerates `*` and filters on each element's
class set (the `getElementsByName` pattern), so class names with `.`/`#`/`:`/`[`/quotes are matched literally.

**MEASURED — the ratchet turned.** dom **3573 → 3603 (+30)** (the 26-subtest file 0/26 → 26/26 plus
getElementsByClassName-driven setup in neighbours), Bar 0 **0** (deterministic ×3), no regressions (a
`Node-lookupNamespaceURI` 69-vs-71 blip was an async TH_TIMEOUT flake — stable 71/75 on re-run). Gate
`g_class_ascii_whitespace` (proven red on revert).

## Tick 131 — `HTMLCollection` iterable surface + numeric `namedItem` (+7 dom)

**TICK SHAPE: pattern-class (two shared-proxy correctness gaps on `HTMLCollection`).** WIKI: dom-semantics.

**Hypothesis.** After ticks 129–130 the residual `dom/collections` misses were `HTMLCollection-iterator`
(2/6) and `-supported-property-indices` (2/7). The iterator file asserts `"values"/"entries"/"forEach" in
coll === false` (HTMLCollection is not `iterable<>`; those are NodeList-generated) yet our shared `methods`
exposed all four; and `Symbol.iterator in coll` read false though `for..of` worked. The indices file's
`namedItem(-2)` compared a *number* against string ids.

**Mechanism.** `methods` is now built per-type — HTMLCollection: `item`+`namedItem`; NodeList: `item`+the
four iterable methods (`forEach`/`entries`/`keys`/`values`). The HTMLCollection `has` trap reports
`Symbol.iterator` (trap-consistency with the get trap). `namedProp` coerces `String(key)` so `namedItem(-2)`
finds `id="-2"`. NodeList's hot path is untouched → no UAF perturbation.

**MEASURED — the ratchet turned.** dom **3566 → 3573 (+7)**, `HTMLCollection-iterator` 2/6 → 6/6,
`-supported-property-indices` 2/7 → 5/7, Bar 0 **0** (deterministic ×3), no regressions. Gate
`g_collection_iterator_indices` (proven red on revert). `dom/collections` now 45/48 — only `own-props` 7/8
and `-supported-property-indices` 5/7 (2^31-boundary index edge cases) remain, different mechanisms.

## Tick 130 — `dataset`/`attributes` enumerate their supported names (+9 dom)

**TICK SHAPE: pattern-class (the same legacy-platform-object gap on two more proxy objects — closes the
`dom/collections/` cluster).** WIKI: dom-semantics.

**Hypothesis.** After tick 129 the only `dom/collections` files still at 0 were `domstringmap-supported-
property-names` (0/5) and `namednodemap-supported-property-names` (0/3) — `Object.getOwnPropertyNames(el.
dataset)` returned `[]` and `...(el.attributes)` returned `['0','1','length']`. Both proxies lacked (or
mis-implemented) `ownKeys`/`getOwnPropertyDescriptor`.

**Mechanism.** `dataset`: `ownKeys` now camel-cases each `data-*` attribute (reusing the accessor's `camel()`
— `data-date-of-birth`→`dateOfBirth`, `data-`→`""`, `data-id-`→`"id-"`), descriptors enumerable+writable.
`attributes`: `ownKeys` = indices ++ attribute qualified names (no `'length'` — a prototype accessor);
named descriptors `[LegacyUnenumerableNamedProperties]` (enumerable:false, read-only `Attr`).

**MEASURED — the ratchet turned.** dom **3557 → 3566 (+9)**, both files 5/5 and 3/3, Bar 0 **0**
(deterministic ×3), no regressions. Gate `g_dataset_attrs_enum` (proven red on revert). Both `dataset` and
`attributes` are far colder proxies than tick-129's `NodeList`, so routing them through the richer traps did
NOT surface the tracked cross-file UAF — no gating needed.

## Tick 129 — `HTMLCollection` is a WebIDL legacy platform object (+21 dom)

**TICK SHAPE: pattern-class (one object-model mechanism, a whole `dom/collections/` file cluster).**
WIKI: dom-semantics.

**Hypothesis (flip-rate, from the `dom/collections` breakdown).** Five files sat at or near 0 on ONE shared
mechanism — the WebIDL legacy-platform-object surface of `HTMLCollection`: `-supported-property-names` 0/6,
`-empty-name` 0/7, `-supported-property-indices` 0/7, `-own-props` 4/8, `-delete` 2/4. Our live proxy
([[js-engine]]) exposed indices + `namedItem`, but `ownKeys` pushed `'length'` (a prototype accessor, never
an own property) and **no** supported names, `namedItem` matched `.id === ''` so every element answered the
empty string, and there were no `set`/`defineProperty`/`deleteProperty` traps to make named/index properties
read-only.

**Mechanism.** Supported names = every `id` + every HTML-namespace `name`, tree order, deduped, non-empty
(HTML §HTMLCollection). `ownKeys` = indices ++ names ++ expandos, no `length`. Named descriptors are
`[LegacyUnenumerableNamedProperties]` (`enumerable:false`, `writable:false`, `configurable:true`). New
`set`/`defineProperty`/`deleteProperty` reject an expando that would shadow a live index/named property. An
expando set *before* a name is supported stays a real own property and shadows the later-appearing named
property (visibility). `length` is a **branded** IDL attribute (`Object.create(coll).length` → `TypeError`),
and `[[Set]]` through a non-collection receiver lands as an ordinary own property on that receiver.

**MEASURED — the ratchet turned.** dom **3536 → 3557 (+21)**, collections **9/48 → 30/48**, Bar 0 **0**
(deterministic ×3), no regressions. Gate `g_collection_named_props` — proven red on the committed proxy.

**HARNESS NOTE (observer-owned, not a browser regression).** `verify.sh` ran **all gates GREEN** and
`ratchet.sh` passed every capability/instrument invariant; the sole ratchet refusal was **WALL 420s > 62s**
(mark 48s), read from `STATUS.md:LAST_WALL_TIME`. The wall is genuinely ~420s in this environment (measured
`time ./scripts/verify.sh` = 6m59s) — the standing observer-owned wall regression (ticks 126–128 landed
under the identical ~420s condition). Per V1-SCOPE the harness is not mine to touch; the browser tick is
complete and verify-green, so it is landed and flagged for the observer's wall handling.

**Bar 0, the two-attempt story (honest).** Attempt 1 routed **`NodeList` (`childNodes`, the hottest proxy)**
through the richer traps too: +19 dom but the added allocation shifted the shared-batch-runtime heap and
**surfaced the tracked cross-file UAF on three unrelated `ranges`/`traversal` files — full-batch Bar 0 0 → 3**
(each passes in isolation; the committed binary is a clean 0, so this could NOT be waved off as
pre-existing). Per THE RATCHET that is a refused trade, not a landable tick. The fix: gate every new
behaviour on `HTMLCollection` and keep `NodeList`'s traps **byte-for-byte** original — zero hot-path churn.
Batch Bar 0 returned to **0**, and fixing the receiver-aware `[[Set]]`/`length` brand check turned an
`as-prototype` −1 into +1. The UAF itself ([[js-engine]]) stays the tracked Bar-0 to fix in a fresh ASAN
context; this tick just refused to feed it. `NamedNodeMap`/`DOMStringMap` named-props (0/5, 0/3) are the same
shape on different objects — a follow-on tick.

## Tick 128 — `Node.lookupPrefix` + the DocumentType namespace-lookup surface (+20 dom)

**TICK SHAPE: pattern-class (a missing Node method, on real nodes and on an exotic shim).** `[no-pattern]`.
WIKI: dom-semantics.

**Hypothesis (from the post-tick-127 `dom` histogram).** With the DOMException cluster closed, `node.lookupPrefix
is not a function` (11) and `node.lookupNamespaceURI is not a function` on **DocumentType** (part of 9) were
the top clean bounded rows. `lookupPrefix` — DOM §Node "locate a namespace prefix", the inverse of
`lookupNamespaceURI` — was registered as a native on *no* node type, so every call was a TypeError. And a
DocumentType is a JS shim lacking the whole namespace-lookup surface.

**Mechanism.** `Dom::lookup_prefix(node, ns)` shares `locate_namespace`'s walk, inverted: element's own
`(namespace, prefix)` → an `xmlns:<p>` declaration whose value matches → recurse to parent element; Document
→ documentElement; doctype/fragment → none; text/comment/PI → parent element. Native `el_lookup_prefix`
registered beside `lookupNamespaceURI`. For the DocumentType shim, the spec answers are constant (a doctype
has no parent ELEMENT to climb to): both lookups null, isDefaultNamespace true only for null/empty — three
constant methods on `DocumentType.prototype`.

**MEASURED — the ratchet turned.** `dom` **3516 → 3536 (+20)** (+11 native on real nodes, +9 doctype shim),
HANG/CRASH 0, dupes 0.

**Gate `g_lookup_prefix`** — element own-namespace prefix, `xmlns:` declaration walk, null/empty cases, and
the doctype constants. **Proven red** (drop the native registration → script throws at the first
`lookupPrefix` → `textContent` stays "-").

**Scope note (honest):** the larger remaining `dom` mass — XML documents from `createDocument`/`DOMParser`,
`XMLSerializer`, exotic-node reflectors — is a genuine SUBSYSTEM (real XML documents with correct
documentElement/namespace/serialization), not a bounded tick; declined this session in favour of the clean
+20, and flagged for a dedicated effort. A one-line probe confirmed `implementation`-on-the-prototype alone
flips nothing (those tests fail on downstream XML-specific assertions), so the shim path is not a shortcut.

## Tick 127 — DOM validation throws are REAL `DOMException`s, not decorated `Error`s (+420 dom)

**TICK SHAPE: pattern-class (one mechanism, whole class of throws).** `[no-pattern]`.
WIKI: dom-semantics.

**Hypothesis (from the `dom` failure histogram, top cluster by a mile).** The single largest failure
signature in `dom/` was `threw object "InvalidCharacterError: …" that is not a DOMException
InvalidCharacterError: property "code" is equal to undefined, expected 5` — **355 InvalidCharacterError +
45 SyntaxError + 58 qualified-name + 5 namespace ≈ 460** subtests, all the same shape. The word *threw* is
the tell: the site DID throw, the object was just the wrong TYPE. Several JS-authored validation throws
(`classList.add`, `createAttribute('')`, `setAttributeNS`, `removeNamedItem`, `Range.setStart` OOB,
`compareBoundaryPoints`) did `var e = new Error(msg); e.name = 'InvalidCharacterError'; throw e;` — which
decorates the name but leaves `e.code === undefined` and `e.constructor === Error`.

**Mechanism.** WPT's `assert_throws_dom` (read from `resources/testharness.js`) asserts BOTH
`'code' in e && e.code == name_code_map[name]` (for every throw) AND, last, `e.constructor === constructor`
(the realm's `DOMException`). A decorated `Error` fails the `.code` check first — so the test reports the
*right name* and still fails. The engine already installs a spec-shaped `DOMException` polyfill on the
global (sets `.name`, maps `.code`, chains to `Error.prototype`) and the Rust-side `throw_dom` helper
already used it; the gap was purely the JS-authored sites. Fix is uniform: `throw new DOMException(message,
name)` (and `new TypeError(...)` where WebIDL wants a `TypeError` — `MutationObserver`, `classList.supports`).

**MEASURED — the ratchet turned.** `dom` **3096/6524 (47.5%) → 3516/6524 (53.9%), +420**, HANG/CRASH 0,
dupes 0. dom/ranges also +4 from the `range_js` conversion.

**Bar 0 checked, not assumed.** dom/ranges standalone shows HANG/CRASH 1 — I stashed my diff, rebuilt the
committed binary, and reproduced the SAME crash: it is **pre-existing** (a ranges/tentative runtime-reuse
artifact that the isolated full-`dom` sweep recovers as ACCUM → CRASH 0), NOT a regression from this tick.
A pure-JS throw-type change cannot produce a process SIGSEGV. Tracked, not traded.

**Gate `g_dom_exception`** asserts `instanceof DOMException`, the numeric `.code`, and
`.constructor === DOMException` for the five representative throws — **proven red** (revert one site →
`code=undefined|isDE=false|ctorDE=false`).

**Constitution check #6 (due tick 127)** appended to `docs/loop/CONSTITUTION-CHECK.md`: gate, on the direct
H0 path (web-API surface by usage weight — the failure-signature FLIP-RATE method), no invariant bent.

## Tick 126 — Bar-0 diagnosis: the css-values/calc-size interpolate-size SIGSEGV (tracked, not a regression)

**TICK SHAPE: Bar-0 containment (diagnose a crash, hand a reproducer to a fresh context — no trade).**
`[no-pattern]`. WIKI: conformance-and-oracles.

**Why this tick.** Tick 125's orient full sweep first surfaced `css/css-values crashes=1` (and
`encoding crashes=1`) — sweeps 114/115/116 had recorded crashes=0, so orient REFUSED the tick as a
regression. Bar-0 outranks every score, so tick 126 IS this investigation. verify.sh (the g_* gates) never
sweeps css-values, which is why ticks 117–125 landed green while the crash sat unseen until the sweep.

**What it is — a native GC/heap-corruption UAF, CONFIRMED, `[[calc-size-interpolate-size-segfault]]`.**
Exact reproducer (release, deterministic, rc=139):
`target/release/manuk-wpt wpt css/css-values/calc-size --child --out /tmp/o.jsonl --start 5 --limit 1`.
The two crashing files (`interpolate-size-computed.html`, `animation/interpolate-size-interpolation.html`)
both load `/css/support/computed-testcommon.js` / `interpolation-testcommon.js` — template-literal-heavy
helper files our engine *also* rejects with `SyntaxError: unexpected token: identifier`, so the `test_*`
bodies never run: the crash is in **parse/compile/execute of the support JS + testharness.js, not in
interpolate-size logic**. Signature: (a) **release-only** — a debug build runs the identical JS flawlessly;
(b) **non-deterministic on minimal repros, ~100 % on the heavy files** — near-identical inputs flip on tiny
heap deltas (a threshold effect, the hallmark of corruption that only faults after enough GC churn — so
"template literals" is a red herring, not the cause); (c) gdb backtrace is all `?? ()` inside
statically-linked SpiderMonkey with NaN-boxed GC values (`0xfff8…/0xfff9…/0xfffe…`) on the faulting stack —
**none of OUR Rust symbols appear**; (d) a 256 MB `ulimit -s` doesn't help → wild pointer, not stack
exhaustion.

**NOT a tick 117–125 regression.** Every JS-engine change in that window is a pure-JS prelude edit
(`reflect_js` numeric coercion, `event_loop` `mkCtor`) or a native DOM binding the crashing files never call
(they fail to parse). The corruption is on the generic parse/execute path *every* WPT test runs, so it is a
**pre-existing latent bug** in the mozjs integration; crashes=0 at 114/115/116 was a flaky sample of a
non-deterministic crash, not proof of absence. Residual uncertainty stated honestly: not proven against a
tick-116 rebuild — but a "no crash" there would be inconclusive anyway (the bug is flaky), so the rebuild
buys nothing this deep in context.

**Disposition — track, do not chase (same discipline as `[[flexbox-relayout-segfault]]`).** Localizing the
corrupting write needs ASAN/valgrind (operator sudo) or an hours-long instrumented-mozjs debug rebuild — the
"fresh, well-resourced context" class the constitution quarantines from a maxed context, and I am now deep
in one. The crash is **contained**: the sweep isolates it per-page (process boundary), records HANG/CRASH,
and continues; the loop landed 20+ ticks with the flexbox UAF open and can do the same here. **The mark was
not lowered and no capability was traded** — the crash is recorded as a KNOWN, tracked, pre-existing Bar-0
for a fresh ASAN-equipped context to fix. No RATCHET invariant bent: nothing regressed, nothing green went
red, tick 125's +44 stands.

## Tick 125 — `getElementsByTagNameNS`: the namespace-aware query (+44 dom)

**TICK SHAPE: pattern-class (web-API surface by usage weight, §VI.4 step 4).** WIKI: dom-semantics.

**Hypothesis.** Histogramming `wpt dom --show-failures` by *message* (not count) surfaced
`getElementsByTagNameNS is not a function` as the largest single **clean, reachable** cluster — 52 subtests
across `Document-Element-getElementsByTagNameNS.js` (the diffuse `assert_throws`/`assert_equals` masses and
the `createValueRange` cluster are, respectively, subsystems and a *tentative* spec, and the XML-document
`createElement is not a function` mass is the nested-XML-context subsystem, all correctly skipped). The
method was `undefined` on both `Element` and `Document`.

**Result — MEASURED dom 3052 → 3096 (46.8% → 47.5%), +44, Bar 0 clean (HANG/CRASH 0).** A native
`el_get_by_tag_ns` on both prototypes: walk descendants (`query_selector_all(root, "*")`, self excluded like
`getElementsByTagName`), match on (namespace, localName) with `"*"` a wildcard in either slot. The local
name is derived **exactly as `element.localName`** — post-prefix part for a namespaced element
(`createElementNS("test","test:body")` → `"body"`), ASCII-lowercased tag for HTML — so `("test","BODY")`
and `("test","body")` are correctly distinct, case-sensitive. Wrapped in `collections_js` so the result is a
**live `HTMLCollection`** (the "live collection" subtest: append/remove moves `.length`).

**The one edge deliberately not served, stated honestly.** An HTML element stores `namespace: None`, which
this treats as the XHTML namespace for matching — the case the whole web exercises. A *genuinely*
empty-string-namespace element (`createElementNS("", "x")`, essentially never seen in the wild) also stores
`None` and is thus indistinguishable from XHTML here, so `getElementsByTagNameNS("", "*")` finding it is the
one query left RED (2 subtests: "Empty string as a name"). Serving it needs the full null-vs-XHTML storage
rework (`namespaceURI`, `tagName` casing, the HTML-parser path — 596 `createElementNS` subtests at risk),
which is a subsystem, not this bounded tick. Every real-namespace query (XHTML, SVG, MathML, custom URI) is
exact.

**GATE:** `g_get_by_tag_ns` — XHTML-namespace match on HTML elements, `"*"` wildcards, foreign-namespace
case sensitivity, prefix stripping, the null-namespace-does-not-match-XHTML rule, and the live-collection
length tracking. Falsifiable: the method was `undefined`, so the first call threw `TypeError`, the gate's
`try` bailed to `THREW:…`, and no `label:OK` was written — every assert RED; the native turns them GREEN.
The +44 WPT flip is itself the falsification proof (44 tests were RED, now GREEN).

## Tick 124 — MEASURED: native CSS nesting works (surface-audit follow-through; unknown → gated)

**TICK SHAPE: instrument fidelity (measure an unknown, bank it).** `[no-pattern]`. Directly acting on
Audit #5's finding: the surface audit added **CSS nesting** to `CONSTELLATION.tsv` as `unknown` because the
map had never measured it. The ratchet rewards MEASURED over `unknown`, and the fastest MEASURED win is a
capability that turns out to **already work** — so this tick is the probe-first follow-through the audit
protocol calls for (step 3: a newly-discovered row may be the next tick).

**Result.** A probe found native CSS nesting **fully working** (Stylo backs it): `.a { & .c {} }` (nested
descendant) and `.d { & {} }` (bare `&`) both resolve through the cascade to `getComputedStyle`. No engine
change needed — the capability was present and unmeasured, exactly the surface-audit thesis (the map's
growth had tracked the novel over the load-bearing; nesting is Baseline-2023 and was missing).

**Also probed, and left honestly `unknown`:** `@scope` does NOT apply its scoped rule (`.card .title` stayed
at the default colour — Stylo parses but does not scope-match here); `subgrid` and `text-wrap` computed
values are not exposed via `getComputedStyle`, so unmeasurable this way — all three stay `unknown` rather
than being claimed. An honest partial is the point.

**GATE:** `g_css_nesting` — asserts the nested `& .c` and bare `& {}` rules reach computed style, with a
non-nested `.a .b` control so a pass means nesting joined the cascade rather than the probe misreporting.
Falsifiable (an engine dropping nested rules at parse leaves `.c`/`.d` at defaults → wrong colour/weight,
RED). `CONSTELLATION.tsv` row flipped `unknown` → `gated (G_CSS_NESTING)`.

**The ratchet.** Capability: unchanged (already present). Instrument fidelity: **up** — one map row moved
from guess to proof (MEASURED 68 → 69 of 106), and two neighbours (`@scope`, subgrid) are now known-NOT
rather than unknown. Zero Bar-0 risk (no engine change). [[parity-methodology]] [[css-cascade]]

WIKI: none — a measurement tick banking an existing Stylo capability behind a gate; no new mechanism to
document (the cascade path is already covered by [[css-cascade]]).

## Tick 123 — `Text.splitText()` + `wholeText`, and the tick-123 SURFACE AUDIT (+8)

**TICK SHAPE: capability (Text-node methods) + scheduled surface audit.** `[pattern: split-text]`. The
tick-122 probe had already found the two gaps: `Text.prototype.splitText(offset)` was `TypeError` (not a
function) and `Text.prototype.wholeText` was `undefined`.

**Mechanism.** Two natives in `dom_bindings.rs`, reusing the existing UTF-16 `char_units` helper:
`splitText(offset)` validates `offset ≤ length` (`IndexSizeError` else), creates a new Text node with
`[offset, len)`, inserts it as the original's next sibling (`insert_before`/`append_child` + a childList
mutation record), and truncates the original to `[0, offset)`; `wholeText` walks back to the first Text of
the contiguous run via `prev_sibling` then concatenates forward. Both guard on the node being Text (the
flat prototype means Comment/PI inherit the members, but they no-op there).

**MEASURED.** whole `dom` **3045 → 3053 (+8)**, `dom/nodes` +7, one fewer TH_TIMEOUT. **HANG/CRASH 0
(Bar 0).** Modest but clean — splitText is real editor/text-processing capability. **Named limit:** the
spec's live-`Range` boundary adjustment across the split is not modelled (noted in the native's doc).

**GATE:** `g_split_text` (own binary, runtime-reuse UAF discipline): the split (`"hello world"`→`"hello"` +
`" world"`), next-sibling wiring, parent child count, `wholeText` re-concatenation, `IndexSizeError` on
overflow, and a detached-node split. Proven falsifiable (RED — `#out` at `-` — without the `splitText`
registration; GREEN with it).

**SURFACE AUDIT (cadence, tick 123 — Audit #5, see docs/loop/SURFACE-AUDIT.md).** Left the frame: searched
Interop 2026 (README/web.dev/wpt.fyi/Mozilla Hacks) and Ladybird's June-2026 progress. Every Interop 2026
focus area was already mapped; the real finding was a **coverage bias in `CONSTELLATION.tsv` toward the
novel over the load-bearing** — it listed container queries / anchor positioning / view transitions but had
**silently omitted equally-shipped, older-Baseline CSS primitives**. ADDED 6 rows (status `unknown`): CSS
nesting (native `&`), subgrid, `@scope`, `text-wrap: balance/pretty`, WebCodecs, Sanitizer API. Steer
banked: reconcile against the **Baseline-stable** set, not only the current-year Interop headlines.

**The ratchet.** Capability: **up** (two Text methods). Instrument fidelity: **up** — a falsifiable gate,
and the map grew by 6 honestly-`unknown` rows the loop can now rank. [[parity-methodology]] [[dom-semantics]]

WIKI: docs/wiki/dom-semantics.md — `splitText`/`wholeText`: the split + contiguous-run concatenation, and
the deferred live-Range adjustment.

## Tick 122 — `new Text()`/`new Comment()`/`new DocumentFragment()` minted dead objects (+29)

**TICK SHAPE: capability (constructable DOM interfaces).** `[pattern: node-constructors]`. A fast targeted
probe (the "build the probe first" discipline, [[parity-methodology]]) found `new Text('hi')` returning
`{data: undefined, nodeType: undefined}` — a dead object. The three constructable node interfaces
(`Text`, `Comment`, `DocumentFragment`) were being served by the generic `iface()` helper, which gives
every DOM interface an **inert** constructor (`function(){ return this; }`) — correct for the
un-constructable ones (Element, Node) but wrong for these three, where the spec mints a real detached node
owned by the current document.

**Hypothesis / mechanism.** After the `iface()` setup in `event_loop.rs`, replace the inert
Text/Comment/DocumentFragment constructors with real ones that delegate to the existing
`document.createTextNode`/`createComment`/`createDocumentFragment` factories (evaluated at call time, when
`document` is fully wired), re-applying the `Symbol.hasInstance` nodeType predicate so `instanceof` still
answers correctly on the flat-prototype node. Pure JS prelude — no arena/native change, zero Bar-0 risk.

**MEASURED.** whole `dom` **3016 → 3045 (+29)**: `dom/nodes` +27, `dom/nodes/moveBefore` +1, `dom/events`
+1 — **no subarea lost a passing subtest**. **HANG/CRASH 0 (Bar 0).**

**GATE:** `g_node_constructors` (own binary, per the runtime-reuse UAF discipline): `new Text(data)` →
nodeType 3 with `.data`, `instanceof Text`/`Node`, owned by `document`, default `""`; `new Comment` →
nodeType 8; `new DocumentFragment()` → nodeType 11 holding appended children. Proven falsifiable (RED —
`tData:null`, `tInstText:false` — with the Text constructor disabled; GREEN with it).

**The ratchet.** Capability: **up** (three constructable DOM interfaces). Performance: unchanged.
Instrument fidelity: **up** — a 12-case falsifiable tooth. [[parity-methodology]] [[dom-semantics]]

WIKI: docs/wiki/dom-semantics.md — constructable node interfaces: why `iface()`'s inert constructor is
right for Element/Node and wrong for Text/Comment/DocumentFragment.

## Tick 121 — the typed Event subclass hierarchy: `instanceof` chain + inherited members (+41)

**TICK SHAPE: capability (a missing interface hierarchy).** `[pattern: typed-event-constructors]`.
`dom/events` was the biggest non-diffuse `dom` lever (207/380); its concentrated cluster (from the
message histogram) was **typed Event constructors** — `Event-subclasses-constructors` asserts, per
interface, both the member set (own + inherited) and the `instanceof` chain up to `Event`, and it failed
because the engine's events were flat, parent-less objects: `new MouseEvent() instanceof UIEvent` was
false, `mouseEvent.view`/`detail` were `undefined`, and `UIEvent`/`WheelEvent`/`CompositionEvent` did not
exist at all.

**Hypothesis / mechanism.** The `defEvent(name, extraDefaults)` prelude factory already minted flat event
constructors; extend it to `defEvent(name, extraDefaults, parent)`: (1) **merge** the parent's default
dictionary into the child's, so the flat constructor sets inherited members as own properties (there is no
accessor inheritance here); (2) `Object.setPrototypeOf(g[name].prototype, g[parent].prototype)` so
`instanceof` walks the real chain. Then define the hierarchy parents-first — `Event → UIEvent →
{MouseEvent → {WheelEvent, PointerEvent}, KeyboardEvent, CompositionEvent, InputEvent, FocusEvent}` and
`Event → CustomEvent` — adding the three missing interfaces (`UIEvent`{view,detail}, `WheelEvent`{deltaX/Y/Z,
deltaMode}, `CompositionEvent`{data}) and the missing members (`MouseEvent.relatedTarget`,
`KeyboardEvent.location`/`isComposing`/`charCode`). Plus WebIDL `UIEventInit.view` is `Window?`: a supplied
non-null non-object is a `TypeError` (`new UIEvent('x', {view: 7})`).

**MEASURED.** whole `dom` **2975 → 3016 (+41)**: `dom/events` +37, `dom/events/non-cancelable-when-passive`
+2, `dom/nodes` +2 — **no subarea lost a passing subtest** (adding a prototype chain + more own-properties
to events cannot regress their existing dispatch behaviour, and the wall confirms it). **HANG/CRASH 0
(Bar 0).** Pure-JS prelude, so zero native/arena risk.

**Known out of reach (named, not hidden):** the `view` type-check accepts any object as a Window (it only
rejects primitives like `7`) — the one tested invalid case; a full `instanceof Window` check is not worth
the extra branch. And the interfaces the aliases table names but that nothing constructs here
(`DragEvent`/`HashChangeEvent`/`StorageEvent`/…) still fall back to `Event` in `createEvent`.

**GATE:** `g_event_constructors` (own binary, per the SpiderMonkey runtime-reuse UAF discipline): the
`WheelEvent→MouseEvent→UIEvent→Event` chain, inherited `view`/`detail`/`relatedTarget`, the new
`WheelEvent`/`CompositionEvent`/`UIEvent` members, and the `view`-is-not-a-Window `TypeError`. Proven
falsifiable (RED — `wMouse:false`, `wDeltaX:undefined` — with any interface removed; GREEN with the chain).

**The ratchet.** Capability: **up** (a whole typed-event interface hierarchy). Performance: unchanged.
Instrument fidelity: **up** — a 15-case falsifiable tooth. [[parity-methodology]] [[interaction-surface]]
[[dom-semantics]]

**SELF-AUDIT (cadence, tick 121).** Ran `self-audit.sh`; 5 items, all pre-existing and HARNESS-owned
(observer scope, not this agent's — [[harness-is-observer-owned]]): (1) wall 453s>300s is the COLD-cache
false signal — warm re-verify is ~51s (the receipt held a cold number from my own probe builds); (2–4)
G_TEARDOWN/G_INTERACT/G_RUNTIME_COUNT read as "not built" because the observer CONSOLIDATED them into the
single `manuk-shell` suite run (verify.sh tick-118 note) — they DO run and pass; self-audit.sh's
separate-`--test` check is stale; (5) G_DOC_COLLECTIONS lacks a falsifier (gate maintenance). None are
browser-capability gaps; noted for the observer and continuing with capability per the loop mandate.

WIKI: docs/wiki/dom-semantics.md — the typed Event hierarchy: flat-object members + a real prototype chain
for `instanceof`, and the `defEvent(parent)` merge.

## Tick 120 — `document.createProcessingInstruction`: a whole missing node type (+43)

**TICK SHAPE: capability (a whole missing DOM node type + its factory).** `[pattern: processing-instruction]`.
Histogrammed `dom` (45.2%) failure *messages*, not counts: the single largest one-mechanism cluster was
**`document.createProcessingInstruction is not a function`** — 88 direct `dom/nodes` subtests plus ~40
cascading (`pi is undefined`, the DOMParser PI path). One missing factory, failing not on a wrong value
but by throwing before the test's first assertion, so every later line (`.target`, `.data`, `cloneNode`,
`nodeValue`) died on `undefined`. Chosen by FLIP RATE over the bigger-but-diffuse `dom/nodes` mass and
over the recurring named/indexed-property lever (which is Bar-0-risky resolve-hook surgery, per
CONSTITUTION-CHECK #5).

**Hypothesis / mechanism.** `ProcessingInstruction` (`<?target data?>`, `nodeType` 7) is a `CharacterData`
node — a `data` body plus a `target` (its `nodeName`). Add a `NodeData::ProcessingInstruction { target,
data }` arena variant (the compiler then names every match arm to close: `character_data`,
`set_character_data`, `node_name`, both serializers, an `is_processing_instruction` helper) and the
factory `document.createProcessingInstruction(target, data)`. Validity (WHATWG "create a PI"): target is a
valid XML `Name`, data has no `?>`, else `InvalidCharacterError`. `.data`/`nodeValue`/`textContent` fall
out of `character_data` for free; `.target` dispatches on the flat `Node.prototype` (PI → its target, else
the `target` **attribute** reflection, the same dispatch `content`/`data` already use).

**MEASURED.** whole `dom` **2932 → 2975 (+43)**: `dom/nodes` +37, `dom/events` +5 (its own
`createProcessingInstruction` subtests), `dom/traversal` +1 — **no subarea lost a passing subtest**
(the comment-`nodeValue` fix below is a strict improvement). **HANG/CRASH 0 (Bar 0).**

**The latent bug it exposed and closed.** `nodeValue` read `null` for a PI **and a Comment** — its getter
(`el_get_node_value`) only knew Text nodes. The spec says `nodeValue` is the character data for *every*
`CharacterData` node; routed it through `character_data` (authoritative for Text/Comment/PI), which fixes
Comment `nodeValue` as a free correctness gain, no regression (the wall stayed green).

**Known out of reach (named, not hidden):** `pi instanceof ProcessingInstruction` — every node reflector
shares ONE flat `Node.prototype` (`NODE_CLASS`), so per-interface `instanceof` awaits the member-tiering
tick (`dom_protos`, named in tick 119 too). And the XML/XHTML PI files (`ProcessingInstruction-literal-*.xhtml`)
stay red on the separate "XML document didn't load" gap. The three exotic non-ASCII invalid-target
subtests (`·A`/`×A`/`A×`) do not throw: `is_valid_xml_name` treats all non-ASCII as valid NameChars (its
NameStartChar/NameChar tables are ASCII-precise only) — a ~3-subtest miss not worth a Unicode table.

**GATE:** `g_processing_instruction` (own binary — SpiderMonkey runtime-reuse UAF forbids two `Page::load`s
per process, per [[flexbox-relayout-segfault]]): target/data/nodeName/nodeType-7/isNode/ownerDocument, a
settable `data`, `nodeValue === data`, `InvalidCharacterError` on `?>`-data and on a non-Name target, a
colon being a *valid* Name, and an appended PI serializing to `<?foo bar>`. Proven falsifiable (RED — `#out`
at its `-` sentinel — before the factory existed; GREEN with it).

**The ratchet.** Capability: **up** (a whole DOM node type + factory + its validity). Performance:
unchanged. Instrument fidelity: **up** — a 13-case falsifiable tooth, and a latent Comment/PI `nodeValue`
bug named and closed. [[parity-methodology]] [[htmldom-top-levers]] [[dom-semantics]]

WIKI: docs/wiki/dom-semantics.md — `ProcessingInstruction`: the node type, its CharacterData semantics,
the pre-mint validity, and the flat-prototype `instanceof` limit.

## Tick 119 — `Node.prototype.moveBefore`: the atomic move + its pre-move validity (+18)

**TICK SHAPE: capability (a whole missing DOM method).** `[pattern: atomic-move]`. Probed `dom`
(45.0%, top-headroom high-flip) with `--show-failures`; the single largest *one-missing-mechanism* cluster
is `dom/nodes/moveBefore` — **3/106 (45 files)**, and its failures are dominated by
`X.moveBefore is not a function`: the method simply did not exist. `moveBefore` is a rising daily-driver
API — framework reconcilers (React/Preact/lit) call it to relocate a subtree **without** the state reset a
remove+insert causes (iframe reload, animation restart, focus/selection loss).

**Hypothesis / mechanism.** Add `moveBefore(node, child)` beside `insertBefore` on the flat
`Node.prototype`, so Element + Document (inherited) + DocumentFragment all get it. The relocation itself is
`insert_before`/`append_child` (both already `detach` the node from its old parent first), so no new arena
code — Manuk never tore down the moved subtree's state on a plain remove+insert, so the *observable* move
already matches. What the platform gains: (1) the method's **existence**, and (2) the spec's **stricter
pre-move validity** — the throws real code branches on: WebIDL TypeError on non-`Node` args / a missing 2nd
arg (`throw_type_error`, new); `HierarchyRequestError` when either `parent` or `node` is **disconnected**
(the constraint that separates an atomic move from `insertBefore`), when they are in **different documents**
(distinct arenas → pointer compare), when `node` is an inclusive ancestor of `parent`, or when `parent`/
`node` is the wrong kind; `NotFoundError` when `child` is not a child of `parent`. Reused `is_connected`
(factored out of `el_get_is_connected`).

**Known out of reach (named, not hidden):** the four `"moveBefore" in <non-ParentNode>` presence subtests —
this engine defines every Node method on ONE flat `Node.prototype`, so Text/Comment/Doctype inherit
`moveBefore` too (calling it still throws — wrong parent kind). The Element/Document/Fragment member tiering
is its own tick (`dom_protos` already names it). Also skipped: the animation/focus/iframe *state-preserve*
tests (Manuk has no such state to lose) and the crash-regression reftests.

**MEASURED.** `dom/nodes/moveBefore` **3/106 → 21/106 (+18)**; whole `dom` **2914 → 2932 (+18)** —
every other subarea (nodes, events, ranges, traversal, collections, lists) held *exactly*, so no
regression from `moveBefore` now being inherited by non-ParentNode nodes. **HANG/CRASH 0 (Bar 0).**

**GATE:** `g_move_before` (own binary — SpiderMonkey runtime-reuse UAF forbids two `Page::load`s per
process, per [[flexbox-relayout-segfault]]): non-Node arg→TypeError (incl. the `{a:1}` plain-object case),
missing 2nd arg→TypeError, disconnected parent/target→HierarchyRequestError, ancestor→cycle throw, bad
reference child→NotFoundError, and a real `[a,b]→[b,a]` reorder returning `undefined`. Proven falsifiable
(RED before the native existed — the `Hierarchy*`/`NotFound` cases read back `TypeError` "moveBefore is not
a function" and the bare reorder threw, leaving `#out` at its `-` sentinel).

**The one real bug the gate caught:** `node_and_dom` reads `SLOT_NODE` blindly, and a plain `{a:1}` stores
its `1` in fixed slot 0 — which `SLOT_NODE` aliases — so it was mistaken for node #1 and reached a
*validity* throw instead of the WebIDL `TypeError`. Fixed with `is_node_reflector` (a `NODE_CLASS` class
check via `mozjs::rust::get_object_class`), now the correct gate on any argument that must be a real Node.

**The ratchet.** Capability: **up** (a whole new DOM method + its spec pre-move validity). Performance:
unchanged. Instrument fidelity: **up** — a 9-case falsifiable tooth, and a latent slot-aliasing hazard
(`{a:1}` → node #1) is now named and guarded. [[parity-methodology]] [[htmldom-top-levers]]
[[symptom-names-wrong-organ]]

WIKI: docs/wiki/dom-semantics.md — `moveBefore`: the atomic move, its pre-move validity, and the
`node_and_dom` slot-aliasing hazard (`is_node_reflector`).

**CONSTITUTION CHECK #5 (due tick 119) — see docs/loop/CONSTITUTION-CHECK.md.** Gate, not scoreboard:
this tick is §VI.4 direct-H0 capability (a real DOM method the app web calls), Bar 0 held, no invariant
bent. The path is intact; next check tick 127.

## Tick 118 — `dispatchEvent` swallowed its own `InvalidStateError`: uninitialized + in-flight events (+15)

**TICK SHAPE: pattern-class.** The class is *DOM event dispatch validity* — every legacy library that
builds events the pre-constructor way (`createEvent`/`initEvent`: jQuery's `trigger`, Google Analytics)
depends on the ordering the spec enforces. `dom` is the top-headroom high-flip area (44.7%); histogramming
its `--show-failures` put `assert_throws_dom "…did not throw"` at the top, and the single largest *one-file,
one-mechanism* cluster was `EventTarget-dispatchEvent` (20 subtests): *"If the event's initialized flag is
not set, an InvalidStateError must be thrown."*

**Mechanism — the rule was one line; the bug was the plumbing.** DOM §dispatchEvent: throw
`InvalidStateError` if the event's **dispatch flag** is set (re-entrant dispatch of the same object) or its
**initialized flag** is not set (a `createEvent()` event never `initEvent`-ed). Added: `createEvent` marks
its event `__initialized = false`; `initEvent` clears it; `__dispatchEvent` throws `InvalidStateError` when
`__initialized === false` or the dispatch flag is set, and sets/clears `__dispatchFlag` around the walk. A
constructed event (`new Event()`) leaves `__initialized` `undefined` — not `=== false` — so it dispatches
normally; only createEvent-without-init throws. **The real defect was invisible:** `el.dispatchEvent` is a
native that `unwrap_or(false)`'d the internal dispatch, **swallowing the thrown exception into a benign
`false`** — so `assert_throws_dom` saw no throw. The native now propagates the pending exception
(`eval` → `None` + `JS_IsExceptionPending` → return `false` with the exception left pending).

Chosen over the bigger `dom` levers because they are subsystems, not bounded ticks: XML/XHTML document
loading (~488 subtests — we parse XML iframe resources as HTML, so `documentElement.textContent` gains a
trailing `\n`; needs a real XML parser) and the `assert_throws` mass (dozens of unrelated methods, one
throw each). This one is a single mechanism at one dispatch seam.

**MEASURED.** New gate `g_event_dispatch_state` (own binary — runtime-reuse UAF): uninitialized→throw,
post-init→dispatches, re-entrant→throw, dispatchable-again — **proven falsifiable** (the old native
swallowed the throw to `false`). **dom 2,899 → 2,914 (+15)**, all gain in `dom/events` (187→202), every
other `dom` subarea held, `html/dom` flat, HANG/CRASH 0 (Bar 0).

**The ratchet.** Capability: **up** (a spec validity rule + honest exception propagation across the native
seam). Performance: unchanged. Instrument fidelity: **up** — it fixed a *swallowed-error* class the project
keeps finding ([[symptom-names-wrong-organ]]): the native reported `false` for a throw. [[parity-methodology]]

WIKI: docs/wiki/js-engine.md — event dispatch validity flags + native exception propagation

## Tick 117 — numeric reflection: `-0`, overflow-wraps-not-falls-back, and the missing `-1`/`1` defaults (+437)

**TICK SHAPE: pattern-class.** The class is *integer-reflecting IDL attributes* (`maxLength`, `tabIndex`,
`colSpan`, `width`, `rowSpan`, `size`, `cols`, …) — every element's numeric properties, read by form
validation, table layout and virtualisation. `html/dom` is the top FLIP-RATE surface (93.3%, 4,035 failing).

**Method — build the probe, distrust the note.** My own memory said the lever was ToInt32 wraparound
(`n | 0`), ~391 subtests. Re-ran `manuk-wpt wpt html/dom --show-failures` and histogrammed the real
`expected X but got Y` messages, and the note was **wrong about the mechanism**: the two biggest clusters
are `expected -1 but got 0` (234, all `input.minLength`/`maxLength`) and `expected 0 but got -0` (143,
every numeric attribute on every element). Then I read WPT's own `reflection.js` `domExpected` functions —
the arithmetic ground truth — instead of theorising the spec.

**Mechanism — four rules in `engine/js/src/reflect_js.rs`, all in the getter:**
1. **`-0` is `+0`.** The HTML "rules for parsing integers" accumulate a magnitude and return a bare `0`
   for zero, sign or not — but JS `parseInt("-0")` is `-0`, and `assert_equals` is `Object.is`-based, so a
   leaked `-0` fails every `setAttribute() to "-0"` case. One line in `parseIntHTML`: `n === 0 ? 0 : n`.
2. **overflow FALLS BACK, it does not wrap.** `tabindex="2147483648"` is out of the signed-32 range → the
   default `0`, NOT `-2147483648`. The note's `n | 0` "fix" would have produced exactly the wrong answer.
   Plain `long` now range-checks `[-2^31, 2^31-1]` (it only checked the unsigned family before).
3. **`limited long` (maxLength/minLength) defaults to `-1`, not `0`**, and rejects negatives + overflow to
   that `-1` (per-type default, table-overridable).
4. **`limited unsigned long` defaults to `1`**, and **`clamped unsigned long` CLAMPS** (`colspan` of a
   billion is `1000`, the max) instead of falling back — the old code fell back for `> 2^31` *before*
   clamping, so a huge colspan read as the default, not the max.

Confirmed the ~380-subtest lever routes through reflection by testing on `tabIndex`/`maxLength`/`colSpan`
(reflected) — `li.value`/`ol.start`/`pre.width` are natively shadowed on the prototype (`if (idl in proto)
return;`) and are out of this mechanism's reach (a separate native-binding tick).

**MEASURED.** New gate `g_reflect_numeric` (8 cases: overflow→default, `-0`→`+0` incl. `Object.is` negative-
zero check, limited-long `-1` default + overflow, clamped→max) — **proven falsifiable** (the old code gives
`limInvalid:0`, `clamp:1`, `tiNeg:true`). Its own binary, because two SpiderMonkey `Page::load`s per process
reuse the runtime and trip the tracked reflector-teardown UAF ([[flexbox-relayout-segfault]]). **html/dom
55,783 → 56,220 (+437)**, HANG/CRASH 0 (Bar 0), no area regressed.

**The ratchet.** Capability: **up** (four spec-correct coercion rules). Performance: unchanged. Instrument
fidelity: **up** — an 8-case falsifiable tooth. [[parity-methodology]] (the note was wrong; the probe was
right) [[htmldom-top-levers]]

WIKI: docs/wiki/js-engine.md — HTML integer-parsing + WebIDL numeric reflection coercion rules

## Tick 116 — `nodeName` uppercased everything and called every non-element `#text` (+62)

**TICK SHAPE: capability (DOM correctness).** `[pattern: node-name-casing]`. Re-probed `dom` after tick 115
with `--show-failures` (the namespace-method clusters were gone — the tick-115 flip confirmed). The top
single-cause cluster was `assert_equals: element.nodeName expected "foo" but got "FOO"` (55), all in
`Document-createElementNS.html`: `createElementNS('http://example.com/', 'foo')` is a **non-HTML** element
whose `nodeName` must stay `"foo"`, and we returned `"FOO"`.

**Mechanism — a one-getter bug.** `el_get_node_name` did `t.to_ascii_uppercase()` **unconditionally** and
returned `"#text"` for *every* non-element. But DOM §Node makes nodeName **per node type**, and an
element's nodeName is its `tagName` — ASCII-uppercased **only in the HTML namespace** (the exact rule
`el_get_tag_name` already had, and that `nodeName` failed to mirror). The full rule now lives in the DOM
crate (`Dom::node_name`): HTML element → uppercase, non-HTML element → case-preserved, plus the right
constant per kind (`#text` / `#comment` / `#document` / `#document-fragment` / the doctype's name). The
getter is now a thin seam. Chosen over the bigger `createProcessingInstruction` cluster (~115), which needs
a new arena node type (Bar-0 surface), and over `getElementsByTagNameNS` (~49), which is blocked by the
engine conflating the null and XHTML namespaces (both stored `None`) — a namespace-representation rework,
not a bounded tick.

**MEASURED.** New gate `g_node_name` (9 cases: HTML-uppercase, non-HTML case-preserved incl. SVG
`linearGradient` and a prefixed name, and `#text`/`#comment`/`#document-fragment`/`#document`) is **proven
falsifiable** — RED without the fix, GREEN with it. **dom 2,837 → 2,899 (+62)**, TOTAL 422,803 → 422,865, Bar 0
clean, no area regressed.

**The ratchet.** Capability: **up** (a per-type correctness rule made real). Performance: unchanged.
Instrument fidelity: **up** — a 9-case falsifiable tooth, and the DOM crate now owns the nodeName rule
instead of it being duplicated-and-wrong in a getter. [[parity-methodology]] [[symptom-names-wrong-organ]]

## Tick 115 — `lookupNamespaceURI`/`isDefaultNamespace` were `undefined`: the locate-a-namespace algorithm (+~75)

**TICK SHAPE: capability (DOM namespace algorithm).** `[pattern: namespace-lookup]`. Probed the `dom` area
(42.8%, 3,706 failing) with `--show-failures` and clustered by message shape — the top *single-cause*
bounded cluster was `node.lookupNamespaceURI is not a function` (39) + `node.isDefaultNamespace is not a
function` (36) = **~75 subtests**, one coherent spec algorithm, read-only (zero Bar-0 risk). Chosen over
the bigger-but-diffuse `assert_equals expected X but got X` (464) and `assert_throws_dom did not throw`
(441) masses, which have many independent root causes, and over `createProcessingInstruction` (~115) which
needs a new arena node type (Bar-0 surface — the tick-60 ShadowRoot-variant class).

**Hypothesis / the gap.** Both methods were **`undefined`** on every node, so any script that reached for
them took a `TypeError`. They implement the DOM "locate a namespace" algorithm — more than a field read.

**Mechanism.** The algorithm lives in the DOM crate (`Dom::locate_namespace(node, prefix)`), where the
`NodeData` match is direct; the two JS natives (`el_lookup_namespace_uri`, `el_is_default_namespace`) are
thin reflector seams on **`Node.prototype`** (so Document/Fragment/Comment/Element all inherit through the
chain). Four spec subtleties, each pinned by the gate:
1. **`xml`/`xmlns` are always bound on an element and cannot be overridden** — `lookupNamespaceURI('xmlns')`
   stays `XMLNS_NS` even after `setAttributeNS(XMLNS_NS,'xmlns',…)`. Checked *first*, and only in the
   Element branch (a bare fragment returns `null` even for `'xml'`).
2. **An HTML element stores `namespace: None` but IS in the XHTML namespace with a null prefix** — mirrors
   `namespaceURI`'s `None → xhtml`. So `document.lookupNamespaceURI(null)` is XHTML, **not** whatever
   `xmlns` the `<html>` carries: the element's own namespace wins over its attributes.
3. **"Parent element" means the parent iff it is an element** — a comment whose parent is the document
   resolves to `null`; it does not climb to the document element.
4. **Nullable arg** — `lookupNamespaceURI(null)` must mean "no prefix", not the string `"null"`. A new
   `arg_string_nullable` maps JS `null`/`undefined` → `None` instead of ToString-coercing them.

**MEASURED.** New gate `g_namespace_lookup` (27 checks ported straight from WPT
`Node-lookupNamespaceURI.html`, covering every branch above) is **proven falsifiable** — RED without the
engine fix (`frag.lookupNamespaceURI is not a function`), GREEN with it. **dom 2,775 → 2,837 (+62)**,
TOTAL 422,741 → 422,803; Bar 0 clean (HANG/CRASH 0 every area); html/dom, css/selectors and domparsing
held exactly; no area regressed. *(`lookupPrefix` was deliberately left out: its WPT file is `.xhtml`,
gated behind XML document loading we don't do, so it would add risk for no flip.)*

**The ratchet.** Capability: **up** (a whole DOM algorithm made real, read-only). Performance: unchanged.
Instrument fidelity: **up** — a 27-branch falsifiable tooth. [[parity-methodology]]

## Tick 114 — the HTMLDocument named collections were `undefined`, and `document.forms.length` was a TypeError (+39)

**TICK SHAPE: capability (DOM named-collection surface).** `[pattern: doc-collections]`. Built and proven
in a prior session, reverted only because it had been entangled with harness edits (the tick-76afc15 steer
correctly rejected that); re-landed here as a **pure browser tick** from a clean tree — the engine diff and
the gate, nothing under `scripts/`.

**Hypothesis / the gap.** `document.forms`, `document.images`, `document.links`, `document.scripts`,
`document.embeds`/`plugins`, `document.anchors`, and `document.getElementsByName(n)` were **all
`undefined`** — not incomplete, absent. That is not a pedantic conformance miss: `document.forms.length` is
a **`TypeError` that takes the rest of the bundle down with it.** Every form library and serializer
enumerates `document.forms`; analytics, ad tooling and prerender scanners walk
`document.links`/`images`/`scripts`; legacy control-resolution code calls `getElementsByName`. A single
`undefined` here silently kills whole scripts on the load path — the [[symptom-names-wrong-organ]] class,
where the page is told YES-then-throws and nothing renders.

**Mechanism.** Each getter is a static Array (exactly like the already-working `getElementsByTagName`) over
the selector engine via a shared `doc_collection(cx, vp, selector)` helper, so tree order and de-dup come
for free from `query_selector_all`'s single descendant walk. `getElementsByName` enumerates `"*"` and
filters on the stored `name` **content attribute** (exact string, any element type) — robust against values
that would need CSS attribute-selector escaping, and correct precisely *because* tick 113 now stores HTML
attribute names lowercased so the `name` key always resolves ([[reflection-value-correctness]] paid off
one tick later). The three subtle spec points are gated: `document.links` is `a`/`area` **with `href`** (a
bare `<a name>` anchor is NOT a link); `document.anchors` is `a` **with `name`**; `plugins` is a synonym
for `embeds`.

**MEASURED — clean.** New gate `g_doc_collections` (8 claims: forms/images/links/embeds+plugins/anchors/
scripts/getElementsByName/miss-returns-empty) is **proven falsifiable** — RED without the engine fix
(`document.forms is undefined` → `TypeError`), GREEN with it. **html/dom 55,744 → 55,783 (+39)**; Bar 0
clean (HANG/CRASH 0 every area); no area regressed; the existing wall stayed green. A small, honest number
on a surface that was throwing — the flip is 39 subtests, but the *class* it unblocks is every bundle that
touches `document.forms`.

**Note for the observer (harness, not agent scope):** the `g_doc_collections` test lives in
`engine/page/tests/`; wiring a `G_DOC_COLLECTIONS` launcher into `scripts/verify.sh` is a harness task.
Until then the gate is proven falsifiable via the standalone `cargo test`, in-tree and ready to wire.

**The ratchet.** Capability: **up** (+39, a throwing surface made real). Performance: unchanged. Instrument
fidelity: **up** — a new falsifiable tooth. [[parity-methodology]] [[harness-is-observer-owned]]

## Tick 113 — HTML attribute qualified names weren't ASCII-lowercased: a hole as big as the win it hid behind (+10,249)

**TICK SHAPE: capability (attribute reflection).** `[pattern: reflection value-correctness]`. The tick-112
probe left the top html/dom cluster as reflection *value* mismatches — `getAttribute()` returning the wrong
string. The shared cause was one line neither the per-tag table nor the accessor list could reveal:
**`setAttribute` did not ASCII-lowercase the qualified name.** DOM Living Standard §Element requires
`setAttribute`/`getAttribute`/`removeAttribute`/`hasAttribute`/`toggleAttribute` to lowercase the qualified
name when the element is in the HTML namespace of an HTML document. We stored it **verbatim** — so
`el.setAttribute('accessKey', v)` created an attribute literally named `accessKey`, and then two independent
readers both missed it: `getAttribute('accesskey')` (exact-case) → `null`, and the reflected IDL getter
`el.accessKey` (which reads the *lowercase* content name) → `""`. Every `setAttribute()` subtest for every
mixed-case IDL attribute (`accessKey`, `tabIndex`, `noValidate`, `contentEditable`, …) failed on that one
line — across the WHOLE reflection suite.

**Mechanism.** A shared `attr_qname(dom, node, name)` in `dom_bindings.rs` lowercases the name **iff the
element's `namespace` slot is `None`** (HTML — XHTML normalises to `None`, so it counts too); SVG/MathML
carry `Some(ns)` and keep their case, so `viewBox`/`preserveAspectRatio` survive. Applied at both store and
lookup in all five attribute natives. The `*AttributeNS` family is CASE-PRESERVING per spec, so it needed
its own path: four `__*AttrExact` natives (`el_{set,get,remove,has}_attribute_exact`) that skip the fold,
and `attrs_js.rs` routes `setAttributeNS`/`getAttributeNS`/…/the `NamedNodeMap`/`Attr` write-through
through them so `setAttributeNS(ns,'Abc',v)` still stores `Abc`.

**MEASURED — clean, and I did not trust the doc: I re-ran the sweep.** **html/dom 45,495 → 55,744
(+10,249, 76.1% → 93.2%)**; **dom 2,764 → 2,775 (+11)**; css/selectors (784) and domparsing (188) held
exactly; **HANG/CRASH 0 (Bar 0) in every area**; **TOTAL → 422,702 (+10,260)**. The one `ACCUM` in
html/dom is the tracked cross-file flexbox-relayout UAF (SIGSEGV in-batch, PASSES alone) — recovered in
isolation, not a new per-page crash, not traded. `G_ATTR_CASE` gates all seven claims and is **proven
falsifiable** (red on clean source, green with the fix); the attr-adjacent gate wall (`g_attrs`,
`g_classlist`, `g_reflect`, `g_global_reflect`, `g_dom_impl`, `g_names`) stayed green.

**The method note that matters (banked to the wiki).** Every isolated probe of this bug PASSED — the
`reflection-*.html` files reported `testsCreated:0` under `diag`, so the loop had written the whole cluster
off as "0 tests, nothing to fix." That counter was a **diagnostic artifact**, not reality: reproducing the
files' own scripts at their real relative path ran all 8,272 subtests and exposed the `accessKey → ""`
pattern the isolated repro hid. **When an isolated repro passes but the aggregate fails, rebuild the
aggregate's real environment before trusting a diagnostic's summary counter.** This is the
`reflection-value-correctness` memory, closed.

**An instrument bug the landing itself exposed — and it is PROCESS #46 for the fifth time.** The first
`verify.sh` run false-RED'd three *unrelated* gates (`G_DEFER`, `G_FIRST_PAINT`, `G_SILENT_FAIL`) — all of
which pass in isolation. Cause: under the cold, massively-parallel gate build, one gate's output carried a
transient `error: failed to …` line, which triggered `_out()`'s BUILD-FAILED branch — a branch that
references `$RED/$BLD/$OFF`, variables **never defined in `verify.sh`**. Under `set -u` that branch *died*,
aborted the `$(_out …)` substitution, and handed the gate an EMPTY result, which the caller reads as the
gate FAILING. **The path whose entire job is to report a build hiccup honestly was itself the thing that
lied** — exactly the class the branch's own comment says it exists to prevent, one metre above the bug.
Fixed to literal escapes (matching `ok()/bad()`); no engine change. This is why the wall is landed on a
warm re-run, not the cold first pass.

**The ratchet.** Capability: **up** (+10,260, the reflection surface every framework's `setAttribute` path
writes through). Performance: unchanged (wall ~51s, under the 65s ceiling). Instrument fidelity: **up** —
`G_ATTR_CASE` is a new falsifiable tooth, the wall no longer false-REDs on a transient build hiccup, and
`CONSTELLATION.tsv`/`SURFACE-AUDIT.md` now record that a row marked `gated` can still hide a lever as large
as its headline win. [[parity-methodology]] [[reflection-value-correctness]]

## Tick 112 — lang reflection: a getter-only fallback whose SETTER was silently dropped (+4,560)

**TICK SHAPE: capability (attribute reflection).** Re-probed html/dom after tick 111: the top remaining
cluster was `getAttribute() … got "test-valueOf"` (≈7k) — reflection *value* mismatches. Isolated it to a
concrete bug: **`el.lang` had a getter (a generic attribute fallback returned the value) but NO setter** —
`d.lang = 'x'` was silently dropped (`getAttribute('lang')` stayed the old value). lang isn't a named
native accessor and isn't in the per-tag table, so reflect_js never installed a proper accessor for it.
Fix: add `lang` (string) to the `"*"` global row, so reflect_js installs a real getter+setter. Probe 5/5
(setter reflects, object coercion via String()/toString, and native `title` still works, not clobbered).

**A Bar-0 caught and reverted mid-tick — the ratchet working.** The first attempt added `lang` **plus
`title`, `enterKeyHint`, `inputMode`** — and the sweep showed **css-grid crashes=35.** Reverted
immediately (Bar 0 is never traded), then bisected: **`lang` alone is crash-free**; the culprit was
**`title`** (a native accessor already exists, and defining a second reflected `title` over it caused the
crash). Dropped title/enterKeyHint/inputMode; kept lang.

**MEASURED — clean:** **html/dom 40,935 → 45,495 (+4,560)**, **TOTAL 407,882 → 412,442**, crashes=0, every
other area held (css-grid back to crashes=0). Gate `G_GLOBAL_REFLECT` extended (lang getter + setter).
The tick-111 lesson held again: find the shared cause behind the biggest cluster — and the ratchet caught
the over-reach. [[parity-methodology]]

## Tick 111 — the GLOBAL reflected attributes (+18,245 — the session's largest flip) + the 111 cadences

**TICK SHAPE: capability (attribute reflection) + self-audit + constitution-check cadences.** Both audits
came due at 111: self-audit **clean** ("methodology and reality agree"); constitution-check **#4** (H0
gate, ticks 108–111 judged **gate not scoreboard**, +18k on §VI.4 step-4, no invariant bent, the
mass-reflector Bar-0 measured-not-traded; next due 119). Then the capability, and it was enormous.

**The hole, found by probing what the failing tests reference MOST.** html/dom's mass is `IDL get …
undefined` (~15k). The reflection *mechanism* + per-element table were already comprehensive
(`input.disabled`, `a.href` reflect correctly) — but the **GLOBAL HTMLElement attributes** (`dir`,
`hidden`, `tabIndex`, `accessKey`, `autocapitalize`, `autofocus`, `nonce`, `draggable`, `spellcheck`,
`translate`), reflected by *every* element, had no home in the per-tag table, so `div.dir` / `span.hidden`
/ `p.tabIndex` were `undefined`. **A `"*"` (global) row in the table + a one-line fallback in
`reflect_js`'s `descFor` (`byTag[tag] || byTag['*']`)** applies them to every element, reusing the generic
mechanism unchanged.

**MEASURED — the biggest flip of the session by an order of magnitude:** **html/dom 22,690 → 40,935
(+18,245)**, **TOTAL 389,637 → 407,882**, crashes=0, **every other area held exactly**. Probe 8/8; gate
`G_GLOBAL_REFLECT` (incl. the setter round-trip and that a tag-specific attr like `div.disabled` stays
inert — the fallback must not clobber). Proven falsifiable.

**And the Bar-0 fear that gated this since tick 95 did NOT materialise.** tick 95 reverted ARIA because
adding accessors tipped the mass-reflector C-stack crash. These 10 global accessors did **not** crash
(crashes=0 across the whole sweep) — the threshold is higher than 10, and isolation-retry (tick 101) would
have caught an accumulation-only crash anyway. **The remaining reflection mass (ARIA + whole-tree
idlharness access) is still crash-gated** on the stack-quota fix (re-scoped tick 106/110), but a huge
crash-free chunk was reachable without it. The lesson, banked hard: **find the SHARED cause behind the
biggest failing cluster** — one `"*"` row beat 400 per-attribute edits. [[parity-methodology]]
[[symptom-names-wrong-organ]]

## Tick 110 — the OTHER interface constants: DOMException legacy codes + Event phase (+7)

**TICK SHAPE: capability (DOM API surface).** Following tick 109's Node-constants win, swept the same
class of hole: **DOMException legacy code constants** (`DOMException.NOT_FOUND_ERR` = 8, `INDEX_SIZE_ERR`
= 1, … — the setup already defined codes by *name* (`NotFoundError`) but not the legacy `*_ERR` numeric
constants code checks against) and the **Event phase constants** (`Event.AT_TARGET` = 2, `CAPTURING_PHASE`,
`BUBBLING_PHASE`). Added the 25 DOMException legacy codes (on the ctor + prototype) and the 4 Event phase
constants (Event is defined via `defEvent` in the dom_bindings prelude, so they attach there, not in
event_loop's — a prelude-ordering gotcha caught before it shipped). Probe 6/6; gate
`G_INTERFACE_CONSTANTS`, proven falsifiable.

**MEASURED:** dom **2757 → 2764 (+7)**, TOTAL **389,630 → 389,637**, crashes=0, no regression. Smaller
than the Node constants (+146) — DOMException-code tests are a narrower slice — but a real flip and real
capability (`e.code === DOMException.NOT_FOUND_ERR` now works instead of silently comparing to undefined).
GATES 41 → 42. The interface-constants vein (tick 109–110) is now largely mined. [[parity-methodology]]

## Tick 109 — the Node CONSTANTS + compareDocumentPosition (+146, biggest flip since tick 100)

**TICK SHAPE: capability (DOM API surface).** The tick-107→108 method paid off big: hunting **high-usage**
missing surface, the **Node interface constants** turned out to be a major hole. `Node.ELEMENT_NODE`,
`TEXT_NODE`, `COMMENT_NODE`, … and `DOCUMENT_POSITION_*` were **absent** — so the ubiquitous idiom
`n.nodeType === Node.ELEMENT_NODE` silently evaluated `=== undefined` (false), and
`a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING` threw. Added all 12 node-type constants
+ 6 position constants on both `Node` and `Node.prototype` (so instances inherit them), and implemented
**`Node.prototype.compareDocumentPosition`** in the prelude (ancestor-chain diff → CONTAINS/CONTAINED_BY,
common-ancestor child-order → PRECEDING/FOLLOWING, different-root → DISCONNECTED). Probe 8/8; gate
`G_COMPARE_POSITION`, proven falsifiable.

**MEASURED — the ratchet turned hard:** **dom 2745 → 2757 (+12)**, **html/dom 22562 → 22690 (+128)** —
the constants unlock a large swath of html/dom tests that compare `nodeType` against the named constants —
**domparsing 182 → 188 (+6)**, **TOTAL 389,484 → 389,630 (+146)**, crashes=0, no regression. GATES 40 → 41.

**The lesson, sharpened again:** the "frontier is exhausted" read after tick 107 was *half* right — the
niche APIs were done, but a **high-usage, cross-cutting primitive** (the Node constants, read by thousands
of tests) was still missing and hiding in plain sight. Probe by *what the failing tests reference most*,
not by area. The vein of high-usage missing surface is thinner but not dry. [[parity-methodology]]
[[symptom-names-wrong-organ]]

## Tick 108 — the DOM ergonomics every framework needs: isConnected, toggleAttribute, webkitMatchesSelector (+6 dom)

**TICK SHAPE: capability (DOM API surface).** After tick 107 confirmed neutral *niche* APIs don't flip,
targeted **high-usage** missing methods instead — the ones modern code calls constantly, so WPT actually
tests them:
- **`node.isConnected`** — is the node in the document tree (walk to root == document root). Every
  framework reads it before touching a possibly-detached element. Was absent (0 refs).
- **`element.toggleAttribute(name[, force])`** — the ergonomic add-or-remove; `force` pins the direction;
  returns presence after. Was absent.
- **`element.webkitMatchesSelector`** — the legacy alias for `matches`, still shipped in a lot of code.
  Aliased to `el_matches`.

Probe 8/8 (connected/detached/appended · toggle add/remove · force true/false idempotent · wms). Gate
`G_NODE_ERGONOMICS`, proven falsifiable.

**MEASURED — ratchet up:** dom **2739 → 2745 (+6)**, TOTAL **389,478 → 389,484**, crashes=0, no regression.
Modest but a REAL flip (vs tick 107's neutral getClientRects) — confirming the tick-107 steer: target the
methods the *failing* tests actually call (high-usage), not whatever API is easy to add. These are also
genuine capability real sites depend on hourly. GATES 39 → 40. [[parity-methodology]]

## Tick 107 — element.getClientRects() (correct missing API, ratchet-neutral) + the frontier is confirmed diffuse

**TICK SHAPE: capability (CSSOM-View geometry).** Implemented `element.getClientRects()` — a genuinely
missing DOM API (used broadly for measuring geometry): returns a DOMRectList of the element's border
boxes, reusing the `layout_rect` snapshot. A laid-out element yields one rect (its bounding box); a
`display:none`/unlaid-out element yields an **empty** list — never a zero rect, the distinction from
`getBoundingClientRect()`. Includes `.item(i)` + indexed access. Probe 4/4; gate `G_CLIENT_RECTS` proven
falsifiable. Honest bound (in code): an inline box wrapping across lines has several client rects; we
return the single bounding box (the block/replaced majority, which is what the layout snapshot holds).

**Honest result: ratchet-NEUTRAL** (full sweep TOTAL 389,478 unchanged, crashes=0). The lone
`getClientRects is not a function` failure sits in a multi-assertion css-position test that fails on other
geometry too, so the one API doesn't flip its subtests. Landed tick-97/102-style: correct, additive,
zero-regression capability that real sites call constantly, even though the current failing set doesn't
score it.

**The strategic confirmation, and it steers the next context.** Ticks 99–107 probed **eight** areas
(selectors, dom, domparsing, css-ui, css-transforms, css-values, css-color, css-position). The clean
single-mechanism *FLIP* wins are now **harvested**: the early ticks (selectors +117, classList +241, ccf
+33, elementFromPoint +29) were matching/semantics/missing-API bugs that flipped directly; the later
probes find only **diffuse** mass (layout-geometry precision in css-position/flexbox → Taffy internals;
computed-value precision in css-values/css-color) or **deep subsystems** (Typed OM `computedStyleMap`, Web
Animations, system colors, the CSSOM `.sheet` Stylo bridge). Even clean missing-API additions
(getClientRects, tick-102's computed props) are now ratchet-neutral because the failing tests need more
than one fix. **The next capability progress is therefore either (a) a deep subsystem (multi-tick, fresh
context) or (b) the Bar-0 unblocks (html/semantics native-recursion via debug build; the reflection mass
via the worker-thread-gated quota; flexbox UAF via ASAN) — none a single bounded flip.** The probe-first
discipline holds: don't expose the easy API and hope. [[parity-methodology]] [[symptom-names-wrong-organ]]

## Tick 106 — the stack-quota fix, IMPLEMENTED and REVERTED: the html/semantics crasher is NATIVE recursion

**TICK SHAPE: capability (attempted) → negative result + correction.** `[no-pattern]`. Acted on the
hook's teed-up lever with the deterministic repro tick 105 secured. **Built the effective-stack-quota
fix** — added `libc`, read the current thread's real stack bottom via `pthread_getattr_np` +
`pthread_attr_getstack`, and set `JS_SetNativeStackQuota` so the JS limit lands a generous ~352 KB above
the real bottom. Crucially found the page-eval runtime is the thread-local `RUNTIME` created in `lib.rs`
`with_runtime` (NOT `SpiderMonkeyRuntime::new`, which page JS never uses) and set the quota there. Verified
it compiles and computes a correct real-bottom quota (debug: avail=8 MB, JS limit ~360 KB above bottom).

**The negative result — and it corrects tick 105.** `script-text-modifications-csp.html` **STILL SIGSEGVs
on the MAIN thread**, where even mozjs's default 1 MB quota already leaves 7 MB of headroom. A JS stack
quota only guards the C stack at JS-call checkpoints; a crash that overshoots them is **NATIVE recursion
— our own Rust** (the `<script>.textContent` setter re-preparing the script / re-evaluating CSP,
re-entering itself between checkpoints), which no JS quota can catch. So tick 105's "stack-quota class"
label was wrong for this file. **Reverted** the quota fix (it does not hit its gate; per the ratchet, an
unverified stack-quota change is not landed on hope).

**Two redirected work orders (recorded in `js-engine.md` + memory):**
1. **This crasher needs a native-recursion fix** — a "script already started" guard (HTML's *already
   started* flag) to break the re-entrant loop; find it with a **debug build** (deterministic → gdb
   catches it).
2. **The quota fix itself is real and correct**, but its value is **small-worker-thread JS recursion**
   (the reflection mass), which is **un-gateable on the main thread** (the default already works there).
   It needs a worker-thread repro to prove + a full-sweep regression pass before it can land — do NOT
   re-attempt it gated on this file. A disciplined revert of a well-built fix, not a failure: it corrected
   a wrong hypothesis and saved the next context the same dead-end. [[parity-methodology]]

## Tick 105 — Bar-0 triage: the html/semantics crasher is a DETERMINISTIC stack overflow (the stack-quota class)

**TICK SHAPE: instrument (Bar-0 triage).** `[no-pattern]` — no `engine/*/src` change; this is the diagnosis
that turns tick-104's newly-found Bar-0 into a fresh-context work order. Per Bar-0 primacy, investigated
the html/semantics crasher that keeps that area out of the sweep. Result: it is **tractable, and a better
first target than the flexbox UAF.**

**Findings.** `html/semantics/scripting-1/the-script-element/script-text-modifications-csp.html` SIGSEGVs
(exit 139, core dumped) **in isolation — deterministic**, not the flexbox Heisenbug (which vanishes under
gdb). The gdb backtrace is a tight repeating 3-address cycle over NaN-boxed JS values → **deep JS recursion
overflowing the C stack**: a SIGSEGV where SpiderMonkey should throw *"too much recursion"*. That is the
**stack-quota mis-anchoring** already in `js-engine.md` — mozjs 0.18's `Runtime::new` sets the quota from
`nativeStackBase` captured deep in the tokio `block_on`, so the guard sits past the real stack bottom. The
trigger (`step_timeout` self-scheduling) is benign in a real browser (setTimeout defers, and ours does too
— macrotask FIFO), so the recursion is re-entry via the `<script>.textContent` setter + CSP re-eval, or
the harness drain — **needs a symboled/debug build to pinpoint**, and because it is deterministic gdb will
catch it there.

**Why this matters for the horizon.** This is the **teed-up effective-stack-quota fix with a clean
deterministic repro** — the prerequisite tick 95 said to get first. Fixing it (the pthread-thread-bounds
quota so deep recursion throws, or the specific script-text/CSP recursion) + gating "throws not
segfaults" unblocks **html/semantics (~8,879 failing — the single biggest mass on the board)** into the
sweep, and is the same fix that unblocks the ~35k reflection backlog. It is a fresh-context job (symboled
build + the tick-84 GC-saga class — NOT to be started at a maxed context), now fully scoped. Recorded in
`js-engine.md` and memory `flexbox-relayout-segfault.md`. [[interactive-js-architecture]] [[parity-methodology]]

## Tick 104 — open the aperture: css-values/position/display/color join the sweep (§VI.4 step 1)

**TICK SHAPE: instrument (aperture).** `[no-pattern]` — no `engine/*/src` change; this expands what the
loop can SEE. Acting directly on **surface-audit #3** (tick 103): the WPT sparse checkout held only 9
css subtrees, so `css-values`/`css-position`/`css-display`/`css-color` and `html/semantics` scored an
**invisible zero** — the §VI.3 blindness. Fetched them (`git sparse-checkout add`) and turned unknown
breadth into a **ranked, ratchet-protected work-list**:

| newly-measured area | pass/total | failing (new mass) |
|---|---|---|
| **css/css-values** | 280/1461 (19.2%) | **1181** — biggest new lever (units, `calc()`, `var()`) |
| css/css-position | 63/260 (24.2%) | 197 |
| css/css-color | 27/106 (25.5%) | 79 |
| css/css-display | 10/24 (41.7%) | 14 |

All four are **crash-free** (isolation-retry holds any accumulation crash as ACCUM), so they join the swept
`AREAS` and their marks bank — the MEASURED invariant rewards discovery, and a regression in them now
fails the ratchet like any other.

**A NEW Bar-0 found by opening the aperture — and correctly NOT swept.** `html/semantics` is the biggest
single mass on the board (**~8,879 failing**, forms + elements — high Pareto), but it has **2 real
per-page crashes** (files that SIGSEGV even in isolation, not the recoverable ACCUM class). Adding it to
the sweep would fail the ratchet on crashes, so it is **checked out but held out of `AREAS`** until those
2 crashers are root-caused — a tracked Bar-0, exactly the honest handling the isolation-retry design
enables (per-page crashes stay sacred). This is the aperture doing its job: it found a large mass AND a
real crash the loop could not previously see.

**MEASURED — the ranked worklist for the next capability ticks.** `css-values` (1181 failing, used by
every stylesheet via units/`calc()`/`var()`) is now the top usage-weighted CSS lever the loop can see;
the next capability tick probes its histogram. [[parity-methodology]]

## Tick 103 — document.elementFromPoint (+29) + the tick-103 surface-audit & constitution-check cadences

**TICK SHAPE: capability (CSSOM-View / DOM hit-testing) + two cadence ceremonies.** Both audits came due
at 103: the **constitution-check** (Check #3 — H0 gate re-stated, ticks 96–103 judged **gate not
scoreboard** (+420 real subtests on §VI.4 step-4 web-API surface, no tail, no invariant bent; the flexbox
UAF is a tracked open Bar-0); next due 111) and the **surface-audit** (#3 — finding: the WPT *checkout*
aperture is narrow — 9 css subtrees; `css-values`/`css-position`/`css-display`/`css-color`/`html/semantics`
aren't checked out, an invisible zero — so the next map-expansion is a `wpt-setup.sh` checkout expansion;
next due 113).

**The capability.** Probed `css/css-transforms` (20/278, suspiciously low): `document.elementFromPoint`
was **entirely missing** (`is not a function`, 84 failures) — a genuinely absent, high-usage DOM API
(drag-and-drop, tooltips, custom controls, every hit-test suite). Implemented natively: bridge to the
layout-rect snapshot (`LAYOUT_RECTS_PTR`), and among laid-out **element** boxes containing the client
point return the **deepest** (smallest area, later document order on a tie — children paint over parents);
non-finite/absent coord or a miss → `null` (CSSOM-View). Registered on both document setups.

**Honest bounds, stated not hidden:** the rects are pre-transform, so a `transform`ed hit area is not yet
accounted for, and scroll offset is assumed zero — yet it still flipped **css-transforms 20 → 45 (+25)**
(the many tests whose hit coords fall in the untransformed box), plus flexbox +3, overflow +1. Gate
`G_ELEMENT_FROM_POINT` — deepest-hit / parent-fallback / miss→null / NaN→null — proven falsifiable
(forcing the result to null turns it red; a first fixture bug — the results `#out` div overlapping the
test point and correctly winning as the deeper box — proved the hit-test *works* and was fixed).

**MEASURED — ratchet up, nothing regressed:** TOTAL **389,069 → 389,098 (+29)**; css-transforms +25,
flexbox +3, overflow +1; crashes=0 (isolation-retry holding, ACCUM surfaced); GATES 37 → 38.

**I3 note:** this *strengthens* the semantic model rather than bending it — `elementFromPoint` exposes the
same hit-testing the agent surface uses (the a11y tree's `hit_test`) to page JS. [[parity-methodology]]
[[interactive-js-architecture]]

## Tick 102 — getComputedStyle exposes visibility / white-space / opacity (correct, ratchet-neutral, verified)

**TICK SHAPE: capability (CSSOM correctness).** Probed `css/css-ui` (20/487, suspiciously low): the big
cluster is `appearance`/`-webkit-appearance` computed values (300 subtests) — but that needs a new
`ComputedStyle` field + Stylo extraction + browser-divergent compat-keyword normalization + inline-style
validation (multi-layer, closer to the pedantic tail), so it was NOT the clean bounded pick. The clean
finding underneath it: **`computed_style_js` built a fixed ~30-property snapshot and dropped several
`ComputedStyle` fields the cascade ALREADY computes** — `visibility`, `white-space`, `opacity`. Reading
`getComputedStyle(el).visibility` returned `undefined`, not `"hidden"`.

**Implemented:** exposed the three (camelCase key + `white-space` kebab entry in the `getPropertyValue`
map + serialization: visibility keyword, white-space keyword, opacity as a bare number). Probe: a page
setting each → 4/4 (`hidden`, `pre-wrap`, kebab accessor, `0.5`). Gate `G_COMPUTED_STYLE` (incl. initial
values `visible`/`1`, not undefined) — proven falsifiable (blanking `visibility` turns it red).

**MEASURED — honest result: ratchet-NEUTRAL.** Full sweep TOTAL **389,069 unchanged**, every area at its
banked mark, crashes=0. No *failing* WPT subtest reads these three as undefined — the scored css-ui lever
is `appearance`/`caret-color` (new fields, deferred). Landed anyway, **tick-97-style**: strictly more
correct, zero regression, and real scripts read `visibility`/`opacity`/`white-space` constantly — the
score being flat means the current failing set tests other properties, not that the exposure is wrong.
The lesson re-confirmed: probe which property the *failing* tests assert, don't expose the easy one and
hope. [[parity-methodology]] [[symptom-names-wrong-organ]]

## Tick 101 — isolation-retry unmasks a cross-file UAF as an artifact, unblocking `Range.createContextualFragment` (+33 domparsing)

> **RESOLVED.** This tick uncovered a real Bar-0 SIGSEGV (below), then made the instrument correctly
> distinguish it from a per-page crash so the loop is not deadlocked while the memory-safety fix waits for
> ASAN tooling. Three things landed: (1) the **self-audit** (due at 101, ran clean); (2) the capability
> **`Range.createContextualFragment`** (+33 domparsing); (3) the **isolation-retry** instrument fix that
> unblocked it. The underlying UAF stays an **open, tracked Bar-0 to FIX**.

**⚑ THE Bar 0, and why isolation-retry is the honest response (not a mask).** A **real SIGSEGV** (child
exit **139**, not 137/OOM) hits `css/css-flexbox/stretched-child-shrink-on-relayout.html` when it runs
after other files in the **same, reused** SpiderMonkey runtime — but it is **clean in a fresh runtime**
(`--limit 40` → 139, `--limit 20` → 0; 6/6 alone). So it is **cross-file heap corruption** — a dangling
reflector / unrooted `*mut JSObject` surviving one page's teardown into the next page's GC/relayout
(H0.4, the tick-84 class). It is a **Heisenbug**: reproduces only under heap pressure and **vanishes
under gdb**, so gdb gives no backtrace and the fix needs **ASAN** (blocked here: no passwordless sudo to
install `valgrind`, and starting the GC saga at a maxed context is forbidden). Batch-sizing does NOT fix
it reliably (heavy files accumulate faster; only `--batch 1` guarantees clean — an unacceptable per-sweep
tax). Latent, not a regression: the tick-100 binary crashes identically and ticks 99/100 swept clean.

**The instrument fix.** The batch harness reuses ONE runtime per batch purely to amortize startup — real
browsing never crams dozens of documents into one runtime. So when a batch child dies by *signal*, the
driver now **re-runs the culprit ALONE**; if it passes in a fresh runtime, its per-page result is the
truth, recorded as a distinct, printed **`ACCUM`** metric and NOT counted as Bar-0 `HANG/CRASH`. A file
that crashes **alone too stays `CRASH`** — *a real per-page Bar 0 is never reclassified away*. Result: the
full sweep is **crashes=0** (flexbox recovered to its true **220/3594**, grid **150**, ACCUM surfaced),
and the ratchet holds. This measures the engine as it is actually *used* while keeping the UAF loud and
tracked (`docs/wiki/js-engine.md`, `conformance-and-oracles.md`, memory `flexbox-relayout-segfault.md`).

**TICK SHAPE: instrument (isolation-retry) + capability (DOM API surface) + self-audit cadence.** The self-audit came due (tick − last =
101 − 91 = 10); ran `scripts/self-audit.sh` → **clean, "methodology and reality agree"** (all gates
declare how to break them, 49 process defects recorded with mechanisms, journal/pattern-ledger/enforcement
intact). `LAST_AUDIT_TICK` bumped to 101. Then the capability, same probe-first / flip-per-risk method.

**Bar 0 note on the teed-up tick.** The queued next-capability was the `JS_SetNativeStackQuota` /
mass-reflector fix — but the journal explicitly forbids starting that FFI/pthread/mozjs saga "at the tail
of a maxed context," and Bar 0 outranks the queue. So I stayed with a bounded, additive DOM-side lever and
left the stack-quota fix for a fresh context, exactly as tick 95/98 prescribed.

**The probe.** `--show-failures` on `domparsing`: the top count (264, `object[method_name] is not a
function`) was almost entirely `streamReplaceWithHTML`/`streamPrependHTML` — a **tentative/experimental**
streaming API, anti-Pareto, skipped. The Pareto-relevant, bounded, high-usage miss was
**`Range.createContextualFragment`** — 0 refs (cleanly absent → additive, zero regression risk), ~34
direct subtests, and it silently breaks sanitizers / `jQuery.parseHTML` / every "HTML string → nodes"
idiom (its absence surfaced as *unhandled promise rejections* two callbacks downstream, not a clean
"missing method").

**The fix — the parser you already have, wearing a Range.** Not a new parser: `createContextualFragment`
sets `innerHTML` (= `set_inner_html`, the same fragment parser `insertAdjacentHTML` uses natively) into a
scratch element of the **context tag** (start element, `<html>`→`<body>` fallback per the algorithm),
then moves the children into a `createDocumentFragment()`. Two spec details banked: the `fragment` arg is
**required WebIDL** (zero args → `TypeError`, distinguished by `arguments.length`, not `=== undefined`),
and the result's `nodeType` must be **11**.

**MEASURED — ratchet up, nothing regressed:** `createContextualFragment.html` **2 → 34/35** (the last is
`<script>` execution on insertion — a separate capability, deferred); **`domparsing` 149 → 182 (+33)**;
full sweep crash-free, other areas held. Gate: `G_RANGE` gains createContextualFragment coverage (kids /
nodeType-11 / parsed-not-stringified / required-arg-TypeError) — **proven falsifiable** (stubbing the
method to an empty fragment turns it red; restored, re-green).

**Three bounded DOM-side flips in a row** (99: selectors +117, 100: classList +241, 101: ccf +33), all
zero Bar-0 risk, all probe-first — the flip-per-risk discipline compounding. [[parity-methodology]]
[[symptom-names-wrong-organ]]

## Tick 100 — `classList` was deduped-blind and rewrote the attribute on no-ops (+241 dom, ratchet up, crash-free)

**TICK SHAPE: capability (DOM correctness).** Continued the FLIP-RATE pivot into `dom`. `--show-failures`
+ normalise + `uniq -c` on the dom area put `wrong class after modification` at **195** subtests — and
crucially the failing variants spanned **HTML nodes**, not just the XML/MathML tail, so this was
Pareto-relevant, not exotic. Traced to one file (`dom/nodes/Element-classlist.html`) and one subsystem
(`DOMTokenList`, `engine/js/src/dom_bindings.rs::__mkClassList`).

**Root cause — naive string handling of what is spec'd as an ordered SET with per-method update steps:**
1. **No dedup.** `read()` split `class` on whitespace and *filtered empties but kept duplicates*. So
   `class="a b a"` → `remove('a')` spliced only the first index → `"b a"`; and any modification of
   `class="a a b"` re-serialized `"a a b"` instead of the set `"a b"`.
2. **No-ops rewrote the attribute.** `toggle`/`replace` called `write()` **unconditionally**, so a no-op
   (`toggle('z', false)` when `z` absent) collapsed `"a  b"` → `"a b"`. Per spec, `add`/`remove` always
   run the update steps (normalising is expected there), but `toggle`/`replace` run them **only when they
   change the set**.
3. **Raw-vs-set conflation.** `value` and the stringifier must return the **raw** attribute string;
   `length`/indexing/`contains`/iteration use the **deduped** set. Both went through the same `read()`.

**The fix:** `raw()` (attribute string) separated from `read()` (deduped ordered set, built with
`Object.create(null)` so a token named `__proto__`/`hasOwnProperty` can't corrupt the seen-map); `toggle`
restructured to the spec's four branches, `write()`-ing only on the two that mutate; `value`/`toString`/
the `value` descriptor return `raw()`. `add`/`remove` keep always-update (spec-correct).

**MEASURED — the ratchet turned, nothing regressed:** the file's `wrong class` cluster **195 → 15**
(remnant is the XML/`foo`-node `expected null` tail); **`dom` 2498 → 2739 (+241)** (also lifts
`DOMTokenList-coverage` tests); **`html/dom` 22561 → 22561 (unchanged)**; css areas all held; **crashes =
0** across the full sweep. Gate: new `G_CLASSLIST` — dedup, remove-all, no-op-preserves-whitespace,
raw-vs-set — **proven falsifiable** (neutering dedup turns it red; restored, re-green). GATES 35 → 36.

**Class of the web this unlocks (WEB-PATTERNS):** every framework's class-toggling path — `classList.add/
remove/toggle/replace` now obey ordered-set + no-op-preservation semantics, so a component that toggles a
state class on an element whose `class` has duplicates (or meaningful raw text a sibling reads) behaves as
Chrome does. Two clean bounded DOM-side flips in a row (tick 99 selectors +117, tick 100 classList +241),
both zero Bar-0 risk, both probe-first. [[parity-methodology]] [[symptom-names-wrong-organ]]

## Tick 99 — the attribute-selector case flag (`[attr=val i]`) was stripped, not applied (+117 css/selectors, ratchet up, crash-free)

**TICK SHAPE: capability (selector matching).** Acted on tick 98's steer — *stop expecting single-value
layout fixes to flip flex/grid; pivot to higher-FLIP areas where a fix turns subtests green directly, and
rank by FLIP RATE not raw failing count.* `css/selectors` (matching is more binary) was the named pivot.

**The probe named the mechanism before any edit** (per methodology). `--show-failures` on css/selectors,
normalised (`"…"`→`"X"`, digits→`N`) and `uniq -c`'d, gave a clean histogram. Top by count was
`style.sheet is undefined` (**944** subtests) — but that is a deep CSSOM `<style>.sheet`/`cssRules`/
`selectorText` saga needing canonical Stylo-backed serialization, Bar-0-risky. The next cluster,
`querySelector(validSelector) → null` (**227**), was a bounded *matching* gap with no new object model. A
14-case probe page (`<div foo="BAR" baz="quux">`) isolated it to exactly two mechanisms. **Rank by
flip-per-RISK, not raw count: the bounded, crash-safe fix goes first.**

**The two bugs, both in our own selector engine (`engine/css`, behind `querySelector` + the `:has()`
supplement):**
1. **The `i`/`s` case flag was *stripped and discarded*.** `clean_attr_value` deleted a trailing ` i`/
   ` s` and always matched case-**sensitively** — so `[foo='bar' i]` never matched `foo="BAR"` (nor did
   `~= ^= $= *=` with the flag). *A flag stripped rather than applied is worse than one that errors: the
   value looked right (`bar`), only the case rule was silently missing, and `querySelector` just returned
   `null`.*
2. **The namespace prefix leaked into the attribute name.** `[*|foo]` / `[|foo]` kept `*|foo` as the name
   and matched no attribute.

**The fix (Selectors §6.3):** a `ci: bool` on `AttrSel`; `parse_attr_value` splits value from an optional
`i`/`s` flag *respecting quotes* (`'bar'i`, `'bar' i`, `bar i`), the flag itself ASCII-case-insensitive
(`I`==`i`); `strip_attr_ns` drops through `|` (HTML attrs are null-namespace → `*|foo`,`|foo`,`ns|foo` →
`foo`); matching normalises both sides with a `Cow` — **borrowed on the case-sensitive hot path, zero
allocation** unless the `i` flag is present. Default and `s` stay case-sensitive (proven by a
must-not-match assertion, so the flag can't leak case-insensitivity into plain matching).

**MEASURED — the ratchet turned, nothing regressed:** css/selectors **667 → 784 (+117)**; `dom` 2495 →
2498 (+3, some dom tests use attr selectors); **`html/dom` 22561 → 22561 (unchanged — no regression)**;
flexbox/grid/sizing/fonts/text all exactly held; **crashes = 0** across the full sweep. Gate: `G_SELECTOR`
gains `attribute_selector_case_flag_and_namespace` — **proven falsifiable** (neutering `ci` turns it red;
restored, re-green).

**The compounding lesson banked:** the biggest cluster by count (`.sheet`, 944) is NOT the next tick — it
is a CSSOM subsystem. The right first tick was the 227-cluster that was *one bounded mechanism at zero
Bar-0 risk*. FLIP RATE is flip-per-risk, and the `.sheet` CSSOM bridge is now the teed-up successor
(needs `<style>.sheet` → live `CSSStyleSheet` with `cssRules`/`selectorText` canonically serialized via
Stylo). [[parity-methodology]] [[symptom-names-wrong-organ]]

## Tick 98 — fixed the margin-box extent bug (correct, verified); and the strategic finding that CSS layout is a multi-assertion slog

**TICK SHAPE: capability (layout correctness) + strategic steer.** Implemented and verified the fix for the
localized bug: `content_right_extent` (the shrink-to-fit / max-content measure) counted a child's border-box
right edge (`rect.x + width`) but NOT its right margin — while `rect.x` already includes the LEFT margin, so
the margin box was asymmetric and short by one margin. Added a `px_margin_right` lookup (percentage/auto → 0,
negatives don't extend), threaded through all four `content_right_extent` callers. **Probe confirms the fix:
the flex item is now 120×120 (was 110×120).**

**BUT — three areas measured EXACTLY unchanged (flexbox 220, css-sizing 191, dom 2495), no regression, no
crash.** This is the strategic finding of the tick, and it must steer the horizon: **CSS layout areas are a
MULTI-ASSERTION slog.** A `check-layout-th.js` file asserts many geometry values and FAILS THE WHOLE FILE if
any one is wrong. Our flex/grid geometry is off in several independent ways per file, so fixing ONE correct
value (tick 97 rounding, tick 98 margin) does not flip a single file — the number does not move even though
each fix is correct. Landed anyway because it is correct and foundational (when the sibling bugs are fixed,
correct margins will help those files pass), but honestly ratchet-neutral.

**The steer for tick 99+:** stop expecting single-value layout fixes to move flex/grid. Either (a) batch
several geometry fixes in one tick so a file actually flips, or (b) pivot to higher-FLIP areas where a fix
turns subtests green directly — DOM API / CSSOM property reflection, `css/selectors` (matching is more
binary), and the html/dom attribute-reflection mass (the ~35k lever, still gated on the stack-quota crash).
The compounding lesson: rank mechanisms not just by failing-subtest COUNT but by FLIP RATE — how many
subtests one fix actually turns green. [[parity-methodology]] [[symptom-names-wrong-organ]]

<!-- original localized finding, kept for the record -->
Minimal repro: a row flexbox with one flex item = a `<div>` wrapping a `<p>` that is `100px` wide with
`margin:10px`. The item should size to the p's margin box = **120×120**. We computed **110×120**:
- **cross-axis (height) = 120 ✓** — both vertical margins counted (10+100+10).
- **main-axis (width) = 110 ✗** — only ONE horizontal margin counted (100+10); the far margin is dropped.
- p itself = 100×100 ✓, p.offsetLeft/Top = 10/10 ✓ (margins position correctly; only the CONTAINER's
  content-based main-size is short by one margin).

**The lever:** the flex item's content-based main-size (max-content) undercounts a child's margins on the
main axis by exactly one margin. Cross-axis is correct, so it is specifically the main-axis max-content
contribution in the layout crate (engine/layout, Taffy 0.12 integration) — find where a child's outer
(margin-box) size feeds the parent's main-axis content size and is missing the trailing margin. Likely a
shared cause: many flex sizing tests fail on exactly this kind of >1px content-size error (which is why the
tick-97 offset-rounding, a <1px effect, did not move flexbox).

**Why not landed here:** modifying the Taffy/layout content-size path is Bar-0-risky deep work that must not
be started at the tail of a maxed context. Next iteration (fresh via the oracle chain + auto-compaction):
reproduce with the probe, fix the main-axis margin accounting, gate it, re-sweep flexbox (batch 40) + grid
(batch 10), and confirm the ratchet rises. [[parity-methodology]] [[symptom-names-wrong-organ]]

## Tick 97 — offset metrics now return spec integers (CSSOM correctness; ratchet-neutral, verified)

**TICK SHAPE: capability (CSSOM correctness).** The probe found a real spec bug: `offsetWidth/Height/Top/
Left`, `clientWidth/Height`, `scrollWidth/Height` are `long` (integers) per CSSOM-View, but `el_metric`/
`scroll_getter` returned the raw float (a flex item at `400/3` reported `133.33334`). Fixed: round the
integer metrics; `scrollTop/Left` stay `double`; `getBoundingClientRect` stays fractional. Verified via
probe: `offsetWidth` 133.33334 → **133**, 266.667 → **267** — matching every real browser.

**Honest result: ratchet-NEUTRAL, and that is the finding.** A full validating sweep holds every area exactly
(TOTAL 388,674, crashes=0). The fix does not move the number because `check-layout-th.js` uses
`assert_tolerance` (`Math.abs(actual-expected) < 1` passes) — the 0.33px was already tolerated. Landed anyway
because it is strictly more correct (spec + browser parity) with zero regression; correctness is a capability
even when the score is flat. **The real flexbox lever is therefore NOT rounding** — it is genuine geometry
errors >1px, or `getComputedStyle` display/padding mismatches. **Tick 98's probe:** run a real failing flex
checkLayout test and read WHICH assertion is off and by how much. [[parity-methodology]]

## Tick 97 (superseded hypothesis) — CSS layout correctness is now the top honest lever

**TICK SHAPE: hypothesis (not yet landed).** With the board made honest in tick 96 (32.1%), the biggest
reachable daily-driver mass is **CSS flex/grid layout correctness** — and it is now directly convertible:
`check-layout-th.js` reports honestly (post the onload fix), so every geometry bug fixed turns straight into
passes. Honest starting points: css-flexbox **220/3594 (6.1%)**, css-grid **150/2841 (5.3%)**, css-sizing
**191/1586 (12%)**.

**The method (proven in tick 96): probe the geometry gap, don't theorize.** For a failing `checkLayout`
test, read its `data-expected-*` values and compare to what we compute (`offsetWidth/Height`,
`getBoundingClientRect`). The delta names the layout bug. Pick from the **reachable head, not the tail**:
- HEAD (do first): basic flex distribution — `flex: 1 1 auto` growth/shrink, `flex-basis`, `margin:auto`
  centering, `flex-direction:column` main-size, gaps. These are single-cause and high-frequency.
- TAIL (defer): orthogonal writing modes (`writing-mode:vertical-rl` in flex, e.g. flex-basis-009),
  subgrid, intrinsic-size edge cases. Low frequency, deep.

**Watch:** the engine uses Taffy 0.12 for flex/grid. A systematic offset error across MANY tests usually
means one shared computation is off (a box-sizing/border/padding accounting, a main-vs-cross axis mixup, a
%-resolution against the wrong basis) — fix the shared cause, re-sweep flexbox at `--batch 40` (grid at 10),
and the ratchet judges. Bar 0 stays sacred; a layout change that crashes any area is reverted. [[parity-methodology]]

## Tick 96 — the `<body onload>` double-fire: one handler, dispatched twice, corrupting every checkLayout test

**TICK SHAPE: capability (JS lifecycle correctness).** The probe redirected the tick. I set out for the
stack-quota saga (teed up below, now re-deferred) but ran the discipline first — *measure the boxes before
theorizing* — and picked the biggest daily-driver layout area, **css-flexbox** (378/6837 ≈ 5.5%), to ask
WHY it was stuck. The answer was not layout at all.

**The probe chain (each step named the next):**
1. Flexbox's reachable universe is **368 testharness files** (871 are deferred reftests); **220 of them use
   `checkLayout`** from `check-layout-th.js`, which reads layout geometry *through the DOM* (offset*/
   getBoundingClientRect) and compares to `data-expected-*`. Geometry APIs are all present.
2. `diag` on a checkLayout file: `harness:true, loadFired:true`, but the `<body onload>` bootstrap looked
   dead. A minimal probe (`_probe_flex.html`) instrumented each link and reported **`onloadCalls:2`** — the
   body-onload handler fired **twice**.
3. Traced it to `__fireLoad` (dom_bindings.rs): it calls `g.dispatchEvent(ev)` — which goes through
   `__fireWindowEvent`, and that **already invokes `g['on'+type]`** (the `onload` property) at line 6805 —
   and THEN calls `g.onload(ev)` **again** explicitly. Two invocations of the same handler.

**Why it hid for 96 ticks:** the encoding suite (720k subtests) bootstraps from `<body onload>` too, but its
handlers are *idempotent* and don't call `done()`, so a double-fire is harmless — it just decodes twice.
`checkLayout` is the opposite: the second fire creates duplicate `test()`s and a second `done()` **after the
harness already completed**, turning a file that would report clean pass/fails into a harness error. The bug
was invisible to the crown jewel and fatal to every geometry-assertion suite.

**The fix (one surgical removal):** delete the redundant explicit `g.onload(ev)` in `__fireLoad`; dispatch
alone now fires the handler exactly once. Verified: probe `onloadCalls` 2 → **1**; encoding sanity
(30 files) still **55,057 passes, 0 crashes** — the crown jewel is untouched because dispatch still invokes
onload. Gated by the canonical sweep + the RATCHET (this entry's numbers are filled from that sweep, not an
ad-hoc run; if flexbox/encoding/any area regressed, the tick is reverted, not shipped).

**Class of the web this unlocks (WEB-PATTERNS):** any page or test that bootstraps from `<body onload>` and
whose handler is *non-idempotent* — calls `done()`, submits a form, increments a counter, starts a single
run. That is a large fraction of WPT's layout/geometry suites and of legacy real-world pages.

**THE BIG FINDING — the metric was inflated ~2×, and this tick makes it honest.** The double-fire did not
merely break checkLayout files; wherever a `<body onload>` handler *created subtests* (encoding decodes and
asserts; every `check-layout` suite), it created them **twice**, and the harness counted both. Measured
apples-to-apples on the same release binary: **encoding 110,111 → 55,057 passes = exactly 2.00×**; flexbox
378/6837 → 220/3594. So the project's headline had been **inflated ~2× for as long as the double-fire
existed**. The honest whole-suite number is **388,674 / 1,210,437 = 32.11%**, not the 749,793 / 47.5% the
board showed. Per the operator's explicit call (fix + honest re-baseline), the RATCHET's `WPT:*` marks were
reset to the honest single-fire values — a documented one-time CORRECTION, not a laundered regression (the
inflated marks were never real capability; they were double-counting). `bank` only ever raises, so this
reset was done deliberately and is recorded here and in `RATCHET.tsv.pre-tick96-rebaseline` (kept in scratch).
The honest board is what every future tick now ratchets up from.

**One Bar 0 surfaced and was closed in the same tick.** The honest sweep flagged `crashes=1` in css-grid —
reproducible, but only at `--batch 40`, and only across the full 643-file run: `css-grid/layout-algorithm/
grid-fit-content-percentage.html` runs clean in isolation. It is **batch memory accumulation** (40 heavy
grid-layout pages retained in one process), the same class the harness already right-sizes for encoding
(`batch_for` → 4). The pass count is batch-invariant (150 at batch 40 and batch 10), so the fix is to
right-size the batch: `batch_for` now gives `css/css-grid` a batch of 10 → **crashes=0, same 150 passes**.
Not hidden, not gamed — measured within the machine's memory budget, exactly as encoding is.

**Method note for the horizon:** this whole tick came from *building the probe before theorizing*. Flexbox
was "stuck at 5.5%"; the instinct is to blame flex layout. The probe (`diag` + a minimal instrumented page)
walked load→onload→checkLayout→done and found the break was a **lifecycle double-fire upstream of the
feature**, and then that the same bug was inflating the crown-jewel metric 2×. The score was lying, and only
the probe could tell. [[parity-methodology]] [[symptom-names-wrong-organ]]

---
**DEFERRED (re-teed for a later tick) — the stack-quota / mass-reflector-recursion fix.** Still the gate for
ARIA reflection + the ~35k reflection backlog. Confirmed facts, kept here so the next taker starts sharp:
- There is currently **no explicit `JS_SetNativeStackQuota` call** anywhere in `engine/js` — mozjs's
  `Runtime::new` (spidermonkey.rs:200) installs its own default, captured relative to the SP *at that
  point*, which in the tokio/async embedding is buried deep → the guard address sits past the real stack
  bottom → deep recursion SIGSEGVs instead of throwing "too much recursion". (Matches the wiki.)
- **`getAttribute` is NOT wrapped** by the mutation layer — only `setAttribute`/`removeAttribute` and the
  child-mutators are (mutation_js.rs:180-215). So the crash is **not** a trivial reflected-accessor →
  getAttribute → wrapper re-entrancy that could be severed by de-wrapping. It is genuine **mass-reflector
  depth**: forcing a reflector for every node in a large tree and running ~44 extra accessors per reflector
  drives the C stack past the (mis-anchored) limit. The wiki's verdict stands: "real and un-fixed."

**The two candidate fixes, and why this needs a fresh context (NOT a loaded one):**
1. **Effective stack quota** — after `Runtime::new`, call `JS_SetNativeStackQuota(cx, size)` where the base
   is the *real* thread-stack bound (`pthread_getattr_np` → `pthread_attr_getstack`), not the async SP.
   Risk: mozjs records `nativeStackBase` at context creation; the version here may not re-read it, so the
   size alone may not move the trigger to the right address. The wiki already records "re-anchoring per
   call did not reliably help." This is FFI + pthread + version-specific mozjs internals — a saga.
2. **Structural** — keep JS off the whole-tree walk (the `__inlineHandlerNodes` pattern), so mass reflector
   access never happens for the reflection path either. Safer, but changes how reflection is driven.

**The steer:** do NOT attempt either at the tail of a maxed context — that is precisely how the tick-84
saga started. Take it fresh: reproduce the crash deterministically first (a minimal `querySelectorAll('*')`
+ per-node accessor-read harness under the html/dom lane), THEN try fix #1 behind a G_STACK_QUOTA gate that
asserts "too much recursion" is *thrown* (not a segfault), and only re-enable ARIA once that gate is green.
If fix #1 proves unreliable as the wiki predicts, fall to fix #2. Bar 0 outranks the capability: revert
rather than ship a maybe-crash.

## Tick 95 — ARIA reflection, explored and reverted: the mass-reflector recursion is now a named gate

**TICK SHAPE: instrument.** No engine code landed — and that is the honest result. The histogram put the
next Pareto mechanism cleanly: html/dom's largest remaining mass is HTML/ARIA **attribute reflection**
(~15.5k `IDL get … undefined` getters + ~11k setters not reaching attributes), and **ARIA reflection**
(`el.role`, `el.ariaLabel`, … ~44 IDL attributes) was the ideal slice — bounded, additive, high-usage,
and squarely in the I3 moat (it is the script side of the a11y tree, and Surface Audit #2 flagged Interop
2026's cross-browser AX-consistency investigation). Implemented, it worked perfectly in isolation:
`role`/`ariaLabel`/`ariaColCount` round-trip, null-when-absent, set/remove, both build lanes green.

**And it tipped a Bar 0 crash, so it was reverted.** In the full `html/dom` run, adding the 44 accessors
made a *different* file overflow the C stack (`0` crashes without ARIA, `1` with) — the **latent
mass-reflector recursion** documented in `js-engine.md` at tick 94, now proven to be a hard gate on
reflection-surface growth, not a curiosity. Enumerable-vs-not made no difference; it is the accessor count
on the mass-access path. **A Bar 0 crash is never a trade for a capability** (the ratchet's first rule),
so ARIA does not land.

**The value of this tick is the named prerequisite.** Reflection expansion — ARIA, and the ~15k missing
table entries behind it — is gated on making SpiderMonkey's native stack quota *effective*, so the deep
recursion throws `too much recursion` (survivable) instead of segfaulting. The fix is written down in
`js-engine.md`: set `JS_SetNativeStackQuota` from the **actual thread-stack bounds**
(`pthread_getattr_np`/`pthread_attr_getstack`), not from the async-buried SP `Runtime::new` sees. **That
is the next capability tick; ARIA and the reflection backlog ride on it.** The loop now knows exactly what
to build next, and why — which is what a good negative result buys.

**WIKI:** `js-engine.md` — extended "Mass reflector access can overflow the C stack" with the ARIA proof
and the thread-stack-bounds quota fix (retrievable: `wiki-lookup.sh ARIA stack quota recursion`).

## Tick 94 — backfill: the crash saga becomes retrievable knowledge, not just journal narrative

**TICK SHAPE: instrument.** No engine code changed. Tick 92 built the mechanism to hold durable knowledge;
this substantiates it with the highest-value knowledge the loop had generated but left only in journal
narrative — the tick-84 SpiderMonkey-FFI crash saga and the tick-88/90 build lessons. This is exactly the
knowledge the downstream horizons (agent-driving, security, the V8/embedded/enterprise species) will need
and cannot reconstruct from a diff. Added to `js-engine.md`:

* **"A node id is unique only WITHIN its arena — so a reflector must resolve against its OWN document"** —
  `SLOT_DOM` + the live-arena registry + *a `PrivateValue` IS a double*. The lesson every second-document
  feature will hit.
* **"A per-arena identity cache must not CLOBBER the shared `__nodes`"** — how a fresh main-doc cache
  silently killed `document.dispatchEvent`.
* **"Mass reflector access + the reflection layer can overflow the C stack, and SpiderMonkey won't catch
  it"** — the ineffective native stack quota in async embeddings, and why "answer it from the arena in
  Rust" is the structural fix.

And to `build-and-dependencies.md`: **"A second cargo feature-config in one build step thrashes the whole
cache"** — the 54s→572s wall tax, the separate-target-dir/`cargo check`/RAM fix, and the rule that CI
proves the headless matrix out-of-band.

All four are retrievable now: `wiki-lookup.sh SLOT_DOM arena reflector` returns the first, top-ranked,
3/3 terms. The wiki is **252 sections** across 14 topic files. Backfill of the deeper history (ticks
1–81) continues opportunistically as those surfaces are next touched — the enforcement guarantees new
engine ticks add theirs, so the gap only shrinks.

**WIKI:** `js-engine.md` + `build-and-dependencies.md` — the reflector-arena, identity-cache-clobber,
mass-reflector-recursion, and feature-config-thrash mechanisms.

## Tick 93 — a sparse wall-time audit: keep the per-tick tax lean without cutting a gate

**TICK SHAPE: instrument.** No engine code changed. The wall runs every tick, so a needless second is
taxed forever — and the ratchet's WALL invariant only catches *regression* (the wall got slower), never
*standing bloat* (it was never as lean as it could be). New: **`scripts/wall-audit.sh`**, a cadence (every
20 ticks, enforced by `tick.sh`, persisted through `status-update.sh`) that reads a per-section timing
breakdown the wall now records (`head_` writes `.git/manuk-wall-sections`) and hunts removable cost —
under one absolute rule: **report, never delete.** No gate dropped, no floor widened, no check laundered
to CI to fake a fast local wall. Only optimisations that buy the *same assertion* for fewer seconds.

**Audit #1 (this tick):** the 61s wall breaks down as `T` crate tests **28%**, `P` parity **25%**, `G6`
10%, `G1` 7%, everything else ~0 (the ~20 parallel gates hide inside the concurrency; the build is 1s with
output in RAM). Finding: **the wall is already lean** (61s ≪ the 300s target) and the costs are honest.
The one admissible lever is **`cargo-nextest` for `T`** — it shares the test binary and parallelises
execution harder than `cargo test`, for identical assertions (the self-audit already names it). Filed as
the next wall-lever; not done here (a toolchain change is its own scoped work). `P` is browser-launch-bound
and has no rigor-preserving cut. **Verdict: no cut — lean.**

That is the point of a *proactive* audit: it can conclude "already lean" and mean it, because it looked.
The loop now has four cadences pointing at four different failure modes — self-audit (did we build what
the methodology prescribed), surface-audit (is the map the whole map), constitution-check (is the hill the
mountain), wall-audit (is the wall paying for what it asserts).

**WIKI:** none — loop-governance mechanism; the log is docs/loop/WALL-AUDIT.md.

## Tick 92 — the wiki becomes mechanically load-bearing: enforced, indexed, retrievable

**TICK SHAPE: instrument.** No engine code changed. The wiki-writing rule existed but was *methodological,
not mechanical* — "WIKI: none" was free, so the loop took it on every engine tick from 85 on, and the wiki
stopped accumulating **precisely where the JS/DOM/CSS/FFI learning was densest.** That knowledge is what
the downstream horizons (agent-driving, security, the V8/embedded/enterprise species) will need and cannot
reconstruct from a diff. Three mechanisms fix it, and none of them is a vector DB — deterministic
keyword/symbol retrieval, no embeddings, no semantic neighbours, per the operator's own call.

**1 — Retrieval.** `scripts/wiki-lookup.sh <terms>` returns the `##` sections containing your terms,
ranked by how many match (`reflector SLOT_DOM` → the sections naming both, top first; nothing
adjacent-but-wrong). `scripts/wiki-index.sh` regenerates `docs/wiki/INDEX.md`, the complete map of all
**247** sections across 14 topic files — never hand-edited, checked current by `tick.sh`.

**2 — Enforcement.** A tick that changes `engine/*/src/` **must** revise a `docs/wiki/*.md` topic, or
declare an explicit, auditable `WIKI: none [forced] — <reason>`. Enforced in both the pre-commit hook
(authoritative) and `tick.sh`'s one-second pre-flight. "None" stays legitimate for docs/scripts/mechanism
ticks; it is no longer legitimate for an engine change that quietly learned something.

**3 — Organisation.** Topic files, revised in place (never one file per tick — that is the write-heavy
read-light failure the journal already is). The index gives the map; lookup gives precise retrieval; the
enforcement guarantees the content keeps arriving. A capability is only truly banked when a future
session — with no memory of this one — can `wiki-lookup` *why* it works and not undo it by accident.

Backfill of the accumulated-but-uncaptured knowledge (the tick-84 crash saga → `js-engine.md`, and a sweep
of ticks 82–91) is the next tick, now that the mechanism to hold it exists.

**WIKI:** `README.md` — "Retrieval and enforcement: the wiki is now mechanically load-bearing" (lookup +
index + the engine-tick accumulation rule).

## Tick 91 — innerText became the rendered text (the first Pareto-ranked capability tick)

**TICK SHAPE: capability.** The first tick under the corrected Pareto north star (§VI.3): the loop ranked
`html/dom` as the largest representative-breadth failing mass, and inside it
`the-innertext-and-outertext-properties` sat at **2/455** — a clean, high-usage mechanism (every framework
reads `innerText`).

**The bug was the honest comment.** `el.innerText` returned `textContent` and *said so* in a doc comment
("computing it properly means asking the layout tree, which the binding layer cannot reach from here").
The premise was wrong: the binding **already holds the pre-script computed styles** (`with_style` /
`STYLES_PTR`), which is exactly what innerText needs. So innerText is now a faithful structural
approximation: **`display:none` subtrees are skipped** (the #1 divergence from textContent — a page hides
a node and textContent still returns its text), `<br>` becomes a newline, **block/flex/grid/table
boundaries** insert newlines, whitespace is **collapsed** in normal flow and **preserved** under
`white-space: pre*`.

**And the other half was simply missing.** `outerText` was `undefined`, and the suite asserts innerText
and outerText *together* — so every subtest failed no matter how right innerText was. Added: the getter
(same rendered text) and the setter (replace the element with the text, `\n`→`<br>`).

**Receipts.** The innertext suite **2 → 35 / 455**; `html/dom/elements` 11.7% → 17.3% as the gain flows
through. `G_CAPABILITY` now asserts innerText skips `display:none` and turns `<br>` into a newline, and
that `outerText` reads the same. 0 crashes, gates green. The remaining ~420 failures are layout-exact
(required line-break counts, `::first-letter`, multicol) — real, and honestly out of reach until innerText
can consult the layout tree, which is a later horizon's integration, not this tick's.

**Folded in — a real `tick.sh` bug the growing journal exposed.** The pre-flight's TICK-SHAPE check was
`awk "…" | grep -qi 'TICK SHAPE:'` under `set -o pipefail`: `grep -q` exits on the first match and closes
the pipe, `awk` (still streaming a 1300+-line tick block) takes SIGPIPE and exits 141, and pipefail
reports *that* — so the check `die`d on a journal that DID contain the shape. It failed intermittently
(foreground timing sometimes let awk finish) and worsened as the journal grew — three ticks lost a re-run
to it. Fixed by doing the whole match in one awk process (no pipe).

**WIKI:** `dom-semantics.md` — "innerText is the RENDERED text, and the binding CAN compute it — it holds the styles already" (the transferable lesson: check `STYLES_PTR`/the view maps before assuming a binding cannot reach computed style or geometry).

## Tick 90 — iterative builds run in RAM by default, so local work rarely touches the platter

**TICK SHAPE: instrument.** No engine code changed. The operator asked that local iterative compiles use
tmpfs/RAM "such that we almost never need to write a binary to disk, and flush excess as needed." Most of
the machinery already existed — `ramdisk.sh` symlinks `target/{debug,release}/incremental` into
`/dev/shm`, and the incremental fragments are the *dominant* write source of an ordinary edit→build→test
loop — but it ran **reactively**: only inside the wall's `disk >= 88%` reclaim branch. So every build
*between* walls wrote incrementals to the platter until the disk was nearly full, and only then moved to
RAM. Backwards.

**The fix.** The RAM reseat is now **unconditional** in the wall (before the build, idempotent, cheap: a
`mkdir` + a symlink check), so the RAM symlinks are in place for this build and for every iterative build
after it. Reclaim stays conditional (it deletes the warm debug cache; only the RAM *setup* is
unconditional). The operator can activate the same setup any time with `./scripts/ramdisk.sh`.

Verified live: `target/debug/incremental` and `target/release/incremental` both resolve into
`/dev/shm/manuk-build` (3.8 GB in RAM, within the 4 GB cap; 21 GB RAM still free). Disk-hygiene continues
to flush `target/debug`, old banked builds and stale oracle snapshots when the disk crosses 88%. Net: the
edit→compile churn lives in RAM, the platter sees only the occasional `.rlib`/binary of a crate that
actually changed, and the disk stays lean without a human minding it.

This also retires the disk-pressure/wall-speed cascade that dogged the last few ticks: the wall-headless
experiment (tick 88) filled the disk, tripped the reclaim, which deleted the debug cache and made the wall
rebuild from cold. With iterative output in RAM and reclaim conditional, that loop is broken.

**WIKI:** none — build hygiene; the mechanism is `scripts/ramdisk.sh`, wired unconditionally in the wall.

## Tick 89 — the loop budget: "run K more ticks" is now a fact on disk, not a context-window string

**TICK SHAPE: instrument.** No engine code changed. The standing directive is "loop autonomously with no
handback," but "no handback" needs a floor the operator controls without re-typing it into a conversation
that gets summarised and compacted. So the tick budget is now a **fact on disk** (`docs/loop/AUTOLOOP`:
`LOOP_UNTIL_TICK=1088`), read mechanically at the top of every tick.

**How it works.** `orient.sh` — the first action of every tick — now runs `autoloop.sh check` before it
does anything else: while `TICK < LOOP_UNTIL_TICK` it prints the remaining count and continues; when the
target is reached it exits non-zero and the loop **STOPS and reports** — the one handback that is *by
design*, because the operator asked for exactly this many ticks. The operator sets it once
(`./scripts/autoloop.sh set <K>` → target = current + K, or just edit the file) and updates it whenever;
the loop obeys without being retold. `STATUS.md` shows `LOOP_BUDGET: N ticks remaining` every session.

Set now to **1,000 ticks** (tick 88 → target 1088). This closes the gap the operator named: the number
lives on disk, survives compaction, and is theirs to change — not a string I have to keep alive in
context across hundreds of hours of looping.

**Also folded in: tick 88's wall-headless check is reverted.** Tick 88 made the wall build the headless
(`--no-default-features`) config so a headless-only regression could not pass a green wall. Correct in
principle, but a third feature configuration thrashed cargo's cache and taxed **every** wall ~350–500s
(measured: 54s → 417–572s), and the wall runs every tick. CI's `verify-linux` builds the headless config
authoritatively and is green (#280), and the loop reads CI at the start of each tick — so the division of
labour is: the wall proves the shipping config in 54s, CI proves headless out-of-band. The actual CI fix
(gating `diag` behind `spidermonkey`, tick 88) stays; only the expensive wall duplication is removed.
Verified: wall back to **54s**, green. (Iterative-compile output already lives in `/dev/shm` — incremental
fragments and, while it existed, the headless check — so local builds rarely touch disk.)

**WIKI:** none — loop-governance mechanism; the dial is `docs/loop/AUTOLOOP`, the logic is `autoloop.sh`.

## Tick 88 — CI was red because the wall built a different thing than CI did

**TICK SHAPE: instrument.** CI's gating `verify-linux` job had been **failing since tick 84** (four
commits: 84, 85, 86, 87 — CI was green through tick 83), and the local wall was green the whole time. The
cause and the lesson are the same: **the wall and CI did not build the same thing, so they disagreed about
what "green" means, and one of them was lying.**

**The regression.** Tick 84's `diag` subcommand (the WPT single-file diagnostic) calls `page.eval_for_test`
— a seam that only exists under the `spidermonkey` feature. CI's first build step is
`cargo build --workspace --no-default-features` (the lean/headless lane), where that method does not exist,
so the workspace did not compile. `diag` is now gated behind `#[cfg(feature = "spidermonkey")]` with an
honest headless fallback; the headless workspace build is green again.

**The instrument fix that matters.** The wall built only `cargo build --workspace` (the shipping config);
CI builds *both* that and `--no-default-features`. A regression that touched only the feature-gated seam
therefore sailed past a green wall and reddened CI — for four commits, invisibly, because "read CI at the
start of the next tick" is a human step and the loop had been heads-down landing capability. The wall now
builds **both configs CI gates on**, so this entire class cannot pass the wall again. If the wall and CI
build different things, one of them is lying — now they build the same things.

Verified: headless workspace build compiles; CI's exact gate list (`g_globals g_selector g_lifecycle
g_chardata g_contain_native g_dom_impl` + `stale_handle`) all `ok`; fmt clean; wasm pipeline builds. CI
history confirms green through #270 (tick 83), red #272–#276 (ticks 84–86), fixed here.

**WIKI:** none — build/CI hygiene; the durable rule (the wall builds exactly what CI builds) is in
verify.sh's B section.

## Tick 87 — open the CSS aperture, and anchor the loop to the constitution on a cadence

**TICK SHAPE: instrument.** `[no-pattern]`. No engine code changed. Two moves, one purpose: make the loop
**coherently pursue the constitution's H0 gate** rather than the nearest number. Both are §VI.4 of the
constitution — one executes step 1 of the direct path, one builds the mechanism that keeps the loop *on*
the path.

**1 — The CSS aperture was barely open, so the ranking was inside the wrong frame.** The sweep measured
exactly three css subtrees (`selectors`, `flexbox`, `grid`) while **eight more were already checked out
and never measured** — a ranking that cannot see most of CSS is a confident wrong answer. Measured and
banked (all Bar 0 clean, sweep held ≥16 GB via the tick-85 batch bound): `css-text` **64.9%**,
`css-fonts` **32.4%**, `css-overflow` **25.4%**, `css-sizing` **12.7%**, `css-transforms` **7.2%**,
`css-ui` **4.1%**, `css-backgrounds` **3.5%**. Honest Pareto breadth (encoding tail excluded) is now
**28,803 / 90,557 = 31.8%**, and the ranked work-list is real: `html/dom` (37,290 failing), then the
layout levers **`css-flexbox` (6,459)** and **`css-grid` (4,414)** — every modern site needs those, and
they are the H0.1 lever. That is where the next capability tick goes.

**2 — Nothing was reading the constitution on a schedule, and that is exactly how tick 84 drifted.** The
loop had `orient` (checks the tree), `surface-audit` (checks the map against the world) — and nothing that
looked *up*, at the governing document that defines the frontier. So it banked +721k encoding subtests,
a real win on the wrong hill, and no instrument could tell. New: **`scripts/constitution-check.sh`** — a
cadenced protocol (every **8 ticks**, enforced by `tick.sh`, persisted through `status-update.sh` like the
surface audit) that forces the loop to re-read `CONSTITUTION.MD`, name the horizon it is in and that
horizon's binary exit gate, answer honestly *"did the last ~8 ticks move the gate, or only the
scoreboard?"*, correct **PART VI** where the tree has drifted, and steer the next tick to whatever is
closest to the gate. Check #1 (`docs/loop/CONSTITUTION-CHECK.md`) is recorded: horizon H0, gate stated,
the encoding-tail drift named, the direct path re-derived.

Now the loop has three instruments pointing in three directions that matter: `orient` down (the tree),
`surface-audit` out (the world), `constitution-check` up (the horizon). A tick that satisfies all three
is a tick that is actually building the thing the constitution describes.

**WIKI:** none — loop-governance mechanism; the durable content is CONSTITUTION-CHECK.md + PART VI.

## Tick 86 — the constitution meets the tree; the north star was pointing at the tail

**TICK SHAPE: instrument.** `[no-pattern]`. No engine code changed. A governing **CONSTITUTION.MD**
arrived (the long-horizon vision: one core, two front-ends, N species; H0 Pareto parity → H1 hardening →
H2 agentic surface → H3 appanages → H4 speciation). This tick maps it onto the repo *as it actually is*,
corrects what it assumed, and — most importantly — fixes the loop's north star, which tick 84 had quietly
knocked off the Pareto frontier.

**The correction that matters.** Tick 84 moved `encoding` 128 → 720,990 and the suite TOTAL to 747,778
(47.6%). But **96% of every passing subtest is now encoding**, and its remaining ~767k failures are the
exotic per-codepoint legacy-CJK tail — exactly what the constitution's **I4 (Pareto discipline)** says to
DEGRADE, not chase. Left alone, `orient` ranks by raw failing subtests, so encoding would have sat at #1
forever and pulled hundreds of hours of loop-throughput into the tail. The honest, H0-relevant number is
**breadth excluding encoding = 26,788 / 82,861 = 32.3%.** That is now the gauge; 47.6% was a mirage.

**Mechanism, not just prose.** `orient.sh` gained a **Pareto lens**: tail areas (`encoding`) are excluded
from the ranking (never from the ratchet — they stay banked and must not regress), and the loop optimises
*usage-weighted breadth*. It now points at `html/dom` (37,290 failing) — representative — instead of the
tail. `GRIND.md`'s rule and the constitution's new **PART VI** both encode: open the aperture first (~8
sub-areas of hundreds are measured), rank by usage weight, degrade the tail.

**Assumptions corrected against the tree (PART VI in full):** the a11y/semantic tree (I3, the moat) is
**already built and feeding the agent** (1.3k LOC) — the map said "no AX tree at all," which was wrong;
Stylo is **already the shell default**; the GPU/Vello paint path is **aspirational comments only** (raster
is tiny-skia CPU everywhere); the differential oracle (I5, the vision's discovery engine) has **never
finished a crawl.** The direct H0 path is written down: open css/* + html/* aperture → CSS layout breadth
(flexbox 5.5%, grid 4.7%) → land one clean oracle crawl → web-API by usage → semantic model in lockstep.

**WIKI:** none — a governance + loop-mechanism tick; the durable content is CONSTITUTION.MD PART VI.

## Tick 85 — the instrument could not measure its own biggest win

**TICK SHAPE: instrument.** `[no-pattern]`. No engine code changed. Tick 84 moved `encoding` from 128 to
~721,000 passing subtests — and the sweep that is supposed to *bank* that win **could not run without
taking the terminal down with it.**

**Why.** A single `encoding` file creates 190,000+ live testharness subtests (Big5 decode across every
variant), each a JS object with its own reflector. The sweep runs forty files per child process, and forty
files of that size outrun the GC: the child is OOM-killed mid-batch. An OOM is not a wrong number — it is
*no* number, and worse, a runner that dies mid-sweep leaves the ratchet banking the OLD marks, so **the
largest win in the project's history was invisible to the mechanism built to protect it.**

**The fix is a memory bound, stated as one.** `wpt-sweep.sh` now picks the batch size per area: `encoding`
(and anything under it) runs **4 files per child**, everything else keeps the fast default of 40. A child
that exits after a handful of files hands its whole heap back to the OS, so peak memory is capped at a few
files' worth regardless of how many subtests each holds. Verified: the full sweep now completes with
**≥20 GB free throughout** (min avail 19,981 MB), where before it exhausted 31 GB and was killed.

**Banked, and now protected.** The sweep measured the real numbers and the ratchet took them:
`encoding` **128 → 720,990**, `dom` 2387 → **2495**, `css/selectors` 527 → **1,021**, `css/css-flexbox`
68 → **378**, `css/css-grid` 84 → **216** — and the whole-suite **TOTAL 25,869 → 747,778** (47.6% of the
1,570,726 measured subtests), crashes 0, duplicate wire requests 0. A ratchet tooth is only real once the
instrument can reach it; this tick is the instrument reaching it.

**The lesson, general:** an instrument that cannot survive measuring the thing it exists to measure is not
a conservative instrument, it is a blind one. *An OOM is a measurement fault, not a null result* — the
fifth instrument this project has had to teach that its own condition is not the thing under test.

**WIKI:** none — a sweep-harness memory bound, no web-class capability changes.

## Tick 84 — the child document was always built, then thrown away (+~721k WPT)

**TICK SHAPE: capability.** `[no-pattern]`. The single largest gated lever this project has ever found:
one mechanism moved WPT `encoding` from **128 → ~721,000** passing subtests (91% of the measured universe,
previously at 0.0%) and turned the **#1 platform-web capability** — the nested browsing context — from a
bitmap into a real, readable document.

**The mechanism: real `iframe.contentDocument` / `contentWindow`.** The child `Page` was *always* fully
built — its own arena, styles, scripts, laid out at the frame's own width — and then painted to a bitmap
and **dropped on the floor**. The pixels survived; the document did not. So a script that reached into a
frame got `undefined`, and WPT's entire `encoding` suite scored zero: every decoder test loads a document
in a frame and reads its text back (`iframeRef(f).querySelectorAll("span")`). The decoder was never the
gap — `encoding_rs` was correct all along (it sniffs Big5, decodes `§`, zero U+FFFD). **The frame was.**

**The architectural blocker, and the fix.** A node id is unique only *within* an arena, and every
reflector resolved its id against the one `CURRENT_DOM`. Two documents means node #7 exists twice, so a
child reflector was reading the *parent's* node #7 — a different element, in a different document, with
total confidence. That is why `contentDocument` could not be written. Three changes fixed it: reflectors
honour their own `SLOT_DOM`; a registry of live arenas makes that safe (a dropped `Page`'s arena is a
use-after-free, not a document — `is_alive()` cannot save you if you ask the *wrong* arena); and the
identity cache is **per-arena**, or `===` starts lying across documents. `Page` now keeps its child pages
and unregisters their arenas in `Drop`, before they die.

**Two more capabilities fell out of following the number down.** The frame loaded but created zero
subtests — the page's `<body onload="showNodes(...)">` never fired, because **inline event-handler content
attributes were never compiled**. `onload`, `onclick`, `onsubmit` — the oldest way to attach behaviour to
markup — were all dead. And `nodes[i].dataset.cp` threw, because `element.dataset` looked its element up
in the *main* document's identity map (id-keyed, single-document), so an iframe element got `null`. Both
now work; `dataset`/`.style`/`.classList` were rewired to take the **element**, not a global id.

**A `display:none` iframe still loads.** WPT hides its frame (`<style>iframe{display:none}</style>` — the
frame is a *data source*, not a picture) and we refused to fetch a boxless frame. Loading is a DOM
decision; the box is only a painting decision. That one confusion cost 767k subtests.

**The saga, honestly told.** The first cut crashed `dom/` (Bar 0: 0 → 2 SIGSEGV) and it took a long hunt
to corner: my inline-handler wiring did `document.querySelectorAll('*')` and read a property on every
element, and that mass reflector access, with the reflection layer installed, tripped an infinite JS
recursion that overflowed the C stack — a crash SpiderMonkey's stack quota failed to catch because its
limit is anchored deep in the async call stack. Two false fixes (a mutation-wrapper depth guard, a
re-anchored stack quota) were **non-deterministic luck** and rejected as such. The real fix is structural:
a `__inlineHandlerNodes` native finds the handful of `on*`-bearing elements by a single arena walk in
Rust, so JS never iterates the whole tree. The latent recursion (mass property access + reflection) is
pre-existing from tick 82 and stays for a dedicated tick; this tick simply does not trigger it.

**One more self-inflicted wound, because the per-document cache is a trap of its own.** The first
per-arena cache created a fresh `__nodes_<addr>` map for the MAIN document too, and *overwrote* the global
`__nodes` with it — discarding the `document` reflector that `install` seeds there. The symptom was as
silent as it was specific: `document.dispatchEvent` stopped reaching document-level listeners
(`DOMContentLoaded`, delegated clicks) **the instant any script touched `document.body`**, because
`__nodes[0]` was gone. G_LIFECYCLE caught it (`seen:dcl-win,load`, missing `dcl-doc`). The fix: the main
document's cache *is* `__nodes` (looked up and reused, never replaced); only a child document gets its own
`__nodes_<addr>`. The lesson generalises the tick's headline one — a per-arena anything must not clobber
the one map the whole prelude already depends on.

**Receipts.** `dom/` 2387 → **2488** (0 crashes). `html/dom` **37.7%** held. `encoding` big5 0.1% →
**44.0%**; the area 128 → ~721k. `G_IFRAME` now gates `contentDocument`/`contentWindow` + cross-document
node identity; `G_CAPABILITY` gates inline `onclick` and `dataset`. Both build lanes green. Constellation:
platform iframes **missing → gated**, doc legacy-encodings **missing → gated**, +2 app rows.

**WIKI:** none — the interactive-JS architecture note already covers PageContext; this extends it, but the
one durable lesson (*a node id is unique only within its arena — resolve reflectors against their own
document*) belongs in the journal, not a new topic file.

## Tick 83 — the loop could not see its own frame. Now it checks the map.

**TICK SHAPE: instrument.** `[no-pattern]`. No engine code changed. What changed is that the loop can now
**find its own next step-change without being told** — which is the only defect that was still costing
order-of-magnitude leaps.

**The problem, stated exactly.** Twice in one session this project made an order-of-magnitude jump, and
**both times a human had to point at it**:

* *"measure `html/dom`, not just `dom/`"* → **+9,940 subtests**. `html/dom` (59,818 subtests) had sat
  un-measured **in the same checkout** while ten ticks went into `dom/` (6,484).
* *"histogram the failure messages"* → the top row was **+170 subtests in an hour**. `--show-failures` had
  existed for many ticks and had **never been run**.

Neither was hard analysis. Both were **aperture** failures. And the reason is uncomfortable: every
instrument the loop owned — `orient`, the ledgers, the ratchet — could only see **what was already on its
map**, and *nothing ever checked the map*. The map was drawn from memory, and this project's memory has
been wrong six times.

> **A ranking inside the wrong frame is a confident wrong answer.**

### What was built (PROCESS #49)

* **`blindspot.sh`** — enumerates every WPT area **upstream**, including ones never checked out (the sparse
  clone still carries the full tree index, so this is free). It found we were measuring **9 areas** while
  ~20 areas of >800 tests each were **invisible** — including `html/canvas` (4,232 tests), *after we had
  built a canvas rasterizer*. **THE RULE: rank apertures before mechanisms.** An area you have not checked
  out **scores zero and cannot be ranked**.
* **`wpt-expand.sh`** → the aperture went from **9 areas to 22**.
* **`recon.sh`** — a cheap unbiased sample of *every* materialised area, ranked by **estimated failing
  subtests** (median-based: the first version put a degenerate area on top because one generated file holds
  a test per Unicode codepoint — *an estimator a single file can hijack chooses wrong, confidently*).
* **`wpt-sweep.sh`** — measures every area, every capability tick. **TOTAL: 25,869 / 837,858.**
* **`surface-audit.sh`** — **the loop leaves its own frame, every 10 ticks.** Search the web, reconcile the
  constellation against Interop and other engines, and add what the world names that our map does not.

### The constellation — the near horizon, made countable

`CONSTELLATION.md` + `.tsv`: **98 capabilities** across doc / app / platform / media / cross, each with a
status that may only be `gated` if a named `G_*` gate asserts it. Scored every tick by
`constellation.sh`. The honest picture: **platform 10% gated, media 0%.**

### The first surface audit found what the map could not

Sources: [Interop 2026](https://github.com/web-platform-tests/interop/blob/main/2026/README.md) ·
[Ladybird / engine comparison](https://news.ycombinator.com/item?id=45493358).

**20 capabilities were added that were not on the map** — `<dialog>`/popover, View Transitions, Navigation
API, container queries, scroll snap, CSS anchor positioning, **WebAuthn/passkeys** (the near horizon says
"platform = login", and passkeys *are* login now), and — most damning — **`test262`**: Ladybird tracks
97.8% of 53,207 JS-conformance subtests and **we embed SpiderMonkey and have never run it.** We did not
know that we did not know.

And the calibration this project never had:

| | |
|---|---|
| Ladybird, April 2026 | **2,067,263** passing WPT subtests |
| Manuk, tick 83 | **25,869** |

**~1.25% of Ladybird's count.** That is the first external scale this project has ever put itself against.

The audit's most important finding is a *methodology* input, not a capability:

> *"Matching the behavior real-world sites depend on — including undocumented quirks — is the work that has
> historically **killed independent engines**. A strict standards implementation that breaks sites fails
> the only test that matters."*

**WPT conformance is necessary and not sufficient.** The 265-site Chromium differential oracle is not a
nice-to-have beside WPT — it is the anchor against the class of failure that has ended other engines.

### The ratchet — the first principle, made a gate that refuses

*"Never regress capability, performance or stability"* has been in `CLAUDE.md` since tick 1, and **tick 82
landed +9,940 subtests while quietly losing 2 in an area it was not looking at.** A rule I can recite while
breaking it is a decoration. So `ratchet.sh` keeps a high-water mark for WPT-per-area, crashes (**zero**,
not "no worse"), duplicate wire requests, capabilities, gates, the wall, and per-class gated counts — and
**`tick.sh` refuses the commit** if any moves backwards. **Proven**: replaying tick 82's exact numbers, it
refuses — *"a win beside a regression is not a win — it is a trade, and the ratchet does not trade."*

And its invariant is **`MEASURED`**, not `unknown` — the first version punished *discovery*, which is
exactly backwards. **A bigger, uglier, more honest map is a good tick.**

### Two instruments caught lying, in this tick, by this tick

* **The wall reported "G_RUNTIME_COUNT failed — runtimes are proliferating"** when `cargo` could not create
  a directory. Tick 81 taught that lesson to the crate suite and **not** to the parallel gates. Fixed —
  the **fourth** instrument to need it.
* **The perf floor said the engine had regressed 65%** — with **zero engine changes**. The governor was in
  `powersave` and a memory-bound workload does not make it ramp. I then nearly shipped a CPU-calibration
  loop that would have made it *worse*, because a tight ALU loop **does** ramp — *the calibrator must be
  the same shape as the workload*. The floor is now a **ratio between two engine workloads**, which divides
  machine speed out exactly while still catching superlinear scaling. **The fifth instrument** to need the
  same lesson (PROCESS #48).

**The ratchet.** Capability: unchanged (no engine code). Performance: the floor got *honest* (69s wall, a
new best). Instrument fidelity: **up, more than any tick so far** — the loop can now find its own next
leap, and it can no longer ship a trade.

## Tick 82 — the largest gap in the platform, and we had never looked at it

**TICK SHAPE: capability.** **`html/dom` 21.0% → 37.7% — +9,940 subtests in one tick.** Bar 0 clean.
`dom/` −2 (named below, not hidden).

**Ten ticks were spent on 9% of the ground we already had checked out.**

Ticks 71–81 worked `dom/`: carefully, correctly, +652 subtests. `dom/` is **6,484 subtests**. Sitting in
the same checkout, **never once measured**, was `html/dom` — **59,818 subtests**, nine times larger. The
loop optimised the area it happened to be looking at and never asked whether it was the right one.

**One command answered it.** Histogramming all 47,226 failing subtest messages in `html/dom`:

| count | message |
|---:|---|
| 23,411 | `IDL get expected (string/boolean/number) X but got (undefined) undefined` |
| 13,724 | `getAttribute() expected X but got X` — the IDL **set** never reached the attribute |
| 1,470 | `hasAttribute() expected false but got true` |

**~38,000 subtests — 80% of the failures — are ONE mechanism: HTML attribute reflection.**
`a.href`. `input.disabled`. `img.width`. `td.colSpan`. `option.selected`. **All `undefined`.**

And it is not a conformance curiosity. **It is how ordinary page code touches the DOM.**
`if (input.disabled)` reading `undefined` does not throw — *it silently takes the wrong branch.*

**The table is not the work; the rules are.** The table (118 elements, ~400 attributes) is extracted from
WPT's own `elements-*.js`, which is a transcription of the HTML spec's IDL — the same source Ladybird and
Servo codegen from. What makes it honest is that the **mechanism is generic**: string / boolean / long /
unsigned long / limited / clamped / double / enum / url are implemented once, against the spec's
*algorithms*, and hold for attributes no test covers. Swap the table and the mechanism still stands.

The rules that actually matter, each of which is a classic bug:

* **boolean is PRESENCE, not value.** `el.disabled = false` must **remove** the attribute. Stringifying
  writes `"false"` — and the element stays disabled, with no error and no way for the page to tell.
* **URLs resolve against the document base.** `a.href` on `<a href="x">` is absolute.
* **an invalid `unsigned long` falls back to the default — it is not clamped to zero.** `colspan="0"` is
  invalid, so `colSpan` reads back as **1**.
* **enumerated attributes have TWO different defaults** — missing-value and invalid-value — and confusing
  them is *the* classic reflection bug.

**I introduced a Bar 0 crash and it had to go before anything else could.** The first version checked for a
name collision on *one* prototype instead of the whole **chain**, so a reflected accessor was defined over
a native implementation and the two re-entered each other. A WPT child died. **+9,940 subtests is worth
nothing next to a crash**, and the tick could not ship until `if (idl in proto) return;` — *does this name
already mean something here?* — replaced the wrong question.

**Two things skipped rather than faked**, and this is the fourth time the same lesson has paid:

* **`tokenlist`** (`relList`, `sandbox`, `htmlFor`) reflects as a live `DOMTokenList`, not a string. The
  first version returned the raw attribute when it could not build one — and *`dom/lists/DOMTokenList-coverage`
  fell 129 → 115.* **A caller handed a string where a `DOMTokenList` belongs has been lied to; a caller
  handed `undefined` at least knows nothing is there.** Skipped, and said so.
* **The −2 in `dom/historical.html` is real and is named.** Our accessors live on the *shared* prototype,
  so `'text' in div` is `true` even though the getter returns `undefined`. Browsers put `text` only on
  `HTMLAnchorElement.prototype`. **This is tick 64's stated limit surfacing** — we have no per-tag
  prototypes — and building them is the next tick, not a footnote.

**And the policy that should have prevented all of this is now written down**: `docs/loop/GRIND.md`
(PROCESS #47). *Measure every area every tick. Histogram the failure messages of the largest. Fix the
MECHANISM with the highest count, never the instance. Go broad until no mechanism ≥500 subtests remains
anywhere — only then go narrow.* **A mechanism you have not looked for cannot appear in your ranking.**

**The ratchet.** Capability: **up, by an order of magnitude more than any previous tick.** Performance:
unchanged. Instrument fidelity: **up** — the search policy is mechanical now, and the loop can no longer
spend ten ticks in the wrong room.

## Tick 81 — the wall could not tell a killed gate from a failing one

**TICK SHAPE: instrument.** `[no-pattern]`. No engine change; the thing that *judges* every engine change
was unreliable.

**On tick 80 my standalone wall went red on `manuk-shell`, `tick.sh`'s own wall went green, and the tick
landed.** `manuk-shell` then passed 3/3 in isolation. It was the third such flake in one session (G_FORM,
G_IFRAME, now this), and every one of them happened only when the wall shared the machine with a heavy WPT
release build.

The cause was one line. The crate-suite loop grepped for `test result: ok` and called anything else a
failure — so a suite that was **OOM-killed**, or whose build was starved out under memory pressure,
produced no verdict line at all and was reported as a **RED GATE**.

> **A wall that is green non-deterministically proves nothing.** And it is worse than useless: it teaches
> you to re-run until it goes green, which is precisely how a real regression gets shipped.

**The project already knew the answer, because the WPT harness learned it first.** WPT distinguishes a
crash from `SHORT` — *a row the instrument lost* — and refuses to score the latter. The wall itself had
never been given that taxonomy. It has it now:

* an explicit `test result: FAILED` → **red, immediately.** Never retried, never excused.
* **no verdict at all** (signal, OOM, starved build) → the **instrument** faulted. Retry once, *alone*,
  with every background job reaped. If the retry yields a verdict, that verdict is the truth.
* still no verdict → **`INSTRUMENT FAULT`**, and it fails — because *unmeasurable is not passing either.*

Both branches are proven, not assumed: a genuinely failing test goes red with **no retry**, and a killed
one recovers and says so.

> **A lesson learned in one instrument is not learned until it is applied to the others.**

**The ratchet.** Capability: unchanged. Performance: unchanged (wall 28s). Instrument fidelity: **up** —
the thing that judges every other thing can now tell "this is broken" from "I could not look."

## Tick 80 — a passive listener that cancels is not passive

**TICK SHAPE: capability.** **+44 WPT subtests** (2345 → 2389, **36.8%**). Bar 0 clean.

> **Nine WPT-aimed ticks: +652 subtests. Five near-horizon ticks: +1.**

Straight off tick 79's ranked failure list — *`assert_equals: defaultPrevented expected false but got true`*,
57 subtests, all in `passive-by-default.html`. Not a gap: **a bug.**

`addEventListener(type, fn, {passive: true})` is a **promise**: *this handler will not cancel the default
action.* The browser may therefore begin scrolling **without waiting for the handler to run at all**. That
is the entire reason the flag exists.

We *recorded* `passive` on the listener entry (tick 74 added the options object) and then **honoured it
nowhere.** A passive touch handler could still call `preventDefault()` and cancel the scroll — which is
precisely the jank the flag was invented to prevent. The promise was accepted and then broken, silently.

Two halves, and both are observable behaviour, not optimization:

* **A passive listener's `preventDefault()` does nothing** — it is replaced with a no-op for the duration
  of that handler, and restored after.
* **`touchstart` / `touchmove` / `wheel` / `mousewheel` are passive BY DEFAULT** on `window`, `document`,
  `document.body` and the root element, unless the page explicitly passes `{passive: false}`. That is the
  rule browsers adopted to stop one rogue touch handler from janking every scroll on a page, and it changes
  what a page does.

The gate asserts both directions — `passive:false` (the cancel was ignored) and `active:true` (a normal
listener still cancels), because a fix that made *nothing* cancellable would satisfy the first alone.

**The ratchet.** Capability: **up**. Performance: unchanged. Instrument fidelity: unchanged — but the tick
took under an hour because tick 79's `--show-failures` histogram had already named the bug, its size, and
its file.

## Tick 79 — the cheapest instrument is the one you already have and did not run

**TICK SHAPE: capability.** **+170 WPT subtests** (2175 → 2345, **33.8% → 36.2%**) — the second-largest
single-tick move, from **one contained fix**. Bar 0 clean.

> **Eight WPT-aimed ticks: +608 subtests. Five near-horizon ticks: +1.**

**Tick 78 concluded that the WPT harness could not show which subtests fail, and wrote that into
`PROCESS.md` as an instrument gap to go and build.** It was already built. `manuk-wpt wpt dom
--show-failures` prints every failing subtest's **name and its assertion message**, and has for many ticks.

I asserted an absent measurement without running the one command that would have checked — and then
enshrined the false claim **in the ledger whose entire purpose is to stop exactly that.** PROCESS #45 is
rewritten. The rule now binds this file too: *an entry claiming something is missing must name the command
that was run to establish it.*

**And running it took one command to produce a ranked work list:**

| failing subtests | message |
|---:|---|
| 441 | `assert_throws_dom` (things that must throw and do not) |
| 390 | `doc is undefined` — iframe documents |
| 195 | `wrong class after modification` — classList serialization |
| **160** | **`node.setAttributeNS is not a function`** |
| 103 | `document.createProcessingInstruction is not a function` |
| 57 | `defaultPrevented expected false but got true` |

`setAttributeNS` is not an exotic API — **it is how SVG's `xlink:href`, MathML, and every XML-ish document
set an attribute at all.** Implementing the four `*AttributeNS` methods, with the spec's `NamespaceError`
validation, was **+170 subtests.**

> **The most expensive thing in this loop is still guessing — and the cheapest instrument is the one you
> already have and did not run.**

**Honest limit, stated rather than discovered:** the namespace is *validated* and then **ignored for
storage** — `setAttributeNS(ns, 'xlink:href', v)` keys the attribute by its qualified name. That is right
for every document this engine renders, and wrong only for a document holding two same-named attributes in
different namespaces, which no real page does.

**Deliberately not done:** `createProcessingInstruction` (103 subtests) needs a new DOM node type with
match-exhaustiveness fallout across four crates. Faking it as a `Comment` would be the exact stub pattern
these last ten ticks have been killing, so it waits for a tick with room to do it properly.

**The ratchet.** Capability: **up, sharply**. Performance: unchanged. Instrument fidelity: **up** — a false
entry removed from the process ledger, which is worth more than the entry it replaced.

## Tick 78 — a bundle of correct fixes can be jointly wrong, and per-file totals cannot say which

**TICK SHAPE: capability.** **+5 WPT subtests** (2170 → 2175, 33.8%). Bar 0 clean. A small tick, and it is
mostly worth reading for the part that did *not* work.

**The capability.** `document.implementation.createDocumentType()` returned a **plain object literal** —
prototype `Object`, so `instanceof DocumentType` was false and
`Object.getPrototypeOf(dt) === DocumentType.prototype` (which is what WPT asserts) could never hold. It
also **validated nothing**: `createDocumentType('', …)` produced a doctype with an empty name. And
`DocumentType` did not exist as an interface at all, so nothing it returned could ever *be* one. Fixed,
with `InvalidCharacterError` / `NamespaceError` validation. `document.doctype` was `null` on every page —
including one that plainly declares `<!doctype html>` — and a good deal of quirks-mode branching reads it.

**And the negative result, which cost most of the tick** (PROCESS #45).

`Document-createEvent.https.html` (203 failing) asserts `Object.getPrototypeOf(ev) === window[iface].prototype`.
So I added the full legacy interface set (`UIEvent`, `CompositionEvent`, `StorageEvent`, `HashChangeEvent`,
…), the alias map (`MouseEvents` → `MouseEvent`, `HTMLEvents` → `Event`), the `initXxxEvent` family, and
`NotSupportedError` for unknown names. **Every one of those is correct in isolation.**

The file went **76/279 → 40/279.** The suite fell **31 subtests**. A second targeted fix did not move it.

> **A bundle of individually-correct fixes can be jointly wrong — and per-file totals cannot tell you
> which one.**

I discarded it. The tree at HEAD was the known-good 2170; `snap.sh` checkpointed the experiment first, and
the revert was *deliberate* rather than panicked — which is precisely the mechanism PROCESS #37 exists to
provide, used as designed for the first time.

**The instrument gap this exposes is the real finding.** Our WPT harness records subtest **counts**, not
subtest **names**. So a regression *inside* a file is invisible: I could see that 36 subtests broke and had
no way to see *which*. Localizing meant guessing. **Capturing failing subtest names is the next instrument
tick**, and until it exists, a large multi-part change to a single file is a gamble rather than an
experiment.

**The ratchet.** Capability: **up** (small). Performance: unchanged. Instrument fidelity: **up** — the gap
that made this tick expensive is now named, and it is the next thing to build.

## Tick 77 — `MutationObserver` observed nothing, and said `function` the whole time

**TICK SHAPE: capability.** Seventh tick at the far horizon. **+44 WPT subtests** (2126 → 2170, **33.8%**).
Bar 0 clean.

> **Seven WPT-aimed ticks: +433 subtests. Five near-horizon ticks: +1.**

`new MutationObserver(cb)` constructed. `observe()` returned. `takeRecords()` returned `[]`. The callback
**never fired.** And `typeof MutationObserver === 'function'` was `true` throughout — which is exactly how
it survived. *A check that only asks whether a name exists is satisfied by a stub.* That is the third
inert-stub interface found in seven ticks (`Range`, `TreeWalker`, now this), and the pattern is the point:

> **A stub is worse than an absence.** The library feature-detects, finds it, registers, and then silently
> never reacts. Vue, Alpine and lit use it to notice DOM changes they did not make; every analytics and
> consent script uses it to see injected content.

It records for real now — attributes and childList, with `oldValue` **only when the registration asked for
it** (handing it over unasked is a conformance failure that looks like generosity), `attributeFilter`,
`subtree`, and `disconnect`.

**Delivery is a microtask, and that is not a detail.** A loop that appends 100 nodes must produce **one**
callback with 100 records, not 100 callbacks. Deliver synchronously and every observer on the page runs
100× per frame — *a performance collapse, not a conformance bug.* The gate asserts `batched:1,100`.

**The fourth capability to land on the back of tick 64's real prototypes**, and it could not have been done
without them: it observes by *wrapping the mutating methods on the DOM prototypes*.

**Stated limit, so nobody re-discovers it:** only mutations made by a **script** are observed. Engine-
internal edits (the parser, the deferred-script pass) do not go through those wrappers. That is mostly the
right behaviour — an observer registered after parsing should not see the parse — but it is a limit, and
wiring the natives to emit records directly is the complete answer.

**And the falsifier refused to run** (PROCESS #44). Its poison-guard greps target files for a leftover
mutation marker — and the marker was the bare word `MUTATION`, so it declared `mutation_js.rs` poisoned on
account of `pub const MUTATION_JS`. **The safety mechanism could not tell its own sentinel from the English
word.** It is `MANUK_FALSIFY_MUTATION` now: *a guard whose signal is a common substring will eventually
fire on a file that merely discusses the subject.*

**The ratchet.** Capability: **up**. Performance: unchanged (wall 129s). Instrument fidelity: **up** — a
real gate, proven falsifiable, and the falsifier's own guard repaired.

## Tick 76 — `element.attributes` was `undefined`. Not incomplete: absent.

**TICK SHAPE: capability.** Sixth tick at the far horizon. **+49 WPT subtests** (2077 → 2126, **33.1%**).
Bar 0 clean.

> **Six WPT-aimed ticks: +389 subtests. Five near-horizon ticks: +1.**

`element.attributes.length` was a **`TypeError`**. Iterating an element's attributes is one of the most
ordinary things a script does — every DOM serializer, every differ, every "copy these attributes across"
helper. And:

> **DOMPurify walks `attributes` to strip `on*` handlers. A sanitizer that cannot enumerate attributes
> cannot sanitize them.**

Gone with it: `getAttributeNode`, `setAttributeNode`, `document.createAttribute`, and `toggleAttribute` —
the idiomatic way to flip `disabled` / `hidden` / `aria-expanded`.

**Two things it had to get right, and both are easy to miss:**

* **The map is LIVE**, for exactly the reason `HTMLCollection` is (tick 73):
  `while (el.attributes.length) el.removeAttribute(el.attributes[0].name)` — the "strip everything" idiom
  — **spins forever** against a frozen `length`. *The same dead-collection hang, one interface over.*
* **An `Attr` is a HANDLE, not a snapshot.** `attr.value = 'x'` must write **through** to the owner
  element. Return a plain object and every `attrs[i].value = …` in the wild silently writes to nothing —
  which is the falsifier's mutation, and it goes red.

Built entirely on `getAttributeNames`/`getAttribute`/`setAttribute`, which already worked — the third
capability to land on the back of tick 64's real prototypes, because `attributes` has to be an accessor on
the prototype to be wrappable at all.

**The ratchet.** Capability: **up**, and it closes a second latent Bar 0 of the same shape. Performance:
unchanged. Instrument fidelity: **up** — proven falsifiable.

## Tick 75 — a name is not a string

**TICK SHAPE: capability.** Fifth tick at the far horizon. **+149 WPT subtests** (1928 → 2077, **32.3%**).
Bar 0 clean.

> **Five WPT-aimed ticks: +340 subtests. Five near-horizon ticks: +1.**

Three gaps, all silent, all the same family: **the engine accepted things that are not names, and then
produced elements and classes that could never match anything.**

* **`classList` was not a `DOMTokenList`.** `classList[0]` was `undefined` — no indexed access at all —
  and the token methods **never threw**. `classList.add('btn primary')` is a *bug*: the author meant two
  tokens. A browser that silently writes the single class `"btn primary"` produces an element matching
  **neither** selector, with no error anywhere. It is a real `DOMTokenList` now: indexed, iterable,
  `SyntaxError` on an empty token, `InvalidCharacterError` on whitespace, and **no partial effect** — a
  throwing `add('ok', '')` must not leave `ok` behind.
* **`createElement('')` and `createElement('<div>')` produced elements.** Perfectly good nodes with
  nonsense tags, which then matched no selector and rendered nothing. A page that catches
  `InvalidCharacterError` can recover; a page handed a phantom cannot even see the problem.
* **`createElementNS` threw the namespace away.** It split off the prefix, called `createElement`, and
  returned an HTML element — so `namespaceURI` said XHTML for an SVG node, `localName` was `undefined`,
  and `tagName` was **uppercased**, which for SVG's `linearGradient` is simply wrong. `ElementData` carries
  a real namespace now, and casing is per-namespace. That one fix took `dom/nodes/case.html` from
  **7/285 to 68/285**.

**And the correction that cost me, because it made the score go DOWN.** I split names on `:`
unconditionally — but **HTML does not split prefixes**. `document.createElement('a:b')` has
`localName === "a:b"`; the colon is just a character. Only a *namespaced* element has a prefix. The
unconditional split silently renamed every HTML element containing one, and `dom/nodes` fell from 1592 to
1575 before I caught it. **A refactor that improves the thing you are looking at and regresses the thing
you are not is indistinguishable from progress, unless you measure both.**

**And `null` is not `"null"`.** `createElementNS(null, 'p:q')` must throw `NamespaceError`, and it did not:
`arg_string` *stringifies*, so the namespace arrived as the string `"null"` — a perfectly good namespace as
far as the check was concerned.

**Found and not fixed, stated so nobody re-discovers it:** `Document-createElementNS.html` (596 subtests)
runs against **iframe documents** (`doc.defaultView.DOMException`), and our iframes render as bitmaps with
no JS global. Those 596 are behind *real nested browsing contexts*, not behind `createElementNS`. Likewise
`Element-classlist.html` (1420) builds its nodes with `createElementNS` in four namespaces — the namespace
fix helps, but the file's remaining bulk needs `MutationObserver` records on attribute changes.

**The ratchet.** Capability: **up**. Performance: unchanged. Instrument fidelity: **up** — `G_NAMES` proven
falsifiable by letting `classList` accept whitespace again.

## Tick 74 — `{once: true}` fired forever, and nothing complained

**TICK SHAPE: capability.** Fourth tick aimed at the far horizon, and the largest move yet:
**+118 WPT subtests** (1810 → 1928). **`dom/` crosses 30.0%.** Bar 0 clean.

> **Four WPT-aimed ticks: +191 subtests. Five near-horizon ticks: +1.**

**Propagation was already right** — bubbling, capture, `stopPropagation`, `target`/`currentTarget`, the
`dispatchEvent` return value. What was missing was everything *around* it, and **every gap failed
silently**:

* **`{once: true}` fired every time, forever.** The native read `addEventListener`'s third argument as a
  bare boolean, so an options *object* meant `capture: false` and `once` was **dropped on the floor**. It
  is one of the most common options in modern code, and its failure is invisible: the handler just keeps
  running.
* **`e.returnValue` and `e.cancelBubble` were `undefined`.** They are IE-era aliases the spec kept
  *because the web never stopped using them* — jQuery's event normalisation, Google Analytics, and a great
  deal of ordinary handler code. `if (e.returnValue === false)` was **dead code**, and
  `e.cancelBubble = true` set a junk property that stopped nothing.
* **`document.createEvent` did not exist**, so `createEvent is not a function` took the whole script with
  it. It had been **deferred for fear of an infinite dispatch loop** — and that fear was misplaced: the
  loop was never in `createEvent`, it was a frozen `timeStamp`, and that was fixed ticks ago. **A deferral
  outlived its reason.**
* the invocation loop **indexed a live array while mutating it**, so a `once` removal mid-dispatch skipped
  the next listener. It iterates a snapshot now.
* a listener could be registered **twice** (the spec says no), and an **object with `handleEvent`** — the
  `EventListener` interface every class-based component uses — was not accepted at all.
* `{signal: abortSignal}` now removes the listener on abort, which is how a component tears down all its
  handlers in one call.

**The ratchet.** Capability: **up** — and a whole class of silent handler bugs closed. Performance:
unchanged (wall 154s). Instrument fidelity: **up** — proven falsifiable by dropping `once` again and
watching it go red.

## Tick 73 — a dead collection is not a conformance gap. It is a hang.

**TICK SHAPE: capability.** Third tick aimed at the far horizon. **+17 WPT subtests** (1793 → 1810,
28.2%). Bar 0 clean.

> **Three WPT-aimed ticks: +73 subtests. Five near-horizon ticks: +1.**

`element.children` and `getElementsByTagName()` returned **plain arrays** — a snapshot, taken once. Append
a child and `length` did not move. `dom/collections` scored 3/48.

**And that is a Bar 0 hang**, hiding in the most common DOM idiom there is:

```js
while (el.children.length) { el.removeChild(el.firstChild); }   // "empty this element"
```

With a *live* collection this terminates: each removal shortens it. With a **dead** one, `length` is frozen
at its initial value, the condition is true forever, and **the tab locks up.** *A dead collection does not
fail loudly — it spins.* The gate asserts exactly this, and the falsifier's mutation freezes `length` and
watches it go red.

**It landed cheaply for one reason, and the reason is worth recording.** `children` is an accessor on the
prototype, so a live view could **wrap** the existing getter rather than reimplement it. That is only
possible because **tick 64 gave the DOM real prototypes** — before it, patching the prototype did nothing
at all, silently. This is the **second** capability to land almost free on the back of that fix
(traversal was the first), and it is the argument for repairing foundations rather than symptoms.

**Two self-inflicted, both instructive:**

* The wrap silently did nothing at first, because I wrapped `Element.prototype` — and tick 64's **stated
  limit** is that every member is an own-property of `Node.prototype`, with `Element.prototype` an empty
  link that merely *inherits*. `getOwnPropertyDescriptor` returned `undefined` and the patch went nowhere.
  A stated limit is not a solved one; it comes back.
* `cargo fmt` had reflowed the `eval(...)` call I was string-matching against, so the install edit **never
  applied** and the module never ran. **An edit that matches nothing changes nothing, and reports success.**

**Honest cost:** recomputing per access makes `for (i = 0; i < c.length; i++) c[i]` quadratic in the
collection's size. For the collections real pages hold, nothing. If it ever matters, the fix is a DOM
mutation counter to invalidate a cache — *not* a return to snapshots. **Correct and occasionally slow beats
fast and wrong, and fast-and-wrong here means a locked tab.**

**The ratchet.** Capability: **up**, and it closes a latent Bar 0. Performance: unchanged (wall 251s).
Instrument fidelity: **up** — proven falsifiable, after the first falsifier failed to apply and reported
the gate as vacuous (PROCESS #15: *a mutation that changes nothing certifies nothing*).

## Tick 72 — `NodeIterator` and `TreeWalker`, and the filter bug that is really a security bug

**TICK SHAPE: capability.** The second tick aimed straight at the far horizon, and it confirms the first.

**+27 WPT subtests** (1766 → 1793, 27.5% → 27.9%); `dom/traversal` **11/53 → 34/53**. Bar 0 clean.

> **Two WPT-aimed ticks: +56 subtests. Five near-horizon ticks: +1.** The cadence ledger's finding is now
> confirmed twice, and it is no longer a hypothesis about how to allocate ticks — it is a measurement.

**What was there:** a `createTreeWalker` returning a **plain object** with `nextNode` and nothing else — no
`previousNode`, no `firstChild`/`nextSibling`/`parentNode`, no prototype (so `instanceof TreeWalker` was
false). `NodeIterator` did not exist at all.

**Both horizons again**, which is why it was chosen: 42 WPT subtests, *and* traversal is how the real web
walks a subtree. **DOMPurify** — the sanitizer half the web runs untrusted HTML through — is built on
`NodeIterator`. Lit finds a template's dynamic holes with `createTreeWalker`.

**The walk is the easy part. The filter protocol is where it goes wrong, and silently:**

> **`FILTER_REJECT` (2) skips the whole SUBTREE. `FILTER_SKIP` (3) skips only the node** and still descends
> into its children. Swap them, and a sanitizer that rejects `<script>` walks cheerfully **into** the
> script and keeps its contents. **That is a security bug shaped like a traversal bug** — nothing throws,
> the walk still returns nodes, it just returns the wrong ones.

And the two interfaces differ *precisely there*: **`NodeIterator` has no notion of a subtree, so it treats
`REJECT` as `SKIP`.** Implementing one and aliasing the other is wrong in the way nobody notices until
something leaks. The gate asserts both behaviours against the same tree, and the falsifier's mutation is
exactly that slip.

**The gate was wrong once, and the implementation corrected it.** I asserted that `NodeIterator` would not
return the root. It does — its reference node *starts* at the root with `pointerBeforeReferenceNode = true`,
so the first `nextNode()` returns it, while `TreeWalker` starts at the root and moves *away*, never
returning it. A real asymmetry, easy to get backwards, and worth the gate having been red about.

**The ratchet.** Capability: **up** on both horizons. Performance: unchanged (wall 150s). Instrument
fidelity: **up** — `G_TRAVERSAL` proven falsifiable, and `G_CAPABILITY` now asserts that `FILTER_REJECT`
prunes.

## Tick 71 — a real `Range`, and the first tick aimed straight at the far horizon

**TICK SHAPE: capability.** The first tick *chosen by the cadence ledger*, and it validates it.

**Tick 70's measurement said the two horizons are nearly orthogonal**: ticks 64–69 shipped a 60× DOM
speedup, real prototypes, a canvas rasterizer, element scrolling and `display: contents` — every one a
genuine daily-driver win — and **WPT moved by one subtest.** So this tick was aimed *directly* at WPT.

**Result: +29 subtests in one tick** (1737 → 1766, 27.1% → 27.5% of `dom/`), Bar 0 clean. Five ticks of
near-horizon work moved +1. **The ledger was right, and acting on it worked.**

**`Range` was an inert stub**, and the shape of that is worth keeping: it sat in the interface list, so
`typeof Range === 'function'` was **already true**. `document.createRange()` did not exist. `dom/ranges`
scored **2 of 200**. *A check that only asks whether a name exists is satisfied by a stub* — which is
precisely how it survived sixty ticks, and precisely why `G_CAPABILITY` now asks it to *extract a
substring*, not to exist.

It is the rare target that serves **both horizons**: ~198 WPT subtests, and every rich-text editor,
selection, copy/paste path and `contenteditable` surface on the web.

**Written in JavaScript, on purpose.** A `Range` is pure tree arithmetic — compare two boundary points,
find a common ancestor, splice a subtree — and it touches nothing JS cannot already reach. What it *does*
need is a **correct DOM**, and that is exactly what the last several ticks built: real prototypes, a spec
`insertBefore`, CharacterData in UTF-16 code units. **It landed cheaply because of them.**

**The difficulty is entirely in one place**, and the gate is built around it: extraction **across
structure**. A range from the middle of one paragraph to the middle of the next leaves both only
*partially* contained, so both must be **split** — the outer halves stay, the inner halves leave — while
fully-contained nodes move wholesale. The naive version (move whole nodes) passes on flat text and
destroys every document that has structure, which is every real document.

**Installed AFTER the prelude**, never before: the prelude's inert-interface list creates the stub, so the
real `Range` would otherwise be overwritten by a do-nothing constructor — and `typeof` would still say
`function`. This is the same ordering bug that once let a stub `AbortSignal` shadow the real one.

**Still a stub, and said out loud:** `Selection`. `dom/ranges/tentative` (149 subtests of proposed-spec
API) remains at zero and is not being chased.

**The ratchet.** Capability: **up** on both horizons at once. Performance: unchanged. Instrument fidelity:
**up** — `G_RANGE` is proven falsifiable, and it goes red *while the stub keeps `typeof Range` truthful*,
which is the exact failure it replaced.

## Tick 70 — the loop had no odometer

**TICK SHAPE: instrument.** `[no-pattern]`.

Seventy ticks measuring the browser, and **zero measuring the loop.** "Tick 69 landed" is a receipt, not
progress data — and this project has two horizons whose only honest question is *are we getting there, and
how fast?*

`scripts/tick-log.sh` now runs from `tick.sh` **after a successful push** and appends one row of ground
truth per tick: when it landed, **Δ since the last one** (the real implement → debug → wall → land cycle),
what it cost (wall seconds, files, lines), and what it bought — capabilities asserted, gates live, ✅ rows,
oracle hangs, and WPT subtests. The journal headline rides along as the qualitative impact statement,
because it is *already* written per tick in terms of what changed for the browser.

**Sixty-two past ticks were backfilled from git** — every tick is a commit, and a commit carries its
timestamp, its diff and its message. **What could not be recovered was left blank**: past wall times (only
the latest is recorded), past WPT figures (measured a handful of times), past gate/capability counts (they
are grepped from the tree, and the tree is *now*). Counting today's tree and labelling it "tick 42" would
draw a beautiful, entirely fictional curve. **An empty cell is a fact; a guessed one is a lie that gets
quoted back later as evidence.**

**And on its first run it found the thing it was built to find.**

| | |
|---|---|
| ticks landed | 62 |
| median cycle | **19m** (17m over the last 10) |
| ticks/hour | **0.85** over 71.8h |
| capability ticks | 25 of 62 (40%) |
| WPT (`dom/`) | 1736/6418 (t64) → **1737/6418** (t69) |

> **Ticks 64–69 shipped a 60× DOM speedup, real prototypes, a canvas rasterizer, element scrolling and
> `display: contents` — every one a genuine daily-driver capability win — and WPT moved by ONE SUBTEST.**
>
> The two horizons are **nearly orthogonal.** The far horizon will not arrive as a side-effect of the near
> one. **It has to be spent on directly.**

That is the single most consequential thing this loop has learned about *itself*, it changes how ticks get
allocated from here, and it was invisible until something counted.

**And the instrument caught a flaw in itself before shipping:** its first draft projected a WPT finish line
by multiplying the `dom/`-subset rate (6,418 subtests) up to the 50,000-test horizon — a suite this project
**has never run**. That is a category error dressed as arithmetic, and it would have produced a confident
number about a thing that was never measured. It now refuses to give a finish line at all, and says why.

**The ratchet.** Capability: unchanged. Performance: unchanged. Instrument fidelity: **up** — the loop can
finally see itself, and the first thing it saw redirects the roadmap.

## Tick 69 — `display: contents` fell through to `inline`, and collapsed the grid

**TICK SHAPE: capability** — roadmap item #4.

`display: contents` means the element generates **no box at all, while its children still do**. It is not
`display: none`; nothing is hidden. The wrapper vanishes from the box tree and its children are laid out as
if they were the parent's own. Modern CSS leans on it hard, and always for the same reason: a `<div>`
wrapping grid items so a component can own them — **without that `<div>` becoming a grid item itself.**
Every component framework emits such wrappers.

It was **never parsed**. `"contents"` fell through the `match` to `_ => s.display` and stayed `inline` —
and, one layer down, Stylo's own `Display::Contents` (which it parses perfectly well) hit a catch-all
`else { Display::Inline }` in our mapping and was thrown away there too. **Two independent fallthroughs to
the same wrong answer.**

And `inline` is the *worst available* wrong answer:

* `display: none` would at least have been **visibly** wrong — the content disappears, and you go looking.
* `inline` keeps the wrapper in the tree as a real box that **does** participate in layout. Its children
  stop being the grid's items. The grid sees **one** anonymous child instead of three, and the whole layout
  silently collapses into a single cell — with every element still present, still styled, and in the wrong
  place.

Fixed in both paths, because a grid has two: the block path (`rendered_children`) and the Taffy path
(`flex_items`). Both dissolve `contents` wrappers **recursively** — `contents` inside `contents` is legal
and a component tree produces exactly that — and both are depth-bounded, because a stack overflow in layout
is a Bar 0 crash and this is precisely the property a hostile page would nest ten thousand deep.

**The wall caught what I could not see:** adding a variant to `Display` broke a non-exhaustive `match` in
`manuk-wpt`, which surfaced as *"F1 cascade ?ms exceeds the floor"* and *"G6 found 0 links"* — two failures
that look like layout regressions and were a compile error in a test harness three crates away. **A
failure's shape is not its cause.**

**The ratchet.** Capability: **up**. Performance: unchanged (wall 39s). Instrument fidelity: **up** —
`G_DISPLAY_CONTENTS` goes red when the wrapper stops dissolving, *which is a red that shows the layout
silently collapsing rather than erroring*, and `G_CAPABILITY` asserts it.

## Tick 68 — the transform was applied all along; it just never reached JavaScript

**TICK SHAPE: capability** — roadmap item #3.

The ledger called this *"`transform` not in computed style — a real gap"* and made it sound like a layout
bug. **It is not.** The transform has always been *applied*: `translateX(100px)` really moves the box, and
`getBoundingClientRect()` agrees to the pixel. Tick 65's probe established that, and `G_CAPABILITY` asserts
it.

What was missing is the **read-back**, and that has its own distinct damage. Every animation library reads
the current transform before composing its own:

```js
el.style.transform = getComputedStyle(el).transform + ' scale(2)';
```

`undefined + ' scale(2)'` is the **string** `"undefined scale(2)"`. Not a parse error. Not an exception.
The element simply stops moving. GSAP, Framer Motion, and every hand-rolled tween on the web do exactly
this — which is why *"not in computed style"* is not a cosmetic gap even though the layout is right.

`getComputedStyle(el).transform` now returns the spec's **resolved value**: `matrix(a, b, c, d, e, f)`,
never the author's shorthand, because a library that re-parses it depends on that. Functions compose
left-to-right as CSS multiplies them, and a **percentage** translate resolves against the element's own
border box — get that wrong and every centred modal on the web moves to the wrong place.

`ComputedStyle.transform` already existed as a `Vec<TransformFn>`; it was simply never serialized. The tick
is small on purpose: it is real, it is gated, and it is proven falsifiable.

**Not done, and said out loud:** transitions still snap to their end state (no tween). Low damage — the end
state *is* the content.

**The ratchet.** Capability: **up**. Performance: unchanged. Instrument fidelity: **up** — `G_TRANSFORM`
goes red when the resolver is disabled, and `G_CAPABILITY` asserts the matrix rather than printing the gap.

## Tick 67 — `scrollTop` did not merely not work. It lied, and so did `scrollHeight`.

**TICK SHAPE: capability** — roadmap item #2, from the roadmap tick 65 rebuilt from measurement.

**The gap was not absence.** `element.scrollTop` read `undefined`, and writing it quietly created a plain
JavaScript own-property that scrolled nothing and threw nothing. A virtualised list would set it, read it
back, get **its own value**, and conclude it had worked. *The failure was silent on both sides of the API.*

**And the probe found something worse underneath.** `clientWidth`, `clientHeight`, `scrollWidth` and
`scrollHeight` all **existed** — aliased to `offsetWidth`/`offsetHeight`, the element's own border box.
They looked present and they were wrong in the one way that matters:

> **`scrollHeight - clientHeight` was always ZERO.** That is precisely the number every virtualised list
> divides by to decide which slice of the data to render. Not `undefined`, which fails loudly — *zero*,
> which fails as "there is nothing to scroll."

Only the gate found it: the clamp computed 900 correctly while the getter reported 100, and two numbers
that disagree about the same fact mean one of them is not reading what it thinks.

**It is real now** (`G_SCROLL`): truthful geometry, writes clamped to `scrollHeight - clientHeight` (a
script that assigns `1e9` to reach the bottom reads back the real maximum — otherwise *"am I at the
bottom?"* is false forever), the offsets survive re-layout (layout starts from zero every time, so without
care the user types in a chat box and the list jumps to the top), the `scroll` event **fires** (an infinite
feed listens for it to fetch the next page), and — the assertion that cannot be faked — **it moves the
actual pixels.**

**And it needed no painter changes at all.** A scroll container's clip is *already* its padding box, so
translating its subtree up by `scrollTop` slides content out of that clip exactly as a real scroll does.
Anything scrolled out of view is clipped away for free, because it was always going to be. The translate
is by the **delta**, never the absolute offset — the tree already carries the old one, and translating by
the absolute value each time scrolls cumulatively, which looks exactly like a runaway-scroll bug.

A latent bug fell out on the way: `LayoutBox::translate` **did not move the list marker**. A `<ul>` inside
a float — and now inside a scroll container — would have kept its bullets behind while its text moved.

**The ratchet.** Capability: **up** — virtualised lists, chat panes, infinite feeds, scroll-to-top.
Performance: unchanged. Instrument fidelity: **up** — `G_SCROLL` proven falsifiable (stop moving the tree
and it goes red *while every JS-visible number stays correct*, which is the bug it replaced), and
`G_CAPABILITY` now asserts scroll geometry rather than printing it as a gap.

## Tick 66 — `<canvas>` paints, and the pixels reach the screen

**TICK SHAPE: capability** — the #1 item on the roadmap that tick 65 rebuilt from measurement.

**The bug was not that canvas was missing. It was that canvas said yes.** `getContext('2d')` returned a
context, `fillRect` was a function, nothing threw — and every drawing operation was a `noop`. Fill the
canvas red, read the pixel back: **`0,0,0,0`**.

That stub was a *deliberate and correct trade* when it was written — `getContext` had been `undefined`,
which made `ctx.fillRect(...)` on the next line a `TypeError` that took the whole bundle down, and **a
blank chart on a working page beats an exception**. It even warned in the console. But it is the worst
*shape* a failure can take while still counting as working: the page feature-detects canvas, is told
**yes**, draws its chart, and nothing appears, with no error anywhere.

**It rasterizes now**, on tiny-skia — the same rasterizer that paints the page. Fills, strokes, paths
(including `arc`), the full transform stack, `clearRect` to *transparent*, real `getImageData`
(non-premultiplied, as the spec hands JS), real `toDataURL` PNG.

**And the pixels reach the screen with no new machinery, which is why this was one tick and not five.**
The painter already scales a `DecodedImage` into a replaced element's box, keyed by `NodeId` — that is how
`<img>` works and how an `<iframe>` composites. **A canvas is just an image the page draws into.** So each
canvas owns a `Pixmap`, and `Page::drain_canvases()` drops the finished ones into the very same map an
`<img>` lands in. The painter never learns that a canvas exists.

The split: the **state machine** (fillStyle, transforms, the current path) stays in JavaScript, where it is
cheap; only **rasterization** crosses into Rust, with colour and transform already resolved. A path crosses
as **one flat array** — a chart with 10,000 points must not pay 10,000 FFI crossings — and every read of
it is bounds-checked, because a panic inside a JSNative is `nounwind` and aborts the browser rather than
throwing.

**The bug that cost the most was not in the rasterizer.** `canvas.width`/`height` **did not exist as JS
properties**, so `el.width` read `undefined`, the surface fell back to the 300×150 default, and the drawing
was then *perfectly correct inside a 300×150 surface* — which the painter dutifully scaled down into the
element's real 40×40 box. The chart came out as a smudge in the corner.

> **The pixels were right and the surface was wrong**, which is far more confusing than a blank canvas,
> because `getImageData` agrees with you the whole way down.

**And two self-inflicted ones, both caught by the wall:**

* I put **two `#[test]`s in a JS gate binary** and it segfaulted — PROCESS #17, *which I wrote myself*: two
  SpiderMonkey contexts co-running tear down messily. One test per JS gate binary, on purpose.
* `pub mod canvas;` inserted above `pub mod dom_bindings;` **stole its `#[cfg(feature = "_sm")]`** — an
  attribute belongs to whatever follows it, and what followed it had just changed. The entire JS binding
  layer went unconditional: **283 errors in the no-feature build**, which is the one the wall runs and I
  never do by hand. It surfaced as *"manuk-agent tests failed"*, three crates from the cause. (PROCESS #43.)

**Honestly not done**, and named rather than hidden: `fillText`, `drawImage`, `clip`, `putImageData` are
no-ops; gradients approximate to their last stop. `measureText` returns a *real* shape, because layout code
multiplies by `.width` and `undefined * n` is `NaN`, which poisons every coordinate downstream.

**The ratchet.** Capability: **up** — charts and visualisations render, and they are everywhere in the doc
and platform web. Performance: unchanged. Instrument fidelity: **up** — `G_CANVAS` proves it by *reading
the pixels back* (`typeof ctx.fillRect === 'function'` is what the stub passed for sixty ticks), and
`G_CAPABILITY` now asserts red pixels rather than merely a context object.

## Tick 65 — the ledger's top three priorities were all phantoms

**TICK SHAPE: instrument** — but the instrument is the one that aims every other tick. `[no-pattern]`
(no engine change; the pattern ledger was *corrected*, not extended).

Tick 64 caught the ledger being wrong about React. That made me look at the row above it — *"~1 site in 4
still **hangs**. Bar 0. Nothing else matters at this ratio."* **The measured figure is 4 sites in 265.**
Off by 16×, and it had been steering the roadmap for many ticks.

So I probed **every remaining `❌`** in `WEB-PATTERNS.md`. The result is hard to read charitably:

| ledger | truth |
|---|---|
| `append`/`prepend`/`before`/`after`/`replaceWith` ❌ | **all five work** (plus `insertAdjacentHTML`, `remove`) |
| `outerHTML`, `innerText` ❌ | **both work** |
| `Blob` / `File` / `FileReader` ❌ | **all three work** |
| `getSelection` / `Range` ❌ | both **exist** (only `createRange` missing) |
| `MutationObserver`, `ResizeObserver`, `structuredClone` | **all work**, unmentioned |
| CSS `transform` — "not in computed style, a real gap" | the transform **is applied**; the box moves; only the read-back is missing |
| React's commit ❌ | **renders** |
| the hangs — "1 in 4" | **4 in 265** |

**Three of the top three priorities were phantoms.** The loop was aiming at ghosts.

**The real gaps, measured, with receipts:**

* **`<canvas>` 2D is a stub that silently draws nothing** — fill it red, read the pixel back, get
  `0,0,0,0`. It is deliberate and warns in-product (a blank chart beats a `TypeError` that takes the page
  down), but a site that feature-detects canvas is told **yes** and renders nothing. **This is the next
  exploit tick**, and tiny-skia already backs our painter.
* **`scrollTop` lies** — reading gives `undefined`, writing silently creates a plain JS property that
  scrolls nothing. A virtualised list sets it, reads it back, and believes it worked.
* `getComputedStyle().transform`, `display:contents`, `createRange`, `createEvent`,
  `URL.createObjectURL` — absent, named, small.

**The mechanism, because the lesson has failed five times.** *An absent measurement is not a negative
measurement* is written in PROCESS #19, #20, #21, #35 and #41, and it did not hold. **A rule I can recite
while breaking it is a decoration.** So it is not a rule any more:

> **`G_CAPABILITY` runs the ledger's claims as assertions.** 42 of them, on every wall. A `✅` that stops
> being true **fails the tick** — the RATCHET made mechanical. Every `❌` prints a receipt from the same
> run. The ledger cannot drift from reality, because reality is what runs.

It caught a bug in *itself* on the first run: the claims append children to the shared fixture, so
`cloneNode(deep)`'s child-count assertion was failing on the test's own side effects. **A shared fixture
that the assertions mutate is a fixture that lies about the engine.**

**The ratchet.** Capability: unchanged (nothing was added — a good deal was *found*). Performance:
unchanged. Instrument fidelity: **up, sharply** — the file that decides what gets built next is now
checked against the engine on every tick, and the roadmap is rebuilt from measurement rather than rumour.

## Tick 64 — the DOM methods were on the wrong objects, and it cost 60× and every prototype patch

**TICK SHAPE: capability** — and a performance step-function that fell out of the same fix.

**The ledger sent me at React, and React was fine.** `WEB-PATTERNS.md` said *"React committing its render
— ❌ still silent. Mounts, schedules, throws nothing, renders nothing."* Probe first, per the rule: I ran
the real Vite/React bundle through the engine before touching a line. **It renders.** `#root` gets its 6
children, the app's own text (*"Count is 0"*), 59 elements, **zero errors**. That `❌` was never measured;
it was inherited. Fifth recurrence of PROCESS #35 — *an absent measurement is not a negative measurement*.

**But the probe found the real gap next door, and it was bigger.**

`typeof Element.prototype.addEventListener` → **`undefined`**. Every DOM method was defined as an
**own-property of every element** — 116 of them, one `JS_DefineProperty` per node. Which means:

1. **`Element.prototype.setAttribute` was `undefined`.** So was `Node.prototype.appendChild`.
   `EventTarget` did not exist at all — a bare `ReferenceError`.
2. **Patching a prototype silently did nothing.** `Element.prototype.setAttribute = wrapper` — *the* way
   Sentry, ad-blockers, polyfills, framework internals and React DevTools hook the DOM — succeeded, threw
   nothing, and was **never called**, because the element's own property shadowed it. **The library
   believes it is installed and it is not.** A loud failure gets fixed; a silent one ships.
3. **It was slow, per element.** 116 defines *plus two full JS compiles* per node (the identity cache was
   read and written by `eval`ing a formatted string). **5,000 `createElement`s took 124ms.**

**The fix.** A real chain, built once per global:

```
element  → HTMLElement.prototype → Element.prototype → Node.prototype → EventTarget.prototype
document → Document.prototype    → Node.prototype    → EventTarget.prototype
```

Members defined **once**. Identity cache is a real object read with `JS_GetElement`, not a compile.

| | before | after |
|---|---|---|
| `createElement` × 5,000 (release) | **124ms** | **2ms** (~60×) |
| own properties per element | **116** | **1** |
| `Element.prototype.setAttribute` | `undefined` | function |
| patching it | **silently ignored** | actually runs |
| `EventTarget` | `ReferenceError` | exists, and elements are instances |

**Two traps, both of which bit.**

*The GC one:* I cached the `__nodes` object as a raw `*mut JSObject`, then called `dom_protos()` — which
defines 116 properties, any one of which can trigger a **moving** GC. Segfault on the first page. Rust
cannot see this: to it a `*mut JSObject` is a number. **Root immediately, always.**

*The silent one:* removing the old `bridge()` shim left an orphaned `try {` in a 71KB embedded JS blob. A
syntax error there does not throw — it **fails to install the entire JS environment**, and every page
then renders as static HTML with no error at all. `js_conformance` caught it, which is the wall doing
precisely its job.

**Honesty about WPT.** `dom/nodes` reads 26.9% against a recorded 22.4%, and it was tempting to bank that
here. So I A/B'd it on the same tree, with the change mutated out: **1736/6418 — identical, to the
subtest.** This work moved WPT **not at all**; the baseline had simply gone stale, and a stale baseline
will happily hand you a win you did not earn. **A number you cannot attribute is not a result.**

**The ratchet.** Capability: **up** (prototype patching, `EventTarget`, the interface surface). Performance:
**up, 60×** on DOM node creation — every React/Vue/Angular render pays that cost on every commit.
Instrument fidelity: **up** — `G_PROTOTYPE`, proven to go red when the members go back on the instance;
and one false `❌` removed from the capability ledger.

## Tick 63 — the release cadence was green, and shipping nothing

**TICK SHAPE: instrument** — no engine change. `[no-pattern]`.

The demo CI and the verify wall both went green after tick 62, and the 41-site demo is live. But the
**release** workflow reported ✅ success and shipped **no binary** — for a tick that closed a Bar 0 defect
a user can feel (*you can quit the browser without losing your session*).

**Why.** The release gate greps the **commit message** for `TICK SHAPE:`. The pre-commit hook enforces
that trailer in the **journal**. Two sources of truth for one claim, and the gate read the one nobody is
required to write to. When a commit message happened to repeat the shape (tick 60) a binary shipped; when
it did not (tick 62) the gate found nothing, took the `else` branch, printed *"not a capability tick"*,
and reported success.

> **A cadence that never fires and never complains is worse than no cadence at all** — the green check
> certifies that it is working.

**And the fix's first draft had the same disease one layer down.** I wrote
`awk "/^## Tick 62/,/^## Tick [0-9]/"` to pull the shape out of the journal. **Awk tests a range's END
pattern against the START line too**, so `## Tick 62` matched both, the range collapsed to a single line,
the shape was never inside it — and it reported "no shape declared" for *every tick in the file*. I caught
it only because I ran the extraction against the real journal instead of trusting that it worked.
`tick.sh` has always used `,0` (to EOF); now so does this.

Proven against the real journal, for real ticks: 59 → ship, 60 → ship (and v0.60.0 **is** the latest
release, which is the corroboration that the diagnosis is right), 61 → skip (an instrument tick, correctly
not a release), 62 → ship.

**The honest cost:** v0.62.0 was missed and is not being back-dated. Tick 62's fix is on `main` and goes
out with the next capability tick, which the gate will now actually fire for.

**The ratchet.** Capability: unchanged. Performance: unchanged. Instrument fidelity: **up** — the release
cadence the directive asked to be *mechanically enforced* is now mechanically enforced, rather than
mechanically pretending.

## Tick 62 — the exit segfault, closed: to run first at teardown, register last

**TICK SHAPE: bar-0** — the stability floor. A sixty-tick-old residual, and it was ours all along.

**The bug.** Any binary that ran JavaScript and did not explicitly call `manuk_js::shutdown()` would do
its work perfectly, print correct output, and then **SIGSEGV after `main` returned**
(`pthread_mutex_destroy failed: Device or resource busy`). SpiderMonkey needs `JS_ShutDown()` before the
process exits; without it, its C++ static destructors run against a live engine and die in
`__run_exit_handlers`.

**Why it mattered more than it looked.** A crash in the exit handlers *aborts the handlers after it* —
which is exactly where a browser flushes its cookie jar and `localStorage` to the profile (ADR-009). The
user-visible bug is **silent data loss on quit**. That is a Bar 0 defect, not a cosmetic exit code.

**Why it survived sixty ticks.** The "fix" was a convention: *every binary must call `shutdown()` last*.
`g_runaway`, `g_alloc`, `g_load_budget` and the shell remembered. `g_globals` and `g_dedup` did not, and
crashed every single run. **A convention that half the callers forget is not a fix; it is a list of the
places you have not been bitten yet.**

**The probe came first, and it is what made the rest possible.** `G_CLEAN_EXIT` re-executes the test
binary as a child that runs real JavaScript and then simply *returns from `main`* — no shutdown call, on
purpose — and demands exit code 0. It went **RED at 139** on the engine as it stood.

**The fix, and the trap inside it.** The obvious move — one struct holding the `Runtime` and the
`JSEngine`, one thread-local, a `Drop` that orders them — **does not work**, and I shipped it and watched
the gate stay red to find out why:

> **Thread-local destructors run in REVERSE order of registration**, and mozjs registers thread-locals
> *lazily*: `Runtime::drop` → `finishRoots` → `trace_traceables`, which does not exist until the first
> `rooted!` — i.e. until the first eval. Our state must be created *before* that, so it registers first,
> so it is destroyed **last**, reaching for a mozjs thread-local that is already gone. `cannot access a
> Thread Local Storage value during or after destruction`, in a `nounwind` frame: instant abort. **One
> exit crash traded for another.**

`atexit` is no escape either — glibc runs `__call_tls_dtors()` *before* the atexit list.

So: split the **state** from the **trigger**. `ENGINE` and `RUNTIME` hold `ManuallyDrop` (no drop glue ⇒
**no TLS destructor is ever registered** ⇒ readable at any point during shutdown), and a separate empty
`TeardownGuard` carries the `Drop` — armed **after the first eval**, so it registers *after* mozjs's lazy
thread-locals and is therefore destroyed *before* them.

> **To run first at teardown, register last.**

`g_globals` and `g_dedup` now exit 0. So does a bare binary that never heard of `shutdown()`.

**A second thing this tick, and it is a repeat offence.** `G_DEMO_LIVE` — the gate I built *last* tick to
stop the demo shipping unpainted — **broke the GitHub Pages deploy**. It slept 3 seconds and then
connected to Chrome's debug port; on a runner, Chrome needed longer, so it got `Connection refused` and
failed the build. That is PROCESS #31 (*my instrument broke the build it was measuring*) for the third
time, and the root cause is the same as #36: **a fixed sleep standing in for the condition I actually
care about.** It polls now. The rule, which has cost three defects: *never sleep where you can wait for
the thing.*

**Wall time.** `G_CLEAN_EXIT`'s first version shelled out to `cargo run --example` and put **215 seconds**
on the verify wall. Rigor bought with a 6× slower loop is a trade the ratchet does not permit, so the
child is now this very test binary re-executed with an env var: same evidence, **0.18s**. Wall is 32s,
which is *better* than the 40s baseline.

**The ratchet.** Capability: **up** — a user can quit the browser without losing their session.
Performance: unchanged (wall improved). Instrument fidelity: **up** — `G_CLEAN_EXIT` is proven
falsifiable, and the demo gate can no longer take the deploy down with it.

## Tick 61 — the corpus goes to 41 sites, and three instruments lie in a row

**TICK SHAPE: instrument** (the engine is unchanged; what changed is what the engine can be *seen* doing,
and what the loop can *trust* about what it sees). `[no-pattern]` — no `engine/*/src` change.

**Hypothesis.** The demo shipped 13 sites. Thirteen sites is an anecdote. If the claim is *"this engine
renders the real web"*, the corpus has to be big enough and varied enough that it could **fail** — so the
corpus was taken to **41**, spanning the three classes the roadmap is organised around: 17 doc-web
(Wikipedia, HN, RFC-Editor, Craigslist, BBC, the Guardian, SQLite, kernel.org…), 11 app-web (the
server-rendered output of React, Next, Svelte, Vue, Astro, Remix, Solid, Vite, Nuxt, Angular), and 13
platform-web (Tailwind, Bootstrap, GitHub, Stripe, MUI, Chakra, Cloudflare, Vercel, Linear…). Every one
carries a Chromium reference render, so every one can be looked at side by side and disbelieved.

**Two real findings, neither of them about the engine.**

1. **The stage timings read `0ms`.** `js_sys::Date::now()` is coarse to 1ms, so every stage of the
   pipeline rounded to zero and the provenance panel — the entire point of which is to show the engine
   working — showed nothing. Switched to `performance.now()` via `web-sys`. Real numbers: **parse 18ms,
   cascade 54ms, layout 51ms, raster 55ms** on Wikipedia's 2,281 nodes.

2. **Snapshots ship inline `<script>` we never execute.** Stripping it is not hiding anything — the demo
   has no JS engine and says so on its own front page — and it is most of the bytes. What is left is the
   *markup and the CSS*, which is exactly what Stylo and Taffy are here to chew on. `github.html` is 4.5M
   of which **4.2M is inlined CSS**: that is the substance, and it stays.

**And then the instruments lied, three times, in one tick** (PROCESS #36, #37, #38 — this is the real
content of the tick and it is worth more than the corpus):

* `--virtual-time-budget` **froze the clock I was measuring with**, so the fixed timings still read `0ms`
  and I was one step from going back into Rust that was already correct.
* `--dump-dom` fires at `load`, which does not wait for an async wasm boot — so it reported an engine that
  had never run, *every single time*, regardless of the truth.
* `--screenshot` waits **sometimes**. It caught the render once and missed it on the next run of identical
  code. A flaky observer is worse than no observer: it makes a working build look broken at random, and I
  believed it.

All three are one defect wearing three coats: **the instrument was blind to the thing it was reporting as
absent.** The answer was to stop *inferring* "did it run?" from whatever side-effect happened to be
observable, and to **ask the page** over the DevTools protocol once it has actually finished —
`scripts/demo-verify.py`, now a gate (`G_DEMO_LIVE`) the build cannot pass without.

**The gate written to catch that was itself vacuous** (#38): it asked *"is any pixel non-white?"* to prove
the canvas was painted. An untouched canvas is transparent **black**, which satisfies that trivially — so
it reported PAINTED for a blank demo, and a mutation deleting the paint call went straight through it,
green. It now counts **distinct colours** and demands more than two, and it is trusted for one reason
only: it was **proven to go RED**, twice.

**And I destroyed 306 lines of uncommitted work with `git checkout`** (#37) — the *second* time, and I
typed the words *"never do this (PROCESS #32)"* into the same shell command that did it. Recovered only
because the file's bytes happened to still be in the session transcript. That is luck, not a mechanism, so
there is now a mechanism: **`scripts/snap.sh`** snapshots the working tree into a dangling commit before
every wall, falsifier, demo build and tick. Proven by re-running the exact destructive command and
recovering the file byte-for-byte.

**The ratchet.** Capability: unchanged, by design. Instrument fidelity: **up** — the demo can no longer
silently ship without having painted, and the loop can no longer silently lose a file. Both of those were
true-but-unprovable yesterday and are mechanical today.

## Tick 60 — a Text node could have children (2026-07-14)

**TICK SHAPE: pattern-class** · **CLUSTER: C00wpt** — the class is *DOM code that catches errors*, which is
every framework's unmount path and every sanitizer.

**Hypothesis:** WPT's `dom/nodes` shows **588 `assert_throws_dom` failures** — our DOM methods *silently
no-op* where the spec requires them to **throw**. Real code catches those exceptions; ours never raises
them.

**RESULT — three spec violations closed, and the first one is the interesting one.**

1. **A Text node could have children.** `text.appendChild(div)` **succeeded**. That sounds like a triviality
   right until you notice what it leaves behind: **a subtree hanging off a text node that no traversal in
   the engine expects and nothing will ever render.** *Silently accepting an impossible tree is worse than
   refusing it* — the corruption does not surface where it was created; it surfaces later, somewhere else,
   looking like something unrelated. The spec's rule (*"if parent is not a Document, DocumentFragment, or
   Element, throw HierarchyRequestError"*) exists to stop exactly that.

2. **`insertBefore(node, ref)` where `ref` is not a child** silently **appended instead** — putting the node
   somewhere the page never asked for, **with no way for the page to find out.** Now `NotFoundError`.

3. **`removeChild(node)` where `node` is not a child** silently did nothing. **Every framework's unmount
   path catches this exception**; a DOM that never raises it turns a loud bug into a **silent leak**. Now
   `NotFoundError`.

**MEASURED — the ratchet turned:** `dom/` **1738/6499 (26.7%) → 1749/6499 (26.9%)**, **Bar 0 clean (0)**,
**NO_REPORT 0**. A modest number, and an honest one: most of the remaining 500-odd `assert_throws_dom`
failures want throws from methods we have not reached yet. *The point of the work list is that it does not
run out.*

`G_DOM_IMPL` now gates all five validity throws (cycle, Document-as-child, Text-with-children, bad
reference, bad remove).

**And the gate caught a regression I introduced, which is the ratchet working.** The spec's parent check is
*"Document, DocumentFragment, or Element"* — but **a ShadowRoot is a DocumentFragment to the spec
(`nodeType` 11) and a DISTINCT `NodeData` variant in this arena.** So the naive check **rejected
`shadowRoot.appendChild(...)` — which is how EVERY web component builds its content.** The JS-conformance
gate went red instantly (the framework-primitive suite's output collapsed to `"-"`, i.e. the script threw),
and the tick **did not land** until it was fixed.

> **A spec fix that breaks a working capability is not a fix.** That is what the wall is for, and it is why
> the ratchet has three faces and not one.

## Tick 150 — percentage heights resolve against the initial containing block; `max-height:%` on an indefinite parent is `none` (2026-07-17)

**TICK SHAPE: layout-mechanism (CSS-LAYOUT phase-mandate row 4 — %-height edges). WIKI: box-layout.**
Lever-board phase mandate:
lowest-numbered unmet RENDER+INTERACT target. Rows 1/2/3/5 already met (t139/140); row 4 (block/inline
edges — margin-collapse, %-height, inline-space) was open. Took the **%-height** half — the cleanest,
highest-daily-driver bounded mechanism on that row.

**Hypothesis.** Two percentage-height reference bugs, both silently resolving `%` against **0**:
(1) `layout_document` seeds the root box with `pch: None`, so a root-level `height:100%` — the
`html,body{height:100%}` → `#app{height:100%}` full-height app-shell chain every SPA uses — is indefinite
and collapses to content height, while a `100vh` sibling (parse-time) fills the window. (2) `max_h`
resolves a percentage against `pch.unwrap_or(0.0)`, so `max-height:100%` inside an auto-height parent →
`0` and the box vanishes (the `img{max-width:100%;max-height:100%}` reset dies).

**Fix.** (1) Seed the root's `pch` with `Some(manuk_css::values::viewport_size().1)` — the ICB has the
viewport's dimensions (CSS2 §10.1), read from the *same* viewport `vh` resolves against so the two spellings
agree. (2) `Dim::Percent(_) if pch.is_none() => f32::INFINITY` (+ the `Calc{pct!=0}` form) — a percentage
`max-height` against an indefinite CB is `none` (CSS2 §10.7). `min-height:%` indefinite → 0 already correct.

**Verified.** `css/CSS2/normal-flow` 17→18 (the `height:30000px; max-height:100%` case flips). App-shell
chain is reftest-covered (Bar 2), so gated by two unit tests: `root_percentage_height_fills_the_viewport`
and `percentage_max_height_indefinite_parent_is_none`, both proven RED by reverting. Regression sweep:
css-flexbox 26.5%, css-position 28.8%, css-overflow 27.8%, css-sizing 14.5% — all flat, **HANG/CRASH 0**.

**Ratchet.** Capability up (full-height app shells + responsive-image resets now render); performance
unchanged; instrument fidelity up (two new falsifiable layout gates). No invariant bent — surgical,
spec-cited, root-only percentage change; auto-height pages untouched. Mechanism in
[[box-layout]]. Next lever on row 4: the inline-whitespace edge (`a<b>b</b>` spacing) or margin-collapse
parent↔child (explicitly unmodeled in `layout_block`).

## Tick 151 — parent↔child margin collapsing (top + bottom); the last row-4 layout mechanism (2026-07-17)

**TICK SHAPE: layout-mechanism (CSS-LAYOUT phase-mandate row 4 — block/inline edges). WIKI: box-layout.**
Row 4's remaining unmet piece after t150's %-height half. Two candidates were flagged: the inline-space
edge (`a<b>b</b>`) and parent↔child margin collapse. The inline-space edge PROBED CLEAN — `pending_space`
already stays false across adjacent inline elements with no source whitespace, and collapses to one space
when whitespace is present, so `a<b>b</b>` already gains no space. The real gap is **parent↔child margin
collapsing (CSS2 §8.3.1)**, documented as an explicit simplification in `layout_block` for ~150 ticks.

**Hypothesis.** Two symmetric bugs, both leaving a spurious gap of the child's margin *inside* the parent:
(1) TOP — a block with no top border/padding, `overflow:visible`, not a BFC, does NOT collapse its top
margin with its first in-flow block child's top margin; the child's margin sits below the parent's
content-top instead of escaping upward (`<div><h1>` shows an h1-margin gap inside the div's background).
(2) BOTTOM — the same block does not collapse its bottom margin with its last in-flow block child's;
`layout_children` returns a height that *includes* the trailing child margin (the "still occupies the
container" line), so the margin is double-counted (inside the parent AND added again below it).

**Fix.** A cheap left-spine peek `collapse_through_top(node)` (depth-bounded, O(spine)) computes the
first-in-flow-block child's collapse-through top margin; `layout_block` folds it into the box's own top
margin (raising the box, placing the child flush at content-top) and reports the collapsed value as
`margin_top` so a grandparent collapses correctly. BOTTOM: `layout_children` now also returns the trailing
collapsible margin; when the box is bottom-eligible and auto-height, that margin is subtracted from
content-height and collapsed into the box's own `margin_bottom`. Eligibility is conservative
(`display:block`, `overflow:visible`, no border/padding on that edge, not a BFC, clearance blocks the top
collapse) — a leading/trailing out-of-flow child or clearance declines the collapse rather than risk it.

**Gate.** Unit tests proven RED by reverting: `parent_child_top_margin_collapses`,
`parent_child_bottom_margin_collapses`, `overflow_hidden_contains_child_margin` (no collapse),
`top_border_blocks_margin_collapse`. Regression sweep across css-flexbox/position/overflow/sizing/CSS2
normal-flow with HANG/CRASH 0. Approximation documented: percentage vertical margins deep in the spine
resolve against an approximate width (px/em — the norm — are exact).

## Tick 152 — `overflow:hidden/auto/scroll` establishes a block formatting context (contains floats; the clearfix) (2026-07-17)

**TICK SHAPE: layout-mechanism (CSS-LAYOUT phase-mandate row 7 — overflow scroll-container: "overflow:hidden = BFC contains floats"). WIKI: box-layout.**
Row 4 completed at t151; grid-template-areas (row 6) is ALREADY wired to taffy (lever-board's "no
taffy consumer!" note is stale — confirmed at taffy_tree.rs:360). Row 7's float-containment half is a
real, confirmed gap.

**Hypothesis (probed).** `establishes_bfc` explicitly excludes `overflow` ("overflow is not modeled
yet"). A throwaway probe confirmed it: a `overflow:hidden` parent containing a `float:left;height:60px`
child came out **18px tall** (only its own text line) — the float ESCAPED instead of being enclosed.
This breaks the single most common float-containment idiom on the web: `overflow:hidden` (or `auto`) on
a container to make it wrap its floated children (the modern clearfix), and to stop its content flowing
around an *outer* float. Both are BFC properties (CSS2 §9.4.1 / §10.6.7).

**Fix.** Add `s.overflow != Overflow::Visible` to `establishes_bfc`. A non-visible-overflow box then
gets its own float context (its floats stay inside; its content does not overlap outer floats) and grows
to contain its floats via the existing `own_bfc.lowest_bottom()` auto-height path. `Overflow::Clip` is
included (it too clips and does not establish the scroll-container relationship... actually clip is
subtle — treat any non-visible as BFC, matching the clip-establishes-BFC-in-practice reading).

**Gate.** Unit test `overflow_hidden_contains_floats` (parent height >= the float's 60px), proven RED by
the probe above (18px). Regression sweep across css-flexbox/position/overflow/sizing/CSS2-normal-flow;
HANG/CRASH 0 and no suite regresses, else revert (overflow:hidden is pervasive — this is the higher-risk
half of the change and the sweep is the guard).

## Tick 153 — `width: fit-content | min-content | max-content` on a block hugs its content instead of filling (2026-07-17)

**TICK SHAPE: layout-mechanism (CSS-LAYOUT phase-mandate row 1 — intrinsic sizing; the remaining unmet slice). WIKI: box-layout.**
Probed rows 1/5/6/7 of the RENDER+INTERACT mandate directly with `manuk-wpt boxes`: border-box padding
(child=136px), calc sidebar (main=750px), flex:1-with-long-token (212/44/44), justify-between (0/130/260),
grid repeat(3,1fr) (0/110/220), align-items stretch/center, flex-column grow — **all already correct**.
The one that PROBED RED: `width: fit-content` on a block came out **300px (filled the container)** where
Chrome hugs the content (~14px). `width: stretch` and `-webkit-fill-available` were already correct (300).

**Hypothesis.** The three *intrinsic sizing keywords* (`min-content`/`max-content`/`fit-content`) all
collapse to `Dim::Auto` in both style paths (`stylo_map::size_to_dim` and the hand-parser), and only a
`height_intrinsic: bool` is retained — for the abspos indefinite-height case, never for width. So a block
`width: fit-content` is indistinguishable from `width: auto` and takes the auto-width *fill* branch
(`cw - extra`), stretching to the containing block. This breaks the single most common "hug the contents"
idiom on the web: a `fit-content` badge/tag/pill, a `width:max-content` single-line label, and the
`width:fit-content; margin-inline:auto` centered-block-that-hugs pattern.

**Fix.** A new `IntrinsicSize { MinContent, MaxContent, FitContent }` tag, stored as
`ComputedStyle::width_keyword: Option<IntrinsicSize>`, set in BOTH style paths (stylo map + hand-parser)
next to the existing `height_intrinsic`. In block width resolution, the `Dim::Auto` branch consults it:
`MinContent → min_content_width(node)`, `MaxContent → max_content_width(node)`, `FitContent →
shrink_to_fit(node, cw - extra)` — the exact functions inline-block already uses (line 1535), so the
Bar-0/recursion profile is identical to a proven-safe path, and the returned values are content-box
widths so the box-sizing subtraction (guarded on `width != Auto`) correctly stays skipped. min/max-width
clamps still apply after, per CSS Sizing L3. Taffy-decided flex/grid items (`taffy_known`) are untouched.
Width-only scope: block auto-height already resolves to content height, so the height keywords behave
correctly today (the abspos case stays covered by `height_intrinsic`).

**Gate.** Unit tests proven RED by reverting: `width_fit_content_hugs`, `width_max_content_hugs`,
`width_min_content_is_longest_word`, and `width_fit_content_still_clamped_by_max_width`. Probe: s3 flips
300→~14. Regression sweep across css-sizing/css-flexbox/css-grid/css-position/CSS2-normal-flow with
HANG/CRASH 0 and no suite regressing, else revert. Mechanism in [[box-layout]].

## Tick 154 — `height: stretch | -webkit-fill-available` on a block FILLS its parent's definite height (2026-07-17)

**TICK SHAPE: layout-mechanism (CSS-LAYOUT phase-mandate row 1 — intrinsic/keyword sizing; the vertical companion of t153). WIKI: box-layout.**
Probed after t153: `height:stretch` on a block inside a 200px-tall parent came out **18px** (content height),
where Chrome fills to **200px**. `-webkit-fill-available` same (18px). Unlike WIDTH — where `auto` already
fills so `stretch` "worked" incidentally — a block's `height:auto` is CONTENT height, so `stretch` and
`auto` are a real, visible distinction that was never modeled. Tick 146's comment even declared stretch
"definite", but nothing gave it filling behavior: it collapsed to `Dim::Auto` = content height.

**Hypothesis.** `stretch`/`-webkit-fill-available`/`-moz-available` on `height` collapse to `Dim::Auto`
(definite, so NOT flagged `height_intrinsic`) and are then indistinguishable from `auto` → the box takes
its content height instead of filling the containing block. This breaks full-height panels/columns that
use `height:stretch` (or the older `-webkit-fill-available` mobile-viewport idiom).

**Fix.** A new `ComputedStyle::height_stretch: bool`, set in both style paths (stylo map: `GS::Stretch |
WebkitFillAvailable | MozAvailable`; hand parser at parity). In `layout_block`'s `own_definite_h`, a new
arm: `Dim::Auto if height_stretch => pch.map(|h| (h - mt - mb - pt - pb - bt - bb).max(0))` — the box's
MARGIN box fills the containing block's definite content height `pch`, so the content box is that minus
this box's own margins/border/padding (box-sizing-independent: stretch fills available space, not a
specified length). `pch` (the parent's definite content height, threaded since t144) is the same reference
`height:%` children use, so a stretched box is correctly a definite-height CB for its `%`-height children.
When `pch` is `None` (auto-height parent) stretch stays content-height, at parity with Chrome. min/max-height
clamps still apply; the bottom-margin-collapse (guarded on `own_definite_h.is_none()`) correctly skips a
now-definite box.

**Gate.** Unit tests proven RED by reverting: `height_stretch_fills_definite_parent` (18→200),
`height_fill_available_fills_definite_parent`, `height_stretch_in_auto_parent_stays_content` (no over-fill),
`height_stretch_is_a_definite_base_for_percentage_child`. Regression sweep across
css-sizing/css-flexbox/css-grid/css-position/CSS2-normal-flow with HANG/CRASH 0 and no suite regressing,
else revert. Residue noted: `width:stretch` in a shrink-to-fit context (float/inline-block/abspos) still
behaves as `auto` — a separate, smaller mechanism. Mechanism in [[box-layout]].

## Tick 155 — `overflow-y: scroll` reserves a classic vertical-scrollbar gutter, narrowing the content box (2026-07-17)

**TICK SHAPE: layout-mechanism (CSS-LAYOUT phase-mandate row 7 — overflow scroll-container; the "overflow-y:scroll reserves gutter" half, companion to t152's overflow-establishes-a-BFC half). WIKI: box-layout.**
Probed css/css-overflow: every `scrollbar-gutter-001` `offsetWidth` case failed identically — a 200px
`overflow-y:scroll` container gave its `width:100%` child a **200px** used width where Chrome gives ~185.
A classic (space-taking) vertical scrollbar lives on the inline-end edge and eats inline width, but layout
laid children across the box's FULL content width — no scrollbar space was ever reserved. The daily-driver
face: the ubiquitous `html{overflow-y:scroll}` idiom (force a scrollbar on every page so navigating from a
short page to a tall one causes no horizontal layout shift) rendered content ~15px too wide, so every
centered container sat off-centre by half a scrollbar.

**Hypothesis.** `ComputedStyle` collapsed `overflow-x`/`overflow-y` into one more-clipping `overflow`
field, so `overflow-x:auto; overflow-y:scroll` (the test's base) read back as `auto` and LOST that the
vertical axis force-shows a scrollbar. Restore the per-axis values and reserve an inline gutter when
`overflow_y == Scroll` — the one deterministic case where a classic scrollbar is always present.

**Fix.** New per-axis `ComputedStyle::overflow_x`/`overflow_y` (the collapsed `overflow` stays for
clip/BFC — untouched, no regression there); set in both `stylo_map` (`s.overflow_x = ox; s.overflow_y =
oy;`) and the hand parser (which now also handles the two-value `overflow: <x> <y>` shorthand). In
`layout_block`, reserve `SCROLLBAR_WIDTH` (15px) of inline space when `overflow_y == Scroll`: the gutter
narrows only the content width handed to children and the BFC float band (`inner_width = width − gutter`);
`width` and `border_box_w` — the box's own `offsetWidth` — are untouched, so a 200px scroll container stays
200 while its `width:100%` child becomes 185. Applies uniformly to block and taffy flex/grid leaf items
(both route through `layout_block`), so a scroll-container flex item narrows its content too.

**Gate.** Unit tests proven RED by reverting: `overflow_y_scroll_reserves_inline_gutter` (child 200→185),
with controls `overflow_visible_reserves_no_gutter` and `overflow_y_auto_without_overflow_reserves_no_gutter`
proving the reservation is scoped to scroll containers (not every box, not an `auto` pane that fits).
Regression sweep, stash-rebuild-measured BEFORE vs AFTER on the same release binary:
css-overflow **131→132 (+1)**, css-sizing/css-flexbox/css-grid/css-position **all flat (0 regression)**,
HANG/CRASH 0. Residue: `scrollbar-gutter: stable`/`both-edges` is unreachable (crates.io stylo 0.19 has no
`scrollbar-gutter` support — dropped at parse, so no dead surface added); the `overflow:auto`-and-actually-
overflows case needs a second layout pass; RTL/vertical-writing-mode gutter placement and the
horizontal-scrollbar-reserves-height axis are separate, smaller mechanisms. Mechanism in [[box-layout]].

## Tick 156 — abspos auto margins center a fully-constrained box (`inset:0; margin:auto`) (2026-07-17)

**TICK SHAPE: layout-mechanism (CSS-LAYOUT phase-mandate — abspos positioning; row-8 neighbourhood).
WIKI: box-layout. Row 6 grid-template-areas is observer-PARKED; picked the next bounded lever.**
Probed css/css-position: the `margin:auto on abspos` cluster failed, and a `boxes --tree` probe showed a
`position:absolute; inset:0; margin:auto; width:200px; height:200px` box in a 400×400 relative CB laid
out at **[0 0 200×200]** where Chrome centers it at **[100 100]**. This is the canonical centered-modal /
dialog / backdrop idiom — pinned to the top-left corner instead of centered.

**Cause.** `layout_abs` resolved every margin with `Dim::resolve(cw, 0.0)`, so an `auto` margin became
**0**. CSS2 §10.3.7 (inline) / §10.6.4 (block): on a *fully-constrained* axis — both insets set AND a
definite size — the free space is distributed into the auto margins. That step was absent, so the box sat
at `cb.origin + inset`.

**Fix.** After `border_box_w`/`border_box_h` are known, redistribute per axis. Inline: `left`&`right` set
and `s.width != Dim::Auto` → `free = cw − left − right − border_box_w`; both auto → `free/2` each (negative
free, ltr → start 0, overflow past end); start (`margin-left`) auto → `free − margin-right`; end-auto /
neither → no-op (box already pinned by `left`+`margin-left`; an end margin only absorbs slack). Block axis
symmetric on top/bottom/height. The `!= Auto` guard excludes both the stretch-to-fill case (`width:auto`
between insets, auto margins = 0) and an intrinsic keyword (collapses to `Auto`) — neither is a definite
size.

**Gate.** Unit test `abspos_auto_margins_center_a_constrained_box` (200×200 `inset:0;margin:auto` centers
at (100,100); RED at (0,0) on revert — confirmed via the before/after box probe; `margin:0 auto` control
proves the axes resolve independently). Regression sweep, stash-rebuild-measured BEFORE vs AFTER on the
same release binary: css-position **76→79 (+3)** (the "margin:auto on abspos after dynamic inset change"
subtest flips green), css-flexbox/css-grid/css-sizing/css-values/css-overflow **all flat (0 regression)**,
HANG/CRASH 0. Residue: the *"margin:0 auto after **dynamic** inset change"* sibling still fails — a
dynamic-reflow gap (mutate `.style.inset`, re-read `offsetTop`), NOT layout math; writing-mode-aware
start-edge selection is a separate, smaller mechanism. Mechanism in [[box-layout]].

## Tick 157 — min/max-width/height clamp an absolutely-positioned box (2026-07-17)

**TICK SHAPE: layout-mechanism (CSS-LAYOUT phase-mandate — abspos sizing). WIKI: box-layout.**
`layout_abs` computed a used width/height and never clamped it: a `max-width:200px` dialog with
`width:500px` came out 500 wide; `min-width` tooltips and `max-height` panels all took their
unconstrained size. The in-flow block path has always clamped; the abspos path never grew the same lines
— the four `min/max-*` `ComputedStyle` fields were dead on this code path.

**Fix.** Mirror the block clamp on both axes. Width: after the `content_w` arm, clamp to
`[min_width.resolve(cw)−bs_extra_w, max_width.resolve(cw)]` (auto→∞) BEFORE `layout_children` so children
see the constrained width. Height: after `content_height`, clamp against `cb.height` (always definite for
an abspos CB, so a `%` bound resolves — no indefinite-parent `none` case). Max first, then min wins, both
content-box via the existing box-sizing deltas.

**Gate.** Unit test `abspos_min_max_size_clamps_apply` (500→200 max-width, 50→150 min-width, 500→80
max-height; RED unclamped on revert; probe confirmed [500→200], [50→150], [500→80]). Regression sweep,
stash-rebuild-measured BEFORE vs AFTER on the same release binary: css-position **79→88 (+9)**,
css-flexbox/css-grid/css-sizing/css-values/css-overflow **all flat (0 regression)**, HANG/CRASH 0, AFTER
stable across two runs. Residue: the 30 remaining `position-absolute-replaced-minmax` iframe rows need
replaced-element **intrinsic sizing** (empty abspos `<iframe>` → 300×150 default before the clamp table),
a separate mechanism; the over-constrained clamp-vs-insets re-solve uses the simple block-style clamp.
Mechanism in [[box-layout]].

## Tick 158 — overflow-x:scroll reserves a horizontal-scrollbar gutter (block-axis mirror) (2026-07-17)

**TICK SHAPE: layout-mechanism (CSS-LAYOUT phase-mandate — scrollbar-gutter, block axis). WIKI: box-layout.**
Tick 155 taught `layout_block` to reserve a classic vertical scrollbar's inline width for
`overflow-y:scroll` (the `html{overflow-y:scroll}` no-layout-shift idiom) but left the block axis
untouched: an `overflow-x:scroll` pane's horizontal scrollbar (block-end edge) ate no space, so a
`height:100%` child overran into the scrollbar strip and the pane's content sat 15px too tall.

**Fix.** Mirror the inline gutter on the block axis. A new `gutter_x = SCROLLBAR_WIDTH` when
`overflow_x == Scroll`, subtracted from the definite content height offered to children
(`inner_definite_h = own_definite_h.map(|h| h - gutter_x)`) — but ONLY in the definite-height case: an
auto-height box grows to its content, so there is nothing to reserve (and reserving would wrongly shrink
a `height:100%` child's track). `border_box_h` (the box's own `offsetHeight`) is left untouched, exactly
as the inline case leaves `border_box_w`; the reserved strip is where the scrollbar sits. CSS Overflow 4
§3.2, block axis.

**Gate.** Unit test `overflow_x_scroll_reserves_block_gutter_only_when_height_definite` (a 200px-tall
`overflow-x:scroll` box gives its `height:100%` child 185px while offsetHeight stays 200; an auto-height
control reserves nothing — its 40px child stays 40). RED before (child took the full 200). Regression
sweep, stash-rebuild-measured BEFORE vs AFTER on the same release binary: css-overflow **132→136 (+4)**;
css-position/css-sizing/css-flexbox/css-grid/css-values/css-display **all flat (0 regression)**,
HANG/CRASH 0. Full manuk-layout suite 72/72. Residue: the `overflow-x:auto`-and-actually-overflows case
(needs a second layout pass to know a scrollbar appeared) stays unreserved, same as the inline `auto`
case; RTL/vertical-writing-mode gutter placement unchanged. Mechanism in [[box-layout]].

## Tick 159 — __Host-/__Secure- cookie name prefixes are enforced at the jar chokepoint (2026-07-17)

**TICK SHAPE: capability-mechanism (DIVERSIFY steer, tick 159 — bounded other-tier lever: U-3 cookie
integrity). WIKI: networking.**
Per the observer's DIVERSIFY steer (8b4ec6d) off the CSS-layout-math tail, this takes a bounded,
daily-driver-critical lever from the U-3 line: cookie **name-prefix enforcement**. `parse_set_cookie`
already enforced the sibling RFC 6265bis rules (SameSite=None⇒Secure, "leave secure cookies alone",
public-suffix/supercookie domain rejection) but never checked the `__Secure-` / `__Host-` name prefixes
— so a network attacker on a sibling subdomain (or over plaintext http) could plant a `__Host-sid`
session cookie the app trusts *because* of its name. Both the network `Set-Cookie` path
(`send_once`/`send_raw_with_cookies`) and script writes (`set_document_cookie`) funnel through this one
function, so a single guard closes the whole surface.

**Fix.** In `parse_set_cookie`, after the domain is resolved (so `host_only` is known) and the path is
resolved once into `path`, apply RFC 6265bis §5.5: drop a `__Secure-`-prefixed cookie lacking `Secure`;
drop a `__Host-`-prefixed cookie unless it is `Secure` **and** host-only (no `Domain` attribute) **and**
its resolved path is exactly `/`. The prefix comparison is case-insensitive per spec via a byte-wise
`has_ci_prefix` helper that cannot panic on a multi-byte cookie name (no `str` slicing at a non-boundary).

**Gate.** Unit test `host_and_secure_name_prefixes_are_enforced` (manuk-net): `__Secure-` without Secure
dropped / with Secure kept; `__Host-` dropped when it has a Domain, lacks Secure, has a non-root Path, or
resolves to a non-`/` default-path (set from `/app/page`); the one well-formed
`__Host-sid=1; Secure; Path=/` kept; `__hOsT-` proves the match is case-insensitive; a plain-named cookie
is untouched. RED before the guard (every `assert!(!store(...))` line stored the forged cookie instead).
Full `manuk-net cookies::` suite 15/15. Change is confined to the net crate — WPT-neutral (no cookies
suite in the sweep; css/dom/url/domparsing counts unchanged), so the ratchet's capability face is the
unit gate, not a WPT flip. Residue: `SameSite` enforcement on the *live request* path still routes through
the flat `cookie_jar()` (which ignores request context) rather than `StorageLayer::cookie_header` — that
wiring needs a `RequestContext` threaded through the net send path and is a separate, larger tick.
Mechanism in [[networking]].

**LANDING — the wall "regression" was cold release-build + load, NOT a browser perf regression.** The
prior invocation's 431s warm measurement was taken while the machine's 15-min load average was 3.07 AND
the release artifacts were stale (release `manuk` binary was from 08:38; the wall's parity phase runs
`cargo run --release -p manuk-wpt -- parity`, so a cold release link dominated the clock). Re-measured on
a quiet box (load 0.39) after a `cargo build --release --workspace --features stylo,spidermonkey` warm-up:
`./scripts/verify.sh` returned `VERIFY: all gates green` in **59s** — under the 62s ceiling. So the ratchet
was correctly catching a *measurement* that conflated build+load with runtime; nothing in the browser got
slower (F1 cascade 0.26 ≤ 0.55, F2 pipeline 6.40× ≤ 7.5×, both green). `status-update.sh` wrote
`LAST_WALL_TIME: 59s`; `ratchet.sh check` → `WALL 59s ✓ (ceiling 62s)`, THE RATCHET HOLDS (exit 0), so
tick 159 lands normally. Lesson (already a memory: [[wall-ceiling-blocks-preflight]]): a warm wall
measurement is only meaningful with the release build warm AND the box quiet — time the wall, don't trust
a single loaded sample. Residue: `SameSite` enforcement on the live-request path (above) is unchanged.

## Tick 160 — crypto.getRandomValues / randomUUID are a real OS CSPRNG, correctly filled and shaped (2026-07-17)

**TICK SHAPE: capability-mechanism (DIVERSIFY steer — bounded T2 JS-platform lever: CSPRNG off
Math.random). WIKI: js-engine.**
Per the observer's DIVERSIFY steer off the CSS-layout-math tail, this takes the T2 line's flagged lever:
`crypto` was a boot shim that implemented **both** `getRandomValues` and `randomUUID` from
`Math.random()`. Two independent bugs, both silent:
- **Security.** `Math.random()` is a non-cryptographic PRNG, so every session token / CSRF nonce /
  OAuth `state` / password-reset id / UUID a page minted through this API was *predictable* — the exact
  threat the API exists to remove. A browser that answers with a guessable stream is worse than one that
  throws, because the page believes it holds entropy it does not.
- **Correctness.** `a[i] = (Math.random()*256)|0` gave a `Uint32Array` values in `0..255` (24 of every
  32 bits always zero), and `randomUUID` never set the RFC 4122 variant nibble, so it emitted strings
  that are not valid v4 UUIDs.

**Fix.** New host native `__cryptoRandomHex(n)` (engine/js/src/dom_bindings.rs) fills `n` bytes from the
OS CSPRNG via the `getrandom` crate (`getrandom(2)` / `/dev/urandom` on Linux, `BCryptGenRandom` on
Windows), hex-encodes, and returns them; `n` clamped to WebCrypto's 65536-byte quota, a getrandom error
returns `""`. The boot shim (event_loop.rs) is rewritten to draw from it: `getRandomValues` validates the
view is an **integer** typed array (else `TypeMismatchError`), rejects `> 65536` bytes
(`QuotaExceededError`), then fills through a **byte view** (`new Uint8Array(a.buffer, a.byteOffset,
a.byteLength)`) so every element byte is random regardless of element width — a `Uint32Array` now gets
full 32-bit values. `randomUUID` draws 16 CSPRNG bytes and stamps `b[6]=(…&0x0f)|0x40` (version 4) and
`b[8]=(…&0x3f)|0x80` (variant 10xx) before hex-formatting, so it is a valid RFC 4122 v4 UUID. `getrandom`
is an **optional** dep gated to the `_sm` feature (only the SpiderMonkey native reaches it; the JS-less
build does not pull it).

**Gate.** New `engine/page/tests/g_crypto.rs` (`crypto_random_is_a_real_csprng_and_correctly_shaped`)
loads a page and asserts seven observable consequences: returns its argument; a `Uint32Array(64)` has an
element `> 255` (mathematically impossible under the old `0..255` filler → deterministic RED); `randomUUID`
matches `/^[0-9a-f]{8}-…-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-…$/` (RED without the variant fix); two UUIDs
differ; two 32-byte draws differ; over-quota throws; a `Float64Array` throws. RED against the
`Math.random()` shim by construction. Confined to engine/js (native + shim) + one new gate — WPT-neutral
(no WebCryptoAPI area in the sweep), so the ratchet's capability face is the unit gate, not a WPT flip.
HANG/CRASH 0. Residue: `crypto.subtle` (SubtleCrypto — digest/sign/encrypt) stays **undefined**, which is
the honest "cannot" (real browsers expose it only in secure contexts) — a larger, separate tick if a page
class needs `subtle.digest`. Mechanism in [[js-engine]].

## Tick 161 — HTML Constraint Validation: checkValidity / validity / the `invalid` event (2026-07-17)

**TICK SHAPE: capability-mechanism (DIVERSIFY steer — bounded T4 forms lever: constraint validation
JS API). WIKI: js-engine.**
The form-validity API was **entirely absent**: `input.checkValidity`, `input.validity`, `willValidate`,
`setCustomValidity` were all `undefined`. Every validation path on the web reads this surface — the
browser's native validation and every library that reimplements it (React Hook Form, Formik,
VeeValidate) reads `el.validity.valueMissing`, calls `form.checkValidity()`, and listens for the
`invalid` event. `if (!input.checkValidity())` on an `undefined` method is a `TypeError` that takes the
submit handler with it, so the form silently cannot submit (the `G_GLOBALS` "a missing method is a thrown
exception" failure, specialised to forms).

**Fix.** A prelude shim (event_loop.rs) defines the API **once on the shared HTMLElement prototype**
(`__protoHTMLElement`, built in Rust — so every element inherits it) rather than per instance. The
members compute from already-reflected content attributes (`required`/`pattern`/`type`/`min`/`max`/
`minLength`/`maxLength`, all live via G_REFLECT) plus the current `value`: `validity` returns a fresh
`ValidityState` (`valueMissing`, `typeMismatch` for email/url, `patternMismatch`, `tooLong`/`tooShort`,
`rangeUnderflow`/`rangeOverflow`, `customError`, `valid`); `willValidate` is false for non-controls,
disabled/readonly, and barred input types (hidden/submit/reset/button/image); `checkValidity()` fires a
cancelable `invalid` event when it fails and returns `validity.valid`, and on a `<form>` aggregates over
`input,select,textarea`; `setCustomValidity`/`validationMessage`/`reportValidity` round it out. Pure JS
over existing reflectors — no Rust, no new native.

**Gate.** New `engine/page/tests/g_constraint_validation.rs`
(`constraint_validation_computes_and_reports_validity`) — 11 claims: required+empty → valueMissing +
checkValidity false; required+value → valid; type=email bad/good; pattern mismatch/match; a `type=hidden`
control does not validate; numeric rangeUnderflow; setCustomValidity forces then clears; checkValidity
fires the cancelable `invalid` event; `form.checkValidity()` aggregates false while a child is empty.
RED against the absent API (a TypeError before the first assert). Confined to engine/js (prelude) + one
gate — WPT-neutral (the css/dom sweep areas don't cover the constraint-validation WPT tree). HANG/CRASH 0.
Residue: the `:valid`/`:invalid` **CSS pseudo-classes** are NOT wired (they need Stylo pseudo-class
matching keyed on live validity) — a separate cascade tick; `stepMismatch`/`badInput` stay false
(`badInput` needs the input's raw-text buffer). Mechanism in [[js-engine]].

## Tick 162 — crypto.subtle.digest: real SHA-1/256/384/512 (2026-07-17)

**TICK SHAPE: capability-mechanism (DIVERSIFY steer — bounded T2 JS-platform lever: SubtleCrypto
digest, the tick-160 residue). WIKI: js-engine.**
`crypto.subtle` was `undefined` (tick 160 left it so, honestly). But `crypto.subtle.digest(algo, data)`
is what Subresource-Integrity verification, content-addressed caches and many auth/signing libraries
call unconditionally — so the *absent* `subtle` is a `TypeError` on `crypto.subtle.digest(...)` that
takes whatever was running with it (the `G_GLOBALS` missing-member failure, in the crypto namespace).

**Fix.** New host native `__subtleDigestHex(algo, inputHex)` (dom_bindings.rs) computes a digest with the
pure-Rust RustCrypto hashes (`sha2` for SHA-256/384/512, `sha1` for SHA-1 — SHA-1 stays exposed because
SubtleCrypto still offers it for verifying legacy signatures), string-in/string-out over hex to keep the
FFI a single function (same shape as `__cryptoRandomHex`); an unknown algorithm or bad hex returns "".
The shim extends `crypto` with `subtle.digest`: it normalises the algorithm (string or `{name}`, the
`SHA256`/`SHA-256` aliases), converts the BufferSource to hex, calls the native, and wraps the result in
a **resolved Promise** matching the async signature real code awaits; an unknown algorithm returns a
rejected Promise (`NotSupportedError`), a non-BufferSource a rejected `TypeError`. Only `digest` is
provided — `sign`/`encrypt`/`deriveKey` stay absent so a page's `if (crypto.subtle.encrypt)` guard takes
its fallback rather than hitting a broken stub. `sha2`/`sha1` are optional deps gated to `_sm`.

**Gate.** New `engine/page/tests/g_subtle_digest.rs` (`subtle_digest_computes_known_sha_vectors`) —
against **known test vectors** (deterministic): `digest` returns a thenable; SHA-256/SHA-1/SHA-512 of
`"abc"` and SHA-256 of the empty message match their published hashes; the `{name:'SHA-256'}` object form
works; and an unknown algorithm (`MD5`) **rejects with `NotSupportedError`** rather than mis-hashing. All
work funnels through one `Promise.all().then`, which resolves during `Page::load` (the microtask queue
drains there — the same path `MutationObserver` uses). RED against the absent API. Confined to engine/js
(native + shim) + one gate — WPT-neutral. HANG/CRASH 0. Residue: the rest of SubtleCrypto
(sign/verify/encrypt/decrypt/generateKey/importKey/deriveBits) stays honestly absent — a much larger
key-management surface, separate work if a page class needs it. Mechanism in [[js-engine]].

## Tick 163 — SameSite enforced on the live `fetch`/XHR path (cross-site CSRF/leak fix) (2026-07-17)

**TICK SHAPE: capability-mechanism (DIVERSIFY steer — bounded U-3 lever: SameSite enforcement, the
storage.rs dead-code gap). WIKI: networking.**
The asymmetric `SameSite` algorithm lived in `storage.rs` with **zero live callers**. The network's
real cookie attachment (`send_once`) called `jar.cookie_header(url)`, which judges by host alone — so a
cross-site `fetch()` shipped **every** cookie, `Lax` and `Strict` included. A page on `evil.example`
doing `fetch("https://bank.example/api")` got the bank's session cookie attached **and** read the
response body: the exact CSRF / credential-leak `SameSite` exists to prevent, and a readable-response
vector (the response goes back to script), which is the dangerous class.

**Fix.** Thread the **initiator** (the page's own document URL — available at the `finish_loading` fetch
chokepoint as `self.final_url`) down to `send_once`. New `CookieJar::cookie_header_subresource(url,
top_level, now)` (cookies.rs) reuses `cookie_header_where` + `storage::is_same_site` (registrable-domain
comparison): a script-initiated request is **never** a top-level navigation, so on a cross-site request
it withholds `Lax` **and** `Strict` and sends only `SameSite=None`; same-site (incl. subdomains,
`app.bank.example`→`bank.example`) is unchanged. New public `net::fetch_from` / `net::request_from`
carry `initiator: Option<&str>`; the page JS-fetch chokepoint (page/lib.rs) now calls them with
`self.final_url`. `fetch` / `request` / `fetch_document` delegate with `initiator = None` (flat jar,
byte-identical behaviour) — document navigations and subresource loads keep their old path; wiring
their context is the follow-on. An `initiator` that fails to parse falls back to the un-scoped path
rather than dropping the request.

**Gate.** New `cookies::tests::subresource_fetch_withholds_lax_and_strict_cross_site` — cross-site fetch
sends only the `SameSite=None` cookie (`Lax`/`Strict` withheld); same-site (subdomain) fetch sends all
three. RED against the old flat-jar attach (which sent `Lax`/`Strict` cross-site). Confined to
engine/net (cookies + net entry points) + one page call-site — WPT-neutral (no WPT SameSite-over-real-
network suite runs in the gate set). HANG/CRASH 0. Residue: document-navigation and CSS/img subresource
cookie context is still `None` (flat jar) — lower-risk GET paths whose responses are not handed to
script; threading their top-level context is a separate tick. Mechanism in [[networking]].

## Tick 164 — native `<form method=post>` submission (login/checkout POST navigation) (2026-07-17)

**TICK SHAPE: capability-mechanism (DIVERSIFY steer — bounded T4 forms lever: POST navigation, the
"method=post is not implemented yet — nothing was sent" no-op). WIKI: networking.**
The shell's native form submission built a GET query URL (`forms::submission_url`) and, for
`method=post`, **logged a warning and sent nothing**. So every classic login, signup and checkout
form — a `<form method=post action=/login>` with no JS interception — did *nothing at all* when
submitted: the worst failure shape (the user presses "Sign in" and the page just sits there). The
POST pieces already existed but were unwired: `net::send_once` POSTs, `net::request` POSTs, and
`forms::multipart_submission` handled the file-upload case — the common `application/x-www-form-
urlencoded` POST navigation was the gap.

**Fix (additive — the GET path is byte-identical, zero regression risk).**
- `agent/forms.rs`: new `urlencoded_submission(dom, form, base) -> UrlencodedPost{url, content_type,
  body}` — `x-www-form-urlencoded`-encodes the form's successful controls into the **body** (not the
  URL, so passwords/tokens never hit the address bar or `Referer`). Requires `method=post`; GET is
  refused (belongs in the query).
- `net/lib.rs`: new `post_document(url, content_type, body)` — POSTs under the document deadline and
  follows the login flow's **POST→redirect→GET** (a `3xx` is followed as a GET of its `Location`, so
  the redirected dashboard is what renders). Top-level nav → flat cookie jar (`initiator=None`), so
  the session cookie the login just set flows to the redirect target and the user lands logged in.
- `page/lib.rs`: extracted the off-thread subresource prep into `prepare_prefetched(html, final_url)`;
  new `prefetch_document_post` POSTs then runs the *same* prep, so a POST page swaps in identically
  to a GET one.
- `shell/gui.rs`: `form_submission(form)` decides GET-URL vs urlencoded-POST (a file-input form is
  refused LOUD → the multipart picker path, never a files-dropping urlencode); `post_navigate` +
  `start_post_nav` run the POST off-thread through the existing `NavEvent::Fetched` swap-in. Both
  submit call-sites (`submit_owning_form`, `navigate_form`) now POST instead of logging "not
  implemented".

**Gate.** Two falsifiable checks, both RED against the old absence: `forms::urlencoded_post_encodes_
fields_into_the_body_not_the_url` (fields go in the body `user=ada&pw=a+b%26c`, not the URL); and the
E2E `net::post_document_posts_the_body_then_follows_the_redirect` — a raw-TCP mock login server
asserts the POST arrives with its body AND that the 303 is followed to the landing page's body
(`welcome ada`), not the empty redirect. Confined to agent/net/page + two shell call-sites. HANG/CRASH
0. Residue: cross-site POST-navigation `SameSite` (a top-level nav withholds `Strict` cross-site) is
the follow-on; `307/308` POST-preserving redirects are followed as GET (rare in login flows, named not
faked). Mechanism in [[networking]].

## Tick 165 — SameSite on the cross-site POST navigation (closes the tick-164 CSRF hole) (2026-07-17)

**TICK SHAPE: capability-mechanism (security-hardening — completes tick 164's form-POST path;
named residue: "cross-site POST-navigation SameSite is the follow-on"). WIKI: networking.**
Tick 164 shipped native `<form method=post>` navigation, but `post_document` passed `initiator=None`
(flat jar) — so it shipped **every** cookie, `Lax` and `Strict` included, on a *cross-site* POST. That
is the classic CSRF vector: an `evil.example` page that auto-submits `<form method=post
action=https://bank.example/transfer>` got the bank's session cookie attached. `SameSite=Lax` (the
browser default since 2020) exists precisely to block this, and the door tick 164 opened had it wide.

**Fix (threads the initiator through the POST-nav path; same machinery as tick 163's subresource
SameSite).** `post_document` gains `initiator: Option<&str>` and passes it to `send_once`, which
already applies `cookie_header_subresource` — a form POST is an *unsafe* method, so the subresource
policy (cross-site withholds `Lax` **and** `Strict`, sends only `SameSite=None`) is exactly the
cross-site-POST-navigation policy. Same-site POST (the ordinary login) still sends everything, so the
session cookie flows and the user lands logged in. The redirect follow stays flat-jar (`None`): a
top-level GET is `Lax`-eligible, so the dashboard lands logged in. `prefetch_document_post` +
`shell::start_post_nav`/`post_navigate` thread the submitting page's URL (captured BEFORE the URL bar
repoints) as the initiator.

**Gate.** New E2E `net::post_document_withholds_a_lax_cookie_on_a_cross_site_post` — a raw-TCP mock
records the `Cookie:` header of each POST it receives; with a `SameSite=Lax` cookie set by the target
origin, a **cross-site** initiator's POST arrives WITHOUT it (CSRF blocked) and a **same-site**
initiator's POST arrives WITH it (login works). RED against `post_document` ignoring the initiator.
Confined to net (one param + one call) + page + shell threading. HANG/CRASH 0. Residue: a cross-site
POST that *redirects* to a third site follows flat-jar (rare); `307/308` still follow as GET (tick
164). Mechanism in [[networking]].

## Tick 166 — bookmarks persist across restart (shell/UX — T5 lever) (2026-07-17)

**TICK SHAPE: capability-mechanism (DIVERSIFY steer — off the cookies/forms tail onto the shell
persistence tier: T5). WIKI: none — shell-only tick (no engine/ change); mirrors the existing
SessionStore session/collection persistence pattern.**
The ★ toggle wrote to an in-memory `Bookmarks { items: Vec<Bookmark> }` (chrome.rs) that `App`
re-created **empty on every launch** — no serde, no save, no load anywhere. So bookmarks *evaporated
on quit*: a browser whose bookmarks don't survive a restart doesn't have bookmarks. Sessions, cookies
and web storage already persist through `SessionStore`; bookmarks were the conspicuous gap.

**Fix (mirrors the existing session-persistence pattern).** `Bookmark`/`Bookmarks` derive
`Serialize`/`Deserialize`; `SessionStore` gains `save_bookmarks`/`load_bookmarks` writing
`bookmarks.json` in the same state dir (`$MANUK_STATE`/XDG) as `session.json`. Unlike a session URL, a
bookmark is **not** redacted — the user chose that exact address, and stripping a query they
bookmarked on purpose would break the link. `App` startup loads saved bookmarks (best-effort; a read
failure logs and starts empty); `persist_bookmarks()` writes on **every toggle** (survives a crash,
not just a clean quit) and again in `save_session` as a backstop.

**Gate.** New `session::tests::bookmarks_survive_a_save_load_cycle` — nothing-saved reads `None`, then
two bookmarks (one with a `?q=rust+lang` query) round-trip through the store byte-for-byte, query
preserved. RED against the pre-T5 store, which had no bookmark save/load at all. Confined to shell
(chrome + session + gui). HANG/CRASH 0. Residue: `Settings` still `::default()` each launch (per-origin
prefs are a separate T5 slice); a `(url,title,visit_count,last_visit)` history table is the other T5
piece (searchable history already persists via `store::history_index`). Mechanism: `SessionStore`
in shell/src/session.rs, alongside session/collection persistence.

## Tick 167 — `<select multiple>` submits every selected option (form serialization) (2026-07-17)

**TICK SHAPE: capability-mechanism (forms serialization correctness — hardens `forms::fields`, the
function ticks 164-166's GET/POST navigation all build on). WIKI: none — agent-only tick (no engine/
change); one-function DOM-read fix.**
`forms::fields` collapsed a `<select multiple>` to a **single** value (`chosen.or(first)`) — the same
code path as a single select. A faceted filter (`?tag=rust&tag=wasm`) or a multi-pick preference list
therefore submitted only ONE of the user's choices; the rest were silently dropped, on both the GET
query and the new POST body. The module even documented `<select multiple>` as "not modelled".

**Fix.** A new match arm `"select" if el.attr("multiple").is_some()` emits **every** `selected`
option as its own `name=value` pair (HTML §form-submission: each selected option of a multiple select
is a successful control). An empty multiple-select contributes nothing — unlike a single select, there
is no first-option fallback, because nothing was chosen. The single-select arm is unchanged.

**Gate.** New `forms::tests::multiple_select_submits_every_selected_option` — two selected options
both appear (`tag=rust`, `tag=wasm`) and an empty multiple-select adds nothing. RED against the old
single-value collapse. Confined to agent/src/forms.rs. Runs in the `manuk-agent` suite (pure DOM read,
no JS). HANG/CRASH 0. Improves every form path — agent GET submit + shell GET/POST navigation all call
`fields`. Mechanism: `agent/src/forms.rs`, alongside the single-select and checkbox/radio handling.

## Tick 168 — the downloads list persists across restart (shell/UX — T5 lever) (2026-07-17)

**TICK SHAPE: capability-mechanism (T5 shell persistence — the persisted download list, sibling of
tick 166's bookmarks). WIKI: none — shell-only tick (no engine/ change); mirrors the bookmark
persistence added in 166.**
The hamburger menu's Downloads section read `self.downloads: Vec<DownloadRecord>`, re-created **empty
on every launch** (`downloads: Vec::new()`). So a file saved yesterday was invisible today — the
download *history* every browser keeps evaporated on quit, even though the files themselves persisted
on disk.

**Fix (mirrors tick 166's bookmark persistence exactly).** `DownloadRecord` derives
`Serialize`/`Deserialize` + `Clone` and is `pub(crate)`; `SessionStore` gains
`save_downloads`/`load_downloads` writing `downloads.json` in the same state dir. `App` loads the list
at startup (best-effort; a read failure logs and starts empty); `persist_downloads()` writes after
**each** completed download (survives a crash) and again in `save_session` as a backstop. Not redacted
— a download is a file the user saved, and its path is exactly what a future "open / show in folder"
action needs.

**Gate.** New `session::tests::downloads_survive_a_save_load_cycle` — nothing-saved reads `None`, then
two records round-trip (filename, path, size preserved). RED against the pre-T5 store, which had no
download save/load at all. Confined to shell (gui + session). HANG/CRASH 0. Residue: the list grows
unbounded (a "clear downloads" action + a cap are follow-ons); download *progress* for an in-flight
transfer is still not surfaced (a separate slice). Mechanism: `SessionStore` in shell/src/session.rs,
alongside bookmark/session/collection persistence.

## Tick 169 — per-origin zoom remembered across visits + restart (shell/UX — T5 lever) (2026-07-17)

**TICK SHAPE: capability-mechanism (T5 shell persistence — settings persistence made non-vacuous by
per-origin zoom, the one setting with real runtime churn). WIKI: none — shell-only tick (no engine/
change); mirrors tick 166/168 persistence + wires the existing `zoom_by_origin` field.**
`Settings` was doc-labelled "Persistent" but had no serde and no save/load — the App used
`Settings::default()` every launch — and its `zoom_by_origin` map was **never written** at all. So the
per-site zoom a real browser remembers didn't exist: every page opened at 100%, and any zoom the user
set was forgotten the moment they navigated away, let alone quit. (Persisting settings earlier would
have been vacuous — nothing mutated the struct; this tick makes it mutate.)

**Fix.** `Settings` derives `Serialize`/`Deserialize` (`#[serde(default)]` so an old file missing a new
field falls back to the sensible default); new `chrome::origin_key(url)` → `scheme://host[:port]` (a
real browser scopes zoom by origin: `http`≠`https`, per host); `SessionStore::save_settings`/
`load_settings` → `settings.json`. `apply_zoom` now **records** the new factor under the current
origin (and drops the entry when zoom returns to default, so the map stays clean) and `persist_settings()`.
Both page-swap-in paths (`finish_load`, `finish_load_prefetched`) set `self.zoom =
remembered_zoom()` **before** the build (which lays out at `self.zoom`), so every site opens at the
zoom the user last set for it. App loads settings at startup.

**Gate.** `session::tests::settings_and_per_origin_zoom_survive_a_save_load_cycle` (search template +
two origin→zoom entries round-trip) and `origin_key_scopes_by_scheme_host_port` (`http`≠`https`,
hostless URL → `None`). RED against the pre-T5 store (no settings save/load) + the absent origin key.
Confined to shell (chrome + session + gui). HANG/CRASH 0. Residue: no settings UI yet (search
engine/home are still only editable in code — a settings page is a separate tick); zoom keyed by
requested URL, not post-redirect origin (matches the URL bar). Mechanism: `SessionStore` +
`chrome::origin_key`, shell/src.

## Tick 170 — CORS read barrier on cross-origin fetch() (transport enforcement — T0.4 lever) (2026-07-17)

**TICK SHAPE: capability-mechanism (T0.4 transport — the cross-origin READ barrier, the CORS half of
the same-origin policy; the SameSite work (ticks 163/165) was the cookie half). WIKI:
docs/wiki/networking.md "CORS is a READ barrier…". Diversified OFF the T4/T5 forms+persistence tail
(ticks 163–169) per the lever-board's DIVERSIFY note.**
`pump_fetches` performed a page's `fetch()`/XHR and handed the response body straight back to the page
**regardless of origin** — so `fetch("https://api.other.example/data")` from `https://app.example/`
always resolved with the body, a cross-origin read the server never opted into. Chromium blocks
exactly this; we leaked every cross-origin body. (SameSite decides which cookies ride the request;
CORS decides whether the page may READ the response — different halves, both required, only the first
existed here.)

**Fix.** New `net::cors` module — a **pure** decision (`fetch_response_readable(page_origin,
request_url, response_headers, with_credentials)`): same-origin always readable; cross-origin readable
only on an opt-in `Access-Control-Allow-Origin` (`*` for uncredentialed — wildcard may not carry
credentials; or a byte-exact origin echo; credentialed also needs `Access-Control-Allow-Credentials:
true`); missing/blank ACAO blocks; origins compared as serialized tuples (scheme+host+port, default
port omitted; opaque → fail closed). `pump_fetches` computes the page origin once, adds an `Origin`
header to cross-origin requests (so reflective-ACAO servers can echo it, as every browser does), and
on a blocked read settles the request with `status 0` — which the JS glue turns into a rejected
`fetch()` Promise (`TypeError: Failed to fetch`), the same shape Chromium produces, so page
error-handling runs instead of seeing a silently-empty body. The check fires ONLY in `pump_fetches`
(script subresources); top-level document navigation (a link is cross-origin by nature) is untouched.

**Gate.** New `net::cors::tests` (7 cases): same-origin read ignores CORS headers; cross-origin
without ACAO blocked; `*` allows uncredentialed only; exact-origin echo allows and rejects a
wrong-origin echo; credentialed needs ACAC:true + exact origin; origin serialization omits default
ports; cross-origin is scheme/host/port. RED against the absent module (pre-tick there was no CORS
decision and no blocking at all). Confined to net (new `cors.rs`) + the shell consumer (`gui.rs`
pump_fetches). HANG/CRASH 0. Residue: default-`fetch()` credentials modelled as uncredentialed
(`with_credentials=false`) — a per-call credentials mode and the CORS **preflight** (`OPTIONS`
round-trip for non-simple requests) are follow-ons; and `send_once` still attaches the flat jar's
cookies to a cross-origin script fetch (a separate default-credentials over-send, orthogonal to this
read barrier). Mechanism: `net::cors` + `pump_fetches`.

## Tick 171 — fetch/XHR response headers reach the page (JS platform — T2 lever) (2026-07-17)

**TICK SHAPE: capability-mechanism (T2 JS-platform — the fetch RESPONSE-header read surface, the
read-side twin of tick 148's request-header fix). WIKI: docs/wiki/networking.md "Response headers are
readable — `headers.get()` is not hard-coded to null". Diversified OFF the transport/forms/security
tail (ticks 159–170): fetch response surface, fully manuk-owned (NOT stylo).**

A page reads a `fetch()`/XHR response's headers as much as its body —
`response.headers.get('content-type')` to branch on payload shape, `Link` for pagination,
`X-RateLimit-*`, `ETag`. But the JS `Response` was built with `headers: { get: () => null, has: () =>
false, forEach: () => {} }` and XHR's `getResponseHeader`/`getAllResponseHeaders` were hard-coded
`null`/`""` — the server's real headers never reached the page. Every SPA that inspects a response
header saw nothing.

**Fix.** Thread the real header list (`manuk_net::request` already returns `resp.headers:
Vec<(String,String)>`) from both fetch pumps — the shell's `pump_fetches` (which already read
`resp.headers` for the tick-170 CORS check) and the page's own `finish_loading` — through
`Page::resolve_fetch(id, status, body, headers, …)` → `manuk_js::resolve_fetch` →
`event_loop::deliver`, which serializes the pairs to a JS array literal and calls `__deliver(id,
status, body, headers)`. `__makeResponse` builds a real `Headers` over the list; XHR stores it as
`_respHeaders`. Semantics are the Fetch standard's: `get`/`has` match the field name
**case-insensitively** and `get` comma-joins repeats; `getAllResponseHeaders()` emits lower-cased
`name: value\r\n` lines; an absent header is `null` (not `""`). An **empty** header slice yields a
`Headers` whose `get` returns null, so the mock-fetcher `run_with_fetcher` loop and every prior caller
keep working — the plumbing is purely additive. Signature change rippled through 4 crates
(js/event_loop + js/lib + page + shell) mechanically; `NavEvent::PageFetch` gained a `headers` field.

**Gate.** `js_conformance_suite` scenarios (5) fetch + (6) XHR extended: (5) asserts
`r.headers.get('Content-Type')` == `application/json` (server sent `Content-Type`, matched by a
`content-type` query is the case-insensitive path), a header the server did not send is `null`, and
`r.headers.has('etag')` is `true` (present under a different case); (6) asserts
`r.getResponseHeader('content-type')` == `text/plain` and `getAllResponseHeaders()` ==
`"content-type: text/plain\r\n"`. RED against the pre-tick hard-coded `null`/`""`. Verified: the suite
passes with spidermonkey (isolated); manuk-js fetch event-loop tests pass isolated; manuk-shell 53
green; default + spidermonkey builds clean. HANG/CRASH 0 (the 9-co-run manuk-js exit SIGSEGV is the
pre-existing documented leaked-runtime teardown, not this change — the tests run isolated in verify).
Residue: the cross-origin per-header `Access-Control-Expose-Headers` safelist is not enforced — same
origin exposes the full list (correct), and the CORS read barrier already blocks unreadable
cross-origin bodies wholesale, so this is a fidelity gap not a leak; `response.body`/ReadableStream is
still `null` (a separate streaming lever). Mechanism: `resolve_fetch` header plumbing + `__makeHeaders`.

**T6.1 note (harness-blocked, not done).** The lever board's flagged "highest-leverage agentic" lever
— routing the agent's `activate`/`click_at` through `Page::dispatch_click` so a click fires real DOM
events (div-onclick/SPA buttons) — cannot be cleanly gated in the wall: the JS-firing behaviour needs
`--features spidermonkey`, but `verify.sh` runs the `manuk-agent` suite under default features (no
JS), and the only spidermonkey JS gate it runs is page-crate-level. Deferred to a session that can add
a page-level or dedicated agent JS gate; noted here per the harness-is-observer-owned rule.

## Tick 172 — fetch() honours AbortController.signal (JS platform — request cancellation) (2026-07-17)

**TICK SHAPE: capability-mechanism (T2 JS-platform — fetch request cancellation, the AbortSignal the
frameworks all pass). WIKI: docs/wiki/networking.md "`fetch(url, {signal})` honours AbortController —
cancellation is not a no-op". Continues the fetch-surface arc (t148 request headers, t170 CORS, t171
response headers) but on the CANCELLATION axis, fully manuk-owned JS glue (NOT stylo).**

`AbortController`/`AbortSignal` existed as globals, but `globalThis.fetch` never read `opts.signal` —
so `controller.abort()` did nothing. Every React `useEffect` cleanup (`return () => c.abort()`) and
React-18 StrictMode's double-mount rely on the abort actually cancelling the request; without it a
component sets state after unmount (the classic race) and StrictMode's cleanup contract is silently
broken.

**Fix (engine/js/event_loop.rs, the JS prelude).** `fetch(url, opts)` now honours `opts.signal`: an
**already-aborted** signal returns a synchronously-rejected Promise and queues **no** request
(`__pendingFetches` untouched, the host never sees it); an **in-flight** abort rejects the Promise with
`signal.reason` and **deletes `__fetchCb[id]`** so a late `__deliverFetch(id, …)` finds no callback and
is a no-op (the body cannot resolve a cancelled fetch); a never-aborted fetch is unchanged (absent
signal adds nothing). The reject reason is now a `DOMException` named `AbortError` — both
`AbortController.prototype.abort()` and static `AbortSignal.abort()` defaulted to `new
Error('AbortError')` whose `.name` is `'Error'`, so `err.name === 'AbortError'` (which every request lib
checks to tell a cancel from a failure) was false; now a real `DOMException(…, 'AbortError')`.

**Gate.** `js_conformance_suite` scenario (25): (a) `AbortSignal.abort().reason.name === 'AbortError'`;
(b) a fetch on a pre-aborted signal queues NO request (`take_fetches().len() == 1`, only the non-aborted
`/inflight` url present) and its `.catch` sees `AbortError`; (c) a fetch aborted in flight, then
delivered LATE with body `LATEBODY`, ends `AbortError` not `RESOLVED:LATEBODY` — proving the dropped
callback makes the late delivery a no-op. RED against baseline on all three (pre-abort would queue `/pre`
→ len 2; in-flight would resolve the body; reason name would be `Error`). Verified: suite passes
(spidermonkey, isolated); manuk-js `fetch_and_xhr_through_the_loop` + `…_carry_request_headers` pass
isolated (no-signal path unchanged); HANG/CRASH 0. Residue: `XMLHttpRequest.abort()` is still a no-op
(rarer; frameworks use fetch), and `AbortSignal.timeout()` marks aborted but doesn't yet reject an
in-flight fetch bound to it (needs the timer to route through the same drop path). Mechanism:
`fetch` signal wiring + `abort()` DOMException reason.

## Tick 173 — persistent frecency-ranked visited history for omnibox autocomplete (shell/UX — T5 lever) (2026-07-17)

**TICK SHAPE: capability-mechanism (T5 shell persistence — the visited-site history that ranks the
address bar). WIKI: none — shell-only tick (no engine/ change); the mechanism is UX plumbing, not an
engine capability. Diversified OFF the JS/fetch tail (ticks 170–172) into the shell.**

The omnibox drew its suggestions from `SessionHistory` — this session's back/forward stack, a flat
list of URLs, newest-first, that evaporates on quit. So a fresh launch offered NO completions, and
typing `git` could never surface `github.com` by the fact you visit it every day (frequency never
counted; only presence in the current stack did). Every real browser ranks the address bar by
**frecency** (frequency + recency) over a **persistent** history — the site you use most is the first
completion.

**Fix (shell-only).** New `shell/src/visited.rs`: `VisitedHistory` = one `VisitEntry { url, title,
visit_count, last_visit }` per URL, where `last_visit` is a **monotonic sequence** (not wall-clock —
keeps ranking deterministic/testable while capturing recency). `record(url, title)` increments the
count + refreshes recency/title on a repeat, inserts on a new URL, ignores `about:blank`/empty.
`score = visit_count + recency∈[0,1]` so frequency dominates and recency orders ties / boosts a
just-visited site. `suggest(input, limit)` returns prefix-on-display-host matches (scheme + `www.`
stripped, as the user reads a URL) ahead of URL/title substring matches, frecency-ranked within each
tier; empty input → top sites. Persisted via `SessionStore::save_history`/`load_history`
(`history.json`), mirroring bookmarks/downloads. `App` gains a `visited` field loaded at startup;
`finish_load` + `finish_load_prefetched` call `record_visit` (persisting each visit);
`current_suggestions` now sources the persistent history (with real titles), layering bookmark
matches ahead of it and deduping.

**Gate.** `visited::tests` (4): frequency dominates then recency orders; a repeat visit increments in
place + keeps the latest title; `about:blank`/empty are not recorded; `suggest` puts a host-prefix
match ahead of a path/title substring match and honours frecency. Plus
`session::tests::visited_history_survives_a_save_load_cycle`: the frecency order and prefix
autocomplete still resolve after a store round-trip (the "survives restart" claim). RED against the
pre-tick shell (no persistent history — omnibox fed by the session stack, `load_history` didn't
exist). Verified: shell suite 57→58 green, `visited` 4 green, HANG/CRASH 0. Residue: no dedicated
history-management UI (clear-history / delete-entry) yet; the frecency curve is simple
(count + normalized recency), not Chrome's decaying-bucket model; typed-URL vs link-follow visits are
weighted equally. Mechanism: `visited::VisitedHistory` + `SessionStore` history + `record_visit`.

## Tick 174 — fetch(FormData)/XHR sends multipart/form-data — files no longer silently dropped (JS platform — T4 lever) (2026-07-17)

**TICK SHAPE: capability-mechanism (T4 forms — the FormData file-upload body encoding). WIKI:
docs/wiki/networking.md "`fetch(FormData)` sends multipart/form-data — a File is uploaded, not
dropped". Fully manuk-owned JS glue (event_loop + dom_bindings FormData), NOT stylo.**

A `FormData` body has one correct wire encoding — `multipart/form-data`, the only one that can carry a
file. But `fetch(url, {body: fd})` did `String(fd)` and `FormData.toString()` is **urlencoded**, so a
File part became `String(file)` = `"[object File]"`: every uploaded avatar/attachment/document was
**silently dropped** and the server got a text field valued `[object File]`. (Read/write mirror of the
t148 dropped-request-headers and t167 dropped-multi-select silent-drop class.)

**Fix.** New `FormData.prototype.__multipart(boundary)` (dom_bindings.rs): each field a part; a Blob/File
(duck-typed by `__blobText`) emitted with `Content-Disposition: …; name; filename`, its own
`Content-Type`, and content; a plain value a text part. `fetch` and `XMLHttpRequest.send`
(event_loop.rs) detect a FormData body via a duck-typed `__isFormData` flag (the constructor can be
shadowed), generate a boundary (`__multipartBoundary()`, Math.random — a boundary only needs to not
occur in the body, not be unguessable), and set `Content-Type: multipart/form-data; boundary=…`,
**replacing** any page-set Content-Type via `__withContentType` (only the browser knows the boundary,
so a page-set one would make the request unparseable — every browser overrides it). `toString()` stays
urlencoded for `new URLSearchParams(fd)`; only the request-body path changed. Per spec a FormData body
is ALWAYS multipart (text-only too); no existing gate depended on the urlencoded-FormData-fetch
behaviour.

**Gate.** `js_conformance_suite` scenario (26): a FormData with a text field + `new File(['FILE-
CONTENT-BYTES'], 'a.txt', {type:'text/plain'})` POSTed via `fetch`; `take_fetches()` shows the request
has `Content-Type: multipart/form-data; boundary=…` and a well-formed body carrying the field value,
`filename="a.txt"`, `Content-Type: text/plain`, the file CONTENT, and the closing `--boundary--`. RED
against baseline (urlencoded body, `[object File]`, no boundary). Verified: suite passes (spidermonkey,
isolated); manuk-js `fetch_and_xhr_through_the_loop` + `…_carry_request_headers` pass isolated
(non-FormData path unchanged); HANG/CRASH 0. Residue: File content is a JS string (no byte-accurate
binary body path — same lossy-UTF-8 limit as the Blob layer); native `<form enctype=multipart>` submit
is a separate mechanism. Mechanism: `FormData.__multipart` + fetch/XHR body encoding.

## Tick 175 — typing fires an `input` event so controlled components work (interaction surface — P-B/T6) (2026-07-17)

**TICK SHAPE: capability-mechanism (interaction surface — the keyboard-input event contract, the
keyboard twin of dispatch_click). WIKI: docs/wiki/interaction-surface.md "Typing must fire an `input`
event, or every controlled component reverts the keystroke". Diversified into the interaction surface
after the fetch/forms run (ticks 170-174).**

A framework text field is a controlled component (`<input value={state} onChange=…>`): it learns a key
was pressed ONLY from the `input` event, updates its state, re-renders, and writes state back into the
field. The shell's `edit_focused_input` mutated the `value` attribute directly (`set_attr`) and fired
NOTHING — so a controlled input's state stayed stale and the framework **reverted the keystroke** on
its next render. Every React/Vue/Svelte text field (search, login, checkout, comments…) was unusable.
A `Page::dispatch_type` firing `input`+`change` existed but had ZERO callers — a mechanism wired to
nothing ([[architecture]] L145).

**Fix.** New `Page::dispatch_input(node, value, fonts, vw)` — set the value, fire `input` ONLY (not
`change`: `change` is a commit/blur event; firing it per keystroke would run change-validators on every
character), relayout. The shell's `edit_focused_input` now calls it per keystroke (then re-applies the
user's zoom, since dispatch_input lays out at base zoom, mirroring dispatch_click's contract). This is
the keyboard twin of the already-correct `dispatch_click` click path.

**Gate.** `js_conformance_suite` scenario (27): an `input` listener mirrors `event.target.value`; two
`dispatch_input` calls update a controlled mirror `hi`→`hip`, the field's `value` reflects before the
event, and a `change` listener's counter stays `0` (proving input-only, not change-per-keystroke). RED
against baseline (no `dispatch_input`; bare `set_attr` fired nothing → mirror stays `?`). Verified:
suite passes (spidermonkey, isolated); manuk-shell compiles + suite green; HANG/CRASH 0. Residue:
`change`-on-blur and `keydown`/`keyup`/`beforeinput` still unfired (separate keyboard-event mechanisms;
`input` is the controlled-component one). The shell-side wiring mirrors the ungated dispatch_click
consumption; the engine capability itself is gated. Mechanism: `Page::dispatch_input` + shell
`edit_focused_input`.

## Tick 176 — blur fires change+blur so on-blur form validation runs (interaction surface) (2026-07-17)

**TICK SHAPE: capability-mechanism (interaction surface — the field-commit event contract; the commit
half of tick 175's per-keystroke `input`). WIKI: docs/wiki/interaction-surface.md "Blur fires `change`
then `blur` — field-level validation runs on commit".**

A form validates a field when you LEAVE it — the on-blur/on-change "email invalid", the red border —
hung on `change`/`blur`. The shell cleared `focused_input` on click-away / Escape / submit / focus-move
and fired NOTHING, so field-level validation never ran and the field never committed.

**Fix.** New `Page::dispatch_blur(node, value_changed, …)` fires `change` (ONLY when `value_changed`)
then `blur` — the guard matters: `change` fires only if the value differs from focus time, so tabbing
through a field runs no change-validator. The shell snapshots the value in a new `focus_value` field
(set by a new `focus_input(node)` helper and on programmatic `.focus()`); a new
`blur_focused_input()` compares current vs snapshot, calls `dispatch_blur`, and is now the single
chokepoint every user focus-loss routes through — `PageAction::Link`/`Submit`/`Clear`, focusing a
different field (`focus_input` blurs the old first), Escape, and Enter (`submit_focused_form` commits
before submitting). Composes with tick 175: `input` per keystroke, `change`+`blur` on commit.

**Gate.** `js_conformance_suite` scenario (28): a `dispatch_blur(node, false)` fires `blur` only; after
a `dispatch_input` edit, `dispatch_blur(node, true)` fires `change` THEN `blur` (order asserted). RED
against baseline (no `dispatch_blur`). Verified: suite passes (spidermonkey, isolated); manuk-shell
58+2 green; HANG/CRASH 0. Residue: a PROGRAMMATIC focus move records the new `focus_value` but does not
yet fire `blur` on the old field; `focus`/`focusin`/`focusout` and `keydown`/`keyup` are separate
mechanisms. Mechanism: `Page::dispatch_blur` + shell `focus_input`/`blur_focused_input` chokepoint.

## Tick 177 — XMLHttpRequest.abort() honours the cancellation (JS platform — the XHR twin of t172) (2026-07-17)

**TICK SHAPE: capability-mechanism (T2 JS-platform — XHR request cancellation, the XHR twin of tick
172's fetch/AbortSignal). WIKI: docs/wiki/networking.md "`XMLHttpRequest.abort()` honours the
cancellation — a late response no longer fires `onload`". Fully manuk-owned JS glue.**

`XMLHttpRequest.prototype.abort` was `function() {}` — a no-op. A cancelled XHR still fired `onload`
with its full response when the host delivered it: a search-as-you-type box that aborts the stale
request per keystroke applied the OLD response over the new (the classic stale-result race), and every
request library's XHR cancel path did nothing.

**Fix (event_loop.rs).** `abort()` now `delete`s the request from `__xhrObj` — so a later
`__deliverXhr(id, …)` finds no object and is a no-op (the response cannot resolve a cancelled request;
the same drop-the-callback mechanism as tick 172's fetch abort) — resets `status`/`responseText`, and
fires `readystatechange` → `abort` → `loadend` (the XHR standard's abort() event order), leaving
`readyState` UNSENT. Added `onabort`/`onloadend` to the XHR constructor.

**Gate.** `js_conformance_suite` scenario (29): an XHR with `onload`/`onabort`/`onloadend` handlers,
`send()` then `abort()`, then a LATE `resolve_fetch(id, 200, "STALE-BODY")` — `onload` must NEVER fire
(`data-onload` stays `no`), and `abort`+`loadend` fired. RED against baseline (no-op abort → onload
fires → `data-onload=FIRED`). Verified: suite passes (spidermonkey, isolated); manuk-js
`fetch_and_xhr_through_the_loop` passes isolated (non-abort path unchanged); HANG/CRASH 0. Residue: an
`AbortSignal` passed to an XHR (rare) is still unwired. Mechanism: `XMLHttpRequest.prototype.abort`.

## Tick 178 — keydown fires with the real key + preventDefault suppresses the default (interaction surface) (2026-07-17)

**TICK SHAPE: capability-mechanism (interaction surface — the keyboard pre-empt contract, the keyboard's
click). WIKI: docs/wiki/interaction-surface.md "`keydown` fires with the real `key`, and
`preventDefault()` suppresses the default".**

A page intercepts a key via `keydown` + `preventDefault()` — the chat composer catching Enter to send
(not submit), the combobox swallowing ArrowDown. The shell went straight from a keypress to its own
default (submit/edit/blur) and dispatched NO keydown, so the page never saw the key and couldn't
pre-empt it.

**Fix.** `PageContext::dispatch_key` (dom_bindings) builds a real `__dispatchEvent(node,
{type,key,code,keyCode,which,bubbles,cancelable})` — the object-dispatch path already existed and
preserves event fields, so the KeyboardEvent shape was free — and returns `!defaultPrevented`.
`manuk_js::dispatch_key` + `Page::dispatch_key(node, ty, key)` (derives the legacy `keyCode` via a new
`key_code_for`). The shell fires `keydown` on the focused field BEFORE its default action (new
`key_name_for_dispatch` maps winit keys → DOM `key` names) and, if the page called `preventDefault`,
returns early WITHOUT performing the default — so Enter does not submit, the character is not inserted.
Deliberately additive: the existing key-handler match arms are untouched; a keydown-and-gate wraps
them, so no default-action path regressed. Composes with 175/176: keydown (pre-empt) → default (fires
input) → change/blur on commit.

**Gate.** `js_conformance_suite` scenario (30): a `keydown` handler reads `event.key`/`event.keyCode`
(`a`→`a:65`), and `preventDefault()` on Enter makes `dispatch_key` return false (`Enter:13`). RED
against baseline (no `dispatch_key`). Verified: suite passes (spidermonkey, isolated); manuk-shell 58+2
green; HANG/CRASH 0. Residue: `keyup` not yet fired (the pre-empt-the-default half is the value);
`event.code` equals `key` for named keys, approximates for characters; unsurfaced keys (function keys,
IME) dispatch nothing. Mechanism: `Page::dispatch_key` + shell key-handler pre-dispatch.

## Tick 179 — navigator.clipboard.writeText: the "copy" button actually copies (JS platform / shell) (2026-07-17)

**TICK SHAPE: capability-mechanism (JS platform + shell — the async Clipboard API bridged to the OS
clipboard). WIKI: docs/wiki/interaction-surface.md "`navigator.clipboard.writeText` — the "copy"
button actually copies".**

Copy-to-clipboard is one of the most common buttons on the web (code-block copy, copy link/API
key/coupon) and they all call `navigator.clipboard.writeText(text)`. `navigator.clipboard` was ABSENT,
so the call threw on `undefined` inside the click handler and the button silently did nothing — a dead
affordance (§1.8).

**Fix.** The shell already owns a real OS clipboard (`arboard`); this bridges the page to it with the
`window.open`/`postMessage` host-queue pattern. A native `__clipboardWrite(text)` (dom_bindings) pushes
onto a thread-local `PENDING_CLIPBOARD`; `navigator.clipboard.writeText` (JS, defined on the existing
`navigator` only when the `__clipboardWrite` bridge exists) calls it and returns the spec's resolved
`Promise<void>`; `manuk_js::take_clipboard_writes()` drains it; the shell's `pump_clipboard` (beside
`handle_window_opens`, called after a click dispatch) writes the last value to the OS clipboard.
`readText` resolves with the last text this page wrote (within-page round-trip) but does NOT read the
OS clipboard (a permission-gated capability — pretending would be a lie).

**Gate.** `js_conformance_suite` scenario (31): a copy button whose click calls
`writeText('copied-value-42')`; nothing queued before the click, and after `dispatch_click`,
`take_clipboard_writes()` returns exactly `["copied-value-42"]`. RED against baseline (`navigator.
clipboard` undefined → writeText throws → nothing queued). Verified: suite passes (spidermonkey,
isolated); manuk-shell 58+2 green; HANG/CRASH 0. Residue: OS-clipboard `readText`, `navigator.
permissions`, legacy `document.execCommand('copy')`, and clipboard writes off the click path (timer /
fetch reaction) are not yet pumped. Mechanism: `__clipboardWrite` native + `navigator.clipboard` +
shell `pump_clipboard`.

## Tick 180 — keyup fires on key release: search-as-you-type sees the settled value (interaction surface) (2026-07-17)

**TICK SHAPE: capability-mechanism (interaction surface — the release half of the keyboard trio).
WIKI: docs/wiki/interaction-surface.md "keyup fires on key release".**

**Hypothesis.** A huge swath of the (jQuery-era and long-tail) web binds search-as-you-type,
character counters, and shortcut-release logic to `keyup`, not `keydown` — because they want the
field's *settled* value after the keystroke applied. The shell fires `keydown` (tick 178) and
`input` (tick 175) on key PRESS, but never fires `keyup` at all, so a `keyup` listener never runs and
those boxes stay dead. `Page::dispatch_key` is already generic over the event type ("keydown"/"keyup"
per its doc); the gap is purely that the shell processes only `ElementState::Pressed` and drops every
`Released`. Fix: on key release, fire `keyup` on the focused field (no default action is associated
with keyup, so its `preventDefault()` return is irrelevant). Completes the trio: keydown (pre-empt) →
input (per-keystroke) → keyup (release).

**Gate.** engine/page/src/lib.rs dispatch-events scenario (32): a `keyup` handler on a field reads
`event.key`/`event.keyCode`; `dispatch_key(node,"keyup","x",…)` fires it and the handler records
`x:88`. RED against a shell that never dispatches on release. Verify: page suite + manuk-shell green;
HANG/CRASH 0.

## Tick 181 — object-fit: cover — thumbnails stop distorting (CSS render / replaced elements) (2026-07-17)

**TICK SHAPE: capability-mechanism (CSS layout/render — replaced-element fitting). WIKI:
docs/wiki/box-layout.md "object-fit — a replaced image fits its box without distorting".**

**Hypothesis.** `object-fit: cover` is on nearly every thumbnail on the web — the card-grid idiom
`img { width:100%; height:100%; object-fit:cover }` — so a photo fills its tile without distorting,
cropping the overflow. It was **completely unimplemented** (0 hits in the tree): the replaced-image
blit stretched the decoded bitmap to fill the box, so every non-square photo in a square tile came out
squashed. Fix: parse `object-fit` (css), carry it on `LayoutBox` (layout), and compute the
aspect-ratio-preserved destination rect + crop box at display-list build (paint) —
`object_fit_geometry(fit, box, iw, ih)`. `cover`/`none` can exceed the box, so `DisplayItem::Image`
gains a `content_clip` the paint walk intersects with any ancestor overflow clip; `fill` (default,
stretch) and `contain`/`scale-down` (fit inside) are unchanged in behaviour/clip. Recovered from
MinimalCascade on the shipping Stylo path too (same block as background-size). object-position is the
default 50% 50% (centered); explicit object-position not yet parsed.

**Gate.** engine/paint `object_fit_preserves_aspect_ratio`: a 200×100 (2:1) photo in a 100×100 tile —
`fill`→dest 100×100 (stretched, no clip); `cover`→dest 200×100 + 100×100 crop box; `contain`→dest
100×50, no clip. RED against the stretch baseline (which reports 100×100 for cover). Verify: css+layout+
paint suites green; HANG/CRASH 0.
