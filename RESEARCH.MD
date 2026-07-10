# RESEARCH.md — Manuk Browser Research Directive

A standing research agenda for the Manuk engine, organized by optimization axis.
This is **not** a one-shot query. Each numbered block below is an independent
research pass with its own success criterion. Run them separately; fold findings
back into `CLAUDE.md` (Directions of improvement) as concrete, seam-scoped items.

## How to use this

For each research block:
1. State the **decision it feeds** (what changes in the codebase or roadmap if the
   answer is X vs Y). If a block doesn't change any decision, skip it — it's trivia.
2. Prefer **primary sources**: engine source (Servo, Chromium, Gecko, Ladybird,
   WebKit), WPT, W3C/WHATWG specs, crate docs and benchmarks, CVE/security writeups,
   academic papers. Treat blog posts and aggregators as leads to verify, not answers.
3. Every finding must resolve to one of: **(a) reuse crate X**, **(b) hand-roll
   because <specific reason>**, **(c) defer, blocked on <prerequisite>**, or
   **(d) out of scope per CLAUDE.md non-goals**. A finding with no disposition is
   incomplete.
4. Respect the **standing constraints** (below) — they pre-empt whole categories of
   answer and prevent re-litigating settled decisions.

## Standing constraints (do not re-open without explicit human sign-off)

- **JS engine is vendored SpiderMonkey via `mozjs`.** No research into writing a JS
  engine, patching JIT/GC, or swapping to V8. Modification boundary is fixed.
- **No bot-evasion in the core engine.** Fingerprinting research is bounded to the
  *legitimate human-operator* framing only (see Axis F) — never generalized
  scraping evasion, never for the agent path.
- **No DRM/EME, no horizontal multi-node scaling, no Chromium-quirk parity** beyond
  what WPT compliance yields. These are settled non-goals.
- **WPT is the conformance ground truth.** Research that proposes a *different*
  primary conformance corpus is out of scope; research that improves how we *run*
  WPT is in scope.
- **Build-vs-reuse is not free choice.** The bar for hand-rolling over a mature
  crate is: the crate crosses a security/correctness boundary we can't own, OR it's
  measurably the bottleneck, OR it forces bloat we can quantify. "We could write it
  ourselves" is not sufficient — every hand-roll is future maintenance surface.

---

## Axis A — Rendering performance & quality

**A1. Vello integration maturity for a real browser paint tier.**
Decision it feeds: whether to wire Vello GPU behind the existing `Painter` trait
now, or hold on the CPU/`tiny-skia` tier longer.
Investigate: current Vello release state and known gaps (conflation artifacts,
blur/filter support, clip/gradient/image coverage); how Servo and other consumers
handle the alpha-stage rough edges; whether Vello Hybrid or Vello CPU is the right
default for background tabs vs. focused. Compare against staying on `tiny-skia`
for correctness-first and adding GPU later.

**A2. Incremental/damage-driven repaint & relayout.**
Decision it feeds: the design of the relayout path (currently full-page).
Investigate: how Servo/Blink/WebKit structure dirty-bit propagation, style
invalidation sets, and layout-tree reuse; what data structures make subtree
invalidation cheap; whether `taffy` supports incremental relayout or forces full
recompute. Map to the compositor's existing `Damage` tracking.

**A3. Shaped-run and decoded-image caching.**
Decision it feeds: where to add caches and what keys to use.
Investigate: cache key design for shaped text runs (font+size+text+features) and
decoded images; eviction policy; memory accounting per tab. Cross-reference the
per-tab memory budget in the compositor.

## Axis B — Latency (click-to-navigate)

**B1. Streaming parse→layout as bytes arrive.**
Decision it feeds: wiring the incremental path README/CLAUDE both note is
"positioned but not wired."
Investigate: how html5ever's streaming sink is driven incrementally in practice;
how Blink's preload scanner and incremental layout overlap network and CPU; what
the minimal correct "layout the DOM built so far" checkpoint looks like; risks
(layout thrash, reflow on late CSS).

**B2. Connection reuse, preconnect, DNS-prefetch.**
Decision it feeds: `engine/net` connection model (currently one connection per
request).
Investigate: hyper connection pooling patterns; speculative preconnect on link
hover (what signal, what TTL, cancellation); happy-eyeballs / dual-stack; the
security/privacy implications of speculative connections (do not prefetch
cross-origin without policy).

**B3. HTTP/2 activation and HTTP/3/QUIC.**
Decision it feeds: turning on the already-compiled h2 path and adding `quinn`.
Investigate: ALPN negotiation correctness; when h2 multiplexing actually wins for
browsing workloads vs. h1 connection reuse; `quinn` maturity and integration cost;
compression (gzip/br) decode — which crate (`flate2`, `brotli`) and the bloat cost.

