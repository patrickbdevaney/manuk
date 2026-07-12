# Manuk Development Methodology v2
**Purpose:** This document replaces judgment-driven ticking with a differential-oracle-centered
development loop. Read this in full before starting a tick. It is a standing instruction set, not
a one-time briefing — reload it at the start of every session and after every EPOCH audit.

**Primacy note:** Where sources disagree, this document resolves in favor of the strategic
architecture (differential oracle first, interface-first parallelization, G_ALLOC gate, Framework
Exception Miner, invalidation sets, a11y-tree-as-oracle). Specific unverified statistics from any
prior research pass (WPT counts, LOC breakdowns, bug-population estimates) are NOT to be
treated as facts about this codebase — where a number is needed, compute it live from this
project's own data using the procedures below. Do not cite a borrowed number in a commit
message or design doc as if it describes Manuk.

---

## 0. The two generalizations that govern every decision below

1. **A gate that does not measure what the user feels will report green while the user
   suffers.** Before adding any feature, ask: what gate would have gone red if this were
   already broken? If you can't name one, add it before or alongside the feature.
2. **The discovery rate has not flattened.** Every enumerated gap is a known unknown; the
   true item count is larger than any list, including this one. Treat every "done" as
   provisional until the oracle (Part 2) has looked at it.

---

## Part 1 — The Tick Loop v2

Unchanged in spirit, revised in mechanics:

```
take stock → consult priority ledger (data-driven, Part 4) → read Blink/Gecko for
algorithm only → implement behind frozen interface (Part 3) → run risk-based verify
wall (Part 5) → if touched a shared type, run full wall → journal (structured, Part 7)
→ commit with reference-source citation
```

**What changes from v1:**
- The priority ledger is no longer hand-ordered by judgment. It is generated from usage-frequency
  data intersected with oracle divergence data (Part 4). Judgment breaks ties only.
- The verify wall gains two new mandatory gates that did not exist before: **G_ALLOC**
  (Part 5.2) and **G_TEARDOWN** (Part 5.3). No commit ships without them, ever, starting now.
- EPOCH audits are no longer purely manual. Each EPOCH audit runs the full differential oracle
  crawl (Part 2) against the current corpus and regenerates the priority ledger before any
  cross-axis optimization work begins.

---

## Part 2 — The Differential Oracle (build this first, before any other item in this document)

This is the single highest-leverage system not currently in place. Build it before resuming
normal feature ticks. Budget: this is worth pausing the priority ledger for.

### 2.1 Architecture

```
[generator] → [render in Manuk] ⟍
                                   → [diff] → [cluster] → [rank by cause] → priority ledger
[generator] → [render in headless Chromium via CDP] ⟍
```

- **Generator, two feeds, run both:**
  1. *Real-web feed*: crawl a defensible sample (Tranco top-N, seeded from your existing
     22-site corpus's traversal classes, expanded incrementally — start at N=200, grow toward
     N=5,000+ as the pipeline proves stable). Fetch each URL once, save the raw HTML/DOM
     snapshot, and feed the *identical* snapshot to both engines. This is mandatory —
     re-fetching independently for each engine is what caused the stuck-at-5,122px false
     signal in your history (two different Chrome renders being compared). Pin one canonical
     Chromium build/flags for the oracle's lifetime; log its version in every diff report.
  2. *Synthetic/mutation feed*: mutate HTML/CSS/JS from a seed corpus (start from your 72
     synthetic boxes and the 22-site corpus). This is a lower priority than the real-web feed
     initially — the real web will surface your actual gaps faster per unit effort than a
     grammar fuzzer will, because your engine is young. Revisit grammar fuzzing once
     real-web divergence rate drops.
- **Render:** your engine's headless mode; Chromium headless over CDP. Use the identical
  viewport, device-pixel-ratio, and font config on both sides, or the diff is noise from day one.
- **Diff:** two signals, computed per page:
  - *DOM geometry*: reuse your existing box-tree dump (computed `display` per box) and
    intrinsic-size tracer. Diff against Chromium's equivalent (extract via CDP
    `DOM.getBoxModel` / accessibility tree / computed style queries — do not screen-scrape).
  - *Coarse pixel*: block-grid comparison, not full pixel diff — full pixel diff is a Bar 2
    concern and will drown you in noise before Bar 1 is reached.
  - **Guard against degraded documents**: before diffing, check that the Chromium render
    itself looks non-degraded (has a body with substantial content, isn't a no-script/error
    fallback). If Chromium's own render is degraded, discard the sample — don't diff against a
    broken oracle. This directly prevents the "MediaWiki no-script fallback graded as your bug"
    failure class.
