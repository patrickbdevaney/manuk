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

### 4.1 Selection rule: broadest first, always. Depth is never a reason to go first.

The ledger is ordered by **asymmetric breadth**, not by usage frequency alone and never by
interest, difficulty, or how nearly-done something is:

1. **Saltatory breadth first.** An item that unlocks *a whole class of design pattern* across the
   widest set of sites outranks everything else, always. `:checked` (every CSS-only menu, accordion,
   dropdown and sidebar on the web), `display:none` in a flex container (every hidden menu, modal
   and template), form-control rendering (every form) — these are the shape. One change, a whole
   pattern-class of the internet starts working. **Take every one of these before anything narrow.**
2. **Then high-frequency sites** — the patterns concentrated in what people actually load most.
3. **Then concentrated/specialised use** — narrow, deep, or complex items, however tempting.

The oracle's ranking *is* this rule, mechanised: clusters are ranked by **distinct sites (and
distinct traversal classes) explained**, so the broadest cause is at the top by construction. That
ordering is not advice — it is the queue.

**Never invert this.** A deep item is not more urgent for being deep, and a narrow item is not more
urgent for being nearly finished. The failure mode this rule exists to prevent is spending a week on
one site's last 8px while an entire design pattern is broken across ten thousand sites.

**Ledger score = divergence_breadth (sites × classes) → usage_frequency → everything else.** This is exactly the mechanism that
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

## Part 15 — Font/Text Stack: Adopt the Library, Don't Extract the Algorithm

This is a narrow, explicit exception to "read Blink/Gecko for the algorithm only, never the
code." Text metrics/hinting/shaping is a case where the correct move is to **link the actual
sanctioned third-party libraries Chromium itself uses**, exactly as you already do with Stylo
and SpiderMonkey — not re-derive their internals from source reading.

- **Metrics, outline loading, hinting**: adopt **Skrifa** (Rust, part of Google's Fontations
  family). This is the literal library Chromium ships as of Chrome 133–145, replacing FreeType.
  Google ran extensive pixel-comparison testing between FreeType and Skrifa before shipping it
  and maintains ~700 unit tests spanning low-level parsing through hinting virtual machines.
  Adopting it directly makes your glyph metrics/hinting **byte-compatible with Chrome's current
  pipeline**, not merely algorithm-similar.
- **Shaping**: adopt **HarfBuzz** (C, via Rust bindings) for glyph substitution/positioning
  (GSUB/GPOS), complex script shaping, ligatures. Both Chromium and Firefox use HarfBuzz for
  this layer — it is a shared, independent library, not engine-specific code, and is a
  legitimate sanctioned FFI dependency under your existing constraints.
- **Rasterization** stays yours (tiny-skia) — Skrifa deliberately doesn't rasterize; it only
  replaces the metrics/outline/hinting layer that Skia consumes.
- **Known gap to track, not block on**: Skrifa currently exposes no stem-darkening
  (small-size emboldening) controls the way FreeType did. File this as a known, narrow
  divergence — it will show up as a small-text rendering difference in the oracle, not a
  structural one.
- **Priority**: do this before deep-diving any further font/text layout work. It collapses
  what would otherwise be an open-ended "days each regardless of methodology" subsystem (per
  your own EPOCH analysis) into a bounded integration task, and it's the single highest-value
  unlock for the Bar 2 pixel-precision tier specifically, since font metrics are usually the
  dominant source of persistent sub-pixel drift.

---

## Part 16 — Codebase-Aware Context: LSP-First, Not Chunk-Retrieval

The rust-analyzer LSP plugin is already installed. This section governs *how* it and
complementary tooling are used, not whether to install anything further.

### 16.1 LSP is the default navigation tool, not a fallback
- Before reading a file to understand its structure, prefer LSP operations (go-to-definition,
  find-references, call hierarchy, incoming/outgoing calls) over grep or full-file reads. This
  is exact-graph navigation from the compiler's own semantic model — it cannot bisect a
  function across an arbitrary boundary the way embedding-chunk retrieval can, which is exactly
  the property you asked to preserve.
- Before any non-trivial refactor (renaming a shared type, changing a frozen interface per Part
  3), run find-references first and treat the returned call sites as the actual blast radius —
  not an estimate of it.
- Rely on automatic post-edit diagnostics (type errors, missing imports) as an immediate,
  free correctness signal on every edit — this is faster and more complete than waiting for the
  next full verify-wall run to catch a type-level mistake.
- **Known operational risk**: rust-analyzer can be memory-heavy and slow to respond on a large,
  mozjs/Stylo-heavy workspace. If it becomes sluggish or crashes mid-session, disable it for
  that session and fall back to targeted grep + `cargo check` rather than stalling — don't treat
  a degraded LSP as a hard blocker.

### 16.2 A cheap, generated orientation layer for session start
LSP answers precise questions but assumes you already know roughly where to look. At the start
of a fresh context window (new session, or post-compaction), generate — don't hand-maintain — a
lightweight repository overview:
- Parse the workspace with tree-sitter (or reuse `cargo doc`/`rustdoc` JSON output) to produce a
  per-crate symbol list: public types, trait boundaries, and the frozen-interface types from
  Part 3.1 called out explicitly.
- This is deliberately *not* a PageRank-ranked map or an embedding index — it's a flat,
  regenerated structural summary, cheap to produce, always current (regenerate at every EPOCH
  audit or whenever crate boundaries shift), and it exists purely for orientation. Once oriented,
  switch to LSP for anything precise.
- Do not let this orientation artifact go stale and get trusted as ground truth — regenerating
  it is cheap; treating a six-week-old hand-written architecture doc as current is the actual
  failure mode this replaces.

---

## Part 17 — Long-Horizon Memory & Subagent Research Architecture

Research-heavy work (the differential oracle's crawl, WPT triage, delta-debug reduction,
Framework Exception Miner runs) must not accumulate in the main orchestrator's context. Model
degradation with context length happens even when the relevant information is still present in
context — so a context stuffed with raw crawl logs measurably degrades judgment quality on the
*next* tick's decisions, independent of whether it technically still fits the window.

### 17.1 The rule
- Any task that is fundamentally "go look at a lot of things and come back with a verdict"
  (oracle crawl analysis, WPT failure triage, delta-debug reduction, framework-exception
  parsing) runs as an **isolated subagent call**, not inline in the main tick thread.
- The subagent returns a **condensed, structured result** — cluster name, site-count/weight,
  root-cause hypothesis, suggested file/function, confidence — never raw diffs, raw crawl
  output, or raw exception logs. Target roughly 1,000–2,000 tokens per returned summary,
  regardless of how much raw data the subagent processed to produce it.
- The main orchestrator thread stays long-lived across many ticks and holds: the current
  priority ledger, the frozen-interface contract state, and the structured journal (Part 7) —
  not research exhaust.

### 17.2 When to compact
- Compact at natural tick/subgoal boundaries (a tick's journal entry is written and the tick is
  closed), not on a fixed turn-count or token-count schedule. Compacting mid-subgoal risks
  erasing information the model still needs for that subgoal; compacting only when a window is
  nearly full risks the context already being degraded by staleness before compaction fires.
  Tick boundaries are the correct trigger because they're exactly the points where "this unit of
  work is done, nothing further from it is needed verbatim."
- The memory-filesystem (journal, priority ledger, oracle cluster registry) is the durable
  record across compactions — the in-context conversation history is disposable once its
  contents are captured there.

---

## Part 18 — Multi-Objective Regression Guard

