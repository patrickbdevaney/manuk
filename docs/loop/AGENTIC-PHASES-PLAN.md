# Manuk Agentic-Phases Plan (Phases 1–6) + Nested Research-Cascade Discipline

Standing directive (owner, 2026-07-17). This file is the canonical, self-executing plan for everything AFTER
Phase 0 (daily-driver parity). It carries: the Phase-0 completion trigger, the nested research→implement cascade
that runs phases 1–6 WITHOUT further owner input, the decoupled-module architecture rule, the testing/oracle
expansion, the bespoke methodology for hard subsystems, and the five DEEP-RESEARCH PROMPTS (drafted now, run
later — each updated with ground truth immediately before it runs). North star: [[HORIZON.md]] / reference/horizon.

The core idea: each agentic layer is a decoupled module, and its research prompt is *updated with the ground
truth of the implemented layer beneath it, then run, then implemented* — a cascade that melds real
prerequisites with fresh web research to model the best possible v1 without pre-committing the architecture.

---

## 1. PHASE-0 COMPLETION — trigger + marker

**Completion criterion (observer-assessed; judgment, not one metric):** the SHORTEST-SUFFICIENT-PATH
table-stakes in `reference/cap-research/ROADMAP.md` are met at their falsifiable good-enough bars — i.e. the
"document + download + un-gated-SPA web" milestone: transport/session (T0), CSS render within ±1px on the
corpus (T1), boot-critical JS + storage (T2), forms/POST/validation (T4), shell persistence + session restore
(T5), agent actuation firing real DOM events (T6.1). Media (T3) is v1-optional. Subsystem levers legitimately
parked/decomposed (grid-areas, IndexedDB, Service-Workers) do not block completion if the corpus renders and
drives.

**When met, the observer MUST:**
1. `touch .git/manuk-phase0-complete` (the marker other tooling can check).
2. Append a `PHASE 0 COMPLETE (<date>): daily-driver milestone met — <one-line summary of the corpus state>`
   line to `docs/loop/JOURNAL.md` AND update the "Status" line in `HORIZON.md`.
3. Print/commit a lever-board banner so the grind agent stops opening NEW daily-driver levers and the loop
   transitions to the cascade below.
4. Kick off the NESTED CASCADE (§2), starting with research-prompt 6.1.

---

## 2. THE NESTED CASCADE (the standing rule for phases 1–6)

Run strictly in this order. For EACH step: (a) UPDATE the prompt (§6.x) by injecting the current ground truth
— the wiki lever-map, the work traces/JOURNAL, the repo, AND the just-implemented layer beneath it; (b) RUN the
deep research (multi-agent Workflow, like the capability + velocity sweeps); (c) synthesize → design → IMPLEMENT
the module via the tick loop (or a bespoke workflow for hard parts, §5); (d) verify against the expanded gates
(§4); (e) only THEN advance to the next step.

```
Phase 0 done ─► 6.1 Browser Automation API SURFACE  (the ingress substrate)
             ─► 6.2 Reference HARNESS               (the LLM agentic loop+tools driving the surface)
             ─► 6.3 MCP SERVER                      (lean bindings for terminal agents to drive the surface)
             ─► 6.4 Consumer PROMPT-TO-ACTION UI    (in-browser "Claude Code for browsers"; local gguf/endpoint)
             ─► 6.5 PERFORMANCE + SECURITY          (cross-cutting; nested over the whole stack)
```

Rationale: the API surface can only be designed from the REAL daily-driver capability surface; the harness's
tools can only be designed from the REAL API surface; the MCP exposes the REAL harness/surface; the consumer UI
rides the REAL harness backend; perf/security optimizes the REAL whole. Updating each prompt with the
implemented prerequisite is what makes each v1 optimal. Tab management/hibernation/isolation and the consumer UI
placement/UX are delegated to observer judgment + web research (§7) — do NOT block on owner input.

---

## 3. DECOUPLED-MODULE ARCHITECTURE RULE

Each layer ships as a **clean, isolatable module** so the release-upgrade surface cascades cleanly: when the
daily-driver parity/WPT surface expands, re-derive the API surface → then the harness → then the MCP → then the
consumer UI, in that order, each a bounded update rather than an entangled rewrite. Enforce: the browser
codebase keeps the automation-relevant seams (a11y tree, BiDi, page/DOM action dispatch, network intercept,
tab/session lifecycle) behind stable, documented, independently-testable interfaces. Prefer a dedicated
`engine/automation` (or similar) crate that depends on the engine but is depended-on by the API-surface module,
which is depended-on by the harness, etc. — a strict DAG, no back-edges. The API surface is the PINNABLE contract
(freezing it is the prerequisite for any eventual model fine-tune per [[post-daily-driver-vision]]).

