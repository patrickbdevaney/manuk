# Engineering Synthesis — Integrating the Competitive Research into the 0–6 Roadmap

**Source:** `ENGINEERING.MD` (decision-grade competitive research report).
**Written:** 2026-07-18, tick 232, Phase 0 at 52% readiness / 94% measured.
**Status:** this document supersedes the phase table in `docs/PROJECT-STATE-AND-VISION.md` Part 3.

---

## 0. The one-paragraph synthesis

The report does not change *what* Manuk is building — it changes *when* three specific
things get built. Most of its verdicts confirm decisions already made (no-ffmpeg, borrow
Stylo/html5ever/rustls, DRM out, oracle-as-prioritizer, concede peak JS). The genuinely new
content is a set of **architectural seams that are cheap to cut now and expensive to retrofit**
(`JsEngine` trait, content-taint boundary, no-pixels pipeline flag, NodeId durability contract),
a set of **load-bearing claims we assert but have never measured** (memory-lean per-tab RSS,
structural fidelity on a real corpus), and a **decay clock** on the agent moat (WebMCP, ~mid-2027)
that makes early agent-API leadership worth more than late perfection. The optimizing path is
therefore: **cut the seams now, fill them later; measure before marketing; keep the capability
grind as the main line.**

---

## 1. What the report changes vs. what it confirms

Separating these matters — most of the document is validation, and validation should not
consume tick budget.

### Confirms (no action; stop re-litigating)
| Decision | Report's basis |
|---|---|
| No ffmpeg; symphonia + cpal + rav1d | ffmpeg is a CVE farm; rav1d passes all dav1d tests, ~5% slower, runs in Chromium |
| DRM/Widevine permanently out | Licensed proprietary CDM; cannot be built |
| Borrow Stylo, html5ever, hyper, rustls, tokio-tungstenite, rustybuzz/swash | Commodities; owning them enables nothing for the agent thesis. "Borrowing commodities is maturity, not compromise" |
| Oracle as prioritization allocator, not WPT coverage | Presto/EdgeHTML died on compat *economics*; WPT measures spec conformance, not site breakage |
| Concede peak JS / MotionMark / WebGL creative | Structurally unwinnable; conceding prevents losable comparisons |
| Image decode in Rust is a real safety win | libwebp CVE-2023-4863 (CVSS 10.0, exploited in the wild, hit every browser) |

### Changes the plan (act on these)
| # | Change | Why it can't wait |
|---|---|---|
| **1** | **`JsEngine` trait seam in Phase 0–1**, not "someday" | Retrofitting after deeper mozjs coupling "costs multiples." This is the single highest-regret deferral in the report. |
| **2** | **Content-taint + capability-token boundary pulled from Phase 6 → Phase 2** | Prompt injection is a *product* threat and "cannot be fully solved at the model layer." Security-as-architecture cannot be bolted on. |
| **3** | **No-pixels agent pipeline + intent-aware scheduler pulled from Phase 5 → Phase 2–3** | They are moat features, not perf features. At 100 agent tabs they save the entire raster+GPU+composite budget. |
| **4** | **Tri-differential oracle (add Firefox)** | Chromium-only diffing overfits to Chromium bugs. Flag only where Manuk disagrees with *both*. |
| **5** | **A real Phase-0 exit criterion**: ≥0.75 structural fidelity on ≥95% of Tranco top-1000, plus ≥0.70 on each top-20 category | The 128-cap checklist measures *capability*; this measures *whether sites actually work*. The checklist is the input; this is the gate. |
| **6** | **Run the memory harness** (RSS+PSS via `smaps_rollup`, Tranco top-100, 10/50/100/500 tabs) | The memory-lean positioning is currently **unmeasured**. Per our own methodology: build the probe first. |
| **7** | **WebMCP in Phase 2** + lead a headless/native-substrate extension | Decay clock: W3C draft Feb 2026, Chrome origin trial, mass adoption ~mid-2027. WebMCP's *non-goals* (headless, autonomy) are exactly our goals. |
| **8** | **Rust JS engine as an explicit long-running track** with numeric switch criteria | Not a Phase; a background track that spans them, with a defined ingress swap. |