- **Cluster, not list:** raw diffs are a firehose (published tools see 90%+ false-positive rates
  from naive diffing before clustering). Cluster by:
  1. First-divergence signature (the first element down the DOM where geometry disagrees) —
  2. Computed-`display`-mismatch class (does the box exist in one tree and not the other, or
     exist in both with different type) —
  3. CSS property/selector implicated (trace back from the divergent box to the matched rule)
  A cluster with 40 site-hits and one root cause outranks 40 individual diffs on the ledger.
  Rank clusters by how many *distinct sites* they explain — this becomes direct input to the
  priority ledger (Part 4), no manual judgment needed.
- **Reduce, in parallel:** once a cluster's minimal root cause isn't obvious,
  delta-debug the smallest failing HTML/CSS/JS case that still reproduces the divergence.
  This is a narrow, closed question — hand it to the parallel verification agents (Part 6), one failing
  case each, fanned out. Use a ddmin-style algorithm (bisect the input, keep the smaller
  case if the divergence persists, repeat). This is the correct and only good use of parallel
  *implementation-adjacent* work; it is not writing engine code.

### 2.2 What this gate replaces
- It supersedes "render one page and look" as primary discovery.
- It does **not** replace the 22-site corpus or the synthetic 72-box wall — those stay as fast,
  deterministic regression gates. The oracle is the *discovery* engine; the wall is the
  *regression* gate. Promote any oracle-found bug that recurs into a permanent wall test.

### 2.3 First-tick checklist for standing this up
1. Wire CDP client against local headless Chromium; confirm you can fetch box-model and
   computed-style data for a known page.
2. Point both your engine and the CDP client at one identical saved DOM snapshot; confirm
   the geometry diff format is sane on a page you already know is correct (should be ~empty
   diff) and one you know is broken (should surface a known bug, e.g. re-inject a page with a
   `<br>` to confirm the oracle would have caught your #1 historical failure).
3. Run against your existing 22-site corpus first — this validates the pipeline against a known
   quantity before spending crawl budget on new sites.
4. Only then expand the crawl frame.

---

## Part 3 — Interface-First Parallelization

### 3.1 What is frozen, and when
Freeze these types at the start of a batch of ticks, not mid-batch:
- `ComputedStyle` (field set + versioned serialization)
- `LayoutBox` tree API (post-Taffy integration boundary)
- `DisplayItem` list shape
- The JS binding table (WebIDL-generated boundary)

A frozen interface is a contract: agents/ticks working behind it may not assume anything about
internals on the other side of the seam beyond the published contract. If a tick discovers the
contract is wrong (missing a needed field, wrong ownership model), it **stops and escalates** —
it does not patch around the gap locally. Patching around a bad contract is exactly what
produced the dangling `*mut Dom` in the JS reflector (loose ownership/lifetime boundaries
mutated iteratively without a frozen contract). The fix going forward: FFI/reflector boundary
ownership is part of the frozen contract, explicitly documented with lifetime rules, not
implicit.

### 3.2 What runs in parallel vs. what serializes
**Parallelizable (behind a frozen contract, in the current codebase's real coupling — verify
this against your own git history, see 3.3, don't assume):**
- Leaf feature implementation within one crate that doesn't touch a frozen type's *shape*
  (e.g., flex basis-size calculation logic, a new CSS property's value parsing, text shaping
  edge cases, an isolated IDL method body).
- Delta-debugging reduction (Part 2.1) — always parallel, always safe, it's read-only against
  the engine.
- Speculative/redundant attempts at a single narrow, well-specified item, when the item has a
  **decisive external oracle** (the differential oracle, not an LLM judge — see 3.4).

**Must serialize:**
- Any change to a frozen type's shape/fields.
- Core event loop / scheduling logic.
- Anything the change-coupling matrix (3.3) shows as high-coupling with other in-flight work.

### 3.3 Compute the change-coupling matrix — don't guess the partition
Do this once, then refresh it every EPOCH audit:
```
git log --name-only --pretty=format:"---%H---" | <co-change parser>
```
Build a file×file co-occurrence matrix from your own commit history (start with your own repo,
not Blink/Gecko — you want *this codebase's* actual coupling, since your architecture differs
from theirs). `C[i][j]` = frequency file `i` changes alongside file `j`. Partition into
low-coupling clusters; only dispatch parallel work within a cluster. Re-run this matrix
periodically — coupling shifts as the architecture matures, and an assumption frozen at project
start will silently go stale.

### 3.4 Merge criterion
A branch merges **without human/orchestrator diff review** if and only if:
1. It respects the frozen interface contract (compiles against it unmodified), and
2. It passes the full risk-appropriate verify wall (Part 5), and
3. It produces zero new red clusters against the differential oracle for its target pages.

Do **not** use an LLM-as-judge panel to pick between speculative candidates. Use the
differential oracle as the sole judge — it's decisive and mechanical where a judge panel is
noisy and inconsistent. Reserve human/orchestrator review for interface changes themselves
(3.1) and for anything the oracle can't yet evaluate (e.g., a11y semantics before the a11y
oracle in Part 9 is built).

---

## Part 4 — Priority Ledger: derive it from data, not judgment

Replace hand-ordering with two intersected signals, refreshed at each EPOCH audit:

1. **Real-world usage frequency** per CSS property / DOM API. Pull from public usage-counter
   sources (Chrome Platform Status use-counters, HTTP Archive/Web Almanac property-frequency
   tables, MDN). Re-fetch current data rather than trusting cached figures from any prior
   research pass — these move year over year and your source docs disagree on specifics
   already, which is itself evidence they shouldn't be hard-coded here.
2. **Oracle divergence weight**: from Part 2's clustering, how many distinct sites in your crawl
   frame does each root cause explain.

**Ledger score = usage_frequency × divergence_weight.** This is exactly the mechanism that
would have put `:checked` at the top of the queue automatically instead of leaving it a stub —
high usage frequency (a load-bearing CSS-only interactivity primitive) times high divergence
weight (every site using the checkbox hack diverges) is not a number judgment would reliably
surface on inspection, but a crawl + clustering pass will.

Judgment breaks ties and handles items the oracle can't yet see (e.g., a capability with zero
current usage-counter signal because no page can use it yet since you don't support it —
watch for this blind spot explicitly).

