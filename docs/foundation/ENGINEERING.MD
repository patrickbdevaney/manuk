# Engineering Manuk to Maximum Competitive Potential: A Decision-Grade Research Report

## TL;DR
- **Ship an interpreter-plus-baseline memory-safe Rust JS engine — but stage it, and keep mozjs behind a `JsEngine` trait now.** The V8↔SpiderMonkey gap is largely a funding artifact on the axes Manuk cares about (SpiderMonkey trails JavaScriptCore ~17% on Speedometer 3.1 and ~38% on JetStream 2.2, per Magic Lasso Jan 2026), and V8's own "JIT-less V8" data shows an engine with no optimizing compiler is only ~40% slower on Speedometer 2.0 and ~6% slower on a simulated real YouTube session. Switch ingress from mozjs when a Rust engine crosses ≈97% test262 + Speedometer-within-2× (~18–36 agent-compute-months out).
- **The agent-substrate moat is real and defensible for 1–3 years but decaying on a visible clock.** Every serious competitor (Comet, Atlas, Claude, Operator, Dia, Opera Neon, Edge Copilot, Brave Leo) wraps Chromium or controls the OS and pays the CDP/DOM/screenshot tax; Manuk's in-process synchronous `to_observation_lines` + native tree-diff is a genuine category difference. But Chrome shipped WebMCP (W3C draft, Feb 10 2026; Chrome 146/149 origin trial) and Auto Browse (Jan 28 2026), so the window to compound the lead is finite (mass WebMCP adoption estimated ~mid-2027).
- **Win on per-tab memory, no-pixels agent tabs, intent-driven tab scheduling, and Rust image/media safety; concede peak JS, MotionMark-class raster, and WebGL creative apps publicly.** These fronts are structurally winnable by a lean Rust core; the concede list prevents losable benchmark comparisons.

---

## Key Findings

1. **The JS performance gap is workload-dependent and mostly irrelevant to Manuk's profile.** V8's own "JIT-less V8" post (v8.dev/blog/jitless): Speedometer 2.0 only ~40% slower without the optimizing compiler; Web Tooling Benchmark 80% slower; a simulated YouTube living-room session only ~6% slower. TurboFan buys 4.35× on JetStream but only >1.5× on Speedometer (v8.dev/blog/maglev). Real interaction-heavy sites derive limited benefit from the optimizing tier.
2. **Interpreter-only conformance is a solved problem.** QuickJS (Fabrice Bellard, ~one-genius-year) reaches ~99% test262 with ES2025, interpreter-only, at 2–3× V8's interpreter / 20–100× TurboFan. Boa (Rust) is ~95.5% test262 (register VM, NaN-boxing, no JIT); Ladybird's LibJS ~97.8% of test262 subtests, deliberately JIT-less with a hand-written assembly interpreter (AsmInt).
3. **Memory safety eliminates the classes constituting ~70% of severe browser CVEs by construction — but not in the JIT tier.** Chromium's canonical figure (chromium.org/Home/chromium-security/memory-safety): "Around 70% of our high severity security bugs are memory unsafety problems... Half of those are use-after-free bugs (Analysis based on 912 high or critical severity security bugs since 2015)." V8 type-confusion zero-days recur (CVE-2025-13223, exploited in the wild). A Rust interpreter eliminates JS-heap UAF/OOB by construction; a Rust optimizing JIT would reintroduce miscompilation risk Rust does not prevent.
4. **Site isolation costs 10–13% RAM; Manuk's memory-safe core can invert the topology.** Because DOM/layout/paint are memory-safe Rust, the residual compromise surface is mostly the JS engine — arguing for "JS-engine-per-site, everything else shared," a topology neither incumbent can use because their whole renderer is unsafe.
5. **Tab scheduling is Manuk's structural win.** Microsoft Edge Blog (Edge 100): "each sleeping tab saves 85% of memory and 99% CPU"; the original experiment across ~13,000 devices showed a "median memory usage reduction of 26%." But no incumbent scheduler has the signal Manuk has — the agent declares intent about which tabs it will need and when.
6. **The compat long tail killed Presto and EdgeHTML; the differential fidelity oracle is the counter.** Interop 2026 focus areas (Apple/Google/Igalia/Microsoft/Mozilla) reveal what is load-bearing; a tri-differential oracle (Chromium + Firefox) as a user-minute-weighted prioritization allocator is a defensible advantage over naive greenfield engines.
7. **Media and images are a clean Rust safety win.** libwebp's CVE-2023-4863 (reassigned CVE-2023-5129, CVSS 10.0, exploited in the wild, affected virtually every browser) is the archetype; rav1d (Rust AV1) passes all dav1d tests, runs in Chromium, and is ~5% slower than dav1d (ISRG Prossimo); image-rs eliminates a classic CVE farm by construction.
8. **The industry backdrop favors the play.** Mozilla is ~86% Google-search-revenue dependent ($510M of $593M in 2021–2022, per its financial statement via Fortune) and cut ~30% of foundation staff; Google keeps Chrome (divestiture rejected Sept 2 2025); Servo runs on <$8k/month and still embeds C++ SpiderMonkey; Ladybird is the only other from-scratch engine and it is not an agent substrate.

---