---

## 4. TESTING / DIFFERENTIAL-ORACLE EXPANSION (how the tick loop verifies the agentic phases)

The tick loop's ratchet + falsifiable-gate discipline extends to each new module with a matching MECHANICAL
measurement — the parity/differential-oracle idea generalized from "render-diff vs Chrome" to "behavior-diff vs
a reference":

- **API surface — CONFORMANCE gates.** For each API primitive (navigate/observe/locate/act/wait/extract/
  intercept/session), a deterministic test against a fixed local page corpus asserting exact behavior; a
  trace-replay gate that records real browsing traces and replays them through the surface (regression net).
- **Harness — TASK-COMPLETION oracle.** A task corpus (adapt WebArena / WebVoyager / Mind2Web / a bespoke
  daily-driver-corpus of the sites in ROADMAP). Metric: **% of tasks completed WITHOUT falling back to vision**
  (the FB metric already named in lever-map). DIFFERENTIAL form: Manuk-harness+LLM vs Chrome+Playwright+same-LLM
  on the same tasks → "agentic parity" the way render-diff gives "render parity." Gate on no-regression.
- **MCP — zero-shot-usability gate.** A fresh agent (Claude Code/Codex) given only the MCP tool descriptions
  completes a fixed task set; measure success + tool-call efficiency; assert descriptions are self-sufficient.
- **Consumer UI — E2E + UX gates.** Prompt→action E2E on the task corpus through the real frontend loop; action-
  preview/approval correctness; local-gguf bootstrap smoke test.
- **Perf — REGRESSION gates.** Wall/memory/hibernation benchmarks folded into verify.sh as ratcheted floors
  (like the existing perf floors); no regression lands.
- **Security — ADVERSARIAL gates.** A prompt-injection corpus (malicious page content trying to hijack the
  browsing agent), capability-scope escape tests, POST-integrity, MCP auth/scoping — all as red-team gates that
  must stay green. Agentic browsing is itself a prompt-injection surface; this gate class is non-negotiable.

Each becomes a new G-class gate in verify.sh with a RATCHET invariant, so the agentic phases are held to the
same "never regress, falsifiable bar" rigor as WPT/daily-driver.

---

## 5. BESPOKE METHODOLOGY FOR HARD NOVEL SUBSYSTEMS / BLOCKERS

Atomic ticks fail on subsystem-scope work (proven: grid-areas 2h stall, tick 156; the diversify steer, tick
159). The standing escalation, "the tick loop used more robustly":

1. **Detect** — the park/diversify signals (a lever that stalls >~60min, discards WIP, or spans multiple
   session windows without landing) mark it SUBSYSTEM-SCOPE.
2. **Escalate, don't grind** — instead of one atomic tick, invoke a **bespoke multi-agent Workflow** for that
   subsystem: design-panel (N independent architectures → judge → synthesize) → decompose into a scaffold→fill→
   gate sub-tick sequence → implement each sub-tick (worktree-isolated if they mutate in parallel) →
   adversarially verify → land as a coordinated series of commits behind a flag/internal-API until complete.
3. **Park + preserve otherwise** — if not worth a dedicated run now, `git stash` the WIP, lever-board PARK it,
   diversify to bounded levers (the proven playbook, [[complexity-wall-watch-and-playbook]]).
4. **The agentic layers themselves are subsystems** — expect 6.1–6.5 each to be a bespoke-workflow effort, not a
   single tick. The nested cascade (§2) IS the top-level decomposition; within each, use design-panel workflows.

The tick loop thus gains a tier above the atomic tick: detect-subsystem → spawn-structured-workflow → verify →
resume. This is how hard daily-driver subsystems (IndexedDB/Service-Workers/MSE) AND the agentic phases get
built without a human.

---

## 6. THE DEEP-RESEARCH PROMPTS (drafted now; each UPDATED with ground truth immediately before it is run)

Every prompt below has three parts: **[INJECT-BEFORE-RUN]** (ground truth to paste in when its phase arrives),
**[SURVEY]** (the external SOTA to research), **[PRODUCE]** (the deliverable). Run each as a wide multi-agent
sweep (repo-grounded audit streams + web-search SOTA streams + high-effort synthesis), like the capability
sweep. Keep them DECOUPLED — updating/ running one must not require touching another.

### 6.1 — BROWSER AUTOMATION API SURFACE  (the pinnable ingress substrate)