---

## Part 5 — Verify Wall v2

Existing gates (build, 72/72 synthetic parity, G1–G6, F1–F4, crate tests) stay. Two mandatory
additions, plus a scheduler.

### 5.1 Why the additions are mandatory
Your F1–F4 performance floors and G1–G3 correctness gates were all green through the
`ComputedStyle`/rect-map clone-per-wheel-event regression, because none of them measured
per-input-event work. A load-time bench measures throughput on an idle queue; it does not
measure marginal cost of an event handler doing something pathological. This is not a gap you
patch once — it's a category of gate you were missing entirely.

### 5.2 G_ALLOC — Allocation-Rate Gate
- Instrument a tracking global allocator (or OS-level memory profiling) around the dispatch of
  a single synthetic input event (wheel, mousemove, keystroke) on a large-DOM test page.
- Assert: allocation count between event-arrival and frame-completion is **near-zero when no
  listener is registered for that event type**, and does not scale with DOM size when a
  listener *is* registered (assert sub-linear, ideally O(changed-subtree), not O(n)).
- This gate must run on the shipping Stylo+SpiderMonkey configuration, same as every other
  gate — a debug-config-only version of this gate would have missed the exact regression it
  exists to catch.
- Fails loud: a red G_ALLOC blocks the commit exactly like a failed crate test.

### 5.3 G_TEARDOWN — Lifecycle/Teardown Gate
- Forbid `libc::_exit()` or any process-exit path that bypasses Rust `Drop` in test and release
  builds; a leak sanitizer (ASan/LSan) run in the verify wall must flag any bypassed teardown
  as a hard failure, not a warning.
- Rationale: a workaround that hides a crash is a data-loss bug wearing a disguise. Test this
  by asserting that a deliberately-triggered panic/exit path in a test still flushes the profile
  store — if it doesn't, the gate is red.

### 5.4 Risk-Based Gate Scheduler
- Compute which gates a diff can plausibly affect using the change-coupling matrix (3.3): a
  diff touching only `paint`/`compositor` cannot change G2 JS-conformance outcomes.
- Run cheap/fast gates synchronously on every commit (build, synthetic 72-box, crate tests,
  G_ALLOC, G_TEARDOWN). Defer the heaviest gates (G1 real-site fidelity, G2 JS conformance,
  full differential-oracle crawl) to an async secondary pipeline **unless** the diff touches a
  frozen shared interface or an FFI boundary, in which case run the full wall synchronously,
  no exceptions.