Unlike a single-scalar optimization target, Manuk must improve on many axes simultaneously
(correctness breadth, latency, memory, stability, feature completeness) without a gain on one
axis silently costing another. The mechanism for this is not novel — it's disciplined use of
what you're already building:

- **Treat the differential oracle's passing cluster set as a monotonically-growing golden
  master.** Every cluster the oracle has ever confirmed clean is a regression test from that
  point forward. A commit that fixes a new cluster but flips a previously-clean cluster red is
  a regression, full stop — it does not net out against the new fix. This is what actually
  prevents "many axes chasing many goals" from degrading into whack-a-mole: the oracle corpus
  *is* the multi-objective specification, built empirically rather than hand-written, and it
  only grows.
- **Every non-functional axis gets the same treatment as a functional one.** F1–F4 performance
  floors, G_ALLOC, G_TEARDOWN, and the new gates in Part 19 below are not "nice to have"
  checks distinct from correctness — a latency or memory regression is a Bar-1 regression under
  this framing, because the stated goal includes "a person doesn't see breakage" and a stall or
  a memory blowup is exactly the kind of breakage a person sees.
- **No gate is ever loosened to make a feature land.** If a new feature's correct
  implementation cannot pass the existing wall, the wall is right and the implementation is
  incomplete — this is the direct antidote to cyclomatic-complexity creep from many
  simultaneously-evolving concerns: the wall is the invariant; features conform to it, not the
  reverse.

---

## Part 19 — Rust Throughput, Concurrency, and Deduplication Discipline

The browser's runtime is a single pervasive hot path, not a set of independent modules with
occasional performance-sensitive sections. This section is mandatory reading before touching
`net`, `layout`, `paint`, `compositor`, `js`, or `page`, and its gates are enforced by Part 19.5
below — not left as style guidance.

### 19.1 Adopt Salsa for incremental computation instead of hand-rolling invalidation
Part 11 of this document (algorithmic efficiency) previously specified extracting Blink's
invalidation-set *algorithm*. Revise that: **adopt Salsa**, the general-purpose Rust
incremental-computation framework that rust-analyzer itself is built on (you are already
depending on rust-analyzer's engine for LSP navigation — this is the same lineage of tool,
proven at the scale of "recompute semantic analysis of a large codebase on every keystroke").

- Model style computation, layout, and paint-list generation as **Salsa queries**: pure
  functions from inputs (DOM state, stylesheet state, viewport state) to outputs (computed
  style, layout box tree, display list). Salsa automatically tracks the dependency graph,
  memoizes results, and on a mutation, recomputes only the transitively-affected queries — this
  is the invalidation-set *outcome* you wanted, produced generically and correctly by a
  battle-tested library instead of hand-implemented and re-debugged.
- Use Salsa's **durability** system for inputs that rarely change (e.g., the parsed UA
  stylesheet, static viewport-independent constants) so that even the graph-traversal
  bookkeeping is skipped for no-op-relevant mutations, not just the recomputation itself.
- Salsa values are cheaply shareable across threads — this composes directly with the Rayon
  parallelism in 19.2 below (parallel style/layout passes reading from the same memoized query
  graph without contention).
- This does not replace the differential oracle's job of verifying *correctness* of what gets
  recomputed — it replaces the *mechanism* by which recomputation scope is determined, and
  removes an entire class of hand-rolled dirty-bit bugs (the exact class that produced the
  wheel-event clone regression in your history — stale or over-broad invalidation is a Salsa
  problem Salsa is specifically designed not to have).

### 19.2 Strict separation: Tokio for I/O, Rayon for CPU-bound data parallelism
This is a hard architectural rule, not a preference:
- **Tokio** owns `net` (hyper/rustls), timers, and any genuinely I/O-bound waiting (subresource
  fetch, WebSocket/streaming, IPC). Tokio's worker threads are sized ~1-per-core and are
  designed for tasks that yield frequently at `.await` points.
- **Rayon** owns CPU-bound, data-parallel work over collections: style recalculation across a
  set of dirty nodes, layout of independent subtrees, paint-list generation across
  non-overlapping regions, hit-testing over spatial partitions. Rayon uses a fixed,
  work-stealing thread pool sized to the core count — this is the mechanism that directly
  answers "effective parallelism tuned to core/thread count": don't hand-tune thread counts
  per-subsystem, let Rayon's pool (sized once, at startup, to `std::thread::available_parallelism()`)
  own that decision globally, and express CPU-bound hot paths as `par_iter`/`join`-style
  parallel operations rather than manual `std::thread::spawn`.
- **Never nest one inside the other without explicit isolation.** Calling Rayon's `par_iter`
  from inside a Tokio worker thread risks thread starvation — Tokio's worker assumes it will
  yield at `.await` points; Rayon's work-stealing assumes threads run CPU work to completion and
  become free. If Tokio's worker blocks waiting on Rayon work that is itself stuck waiting for a
  Tokio-owned resource, both pools can stall. The correct pattern when a Tokio-driven event (a
  network response, a timer) needs to trigger CPU-bound layout/paint work: hand off via
  `tokio::task::spawn_blocking` (or a dedicated channel to a Rayon-consuming thread), let Rayon
  "go wild" on its own pool, and return the result across the boundary — never call into Rayon
  synchronously from async code on a Tokio worker thread.
- Audit every existing `tokio::spawn` call in the layout/paint/style path for this violation as
  part of standing up this rule — this is exactly the kind of latent cross-pool contention that
  won't show up in a light synthetic load bench (the same failure shape as the wheel-event
  clone regression: invisible until real concurrent load).

### 19.3 Minimize task-spawn count on hot paths, not just allocation count
`tokio::spawn`/task creation has real bookkeeping cost (allocation, scheduler queueing,
wake/context-switch) even though it's cheaper than an OS thread. On any per-frame or
per-input-event path:
- Default to inlining trivial async/sync work rather than spawning a task or thread for it.
- For fan-out over a collection (e.g., dirty style nodes, dirty layout subtrees), batch — one
  Rayon parallel-iterator call over the whole collection, not one spawned unit of work per
  element. This is both a throughput rule and a direct extension of the G_ALLOC gate's intent:
  a wheel event that spawns one task per visible DOM node is the same failure class as one that
  clones a rect map per node, just one layer down the stack, and must be caught by the same
  category of gate (19.5).

### 19.4 Contention and duplicate-work elimination
- **Prefer ownership/message-passing over shared locks on any per-frame hot path.** Where a
  `Mutex`/`RwLock` is unavoidable, keep the critical section to the minimum possible span and
  never hold a lock across an `.await` point or a Rayon parallel operation. Prefer Salsa's
  memoized, versioned values (19.1) or immutable/persistent data structures for read-heavy
  shared state (computed style, layout results) over `Arc<RwLock<...>>` — this removes
  read-side contention entirely rather than minimizing it.
- **Coalesce before scheduling, not after.** If multiple independent triggers (a class change, a
  child insertion, a resize) each mark the same subtree dirty within one logical frame, they
  must collapse into a single style/layout/paint pass for that subtree — never one pass per
  trigger. This is the general form of the event-coalescing rule already in Part 11 (scroll
  events); apply the same principle to *any* class of mutation batching, not scroll alone.
- **JS execution must not double-fire.** Microtasks drain exactly once per macrotask boundary;
  `requestAnimationFrame` callbacks batch into exactly one pass per frame; a given event must
  dispatch to a given listener exactly once per logical occurrence even under coalesced input.
  These are correctness properties as much as performance ones — a double-dispatched event is
  both a wasted-work bug and a behavioral-divergence bug the oracle should independently catch
  (G5 interaction parity), so a violation here should show up as two separate gate failures, not
  one; if it only shows up in one, the other gate has a blind spot to close.