## DOMAIN A — The JavaScript Engine Question

### A1. The actual V8↔SpiderMonkey delta
Current third-party benchmarks (Magic Lasso, Jan 2026, M2 MacBook Air): Chrome/V8 and Safari/JavaScriptCore are effectively tied on Speedometer 3.1 (42.7 vs 41.9) and JetStream 2.2 (419.2 vs 436.8). **Firefox/SpiderMonkey trails: ~17% slower on Speedometer 3.1 (35.7 vs 41.9) and ~38% slower on JetStream 2.2 (270.9 vs 436.8).** Chrome hit a record 52.35 on Speedometer 3.1 (Chrome 139, M4, June 2025) and improved JetStream ~10% to a record 469 in 2026.

**Attribution:** The SpiderMonkey lag is substantially a funding artifact. V8 added Maglev (mid-tier optimizing compiler) and Sparkplug (baseline JIT) as *additional* tiers — headcount investments. SpiderMonkey has three tiers (interpreter + 2 JITs) vs V8/JSC's four. The delta is concentrated in the *optimizing tier*: on Speedometer, TurboFan buys V8 only >1.5×, and SpiderMonkey's gap is ~17% — the gap on real-interaction workloads is a fraction of the peak-throughput gap.

**Separating peak from baseline:** V8's own "JIT-less V8" post is the decisive primary source. Disabling the optimizing compiler entirely: Speedometer 2.0 only **~40% slower** (half attributable to the optimizing compiler, half to the regex interpreter); Web Tooling Benchmark 80% slower; **a simulated YouTube session only ~6% slower.** V8's Maglev post confirms JetStream = 4.35× from TurboFan but Speedometer only >1.5×.

**Real-page JS distribution:** No single published HTTP Archive/Chrome-tracing breakdown cleanly partitions real-page main-thread JS time into optimized-tier vs parse/compile/interpreter/glue (FLAG: this data does not exist publicly; Manuk should crawl it). The Web Almanac 2019 gives median total V8 main-thread time of 849 ms desktop / 2,437 ms mobile but does not split by tier. The strongest evidence is indirect: JIT-less V8's near-parity on real workloads (~6% YouTube, ~40% Speedometer) demonstrates that real sites spend little time in optimized hot code — most real-page JS is short-lived parse/compile/one-shot/glue that never gets hot. Edge's "Super Duper Secure Mode" disables JIT "with minor performance impact for daily browsing."

**Verdict A1 (confidence: HIGH):** For Manuk's agent-first profile (hydration, event handling, fetch glue) plus a tab scheduler that keeps hot JS rare, the peak-JIT gap is largely irrelevant. The honest user-perceivable delta with an interpreter+baseline engine on Tranco/CrUX top-1000 is single-digit-to-40% on interaction, invisible on most navigation. **Falsifier:** telemetry showing top-1000 sites spending >30% of main-thread time in provably hot loops would make the optimizing tier load-bearing.

### A2. The CVE spine — what memory safety buys in the JS tier
Chromium's canonical figure: **~70% of high/critical severity security bugs are memory-safety problems** (912-bug analysis since 2015), half use-after-free. V8 type-confusion zero-days recur continuously: CVE-2025-13223 (exploited in the wild, TAG), CVE-2025-13224/13226/13228/13229/13230, CVE-2024-12692, CVE-2026-6363 — all type confusion in a C++ engine.

**Classification:**
- **Eliminated by construction in Rust:** JS-heap use-after-free, out-of-bounds read/write in the runtime/interpreter, GC-object UAF — the class that constitutes most V8/SM exploited CVEs.
- **Reduced but not eliminated:** A JIT tier — Rust does not save you from a *miscompiling* compiler emitting wrong machine code (type-confusion-equivalent logic bugs in the optimizer). Strongest argument for interpreter-first.
- **Persists entirely:** Spec-logic bugs (CVE-2020-6418-class incorrect side-effect modeling), side channels (Spectre).

**Incumbent counterfactual:** V8 Sandbox (in-heap, shipped April 2024) limits OOB within the heap; MiraclePtr mitigated **57% of use-after-free vulnerabilities in privileged processes** (Google Online Security Blog, Jan 2024: "exceeding our initial estimate of 50%," from 168 UAF reports), turning UAF into non-exploitable crashes via BackupRefPtr/PartitionAlloc quarantine at ~7% main-thread contention cost. But MiraclePtr does not cover the renderer's most-attacked surface, and the V8 sandbox is explicitly not a memory-safety guarantee.

**Deliverable sentence (confidence: HIGH):** *A Rust engine with an interpreter + baseline JIT eliminates roughly the ~55–65% of historically exploited V8/SM CVEs that are JS-heap UAF and runtime OOB by construction, versus the ~57% of use-after-frees (a subset) that V8 neutralizes via MiraclePtr and the partial OOB containment of the V8 Sandbox — but a Rust optimizing JIT would reintroduce the miscompilation class that neither Rust nor sandboxing fully eliminates.* **Falsifier:** a root-cause census of 2018–2026 exploited-in-the-wild V8 CVEs showing >40% are spec-logic or JIT-miscompilation rather than heap UAF/OOB.