- Batch rule, formalized: a change may batch into one verify cycle with other pending changes
  **iff** its file set has co-change weight 0 (per 3.3's matrix) with every other batched
  change's file set, **and** it touches no frozen shared type. Otherwise isolate it.

---

## Part 6 — Parallel Verification Agents: what they are for

Unchanged in kind, expanded in use. Parallel verification agents answer **narrow, closed
questions** and now have three concrete queues to serve, in priority order:

1. **Delta-debugging reduction** of oracle-found failing cases (Part 2.1) — the best-fit use, since
   minimisation is inherently narrow and parallelises cleanly.
2. **WPT triage** (Part 8) — classifying failures into the existing cluster taxonomy.
3. **Speculative narrow-item verification** (3.4) — checking a candidate implementation against the
   oracle, not judging between candidates by opinion.

Route long-context work (spec/WPT parsing) and small, repetitive, high-volume work (single
delta-debug steps) to whichever agents suit each. **Never route implementation or diagnosis to
them** — they answer questions; they do not write engine code. Anything that exceeds a single
agent's budget escalates to the orchestrator rather than being truncated to fit: a check shortened
to make it affordable is a check that no longer asks the question it was written to ask.

## Part 7 — Journal structure (so ledger + oracle data compound instead of evaporating)

Every tick's journal entry must record, in addition to what changed:
- Which oracle cluster (if any) this tick targeted, and its site-count weight at tick start.
- Whether the fix was verified by the oracle re-running clean on the same cluster (not just by
  the synthetic wall).
- The reference file/function consulted in Blink/Gecko for algorithm extraction (unchanged
  requirement).
- A one-line note if this tick's discovery method was **not** the oracle (i.e., still found by
  manual inspection) — this is the signal that tells you the oracle's crawl frame or clustering
  needs to expand. If manual-discovery ticks stop dropping to near-zero as the oracle matures,
  the oracle's coverage is insufficient, not the codebase's quality.

This journal is also the input to the residual-estimate procedure below — do not skip entries.

---

## Part 8 — WPT: steering signal, not scoreboard

- Wire a WPT runner against your headless mode once WebDriver BiDi (Part 9) exists, since
  that's dual-use infrastructure.
- Subset by, in priority order: (1) the usage-frequency ledger (Part 4), (2) oracle failure
  clusters (Part 2), (3) spec area. Do not attempt the full ~2M-subtest suite in the inner loop —
  run it nightly/async, publish a dashboard, and don't let it gate a tick.
- Exclude legacy encoding/CJK-heavy directories from your steering percentage if they turn
  out to dominate subtest count disproportionately relative to their real-world relevance —
  verify this claim against the current WPT tree yourself rather than trusting a cached
  percentage from prior research; don't hardcode an unverified "60%" figure into tooling.
- Do not gate Bar 1 on any WPT percentage. Use it as one more divergence signal feeding the
  same priority ledger as the oracle.

---

## Part 9 — SPA Frontier: the Framework Exception Miner

This is the correct, general form of the `a.protocol`/`document.scrollingElement` discovery.

### 9.1 Build it as a system, not a one-off
1. Pull barebones starter templates: Create-React-App, Next.js (App Router + a Pages Router
   sample), Vite+React, Vue/SvelteKit, Angular — in that order, since React/Next unblock the
   most real-world traffic.
2. Drive each headlessly in Manuk. The engine must print thrown exceptions instead of
   discarding them (if this isn't already universal, make it universal — every swallowed
   exception is a hidden Framework-Exception-Miner signal you're throwing away).
3. Parse each exception's call stack; auto-generate an implementation ticket for the missing
   IDL member/API named in the error.
4. Rank the resulting backlog by **how many of the N starter templates each missing item
   unblocks** — this is your SPA priority ledger, separate from but structurally identical to
   Part 4's document-web ledger.

### 9.2 Minimum viable substrate, build in this order (converges fastest across all four source
documents independently)
1. **Event system + on-read geometry**: `addEventListener`, event objects, `el.onclick`
   property and attribute handlers; `getBoundingClientRect`/`offsetWidth`/`clientHeight`/
   `scrollTop` recomputed on read, never a stale snapshot. This is hydration's hard gate —
   nothing else matters until this is correct, because mismatched first-render geometry aborts
   hydration and cascades into full client-side re-render (visible as flashing/breakage).
2. **Scheduling**: `requestAnimationFrame`, correct microtask ordering (`queueMicrotask`,
   Promise resolution timing relative to rAF), `MessageChannel`/`setTimeout` (React's
   scheduler depends on these).
