# Manuk — The Full Project Vision (post-daily-driver horizon)

Captured 2026-07-16 from the project owner. This is the NORTH STAR: a finite, tractable definition of the
whole project. Phase 0 (daily-driver) is in flight now; phases 1–6 become the tick to-do list in order once
the daily-driver checklist (`reference/cap-research/ROADMAP.md`) is complete. Each later phase opens with a
DEEP-RESEARCH prompt (like the capability + velocity sweeps) before its ticks are scheduled.

The whole thesis: **a Rust, from-scratch, memory-safe, agent-native browser** — one that a human daily-drives
*and* that exposes a unified surface for agents to drive, with an optional in-browser "Claude Code for browsers"
prompt-to-action layer runnable on most computers via a small local LLM.

---

## Phase ordering (each phase → tick work when reached)

### Phase 0 — DAILY-DRIVER CAPABILITY  *(IN PROGRESS)*
The 7-tier checklist in `reference/cap-research/ROADMAP.md`. Two owner-added items fold into it:
- **Massive-corpus oracle validation (run INFREQUENTLY).** Validate the browser diff oracle against a much
  larger site corpus than the current sweep, to uncover the remaining tail of parity needs. Design it as a
  low-frequency job (not per-tick) so it surfaces the long tail without taxing cycle velocity.
- **Round out JS/TS scripting interactivity** if needed — the owner's LAST expected daily-driver area. Verify
  real-world script/interactivity coverage (event dispatch, DOM mutation, framework runtime) is sufficient.

Daily-driver is "the nearest thing" and is engineerable to a bounded tick count / timeline — that
tractability is the whole point.

### Phase 1 — UI/UX BROWSER FEATURES (tabs first)
Builds on Tier-5 shell (much already shipped: tabs, omnibox, session restore, cookie persistence).
- **Tab-set preserve/restore on restart** — a TOGGLEABLE startup setting (restore last session vs fresh).
- **Lean tab ops** — add / duplicate / close, ergonomic and fast.
- **Mute/unmute tab** — pending media (video/audio) support from Tier-3.
- **Pin-to-stay-warm** — the user pins workflow-relevant tabs to stay loaded (not hibernated), kept in
  memory/CPU so they stay ready.
- **⚑ DESIGN DECISION (needs research when reached): tab hibernation strategy.** Two poles —
  (a) STRICT hibernation of background tabs (minimize resource use), vs (b) UTILITY/practicality-based
  tab *warming* (keep high-frequency background tabs warm in a lean, ergonomic way, possibly with an
  intelligent auto-prioritized hibernate). Research the optimal ergonomic config; don't hard-code a pole.

### Phase 2 — AGENTIC BROWSER AUTOMATION API SURFACE (the ingress)
The **ground-up INGRESS to drive browser actions** — the execution contract/primitives that actually move the
browser. This is the stable, PINNABLE layer. **Seeded already by daily-driver Tier-6** (a11y tree, BiDi,
`dispatch_click` actuation, occlusion-aware hit-test). This phase defines + hardens + documents that surface so
external frameworks / Python async pipelines / agents can drive it directly. Distinct from the harness (Phase 3).

### Phase 3 — DEFAULT AGENT HARNESS ("Claude Code for browsers")
The harness = **the set of tools an LLM can use**, layered on the Phase-2 ingress: manages context, steers
multi-step prompt→action, lists skills, exposes tools. **Harness ≠ API surface** — the surface is the ingress/
primitives; the harness is the LLM-facing toolset + orchestrator on top. **Three deployment modes against the
ONE surface:**
1. Our default harness + toolset → **consumer prompt-to-action** (Phase 4 chat bar).
2. Our default harness → **dev/enterprise headless browser automation**.
3. **Bring-your-own harness / agent framework** → drives our API surface directly.

Baseline goal: a plain INSTRUCT LLM (no fine-tune) finds the tools from good skill-listing + tool exposure and
operates the browser "decently ok." Small-LLM affordances noted by the owner: Gemma SWA / Qwen GDN context
handling suit a steering harness.

### Phase 4 — CONSUMER PROMPT-TO-ACTION GUI
An OPTIONAL in-browser prompt bar (likely under the URL/chat bar) where the user types natural language and
a small local LLM drives structured tool ops (add/delete/duplicate tabs, search, navigate, in-browser
actions). Requirements:
- **Reuses the SAME browser-driving surface** as the Phase-2 dev API (unified DNA — one surface, two front
  doors: dev integration + consumer chat bar).
- **Two run modes, both easy in settings:** (1) a shippable build BUNDLING a small gguf (Gemma 4 e4b /
  Qwen 3.5 4b class, mobile/most-CPU capable) via a llama.cpp server runtime + gguf download interconnect;
  (2) user supplies their own OpenAI/Anthropic-compatible endpoint (their llama.cpp/vLLM, or Groq, etc.) —
  model + endpoint URL + configs easily entered. Simple UX; simple distribution.