### A3. From-scratch Rust engine feasibility (calibration)
| Engine | Language | test262 | Tier | Investment | Cap / note |
|---|---|---|---|---|---|
| QuickJS/-ng | C | ~99% (ES2025) | Interpreter-only | ~1 genius-year (Bellard) | 2–3× V8 interp / 20–100× TurboFan |
| Boa | Rust | ~95.5% | Register VM, NaN-boxing, no JIT | ~6 yr, dozens of contributors | perf (conformance near cap) |
| Ladybird LibJS | C++/Rust | ~97.8% subtests | AsmInt hand-asm interp, JIT-less | part of full browser | ~40× slower Speedometer (2023; much improved since) |
| Nova | Rust | ~80% | Data-oriented/ECS, no JIT | small team, NLnet grant, since 2023 | 99% targeted "end of next year" |
| Kiesel | Zig | ~90–95% | Bytecode VM, no JIT | solo (Linus Groh), 608 commits since 2023 | for-fun project |
| Starlight | Rust | abandoned | JIT attempt | solo | killed by unsafe-JIT maintenance burden |
| LibJS JIT | C++ | (removed) | Cranelift/custom | — | removed Feb 2024: "complexity and security burdens... not reasonable" |

**Calibration synthesis:** QuickJS is the existence proof that near-100% conformance is achievable interpreter-only by ~one engineer-year. Boa proves a Rust engine reaches ~95.5% in ~6 contributor-years. Ladybird's AsmInt (hand-written assembly bytecode interpreter, gains: Kraken 1.44×, Octane 1.37×, SunSpider 1.69×) shows JIT-less can be pushed hard. **The conformance ceiling is not the risk; performance and web-embedding integration are.**

**Estimates (agent-compute-months; least-known variable — FLAG):**
- (a) Full ES2026 + tracked test262 at interpreter speed: **~12–24 engineer-equivalent-months** from a strong start; with agentic-coding multipliers (undocumented for VM workloads), perhaps **8–18 agent-compute-months**.
- (b) + baseline JIT within 2–5× V8 baseline tier: **+12–24 months**; miscompilation CVE risk enters, ROI drops.
- (c) + optimizing tier: **+24–48 months** and reintroduces the exact CVE class Manuk exists to avoid. **Do not build (c).**

**Agent-first shortcut (confidence: HIGH):** Ship interpreter+baseline; be product-competitive because the JS profile is glue/hydration-heavy, tab scheduling keeps hot JS rare, and peak-JIT sites are outside honest scope. Modeled delta on Tranco top-1000 vs mozjs: near-parity on navigation, single-digit-to-40% on heavy interaction — acceptable.

### A4. Transpilation / parity-annotated porting
**Prior art:** C2Rust (Immunant) produces unsafe raw-pointer Rust requiring heavy manual rewrite; rav1d (dav1d→Rust via c2rust + manual) is the strongest existence proof — fully functional, passes all dav1d tests, runs in Chromium, ~5% slower. DARPA TRACTOR (2024–2025, ~$14M; teams incl. ForCLift verified-lifting at UIUC/Wisconsin/Berkeley/Edinburgh, $5M) targets exactly this but is **research-stage, not production** as of 2026; LLM C→Rust "often very good, but not always" (Wallach) and hampered by scarce Rust training data.

**V8 vs SpiderMonkey portability:** V8 is heavy C++ templates/inheritance — the *hardest* case. SpiderMonkey's older tiers are more C-ish. Neither is as tractable as dav1d (self-contained ~C). A full engine port is qualitatively harder than any TRACTOR success to date.

**Formal anchors:** ECMA-262 is increasingly mechanized; esmeta (spec-extracted interpreters + conformance fuzzing); test262 as oracle. A "parity-annotated port" gated module-by-module on behavioral equivalence via esmeta-derived differential fuzzing with the incumbent engine co-executing as oracle is sound but a large CI-engineering project (~6–12 months just for the pipeline).

**Licensing:** V8 is BSD-3 (permissive, attribution only); SpiderMonkey is MPL-2.0 (file-level copyleft, viral only at file granularity). **Upstream tracking tax:** both move constantly; a port becomes a treadmill — the decisive argument against porting.

**Verdict A4 ranking (confidence: MEDIUM-HIGH):**
1. **Build-from-scratch interpreter (Boa/Nova-informed)** — best long-term; owns the safety story; no treadmill.
2. **Keep-mozjs-and-sandbox-it** — best near-term (A5).
3. **Port-SM** — MPL treadmill; marginal.
4. **Port-V8** — template hell + BSD treadmill; worst.
**Falsifier:** TRACTOR/ForCLift demonstrating verified, performance-preserving translation of a template-heavy C++ codebase of V8's scale by 2027 would reorder this.

### A5. Keep-mozjs-but-contain-it counterfactual
Strongest containment: **RLBox-style SFI**, Wasm-sandboxing the JS engine (wasm2c/WAMR precedents; measurable perf tax), or **dedicated JS process per site with shared-nothing DOM proxies** — but the last taxes Manuk's in-process synchronous diff moat (Domain B), which is the whole point. Manuk has already hit the two-contexts-per-process constraint and calc-size-SIGSEGV-class bugs; these are embedder pain fixable upstream only slowly.

