# REPOMAP 05 — Automation & Accessibility Surfaces

How production engines expose **automation and accessibility** — the accessibility
tree, DevTools/CDP, WebDriver and WebDriver-BiDi — and what Manuk, whose north-star
differentiator is being **agent-native**, should fold in. The thesis, stated up front
so the rest can be read against it: every shipping engine builds an automation surface
for a *remote controller* (a test harness in another process, speaking a wire protocol).
Manuk's controller is an **in-process AI agent** that already shares `engine/page`.
That collapses two layers everyone else pays for — a serialization/delta engine and a
node-addressing regime — and is the entire ergonomic and latency opportunity.

Cross-refs: this builds on Manuk's `engine/a11y` (the role+name tree), `agent/`
(Observation/Action loop, capabilities), and the `bidi/` crate (E4 remote end).

---

## 1. Scope & sources

Local clones under `/home/patrickd/manuk/`. Paths surveyed per engine:

**Chromium** (`chromium/`)
- A11y build/serialize: `third_party/blink/renderer/modules/accessibility/` (`ax_object_cache_impl.{h,cc}`, `ax_object.cc`, `ax_node_object.cc`), `ui/accessibility/` (`ax_tree.h`, `ax_node_data.h`, `ax_tree_serializer.h`), `ui/accessibility/platform/browser_accessibility_manager.cc`.
- CDP: `third_party/blink/public/devtools_protocol/` (`browser_protocol.pdl`, `domains/DOM.pdl`, `domains/Accessibility.pdl`, `Input.pdl`), `third_party/blink/renderer/core/inspector/inspector_dom_agent.cc`, `modules/accessibility/inspector_accessibility_agent.cc`, `content/browser/devtools/protocol/input_handler.cc`. Runtime domain lives in `v8/src/inspector` (not surveyed in depth).

**Firefox** (`firefox/`)
- A11y: `accessible/generic/{LocalAccessible,DocAccessible}.cpp`, `accessible/ipc/{RemoteAccessible,DocAccessibleParent}.cpp`, `accessible/base/{NotificationController,EventTree,nsTextEquivUtils,ARIAMap,CachedTableAccessible,CacheConstants.h,AccAttributes}.cpp`.
- Remote: `remote/webdriver-bidi/` (`WebDriverBiDiConnection.sys.mjs`, `modules/root/{session,input,browsingContext,script}.sys.mjs`, `modules/windowglobal/input.sys.mjs`), `remote/shared/webdriver/{Actions,NodeCache,KeyData}.sys.mjs`, `remote/marionette/`. **No CDP implementation exists in this tree** (only Puppeteer CDP test specs under `remote/test/`); the live protocols are Marionette + WebDriver-BiDi.

**WebKit** (`WebKit/`)
- A11y: `Source/WebCore/accessibility/` (`AXObjectCache.cpp`, `AccessibilityObject.cpp`, `AccessibilityRenderObject.cpp`, `AccessibilityNodeObject.cpp`, `isolatedtree/AXIsolatedTree.{h,cpp}`).
- WebDriver: `Source/WebDriver/` (classic HTTP/JSON), `Source/WebKit/UIProcess/Automation/{WebAutomationSession,SimulatedInputDispatcher}.cpp` + `WebProcess/Automation/WebAutomationSessionProxy.cpp` (Inspector Automation protocol; also `Bidi*Agent.cpp`).

**Ladybird** (`ladybird/`) — the closest lean-from-scratch analogue
- A11y: `Libraries/LibWeb/ARIA/` (`ARIAMixin.cpp`, `Roles.{h,cpp}`, `AriaData`) — no separate a11y tree.
- WebDriver: `Services/WebDriver/` (`Client.cpp`, `Session.cpp`) + `Libraries/LibWeb/WebDriver/` (`ElementReference.cpp`, `Actions.cpp`, `InputState.cpp`).

**Manuk today** (READ): `engine/a11y/src/lib.rs`, `agent/src/{lib.rs,forms.rs,capabilities.rs,traversal.rs}`, `bidi/src/protocol.rs`, `shell/src/panel.rs`.

---

## 2. Per-engine approach

### 2.0 The two problems every engine solves

An automation surface is two mechanisms bolted together:

1. **An observation channel** — a semantic model of the page (the accessibility tree,
   or a DOM snapshot) that the controller can read, *kept in sync* with a mutating DOM.
