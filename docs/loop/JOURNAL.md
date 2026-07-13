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