## Axis C — Memory & footprint (lean binary, lean RAM)

**C1. Realizing hibernation for real.**
Decision it feeds: the compositor's tier model, currently policy-only.
Investigate: how Chromium/Firefox actually freeze background tabs (timer
throttling, JS suspension, tile eviction to disk); what SpiderMonkey APIs exist for
suspending/serializing a runtime or sharing startup snapshots across isolates;
measured per-tab baseline before/after.

**C2. SpiderMonkey build-size reduction (inside the boundary).**
Decision it feeds: build config only — explicitly the *safe* side of the
modification boundary.
Investigate: which SpiderMonkey build flags strip WASM, the debugger/inspector
protocol, and unused ICU locale data; measured binary-size delta; correctness
implications (does stripping ICU break `Intl`? which locales do target sites need?).
Never touches JIT/GC/sandbox — config only.

**C3. Dependency-graph hygiene.**
Decision it feeds: `Cargo.toml` across the workspace.
Investigate: current duplicate resolutions (README notes two `tiny-skia`/
`cssparser` versions); `cargo tree`/`cargo bloat` analysis; which deps carry heavy
default features that `default-features = false` would drop; the real binary-size
contribution of each top-level dep.

## Axis D — Compatibility breadth (the long pole)

**Note on framework rendering (React, Next.js, Angular, Svelte, vanilla, PHP-
rendered, and all other frontend/backend stack variants):** this is not
separate scope from D1-D4. No framework invents its own HTML/CSS/JS
semantics — they compile to standard DOM manipulation, CSSOM, and Web APIs,
so a spec-compliant engine renders all of them correctly as a consequence of
D1-D4 landing, not as a per-framework target. Do not treat "support React" /
"support Svelte" as distinct work items — verify against WPT and real pages,
not against a framework list. Two things genuinely aren't covered by general
DOM/CSS work and are worth naming explicitly rather than assumed included:
Custom Elements + Shadow DOM (a distinct spec area many design systems and
some frameworks rely on for component encapsulation), and SSR/hydration
mechanics specifically — `<script type="module">`, dynamic `import()`, and
the History API (`pushState`/`replaceState` for client-side routing),
load-bearing for Next.js/SvelteKit-style frameworks and worth prioritizing
explicitly within D3 rather than falling wherever alphabetical API coverage
happens to land.

**D1. Layout: floats, tables, positioning, margin collapsing.**
Decision it feeds: the from-scratch layout roadmap, per-feature.
Investigate, per feature: the relevant WPT subdirectory and pass count as a target;
how Servo structures each (float band tracking, table fixed/auto algorithms, the
containing-block chain for abs/rel/fixed/sticky, margin-collapse rules); the
spec ambiguities CLAUDE.md warns about (tables especially). Each feature is its
own pass with its own WPT target.

**D2. Stylo real-cascade activation.**
Decision it feeds: replacing the `MinimalCascade` delegate behind the existing
`StyleEngine` adapter with Stylo's real cascade.
Investigate: the actual Stylo embedding API surface (it's built for Servo, not
general embedders); what glue Servo provides that we'd need to replicate; media
queries, `@font-face`, full selector grammar, custom properties, `calc()`; the
build-time cost.

**D3. Web API binding surface (`engine/js::bindings`).**
Decision it feeds: how the DOM projects into SpiderMonkey — the largest-volume
remaining component.
Investigate: how Servo generates WebIDL bindings (its codegen pipeline); whether we
reuse that codegen or hand-write bindings for a prioritized API subset; which APIs
the target-site constellation actually exercises (measure, don't guess — instrument
real Chrome against the target list); event loop + timers, `fetch`/XHR, events,
mutation. Prioritize by target-site usage frequency.

**D4. Loading: external resources, decoding, charset.**
Decision it feeds: `engine/net` + `engine/page` resource model.
Investigate: `<link rel=stylesheet>`/script/image fetch scheduling and ordering;
gzip/br/deflate crate choice + bloat; charset sniffing (the WHATWG encoding
standard; `encoding_rs` — the Gecko crate); data/blob URLs.

### PASS-1 findings (2026-07-10) — Axis D amendment only (Custom Elements/Shadow DOM; SSR/History API)

*Scope note:* this pass covers **only** the two items the framework-rendering note names
as "genuinely not covered by general DOM/CSS work". D1–D4 are untouched here.
WPT pinned at **`7cbfe0be66b3b2c228cf5da70ed3cbeb30b214b3` (2026-07-10)**.