2. **An action channel** — a way to *name* a node stably (node addressing) and to
   *drive* it (hit-testing + input synthesis), plus **readiness signals** so the
   controller knows when the page settled.

The interesting engineering is that in Chromium/Firefox/WebKit the controller lives in
**another process** (or thread), so both channels must be **serialized and delta-synced**
across a boundary. That boundary is the source of most of the complexity — and Manuk
does not have it.

### 2.1 Chromium

**A11y tree — build.** `AXObjectCacheImpl` is the per-`Document` owner; every node is an
`AXObject` keyed by an integer `AXID` in `HeapHashMap<AXID, Member<AXObject>> objects_`
(`ax_object_cache_impl.h:1041`). `GetOrCreate(LayoutObject|Node|...)` (`:449-455`) builds
lazily from the layout tree. **Sync is deferred and layout-gated, not synchronous:** DOM
mutations enqueue callbacks onto `tree_update_callback_queue_main_` keyed by
`AddNodeRequiringCacheUpdate(AXID, TreeUpdateReason)` (`:838`), flushed only when layout is
clean via `ProcessDeferredAccessibilityEvents`; `IsImmediateProcessingRequired` (`:992`)
lets urgent updates jump the queue. This batching is the price of correctness under a
mutating DOM — a lesson for Manuk's cache invalidation (§4).

**Roles/names (AccName).** `AXNodeObject::TextAlternative` (`ax_node_object.cc:5261`) is the
W3C AccName recursion: `AriaTextAlternative` (`:5369`) before `NativeTextAlternative`
(`:7143`), i.e. aria-label(ledby) wins over native labels; the winning source is recorded
in a `NameFrom` enum (`ax_object.cc:1786`). This is exactly Manuk's `accessible_name`
precedence in `engine/a11y/src/lib.rs:475` — but Chromium does the **full** recursion Manuk
documents as a gap (`lib.rs:24`).

**Serialization — the delta engine.** This is the layer Manuk does not need. `AXTreeSerializer<...>`
(`ui/accessibility/ax_tree_serializer.h:75`) keeps a **shadow "client tree"** of what the
consumer already knows and emits only deltas: `SerializeChanges` (`:107`), divergence via
`LeastCommonAncestor` (`:183`), reparenting via `ComputeReparentingLCA` (`:195`). The pump
`SerializeUpdatesAndEvents` (`ax_object_cache_impl.cc:3859`) produces
`vector<ui::AXTreeUpdate>` + `vector<ui::AXEvent>`, shipped renderer→browser over mojo
(`content/renderer/accessibility/render_accessibility_impl.cc:491`) and applied by
`AXTree::Unserialize` into the browser-side authoritative tree
(`browser_accessibility_manager.cc:524`). A location-only fast path
(`SerializeLocationChanges`, `:6120`) ships bounding boxes without a full reserialize.

**Hit-testing.** Renderer-authoritative: `AXObject::ElementAccessibilityHitTest`
(`ax_object.cc:6396`) recurses children including ignored ones. Browser-side `HitTest`
(`browser_accessibility_manager.cc:1428`) is **async** — it round-trips to the renderer —
with `CachingAsyncHitTest`/`ApproximateHitTest` (`:1974`,`:2027`) as synchronous
approximations. Note the tension: a cross-process automation surface cannot hit-test
synchronously without an approximation cache.

**CDP — what Playwright/Puppeteer actually ride on.** Two ID spaces coexist
(`inspector_dom_agent.cc`): ephemeral session `nodeId` (`Bind`, `:570`, `last_node_id_++`)
and **stable process-global `backendNodeId`** backed by `DOMNodeIds` (`:644`), plus JS
`objectId` from Runtime. The `.pdl` schemas live in
`third_party/blink/public/devtools_protocol/domains/` (`DOM.pdl:23` `BackendNodeId`;
`Accessibility.pdl:11` `AXNodeId`, `getFullAXTree` `:232`). `getAXNodeAndAncestors`
(`inspector_accessibility_agent.cc:280`) is the **join point** between the DOM and AX ID
regimes via `dom_agent_->AssertNode(...)` (`:293`). Input is fully browser-side:
`InputHandler::DispatchMouseEvent` (`input_handler.cc:1360`) builds a `blink::WebMouseEvent`
and injects via `RenderWidgetHostImpl::ForwardMouseEvent` (`:756`), routed to the correct
(possibly OOPIF) widget by `InputEventRouter` — **independent of the a11y tree**. This is
the key CDP fact: Puppeteer/Playwright's `page.click(selector)` resolves the selector to a
box via Runtime/DOM, then synthesizes a raw input event at coordinates; the a11y tree is a
*separate* read-only channel (`Accessibility.getFullAXTree`) most scripts never touch.

