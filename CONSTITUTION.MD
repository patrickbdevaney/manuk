# MANUK CONSTITUTION
## The Long-Horizon Governing Document: Invariants, Progression Tree, and Speciation Law

**Status:** Governing document. Sits above CLAUDE.md and README.md; those implement this, and where they conflict, this wins. RESEARCH.md feeds this; it does not override it.

**One-sentence vision:** Manuk is the memory-safe, agent-native web runtime — one shared Rust core that is simultaneously a good-enough headful daily browser for humans and the categorically best drivable headless engine for agents, from which hardened, embedded, enterprise, and wallet-native species fork freely.

---

## PART I — INVARIANTS (never violated, at any horizon)

These are constitutional. Every feature, fork, and refactor is checked against them. A change that violates an invariant is rejected regardless of how attractive it is.

**I1. One core, two front-ends, N species.** A single engine core (net / html / dom / css / layout / text / js / paint / compositor / page) drives both `shell` (headful GUI) and `agent` (headless). All future species fork from this monorepo core. No species ever forks the core itself into divergence — species diverge at the integration layer, build flags, and dependency selection, never by patching shared engine internals per-species.

**I2. Memory-safe Rust DNA, sanctioned dependencies only, never patched internally.** The browser's own code is Rust. The sanctioned FFI set (SpiderMonkey/mozjs, Stylo, Taffy, tiny-skia, wgpu, Tokio, Rayon, hyper/rustls, html5ever) is adopted, tracked upstream, and never forked or patched. The SpiderMonkey modification boundary is absolute: JIT/GC internals and sandbox are never touched — config, FFI, and snapshot tuning only.

**I3. The agent surface is core, not a layer.** The synchronous in-process semantic model — DOM, computed style, layout geometry, and a first-class accessibility tree — is a load-bearing engine subsystem built and maintained at every horizon, not a feature bolted on later. This is the single most durable moat (incumbents are structurally committed to CDP and cannot retrofit it without their own rewrite), so it is never allowed to rot, lag the renderer, or become "the automation module." Every renderer subsystem lands with its semantic-model exposure or it is not done. The AX tree serves agents and accessibility-dependent humans in the same architecture — one substrate, two constituencies.

**I4. Pareto discipline is law.** The target is representative real-web breadth weighted by actual usage — document web, SPA/framework web, social/platform surfaces, high-traffic utility — verified by the differential oracle plus WPT (~83% subtest bar for Horizon 0), with honest graceful degradation on the exotic tail. Per-site pixel-perfection, exotic writing-mode completeness, and Chromium-quirk chasing are constitutionally out of scope. The industry's over-investment in tail coverage is a trap Manuk deliberately avoids; that avoidance is itself part of the opportunity. The hostile bot-walled tier (LinkedIn, X, Indeed-class) is excluded from the compatibility mission.

**I5. The differential oracle is the discovery engine.** Same page rendered in Manuk and headless Chromium, diffed, divergences clustered by root cause — this is the primary mechanism for finding what to build next, at every horizon. It is maintained as first-class infrastructure, because it is also a future product (Horizon 2 fan-out).

