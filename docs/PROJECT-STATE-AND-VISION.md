# Manuk — Project State & Vision

**Snapshot: 2026-07-18, tick 231.** Regenerate the live numbers with `./scripts/phase0-progress.sh`,
`./scripts/constellation.sh`, and `./scripts/lever-board.sh`. Canonical sources: `docs/loop/HORIZON.md`
(the 7-phase north star), `docs/loop/V1-SCOPE.md` (what v1 is), `docs/loop/DAILY-DRIVER-EDGES.md`
(the capability edges), `docs/loop/CONSTELLATION.tsv` (the 128-capability checklist).

---

## PART 1 — WHERE THE PROJECT IS NOW

### The one-line status
**Phase 0 (daily-driver capability) is in progress at ~52% readiness**, with **94% of the capability
checklist now actually measured** (unknowns down from 35 → 8). The engine renders and drives the
mainstream web; the remaining mass is **media** and a set of known-absent platform capabilities.

### The measured state (128 capabilities)

| class | caps | gated | works | partial | missing | unknown | **working** |
|---|---|---|---|---|---|---|---|
| doc (reference/news/docs web) | 31 | 12 | 5 | 3 | 9 | 2 | **60%** |
| app (SPA / framework web) | 37 | 13 | 12 | 2 | 10 | 0 | **70%** |
| platform (real-time, embeds, auth) | 29 | 7 | 3 | 4 | 13 | 2 | **41%** |
| **media** (video/audio) | 11 | 0 | 0 | 1 | 10 | 0 | **5%** |
| cross (Bar-0, perf, a11y, agentic) | 20 | 8 | 0 | 3 | 5 | 4 | **48%** |
| **TOTAL** | **128** | **40** | **20** | **13** | **47** | **8** | **52%** |

- **readiness 52%** = (gated + works + ½·partial) / total — capability *confirmed working*
- **gate-locked 31%** = asserted by a named `G_*` gate; this floor cannot rot
- **measured 94%** = has a real verdict (only 8 capabilities remain unprobed)

**Read the gap correctly:** readiness 52% vs measured 94% means the remainder is largely
**known-absent capability to BUILD** (media decode/demux, WebGL, IndexedDB, Service Worker), *not*
unknowns to discover. The work has shifted from *probing* to *building*.

### What recently landed (the current push)
- **Streaming** — `ReadableStream` + SSE + progressive XHR (AI-chat answers render)
- **a11y node STATES** — the agent can confirm the result of its own actions
- **WebSocket** — real transport via borrowed `tokio-tungstenite` (live chat/real-time)
- **scroll-anchoring** + **forced-reflow** — feeds stop jumping; virtualized lists measure correctly
- **canvas `fillText`** — real glyphs + real `measureText` (the Google Docs / chart-label / terminal enabler)
- **OAuth "Sign in with…"** — probed, found working, **gated end-to-end across providers**
- **hydration** — the server's markup is *adopted, not rebuilt* (SSR/HuggingFace-class sites)
- **media M1–M2** — MSE byte pipe + segment path
- **probe batches** — unknowns 35 → 8

> ⚠ **Correction of record:** the five "finish-line levers" (streaming, a11y-states, WebSocket,
> scroll-anchoring, forced-reflow) were a **MILESTONE, not Phase-0 completion.** Phase 0 is the
> *full* daily-driver checklist — "runs almost every website." It remains in progress.

### What Phase 0 still needs
1. **MEDIA (M3–M7)** — the largest gap at 5%. Borrow-plan: `symphonia` (demux + AAC) + `cpal`
   (audio out) + `libvpx`/`rav1d` (VP9/AV1) → `<video>` playback + A/V sync → controls. Steer YouTube
   to VP9+AAC via `isTypeSupported`. **No ffmpeg** (LGPL + C attack surface + build cost). **DRM out.**
2. **OAuth completion** — redirect flow is gated; still open: interactive cross-origin **iframe
   re-render** (also fixes 3-D Secure), third-party/cross-site cookies, **FedCM** (`navigator.credentials`).