- Polished, accessible, deterministic/high-fidelity, robust, open-source in-browser AI prompt-to-action —
  democratizing local browser automation for consumers.
- Elementary work already exists; this pends production polish of the agent drivability surface + reaching
  sufficient daily-driver parity.

### Phase 5 — PERFORMANCE OPTIMIZATION
### Phase 6 — SECURITY (reasonably good design; builds on Tier-6 capability scoping + anti-injection)

---

## Model escalation ladder (Phase 3/4 default-driver options)
`Gemma 4 e4b / Qwen 3.5 4b`  →  `Gemma 4 26 a4b / Gemma 4 31b / Qwen 3.6 27b/35b a3b`  →  user-supplied
model or API. A fine-tuned small may beat a bigger untrained model on OUR surface; a frontier model may win
untuned. The consumer chat bar and the dev API share the surface, so improvements to either compound.

**The surface is the constant; model size is only an escalation dial.** Because it's ONE unified ecosystem of
automation surface/tools, the eventual fine-tuning of BOTH small and large models targets roughly the SAME
surface — not a different surface per tier. Implication: freezing/pinning a stable, unified automation surface
is the prerequisite for any tuning, and every improvement to that one surface compounds across the whole model
ladder and both front doors (consumer prompt-to-action + dev API). Fine-tuning itself remains out of scope
(below); this only clarifies its eventual target when the owner takes it up.

## Scope boundaries (ENFORCE)
- **IN scope (horizon, for the loop later):** optimal automation API surface design; default harness + tool/
  skill exposure; llama.cpp-server / gguf-download interconnect (extend the existing elementary work);
  consumer prompt-to-action reusing the same surface. Baseline = instruct-LLM-operates-decently, NO fine-tune.
- **OUT of scope INDEFINITELY (until the owner explicitly amends):** fine-tuning ANY models — small (Gemma 4
  e4b / Qwen 3.5 4b) or large (Gemma 4 26b a4b / 31b, Qwen 3.6 27b / 35b a3b), ggufs AND nvfp4 — to the tool
  surface; and trace generation/capture for training. When eventually taken up, the tune learns BOTH the action
  landscape (traces of the surface/API) AND the reference harness; freezing/pinning the surface + harness is the
  prerequisite. **Why it's deferred (not just "later"):** fine-tuning needs attended AI/ML focus + hardware
  provisioning — it CANNOT be unattended-queued into the autonomous tick checklist the way phases 0–6 can. And
  it's NON-BLOCKING: the BYO-endpoint path gives full capability without any tune, and base instructs work
  decently on a well-exposed toolset. So deferring it costs the roadmap nothing. Owner-gated, separate track.
- **NOT our responsibility:** guaranteeing a bundled small gguf always tool-calls correctly, or runs on every
  CPU / embedded llama.cpp backend.

## Deep-research markers (spin a sweep when each phase is reached)
- Phase 1: optimal tab hibernation-vs-warming config.
- Phase 2: how to best design the browser-driving API tool surface (the primitives, granularity, stability).
- Phase 3: the LLM tool harness (context management, multi-step steering, skill/tool exposure).
- Phase 4: the consumer prompt-to-action UI (placement under URL/chat bar, model-config UX, bundling/distribution).
- Phase 5/6: performance profiling priorities + reasonably-good security design.

## The one-line summary
Full daily-driver parity → tab/UX features → agentic browser-driving API surface → default LLM harness to
drive it → consumer prompt-to-action via bundled/user ggufs → performance → security. Bounded, comprehensive,
and complete. Fine-tuning is a separate, owner-gated track outside this project's tick loop.

**Phases 0–6 are LOCKED as the refined optimal roadmap** — the direct, quickest path to the goals in a bounded
tick/time budget. This is a FINITE project (we know exactly what we want and need), and every phase 0–6 is
autonomously tick-executable; only the owner-gated fine-tuning track sits outside the loop.

## Execution plan for phases 1–6 (self-executing)
See **`docs/loop/AGENTIC-PHASES-PLAN.md`** — the standing directive that runs phases 1–6 WITHOUT owner input:
- **Phase-0 completion trigger + marker** (`.git/manuk-phase0-complete` + JOURNAL/HORIZON status when the
  shortest-sufficient-path table-stakes in `reference/cap-research/ROADMAP.md` are met at their ±1px/falsifiable bars).
- **The nested research→implement cascade**: API surface → harness → MCP → consumer UI → perf/security. Each
  layer's DEEP-RESEARCH PROMPT is drafted now (§6 of that file) and UPDATED with the implemented layer beneath it
  immediately before it runs — so each v1 is optimal against real ground truth, not pre-guessed architecture.
- **Decoupled modules** (clean upgrade cascade), **expanded tick-loop gates** (conformance / task-completion /
  zero-shot / perf-regression / adversarial-injection), and a **bespoke workflow escalation** for hard subsystems.
- Owner latitude: tab hibernation/warming + consumer-UI placement/UX are observer-decided, web-search-informed.