**I6. Page content is untrusted input, always.** Everything that crosses from the web into the agent surface — DOM text, AX labels, tool descriptions, WebMCP tool manifests — is adversarial by default. Prompt injection is treated as unsolvable in the general case (OpenAI's own public position); Manuk's answer is architectural: constrained agent interfaces, task-scoped action permissions, explicit consent gating for consequential actions, and a hard separation between what a page can *say* and what an agent is *authorized to do*. This invariant is what makes the "trustworthy agent substrate" positioning credible against Comet/Atlas-class competitors whose failure rates are public.

**I7. Honest walls, stated plainly.** No Widevine/EME HD video (permanent licensing wall). Process-per-tab is leaner but less isolated than per-origin Site Isolation. SpiderMonkey trails V8 on peak JS. These are documented trade-offs shown to users, not hidden — transparency about walls is part of the trust moat.

**I8. Solo + AI-loop throughput is the build model.** Claude Code plus the differential oracle is the engineering engine. Scope decisions always favor the path that keeps a single operator's loop fast: adopt crates aggressively, hand-roll only where tight integration demands it (the semantic model), and never take on a subsystem whose maintenance treadmill exceeds the loop's demonstrated throughput.

---

## PART II — THE PROGRESSION TREE

The trunk is strictly ordered: each horizon has entry conditions (the prior horizon's exit gate) and exit gates (measurable, binary). Branches (species and appanages) hang off the trunk at defined attachment points and activate on triggers, not on calendar. Timelines are deliberately omitted — the stated position is that Pareto parity, the agentic surface, and speciation all land faster than outside estimates assume; the gates order the work, the loop's throughput sets the pace.

```
TRUNK
 H0  Pareto Web Parity ──────────────── the engine is real
  │
 H1  Production + Security Hardening ── the engine is trustworthy
  │
 H2  Full Agentic Surface ───────────── the engine is the wedge   ──┬─► QA Oracle product (fan-out, near-zero delta)
  │                                                                 └─► Headless agent-infra packaging (the commercial lead)
 H3  Appanages ──────────────────────── the engine grows organs
  │     ├─► Native wallet (EIP-1193/6963) + x402/CDP nanopayments
  │     ├─► Consumer prompt-to-action (thin shell layer)
  │     └─► Privacy hardening (fingerprint surface, zero telemetry)
  │
 H4  Speciation ─────────────────────── the engine becomes a genus
        ├─► V8 security-max build (per-origin isolation + V8 sandbox)
        ├─► Embedded / kiosk / IoT (SpiderMonkey, lean)
        ├─► Enterprise auditable / compliance-mandate build
        └─► Memory-lean vs. isolation-granularity fork pair
```

### HORIZON 0 — Pareto Web Parity (the engine is real)

**Objective.** Manuk renders the representative real web, headful and headless, from the same core. Both the human and the agent need this identically — the browser must function before anything downstream matters.

**Scope, in build order:**
1. **Layout completion beyond Taffy** — full block/inline/table/float box model with **double-dirty-bit incremental relayout designed in from day one** (this is the single highest-leverage architectural decision in the renderer; retrofitting incrementality is a rewrite).
2. **Stylo promotion** — graduate the Stylo cascade from feature flag to default; retire the minimal cascade.
3. **Retained display lists + Vello/wgpu paint path** — wire the GPU path; keep tiny-skia as the CPU raster fallback and the embedded-species default.
4. **JS↔DOM bindings hardening** — SpiderMonkey GC rooting across FFI is the largest unsafe surface in the codebase (Servo's documented experience); it gets lint-plugin-grade tooling and the strictest review discipline in the repo.
5. **Web API surface by usage weight** — fetch, storage, canvas, events, history, then down the usage curve; the oracle's divergence clusters decide the order, not the spec's table of contents.
6. **Networking completion** — cache, cookies with SameSite partitioning, HTTP/2/3 via crates.
7. **Semantic model + AX tree land in lockstep** (Invariant I3) — every subsystem above ships with its in-process query exposure. AccessKit is the natural integration point.
8. **Text shaping / i18n to Pareto depth** — HarfBuzz-class shaping for the scripts the representative corpus actually contains; the exotic tail degrades gracefully per I4.

**Exit gate (all binary):**
- ~83% WPT subtest pass rate across categories.
- Differential-oracle-verified viability across all four usage-weighted corpora (document, SPA/framework, social/platform, high-traffic utility).
- The headful shell is daily-drivable by its own developer for real browsing.
- Every rendered construct is queryable through the in-process semantic API.

### HORIZON 1 — Production + Security Hardening (the engine is trustworthy)

**Objective.** Enterprise-grade production hardening: the engine is safe to run on hostile input, safe to update, and safe to recommend. This horizon exists because one high-profile RCE in an immature engine kills trust permanently — hardening precedes marketing.

**Scope:**
1. **Process-per-tab isolation completed and audited** — sandbox profiles per OS (seccomp/Landlock on Linux first, then macOS/Windows), least-privilege renderer processes, brokered file/network access.
2. **Continuous fuzzing as infrastructure** — parser, layout, bindings, and the FFI boundary under coverage-guided fuzzing in CI, forever. This is the cheapest possible substitute for the adversarial scrutiny incumbents have absorbed over decades.
3. **The agent-surface security model implemented, not just designed** (I6) — task-scoped permission grants, consequential-action consent gates, injection-resistant separation of page content from agent authority, and an auditable action log. Hardening the agentic surface is part of *this* horizon, ahead of productizing it.
4. **Operational maturity** — reproducible/deterministic builds, signed releases, an update channel, crash reporting that is opt-in and privacy-clean, CI across Linux/macOS/Windows.
5. **Security documentation** — a published threat model stating the honest walls (I7): per-tab not per-origin, no V8 heap sandbox on the SpiderMonkey base, and what the Rust DNA does and does not buy. This document is a sales asset for H4's enterprise species, written now.

**Exit gate:** fuzzing corpus running continuously with all known crashers fixed; sandbox profiles enforced on all three platforms; agent permission model demonstrably blocks a standard prompt-injection test suite from triggering unconsented consequential actions; reproducible builds verified.

### HORIZON 2 — Full Agentic Surface (the engine is the wedge)

**Objective.** Productize what I3 built: Manuk as the categorically best agent-drivable engine. This is the commercial lead — proven, fast-growing demand (agentic traffic up ~7,851% YoY per HUMAN Security 2026), incumbents structurally stuck on CDP, smallest delta from the core.

**Scope:**
1. **Stabilize the native driving API** — the synchronous in-process semantic query + action interface, versioned and documented as Manuk's primary public API. CDP/WebDriver-BiDi endpoint ships as a *compatibility* shim only, so existing Playwright-shaped workloads can migrate incrementally — but the native API is the product.
2. **Native WebMCP client** — implement `navigator.modelContext` as the first independent, non-Chrome implementation. This converts the largest structural threat (WebMCP commoditizing automation from above) into Manuk's native tongue, and positions Manuk as the reference client if the standard goes cross-browser.
3. **MCP-native integration** — the headless engine exposes itself as an MCP server (browser-as-tool for any agent framework) and consumes MCP where pages/tools offer it. This slots Manuk directly into the dominant agent-tool ecosystem rather than beside it.
4. **In-browser AI agent panel (shell-side)** — the already-planned headful agent panel, running on the same permission model as the headless surface. One security model, two front-ends.
5. **Local-model story completed** — the OpenAI-compat backend plus the bundled llama.cpp local-model manifest, making Manuk the only agent browser with a credible zero-cloud default.
6. **Publish the latency proof** — independently reproduce and publish the in-process-vs-CDP benchmark (the ~24µs vs ~295µs class of measurement, validated against browser-use's own ~100µs CDP round-trip figures). This benchmark *is* the marketing.

**Fan-outs at this attachment point (near-zero incremental delta):**
- **QA Oracle product** — the differential oracle exposed as a cross-engine visual/behavioral regression service; the only non-Chromium truth source in a market where Percy/Applitools/Chromatic all render on Chromium.
- **Headless agent-infra packaging** — the `agent` binary packaged for agent-infrastructure builders and AI labs: container images, a hosted-fleet-friendly footprint, and the native API SDK.

**Exit gate:** native API stable and documented; WebMCP client passing the origin-trial test surface; MCP server mode shipping; benchmark published and reproducible by third parties; at least one external agent-infra consumer running Manuk headless in anger.

### HORIZON 3 — Appanages (the engine grows organs)

These are thin, high-upside layers on top of a now-hardened core and agent surface. They are ordered by conviction-weighted delta, and each carries an honest maturity read.

1. **Native wallet + x402/nanopayments.** A chrome-level EIP-1193/6963 provider with key management architecturally outside any page's reach — this *structurally* defeats the extension-wallet attack classes (clickjacking, injection-race, overlay, header-stripping supply-chain compromise) that no extension wallet can escape. Alongside it: native HTTP-402/x402 handling and Coinbase Developer Platform integration, wired into the agent surface so agents can settle nanopayments under the same consent-gating model as every other consequential action (I6). **Constitutional stance:** the *interface* is built now — it is defensible, it compounds with the agent surface, and the conviction is that the rail is viable on the horizon. The *commercial bet* (marketing Manuk as a payments browser, dedicating loop-throughput to rail-specific features) activates on the demand trigger in Part III. Build the organ; don't bet the organism on the bloodstream maturing on schedule.
2. **Consumer prompt-to-action.** A genuinely thin layer: the shell agent panel + native driving surface already are the product; this adds task templating and UX polish. Constitutionally a *capability demonstration and optionality hold*, never the wedge — the consumer agentic-browser market is distribution-bound (Comet, Atlas, Dia, Chrome Auto Browse) and distribution is not a solo game. Ship it because it is nearly free and showcases the engine; do not fund a distribution war.
3. **Privacy hardening.** Zero telemetry by construction, no default search deal, minimized and documented fingerprint surface. A positioning lever across every species and a near-free differentiator for the security-hardened line; not a standalone moat (Brave/Mullvad/Tor occupy the pure-privacy market).

**Exit gate:** wallet + x402 interface functional end-to-end against testnet + CDP sandbox rails under agent consent gating; prompt-to-action demo-quality in the shell; fingerprint surface audited and documented.

### HORIZON 4 — Speciation (the engine becomes a genus)

The monorepo's forkability is now exercised deliberately. Species diverge at build flags and dependency selection per I1. Each species has a named buyer and an activation trigger (Part III).

1. **V8 security-max build.** The separate monorepo build swapping SpiderMonkey→V8, paired with **per-origin Site Isolation** and V8's heap sandbox. This is the species that resolves the constitution's central acknowledged tension — memory-leanness vs. isolation-granularity — by refusing to compromise: the base engine keeps process-per-tab leanness; the security-max species pays the memory cost for maximum isolation. The two builds are the two ends of the trade frontier, sold to different buyers. This is also the species that answers "but you don't have V8's sandbox" permanently.
2. **Embedded / kiosk / IoT (SpiderMonkey, lean).** tiny-skia CPU default, minimal API surface, small binary — the CEF alternative. SpiderMonkey stays here deliberately: the embedded buyer wants footprint and auditability, not JS peak. Servo's OpenHarmony work proves the category.
3. **Enterprise auditable / compliance-mandate build.** Audit logging, DLP hooks, and agent-action gating as *architecture* (which H1 already built) rather than the retrofit every Chromium-fork enterprise browser (Island, Talon/Prisma, LayerX) sells. Carries the memory-safety-as-compliance-artifact story into the CISA/ONCD regulatory tailwind. Heavy GTM — this species activates with a partner or revenue, not solo hope.
4. **Memory-lean vs. isolation fork pair.** Formalizes the trade as a supported build matrix: {process-per-tab lean, per-origin hardened} × {SpiderMonkey, V8}, with the lean-SpiderMonkey corner as the flagship base and the hardened-V8 corner as security-max. The other two corners exist only if a buyer names them.

---

## PART III — DECISION RULES AND ACTIVATION TRIGGERS

The trunk is calendar-free; the branches are trigger-gated. These rules are pre-commitments so future decisions are made by the constitution, not by mood.

**T1 — WebMCP goes cross-browser** (Firefox or Safari ships `navigator.modelContext`): immediately elevate WebMCP-reference-client positioning to the top of H2 messaging; the standard becoming universal makes Manuk's native implementation the independent reference and multiplies the wedge.

**T2 — x402 demand hardens** (machine-payment daily volume moves from the ~$28K/day gamed-transaction era into sustained millions of non-gamed volume, or a top-tier agent platform ships x402 settlement by default): activate the wallet/payments species commercially — marketing, rail-specific features, CDP partnership pursuit. Until T2, the interface ships but the company is not bet on it.

**T3 — A funded competitor ships a memory-safe agent-native engine first:** pivot positioning from "the engine" to "the best agent-drivability layer and semantic API," and accelerate the QA-oracle and WebMCP-client products where the moat is the toolchain, not the renderer.

**T4 — Enterprise pull materializes** (a named regulated-industry buyer or compliance mandate cites memory-safety procurement requirements at Manuk): activate the enterprise species with that buyer as design partner; do not build enterprise GTM speculatively.

**T5 — Embedded pull materializes** (a device OEM or kiosk integrator engages): activate the embedded species; until then it remains a build-flag-level capability, not a maintained product.

**T6 — Security incident in the wild** (any exploited Manuk vulnerability): all horizons pause; the fuzzing/hardening loop of H1 becomes the only work until root-caused, fixed, disclosed, and the class of bug is structurally prevented. Trust is the asset; everything else is downstream of it.

**Standing prioritization rule.** When two work items compete, prefer the one that (a) advances the current horizon's exit gate, then (b) strengthens I3 (the semantic model), then (c) reduces the oracle's largest divergence cluster. Novelty never outranks the gate.

**Standing scope rule.** Any proposed feature is tested against: which horizon does it belong to? If it belongs to a later horizon, it is filed in RESEARCH.md and not built. Pulling future-horizon work into the present is the primary solo-scale failure mode and is constitutionally forbidden without amending this document.

---

## PART IV — NON-GOALS (permanent, unless amended)

- **Spec parity.** Never the target. Servo and Ladybird have a decade's head start on breadth; Chromium/Firefox are unassailable there; and parity-chasing is the tail-coverage trap this project exists to avoid.
- **Widevine/EME HD streaming.** Permanent licensing wall. Documented, degraded gracefully, never chased.
- **The hostile bot-walled tier.** Out of the compatibility mission by prior decision.
- **Consumer distribution war.** Manuk may *have* a consumer prompt-to-action face (H3), but never funds defaults, deals, or acquisition spend. Engines win consumer share on distribution, not merit; that is not a solo game and not this game.
- **Out-optimizing V8's JS peak.** Accepted structural loss on the SpiderMonkey base; answered by the V8 species, not by heroics.
- **Patching sanctioned dependencies.** Ever (I2).

---

## PART V — THE END-STATE VISION

When the tree is grown, Manuk is a *suite*, not a browser:

- **Manuk** (flagship): the lean, memory-safe, headful-and-headless browser — a genuinely usable GUI daily driver at Pareto breadth, and the best agent-drivable engine in existence, with a native semantic API, WebMCP, MCP, local-model agency, and consent-gated autonomy.
- **Manuk Agent Infra**: the headless runtime sold to agent builders — the CDP-debt escape hatch, benchmarked and reproducible.
- **Manuk Oracle**: the cross-engine QA truth source no Chromium-rendering competitor can be.
- **Manuk Vault**: the wallet/x402-native species — the browser where keys live in the chrome and agents settle nanopayments under human-gated consent.
- **Manuk Sentinel**: the V8 + per-origin-isolation security-max species for the highest-threat buyers.
- **Manuk Embed**: the CEF-killer for devices.
- **Manuk Enterprise**: the auditable-by-architecture compliance browser riding the memory-safety mandate.

One core. Two front-ends. A genus of species. Every one of them possible only because the engine was built from scratch, in Rust, with the agent surface in its DNA — the three properties no incumbent can retrofit without becoming a from-scratch project themselves.

**Amendment rule:** this constitution changes only by deliberate revision with a stated reason recorded in the document's changelog — never by drift, never by an implementation session's convenience.
---

## PART VI — GROUND-TRUTH RECONCILIATION (2026-07-15, tick 86)

*The vision (Parts I–V) is unchanged. This part maps it onto the repository as it actually is, corrects
assumptions the vision made about the stack, and states the direct, measurable path to the H0 exit gate.
It is written by the loop, from the tree, and is re-derived — not remembered — whenever it drifts.*

### VI.1 — What is already built (the vision under-counted the stack)

| Constitution assumption | Ground truth (verified from the tree) |
|---|---|
| **I1** one core, N crates | ✅ real: `net html dom css layout text js paint compositor a11y page` drive `shell` (headful) + `agent` (headless). ~48k LOC. |
| **I3** "the AX tree lands later" | ⚠️ **already load-bearing.** `manuk-a11y` (1.3k LOC) builds a role + accessible-name + geometry + hit-test tree and **already feeds `manuk-agent`'s observation channel**. The moat is not a future task — it exists. Missing: the **AccessKit platform bridge** (screen-reader side). Correct the map: this is `partial`, not `missing`. |
| **I3** agent surface is core | ✅ real: `manuk-agent` is 8.9k LOC — `Observation`, semantic view, actionable-element model, observation policy, prompt generation. |
| **H0.2** "graduate Stylo from flag to default" | ⚠️ **already done for the shell.** `shell` defaults to `["gui","stylo","spidermonkey"]`. `MinimalCascade` remains the **headless / JS-less fallback** and is not obviously worth retiring (it is the dependency-lean test path). |
| **H0.4** GC rooting is the largest unsafe surface | ✅ confirmed the hard way — tick 84's iframe work was a multi-hour hunt through exactly this (a mass-reflector recursion overflowing the C stack; a per-arena cache clobbering `__nodes`). The strictest-discipline call in the vision is correct. |

### VI.2 — What is genuinely not done (the real H0 work)

| Constitution scope | Ground truth |
|---|---|
| **H0.1** layout beyond Taffy + incremental relayout | Layout is on Taffy 0.12; **no double-dirty-bit incremental relayout.** CSS layout breadth is the weak spot: `css-flexbox` **5.5%**, `css-grid` **4.7%** — nearly every real site needs these. |
| **H0.3** Vello/wgpu GPU paint path | **Aspirational comments only.** `manuk-compositor` (526 LOC) describes a tiered Vello-GPU renderer; the actual raster is **tiny-skia CPU**, everywhere. `wgpu 27` is declared and used only for shell windowing. |
| **I5** the differential oracle is the discovery engine | ⚠️ **never completed a full crawl** (`ORACLE_CRAWLED: 0` of 265). The constitution's *primary* discovery mechanism is not operational. Getting it to finish one clean crawl is high-leverage H0 infrastructure. |
| **H0.7** AccessKit integration | Not started (see VI.1). |

### VI.3 — The north-star correction (this governs the loop, per I4)

**The raw WPT subtest total is a Pareto trap, and the loop was walking into it.** Tick 84 moved
`encoding` 128 → 720,990, and the suite TOTAL 25,869 → 747,778 (47.6%). But **96% of every passing
subtest is now encoding**, and encoding's *remaining* ~767k failures are exotic per-codepoint tail —
precisely what **I4 says to degrade gracefully, not chase.** The honest, Pareto-relevant number is:

> **WPT breadth excluding the encoding spike: 26,788 / 82,861 = 32.3%.**  *This* is the H0 gauge, not 47.6%.

Consequences, binding on the loop:
1. **`WPT:TOTAL` is demoted from a north star to a bookkeeping mark.** The loop optimises **usage-weighted
   breadth**, not subtest count. An area that is 5% and used by every site (flexbox) outranks an area that
   is 48% and used by the pre-2010 CJK long tail (encoding).
2. **The aperture is the biggest lever, and it is barely open.** ~8 sub-areas of *hundreds* are measured.
   `css/*` (dozens of high-usage subtrees: values, position, text, fonts, backgrounds, transforms,
   cascade, …) and `html/*` beyond `html/dom` (semantics, canvas, browsers) are unmeasured. Open them
   before ranking, per the existing blindspot discipline — a ranking inside the wrong frame is confident
   and wrong.
3. **`orient` ranks by usage-weighted breadth**, degrading the exotic tail per I4. The encoding tail is
   **done for H0 purposes**; further encoding depth is anti-Pareto and constitutionally out of scope.

### VI.4 — The direct path to the H0 exit gate (leanest ordering, re-derived each cycle)

The H0 gate is four binary conditions (§H0). Mapped to the shortest path from here:

1. **Open the aperture across `css/*` and `html/*`** (blindspot → wpt-expand → sweep). One or two ticks;
   turns the unknown breadth into a ranked work-list. *Unblocks everything below.*
2. **CSS layout breadth** — flexbox / grid / position / text, in usage order. This is the largest
   Pareto-relevant failing mass and the H0.1 lever. The oracle's clusters refine the order.
3. **Get the oracle to finish one clean crawl** (I5) — the truest Pareto signal and the gate's second
   condition ("oracle-verified across the four corpora"). Its hang/crash floor is also Bar 0.
4. **Web-API surface by usage weight** (H0.5) — fetch, storage, canvas, events, history — ordered by the
   oracle's divergence clusters, not the spec ToC.
5. **Keep the semantic model in lockstep** (I3) — every renderer subsystem lands with its a11y/agent
   exposure, and the AccessKit bridge is scheduled once the tree stabilises.

Incremental relayout (H0.1) and the GPU paint path (H0.3) are **real H0 scope but not on the parity
critical path** — they are performance/architecture, and the gate is breadth + daily-drivability +
semantic coverage. They are sequenced after breadth crosses into daily-drivable territory, or pulled
forward only if the oracle shows layout *correctness* (not speed) blocking real sites.

**Standing loop rule (adds to III's prioritisation):** when ranking the next tick, the score is
*usage-weight × failing-breadth*, not failing-subtest-count. The encoding lesson is the precedent: a huge
number that does not move the Pareto frontier is not progress toward H0.