OBJECTIVE: Design the optimal, stable, framework-agnostic browser-automation API surface for Manuk — the
bindings any browser-automation AI-agent framework builds on — grounded in our from-scratch engine's ACTUAL
post-Phase-0 capabilities and unique strengths (unified headful/headless DNA, memory-safe Rust, a11y-tree-native
observation, layout-backed occlusion-aware hit-testing, BiDi). This is the substrate the harness/MCP/consumer
all sit on, and the layer that must be pinnable/versioned for eventual fine-tuning.

[INJECT-BEFORE-RUN]: the completed daily-driver capability surface (reference/cap-research/ROADMAP.md final
state); the wiki lever-map + docs/wiki automation-relevant pages; the a11y-tree/BiDi/dispatch_click/network-
intercept implementation state (file:line); the work-trace corpus (JOURNAL + loop traces) of how real pages were
driven; the current engine crate DAG + where the automation seams live.

[SURVEY]: WebDriver-BiDi spec (the modern standard) and CDP; Playwright & Puppeteer API design (locators, auto-
waiting, network, tracing); accessibility-tree-driven automation and set-of-marks; semantic vs pixel targeting
(browser-use, Stagehand, Skyvern, WebVoyager, SeeAct, AutoWebGLM); Selenium/WebDriver-Classic lessons;
observability/trace formats (Playwright trace, HAR, OTel); API versioning/stability practice; how agent frameworks
prefer to consume a browser (what makes an API "zero-shot-agent-friendly" vs human-ergonomic). Cite sources.

[PRODUCE]: a full API-surface spec — the primitive set (navigate, snapshot/observe [a11y + DOM + set-of-marks],
locate-by-semantics, act {click/type/scroll/select/upload/keyboard}, wait-on-observable-condition, extract/read,
network-intercept, cookie/session/storage, tab/window lifecycle, download/upload, screenshot); sync-vs-async/
streaming model; the data model (node ids, snapshots, results, errors); the exact MAPPING to our engine
internals; a stability/versioning/pinning plan; an observability/tracing design; and how Manuk's unique strengths
become first-class API affordances competitors can't offer. Plus the decoupled-module boundary (crate layout).

### 6.2 — REFERENCE AGENTIC HARNESS  ("Claude Code for browsers")

OBJECTIVE: Design the optimal default reference harness — the agentic loop + tool/skill suite that lets ANY LLM
drive the 6.1 API surface to accomplish a user's browsing/automation task: user prompt → the LLM builds a
rigorous plan → uses the tools → maintains context/memory → loops → verifies post-conditions. This ONE harness
is both a native terminal CLI for driving our browser AND the backend for the 6.4 consumer in-browser client.

[INJECT-BEFORE-RUN]: the IMPLEMENTED 6.1 API-surface spec + crate; our engine's sandbox/temp-file/cache options
(in-browser sandbox vs OS FS vs app cache); the trace corpus of real tasks; the daily-driver capability limits
(where fallbacks are needed).

[SURVEY]: the best coding-agent harnesses — Claude Code, Codex, Aider, OpenHands, OpenCode, Grok's agent, Cursor
— their tool suites, agentic loops, planning, context/memory management, scratchpad/file discipline, permissioning,
subagent/parallelism patterns. Agentic memory & long-context: knowledge-graph memory, document/wiki management,
retrieval, working-memory/summarization, episodic traces. Browser-automation agents: browser-use, Stagehand,
Skyvern, WebVoyager, ReAct/plan-and-execute/reflexion, post-condition verification, self-repair loops, set-of-marks
+ a11y fallback ladders. Sandboxed scratchpad patterns. Zero-shot tool-description design. Cite sources.

[PRODUCE]: the harness architecture — the tool/skill set (mapping to 6.1), the agentic loop (plan→act→observe→
verify→replan with self-repair), the context/memory strategy (page-state summaries, a task "wiki"/scratchpad,
long-horizon browsing memory, KG of the site), the sandboxed temp-file design (where + isolation), interruptibility/
streaming, the fallback ladder (semantic→set-of-marks→vision→refuse), and the SHARED-BACKEND contract that serves
both the terminal CLI and the 6.4 consumer frontend. Decoupled from 6.1 (consumes its stable API only).

### 6.3 — MCP SERVER  (lean bindings for external terminal agents)

OBJECTIVE: Design a lean, high-performance MCP server so a terminal user's coding agent (Claude Code, Codex, or
any MCP client) can drive Manuk's browser — no-frills, fast abstractions, tools/skills any arbitrary agent
understands zero-shot and can work with granularly.

[INJECT-BEFORE-RUN]: the IMPLEMENTED 6.1 API surface + 6.2 harness (decide: does MCP expose the raw API surface,
the harness tools, or a curated blend?); perf characteristics of the surface.