3. **Completeness identity** — real software-WebGL + honest renderer strings, `document.visibilityState`,
   `navigator.permissions.query()`, `userAgentData` + canonical UA (banks/LinkedIn/Cloudflare gating).
4. **IndexedDB** — borrow `redb`/`heed`; unblocks AWS/GCP console login (their auth SDKs hard-fail without it).
5. **The last 8 unknowns**, then the remaining `missing` platform/doc capabilities (Service Worker,
   MathML, bidi reordering, Web Workers, drag-and-drop, file-input actuation).

### Infrastructure state (the loop that builds it)
Self-managing on all three failure modes that have bitten us:
- **Loop death** → cron watchdog (2 min) revives a dead supervisor; flock enforces single-instance
- **Agent hang** → working-flag stall-reaper kills a hung agent tree (30 min threshold)
- **RAM exhaustion** → agent runs in a **20 GiB systemd cgroup** (16 GiB throttle). *A `--unit` name
  collision had silently disabled this for 31 launches and hung the machine; fixed and verified.*
- **Disk fill** → `disk-hygiene.sh` on a 10-minute cron. Measured churn: each build cycle orphans
  ~100 hash-named test-binaries (**~25 GB per ~6 min at peak rebuild**), so pruning must run at that cadence.

---

## PART 2 — THE VISION (as it stands now)

### The thesis
**A from-scratch, memory-safe Rust browser that is agent-native.** One engine that a human daily-drives
*and* that exposes a unified surface for agents to drive — with an optional in-browser
"Claude Code for browsers" prompt-to-action layer runnable on ordinary hardware via a small local LLM.

### Why daily-driver parity is the whole foundation
**The browser's daily-driver capability surface *is* the automation-agent surface.** What a human can do —
see the page (DOM + a11y tree + pixels), click/type/submit, traverse links, go back/forward, scroll a feed,
log in, play a video — is exactly what an agent can do. The API surface, the harness, MCP, and the consumer
prompt-to-action layer are all **downstream** of this. Every capability closed in Phase 0 widens what every
later phase can accomplish. This is why Phase 0 comes first and why it is defined as the *full* checklist.

### What "done" honestly means
**Achievable:** faithfully renders and is usable on *almost every mainstream website* — doc/reference,
SPA/app, social feeds, e-commerce, AI-chat, dev platforms, lighter cloud consoles, and (post-media) YouTube —
at ~0.75+ structural fidelity vs Chromium, measured by the differential oracle.

**Not claimed:** literal pixel-and-every-API parity with Chromium. The honest, named exceptions:
- **Canvas-native office suites** (Google Docs/Sheets/Slides) — *reachable-with-effort* via canvas-text
  (viewing close once `fillText` + `measureText` are solid; full editing needs IME + rich clipboard + perf)
- **Heavy-WebGL creative apps** (Canva, Figma, vector Google Maps) — genuine WebGL wall
- **DRM** (Netflix/Spotify) — a licensed proprietary CDM; cannot be built, only licensed
- A CSS-layout-math tail (complex flex/grid, subgrid) — faithful, not sub-pixel-perfect
- Sites that **whitelist Chromium only** — their policy, not our capability

**Scope boundary — completeness, not evasion:** we expose what a genuine headful browser *has*, with
*honest* values (`webdriver:false` because it's true; a real software-GL context reporting its true renderer;
`visibilityState:'visible'` because it is). We do **not** spoof, rotate fingerprints, or impersonate a
specific competitor build.

---

## PART 3 — THE SEVEN PHASES (0–6): what the product *is* at each

> ⚠ **SUPERSEDED by `docs/ENGINEERING-SYNTHESIS.md`** (tick 232), which integrates the
> `ENGINEERING.MD` competitive research. The table below is retained as the baseline; the
> synthesis moves four things: the **`JsEngine` seam** into Phase 0–1, **taint + capability
> tokens** from Phase 6 → 2, the **no-pixels pipeline + intent-aware scheduler** from Phase 5 → 2–3,
> and replaces the Phase-0 exit criterion with the **oracle gate** (≥0.75 structural fidelity on
> ≥95% of Tranco top-1000) rather than a checklist percentage. It also adds **Track R** (a Rust JS
> engine, interpreter+baseline only, spanning all phases behind the seam).