3. **Observers**: `MutationObserver`, `IntersectionObserver`, `ResizeObserver`.
4. **Loading**: `fetch` completeness, `AbortController`, dynamic `import()`, import maps,
   code-split chunk loading, `history` routing.
5. **CSSOM + misc**: `insertRule`, constructable stylesheets/`adoptedStyleSheets`,
   `structuredClone`.

Gate SPA success on **first-render geometry matching the differential oracle** (Part 2), not on
a hand-checked feature list — this reuses infrastructure you're already building instead of
inventing a parallel SPA-specific pass/fail criterion.

### 9.3 The admissions-test class of bug
The web feature-detects your engine (`'localStorage' in window` etc.). The Framework
Exception Miner will surface these automatically as sites silently degrading to no-script
fallbacks. When it does, implement honest stubs for admission-test properties
(`localStorage`, `sessionStorage`, `indexedDB` presence) immediately, even before the full
underlying implementation is ready — a present-but-inert API beats a missing one for this
narrow purpose, and closes the gap between "looks catastrophically broken" and "actually just
incomplete."

---

## Part 10 — Build/Verify Cycle Compression

Adopt, in this order (highest-confidence, lowest-risk first):
1. **mold or lld linker** — largest single measured win in comparable Rust workspaces; do
   this first.
2. **cargo-nextest** for parallel test execution.
3. **cargo-hakari / workspace-hack crate** to reduce redundant linking across your 16 crates.
4. **Split debuginfo** on Linux/macOS.
5. **Dependency graph surgery**: wall off mozjs/Stylo behind stable crate boundaries so a
   shared-type change in your own code doesn't force their recompilation. This is your longest
   pole — prioritize isolating it early even though it's more work than the linker swap.
6. Nightly `-Z share-generics` — smaller win, adopt opportunistically, don't block on it.
7. **Do not adopt Cranelift for debug builds** — it does not support inline assembly and is
   documented to break on low-level/FFI-heavy code; your mozjs/Stylo FFI boundary makes this
   a poor fit. Skip it.
Combine with the Risk-Based Gate Scheduler (5.4) — the build-time wins compress every tick;
the scheduler compresses which gates each tick even needs to pay for.

---

## Part 11 — Algorithmic Efficiency ("snappy") Roadmap

In dependency order (each one is meaningfully blocked without the last):
1. **Invalidation sets** for style recalculation (Blink's pattern, algorithm-extracted, not
   copied): DOM mutation → compute the specific descendant invalidation set → restyle only
   that scope, not a global cascade. Use dynamic restyle flags (set during matching) for
   `:hover`/`:nth-child`-class pseudo-classes that don't fit the static invalidation-set model.
2. **Layout fragment reuse**: treat layout results as immutable fragments; reuse a cached
   fragment for any subtree whose dirty bit wasn't set by a preceding style/DOM mutation.
   Target: layout becomes O(changed nodes), not O(total nodes).
3. **Damage rects + display-list diffing** for the CPU rasterizer (tiny-skia): track only the
   spatial regions invalidated by a paint change; union and raster only the damaged region.
   Do this before any GPU compositing work — it's the larger near-term win for a CPU-raster
   engine and de-risks the wgpu path by deferring it.
4. **Event coalescing**: scroll/wheel events dispatched faster than display refresh must
   coalesce to one aggregate delta per rAF tick — this is directly downstream of the G_ALLOC
   gate (5.2) and should be verified by it.
5. **Tab hibernation**: serialize/evict background-tab DOM+layout state, keep a lightweight
   restore token and low-res cached bitmap. This is your owned memory differentiator —
   schedule it after the above four, since it's a product win, not a Bar-1 correctness
   blocker, but it is architecture-shaped (touches ownership of the DOM/layout arenas), so
   don't leave it so late that arena design decisions elsewhere foreclose it.

---

## Part 12 — Agent-Native Architecture

- Implement **WebDriver BiDi** as the native automation surface. Do not implement CDP
  mimicry — it's both unnecessary (BiDi is the converging standard; Firefox has already
  dropped experimental CDP support in favor of it) and inconsistent with the honest-fingerprint
  constraint.
- Make the **accessibility tree the single source of truth** for three roles simultaneously:
  (a) the agent's semantic automation surface, (b) the shipping a11y product feature, (c) the
  G5 interaction-parity oracle (script an action sequence, execute via the a11y tree, compare
  outcome to Chromium).