[SURVEY]: the MCP spec (tools/resources/prompts; stdio vs SSE vs streamable-HTTP transports; sampling; roots);
existing browser MCP servers (Playwright-MCP, Puppeteer-MCP, browser-use MCP) — their tool granularity, naming,
snapshot-as-resource patterns, perf; tool-description best practices for zero-shot agent comprehension; MCP
security/auth/scoping. Cite sources.

[PRODUCE]: the MCP server spec — the exact tool/resource set + the abstraction level (lean-but-granular), naming/
descriptions optimized for zero-shot use, snapshot/a11y-tree as resources vs tools, transport/perf design, session
lifecycle, auth/capability-scoping, and the precise relationship to 6.1/6.2 (thin adapter, not a fork). Decoupled
module.

### 6.4 — CONSUMER PROMPT-TO-ACTION UI  (in-browser "Claude Code for browsers")

OBJECTIVE: Design the optional in-browser consumer client — a prompt-to-action chat/agent interface where the
user types a task and a small local (or user-configured) LLM drives the browser via the 6.2 harness backend.
Runnable two ways: a shipped build bundling a small CPU/CUDA-ambidextrous gguf (Gemma/Qwen class) via a llama.cpp-
server bootstrap, OR a user-supplied OpenAI/Anthropic-compatible endpoint / vLLM / llama.cpp server (model +
endpoint + config, simple settings).

[INJECT-BEFORE-RUN]: the IMPLEMENTED 6.2 harness backend contract + 6.3 MCP; our shell/UX (manuk-shell) state;
the existing elementary llama.cpp/gguf-download interconnect work.

[SURVEY]: agentic browser copilots & prompt-to-action UX — Arc/Dia, Perplexity Comet, Edge Copilot, Cursor chat,
Claude Code UX, browser-use UIs; UI PLACEMENT patterns (omnibox dropdown vs side panel vs overlay vs split view —
"under the search bar" per owner); action preview/approval/undo, streaming, task/plan visibility, trust & safety
UX; local-LLM bootstrap UX (gguf download/mgmt, CPU/CUDA detection, small instruct models for tool-use); model-
config settings UX. Cite sources. (Owner grants latitude to choose placement + design — §7.)

[PRODUCE]: the consumer UX design (placement, interaction model, action-preview/approval, streaming, task
visibility), the frontend↔harness integration, the model-config settings design, and the bundled-gguf +
llama.cpp-server bootstrap (download, CPU/CUDA ambidextrous, defaults) with the BYO-endpoint path. Optional/
toggleable. Decoupled from the harness (frontend over the stable backend).

### 6.5 — PERFORMANCE + SECURITY  (cross-cutting; nested over the whole stack)

OBJECTIVE: Design the performance + security optimization pass across the WHOLE system — engine + 6.1 API +
6.2 harness + 6.3 MCP + 6.4 consumer — building on every prior phase's ground truth.

[INJECT-BEFORE-RUN]: the whole implemented stack; measured perf/memory profiles; the security posture (capability
scoping, POST-integrity, codec licensing from ROADMAP) + the new agentic attack surface.

[SURVEY]: browser perf (render/compositor, memory, process/thread model, **tab hibernation vs keep-warm** — the
owner-flagged design decision, mindful of 32GB RAM; intelligent auto-hibernate vs practicality-based warming),
isolation/sandboxing; Rust perf; local-LLM serving perf (gguf quant, KV-cache, CPU/GPU, batching, latency). Security:
capability-scoping & least-privilege for the API/MCP; **anti-prompt-injection for the browsing agent** (malicious
web content hijacking the LLM — the dominant new risk); tool-exposure/MCP auth; sandbox escape; local-LLM safety;
POST-never-downgraded; the codec-licensing audit. Cite sources.

[PRODUCE]: the perf plan (with the tab hibernation/warming config decision + rationale) and the security design
(capability model, anti-injection defenses + the adversarial gate corpus of §4, isolation), as a ratcheted set of
tick-loop gates.

---

## 7. AUTONOMY LATITUDE (observer decides, web-search-informed — no owner block)

Owner grants explicit latitude to research + reason + decide autonomously, and implement what's best, on:
tab hibernation/warming/isolation strategy (6.5); the consumer prompt-to-action UI placement + UX design (6.4,
"under the search bar" as the seed); and the shared-backend/frontend split. Web-search the SOTA, pick the optimal
v1, implement, and record the decision + rationale in the wiki/JOURNAL. Escalate to the owner only a genuinely
irreversible or scope-changing choice.