### 2.2 Firefox

**A11y — local + remote, cache-pushed.** `LocalAccessible` (content process, authoritative,
built from DOM+frames) vs `RemoteAccessible` (parent process, **no DOM access** — every
property served from a cached `AccAttributes` map, `RemoteAccessible.cpp:262`). AccName:
`LocalAccessible::Name` (`LocalAccessible.cpp:142`) → ARIAName → NativeName →
`GetNameFromSubtree` → title → CSS alt, provenance in `ENameValueFlag`. The **cache is
domain-partitioned** (`CacheConstants.h`: NameAndDescription, State, Table, Viewport…);
`BundleFieldsForCache` (`LocalAccessible.cpp:3561`) serializes requested domains,
distinguishing `Initial` (all) vs `Update` (delta, with `DeleteEntry()` sentinels).
Mutations call `QueueCacheUpdate(this, CacheDomain::X)`, coalesced by the
`NotificationController` on the refresh tick and flushed to the parent via
`DocAccessibleParent::RecvCache` → `RemoteAccessible::ApplyCache` (`:182`).

**Remote hit-testing without layout.** The clever bit worth stealing conceptually:
`RemoteAccessible::ChildAtPoint` (`RemoteAccessible.cpp:655`) has no layout in the parent,
so it reads a **`CacheKey::Viewport` array — accessible IDs pre-sorted in z-order/hit-test
order** (`:690`) — and walks it using cached bounds. The local side uses real layout
(`nsLayoutUtils::GetFrameForPoint`, `LocalAccessible.cpp:533`). Takeaway: a flat,
z-ordered hit-test list *is* the serializable form of hit-testing.

**WebDriver-BiDi — why it's the modern standard.** BiDi is the W3C **bidirectional**
successor to classic WebDriver: one WebSocket carries `command`→`result`/`error` **plus
server-pushed `event`s**, so a controller can *subscribe* to load/network/log events
instead of polling. Firefox is the reference implementation.
- Dispatch: `WebDriverBiDiConnection.sys.mjs:177` validates `{id, method, params}`, splits
  `method` into `{module, command}` on the dot (`session.subscribe`), routes to
  `session.execute(module, command, params)`.
- Modules exist at three tiers: `modules/root/` (parent orchestration),
  `modules/windowglobal/` (content-process DOM work), `windowglobal-in-root/` (proxy).
  Present: `browsingContext, script, input, network, log, session, storage, emulation…`.
- **Subscription model** (`modules/root/session.sys.mjs`): a subscription is
  `{eventNames, subscriptionId, topLevelTraversableIds, userContextIds}` (`:26`); empty
  context sets ⇒ global. `subscribe` (`:107`) returns `{subscription: id}`; stored as
  SessionData and pushed to content modules whose `_applySessionData` starts native
  listeners per matching context. This is the machinery Manuk's `bidi/src/protocol.rs`
  approximates with a flat `subscriptions: Vec<String>` (`protocol.rs:186`).
- **Node addressing:** opaque `sharedId` strings backed by `shared/webdriver/NodeCache.sys.mjs`,
  serialized inside the `script` module's "RemoteValue" format; contexts by browsing-context
  id, realms by realm id.
- **Input** (`input.performActions`, `modules/root/input.sys.mjs:285`): builds an action
  Chain (`shared/webdriver/Actions.sys.mjs`), serializes execution through
  `inputState.enqueueAction`, ticks grouping concurrent actions; content-side
  `_dispatchEvent` (`windowglobal/input.sys.mjs:212`) drives `windowUtils` to inject real
  widget events. `releaseActions` replays a cancel list to undo pressed keys/buttons.

### 2.3 WebKit