---

## 2. The organizing principle: three tracks, run in parallel

The phases are a sequence for *product*, but three kinds of work have different urgency logic
and should not be serialized behind each other.

**Track S — Seams (cheap now, expensive later).**
Cut the boundary; do not build what sits behind it. A trait with one implementation is nearly
free today and is the difference between a swap and a rewrite later.
→ `JsEngine` trait · taint at the DOM string boundary · capability-token type · no-pixels flag
in the paint path · NodeId durability contract + test.

**Track M — Measurement (redirects everything else).**
Every unmeasured claim is a claim we cannot prioritize against. Measurement is also the cheapest
category of work we have, and our own methodology memory says to build the probe first.
→ memory harness · tri-differential oracle · Tranco fidelity score + stopping rule · the
real-page JS-tier crawl the report flags as *nonexistent public data*.

**Track C — Capability (the main line).**
The existing Phase-0 grind, unchanged in content: media M3–M7, OAuth O2–O5, IndexedDB,
completeness identity, the remaining unknowns. This stays CO-#1 because Phase 0 gates everything.

Tracks S and M interleave as bounded ticks; they do not pause Track C.

---

## 3. The revised 0–6 roadmap

Changes from the previous table are **bold**.

| phase | name | what the product is at the end | changed |
|---|---|---|---|
| **0** | **Daily-driver capability** *(in progress, 52%)* | Renders and drives almost every mainstream website across doc/app/social/platform/media. **Exit is now the oracle gate (≥0.75 fidelity on ≥95% of Tranco top-1000 + category floors), with the 128-cap checklist as the input signal, not the gate.** | **+ `JsEngine` seam · + memory-harness baseline · + tri-differential oracle · + real exit criterion** |
| **1** | **UI/UX — tabs & session** | A browser you'd live in: session restore, tab ops, mute, pin. **The scheduler is agent-intent-aware from day one** — `U(tab) = P(use \| agent-intent) × warm_cost × memory_pressure`, with strict-minimizer and utility-warmer policies. **Focus-adaptive isolation (Topology 6) prototyped and measured here.** | **+ intent-aware scheduler · + focus-adaptive isolation · + finish `JsEngine` seam** |
| **2** | **Agent API surface** (the ingress) | The stable, pinnable contract external agents drive. **Now also the security and no-pixels phase:** diff receipts on every action, published NodeId durability contract, content-taint + capability tokens enforced at the engine boundary, no-pixels agent tabs, WebMCP support + native-substrate spec extension. | **+ taint/capability (from 6) · + no-pixels (from 5) · + WebMCP · + diff receipts · + NodeId contract** |
| **3** | **Default agent harness** | The LLM-facing toolset + orchestrator on the Phase-2 ingress; three deployment modes against one surface. **Deterministic replay/time-travel for agent debugging lands here.** | **+ deterministic replay** |
| **4** | **Consumer prompt-to-action GUI** | In-browser prompt bar; bundled small gguf or bring-your-own endpoint. | unchanged |
| **5** | **Performance** | **Narrowed to raster and parallelism** — Vello/wgpu compute raster, parallel layout, off-main-thread parse. Memory and scheduling already shipped in 1–2. | **narrowed (memory/scheduler moved earlier)** |
| **6** | **Security** | **Narrowed to sandbox and supply chain** — seccomp-BPF + Landlock, W^X for any JIT, cargo-vet/cargo-audit at engine scale, multi-OS sandbox parity. The injection/capability architecture already shipped in Phase 2. | **narrowed (taint/capability moved earlier)** |
| **R** | **Rust JS engine** *(track, not phase)* | Spans all phases behind the `JsEngine` seam. Interpreter + baseline only — **explicitly never an optimizing tier**, which would reintroduce the miscompilation CVE class the project exists to avoid. | **new track** |

