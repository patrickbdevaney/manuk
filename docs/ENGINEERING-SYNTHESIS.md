# Engineering Synthesis — Integrating the Competitive Research into the 0–6 Roadmap

**Source:** `ENGINEERING.MD` (decision-grade competitive research report).
**Written:** tick 232. **Expanded to a complete placement ledger:** tick 234.
**Status:** supersedes the phase table in `docs/PROJECT-STATE-AND-VISION.md` Part 3.

---

## 0. The one-paragraph synthesis

The report does not change *what* Manuk is building — it changes *when* specific things get
built. Most verdicts confirm decisions already made; a surprising number describe work we have
**already done** and should publish rather than build; the genuinely new content is a set of
**architectural seams cheap to cut now and expensive to retrofit**, a set of **load-bearing
claims we assert but have never measured**, and a **decay clock** on the agent moat. The
optimizing path: **cut the seams now, fill them later; measure before marketing; keep the
capability grind as the main line.**

---

## 1. Three buckets, not two

Sorting the report this way is what makes it actionable — a third of it needs no engineering at all.

### 1a. ALREADY WON — publish, do not build
Verified in-repo at tick 234. These are banked advantages we were not claiming.

| Insight | Reality |
|---|---|
| "Rust image decoders eliminate a classic CVE farm (libwebp CVE-2023-4863, CVSS 10.0)" | **Done.** We ship pure-Rust `image` 0.25 (png/jpeg/gif/webp/bmp/ico) + `resvg`. Zero C image FFI. This CVE class is eliminated by construction *today*. |
| "Borrow rustybuzz/swash over HarfBuzz; shaping is a historical CVE source" | **Done.** `swash` 0.2. |
| "Occlusion/visibility-truthful hit-testing — publish as a differentiator" | **Done.** Built; agents never click an occluded element. |
| "Stable NodeId handles" | **Done** as a mechanism; the *durability contract* is not yet gated (see SEAM-NODEID). |
| "Native semantic tree-diff — highest moat value" | **Done** as a mechanism; per-action *receipts* not yet formalized. |
| "Memory-safe DOM/layout/paint/networking" | **Done.** The ~70%-of-severe-CVEs class is eliminated outside the JS tier. |

**Process implication:** the safety story is stronger than we have been stating. The honest claim
is *"the JS engine is the sole remaining memory-unsafe component"* — not a hedge, a specific,
checkable asterisk.

### 1b. CONFIRMS — stop re-litigating
No-ffmpeg (symphonia + cpal + rav1d) · DRM out · borrow Stylo/html5ever/Taffy/hyper/rustls/
tokio-tungstenite · oracle-as-prioritizer over WPT coverage · concede peak JS/MotionMark/WebGL ·
build-from-scratch JS engine ranked above porting V8 or SpiderMonkey (both are upstream treadmills).

### 1c. CHANGES — the actionable set
Placed in full in §3.

---

## 2. The fidelity finding (not in the report — found while testing it)

The report's Domain D says compat, not performance, kills independent engines, and prescribes a
differential oracle as the counter. We have one. Checking its actual configuration:

- **G1 real-site fidelity is measured and binding**, floor 0.75 → **75.8% visual, 100% coverage.**
- **The corpus is two pages**: Hacker News and a Wikipedia article. Both static doc-web — our
  strongest class.
- `docs/bench/oracle-corpus.txt` holds **281 category-stratified sites**, unused by the gate, and
  its own header warns: *"the bugs we had NOT found were exactly the ones no corpus site happened
  to use."*
- **`RATCHET.tsv` contains no fidelity mark at all.** WPT counts and capability cells ratchet;
  the number that predicts whether real sites work does not.

**Therefore: capability % cannot see feature-present-but-site-broken.** 52% capability is
consistent with either a 0.6 or a 0.3 broad fidelity score. This is the largest single unknown in
the project, and closing it is cheap because the corpus and crawler already exist.