**Decision rule (confidence: HIGH):** Define a `JsEngine` trait boundary NOW (realms, GC handles, host hooks, module loading, microtask queue) so mozjs and a Rust engine coexist. Switch when the Rust engine crosses **≥97% test262, Speedometer within 2× of mozjs, and zero P0 correctness diffs on the oracle top-100.** Retrofitting the seam now costs ~2–4 months; retrofitting after deep mozjs coupling costs multiples more. Build the seam now regardless of switch date.

---

## DOMAIN B — Isolation Architecture

**Incumbent costs:** Chromium site isolation = **10–13% RAM overhead** on desktop (Charlie Reis, Chrome 67), 3–5% on Android (login-only); per-tab renderer ~80–300 MB (80 MB Wikipedia, 200–300 MB YouTube). Firefox Fission is comparable. IPC round-trip for CDP-style observation (serialize→IPC→deserialize) is the tax Manuk's in-process synchronous calls avoid.

**Re-derived 2026 threat model:** Site isolation splits between Spectre-class cross-origin reads and renderer-compromise containment. With a **memory-safe Rust renderer, the renderer-compromise half largely evaporates** — the residual compromise surface is mostly the (C++, today) JS engine. Key asymmetry: **"JS-engine-per-site in its own process, everything else (memory-safe DOM/layout/paint) shared"** — a topology neither incumbent can use because their entire renderer is unsafe.

**Topology model:**
| Topology | Per-tab mem | Cold-tab | Warm-tab | Agent observe→act | Blast radius |
|---|---|---|---|---|---|
| 1. Single-proc, memory-safe, mozjs in-proc | Lowest | Lowest | Lowest | **In-process, ~µs** | High (mozjs bug = all tabs) |
| 2. Process-per-tab | Highest | High | Med | IPC-taxed | Lowest |
| 3. Process-per-site | High (10–13%) | High | Med | IPC-taxed | Low |
| 4. Shared engine proc + per-site JS sandbox procs | Medium | Med | Med | Partial IPC | JS contained, DOM shared |
| 5. SFI/Wasm-contained JS, one process | Low | Low | Low | In-process | JS logically contained |
| 6. **Focus-adaptive hybrid** | Low (agent tabs collapsed) | Low | Low | In-process for agent tabs | Foreground isolated on focus |

**Topology 6 — focus-adaptive isolation — appears novel.** No incumbent does it (their compositor/process model assumes every tab is a potential foreground pixel surface). Agent-driven background tabs collapse into one shared low-privilege process (preserving the in-process diff moat); the human foreground tab is promoted to an isolated process on focus. Sound *if* promotion is crossed only on focus transitions (rare) and the shared agent process runs at reduced privilege with egress controls.

**Recommendation (confidence: MEDIUM-HIGH):** Optimize **Topology 5/6 hybrid for v1**; reserve **Topology 4 (JS-engine-per-site)** for the enterprise speciation where blast-radius rules are strict. Config knob to keep both = the `JsEngine` seam + a process-placement policy layer (~1–2 months). **Falsifier:** a demonstrated cross-tab exfiltration via the shared agent process that egress controls cannot contain would force Topology 3/4 for v1.

---

## DOMAIN C — Performance: Where a Lean Rust Engine Wins

**Per-tab memory & tab-count scaling:** Chrome 150 ~6.5 GB at 50 tabs; Firefox 152 ~3.8 GB (lowest mainstream); Chrome+suspender ~1.8–2.2 GB (architecture-based estimates, SuperchargeBrowser). A fresh Rust core avoids legacy object-graph overhead. **Measurement plan (Manuk must run this — data does not exist for Manuk):** RSS + PSS via `/proc/[pid]/smaps_rollup`; corpus = Tranco top-100 category-stratified; harness = headless load-and-hold at 10/50/100/500 tabs; metric = median + p90 per-tab RSS delta and total RSS. Target: a credible "100-tab RSS budget" of ~2–3 GB (~2× better than Chrome).

**Cold start / page-load / binary size:** Chromium's process model, field-trial config, and feature mass cost startup. Ladybird/Servo/Flow calibrate what a lean engine achieves. **Honest V1 bar:** "feels good, no pathological hangs" as floor; winnable metrics above it: click-to-first-paint and cold process start on a lean binary. Define a *fair and winnable* suite: navigation-heavy corpus, cold-start, and memory — NOT compute JS.

**Tab hibernation vs warming as first-class research:** Prior art — Chrome freezing/discarding + Performance Detection; **Edge Sleeping Tabs (each sleeping tab saves 85% of memory and 99% CPU; median device memory reduction 26% across ~13,000 devices; 1.38 billion tabs slept in Sept 2022)**; Firefox tab unloading; Arc auto-archive; academic MRU/Markov next-tab prediction. **Manuk's unique advantage: the agent declares intent** — it knows which tabs it will need and when, a signal no incumbent scheduler has.

*Scheduler design:* utility function `U(tab) = P(use | agent-intent) × warm_cost × memory_pressure_weight`; two policies — **strict-minimizer** (evict all but declared-needed, for agent-batch) and **utility-warmer** (pre-warm declared-next, for human-interactive). Cold-start amortization via snapshot/restore (compare Chromium bfcache + page-freezing). *Evaluation:* replay agent task traces + human session traces; metrics = memory-at-p90, wake latency, "needed-tab-cold-miss rate." A concrete agenda, not a literature list.