**D-amend-1 — Custom Elements + Shadow DOM. Split the item: DSD is parser work and lands
now; imperative custom elements gate on D3.**
*Finding (primary source — `html5ever` 0.39 `src/tree_builder/mod.rs:1484`, and
`markup5ever` 0.39 `interface/tree_builder.rs:272`):* html5ever **already implements the
Declarative Shadow DOM parsing rules**. The tree builder checks
`attr.name.local == local_name!("shadowrootmode")` on a `<template>` start tag and calls
two `TreeSink` hooks: `allow_declarative_shadow_roots()` (**defaults `true`**) and
`attach_declarative_shadow()` (**defaults `false`**). We consume the tree through
`markup5ever_rcdom::RcDom`, which **does not override `attach_declarative_shadow`** — so
today a `<template shadowrootmode="open">` is parsed as an ordinary `<template>` and the
shadow root is silently never attached. This is a *reuse* opportunity mis-wired, not a
missing capability.
*Finding (why DSD is separable):* Declarative Shadow DOM is **parser-level and requires no
JavaScript** (`<template shadowrootmode>`, standard since Chrome/Edge 111 for the
`shadowrootmode` attribute; the older `shadowroot` name is non-standard). Imperative
`customElements.define()` + `Element.attachShadow()` are JS APIs and therefore sit behind
D3's binding layer. Scoped custom-element registries (`shadowrootcustomelementregistry`)
are shipping but are a strictly later layer.
*Finding (reference implementation):* Servo has shadow DOM with `<slot>` fully supported
including layout, plus the `::slotted()` selector — so the style-side scoping is a solved
problem with a Rust precedent to follow. Note our Stylo trait wall already stubs exactly
these: `parent_node_is_shadow_root`, `containing_shadow_host`, `is_html_slot_element`,
`assigned_slot` all return `false`/`None` (documented in `engine/css/src/stylo_dom.rs`).
*Disposition:*
- **DSD: hand-roll the `TreeSink` over the arena DOM** (reusing html5ever's tokenizer +
  tree builder, which already do the hard part). This *also* deletes the `RcDom` →
  arena copy that `engine/html` performs today — a strict win independent of shadow DOM.
  Requires a real shadow-root node kind in `manuk-dom` and slot assignment.
- **Style scoping:** flat-tree traversal + `::slotted()`, following Servo. Our existing
  `selectors::Element` stubs are the exact insertion points.
- **Imperative `customElements.define` / `attachShadow`: DEFER behind D3.** They need the
  JS binding layer, custom-element reactions, and the upgrade/reaction queue, which is a
  distinct spec area.
- **Scoped registries: out of scope for this pass.**
*Roadmap edit:* CLAUDE.md §D3 — add "Declarative Shadow DOM is D1/parser work, not D3
work"; §D2 — the four shadow stubs in `stylo_dom.rs` become real once a shadow node kind
exists.
*New WPT targets (@ `7cbfe0be`):* `shadow-dom/` (declarative + slots + `::slotted`);
`custom-elements/` (deferred, post-D3). Current pass count: **0** for both (no runner
integration yet, see P0.3).

**D-amend-2 — SSR/hydration mechanics (`<script type="module">`, dynamic `import()`,
History API). Order is the finding: History API first, module scripts second, dynamic
`import()` blocked.**
*Finding (the one that changes priorities):* **server-rendered HTML renders without any
JavaScript** — that is what SSR *is*. Hydration is React/Svelte attaching event handlers to
already-present markup. So a spec-compliant engine with **no** module support still
*displays* Next.js / SvelteKit / Nuxt SSR pages correctly; what breaks is *interactivity*
and *client-side routing*, not rendering. This directly contradicts the intuition that
module scripts are the prerequisite for "supporting Next.js", and it re-orders the work.
*Finding (History API — WHATWG HTML §7.2, nav-history-apis):* `pushState()` creates a new
session-history entry and `replaceState()` mutates the current one; both **update the
document URL without fetching or loading anything**, and **do not fire `popstate`** —
`popstate` fires only on history *traversal*. Both are restricted to **same-origin** URLs.
Next.js's own docs state that native `window.history.pushState`/`replaceState` "integrate
into the Next.js Router" and sync with `usePathname`/`useSearchParams` — i.e. frameworks
call the *platform* API, they do not replace it. Consequence: `history.*` is **pure host
state**, with **zero SpiderMonkey dependency** beyond binding the methods. We already own
a session-history model twice over (`AgentBrowser::history`, `shell::chrome::History`) —
this is a unification, not new machinery.
*Finding (module scripts — SpiderMonkey JSAPI, mozilla-spidermonkey/embedding-examples):*
module loading is entirely **host-driven**: the embedder implements
`JS::SetModuleResolveHook` (now taking a `ModuleRequest` object, with
`JS::GetModuleRequestSpecifier` to read the specifier), compiles with `JS::CompileModule`,
links, and calls `JS::ModuleEvaluate` — which since top-level-await gained a
`MutableHandleValue` **out-parameter carrying the Promise**. Dynamic `import()` needs
`SetModuleDynamicImportHook` and **resolves a Promise**.
*Blocker, already known and now confirmed relevant:* our D3 work recorded that mozjs 0.18's
**promise job-queue wrapper segfaults on call** (`JS::UseInternalJobQueues`/`RunJobs`), which
is why native `Promise.prototype.then` reactions are not routed through our event loop.
Because `ModuleEvaluate` returns a Promise and dynamic `import()` *is* a Promise, **dynamic
`import()` cannot resolve without the job queue**. Static `<script type="module">` with
top-level-await absent may still work (the returned Promise is already-resolved), but this
is unverified.
*Disposition:*
1. **History API — implement now.** Host-only, `[XP]`, no engine risk. Bind
   `history.pushState/replaceState/length/state`, `popstate`, `hashchange`, and the
   same-origin check, over a single unified session-history model shared by shell + agent
   (this also retires the duplicated stacks). Gated on the existing `spidermonkey` feature
   only because the *bindings* need JS; the model itself is not.