### 19.5 The gates that enforce this section (extend Part 5, don't bypass it)
Add these as verify-wall gates, following the same shipping-configuration rule as every other
gate in Part 5:
- **G_SPAWN** (sibling to G_ALLOC): instrument task/thread-spawn count, not just heap
  allocations, between input-event arrival and frame completion. Assert near-zero spawns for
  events with no registered listener, and sub-linear (ideally O(1) or O(changed-subtree), never
  O(DOM-size)) spawns when a listener is registered.
- **G_DEDUP**: instrument a per-frame pass counter for style recalculation, layout, and paint.
  Assert exactly one pass per logical unit of dirty work per frame under normal operation —
  more than one pass over the *same* unhanged subtree within one frame is a hard failure, not a
  warning.
- **G_POOL_ISOLATION**: a targeted check (can be a lint/clippy rule plus a runtime assertion in
  debug builds) that no Rayon parallel operation is invoked from a stack frame currently
  executing on a Tokio worker thread without having crossed a `spawn_blocking` boundary first.
- All three gates run under the shipping Stylo+SpiderMonkey configuration and are subject to
  the risk-based gate scheduler (Part 5.4) for which diffs trigger them — but any diff touching
  `layout`, `paint`, `compositor`, `js`, or the Tokio/Rayon boundary in `page`/`net` always
  triggers the full set, no exceptions, same rule as frozen-interface changes.

---

## Part 20 — Visual Self-Verification Without a Human in the Loop

Claude Code (and any subagent it dispatches) has native multimodal vision and can drive a
browser and inspect screenshots directly — this doesn't require a third-party visual-diffing
service, and the industry's own 2025–2026 trajectory validates the design already specified in
Part 2: raw pixel diffing is the wrong primary signal, since anti-aliasing and font-smoothing
noise dominates it and produces high false-positive rates; the effective tools in this space
converged on coarser, semantic-level diffing for exactly that reason.

- Keep the oracle's primary signal as DOM-geometry + coarse block-grid comparison (already
  specified in Part 2) — this is now externally validated, not just internally reasoned.
- Add a **cheap confirmation pass**: once DOM-geometry clustering (Part 2) narrows a divergence
  to a specific page/region, have the agent capture a screenshot of just that region from both
  engines and describe the visible difference in its own words before a ticket is written. This
  catches the class of divergence that's real but geometry-subtle (a color, a font-weight, a
  border style) without making full pixel-diffing the primary firehose filter.
- This confirmation pass is agent-native by construction — no external SaaS, no human
  screenshot review required, and it composes directly with the subagent-isolation architecture
  in Part 17 (dispatch it as an isolated call, return a condensed verdict).

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
7. Audit the current `text` crate: if it is not already built on Skrifa + HarfBuzz, migrate it
   (Part 15) before further font/text layout work — this is a bounded, high-value integration
   task, do it early.
8. Audit every existing `tokio::spawn` call reachable from the layout/paint/style path for the
   Tokio/Rayon isolation violation described in Part 19.2 — this is likely already present
   latent debt given the codebase predates this rule.
9. Stand up G_SPAWN, G_DEDUP, and G_POOL_ISOLATION (Part 19.5) alongside G_ALLOC/G_TEARDOWN —
   same priority, same "cheap to build, closes a proven gap" justification.
10. Evaluate replacing the planned hand-rolled invalidation-set work (originally specified in
    Part 11) with a Salsa-based incremental computation architecture (Part 19.1) before writing
    new invalidation logic from scratch — this is a build-vs-adopt decision worth making
    deliberately rather than defaulting to "extract the algorithm and hand-implement it," given
    that Salsa is a proven, thread-safe, general solution to exactly this problem.
11. Generate the tree-sitter orientation layer (Part 16.2) once, now, and add it to the EPOCH
    audit checklist for regeneration — use rust-analyzer LSP (already installed) as the default
    precise-navigation tool from this session forward per Part 16.1.

---

## Part 21 — Reprioritization Directive: Attack the Wall Before the Backlog

A self-audit against this methodology surfaced that the highest-leverage prescribed items —
verify-wall compression and oracle breadth — were specified in Parts 2 and 10 but had not
actually been executed, while tick-level backlog work continued. This part exists to close that
gap explicitly and to state, without ambiguity, the correct ordering.

### 21.1 What's already correct and does not change
The two-bar split (functional breadth vs. pixel precision), the breadth-first selection rule,
the differential oracle as the discovery mechanism, and "add the measurement, not just the
patch" as the standing response to any gate that reported green through a real problem — all of
this is working as designed. Recent class-bug fixes (an unmapped `font-family`, flex items that
couldn't shrink, percentage-squaring, stretched images, `font-size: 0` painting glyph-shaped
content across the page, silently-deleted absolute boxes) confirm the shape is right: each was
found by measurement, fixed once, at the class level, and moved many sites at once. Keep doing
exactly that. The problem is not what gets fixed — it's that the infrastructure making fixes
cheap and discovery automatic has been under-built relative to how much it was already
prescribed.

### 21.2 The reordering, in force starting now
1. **Cut the verify wall to under five minutes before resuming backlog ticks.** This is
   infrastructure, it is boring, and it multiplies every subsequent tick — that multiplier is
   why it outranks any single feature fix on the ledger. Concretely and in this order: mold/lld
   linking, cargo-nextest, workspace-hack to stop shared-type edits from cascading into
   mozjs/Stylo rebuilds, and the risk-based gate scheduler (Part 5.4) so a diff touching only
   `paint` cannot trigger a full G2 JS-conformance run. Run the full wall (full corpus, full
   Chromium round-trips, full release build) only on a timer or immediately before release
   banking — never on every tick regardless of what the diff touched. If the wall drops from
   ~30 minutes to ~5, tick rate roughly triples; nothing else available right now has that
   multiplier.
2. **Run the oracle at 200–500 sites now, and let its cluster ranking be the ledger — not
   judgment.** A 20-site corpus is an anecdote about the web, not a measurement of it. Widening
   the crawl frame is not optional polish on Part 2 — it is the difference between "breadth of
   the internet" being a claim and being a number. Re-crawl every few ticks; the ranking that
   comes out is the plan, not an input judgment reweighs.
3. **Load ten real SPA starter apps immediately and mine their exceptions, in parallel with
   (1) and (2), not after.** The SPA web is the single largest unmeasured unknown in the whole
   schedule, and it's a binary risk: if the missing substrate is additive IDL/scheduling work
   it's fast, and if hydration failure cascades into a scheduling-fidelity subsystem it isn't —
   you cannot plan around that distinction while it's unmeasured, and the measurement (the
   Framework Exception Miner, Part 9) is cheap and already works. Converting the largest unknown
   into a bounded, enumerated list is worth doing before committing to any timeline that
   includes the app web.
4. **Only then: incremental invalidation.** Today's full re-cascade on every mutation is
   O(page) per event. Sub-frame latency holding up right now is an artifact of pages not yet
   invalidating much under real interactive load — it will not stay true as SPA and
   interaction coverage grows. This is the one item on this list that is *asymptotically*
   wrong, not merely incomplete, which is why it's sequenced ahead of ordinary backlog work
   but behind the three infrastructure items above (it depends on Salsa per Part 19.1, and
   that decision should be made deliberately, not mid-implementation).
