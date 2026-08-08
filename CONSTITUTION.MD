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
| **H0.1** layout beyond Taffy + incremental relayout | Layout is on Taffy 0.12 (`Cargo.toml:83`, re-verified check #73); **no double-dirty-bit incremental relayout.** CSS layout breadth is the weak spot. ⚠️ **The two percentages this row used to carry (`css-flexbox` 5.5%, `css-grid` 4.7%) were tick-86 WPT readings and are RETIRED as a ranking instrument** — see VI.3. The governing measurement of layout breadth is now **M1 on the in-scope CrUX corpus** (`shape ≥ 0.75` AND jarring-clean), latest **16.9%**, sweep t875. The WPT marks remain a regression ratchet. ⚠⚠ **NARROWED by a NEGATIVE result (check #82, tick 936): the residual mass is NOT in composed block-level width arithmetic.** t932's 25-case composed-width fixture — nested percentages, `%` width with `%` padding on one box in both box-sizings, border-box with asymmetric borders, `width:auto` with a full frame, floats and inline-blocks, `%` inside flex and grid items, `min`/`max` as percentages, `calc(100% − 2em)`, the page-wrapper idiom, `width:100%` in a padded parent — found **24 of 25 already Chrome-exact**, and all three real-prose LINE-COUNT probes matched exactly (so family #3, sub-pixel advance drift flipping a wrap boundary, did not reproduce either). The gap lives in the box types that opt **out** of ordinary block sizing: **tables** (t932 anonymous rows, a 392px container error and a MISSING_BOX; t933 row-height distribution), **inline composition** (t934/t935: an inline box that holds no text of its own contributed no leading, a cascading `dy`), **floats and `clear`** (⚠ ADDED by check #90, tick 1004 — they were on nobody's list and yielded three independent defects in three ticks at 60.4% declared corpus weight: t1000 a left float whose fit test asked the wrong box and walked off its container, t1001 a self-collapsing box's margins, t1002 CSS 2.1 §9.5 rule 6 bounding a float by the TOP of a line box and not its bottom), **out-of-flow boxes under a TRANSFORMED containing block** (⚠⚠ ADDED by check #91, tick 1012 — a category every previous negative result was structurally unable to reach, because t932's composed-width fixture and every battery before t1005 used IN-FLOW boxes: an abspos child was laid out against a containing-block rect read from the already-transformed fragment tree while its `left: 20px` still meant untransformed pixels, so a `scale(2)` container left it at the wrong place AND the wrong size; `position:absolute` is on 76% of the corpus and `transform:` on 65.5%), and **scroll containers** (an instrument question, not an engine one — our gutter model matches a real Chrome while the reference renders with `--hide-scrollbars`). Rank there, not in the arithmetic. ⚠⚠⚠ **THAT LAST PARENTHETICAL IS PROMOTED TO A NAMED CLASS BY CHECK #93 (tick 1021), BECAUSE IT NOW HAS THREE SUBJECTS AND A DECISION RULE — THE MIS-PROVISIONED REFERENCE.** `--hide-scrollbars` (the gutter), `--window-size` (t1016 — a window size is not a viewport, a constant 87px on every reference capture this project had ever taken), and **the interaction media features** (t1020 — headless Chrome answers `hover: none` / `pointer: none` / `any-hover: none` / `any-pointer: none`, the value reserved for a device that *cannot point*, while our engine correctly answers `hover: hover` / `pointer: fine`; **22.9% of the corpus declares one of those queries** and the reference was rendering the touch branch on every one). In all three the reference was **not rendering the page we asked for, and the difference was charged to the engine** — so the fix belongs in the harness, and "correcting" the engine to agree would make the shipping browser wrong for every real user in order to move a number. **The rule that decides which way to go is whether the reference CAN be provisioned:** `hyphens: auto` (t1010) cannot — Chrome's hyphenation dictionaries are a separately-shipped component, so we agree with it *by accident* and building the capability correctly would LOWER the score — whereas a pointing device is one flag. ⚠ And the corollary that makes this a standing cost rather than three closed tickets: **every one of the three was invisible for its whole life because a mis-provisioned reference looks exactly like agreement**, so this category is never found by ranking divergences. It is found by asking a THIRD party — the page itself, or the spec — what the answer should be. ⚠⚠⚠ **AND THE SECOND HALF, from check #83 (tick 944): this row says LAYOUT BREADTH is the constraint, and the measurement says THE METRIC is.** Partitioning every unscored in-scope site by whose failure it is (`fidelity.rs`'s own definitions, not the tag names): of 27 unscored, **17 are NOT OURS** (`oracle-module-shell` 6 — Chrome cannot boot a `type=module` SPA from a `file://` snapshot; `tree-divergence` 5; `shell-only` 3 — *"the ORACLE rendered only N elements"*; `empty-2xx` 3 — **the ORIGIN answered with a zero-byte body**), **7 bound the PAIR's clock** by construction, and **5 are ours** of which two do not reproduce solo. So the M1 ceiling is **82.2-87.4%** against a stated bar of **95%** — the target is 8-13 points above what the instrument can produce, and **no amount of engine work closes it**. `fidelity.rs:3410` says it in the imperative: *"name it, or 8 of the 13 sites carrying shell-only keep buying ENGINE ticks for an INSTRUMENT defect."* Both halves of this row are true and the second dominates: driving `shape_mean` (55.3%) upward on the 108 SCORABLE sites is real work with real value, and it cannot reach the stated bar. **Fixing the instrument (the loopback reverse proxy `fidelity.rs` already names, worth up to 14 sites), re-stating the bar against the SCORABLE denominator, or accepting that 95% means something other than what it says — those are owner decisions, and the loop must not grind engine ticks against a remaining distance that is 87% instrument.** |
| **H0.3** Vello/wgpu GPU paint path | **Aspirational comments only.** `manuk-compositor` (526 LOC) describes a tiered Vello-GPU renderer; the actual raster is **tiny-skia CPU**, everywhere. `wgpu 27` is declared and used only for shell windowing. |
| **I5** the differential oracle is the discovery engine | ⚠️ **CORRECTED (constitution check #51, tick 680) — stale twice over.** The crawl completes (265 sites, 392 clusters), so the original "never completed a full crawl (`ORACLE_CRAWLED: 0`)" is simply out of date. But the deeper correction is about *which* mechanism discovers: **the primary discovery engine is now the INSTRUMENTED LOG, and its yield comes from ATTRIBUTION rather than volume.** Three consecutive findings in four ticks came from one log line each — HTML §7.3.3 absent (`window.__appData__ is undefined at inline.js:1:155`, t677), one source name for every inline script on every page (t679), and our own unbounded virtual clock (`next_in_ms=86400000 vclock_ms=13822876800000`, t680, which had been blamed on the page for six ticks). Each was findable only because the previous tick made the log one degree more specific. That compounds, and it is cheaper per finding than any sweep this project runs. **Standing rule from the same window: `most likely` in a log message is a confession — if a message speculates about state the process is holding, print the state.** The oracle remains the breadth instrument; the log is the depth one. |
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

**⚠️ CORRECTED (check #73, tick 860) — VI.3's OWN NUMBER IS NOW STALE, AND PART VII RETIRED THE WHOLE
CLASS OF NUMBER.** The `32.3%` above is a tick-86 reading and nothing has recomputed it, because
**PART VII — written later and explicitly superseding the roadmap for the near term — says the bar is
*"reliably renders and runs the representative real internet," NOT a WPT percentage*, and that
*"83% and beyond is explicitly OUT OF SCOPE for v1."*** So VI.3's *lesson* is permanent and binding
(rank by usage-weight × failing-breadth, never by subtest count) while its *instrument* has been
replaced. The instrument that governs the loop today is:

> **M1 on the in-scope CrUX corpus** — `shape ≥ 0.75` **AND** jarring-clean, bot-walls excluded per
> `DAILY-DRIVER-CERTIFICATION.md` §3 — banked every sweep in `docs/loop/FIDELITY-PROGRESS.tsv`.
> Latest banked: **19.7% (24/122 in-scope)**, shape-only in-scope pass 27.9%, jarring-clean 36.1%,
> shape_mean 60.4%, cov_mean 86.9% — sweep **t1008**, re-derived from `SWEEP-t1008-rows.tsv` at
> check #93 and reproducing `FIDELITY-PROGRESS.tsv` field-for-field.

**⚠⚠⚠ AND THE DENOMINATOR IS PART OF THE NUMBER — THREE CUTS ARE IN CIRCULATION AND THE LOOP'S PROSE
HAS QUOTED THE MOST FLATTERING ONE (check #93, tick 1021; ARGUMENT CORRECTED at tick 1022, conclusion
unchanged).** t1008 banked `24/122 = 19.7%` in the ledger and **published `27/113 = 23.9%` in prose**,
which check #91 then carried forward as the horizon's latest number. The gap is **4.2 points on the
level**, and the level is the only thing a 95% bar can be measured against. It is **two effects, both
running the same way**:

```text
   ledger f15   24 / 122   19.7%    numerator over reason=="" rows, denominator = in-scope
   prose        27 / 113   23.9%    both over shape_n > 0
                 ^     ^
                 |     +--  9 in-scope sites that yielded NOTHING, dropped from the denominator
                 +--------  3 sites admitted to the numerator, and this is all they are:
                            app.ordertime.com    shape 1.000 from ONE element, coverage 0.040
                            allticketscol.com    shape 1.000 from ONE element
                            awlyaa.education.dz  shape 0.833 from SIX
```

> **shape = 1.000 over one element is not a page that renders correctly; it is a page that was not
> measured.** `100% of nothing is 100%` (t650), arriving through the denominator instead of the
> numerator. `fidelity-progress.sh` counts the numerator over `reason == ""` precisely because the
> instrument has already refused to score those sites — and a re-derivation from the raw rows
> re-admits them unless it re-applies the refusal.

**Binding, so this cannot recur:** the M1 number of record is **`FIDELITY-PROGRESS.tsv` field 15
(`m1_pass_pct`) = shape ≥ 0.75 AND jarring-clean, numerator over scored rows, denominator = IN-SCOPE
(field 11)** — the only cut that refuses **both** a vacuous pass and a silent drop, and the one
`DAILY-DRIVER-CERTIFICATION.md` §4 and the Phase-0 exit both name. A tick may publish any other cut it
finds useful **as long as it is labelled with its denominator and does not appear as "M1"**. No
decision turns on the 4.2 points: both readings are far from 95%, and VI.2's ceiling finding is
untouched. What turns on it is whether the scoreboard is honest, which is I5.

⚠ **And how tick 1021 got the right answer from a wrong argument, recorded because the failure is
generic.** It claimed `113`/`123` could not be derived from the rows at all. They can, in one line —
`shape_n > 0`. The recomputation had matched the ledger on **six** numbers across two sweeps and
treated that as proof its whole classifier was the instrument's; it was, for *excluded* and
*in-scope*, and was not for *scored* — **the one field not in the ratio, and therefore the one never
cross-checked. Six agreeing numbers do not validate a seventh definition.** The producer says it in
plain sight (`scripts/fidelity-progress.sh:88-95`), which makes this the standing rule **READ THE
PRODUCER, NOT ONLY THE CONSUMER**, re-derived the expensive way.

`RATCHET.tsv`'s WPT marks are kept as what they are — **a regression ratchet, not a ranking.** A
window that leaves every WPT mark byte-identical while moving M1 is the *expected* shape of on-mandate
work, not an absence of output. A future check must not re-derive the retired ordering from this Part.

**And the corollary that cost this loop twice (check #73, findings 1–2):** the M1 instrument does not
only emit a number, it emits a **hand-written reason string per unscored site**, and the loop consumes
those strings as a ranked backlog. Two were tested (`shell-only` at t856, `css-starved` at t860) and
**both were false** — one blamed our load deadline for the author's own 404'd `<link>`. Under I5 an
unfalsifiable divergence label does not discover work, it **manufactures backlog**. So:
**a reason string that asserts a CAUSE is a hypothesis with a test attached; run that test on one
member before any cohort it names is treated as engine work.**

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

## PART VII — THE v1 SHIPPABLE RELEASE (the north star and the finish line; supersedes the roadmap as the near-term priority)

The complete, shippable **v1** — the thing that proves the core thesis and constitutes "we built what we set
out to build" — is **exactly and only these four components, treated as one self-contained system.** This
governs tick selection now; it does not replace the horizons but re-scopes the near term: v1 pulls H0-parity
(scoped) + H2-agentic (scoped) forward, scopes H1-security **down** to "good enough," and **defers H3/H4
entirely.**

**1. Daily-driver rendering parity** — breadth-first, usage-weighted, **GUI and headless**. The bar is
*"reliably renders and runs the representative real internet,"* **not a WPT percentage.** Drive WPT
capability enablement breadth-first against the **oracle's representative corpus** — maximize the count of
**real sites moved per fix** — and additionally target **depth on the highest-concentration destinations**
(x.com, Instagram, Facebook, HuggingFace, and the handful of sites where people actually spend time). **A WPT
number is a horizon and a diagnostic, never a gate; 83% and beyond is explicitly OUT OF SCOPE for v1** —
pursuing it optimizes the wrong variable. (This sharpens VI.3 / the standing loop rule: rank by real-sites-
moved, verified against the oracle corpus, not by subtest count.)

**2. The agentic automation surface** — native, complete, optimal. **This is the differentiator, not a
feature — it earns the most polish.** The agent must fully drive and traverse the web through the native
surface: see pages **multimodally**; read the **DOM / a11y / semantic tree as first-class, synchronously-
queryable state**; interact with forms, buttons, links, and JS-driven UI; navigate and manage tabs.
**"Complete" = an agent can reliably drive the same top-N real sites a human daily-drives**, measured against
the **same representative corpus** as rendering parity, not separately. This is what incumbents structurally
cannot retrofit. (The a11y crate is the seam; build the driving surface out against the top sites.)

**3. Reasonable security hardening** — *"good enough," explicitly NOT maximal.* Bar 0 stability (no
crashes/hangs/silent failures), **process-per-tab containment**, and sound handling of the memory-unsafe FFI
surface. It does **NOT** require per-origin isolation, a SpiderMonkey heap-sandbox equivalent, an extension
security model, or best-in-class-anything. **Do not let "security" expand into a research program before v1
ships.**

**4. Reasonable performance** — lean, fast, no pathological hangs or runaway resource use on the
representative corpus. **A "feels good to use" target, not a benchmark-winning one.**

### VII.1 — THE SCOPE-PROTECTION RULE (hard discipline, enforced, not aspiration)

Everything else on the roadmap — the consumer prompt-to-action layer, crypto-wallet / x402 support, the
V8-backed speciation, kiosk / embedded / enterprise variants, per-origin isolation, deep conformance — is
**explicitly deferred and OUT of v1 scope.** It is a menu drawn from *if and when the core finds traction*,
not a plan committed to now.

**Critically: do not build the v1 core toward those later species.** Keep modular seams that make later
addition possible (good architecture insurance) — but **never pay upfront cost, add abstraction, or make a v1
design decision "so it's ready for" a deferred item.** If a choice is *only* justified by a post-v1 goal, it
is out of scope.

**THE TEST for any proposed work (apply every tick):** *does this move one of the four components above
toward shippable, or does it serve something deferred?* If the latter, **it does not happen now.**

### VII.2 — Why this scoping is the priority, not a limitation

The four components together are a complete, coherent, genuinely novel artifact — a **Rust-native, memory-
safe, agent-native browser that daily-drives the real web** — that did not exist before and **stands on its
own even if nothing further on the roadmap is ever built.** Shipping this provably accomplishes the original
thesis; leaving it unfinished while chasing the long tail (83%+ WPT) or the later species does not. **Ship
the core; let the rest earn its way in.**