2. **`<script type="module">` — implement after (1), with a spike first.** The spike is
   whether mozjs 0.18 exposes `CompileModule`/`ModuleEvaluate`/`SetModuleResolveHook` in
   safe Rust at all. Sanctioned embedding API; **does not touch JIT/GC/sandbox**.
3. **Dynamic `import()` — DEFER, blocked on the promise job queue.** The follow-up is
   *research*, not implementation: does a newer `mozjs` fix the `RustJobQueue` crash, or do
   we implement custom `JobQueueTraps`? Nothing should be built until that is answered.
*Roadmap edit:* CLAUDE.md §D3 — record that SSR pages already render; that History API is
host-only; and that dynamic `import()` inherits the promise-job-queue blocker.
*New WPT targets (@ `7cbfe0be`):*
`html/browsers/history/the-history-interface/` (History API);
`html/semantics/scripting-1/the-script-element/module/` (module scripts, post-spike).
Current pass count: **0** for both.

### PASS-2 findings (2026-07-10) — N7 research pass: the promise job queue (unblocks dynamic `import()`)

Executed as IMPLEMENTATION.md N7 specifies: **research only, no implementation.** Its three
questions are now answered, two of them by direct experiment against the code in-tree.

**(a) Does a newer `mozjs` fix the `RustJobQueue` crash? — NO, and the recorded diagnosis
was incomplete.**
*Experiment:* a probe calling `mozjs::rust::wrappers2::UseInternalJobQueues(cx)` on a
`Runtime::new(...)` context **SIGSEGVs inside the call**, reproducing the D3 note.
*Root cause (primary source — `mozjs-0.18.0/src/rust.rs:355-390`):* `Runtime::create` runs
`JS_NewContext` → … → **`InitSelfHostedCode`** unconditionally, and SpiderMonkey requires
`js::UseInternalJobQueues(cx)` to be called **before** self-hosted code is initialized (the
promise machinery captures the queue at that point). mozjs exposes **no hook between the
two**, so the call always arrives too late. This is an **API-shape** problem, not a version
bug: `mozjs` `main`'s `rust.rs` still calls `InitSelfHostedCode` in `Runtime::create`, and
0.18.1 is unchanged. Bumping the version cannot fix it. *The wrapper signature is fine* —
raw jsapi and the safe wrapper agree on `fn(*mut JSContext) -> bool` — so the earlier
"wrapper segfaults" framing was a symptom, not the cause.

**(b) Can we implement a custom `JobQueue` from Rust? — YES, and it is the right path.**
*Primary source:* `mozjs_sys-140.12.0-1/src/jsglue.cpp:45` defines `struct JobQueueTraps`
(`getHostDefinedData`, `enqueuePromiseJob`, `runJobs`, `empty`, plus the interrupt-queue
trio) and `class RustJobQueue : public JS::JobQueue`, with factory functions
`CreateJobQueue` / `DeleteJobQueue` (`jsglue.cpp:1153`). `mozjs::rust::wrappers2::SetJobQueue(cx, *mut JobQueue)`
is in the safe wrapper list.
*Verified by compilation, not assumption:* `mozjs::glue::JobQueueTraps: Default` and
`mozjs::glue::CreateJobQueue: unsafe extern "C" fn(*const JobQueueTraps, *const c_void, *mut c_void) -> *mut JobQueue`
both resolve and type-check from safe Rust today.
*Boundary check (CLAUDE.md, non-negotiable):* this is `mozjs_sys`'s **embedding glue** and
the public `JS::JobQueue` interface — the **same hook every browser uses** to integrate
promise jobs into its own event loop. It does **not** touch SpiderMonkey's JIT, GC, or
sandbox, and requires no patch to SpiderMonkey. It is therefore inside the boundary, in the
same class as `JS_DefineFunction`.