**Parallelism & paint:** Stylo already parallelizes style. Servo's parallel layout sped up some workloads; Ladybird now does off-main-thread parsing/bytecode-gen in Rust (safe parser parallelizes trivially). Vello/wgpu compute-raster is maturing vs tiny-skia CPU paint. **The large structural win incumbents can't ship: an agent-mode "no-pixels" pipeline** — layout + a11y tree only, paint elided for unobserved tabs. Incumbents' compositors assume pixels. At 100-agent-tab scale this saves the entire raster + GPU-upload + compositing budget per background tab (plausibly the majority of per-tab CPU/memory). **Falsifier for the memory thesis:** Manuk's harness showing per-tab RSS within 20% of Chrome would collapse the memory-lean positioning.

**Where NOT to compete (concede publicly):** peak JS throughput (JetStream/Octane), MotionMark-class raster throughput, WebGL/WebGPU creative apps.

---

## DOMAIN D — The Compat Long Tail

**Historical autopsy:** Presto (Opera) and EdgeHTML (Microsoft) were abandoned on stated compat-cost economics — maintaining an independent engine against a Chrome-shaped web became uneconomic. KHTML→WebKit and the general pattern show the "independent-engine death spiral": marginal compat costs rise while user-visible value flattens. Ladybird is the live test case. The cost curve is power-law: top-1000 sites cover most user-minutes; marginal value flattens fast beyond the head.

**WPT vs reality:** Interop 2026 (Apple, Google, Igalia, Microsoft, Mozilla) focus areas signal what the industry considers load-bearing: anchor positioning, container style queries, `<dialog>`/popover, view transitions, scroll-driven animations, WebRTC, Web Compat (ESM module loading, scroll/animation event timing, `user-select`). Firefox went 46→99 on a single-year focus-area score; overall Interop went 25→95 — targeted, test-driven work closes gaps fast. Load-bearing legacy: quirks-mode layout, `document.all`, DOM0 events, some UA-sniffing branches. HTTP Archive custom metrics can quantify per-feature usage (Manuk should crawl this).

**Oracle strategy (pressure-tested):** Differential structural-fidelity testing against Chromium on a crawled corpus is a superior *prioritization allocator* to WPT-driven development because it weights by real-site breakage, not spec coverage. Corpus: **Tranco top-N, category-stratified, weighted by user-minutes and agent-task frequency.** To prevent overfitting to Chromium bugs, go **tri-differential (Chromium + Firefox)**: flag only diffs where Manuk disagrees with *both*. Strongest version: continuous crawl → structural-fidelity score → auto-file highest-user-minute regressions → agentic fix loop.

**Stopping rule / "compat done for v1" (confidence: HIGH):** ≥0.75 structural fidelity on ≥95% of Tranco top-1000 **plus** category floors (≥0.70 on each of the top 20 categories). Estimated agent-compute cost from 52% capability / 94% measured: substantial but bounded by the power-law. **Below-the-line ROI classes to exclude with evidence:** deep MathML (rare outside academic/scientific sites), exotic bidi edge cases, ancient DOM quirks (`document.all` beyond truthiness, legacy `event.srcElement` chains), non-load-bearing print-CSS corners.

---

## DOMAIN E — Build vs. Borrow Framework

**Framework axes:** (1) security surface (memory-unsafe FFI mass, dep CVE density); (2) license; (3) maintenance velocity/bus factor; (4) integration-seam cost; (5) differentiation value (does owning it enable the agent thesis?); (6) replacement cost later. **Industry prior:** everyone borrows commodities (Chromium ships FFmpeg/ICU/SQLite/BoringSSL/ANGLE; Firefox ships ICU/dav1d/cubeb; Ladybird adopted Skia). Borrowing commodities is maturity, not compromise.

| Dependency | Verdict | Rationale / trigger |
|---|---|---|
| **mozjs** | **Borrow-then-replace at milestone** (Rust engine ≥97% test262, Speedometer <2×) | The one strategic liability; C++ safety asterisk. Build `JsEngine` seam now. |
| **Stylo** | **Borrow** (permanent) | Owning cascade is enormous; already Rust and memory-safe; owning it enables nothing for the agent thesis. |
| **html5ever** | **Borrow** (permanent commodity) | Spec-frozen parsing, Rust, memory-safe. Zero differentiation. |
| **Taffy** | **Borrow now; revisit if oracle demands** | Trigger: >5% of top-1000 fidelity misses trace to layout math. |
| **hyper / rustls** | **Borrow** (permanent) | Rust, memory-safe; rustls is a safety win over OpenSSL. |
| **tokio-tungstenite** | **Borrow** (permanent) | Commodity, Rust. |
| **symphonia / cpal** | **Borrow** (permanent) | Rust audio decode/output; memory-safe. |
| **rav1d (AV1)** | **Borrow** | Passes all dav1d tests, runs in Chromium, ~5% slower; memory-safe. Confirms no-ffmpeg is right. |
| **libvpx (VP9) via FFI** | **Borrow reluctantly; steer to AV1/rav1d** | VP9 via C FFI undermines the safety story. In 2026 YouTube still serves VP9 widely. Trigger to drop libvpx: AV1 coverage on target video corpus >90%. |
| **Image decoders (image-rs)** | **Build/borrow Rust — genuine safety win** | libwebp CVE-2023-4863 (CVSS 10, exploited in the wild, affected virtually all browsers) is the archetype. Rust decoders eliminate this class by construction. High differentiation. |
| **Text shaping (rustybuzz/swash vs HarfBuzz)** | **Borrow rustybuzz/swash** | Rust ports; memory-safe; shaping is a historical CVE source. |
| **ICU / Unicode** | **Borrow (icu4x if viable)** | Commodity; icu4x is the Rust path. |