5. **Keep deferring Bar 2.** Pixel precision is dominated by font metrics — Part 15's
   Skrifa/HarfBuzz adoption addresses the largest piece of it, but what remains is one deep
   problem, not many shallow ones. Neither parallelism nor more algorithm-reading shortens a
   deep problem, and depth here buys far less than breadth does — a browser that's pixel-exact
   on one site and broken on a thousand others is not what "usable" means under this
   methodology. Do not let a tick get pulled into micro-tuning one site's rendering to
   pixel-exactness; that effort belongs in Bar 2, and Bar 2 stays deferred.

### 21.3 The standing discipline this establishes
When self-assessment (yours, Claude Code's, or a future EPOCH audit's) finds a gap between what
this document prescribes and what has actually been built, the correct response is to
reprioritize toward closing that gap immediately, not to add the finding to the backlog at its
default priority. Infrastructure that multiplies every future tick's velocity is categorically
different from a feature fix that helps once, and the ledger's data-driven ranking (Part 4)
should be understood to sit *below* this class of infrastructure decision, not above it — Part 4
ranks what to fix once wall-compression and oracle breadth are actually in place, it does not
override the sequencing of building them.

---

## Part 22 — System-Level Runtime Health & Call-Graph Leanness Audit

The gates specified so far (G_ALLOC, G_TEARDOWN, G_SPAWN, G_DEDUP, G_POOL_ISOLATION) catch
specific, named regression classes once they're known. This part adds a standing, general
discipline aimed at the categories those gates don't individually name: contention, hangs,
silent runtime failures, and redundant work anywhere in the load/render/script call graph — the
things a user experiences as "this browser feels unstable" or "this page just never finished
loading," independent of whether any single named gate happens to cover the specific cause.

### 22.1 No silent failure, anywhere, ever
The historical failure where a swallowed script exception ("a page `<script>` threw;
continuing") hid two missing IDL properties killing navigation on every mdbook site is the
general case, not a one-off. Extend that lesson system-wide: every subsystem — page load,
subresource fetch, DOM parsing, style computation, layout, paint, script execution, event
dispatch — must surface failures into the journal/oracle signal, never absorb them silently.
- **G_SILENT_FAIL**: audit every `catch`/`Result`-discarding/`.ok()`/error-swallowing site
  reachable from the page-load and script-execution paths. A caught error that isn't logged,
  surfaced to the Framework Exception Miner (Part 9), or turned into a journal entry is treated
  as a gate violation, not acceptable defensive coding. "Continuing" after an error is only ever
  acceptable if the error is also recorded somewhere the discovery pipeline can see it.

### 22.2 No hangs, no silent runtime failures on page load
A page that never finishes loading, or an interaction that never completes, is a stability
failure category distinct from a rendering-correctness bug, and it needs its own gate rather
than being an assumed side effect of correctness gates.
- **G_HANG**: every page-load, navigation, and interaction-latency test in the verify wall runs
  under an explicit watchdog timeout. A timeout is a hard failure, not a slow pass — treat "the
  gate eventually returned" and "the gate hung and the harness moved on" as categorically
  different outcomes, and instrument the harness so it cannot mistake the second for a skipped
  or inconclusive test.
- Extend the oracle (Part 2) and the SPA Framework Exception Miner (Part 9) to record
  load/hydration wall-clock time per site/app, not just pass/fail — a page that "passes" but
  took 40x longer than Chromium's render is a stability signal the current diff-based clustering
  won't surface on its own, and it belongs in the same cluster-and-rank pipeline as correctness
  divergence.

### 22.3 No contention, no redundant work in the call graph
This generalizes G_DEDUP and G_POOL_ISOLATION (Part 19.5) beyond style/layout/paint to the full
page lifecycle:
- **No duplicate resource fetches**: a given URL should not be fetched twice for one navigation
  unless the page genuinely requests it twice (cache-busting, explicit re-fetch) — audit the
  `net` crate's request path for this specifically, since duplicate fetches are both a
  performance bug and a correctness risk (stale/inconsistent state between the two copies).
- **No duplicate tree renders per navigation**: instrument a counter for full-document
  parse/layout/paint passes per navigation event; more than one for a single navigation (absent
  an explicit re-navigation) is a hard failure, following the same pattern as G_DEDUP's
  per-frame pass counter but scoped to page-load rather than interaction.
- **No duplicate JS module execution or re-evaluation**: a `<script>` or module must not
  execute twice for one load unless the page's own semantics require it (module identity/caching
  rules must be respected) — this is both a spec-correctness item and a leanness item, since
  double-execution is wasted work with potential double-side-effect bugs as a byproduct.
- **No unbounded lock contention on the load/render hot path**: this is Part 19.4's contention
  discipline, restated as an audit target specifically for page-load rather than just
  interaction — profile a cold navigation under concurrent load (multiple tabs/navigations, if
  that's in scope, or repeated rapid navigation in a single tab otherwise) and confirm no lock is
  held across an `.await` or a Rayon parallel operation anywhere in the load path, per 19.4's
  existing rule.

### 22.4 Standing audit cadence
This is not a one-time pass. Fold a runtime-health sweep — checking 22.1–22.3 against whatever
subsystems changed since the last EPOCH audit — into every EPOCH audit alongside the existing
change-coupling matrix refresh (3.3) and the oracle re-crawl (Part 2). A subsystem that was
clean at the last audit is not guaranteed to still be clean; new code is where new silent
failures, new duplicate work, and new contention get introduced, so the audit target list grows
with the codebase, it does not shrink to "known problem areas only."

---



## Immediate Action Items (first sessions under this methodology)

**Tier 0 — do these before touching the backlog, per Part 21.2. Nothing below this line starts
until these are done or genuinely blocked:**
1. Cut the verify wall to under five minutes: mold/lld, cargo-nextest, workspace-hack, and the
   risk-based gate scheduler (Part 5.4). Full wall runs only on a timer or before release
   banking from this point forward, never on every tick unconditionally.
2. Widen the oracle's crawl frame to 200–500 sites (Part 2, Part 21.2 item 2) and regenerate the
   priority ledger from its cluster ranking — stop hand-selecting what to fix next.
3. Load ten real SPA starter apps and run the Framework Exception Miner (Part 9) against all of
   them, in parallel with items 1–2, not sequenced after the document-web work.

**Tier 1 — cheap, high-value, do alongside Tier 0, not blocked by it:**
4. Add G_ALLOC and G_TEARDOWN to the verify wall (Part 5.2–5.3).
5. Stand up G_SPAWN, G_DEDUP, and G_POOL_ISOLATION (Part 19.5).
6. Stand up G_SILENT_FAIL and G_HANG (Part 22.1–22.2) — same "cheap, closes a proven gap"
   justification as the gates above.
7. Compute the change-coupling matrix from this repo's own git history (Part 3.3).
8. Audit the current `text` crate for Skrifa + HarfBuzz (Part 15) before further font work.
9. Audit every existing `tokio::spawn` call reachable from layout/paint/style for the
   Tokio/Rayon isolation violation (Part 19.2) — likely latent debt predating this rule.
10. Begin journaling in the Part 7 structure now, so the removal-model estimate has real data at
    the next EPOCH audit.
11. Generate the tree-sitter orientation layer (Part 16.2) once, now; use LSP as the default
    precise-navigation tool (Part 16.1) from this session forward.

**Tier 2 — sequenced after Tier 0 lands, per Part 21.2 item 4:**
12. Evaluate the Salsa-based incremental computation architecture (Part 19.1) as the replacement
    for hand-rolled invalidation sets — this is the one item on the original list that's
    asymptotically wrong today (O(page) per event), not merely incomplete, so it's a priority
    once infrastructure is in place, but it depends on a deliberate build-vs-adopt decision, not
    a mid-implementation default.

---

## Part 23 — Bar 0: The Stability Floor (prior to and independent of Bar 1)

Bar 1 asks "does this look broken." That's a correctness question. There's a distinct,
more fundamental question that has to be true first: **does the engine ever crash the process,
panic unrecoverably, or hang with no way out, on any page matching a covered pattern class —
and does it degrade gracefully rather than catastrophically on the tail of patterns it doesn't
yet cover.** A page can look wrong and still satisfy Bar 0. A page that renders perfectly nine
times out of ten and hangs the whole browser the tenth time violates Bar 0 regardless of how
good the other nine renders looked. Treat Bar 0 as the floor everything else stands on, checked
before Bar 1 correctness is even asked about for a given pattern class.

### 23.1 What Bar 0 requires
- **No SpiderMonkey-originated crash cascades.** A script that triggers a SpiderMonkey fault
  must not take down the browser process. This needs an explicit containment boundary at the
  FFI edge — catch what can be caught, and where a genuine SpiderMonkey-side fault can't be
  caught in-process, the containment has to happen at the tab/task level (23.2), not by hoping
  it never happens.
- **No unrecoverable Rust panics at the process level.** A panic within a single page's
  render/script/layout work must not unwind past that page's task boundary and take the whole
  process down with it.
- **No unrecoverable hangs.** Covered by G_HANG (Part 22.2) for the verify wall's own tests;
  Bar 0 extends the same expectation to production behavior — a hung page load or hung script
  must be interruptible (by the user, or by an internal watchdog) rather than freezing the
  browser with no recourse.

### 23.2 Containment, not just prevention
You will not prevent every crash-class bug before Bar 1 — that's not the goal. The goal is that
a failure in one page/tab is *contained* to that page/tab: a supervised task boundary per
tab/navigation (Rust `catch_unwind` at the render-task level, or an internal watchdog that can
terminate and restart a hung renderer task) so the failure mode for an uncovered pattern is
"this tab closes or reloads with a graceful message," never "the whole browser crashes or
hangs." This is a narrower, cheaper version of full multi-process isolation (correctly deferred
per the "walk before we run" security decision) — it doesn't require process-level sandboxing,
just a supervised task/panic boundary per tab, and it should be built now rather than deferred,
because it's what makes the 99%-pattern-coverage strategy in Part 24 actually safe to ship
against the uncovered 1%.