**(c) Does `mozjs` expose the module hooks? — YES, all of them.**
`SetModuleResolveHook`, `SetModuleDynamicImportHook`, `SetModulePrivate`,
`GetModuleRequestSpecifier`, `FinishDynamicModuleImport` are present in the generated raw
`jsapi` (the sanctioned unsafe embedding surface); `CompileModule` / `CompileModule1`,
`ModuleLink`, and `ModuleEvaluate` are in `mozjs`'s **safe** wrapper list
(`jsapi2_wrappers.in.rs:251-256`). `ModuleEvaluate` takes the top-level-await
`MutableHandleValue` out-parameter, as the JSAPI docs describe.

**Disposition — the blocker is lifted, and the order is now determined.**
1. **Implement a custom `JobQueue`** (`JobQueueTraps` + `CreateJobQueue` + `SetJobQueue`)
   whose `enqueuePromiseJob` pushes onto the microtask queue `engine/js::event_loop`
   already owns, and whose `runJobs` is our existing drain. This **also retires the
   standing D3 gap** that native `Promise.prototype.then` reactions are not routed through
   our loop — a strictly larger win than dynamic `import()` alone.
2. **Then** `<script type="module">` (compile → link → evaluate, with a resolve hook over
   the fetch layer).
3. **Then** dynamic `import()` (`SetModuleDynamicImportHook` + `FinishDynamicModuleImport`),
   which needs (1) because it resolves a Promise.

Recorded as **N9** in IMPLEMENTATION.md. N7 is **closed**: its acceptance was a written
disposition backed by a primary source, and no implementation was performed under it.

## Axis E — User-facing features (headful parity + polish)

For each, the research question is the same shape: **what's the leanest correct
implementation, and what (if anything) is worth hand-rolling vs. reusing a crate** —
measured against the "every hand-roll is maintenance surface" bar.

**E1. Chrome UI essentials.** Tabs/windows/tab-groups, search/URL bar with
suggestions, find-in-page (how to do incremental find over the fragment/text tree
we already build), zoom (layout reflow vs. transform scale — tradeoffs), back/
forward/refresh/history, bookmarks, a settings model that isn't bloated. For each:
does our existing engine architecture (fragment tree, `visible_text`, `links`)
already give us the primitive, or is new engine surface needed?

**E2. Local encrypted password store + autofill.** This is a **security-critical**
block — treat findings with the same seriousness as the JS boundary.
Investigate: the actual cryptographic design (how Firefox/Chromium derive keys,
OS-keychain integration vs. app-managed master key, at-rest encryption — `age`/
`ring`/`RustCrypto` primitives); autofill's phishing/origin-matching rules (an
autofill that fills the wrong origin is a real vulnerability); do NOT hand-roll
crypto primitives — reuse audited crates, hand-roll only the orchestration.

**E3. Translate-page.** Investigate: the fully-local option (a bundled MT model vs.
calling out to an API — privacy tradeoff), how the "bring your own endpoint" pattern
(E6) could serve this, DOM text-node extraction + reinjection preserving structure.

**E4. Developer tools / console.** Investigate: whether to implement the Chrome
DevTools Protocol (CDP) — which would also make the *agent* side drivable by
existing Puppeteer/Playwright tooling, a real strategic two-birds win worth
weighing — vs. a bespoke inspector; what the minimal useful DevTools surface is
(DOM inspector, console over the SpiderMonkey runtime, network panel over
`engine/net`).

**E5. Extensions.** Investigate honestly: WebExtensions API surface is enormous —
is *any* level of compat realistic, or is it a scope trap? Is there a lean subset
(content scripts + a few APIs) that covers the high-value cases (blockers, password
managers) without the full surface? What does the security model require (extensions
are a major browser attack surface)? Default skepticism — justify inclusion.

**E6. Bring-your-own-key in-browser AI.** This is a **blue-ocean** candidate (see
Axis G), but also a shippable headful feature: a side-panel that talks to a
user-configured OpenAI-compatible endpoint about the current page, and can drive the
browser. Note it reuses the exact `InferenceBackend` abstraction the agent side
already has — the headful AI panel and the headless agent are the *same core* with
different front-ends, mirroring the whole project's thesis. Investigate: the
security model (a page-reading, browser-driving AI panel is powerful — what's the
permission/consent design), prompt-injection from page content into the panel (real
and serious), and how Anthropic's and Google's in-browser assistants scope
capability.

**E7. Profiles / containers / VPN.** Investigate: profile isolation (separate
cookie/storage/history jars — an engine-storage question); container-tab style
per-tab identity isolation; whether "VPN" means bundling a proxy/WireGuard client
(scope-heavy) or just honoring a system/SOCKS proxy (lean) — recommend the lean
framing unless there's a strong reason.

## Axis F — Legitimate-request fingerprinting (bounded, human-operator only)