| phase | name | the product at the end of this phase |
|---|---|---|
| **0** | **Daily-driver capability** *(IN PROGRESS — 52%)* | **"The browser works."** Faithfully renders and drives almost every mainstream website: doc web, SPAs w/ hydration, social feeds, e-commerce, AI-chat, dev platforms, and YouTube-class media. Human-daily-drivable. Persistence (history/bookmarks/settings/cookies/tabs) already shipped. |
| **1** | **UI/UX browser features** (tabs first) | **"A browser you'd actually live in."** Session preserve/restore as a toggle, lean tab ops (add/duplicate/close), mute/unmute tab, pin-to-stay-warm, and a researched **tab hibernation vs warming** strategy (strict resource-minimizing vs utility-based warming — decided by research, not hard-coded). |
| **2** | **Agentic automation API surface** (the ingress) | **"Agent-drivable browser."** The stable, *pinnable* execution contract external frameworks / Python pipelines / any agent can drive directly. Already seeded: a11y tree, `dispatch_*` actuation, `eval_in_page`, occlusion-aware hit-test, NodeId handles — all protocol-shaped (CDP / WebDriver-BiDi). This phase **defines, hardens, and documents** it. Distinct from the harness. |
| **3** | **Default agent harness** ("Claude Code for browsers") | **"An agent that uses the browser."** The LLM-facing toolset + orchestrator on the Phase-2 ingress: context management, multi-step prompt→action steering, skill listing, tool exposure. **Three deployment modes against ONE surface:** (a) our harness → consumer chat bar, (b) our harness → dev/enterprise headless automation, (c) bring-your-own agent framework. Baseline goal: a plain instruct LLM (no fine-tune) drives the browser decently from good tool exposure. |
| **4** | **Consumer prompt-to-action GUI** | **"The consumer agentic browser."** An optional in-browser prompt bar under the URL bar: natural language → structured tool ops (tabs, search, navigate, in-page actions). Reuses the *same* surface as the dev API (one surface, two front doors). **Two run modes:** a shippable build bundling a small gguf (Gemma-4-e4b / Qwen-3.5-4b class) via a llama.cpp runtime, or bring-your-own OpenAI/Anthropic-compatible endpoint. Democratizes local browser automation. |
| **5** | **Performance optimization** | **"Fast."** Cross-cutting optimization over the whole stack once capability is settled. |
| **6** | **Security** | **"Shippable v1."** Reasonably-good security design building on capability scoping + anti-injection; process-per-tab containment and sound memory-unsafe-FFI handling. |

### Discipline governing phases 1–6
- **Each phase opens with a deep-research prompt** (drafted in `docs/loop/AGENTIC-PHASES-PLAN.md` §6),
  updated with ground truth immediately before it runs — never run stale.
- **Nested cascade:** research → implement → verify, one phase at a time, with the observer in the loop
  between phases.
- **Decoupled-module rule:** each agentic layer is a separate module so v1 can ship without any one of
  them being perfect, and so later phases can replace a layer without a rewrite.
- **Not "intractably perfect" per layer** — each is driven to genuine saturation / Pareto / definition-of-done
  given what's available at the time, then the cascade advances.

### Explicitly deferred (indefinitely)
Model **fine-tuning** (needs dedicated ML focus + hardware provisioning, and users can bring their own
endpoint regardless), per-origin isolation, deep WPT conformance (83%+ is a diagnostic, never a gate),
crypto-wallet/x402, V8-backed speciation, kiosk/embedded/enterprise variants.

---

## How to re-derive this document
```bash
./scripts/phase0-progress.sh      # readiness + per-class table (Part 1)
./scripts/constellation.sh        # full capability scoring + biggest holes
./scripts/constellation.sh --gaps # the work list
./scripts/lever-board.sh          # current CO-#1 priorities the loop is executing
git log --oneline | grep 'feat(loop tick'   # what has landed
```