**Independent corroboration from the agent's own tick-233 handoff:** *"Probing beat building.
WebAssembly, CJK line breaking, media queries, hydration, and the entire OAuth redirect flow were
all carried as `unknown` and all already worked — the checklist was steering the loop toward work
that didn't need doing, partly because it goes stale from our own landed ticks."* Two independent
observations of the same failure mode: **we have been optimizing the metric we can see.**

---

## 3. Complete placement ledger

Every actionable insight, placed. Rationale is why *that* phase, not another.

### → PHASE 0 (now — cheap, or a seam whose cost multiplies if deferred)

| Item | Source | Why Phase 0 |
|---|---|---|
| **FID-SWEEP** — broad 281-site category-stratified fidelity sweep, banked to RATCHET | §2 | Biggest unknown; corpus + crawler already exist. Must be a **separate script off the per-tick path** — 281 render+Chrome compares in `verify.sh` would blow the WALL ratchet and brick every tick. |
| **TRI-ORACLE** — add Firefox as second reference | D | Chromium-only diffing overfits to Chromium bugs. Flag only two-way disagreements. |
| **MEM-HARNESS** — RSS+PSS via `smaps_rollup`, 10/50/100 tabs | C | The memory-lean claim is unmeasured and may be false. Cheap. Falsifiable. |
| **SEAM-JSENGINE** — trait boundary around mozjs | A5 | *"Retrofitting after deep mozjs coupling costs multiples."* Highest-regret deferral in the report. One impl; do not write a second engine. |
| **SEAM-NODEID** — gate identity survival across re-render | F2 | Cheap gate on an existing mechanism; kills the #1 source of agent flakiness. |
| **SEAM-TAINT** — mark content-originated DOM strings; capability-token type, inert | F3/H | The *boundary* is the deliverable. Cutting it later means touching every DOM string site. |
| **SEAM-NOPIXELS** — paint-elision flag; layout + a11y only | C/F | A flag in the paint path is cheap now; retrofitting a pixel-free mode into a compositor is not. |
| **Supply chain: cargo-vet + cargo-audit** | H5 | **Verified absent** — no vet/audit anywhere in `scripts/` or CI. Cheap, and dependency count only grows. |
| **Interop-2026 focus areas → CONSTELLATION rows** | D | The industry's own signal of what is load-bearing: anchor positioning, container style queries, `<dialog>`/popover, view transitions, scroll-driven animations, WebRTC, ESM module loading, `user-select`. Free prioritization signal. |
| **Prune below-ROI rows from the board** | D | See §5 — the board currently promotes work the report puts below the line. |
| **Publish the already-won set** (§1a) | E/F/H | Documentation, not engineering. |

### → PHASE 1 (tabs / UX — the scheduler phase)

| Item | Source | Why here |
|---|---|---|
| **Intent-aware tab scheduler** — `U(tab) = P(use \| agent-intent) × warm_cost × memory_pressure`; strict-minimizer + utility-warmer policies | C | Manuk's structural win: *the agent declares intent*, a signal no incumbent scheduler has. Belongs with tabs. |
| **Focus-adaptive isolation (Topology 6)** — agent tabs collapsed into one low-privilege process; foreground promoted on focus | B | Novel; no existence proof. Prototype where the tab model is being built. |
| **100-tab RSS budget target (~2–3 GB)** | C | Target set once MEM-HARNESS gives a baseline. |
| **Snapshot/restore cold-start amortization** | C | Pairs with hibernation. |
| **Finish SEAM-JSENGINE** | A5 | Seam cut in 0; completed here. |

### → PHASE 2 (agent API — now also the security and no-pixels phase)