**A11y.** `AXObjectCache` (per-Document) builds `AccessibilityRenderObject`s from the render
tree keyed by `AXID`. Roles: `determineAccessibilityRole` (`AccessibilityRenderObject.cpp:2415`);
WebDriver bridge string `computedRoleString` (`AccessibilityObject.cpp:3053`). Hit-testing
`accessibilityHitTest` (`AccessibilityRenderObject.cpp:2309`) forces layout then runs a real
render-layer hit test with `HitTestRequest::Type::AccessibilityHitTest`. The
**`AXIsolatedTree`** (`isolatedtree/AXIsolatedTree.h`) is an off-main-thread snapshot for AT
performance: hot booleans packed into an `AXPropertyFlag` bitfield (`:66`) + an `AXProperty`
map for the rest, main thread queues changes under `m_changeLogLock`, reader thread applies
via `applyPendingChanges` (`AXIsolatedTree.cpp:1395`). Geometry is refreshed separately by
`AXGeometryManager`.

**WebDriver.** `Source/WebDriver/` is a standalone HTTP/JSON process translating classic
WebDriver into WebKit's **Inspector Automation protocol** (`Automation.json`). Element
handles are opaque **NodeHandle** strings scoped to (BrowsingContext, Frame); resolution is
done in **injected page JS** — `WebAutomationSessionProxy::elementForNodeHandle`
(`WebAutomationSessionProxy.cpp:320`) calls an atom `nodeForIdentifier(handle)` returning a
`JSElement`. WebDriver's a11y endpoints map straight to AX: `getComputedRole` (`:1650`),
`getComputedLabel` (`:1668`). Input: `SimulatedInputDispatcher` diffs input-source states
and issues `simulateMouseInteraction`/`simulateKeyboardInteraction` (`:306`,`:379`), one
VirtualKey diff at a time.

### 2.4 Ladybird — the lean reference

Ladybird is the most instructive comparison because it is a from-scratch browser that
deliberately **skips the parallel a11y tree entirely**. ARIA lives on the DOM element:
`ARIAMixin` (`Libraries/LibWeb/ARIA/ARIAMixin.cpp`) gives every `Element` `role`/`aria-*`
reflection, roles computed on demand via `role_or_default` — no `AXObjectCache`, no isolated
tree, no serializer. Automation goes straight through the paint tree.

- **Node addressing (trivially portable):** the WebDriver element reference *is* the DOM
  node's `UniqueNodeID` stringified — `String::number(node.unique_id().value())`
  (`WebDriver/ElementReference.cpp:76`); the standard web-element key
  `element-6066-11e4-a52e-4f735466cecf` (`:29`); resolution via `Node::from_unique_id`
  (`:68`) with a staleness check (`is_element_stale`, `:247`). Two weak maps model the spec.
- **Hit-testing:** `is_element_pointer_interactable` (`:261`) computes the in-view center
  point and calls `document.hit_test(center, HitTestType::Exact)` on the **paint tree**
  (`:279`), then checks the hit node equals the element. No AX indirection.
- **Input:** the Actions state machine (`WebDriver/Actions.cpp`) calls `Page::handle_*`
  **synchronously in-process** — `handle_mousedown` (`:1145`), `handle_keydown` (`:1035`),
  CSS→device via `css_to_device_point` (`:1119`). No OS-level synthesis, no cross-process hop.

Ladybird proves the lean thesis: a stable integer node id + paint-tree hit-test + in-process
event dispatch is a *complete* automation surface. Manuk should match this floor and then
exceed it on the agent-native axis.

---

## 3. Manuk today — honest surface and gaps

Manuk already has, in-process and Rust-native, most of what the big engines serialize across
a boundary:

**Observation channel** — `engine/a11y/src/lib.rs` builds a real role+name tree per
HTML-AAM/WAI-ARIA with a pragmatic AccName subset. It has geometry (`build_tree_with_rects`,
`lib.rs:601`), a flat agent-readable rendering (`to_observation_lines`/`to_viewport_lines`,
`:254`/`:266`, each line carrying a click point `@(x,y)`), `find`/`find_containing` by
role+name (`:282`/`:289`), and `hit_test` picking the deepest containing box (`:298`). The
agent's `Observation` (`agent/src/lib.rs:187`) wraps this as `semantics: Vec<String>`,
viewport-clipped, inside the E6 "UNTRUSTED PAGE CONTENT" fence
(`Observation::render`, `:247`) — a genuine anti-prompt-injection design the others lack.