**No-ffmpeg / no-DRM confirmed correct (confidence: HIGH):** ffmpeg is a CVE farm; the symphonia + rav1d + libvpx(reluctant) stack is the memory-safe path. DRM is out of honest scope.

---

## DOMAIN F — Maximizing the Agent-Substrate Moat

**Tax audit of the incumbent path:** Agents pay: (1) CDP/WebDriver-BiDi serialize→IPC→deserialize per observe→act→observe cycle; (2) DOM re-serialization; (3) screenshot observation — tens of thousands of tokens per screenshot vs a few thousand for the a11y tree. Concrete token data: raw DOM ~125,000 tokens on a content page; filtered a11y tree ~15,000 (content-heavy) down to ~200–600 (interactive-focused); Playwright MCP "burns 114K tokens per test." Accessibility-tree parsing is ~93% more token-efficient than raw DOM. Manuk's `to_observation_lines` — in-process, actionability-preserving — targets the sub-1K-token end with zero IPC/serialization tax.

**Competitive teardown (2025–2026):**
- **Perplexity Comet:** agentic behavior via an internal Chrome extension doing DOM injection in the renderer — largest attack surface; Brave demonstrated indirect prompt injection (patched, not architecturally eliminated).
- **OpenAI Atlas:** separates the agent from Chromium via "OWL" (OpenAI Web Layer), native IPC (no JS injection), lower latency, access to rendered + raw HTML — architecturally the strongest competitor, but still Chromium-wrapping and prompt-injectable ("Tainted Memories" CSRF).
- **Claude (Computer Use / Cowork):** OS-level, screenshot-driven, permission-per-action — safest default but highest observation tax.
- **Operator/browser-use/Stagehand/Playwright:** screenshot + a11y/DOM scraping, flaky, token-heavy.
- **Chrome's trajectory (moat decay clock):** WebMCP (W3C draft Feb 10 2026, Chrome 146/149 origin trial) inverts control so sites declare tools to agents ("89% token efficiency vs screenshots"); Auto Browse (Gemini 3, Jan 28 2026). But WebMCP requires site opt-in, has no headless mode, and no major agent calls it in production yet (mid-2027 mass-adoption estimate) — leaving Manuk a window.

**Compounding-moat capabilities, ranked by (moat value × tractability):**
1. **Native semantic tree-diff with transactional action→diff receipts** ("what changed after my action"). Highest value; already built. Spec: every agent action returns a structural diff receipt (added/removed/mutated nodes with stable IDs) synchronously in-process. No incumbent has this without IPC-taxed re-serialization.
2. **Stable NodeId durability across mutations.** Guarantee node identity survives re-render; publish the durability contract. Eliminates the #1 source of agent flakiness (selector breakage).
3. **Injection-resistance as an engine property.** Capability-scoped action permissions with page-content-as-untrusted-input enforced at the engine boundary — not a prompt. Spec: content-originated strings are tainted at the DOM boundary; agent actions require a capability token scoped to the task; tainted content can never escalate to a capability. Operationalizes Meta's "Rule of Two" (untrusted input + sensitive access + state change = at most two) *in the engine*. Nobody has this.
4. **Occlusion/visibility-truthful hit-testing** (already built). Publish as a differentiator: agents never "click" an occluded element.
5. **Deterministic replay / time-travel for agent debugging.** Record observation+action streams; replay deterministically.
6. **Speculative pre-navigation/prefetch driven by declared agent intent** (ties to Domain C scheduler).
7. **Parallel multi-tab agent execution with the intent-aware scheduler.**
8. **Token-optimal page representations** (rule-based compression preserving actionability) — the `to_observation_lines` line of work.
9. **WebMCP implementation + lead a headless/native-substrate extension.** WebMCP's non-goals (headless, full autonomy) are exactly Manuk's goals — implement WebMCP for compat AND lead a spec extension for the native in-process case.

---

## DOMAIN G — Industry Landscape and Timing

**Funding/velocity asymmetry:** Mozilla is existentially Google-dependent — **~86% of revenue from Google search payments in 2021–2022 ($510M of $593M, per its financial statement via Fortune)**, ~$495M in 2023 (three-quarters of revenue); foundation cut ~30% of staff; corporation cut ~60 roles in Feb 2024. So SpiderMonkey's trajectory (fundable-but-underfunded) is *the* one to benchmark against, and Manuk should borrow reference behavior from the leaner, better-documented SpiderMonkey/Warp writeups rather than chasing V8's TurboFan.