| Item | Source | Why here |
|---|---|---|
| **Transactional action→diff receipts** | F1 | Highest moat value. The ingress is where the contract is defined. |
| **Published NodeId durability contract** | F2 | Contract belongs with the API it constrains. |
| **Engine-level injection resistance** — enforce taint; task-scoped capability tokens; Rule-of-Two structurally | F3/H3 | *"Prompt injection cannot be fully solved at the model layer."* Pulled from Phase 6 — security-as-architecture cannot be bolted on. |
| **No-pixels agent tabs** (enforce the Phase-0 flag) | C/F | Pulled from Phase 5: a moat feature, not a perf feature. |
| **Token-optimal representations** — target sub-1K tokens | F8 | a11y tree is ~93% more token-efficient than raw DOM (125K → ~15K → 200–600). |
| **WebMCP + native-substrate spec extension** | F9/G | Decay clock ~mid-2027. WebMCP's *non-goals* (headless, autonomy) are exactly our goals. |
| **Intent-driven prefetch / speculative pre-navigation** | F6 | Consumes Phase-1 intent signal. |
| **Parallel multi-tab agent execution** | F7 | Needs scheduler (1) + no-pixels (2). |
| **Process-placement policy layer** (Topology 5/6 knob) | B | Where the isolation decision becomes configurable. |

### → PHASE 3 (harness)
**Deterministic replay / time-travel** (F5) — record observation+action streams, replay
deterministically. It is an *agent-debugging* tool, so it belongs with the agent, not the engine.

### → PHASE 4 (consumer GUI)
Unchanged. Small-gguf bundle or bring-your-own endpoint.

### → PHASE 5 (performance — narrowed)
Vello/wgpu compute raster · parallel layout · off-main-thread parse/bytecode-gen · icu4x
migration · the *fair and winnable* benchmark suite (navigation-heavy corpus, cold start, memory
— explicitly **not** compute JS). Memory and scheduling already shipped in 1–2.

### → PHASE 6 (security — narrowed)
seccomp-BPF + Landlock · **W^X for any JIT** · SFI/Wasm containment of mozjs if Track R has not
landed · multi-OS sandbox parity. The injection/capability architecture already shipped in Phase 2.

### → TRACK R (spans all phases, behind SEAM-JSENGINE)
Rust JS engine, **interpreter + baseline only**. Never an optimizing tier — it would reintroduce
the miscompilation CVE class the project exists to avoid. Reference set: QuickJS (~99% test262,
interpreter-only), Boa (~95.5%, Rust), Ladybird AsmInt (JIT-less, hand-asm). Benchmark against
**SpiderMonkey/Warp writeups, not V8/TurboFan** — SpiderMonkey is the fundable-but-underfunded
trajectory we should match, and its docs are better.

**Switch criteria (all three):** ≥97% test262 · Speedometer within 2× of mozjs · zero P0
correctness diffs on the oracle top-100.

**Validation method:** esmeta-derived differential fuzzing with mozjs co-executing as oracle,
module-by-module. Late-track — the report notes the pipeline alone is a large CI project.

---

## 4. Process changes (how the loop itself must work differently)

The report and the fidelity finding both indict *process*, not just backlog order.

1. **The checklist is an INPUT; fidelity is the GATE.** Phase 0 exits on ≥0.75 structural fidelity
   on ≥95% of Tranco top-1000 plus ≥0.70 per top-20 category — **not** on a capability percentage.
   Capability % cannot see feature-present-but-site-broken.
2. **Re-probe stale `unknown` cells before building them.** The agent found WebAssembly, CJK line
   breaking, media queries, hydration, and the whole OAuth redirect flow already worked while
   carried as unknown. The checklist goes stale *from our own landed ticks*. A cheap re-probe pass
   must precede any build tick aimed at an `unknown`.
3. **Every green must have a demonstrated way to go red.** The agent's five-tick methodology
   thread: a scripted edit silently matching nothing after `cargo fmt` reflowed its target; an
   `if false` probe; two vacuous probes, one reporting a capability that does not exist.
4. **Never letter-code a lever set that collides with another decomposition on the same board.**
   "M-3" (fidelity) vs "M3" (symphonia demux) cost a tick. Named levers only.