**Action channel** — `AgentBrowser` (`agent/src/lib.rs:137`) exposes `navigate`, `click_by_name`
(role+name → `activate`, `:564`), `click_at` (hit-test → `activate`, `:571`), `type_into`
(`:470`), `submit` (`:480`), `scroll_to`, `back/forward/traverse`. `activate` (`:504`)
models real link/submit/toggle semantics. Forms are modelled honestly
(`agent/src/forms.rs`: GET serialization, POST refused not downgraded, `:131`). Actions are a
typed enum with a JSON tag (`Action`, `:653`) and per-invocation `Capabilities` enforcement
(`agent/src/capabilities.rs`: action-kind allowlist + origin allowlist + sensitivity gate,
single check point `:235`).

**Remote protocol** — `bidi/src/protocol.rs` is a **transport-free WebDriver-BiDi remote end**
(the strategically correct choice over CDP, `bidi/src/lib.rs:3`): `session.*`,
`browsingContext.*` (create/navigate/getTree/screenshot/traverseHistory), `input.performActions`
(pointer click only), event ordering that matches real clients (`Dispatched.before`, `:118`),
subscription filtering (`:186`). It drives the *same* `AgentBrowser`.

**Gaps vs a real automation protocol (stated, not faked):**
- **No stable node handle.** Addressing is by (role, name) string or (x, y) coordinate,
  re-resolved against a freshly-built tree every call (`AgentBrowser::resolve`, `:458`).
  There is no `backendNodeId`/`sharedId`/`UniqueNodeID` equivalent surfaced to the agent, so
  an agent cannot say "the node I saw last turn" — only "a node matching this name now". The
  arena already has `NodeId`; it is simply not exposed as an addressable handle.
- **Tree rebuilt every observation.** `id_index` + `build_tree` run fresh each `observe`
  (`lib.rs:613` → `page.a11y_tree()`); there is no cache, no dirty-tracking, no delta. Fine at
  current scale, but O(DOM) per turn.
- **AccName is a subset.** No `aria-owns` reparenting, no full labelledby recursion, no live
  regions (`engine/a11y/src/lib.rs:23`). Roles cover a curated subset (`Role`, `:71`).