**Read the constraint first: this axis is ONLY about the headful human browser
making legitimate, non-scraping, non-adversarial requests, and is ONLY about not
being *wrongly* rejected as a bot when a real human is driving. It is NOT about
evading detection for the agent path or for scraping. If a finding's mechanism is
indistinguishable from scraping evasion, it is out of scope — the honest framing is
"present as the real, standards-compliant browser we are," not "impersonate
Chrome."**

**F1. Why non-Chromium engines get wrongly walled.**
Investigate: the actual signals sites/CDNs (Cloudflare, Akamai, PerimeterX) use —
TLS ClientHello (JA3/JA4), HTTP header ordering, `User-Agent`, JS-environment
probes; which of these a legitimate browser can present *honestly* and correctly
(e.g. a truthful modern UA, correct header ordering, a spec-complete JS environment)
vs. which cross into impersonation. Draw the line explicitly.
Decision it feeds: which honest-presentation fixes belong in `engine/net` and the JS
environment, and which requests are simply out of scope because serving them would
require dishonest impersonation.

**F2. The graceful-degradation path.**
Investigate: for sites that hard-wall non-mainstream engines, what the *respectful*
fallback is (clear user-facing "this site blocks this browser" rather than an
escalating evasion arms race). This reframes the "hostile tier" from an engineering
problem to a UX-honesty problem.

## Axis H — Unified automation/tool surface (headful panel + headless agent)

One canonical action/tool schema, consumed by both the headless `agent`
binary's `run_task` loop and the shell's in-browser agent panel (Axis E6) —
the panel applies a permission filter on top of the same primitives rather
than defining a second schema. This consolidates several items already
scattered separately in CLAUDE.md's agent frontiers (viewport-clipped
observation, richer actions, an accessibility/semantic tree) into one
coherent design rather than landing them as independent, potentially
inconsistent additions.

**H1. Targeting mechanism.**
Decision it feeds: how `observe()`/`click` generalize beyond link-index-only.
Investigate: extending the indexed-element set to all interactive elements
(buttons, form fields), not just anchors — and its direct overlap with an
accessibility/semantic tree (one investment, two payoffs: agent grounding and
real human a11y). Also scope coordinate-based clicking as a *second*, not
replacement, targeting mode — some multimodal models are trained specifically
for pixel/visual grounding from a screenshot and shouldn't be forced into an
indexed-list convention that fits smaller models better.

