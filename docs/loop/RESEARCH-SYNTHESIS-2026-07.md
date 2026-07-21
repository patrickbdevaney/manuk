# DEEP-RESEARCH SYNTHESIS — 2026-07-21 (tick ~328)

_Answers to the three charter questions in DEEP-RESEARCH-PROMPT.md, from three parallel research
passes: external SOTA, internal audit, and the site-class gap matrix. Companion docs:
PHASE0-BOUNDED-REMAINDER.md (the finish line), FIDELITY-SCORING-REDESIGN.md (the instrument)._

---

## Q1 — WHERE ARE WE, REALLY? (the ground-truth scorecard)

**Breadth is real; the three load-bearing claims are unmeasured.**

| claim | state of evidence |
|---|---|
| "Renders most site classes usably" | PLAUSIBLE — matrix shows the large majority of every class BUILT+gated except video-playback and rich-editing; but the fidelity instrument that would prove it (parent-relative shape scoring) is a document, not a program. Latest raw sweep: 42% of sites cleanly scored, placement 0-17% in the (misleading) absolute frame. |
| "Memory advantage vs Chromium" | UNMEASURED — 100-tab RSS benchmark defined, never run. |
| "JS conformance" | UNMEASURED — test262 never run despite embedding SpiderMonkey (Ladybird publishes 97.8%). |
| Bar 0 (no crash/hang) | STRONG — gated per tick, real containment. |
| Capability ratchet | WORKS but is diluted: last ~40 ticks were one-API-per-tick shims weighted equal to IndexedDB; ready_pct reads 103% and gates nothing. |
| The wall | WAS BROKEN (observer-owned): cron ramdisk flush deleted incremental state mid-compile every 3 min → 93s→694s. Root cause fixed 2026-07-21 (flush now refuses under a live compiler unless near-OOM). |
| Gate coverage | verify watches ~19 of 176 page gates; the rest are ratchet teeth nothing bites on (precedent: g_capability red ~100 ticks unseen). Mitigation: off-tick full-gate sweep (scripts/gate-sweep.sh). |

**Known instrument corrections:** coverage saturates (gates nothing); absolute placement charges one
root cause N times (microsoft: every element off by the same 23px scores 0%); `[id]`-keyed probing
can't measure 39% of the corpus (React/Tailwind). Fix the instrument before pricing the tail.

## Q2 — WHAT REMAINS, BOUNDED?

See **PHASE0-BOUNDED-REMAINDER.md**: 3 subsystems (media playback join+codecs, contenteditable/IME
editing, software WebGL) + ~20 bounded S/M items + 3 verification runs + a 16-item named cut line.
**Realistic bound: 100-150 ticks (~one week of loop time).** The "endless CSS tail" fear is
disconfirmed twice over: internally (the placement metric amplifies single causes into apparent
tails) and externally (use-counter distributions are extremely head-heavy — a small feature head
covers the overwhelming share of page loads; the tail is formally removable).

## Q3 — IS THE 7-PHASE VISION OPTIMAL? (the audit)

**Verdict: the phases are right; the SEQUENCING should overlap; the clock is real.**

1. **Phase 1's moat is confirmed but clocked.** As of mid-2026, NO shipping browser exposes an
   engine-native agent interaction tree for arbitrary pages — everything real is a retrofit
   (Playwright MCP = a11y-tree-over-CDP; Chrome DevTools MCP = CDP; browser-use = DOM
   serialization; OpenAI/Anthropic computer-use = pixels). BUT: WebMCP (navigator.modelContext)
   is in Chrome 149 origin trial, projected on-by-default late 2026; OpenAI retreated from
   browsers (Atlas sunset Aug 2026); the market has settled the winning surface shape.
   **Window: ~12-18 months.**
2. **Therefore: overlap Phase 1 with the Phase-0 tail.** The Phase-0 remainder (media join,
   editing, WebGL) does not block agent-API work — the a11y tree, NodeIds, actuation, and BiDi
   substrate are already strong ("Phase 1 is exposing, not building"). Start Phase-1 surface work
   as soon as Tier-1 jarring items are moving, not after the last cosmetic tick.