### The Track-R switch criteria (all three must hold)
1. ≥97% test262
2. Speedometer within 2× of mozjs
3. Zero P0 correctness diffs on the oracle top-100

Until then mozjs stays, contained, with the C++ asterisk stated honestly rather than hidden.

---

## 4. Near-term tick queue

Track C (media/OAuth/canvas/unknowns) remains CO-#1 and is unchanged. The following interleave.

**Seam ticks (do these early — the whole argument is that delay multiplies cost):**
- `JsEngine` trait extraction: realms, GC handles, host hooks, module loading, microtask queue.
  One implementation (mozjs). Gate: engine compiles and all JS gates pass with every call site
  routed through the trait.
- Taint flag on content-originated DOM strings; capability-token type with task scope. Inert at
  first — the *boundary* is the deliverable, enforcement follows in Phase 2.
- No-pixels flag in the paint path: layout + a11y tree, paint elided. Gate: an agent tab produces
  a correct a11y tree with zero display items.
- NodeId durability test: identity survives re-render/hydration. Gate: stable IDs across a
  framework re-render.

**Measurement ticks:**
- Memory harness: `smaps_rollup` RSS+PSS, Tranco top-100 category-stratified, hold at 10/50/100.
  Publish median + p90 per-tab RSS. **This can falsify our own positioning** — if we land within
  20% of Chrome, the memory thesis collapses and we should stop marketing it.
- Add Firefox as a second oracle reference; flag only two-way disagreements.
- Tranco structural-fidelity score wired to the Phase-0 exit rule.

**The unique-data tick (cheap, and nobody else has published it):**
The report flags that *no public dataset partitions real-page main-thread JS into
optimized-tier vs parse/compile/interpreter/glue*. We run a crawler and control an engine.
Producing that breakdown is a genuine external contribution and it directly tests Track R's
core premise — it is the falsifier for "the peak-JIT gap doesn't matter."

---

## 5. Trigger table (decisions with pre-committed conditions)

Pre-committing these prevents re-litigating them every time the topic resurfaces.

| Decision | Trigger to change course |
|---|---|
| Keep Taffy | >5% of top-1000 fidelity misses trace to layout math |
| Keep libvpx (C FFI) | Drop when AV1 coverage on the target video corpus >90% |
| Keep mozjs | Swap when Track R hits all three switch criteria |
| Memory-lean positioning | Abandon the claim if per-tab RSS lands within 20% of Chrome |
| Topology 5/6 (shared agent process) | Move to Topology 3/4 if cross-tab exfiltration proves uncontainable by egress controls |
| Phase 0 → Phase 1 | Oracle gate met, **not** checklist percentage |
| Interpreter-only Track R | Build a baseline JIT only if telemetry shows top-1000 sites spending >30% of main-thread time in provably hot loops |

---

## 6. What we concede, publicly and on purpose

Peak JS throughput (JetStream/Octane) · MotionMark-class raster · WebGL/WebGPU creative apps ·
DRM/protected media · deep MathML, ancient DOM quirks, exotic bidi · cold start vs an already-warm
Chrome process.

Conceding these is strategic, not defeatist: it prevents losable benchmark comparisons and keeps
the honest claims — memory, agent substrate, safety-by-construction — credible.

---

## 7. Honest open risks

- **Agent-compute estimates for VM/compiler work are the least-known variable.** The report says
  so itself. Track R is scoped by *criteria*, not by a date, precisely because of this.
- **No-pixels and focus-adaptive isolation are novel** — no incumbent ships them, so there is no
  external existence proof. Both must be prototyped and measured, not assumed.
- **The memory claim is unmeasured** and the harness may falsify it.
- **The moat decay clock is an estimate.** WebMCP adoption should be re-checked quarterly; if it
  moves faster than mid-2027, Phase 2 gets more urgent, not less.
- **A mozjs RCE today reaches the shared process.** This is the standing asterisk on the safety
  story until Track R lands. Say it plainly rather than hiding it.