**Chrome divestiture (Sept 2 2025, Judge Mehta):** Google **keeps Chrome and Android** (structural divestiture rejected as overreach); behavioral remedies only (no exclusive default contracts; data-sharing with competitors); a separate ad-tech remedy is pending. **Moat-math implication:** Chrome stays inside Google, so V8/Chrome's agent-surface investment (WebMCP, Auto Browse) continues at full force — the decay clock keeps running. A spun-out Chrome (did not happen) would have slowed that; it didn't, so plan for continued Google pressure.

**Incumbent memory-safety migration pace (the clock on Manuk's safety differentiation):** MiraclePtr mitigates 57% of privileged-process UAF; V8 Sandbox shipped April 2024 and is maturing; Chromium adds Rust incrementally (JPEG XL Rust decoder experiment; Firefox Oxidation ongoing). Incumbents will not be memory-safe wholesale for many years (no whole-renderer rewrite underway), so Manuk's by-construction safety story has a multi-year runway — but the *marketing* differentiation narrows yearly as MiraclePtr/sandbox close specific classes.

**Agentic gold rush — is "only non-wrapper" true?** Verified: Comet, Atlas, Claude, Operator, Dia, Opera Neon, Edge Copilot, Brave Leo **all wrap Chromium or control the OS**. Ladybird is from-scratch but is not an agent substrate. **Manuk's "only from-scratch engine that is also an agent substrate" position is true and defensible.**

---

## DOMAIN H — Security Architecture for v1

**v1 threat model (explicit):**
1. Web-content compromise (renderer bugs) — **largely eliminated** by memory-safe Rust DOM/layout/paint.
2. JS-engine compromise (the C++ mozjs asterisk) — **contained** per A5/B (SFI or JS-per-site process); honest asterisk: while mozjs is in-process, a mozjs RCE reaches the shared process.
3. **Prompt injection / agent hijacking as a first-class browser threat** — the novel one. Published taxonomies (Google DeepMind "AI Agent Traps": content injection, semantic manipulation, cognitive-state, tool-misuse, goal hijacking, multi-agent collusion; WASP, EIA, WebInject, Pop-up attacks). **Consensus (OpenAI, Anthropic, DeepMind 2025): prompt injection cannot be fully solved at the model layer.** Manuk's differentiator is engine-level defense-in-depth: content-taint tracking, capability scoping, egress constraint, Rule-of-Two enforced structurally.
4. Cross-tab/cross-site exfiltration by a hijacked agent — contained by egress controls + capability scoping (Topology 5/6).
5. Supply chain (cargo deps) — cargo-vet + cargo-audit at engine scale; pin + review.

**CVE-class elimination scorecard (marketing-defensible):** Of severe browser CVE classes (~70% memory-safety per Chromium's 912-bug analysis): **Manuk eliminates by construction** the memory-safety classes in DOM/layout/paint/networking/image-decode (the majority of the 70%); **contains** (not eliminates) the JS-engine memory-safety class while mozjs remains; **leaves** spec-logic bugs, side channels (Spectre), and prompt injection (mitigated). Honest claim: *"Manuk eliminates by construction the majority of the memory-safety vulnerability class that constitutes ~70% of severe browser CVEs, with the JS engine as the sole remaining memory-unsafe component pending its replacement."*

**Minimal-sufficient v1 hardening (given Topology 5/6):** seccomp-BPF + Landlock on Linux; W^X for any JIT (baseline JIT must be W^X); agent permission scoping = **task-scoped capability grant** (a token authorizing a specific action class on a specific origin for a bounded task, revoked on completion, never derivable from page content). Explicitly deferred to the security phase: full multi-OS sandbox parity, hardware memory tagging, formal verification of the capability layer — rationale: v1's memory-safe core already eliminates the dominant class.

---

## Integrated Roadmap Implications

The verdicts compose as a dependency chain over the existing 7-phase plan (0 daily-driver → 1 tabs/UX → 2 agent API → 3 harness → 4 consumer GUI → 5 perf → 6 security):

- **A → B, H:** Because A says an interpreter+baseline Rust engine is viable but not yet, mozjs stays in-process for v1, which sets B's topology (must contain mozjs: Topology 5/6) and H's threat model (the C++ asterisk is the residual). **The `JsEngine` seam must be built in Phase 0–1**, earlier than a naive plan — retrofitting later costs multiples.
- **C → F:** The no-pixels agent pipeline and intent-aware scheduler are both performance *and* moat features — **pull them forward from Phase 5 into Phase 2–3** (they are agent-substrate differentiators, not just perf).
- **D → all:** The differential oracle is already the prioritization engine; keep it as the spine through every phase.
- **F → 2:** WebMCP implementation + native-substrate spec leadership should land in Phase 2 (agent API), because the moat decay clock (WebMCP mass adoption ~mid-2027) makes early leadership valuable.
- **Phase-boundary move:** **Advance the security-relevant seams (JsEngine trait, capability/taint layer, focus-adaptive process policy) from Phase 6 into Phases 1–2.** Security-as-architecture cannot be bolted on at Phase 6 given prompt injection is a *product* threat. Keep raster/paint optimization (Vello/wgpu) in Phase 5.

---

## The Concede List (do not compete publicly)
- **Peak JavaScript throughput** (JetStream/Octane) — interpreter+baseline by design; V8 wins and real sites don't need it.
- **MotionMark-class raster throughput** — incumbent GPU compositors are years ahead.
- **WebGL/WebGPU creative apps** — out of honest scope.
- **DRM/protected media (Widevine)** — deliberately unsupported.
- **Exotic legacy compat** (deep MathML, ancient DOM quirks, bidi edge cases) — below the ROI line.
- **Absolute cold-start vs a warm Chrome process on a warm profile** — measure only fair, winnable start scenarios.

## The Compounding-Moat List (ranked, with build specs)
Ranked by moat value × tractability (see Domain F for full specs): (1) tree-diff receipts → (2) stable NodeIds → (3) engine-level injection resistance → (4) occlusion-truthful hit-test → (5) deterministic replay → (6) intent-driven prefetch → (7) parallel multi-tab scheduler → (8) token-optimal representations → (9) WebMCP + native-substrate spec leadership.

## The Three Crisp Answers

**(1) Why not Servo?** Servo is a *rendering engine*, not a full GUI browser plus agent substrate with one owned core, and it carries constraints Manuk's thesis cannot accept. As of 2026 Servo runs on **<$8k/month in donations** (~$7,659/month, May 2026), governed by Linux Foundation Europe, and — critically — **still embeds C++ SpiderMonkey** (it shipped SpiderMonkey 140.10.1 memory-safety CVE fixes in Servo 0.3.0), so it does not deliver the by-construction JS safety story and does not own its JS ingress. Its embedding API (WebView/WebViewDelegate) is maturing but is designed for generic embedding, not for a synchronous in-process agent observation/diff moat; building the agent surface *on* Servo would mean layering the substrate on an engine whose core semantics, JS engine, and roadmap Manuk does not control — reintroducing exactly the governance-and-control dependency the project exists to avoid. Manuk owns DOM/layout/paint/a11y/agent-crate as one core precisely so the agent substrate is native, not bolted onto someone else's compositor.

**(2) Your safety story has a C++ JS engine in it.** True, and quantified: while mozjs (SpiderMonkey) remains in-process, the JS engine is the sole remaining memory-unsafe component — but it is contained, not exposed. Manuk's memory-safe Rust DOM/layout/paint/networking/image-decode eliminate by construction the majority of the memory-safety class that is ~70% of severe browser CVEs (Chromium's own 912-bug figure); the residual is the JS tier, which we contain via SFI/Wasm-sandboxing or a JS-engine-per-site process (a topology incumbents cannot use because their whole renderer is unsafe), and which we retire entirely when a from-scratch Rust engine crosses ≥97% test262 and Speedometer-within-2×. The honest asterisk: a mozjs RCE today reaches the shared process; our containment reduces its blast radius and our roadmap removes it — versus incumbents, whose entire renderer stays unsafe indefinitely and whose V8 ships type-confusion zero-days exploited in the wild (CVE-2025-13223) that even the V8 Sandbox and MiraclePtr's 57% UAF mitigation do not fully close.

**(3) Compat tail kills engines, not perf.** Correct historically — Presto and EdgeHTML died on compat-cost economics, not benchmark scores — which is exactly why Manuk's differential fidelity oracle exists. Rather than chase 30 years of quirks via WPT coverage (which measures spec conformance, not site breakage), the oracle scores structural fidelity against Chromium (and, tri-differentially, Firefox to avoid overfitting to Chromium bugs) on a user-minute-weighted, category-stratified crawl of the Tranco head, then auto-prioritizes the highest-impact regressions into an agentic fix loop. The compat cost curve is power-law: the top-1000 sites cover most user-minutes, so a stopping rule of ≥0.75 structural fidelity on ≥95% of the top-1000 plus category floors defines "done for v1" and explicitly excludes below-the-line classes (deep MathML, bidi edge cases, ancient DOM quirks) with usage evidence. The oracle converts the death-spiral (unbounded tail, no prioritization signal) into a bounded, measured, continuously-allocated engineering problem — the one structural advantage a 24/7 agentic-coding pipeline on prosumer hardware is uniquely suited to exploit.

## Caveats
- **Agent-compute-month estimates for compiler/VM work are the least-known variable** — no published throughput multipliers for LLM agents on JS-engine implementation specifically; treat A3's figures as order-of-magnitude.
- **Some benchmark figures are third-party** (Magic Lasso; SuperchargeBrowser architecture-based estimates) rather than first-party lab measurements; the V8/Google first-party figures (JIT-less V8, Maglev, MiraclePtr, site-isolation 10–13%) and Chromium's 70%/912-bug figure are high-confidence primary sources.
- **The no-pixels pipeline and focus-adaptive isolation are, to the best of this research, novel** — no incumbent implements them — so there is no external existence proof of production soundness; Manuk must prototype and measure.
- **Manuk's memory-lean hypothesis is currently unmeasured** — the Domain C harness must be run before the memory claim is marketed.
- **WebMCP and the agentic landscape are moving monthly**; the moat-decay timing (mass adoption ~mid-2027) is an estimate to re-check quarterly.
- **rav1d is ~5% slower than dav1d and its bounty to close the gap was still open in 2025** — confirm parity before shipping AV1 as the default decode path.