### 23.3 Where this fits the gate structure
Extend G_HANG and G_TEARDOWN (Parts 5.3, 22.2) to explicitly assert containment: a synthetic
test that deliberately triggers a panic or a hang inside one simulated tab's task must result in
that tab closing/reloading, with the rest of the harness (and, in a real session, other tabs)
continuing to function. If containment isn't yet built, this is a Tier 0/1-class item — build it
alongside G_HANG, not after.

---

## Part 24 — Website-Class Corpus: Pattern Coverage, Not Per-Site Tuning

This section makes explicit something the differential oracle (Part 2) has been doing
implicitly since it was specified, so it's understood as one mechanism, not built twice under
two names.

### 24.1 A cluster *is* a website class
Chromium and Gecko engineers don't write bespoke code per website — they cover representative
HTML/CSS/JS *patterns*, and the vast majority of real sites are combinations of a comparatively
small number of recurring patterns. The oracle's clustering step (Part 2.1, step 4) already does
exactly this: a cluster with N site-hits and one root cause **is** a website-class, empirically
discovered rather than hand-enumerated. Do not build a separate "website class taxonomy" as a
new artifact — the oracle's cluster registry already is that taxonomy, and it's better than a
hand-written one because it's derived from what actually diverges across real sites rather than
guessed at.

