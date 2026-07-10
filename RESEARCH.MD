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