3. **Adopt the settled API shape.** Phase-1's surface should be a drop-in SUPERSET of the
   Playwright-MCP snapshot shape (role/name/state + stable refs, compact serialization,
   ~200-400 tokens/snapshot) so every existing agent works day one — then differentiate with
   what retrofits can't do: (a) no-pixels mode (SEAM-NOPIXELS), (b) node identity across
   re-render/hydration (SEAM-NODEID), (c) **provenance-labeled tree nodes** — a11y-tree prompt
   injection is a demonstrated attack class (arXiv 2507.14799) and SEAM-TAINT is the structural
   answer no CDP retrofit can offer.
4. **Build the tree on AccessKit** (mature, cross-platform, May-2026 releases): one tree serves
   OS assistive tech + the agent API + the parked widget-roles work — three roadmap items, one
   subsystem.
5. **Ship navigator.modelContext (WebMCP) early** — Manuk becomes the first engine where the
   site-declared tool surface and the engine-native tree are one system. Phases 2-4 evaluate on
   Online-Mind2Web-style live-site tasks (WebVoyager is saturated/exploitable).
6. **Phases 5-6 order stands.** Memory-safety story is already a Phase-6 asset (pure-Rust image
   decoders eliminate the libwebp CVSS-10 class by construction; rav1d-safe now extends this to
   AV1). Process-per-tab remains the named Phase-1/6 architecture item.

## Methodology upgrades (adopted)

1. **Cumulative-coverage oracle** — the "what % of page loads does our feature set fully cover?"
   question is answerable with one BigQuery query over `httparchive.blink_features.usage`
   (public). This converts the roadmap from vibes to a ranked list. (Queued as an observer task;
   needs only a free BigQuery sandbox.)
2. **Clone the Servo Baseline Readiness methodology** (% of ~439 Baseline Widely-Available
   features at ≥95% quality, via the WPT Feature Manifest) as the public-facing readiness
   number — the only published third-party "how done is an indie engine" metric. Replaces the
   broken ready_pct.
3. **Instrument before tail** — FIDELITY-SCORING-REDESIGN.md is the exit gate; selector-path
   keying + shape scoring + root-cause clustering + jarring invariants; CDP box geometry (no
   screenshots) makes Tranco-1000 sweeps practical.
4. **Per-site intervention channel** (webcompat-addon pattern) — Presto died patching sites from
   the vendor; Mozilla survives with an hours-latency intervention ledger (UA overrides,
   injected CSS/JS, about:compat). Cheap architecture insurance; queue as a small Phase-0/1 item.
5. **Marquee strategy** — one threshold number + one marquee app (YouTube plays) moves
   perception; breadth claims don't (the Ladybird lesson: 90% WPT + Google Sheets).
6. **WPT posture** — keep WPT as a regression ratchet, never a coverage claim (maintainers' own
   framing: "an engineering tool, not a balanced metric"). The encoding suite's 360k passes are
   85% of our TOTAL and must never headline.

## Loop/process defects found and fixed this pass

- **Ramdisk flush killed compiles** (93s→694s wall): fixed at source in scripts/ramdisk.sh —
  flush refuses under a live compiler unless MemAvailable <4G.
- **STATUS.md was ~200 ticks stale** (TICK 128, tick-36 crawl numbers) — the mandated first-read
  booted every session on a false map; generator being repaired.
- **Gate coverage hole** — scripts/gate-sweep.sh (off-tick, memory-capped) runs the FULL gate set
  and reports reds that the per-tick wall can't see.
- **ready_pct 103%** — retired as a gate; replaced by the exit certificate + Baseline-readiness.
- Meta-observation the boards must carry: dashboards go stale from our own landed ticks;
  narrative docs (JOURNAL, wiki) stay honest. Re-probe before building; trust gates on disk over
  TSV rows.