### 24.2 The target, stated precisely
The goal is not "render every website correctly." It's: **cover enough pattern classes,
weighted by real-world prevalence (Part 4's usage-frequency data), that the covered classes
represent something like the large majority of real-world page structure — and for whatever
falls outside that coverage, Bar 0 containment (Part 23) ensures the failure mode is graceful,
not catastrophic.** This reframes "breadth of the internet" from an open-ended, unbounded goal
into a measurable one: coverage percentage of the oracle's crawl frame, weighted by cluster
site-count, is the actual metric — track it explicitly as part of the priority ledger's output,
not just the ranked list of what to fix next.

### 24.3 Crashes and hangs are oracle signals too, and outrank visual divergence
Extend the oracle's diff/cluster pipeline (Part 2.1) to capture a third signal category
alongside DOM-geometry and coarse-pixel diff: **did the engine crash, panic, or hang on this
page at all, independent of whether the render was otherwise correct.** Cluster and rank these
the same way as visual divergence clusters, but weight them above visual divergence in the
priority ledger (Part 4) — a pattern class that crashes the engine is a Bar 0 violation and is
categorically more urgent than a pattern class that merely renders with a visual bug, which is
"only" a Bar 1 gap.

---

## Part 25 — Process & Runtime Minimization Audit

G_SPAWN (Part 19.5) governs per-event task-spawn counts on interactive hot paths. This section
extends the same discipline to the **process-level and runtime-level** structure of the engine
as a whole, which is a distinct failure class: not "too many tasks per click," but "too many
long-lived runtimes/pools/executors existing at all."

### 25.1 The rule
Enumerate every long-lived runtime, thread pool, or executor the process creates: the Tokio
runtime(s), the Rayon pool, any spawned subprocess, any per-tab or per-navigation thread. There
should be **exactly one of each**, created once at process startup and reused for the life of
the process, unless multiplicity is architecturally justified and explicitly documented (e.g.,
one supervised task per tab per Part 23.2 is justified isolation, not proliferation — the
distinction is that a *task* running on a shared runtime is fine; a *new runtime* per action is
not). The canonical failure case to check for explicitly: a new Tokio runtime instantiated per
search, per navigation, or per any other repeated user action, instead of one runtime created
once and reused. This is exactly the kind of latent, invisible-at-idle bug the wheel-event
clone regression already taught this project to watch for, one layer up the stack.

### 25.2 G_RUNTIME_COUNT
Add a gate, alongside G_SPAWN: instrument the process to count distinct runtime/pool/executor
instantiations over a scripted session (multiple navigations, multiple searches, multiple tab
opens). Assert the count stays flat regardless of how many user actions occur — one Tokio
runtime, one Rayon pool, for the whole session, not one per action. A rising count under
repeated action is a hard failure.

### 25.3 Standing audit cadence
Fold this into the same EPOCH-audit cadence as the runtime-health sweep (Part 22.4) and the
change-coupling matrix refresh (Part 3.3) — new code is where new proliferation gets introduced
(a new feature that "just spawns a quick runtime to handle this one thing" is the natural way
this regresses), so the audit target grows with the codebase rather than being a one-time check.

---

## Part 26 — Standing Depth/Breadth Self-Check (continuous, not just reactive)

Part 21 corrected a specific instance of this failure after the fact — time spent on pixel-level
parity against a 20-site suite when the oracle's crawl frame and the SPA frontier were both
still unmeasured. The reactive correction was necessary once, but the underlying judgment call —
"is this tick going deep on something narrow, or wide on something that generalizes" — recurs
every tick, and waiting for another audit to catch the next instance is strictly worse than
catching it before it happens.

### 26.1 The standing check
At the start of every tick, before starting implementation work, state explicitly (in the
journal entry, per Part 7) which of these two shapes the tick is:
- **A pattern-class fix**: targets a root cause the oracle's clustering (Part 2, Part 24)
  identified as explaining multiple sites, or closes a Bar 0 containment gap (Part 23), or
  builds/compresses infrastructure that multiplies future ticks (Part 21). This is in scope by
  default.
- **Single-site or single-instance tuning**: matching one specific site's exact pixel output,
  or fixing a divergence the oracle hasn't clustered as recurring. This is Bar 2 or lower
  priority by default, and doing it now requires an explicit reason stated in the journal (e.g.,
  "this site is disproportionately representative of an uncovered pattern class" — a real
  reason; "it bothered me" is not).
- If a tick starts as the first shape and drifts into the second (a pattern-class fix that turns
  into hours of matching one site's exact rendering), that drift is itself the signal Part 21
  was built to catch — name it in the journal and pivot back, don't let the sunk cost of the
  session carry it forward.

### 26.2 Why this is cheap to enforce
This doesn't require new tooling — it requires one additional line in the journal structure
(Part 7) per tick, stating the tick's shape before work starts. A journal that never states "Bar
2 / single-site, and here's why" is a journal where every tick defaulted to breadth, which is
the desired steady state; a journal that starts accumulating unexplained single-site ticks is a
visible, checkable signal in `STATUS.md` (per the enforcement discussion) that the pull toward
depth is happening again, well before it costs a whole session's worth of drift.

---

## Part 27 — Resource-Budget-Aware Scheduling for the Actual Hardware

The methodology so far assumes parallelism (Rayon sized to core count, the verification key
pool, concurrent oracle crawl workers) without accounting for the fact that these consumers
share one finite, concrete resource envelope: a 24-core/32-thread CPU and 32GB of RAM. RAM, not
core count, is very likely the binding constraint here, given rustc/LLVM codegen for large
crates (mozjs bindings especially), N concurrent headless-Chromium instances for oracle
crawling, the rust-analyzer LSP server, and the tmpfs-mounted build directory (Part 10.1) all
draw from the same physical pool simultaneously.

### 27.1 Don't let RAM-heavy activities run uncoordinated
- **Sequence or throttle oracle crawling against active compilation.** A full mozjs/Stylo
  rebuild and a wide oracle crawl (many concurrent headless Chromium workers) competing for the
  same RAM risks swapping/thrashing rather than either finishing faster. Start oracle crawl
  concurrency conservatively (a small worker count) and tune up empirically by watching actual
  memory headroom during a real crawl, rather than assuming the full 32GB is available to
  whichever process asks for it first.
- **The tmpfs target directory (Part 10.1) is itself a RAM consumer** — its size budget and the
  oracle crawl's worker count both draw from the same 32GB, so size one with the other in mind,
  not independently.

### 27.2 Turn idle time into scheduled background work
The 24 cores are a fixed, scarce resource for a solo operator — unlike a datacenter, there's no
elastic burst capacity, so idle time between interactive sessions is real, recoverable throughput
being wasted if nothing is scheduled to use it. Queue background jobs — oracle re-crawl, the
nightly/async full WPT run (Part 8), delta-debug reduction backlog (Part 2.1) — to run
automatically whenever Claude Code is not actively mid-edit, rather than only running them when
explicitly invoked in an interactive session.

### 27.3 The verification key pool is this project's actual "distributed compute"
Restate explicitly what Part 6 already implies: the ~25-key, two-provider verification pool is
not a minor convenience, it's the practical answer to "what does this solo project have that
looks like Chromium-team-scale automated capacity." It offloads narrow, closed-question work
(delta-debug reduction, WPT triage, exception parsing) off the local 24-core machine entirely,
onto compute this project doesn't have to provision, cool, or maintain. Treat it as the primary
lever for anything narrow and parallelizable, before reaching for more local-machine
parallelism — it doesn't compete with the local RAM/CPU budget in Part 27.1 at all.

### 27.4 If the resource envelope ever changes
If more compute ever becomes available (cloud burst capacity, a distributed sccache backend, a
larger local machine), the things that change are: oracle crawl frame size (Part 2, currently
targeting 200–500 sites, could scale toward the tens of thousands used by industry differential
fuzzers), WPT running continuously rather than nightly (Part 8), and wider speculative/redundant
implementation attempts (Part 3.4) for narrow items with a decisive oracle verdict. None of the
*architecture* changes — Salsa, the Tokio/Rayon split, the gate structure, the oracle's
clustering logic all scale up unchanged; only the parallelism width and crawl breadth would
grow. Note this explicitly so a future decision to add resources is additive to this
methodology, not a reason to redesign it.

---

## Immediate Action Items (first sessions under this methodology)

**Tier 0 — do these before touching the backlog, per Part 21.2. Nothing below this line starts
until these are done or genuinely blocked:**
1. Cut the verify wall to under five minutes: mold/lld, cargo-nextest, workspace-hack, and the
   risk-based gate scheduler (Part 5.4). Full wall runs only on a timer or before release
   banking from this point forward, never on every tick unconditionally.
2. Widen the oracle's crawl frame to 200–500 sites (Part 2, Part 21.2 item 2) and regenerate the
   priority ledger from its cluster ranking — stop hand-selecting what to fix next.
3. Load ten real SPA starter apps and run the Framework Exception Miner (Part 9) against all of
   them, in parallel with items 1–2, not sequenced after the document-web work.

**Tier 1 — cheap, high-value, do alongside Tier 0, not blocked by it:**
4. Add G_ALLOC and G_TEARDOWN to the verify wall (Part 5.2–5.3).
5. Stand up G_SPAWN, G_DEDUP, and G_POOL_ISOLATION (Part 19.5).
6. Stand up G_SILENT_FAIL and G_HANG (Part 22.1–22.2) — same "cheap, closes a proven gap"
   justification as the gates above.
7. Compute the change-coupling matrix from this repo's own git history (Part 3.3).
8. Audit the current `text` crate for Skrifa + HarfBuzz (Part 15) before further font work.
9. Audit every existing `tokio::spawn` call reachable from layout/paint/style for the
   Tokio/Rayon isolation violation (Part 19.2) — likely latent debt predating this rule.
10. Begin journaling in the Part 7 structure now, so the removal-model estimate has real data at
    the next EPOCH audit.
11. Generate the tree-sitter orientation layer (Part 16.2) once, now; use LSP as the default
    precise-navigation tool (Part 16.1) from this session forward.
12. Build the per-tab supervised panic/hang containment boundary (Part 23.2) — this is what
    makes shipping against 99%-pattern-coverage (Part 24) safe rather than reckless, and it's
    Tier-1 cheap: a `catch_unwind` boundary per tab task plus a watchdog, not full process
    isolation.
13. Stand up G_RUNTIME_COUNT (Part 25.2) and audit existing code for the "new Tokio runtime per
    action" failure pattern specifically — check `net`, `page`, and anywhere a search/navigation
    handler exists.
14. Start stating tick-shape (pattern-class fix vs. single-site tuning, Part 26.1) in every
    journal entry from this point forward — this is a zero-cost habit change, not a build task,
    so there's no reason to delay it to a later tier.

**Tier 2 — sequenced after Tier 0 lands, per Part 21.2 item 4:**
12. Evaluate the Salsa-based incremental computation architecture (Part 19.1) as the replacement
    for hand-rolled invalidation sets — this is the one item on the original list that's
    asymptotically wrong today (O(page) per event), not merely incomplete, so it's a priority
    once infrastructure is in place, but it depends on a deliberate build-vs-adopt decision, not
    a mid-implementation default.

---

## Part 30 — JS Runtime: SpiderMonkey Confirmed, Compat Gap Closed by Shim

This is a two-part decision, and both parts got investigated rather than assumed, which is why
both belong in Part 29.2's Settled Decisions list. Record the reasoning here so it doesn't need
re-deriving later.

### 30.1 SpiderMonkey vs. V8: the capability gap people assume exists mostly doesn't

The intuition that "some sites only work on Chromium, so V8 must be more capable" doesn't hold
up. Sites broken on Firefox and working on Chromium are overwhelmingly explained by intentional
browser-sniffing and untested library assumptions, not JS-engine conformance gaps — both engines
are near-fully ECMAScript-conformant, and the old "V8 is just faster" narrative traces back to a
retired, V8-over-tuned synthetic benchmark that modern holistic benchmarks don't reproduce.
For this project's specific architecture — a Rust engine embedding a JS runtime via FFI — the
more decision-relevant fact is that SpiderMonkey's Rust bindings are the most mature,
"browser-grade" JS-engine integration in the Rust ecosystem, more proven than V8's own Rust
embedding path, which has documented integration gaps (missing host objects, challenges
supporting `ExternalArrayBuffer`) in other projects that tried it. SpiderMonkey was the right
choice, and this isn't a hedge — the leanness this project chose it for was never actually in
tension with capability at the ECMAScript-conformance level.

### 30.2 The bar rules out lean/embedded-tier engines entirely, not just SpiderMonkey vs. V8

Worth stating explicitly since it closes off a broader class of "what if we went leaner"
questions before they get relitigated one at a time: engines like QuickJS, Hermes, and
JerryScript are real, well-built, and genuinely lean — but they're built for IoT/mobile/embedded
scripting workloads, not for clearing full browser-grade capability. They trade away modern
JIT tiering, full built-in debugger/profiler support, and depth of spec/API coverage for
footprint, which is exactly the right trade for their target use case and exactly the wrong
trade for this one. **The stated bar is Chromium capability parity, which is itself a superset
of what Firefox/Gecko can do — clearing that bar requires a full, modern, browser-grade JIT
engine, full stop.** That leaves exactly two real candidates, SpiderMonkey and V8, and 30.1
settles which of those two. Don't let "but this other engine is leaner" reopen the question
later without new evidence that the capability bar itself has changed — leanness that costs
capability isn't a trade this project can make, given what Bar 1 already requires.

### 30.3 The one real, narrow gap — and it's a shim, not a rewrite

The one concrete, well-documented compatibility gap that *is* real: `Error.captureStackTrace`,
`Error.prepareStackTrace`, and `Error.stackTraceLimit` are non-standard V8-only APIs that a
meaningful number of popular JS libraries feature-detect and depend on for custom error classes
— real, recurring "works in Chrome, throws in Firefox" bug reports exist against exactly this
API family across multiple widely-used libraries. It's currently a TC39 standardization proposal
precisely because it's become enough of a web-compatibility problem to need standardizing, which
means implementing it now is adopting something headed toward the spec, not chasing a
V8-specific quirk. A smaller, related gap: V8 parses some non-ISO-8601 date-time string formats
more leniently than SpiderMonkey does; some real-world code depends on that leniency.

**The fix, concretely:** implement the `Error.captureStackTrace` family as custom globals in the
JS-environment setup code — the same mechanism used to expose any other API to scripts, not a
SpiderMonkey-internals patch, fully consistent with the "never patch Stylo/SpiderMonkey" rule.
Do the same for the Date-parsing leniency gap if/when the oracle or Framework Exception Miner
surfaces real sites depending on it. Both are bounded, cheap, embedding-layer work.

### 30.4 Extend the Framework Exception Miner to catch this class of gap, not just missing IDL

Part 9's Framework Exception Miner was specified to catch missing standard DOM/IDL surface (the
`a.protocol`/`document.scrollingElement` pattern). Extend its scope explicitly to also catch
non-standard-but-widely-depended-upon API gaps like this one — a library throwing
`TypeError: Error.captureStackTrace is not a function` against a real starter app or a real
crawled site is the identical signal shape (a thrown exception naming exactly what's missing),
and the miner already knows how to surface and rank this kind of signal. Don't build a separate
mechanism for "non-standard API compat" — route it through the same exception-mining pipeline
and let real code, not a hand-maintained list, determine which non-standard shims are actually
worth implementing.

---

## Part 31 — Interaction Surface Knowledge Base: Institutional Memory for What Makes the Web Clickable/Automatable in This Engine

Part 12 specifies the *mechanism* for agent-native automation (WebDriver BiDi, the a11y tree as
single source of truth). It does not, by itself, accumulate the *empirical knowledge* this
project is generating every tick about what actually makes a real page's interaction surface
work correctly here — which button-selection patterns resolve correctly, which scroll/click/
form-fill techniques hold up, which hit-testing edge cases break, and why. That knowledge is
currently scattered across `JOURNAL.md` entries (tick-narrative, write-heavy, per Part 29) and
gate pass/fail history (G5, G6 — which record *that* something broke, not the accumulated
pattern-level understanding of *why*, generalized). This is a real, distinct gap: the oracle's
cluster registry (Parts 2, 24) already serves this role for *rendering* knowledge — a cluster
*is* an empirically-discovered rendering pattern class. There is no equivalent structured,
growing artifact for *interaction* knowledge, and interaction correctness is the direct
substrate of this project's agent-native north star (Part 0's mission statement), not a
side concern to it.

### 31.1 What this artifact is, and why it's separate from what exists

Create `INTERACTION-SURFACE.md` (or a structured directory if a single file outgrows it) as a
peer artifact to `STATUS.md`, `JOURNAL.md`, and `PARITY-LEDGER.md` — not a replacement for any
of them. It differs from the journal in the same way Part 29 already distinguishes lesson
promotion from journal archiving: this is **curated and structured, indexed by
pattern/technique, not by tick** — a fresh session or an external consumer should be able to
look up "how does this engine handle click targets under CSS transforms" or "what's the
`:checked`-class interactivity story" and get a direct answer, not have to search N journal
entries hoping one of them covered it.

### 31.2 Entry schema
Each entry records, at minimum:
- **Pattern/technique name** (e.g., "checkbox-hack CSS-only interactivity," "click targets
  inside `display:none`-then-toggled containers," "hit-testing under `transform: scale()`,"
  "label/form-control association for autofill and programmatic fill," "focus order across
  shadow-DOM-equivalent boundaries," "scroll-anchor stability during dynamic content
  insertion").
- **Status**: solved / partially solved / known gap.
- **What makes it work correctly here**, or what's still broken and why — stated as the
  underlying mechanism, not just "fixed in tick N."
- **Which gate or oracle signal would catch a regression** (G5, G6, the a11y-tree consistency
  check, or a named synthetic test) — this ties every entry back to something mechanical that
  protects it, per Part 28's discipline: knowledge without a gate behind it is exactly the kind
  of thing that regresses silently.
- **Source**: the tick/commit/journal entry it was learned from, for provenance back into
  `JOURNAL.md` per Part 7/29.

### 31.3 Mechanical maintenance — not context-dependent, per Part 28's discipline
The failure mode this must avoid is the one Part 29 named for the journal generally: a lesson
that only exists in someone's memory of a session is not memory, it's an archive nobody
reliably reopens. For this specific knowledge base, the capture policy is stricter than Part
29.1's "promote on recurrence" rule, because interaction-surface knowledge is directly
load-bearing for the project's stated agent-native mission, not a general lesson that may or
may not generalize: **any commit touching interaction-relevant code paths** — event dispatch,
hit-testing, focus/click/scroll/form-handling logic, a11y-tree computation, or the `agent`
crate — **must either reference an existing `INTERACTION-SURFACE.md` entry it's fixing/extending,
or add a new entry**, enforced by the same pre-commit hook mechanism as the journal-entry
requirement (Part 28.2). Determine "interaction-relevant" via the change-coupling matrix (Part
3.3) plus a fixed starting path list (the crates named above) rather than guessing case by case.

### 31.4 Backfill now, not just going forward
Do a one-time retroactive pass — the same shape as Part 29's action item for lesson promotion —
over the existing journal and gate history to extract everything already learned about
interaction correctness into this structured form now: the `:checked` stub and its fix, any
absolute-positioning/click-target bugs already found, any G6 clickability failures already
resolved, anything already known about form controls, scroll behavior, or focus handling. This
knowledge already exists in the project's history; it just isn't in the form that makes it
reusable yet.

### 31.5 Downstream consumers — named explicitly, including ones not yet built
State plainly why this is worth building now rather than deferring: this knowledge base is
consumed by (a) the a11y-tree-as-oracle work (Part 12) — it's the closest thing this project has
to a spec for what "correctly interactable" means here, and should directly inform G5/G6 test
design going forward rather than the tests and the knowledge evolving separately; (b) any
future in-browser prompt-to-action agent surface — noted here as a named future consumer without
committing to its design, since it doesn't exist yet, but the knowledge base should be built as
if it will need to be read by that surface eventually; (c) the external-framework-facing
automation surface (WebDriver BiDi, Part 12); (d) uses not yet anticipated. On (d) specifically:
entries do not need to justify themselves against a currently-known downstream consumer to be
worth capturing — the whole point of institutional knowledge is that its highest-value uses are
often not the ones foreseen when it was recorded. Capture broadly per 31.3's mandatory scope;
let future consumers decide what's useful to them rather than pre-filtering for only what seems
useful now.

### 31.6 Cadence
Fold a review of `INTERACTION-SURFACE.md`'s growth and coverage into the same EPOCH-audit
cadence as everything else (Parts 3.3, 22.4, 25.3) — check that commits touching
interaction-relevant paths are actually producing entries (per 31.3), not just that the file
exists. A knowledge base that stopped growing while interaction-relevant commits kept landing is
the same class of silent drift Part 26 names for tick-shape claims, and should be caught the
same way: mechanically, not by someone happening to notice.

---

## Immediate Action Items (first sessions under this methodology)

**Tier 0 — do these before touching the backlog, per Part 21.2. Nothing below this line starts
until these are done or genuinely blocked:**
1. Cut the verify wall to under five minutes: mold/lld, cargo-nextest, workspace-hack, and the
   risk-based gate scheduler (Part 5.4). Full wall runs only on a timer or before release
   banking from this point forward, never on every tick unconditionally.
2. Widen the oracle's crawl frame to 200–500 sites (Part 2, Part 21.2 item 2) and regenerate the
   priority ledger from its cluster ranking — stop hand-selecting what to fix next.
3. Load ten real SPA starter apps and run the Framework Exception Miner (Part 9) against all of
   them, in parallel with items 1–2, not sequenced after the document-web work.

**Tier 1 — cheap, high-value, do alongside Tier 0, not blocked by it:**
4. Add G_ALLOC and G_TEARDOWN to the verify wall (Part 5.2–5.3).
5. Stand up G_SPAWN, G_DEDUP, and G_POOL_ISOLATION (Part 19.5).
6. Stand up G_SILENT_FAIL and G_HANG (Part 22.1–22.2) — same "cheap, closes a proven gap"
   justification as the gates above.
7. Compute the change-coupling matrix from this repo's own git history (Part 3.3).
8. Audit the current `text` crate for Skrifa + HarfBuzz (Part 15) before further font work.
9. Audit every existing `tokio::spawn` call reachable from layout/paint/style for the
   Tokio/Rayon isolation violation (Part 19.2) — likely latent debt predating this rule.
10. Begin journaling in the Part 7 structure now, so the removal-model estimate has real data at
    the next EPOCH audit.
11. Generate the tree-sitter orientation layer (Part 16.2) once, now; use LSP as the default
    precise-navigation tool (Part 16.1) from this session forward.
12. Build the per-tab supervised panic/hang containment boundary (Part 23.2) — this is what
    makes shipping against 99%-pattern-coverage (Part 24) safe rather than reckless, and it's
    Tier-1 cheap: a `catch_unwind` boundary per tab task plus a watchdog, not full process
    isolation.
13. Stand up G_RUNTIME_COUNT (Part 25.2) and audit existing code for the "new Tokio runtime per
    action" failure pattern specifically — check `net`, `page`, and anywhere a search/navigation
    handler exists.
14. Start stating tick-shape (pattern-class fix vs. single-site tuning, Part 26.1) in every
    journal entry from this point forward — this is a zero-cost habit change, not a build task,
    so there's no reason to delay it to a later tier.
15. Extend the journal schema and pre-commit hook per Part 28.2: require a real oracle cluster
    ID for any `pattern-class`-tagged tick, cross-checked against the last crawl's output — this
    converts the tick-shape self-check from a stated claim into a verified one.
16. Add the tick-counter-driven self-audit trigger per Part 28.2: the pre-commit hook refuses
    further commits once the tick gap since `last_audit_tick` exceeds N, forcing
    `scripts/self-audit.sh` to run rather than depending on anyone remembering it's due.
17. Formalize `STATUS.md`'s schema per Part 28.3 (generated/updated by scripts, never
    hand-narrated) if it doesn't already match — this is what 15 and 16 both read from and
    write to.
18. Do a one-time pass over `JOURNAL.md`'s existing ~19 ticks for recurring lessons (Part 29.1)
    and promote whatever qualifies now — don't wait for the next EPOCH audit to do this
    retroactively once, since real lessons are already sitting unpromoted.
19. Add the Settled-Decisions section to `STATUS.md` (Part 29.2) with at minimum the six
    listed lines — this is a five-minute addition with an outsized effect on preventing
    relitigation drift.
20. Implement the `Error.captureStackTrace`/`Error.prepareStackTrace`/`Error.stackTraceLimit`
    shim (Part 30.3) as JS-environment setup code — cheap, bounded, and closes a real,
    documented compat gap against widely-used libraries.
21. Extend the Framework Exception Miner's scope (Part 30.4) to route non-standard-API
    exceptions through the same pipeline as missing-IDL exceptions, rather than building a
    separate mechanism for this class of gap.
22. Create `INTERACTION-SURFACE.md` (Part 31.1) and wire the pre-commit hook requirement from
    Part 31.3 — commits touching interaction-relevant paths must reference or add an entry.
23. Run the one-time backfill pass over existing journal/gate history for interaction-surface
    knowledge (Part 31.4) — this is real, already-learned knowledge sitting unstructured, same
    priority as item 18's lesson-promotion backfill.

**Tier 2 — sequenced after Tier 0 lands, per Part 21.2 item 4:**
12. Evaluate the Salsa-based incremental computation architecture (Part 19.1) as the replacement
    for hand-rolled invalidation sets — this is the one item on the original list that's
    asymptotically wrong today (O(page) per event), not merely incomplete, so it's a priority
    once infrastructure is in place, but it depends on a deliberate build-vs-adopt decision, not
    a mid-implementation default.