- Concretely: agent actions resolve to a11y-tree node → spatial hit-test via the primary layout
  engine → if the target is occluded/`display:none`/off-screen, the hit-test fails and the
  action is rejected, exactly as it would be for a human. This is what makes agent-driven
  testing a valid differential oracle for interaction parity rather than a privileged bypass.
- Cost, stated plainly: the a11y tree must be computed-on-read and kept synchronously
  consistent with layout. This is real engineering work — schedule it as its own item, not a
  side effect of Part 11's invalidation work, though the two share infrastructure.
- Payoff: agent regression tests and engine correctness tests become the same suite. Don't
  build a second, parallel "agent test" pipeline — route it through Part 2's oracle
  infrastructure.

---

## Part 13 — Residual-Estimate Tracking (compute live, do not import numbers)

Do not adopt any specific bug-population figure from prior research as a planning input — none
of the source documents' numbers are computed from this project's actual data, and at least
one (a Zippin MLE table built on a synthetic example vector with a broken source citation) is
demonstrably not real project data. Instead:

1. From the journal (Part 7), extract the per-tick new-defect-discovery count once you have at
   least 10–12 ticks of real data since this methodology took effect.
2. Because discovery is **serial** (each tick's fix changes the codebase; this is not
   independent parallel sampling of a frozen artifact), do not use a naive two-sample
   Lincoln-Petersen estimator — it will underestimate the residual under serial capture.
3. Use a **removal-model estimator** (Zippin/Moran-style): fit the declining discovery-rate
   curve across ticks; the x-intercept (where marginal discovery rate → 0) estimates total
   population; residual = estimate − cumulative found. This is a real, standard method — apply
   it to your real vector, not a borrowed one.
4. Report the result as a **lower bound**, explicitly scoped to "residual in the current
   capability surface" — exclude the SPA frontier from this estimate entirely and run Part
   13's procedure separately once the Framework Exception Miner (Part 9) has generated its
   own discovery stream. Treating the SPA frontier as part of the same closed population as
   the document-web surface will understate the true residual, since it's a substantially
   unopened population.
5. Re-run this estimate at every EPOCH audit. Expect the estimate to grow, not shrink, as the
   oracle's crawl frame expands (Part 2) — a growing estimate from better instrumentation is
   not the process failing, it's Generalization #2 confirming itself. Don't let a rising number
   trigger false alarm; let it trigger ledger re-prioritization instead.

---

## Part 14 — Bar 1 Milestone Framing

Do not commit to a specific month-count schedule sourced from another project's published
trajectory without first grounding it in your own oracle's early output (Part 2's first few crawls
will tell you your actual current divergence rate against real sites — that's a far better anchor
than any external project's timeline, since architectures differ). Once the oracle has run
against your existing 22-site corpus and an initial ~200–500-site crawl, use the resulting
cluster count and severity distribution, together with the removal-model estimate (Part 13), to
set an internal Bar 1 target — and revisit it at every EPOCH audit rather than treating an
initial estimate as fixed.

What is safe to state now, independent of any borrowed number: Bar 1 is reached when (a) the
oracle's real-web crawl shows first-divergence pushed past the fold on the large majority of
your traversal-class corpus, (b) the SPA MVP substrate (Part 9.2) hydrates and remains
interactive on the top starter templates with no thrown exceptions, and (c) G_ALLOC/G_TEARDOWN
have been green for a sustained run with no regressions. Track these three conditions directly;
don't substitute a WPT percentage or an external timeline for them.

---

## Immediate Action Items (first sessions under this methodology)

1. Stand up the differential oracle (Part 2) against the existing 22-site corpus. Do this before
   resuming the normal priority-ledger tick loop.
2. Add G_ALLOC and G_TEARDOWN to the verify wall (Part 5.2–5.3). These are cheap to build and
   close a proven gap — do not defer them behind the oracle work.
3. Compute the change-coupling matrix from this repo's own git history (Part 3.3).
4. Regenerate the priority ledger (Part 4) once the oracle has produced its first clustered
   output — expect this to reorder your current backlog.
5. Start the Framework Exception Miner (Part 9) against Create-React-App and Next.js as the
   first two templates, in parallel with (not after) the document-web oracle work — it's the
   largest unknown and the sooner it starts generating its own discovery stream, the sooner
   Part 13's SPA-specific estimate becomes meaningful.
6. Begin journaling in the Part 7 structure starting now, so the removal-model estimate has
   real data to fit at the next EPOCH audit.