5. **Re-check the moat decay clock quarterly.** WebMCP mass adoption ~mid-2027 is an estimate; if
   it accelerates, Phase 2 gets more urgent, not less.
6. **Publish falsifiers with claims.** Every headline claim in this doc has a named falsifier
   (§6). A claim without one is marketing.

---

## 5. Below the ROI line — remove from the board

The report names these as excludable *with evidence*. The board currently promotes the first one
as top-remaining item #7, which is a direct contradiction to fix:

- **Deep MathML** — rare outside academic/scientific sites
- **Exotic bidi edge cases** — keep basic bidi; drop the corners
- **Ancient DOM quirks** — `document.all` beyond truthiness, legacy `event.srcElement` chains
- **Non-load-bearing print-CSS corners**

Keep CJK line breaking (real user-minutes; already measured working). The exclusion is *deep*
MathML and *exotic* bidi, not the basics.

---

## 6. Trigger table (pre-committed, so these stop being re-litigated)

| Decision | Trigger to change course |
|---|---|
| Keep Taffy | >5% of top-1000 fidelity misses trace to layout math |
| Keep libvpx (C FFI) | Drop when AV1 corpus coverage >90% |
| Keep mozjs | Swap when Track R hits all three switch criteria |
| Memory-lean positioning | **Abandon the claim** if per-tab RSS lands within 20% of Chrome |
| Topology 5/6 | Move to 3/4 if cross-tab exfiltration proves uncontainable by egress controls |
| Phase 0 → Phase 1 | Oracle gate met — **not** checklist percentage |
| Interpreter-only Track R | Build a baseline JIT only if telemetry shows top-1000 spending >30% of main-thread time in provably hot loops |
| Agent-moat urgency | Re-check WebMCP adoption quarterly; accelerate Phase 2 if it moves faster than mid-2027 |

---

## 7. Deferred past Phase 6 (explicitly, with reasons)

Not "someday" — *decided against for v1*, revisitable only on a named trigger.

| Deferred | Why |
|---|---|
| **Optimizing JIT tier** | Rejected outright, not deferred: reintroduces the miscompilation CVE class the project exists to avoid. |
| **Full parity-annotated V8/SM port** | Upstream treadmill; both engines move constantly. Report ranks porting below build-from-scratch. |
| **Topology 4 (JS-engine-per-site)** | Reserved for an enterprise speciation with strict blast-radius rules; taxes the in-process diff moat that is the whole point. |
| **Formal verification of the capability layer** | v1's memory-safe core already eliminates the dominant class. |
| **Hardware memory tagging** | Platform-dependent; not a v1 differentiator. |
| **Model fine-tuning** | Needs dedicated ML focus + hardware; users can bring their own endpoint. |
| **Per-origin isolation, V8-backed speciation, crypto-wallet/x402, kiosk/embedded variants** | Pre-existing deferrals, unchanged. |
| **Deep WPT conformance** | A diagnostic, never a gate. |

---

## 8. The concede list (public, deliberate)

Peak JS throughput (JetStream/Octane) · MotionMark-class raster · WebGL/WebGPU creative apps ·
DRM/protected media · deep MathML, ancient DOM quirks, exotic bidi · cold start vs an
already-warm Chrome process.

Conceding these is strategic: it prevents losable comparisons and keeps the honest claims —
memory, agent substrate, safety-by-construction — credible.

---

## 9. Honest open risks

- **The broad fidelity number does not exist yet.** Everything about "how viable" hinges on it.
- **Agent-compute estimates for VM/compiler work are the least-known variable** — the report says
  so itself. Track R is scoped by *criteria*, never by a date.
- **No-pixels and focus-adaptive isolation are novel** — no incumbent ships them, so there is no
  external existence proof. Prototype and measure; do not assume.
- **The memory claim is unmeasured** and MEM-HARNESS may falsify it. That is the point.
- **A mozjs RCE today reaches the shared process.** Standing asterisk until Track R lands. State
  it plainly rather than hiding it.