- **Input is coarse.** `input.performActions` supports only pointerMove+down+up
  (`protocol.rs:529`); no keyboard, wheel, multi-pointer, durations. `type_into` sets a
  `value` attribute rather than synthesizing keystrokes — no per-key events, no `input`/`change`
  event dispatch (Manuk's page path is currently JS-less anyway).
- **`script.*` is unimplemented** (`protocol.rs:451`) — needs the JS engine.
- **Readiness is synchronous-by-accident.** Navigation completes before `navigate` returns,
  so `domContentLoaded`/`load` fire together (`protocol.rs:369`); there is no incremental
  parser hook, no network-quiescence signal, no "wait for selector/aria-node".
- **No occlusion in hit-test.** `hit_test` picks smallest containing box, not topmost painted
  (`engine/a11y/src/lib.rs:25`) — z-index stacks can mis-resolve.

---

## 4. Fold-in recommendations — the differentiator

**Design principle: Manuk's controller is in-process and shares `engine/page`. Do not
rebuild the serialize→ship→delta→unserialize pipeline that Chromium/Firefox exist to
maintain. Spend that saved complexity budget on agent ergonomics and round-trip latency.**

Concretely, beat Playwright-on-Chromium by exploiting three structural advantages it cannot
have: (a) no process boundary → synchronous hit-test and zero-copy tree reads; (b) the
controller is a *model*, so the observation format should be token-optimized, not
DOM-faithful; (c) one code path serves both the human shell (via `Handoff`) and the agent, so
state (login, half-filled form) is never lost.

### 4.1 Give every addressable thing a stable handle — borrow `backendNodeId`/`UniqueNodeID`

Adopt Ladybird's model verbatim: **expose the arena `NodeId` as the agent's node handle.**
It is already stable across a page's life and already the key `A11yNode.node` carries
(`engine/a11y/src/lib.rs:216`). Add a handle to each `A11yNode`'s rendered line and let
Actions target `{ "node": 4213 }` in addition to role+name and coordinates. This kills the
"re-resolve the tree every call" fragility and lets an agent reference *the node it saw last
turn* — the single biggest ergonomic gap vs CDP. Add a cheap staleness check
(`is_element_stale` analogue: node still connected + document current). Do **not** invent a
second opaque id space (CDP's nodeId-vs-backendNodeId split is boundary-driven accidental
complexity Manuk has no reason to reproduce).

### 4.2 Cache the a11y tree with dirty-tracking — but keep it a plain in-process tree

Today `build_tree` runs fresh per observation. Borrow Chromium's *invalidation discipline*
without its serializer: keep the last `A11yNode` tree and a dirty flag set on DOM mutation
(the arena already funnels writes through `Dom::set_attr`/`append_child`). Rebuild only when
dirty and layout is clean (Chromium's layout-gating lesson, `ax_object_cache_impl.h:992`).
**Skip** `AXTreeSerializer`, `AXTreeUpdate`, the shadow client tree, the mojo boundary — all
of it exists only to cross a process boundary Manuk does not have. This is the single largest
"bloat to skip."

For hit-testing, adopt Firefox's insight (`RemoteAccessible.cpp:690`): maintain a **flat
z-ordered hit-test list** so `click_at` resolves in one pass *and* respects paint order —
fixing the documented occlusion gap (`engine/a11y/src/lib.rs:25`). The paint tree already
computes stacking; feed its order into the a11y tree's hit-test like Ladybird does
(`document.hit_test(...Exact)`).

### 4.3 The agent-native API: Rust bindings + skills, not a wire protocol

Two audiences, one core:

**(a) In-process Rust bindings (the fast path).** The agent loop should call `AgentBrowser`
methods directly — it already does. Formalize this as the primary API and make it *typed and
handle-based*:

```rust
// observation returns handles, not just strings
struct AgentNode { handle: NodeId, role: Role, name: String, click: Option<(f32,f32)>, occluded: bool }
impl AgentBrowser {
    fn snapshot(&self) -> Vec<AgentNode>;              // flat, z-ordered, token-lean
    fn click(&mut self, node: NodeId) -> Activation;   // synchronous, no round-trip
    fn fill(&mut self, node: NodeId, text: &str);      // type_into by handle
    fn query(&self, role: Role, name: &str) -> Option<NodeId>;
    fn wait_for(&mut self, cond: Readiness) -> Result<()>; // §4.4
}
```

The win over Playwright is latency: `page.click('button')` in Playwright is
JSON→WebSocket→browser→Runtime.evaluate→box→Input.dispatchMouseEvent→ack, ~ms of IPC per
call. Manuk's is a function call over a shared arena — **microseconds**, synchronous, no
serialization. A multi-step agent task (observe→click→observe→fill→submit) that is dozens of
round-trips in Playwright is dozens of function calls here.

**(b) Agent skills (the ergonomic surface).** Expose a small, opinionated set of
**intent-level** skills the model calls, not a faithful DOM API. Manuk already has the right
grain: `click_text{role,name}`, `type{field,text}`, `submit`, `scroll_to`. Keep the surface
**tiny and role/name-first** (`Action` enum, `agent/src/lib.rs:653`) — models are far more
reliable naming "the *Sign in* button" than authoring a CSS selector, and role+name is
injection-resistant in a way selectors are not. Add only: `click{node}` (by handle, §4.1),
`read{node}` (subtree text of one node), and `wait{until}` (§4.4). Resist a `Runtime.evaluate`
equivalent as the *primary* surface — arbitrary JS eval is the biggest prompt-injection and
capability-scoping hole in the CDP model; keep it behind an explicit, capability-gated skill.

**Selectors:** offer role+name and handle first; add a CSS-selector escape hatch backed by
the existing Stylo matcher (`engine/css`) only for the cases role+name cannot express
(nth-of-type, structural). Do not make selectors the primary addressing mode — that is
Playwright's ergonomic weakness for a model driver.

### 4.4 Readiness — the signal Playwright agents actually wait on

Manuk's biggest *functional* gap is readiness. Playwright's auto-waiting (element visible,
stable, enabled, receives events) is why it feels reliable. Fold in a typed `Readiness`
the agent can await, computed **synchronously from the shared page** (no polling, no
event round-trip):
- `NodePresent(role, name)` / `NodePresent(handle)` — resolvable in the current tree.
- `NavigationSettled` — real signal once the incremental parser lands; today it is trivially
  true (`protocol.rs:369` honestly notes both readiness states fire together).
- `NetworkIdle` — once per-request hooks exist (currently one synthetic `responseCompleted`,
  `protocol.rs:384`).
This is where BiDi's *event/subscription* model (`session.sys.mjs:107`) earns its place:
keep Manuk's `bidi` subscriptions for the **external** driver, but for the **in-process**
agent, expose readiness as awaitable Rust futures — strictly better than subscribing to your
own events.

### 4.5 Input synthesis — match Ladybird's floor, not Chromium's ceiling

Drive `Page::handle_*`-style synchronous in-process dispatch (Ladybird, `Actions.cpp:1145`),
not OS-level or cross-process synthesis. Extend the current pointer-only path
(`protocol.rs:529`) to keyboard + wheel through the *same* in-process handlers, and make
`type_into` synthesize per-key events (and fire `input`/`change`) once the JS event loop is
wired — until then, the value-attribute approach (`lib.rs:470`) is an honest stand-in. Borrow
BiDi's **cancel-list/`releaseActions`** discipline (`input.sys.mjs:326`) so a crashed task
leaves no keys stuck down.

### 4.6 Keep the BiDi remote end — as the *compatibility* layer, not the primary API

`bidi/` is strategically right: it makes Manuk drivable by existing Puppeteer/Selenium
without CDP (`bidi/src/lib.rs:3`). Keep and grow it toward W3C conformance
(`input.performActions` full grammar, `script.*` when JS lands, per-context subscriptions
like Firefox's three-tier modules). But it is the **interop** surface for *external* tools;
the **native** surface (§4.3) is where Manuk wins on latency and ergonomics. Do not let BiDi
conformance dictate the in-process API shape — they are different products for different
callers sharing one `AgentBrowser` core.

### 4.7 Bloat to explicitly avoid

- **The full CDP surface.** 50+ domains, dual nodeId/backendNodeId spaces, a delta serializer
  — all boundary-driven. Manuk has no boundary. Skip it entirely.
- **A parallel isolated a11y tree** (WebKit `AXIsolatedTree`, Firefox `RemoteAccessible`
  cache). These exist to serve an *out-of-process assistive-tech reader*. Manuk's a11y tree
  is already the in-process observation channel; a second cached copy buys nothing until/unless
  a platform screen-reader bridge (`accesskit`) demands off-thread reads.
- **Legacy classic WebDriver / Marionette / Automation.json.** BiDi supersedes them; implement
  one modern protocol, not three.
- **Arbitrary-JS-eval as the primary action.** Capability-gate it; do not build the agent's
  ergonomics on `Runtime.evaluate`.
- **Coordinate-only addressing as primary.** Keep `click_at` for pixel tasks, but handle- and
  role+name-addressing are more robust for a model and injection-resistant.

---

## 5. Open questions for frontier research

1. **Node-handle lifetime across navigation/reflow.** Ladybird ties the handle to
   `UniqueNodeID` + staleness; WebKit re-resolves via injected JS each time. For an
   in-process agent that observes across turns, what is the right invalidation contract —
   and should the agent be *told* when a handle it holds went stale rather than silently
   failing?
2. **Token-optimal observation format.** Everyone serializes a DOM-faithful tree. Manuk's
   consumer is a token-metered model. What is the information-theoretically lean encoding of
   "what can I act on here"? Is `to_viewport_lines` (role, name, click point) already near
   optimal, or should it be a diff-from-last-turn (borrowing the *idea* of AXTreeUpdate deltas
   for tokens, not for IPC)?
3. **Readiness without a full event loop.** Can `NetworkIdle`/`NavigationSettled` be computed
   soundly from the fetch layer + incremental parser without reimplementing Chromium's
   lifecycle state machine? What is the minimal honest readiness signal?
4. **Occlusion + interactability semantics.** WebDriver defines pointer-interactability
   (in-view center hit-tests to the element itself, Ladybird `ElementReference.cpp:261`).
   Should Manuk's `snapshot` mark each node `occluded`/`disabled`/`offscreen` so the agent
   never attempts an impossible click? How cheap can that be over the paint tree?
5. **Capability scoping over handles.** `capabilities.rs` gates by action-kind + origin. With
   stable node handles, can grants be *per-node* ("may type into `q`, not `password`", the
   documented gap at `capabilities.rs:29`) without a macaroon-style token?
6. **The dual-consumer contract.** One `AgentBrowser` serves the human shell (`Handoff`,
   `agent/src/lib.rs:162`) and the agent. What invariants must hold so a session can pass
   between them mid-task (an expanded accordion, a focused field) without either side
   observing an inconsistent tree?
</content>
</invoke>
