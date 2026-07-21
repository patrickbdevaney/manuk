# DEEP RESEARCH PROMPT — Phase-0 ground truth, methodology, and the 7-phase vision audit

_Written tick ~324. This is the standing research charter: it defines what we must learn from the
outside world (industry SOTA, academia, the agentic-browser race) and from our own repo (git
history, wiki, tick loop, oracle, WPT battery) to answer three questions conclusively. Re-run any
section when its answer goes stale._

## The three questions

1. **WHERE ARE WE, REALLY?** What fraction of the internet's sites does Manuk render and drive
   with non-jarring Chromium fidelity — measured honestly, not by a proxy that flatters us?
2. **WHAT REMAINS, BOUNDED?** What is the *finite, enumerable* set of capabilities still required
   for daily-driver parity — discovered systematically (not tick-by-tick anecdote), sized in
   ticks, with the diminishing-returns tail explicitly priced and cut?
3. **IS THE VISION OPTIMAL?** Do phases 1–6 (agentic API surface → native harness → MCP →
   consumer prompt-to-action → performance → security) still form the best ordering and shape
   given what the industry is converging on in 2025–2026?

---

## PART A — External: industry SOTA, bleeding edge, academic

### A1. How the industry measures "how much of the web works"
- Chrome **use counters** (chromestatus.com/metrics): per-feature % of page loads. This is the
  ground-truth frequency distribution we should weight capability work by.
- **HTTP Archive / Web Almanac**: real-crawl feature usage (CSS properties by adoption, JS APIs,
  media formats, framework share — React/Next/Tailwind penetration).
- **web-platform-tests + Interop 2025/2026**: what the vendors themselves agreed is the compat
  frontier; wpt.fyi scoring methodology.
- **Tranco / CrUX top-site lists**: stratification and user-minute weighting for corpus design.
- Historical compat programs: Presto's site-patching, EdgeHTML's telemetry-driven compat,
  Mozilla webcompat.com + site interventions ledger. What killed Presto/EdgeHTML — and what the
  minimum viable compat surface actually was.

### A2. How new engines bootstrap and verify fidelity
- **Servo** (2024–26 revival): embedding story, what they run, how they track site breakage.
- **Ladybird**: pre-alpha methodology, their WPT posture and real-site testing.
- **Flow (Ekioh)**: the only other clean-room engine that targets real sites; what they claim.
- Visual/layout regression research: reftest theory, screenshot-diff pitfalls, structural
  layout-tree diffing, any academic metrics for "perceptual layout equivalence".

### A3. The agentic-browser race (Phase 1–4 alignment)
- **WebMCP** (W3C/WICG) status and timeline; Playwright-MCP; browser-use; OpenAI
  Operator/Atlas; Anthropic computer-use + Claude-in-Chrome; Perplexity Comet; Gemini-in-Chrome.
- Agent-benchmark SOTA: WebArena, WebVoyager, Mind2Web, OSWorld — what surface they exercise
  (DOM vs a11y-tree vs pixels) and where engines fail agents.
- The a11y-tree-as-API thesis: who else ships a first-class agent API *inside* the engine
  (nobody = our moat; somebody = our clock).

### A4. Component ecosystem watch (borrow-don't-build)
- Stylo, Taffy, Parley, Blitz, Servo crates, mozjs, wgpu/vello, rav1d, symphonia, cpal, rustls:
  releases and capability jumps since our pins. Anything that obsoletes an in-repo subsystem.

## PART B — Internal: the repo is the other half of the evidence

- **git log** (~324 ticks): capability categories over time, cadence, where ticks concentrated,
  where they thrashed. The last 60 ticks vs the roadmap's claimed priorities.
- **docs/wiki/** + **docs/loop/** (JOURNAL, RATCHET, PHASE0-PROGRESS, DAILY-DRIVER-EDGES,
  CONSTELLATION, FIDELITY-SCORING-REDESIGN, MEDIA, WEB-PATTERNS, AGENTIC-PHASES-PLAN): claimed
  state vs receipt-verified state; which boards are stale.
- **The differential oracle**: exactly how many sites, which categories, what it scores
  (coverage/placement/interactivity), its measurability holes (id-keyed probes vs React/Tailwind
  pages), and its false-signal history (coverage saturation, absolute-offset amplification).
- **The WPT battery**: which suites run, pass counts, what % of remaining failures are
  reftest-precision vs genuine capability gaps.
- **Reference sources** (chromium/, firefox/, webkit/, ladybird/ in reference/): use for
  capability enumeration (e.g. Chromium's use-counter enum lists every feature the web can
  touch) — not for line-by-line porting.
- The **tick loop mechanics**: verify wall history, gate count vs gates-in-the-wall, parked-work
  patterns — is the harness amplifying or damping progress?

## PART C — The synthesis deliverables

1. **Ground-truth scorecard**: our best honest estimate of daily-driver coverage today, with
   error bars and the measurement fixes needed to shrink them.
2. **The bounded remainder**: a finite checklist of remaining Phase-0 capabilities, each tagged
   {usage-weight from use counters} × {tick estimate} × {jarring-if-missing?}, with an explicit
   CUT LINE (the tail we refuse to chase) and named exceptions.
3. **Methodology upgrades**: corpus size/stratification, shape-scoring, root-cause clustering,
   interactivity probes, CDP-side geometry — whatever moves us from "sites sampled" to
   "internet covered" with statistical honesty.
4. **Vision audit**: phases 1–6 re-validated (or re-ordered) against A3; what each phase's exit
   looks like; what Phase-0 work is secretly Phase-1/2 work already done.
5. **Loop steers**: concrete lever-board updates so the grind agent's next N ticks execute the
   bounded remainder, not an unbounded tail.