**H2. Observation richness scaling by model capability.**
Decision it feeds: whether observation detail is a fixed schema or a
harness-set parameter.
Investigate: what a small model (Axis E2's Gemma-4-E4B/Qwen3.5-4B tier) needs
versus what a larger, more capable model could productively consume (fuller
a11y tree, less-clipped screenshot) without degrading reliability at the small
end. Same underlying engine data, different serialization depth depending on
what's driving it — not two implementations.

**H3. Action-set completeness.**
Decision it feeds: which actions beyond `navigate`/`click`/`scroll`/`finish`
are load-bearing enough to prioritize first.
Investigate, per candidate action: typing into form fields, form submission,
back/forward, wait-for-load/wait-for-element, tab management (new/close/
switch) — the last of which is also directly relevant to Axis E1's headful
tab UI, another point of shared surface worth building once.

**H4. Permission-scoping mechanism (panel-specific).**
Decision it feeds: how the shell panel's "task-scoped, not the full agent
action space" constraint (Axis E6) is actually implemented — an allowlist
per invocation, a capability token, something else. Must compose cleanly with
H1-H3 rather than becoming a second schema.

### PASS-1 findings (2026-07-10) — Axis H

*Context that changes the questions:* H1 and H3 were **substantially answered by
implementation** during the Phase 3–5 execution (`§4a` accessibility tree +
`§4b` action set + `E4` BiDi remote end). The research below therefore names the
**residual delta** rather than re-deriving settled ground.

**H1 — Targeting mechanism. RESOLVED: adopt WebDriver BiDi's targeting vocabulary;
both modes, not one.**
*Finding (primary source — w3c/webdriver-bidi `index.bs`, CDDL §7.3.2.3, §7.9.2.1):*
the spec's own locator set is
`Locator = AccessibilityLocator / CssLocator / ContextLocator / InnerTextLocator /
XPathLocator`, and **`AccessibilityLocator = {type:"accessibility", value:{?name, ?role}}`**
— i.e. the standard's canonical element-targeting strategy is keyed on **role + name**,
exactly the shape `manuk_a11y::A11yNode::find(role, name)` already implements. Node
identity across the protocol is `script.SharedReference = {sharedId, ?handle}`, a stable
opaque handle, **not** an ordinal index. Separately,
`input.Origin = "viewport" / "pointer" / input.ElementOrigin` — so coordinate-space and
element-space clicking are *co-equal modes of one action*, settling the "second, not
replacement" question in the affirmative at the spec level.
*Finding (model side):* the literature is unambiguous that both modes are load-bearing.
Set-of-Marks / AxTree ID-selection is a distinct tradition whose known failure modes are
long textual context, hallucination, and absent visual/positional cues; the frontier of
GUI grounding (UI-TARS, MolmoPoint) instead emits **pixel coordinates** directly, with
MolmoPoint reporting SOTA-among-open GUI pointing (61.1 ScreenSpotPro) precisely by
pointing at image features rather than selecting a widget id. BrowserGym exposes **both**
`click(bid)` and `mouse_click(x, y)`. Forcing pixel-grounded models into an indexed-list
convention would discard the capability they were trained for.
*Disposition:* **reuse the BiDi vocabulary; do not invent a third schema.** Keep the two
targeting modes we have (`click_text{role,name}`, `click_at{x,y}`), and close the residual
delta: (a) a **stable `sharedId` node handle** so a target resolved once can be reused and
so E4 can honour `script.SharedReference` / `input.ElementOrigin`; (b) a
`browsingContext.locateNodes` implementation over the a11y tree (`accessibility` locator)
and `manuk_css::query_selector_all` (`css` locator). `xpath` is **out of scope** (no XPath
engine, and no evidence agents need it).
*Roadmap edit:* CLAUDE.md §D3/§4a — record that role+name is the *standard's* locator, not
a Manuk convention, and that `sharedId` is the missing identity primitive.
*WPT target:* none directly (BiDi conformance lives in `webdriver/tests/bidi/`, already
E4's target).

**H2 — Observation richness scaling. RESOLVED on the main question; one sub-question
deferred.**
*Finding (primary source — BrowserGym / AgentLab, the reference research harness):*
observation is **already a flag set, not a fixed schema**: `use_html`, `use_axtree`,
screenshot, Set-of-Marks, plus `extract_center` / `extract_box` / `extract_visible_tag`
augmentations, and configurable history depth. Every element carries a unique `bid`,
bounding box, and visible/clickable flags — a **superset of our `A11yNode{role, name,
bbox}`**, over *one* engine data structure. This directly validates the axis's framing
("same underlying engine data, different serialization depth — not two implementations").
*Finding (what actually degrades reliability):* increasing context length **degrades
solving accuracy even when the necessary information is present in the prompt**, with
agents showing gradual and sometimes sharp degradation past ~100K tokens. An AxTree is
~10× smaller than the DOM but still routinely runs to tens of thousands of tokens.
FocusAgent shows a task-guided retriever cutting observation size **>50%**, after which
**two small models reach 51.5 / 51.8% SR versus 53.0% for a large model on the full tree**
— i.e. pruning closes most of the small-vs-large gap.
*Disposition:* **harness-set parameter over one engine data structure** — confirmed, not
deferred. Crucially, the evidence says the knob should be keyed on a **token budget +
task-relevance**, *not* on a model-capability enum: there is **no evidence that a richer
observation makes a larger model better** at web tasks, while there is direct evidence
that more context makes *every* model worse. So: one `ObservationPolicy` (depth flags +
token budget), lean defaults, viewport-clipping already in place.
*Deferred sub-question (insufficient evidence within budget):* whether depth should be
**auto-tuned** per model (capability detection / adaptive escalation on failure). The
literature evaluates fixed configurations and task-guided pruning; it does not establish
an auto-tuning policy, and inventing one here would be a confident-sounding guess. Needs a
follow-up pass with an actual eval harness before any implementation.
*Roadmap edit:* CLAUDE.md §4 — replace "richer observation for bigger models" framing with
"one policy object, budget-keyed, lean default".
*WPT target:* none (not a web-platform feature).

**H3 — Action-set completeness. RESOLVED: the canonical set is BiDi's, and two of the
candidates are not actions at all.**
*Finding (primary source — `index.bs` CDDL):* the `input` module's primitives are
`PauseAction`, `KeyDownAction`/`KeyUpAction`, `PointerDownAction`/`PointerUpAction`/
`PointerMoveAction`, wheel/scroll, `SetFiles`, `ReleaseActions`. Navigation, history and
tabs live in `browsingContext`: `navigate`, `traverseHistory`, `create`, `close`,
`activate`, `getTree`, `locateNodes`, `handleUserPrompt`. Two candidates from the axis
prompt are **not** separate actions in the standard:
- **wait-for-load** is a *parameter* of navigate —
  `browsingContext.ReadinessState = "none" / "interactive" / "complete"`.
- **wait-for-element** is `browsingContext.locateNodes` (poll a locator), not an action.
*Finding (agent-facing vocabulary):* BrowserGym's high-level set — `click`, `fill`,
`select_option`, `hover`, `press`, `scroll`, `new_tab`, `tab_close`, `tab_focus`,
`go_back`, `go_forward` — maps onto the above with no residue.
*Status against what we shipped (§4b):* `type`, `submit`, `back`, `forward`, `scroll_to`,
`click_text`, `click_at` are landed. **Residual, in priority order:** (1) `wait` readiness
on navigate (currently we always behave as `complete`); (2) **tab management**
(`new`/`close`/`switch`) — shared surface with E1's headful tab UI, per the axis, and the
one item that pays twice; (3) `select_option` (our `<select>` is read-only in `forms.rs`);
(4) `press`/key actions; (5) `hover`; (6) `handleUserPrompt` (dialogs). `SetFiles` is
**out of scope** (file upload is a deliberate non-goal for an autonomous agent).
*Roadmap edit:* CLAUDE.md §4b — record wait-for-load as a navigate parameter, and tab
management as the shared E1/§4b item.
*WPT target:* none directly; BiDi conformance via `webdriver/tests/bidi/`.

**H4 — Permission scoping. RESOLVED: a per-invocation capability set enforced at the
action boundary; not a bearer token, and not a second schema.**
*Finding (CaMeL, Debenedetti et al., arXiv:2503.18813):* the design that actually defeats
prompt injection attaches **capabilities as metadata to values** (data provenance) and
enforces **security policies at each tool call**, with a privileged LLM producing control
flow from the *trusted* query and a quarantined LLM never touching tools. It solves 77% of
AgentDojo tasks *with provable security* versus 84% undefended. The enforcement point is a
check **before each tool call** — i.e. a filter over the *existing* action schema, which is
exactly the composition property H4 demands.
*Finding (why not a token):* macaroons (Birgisson et al., NDSS/CCS 2014) give attenuable
bearer credentials via chained HMACs, whose *raison d'être* is **decentralized delegation
without a round-trip to an issuer** — a holder attenuates its own authority for another
holder across a trust boundary. In-process, where the shell constructs the agent loop
directly and no untrusted party ever holds the credential, an HMAC-chained bearer token
buys nothing over a plain Rust value; it would add crypto surface for zero threat-model
gain.
*Disposition:* **per-invocation `Capabilities` value** — an allowlist of permitted action
kinds plus an origin allowlist and a `allow_sensitive_actions` bit — passed into `run_task`
and consulted by the *existing* `assess_action` guard, which already sits at the CaMeL
enforcement point. The panel (E6) simply constructs a narrower `Capabilities` than the
headless binary. **Macaroon-style attenuation is deferred until authority actually crosses
a process boundary** (out-of-process agent, or agent-to-sub-agent delegation) — at which
point attenuation, not authentication, is the property we would be buying.
*Roadmap edit:* CLAUDE.md §E6 — record the capability object as the panel's scoping
mechanism and note that `assess_action` is already the enforcement point.
*WPT target:* none (not a web-platform feature).

## Axis G — Blue-ocean capabilities (novel to both front-ends)

The prompt here is generative, not just survey: **what can a browser whose engine
and agent share one core do that Chromium+bolt-on-extension cannot?** For each idea,
assess: is it real utility or a demo; what engine surface it needs; is it uniquely
enabled by our architecture.

Seed directions to investigate and pressure-test (add more):
- **Unified human+agent session:** a human browses, then hands the *same live
  session* (cookies, scroll, DOM state) to the agent to continue a task, and takes
  it back — because both sit on `engine/page`. Does any mainstream browser do this
  cleanly?
- **Local-first semantic history:** every page the human visits is already parsed to
  a clean DOM/text by our engine — a private, local, searchable semantic index of
  *actually-read* pages (not CommonCrawl, not cloud), queryable by the in-browser AI.
  Ties to the earlier "content-addressed cache with volatility scoring" idea.
- **Agent-assisted accessibility:** the semantic/accessibility tree the agent needs
  (Axis D3) is also an a11y win for human users — one investment, two payoffs.
- **Deterministic replay / provenance:** because the agent path is deterministic and
  display-free, agent actions can be recorded and replayed exactly — auditable
  browsing for compliance/research use.
- **Instant per-tab resource honesty:** surface real CPU/RAM per tab to the user
  (the compositor already models tiers) — directly serving the "why is my fan
  spinning" pain the whole project started from.

For each: does it need engine work we're already doing anyway (best), or net-new
surface (justify it)?

---

## Output format for each pass

A research pass is done when it produces, for its block:
- **Findings** (primary-source-backed, dated — the web moves).
- **Disposition per item**: reuse crate X / hand-roll because Y / defer on Z / out
  of scope.
- **The specific `CLAUDE.md` roadmap edit** it implies (which section, what line).
- **Any new WPT target** (subdirectory + current-vs-target pass count) where relevant.

Findings that don't change a decision, restate a standing constraint, or propose
re-opening a settled non-goal do not belong in the output.