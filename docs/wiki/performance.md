# PERFORMANCE — what is actually slow, and why

> **The rule that governs this whole file:** a speed advantage is only real if it comes from doing the
> same work **better** — not from **not doing the work**. *"Fast because we never loaded the images"* and
> *"fast because we never ran the script"* are two lies already told and caught here. **A speed claim is
> only admissible next to a coverage number**, which is why `crawl-report.sh` prints coverage first and
> has **no flag to print speed alone.**

## The standing position vs Chromium

Measured on the 265-site corpus, one snapshot, our own clock: **faster than Chromium on 195/211 sites
(92%)**, median **16.1s vs 36.5s**. Slower on exactly one (atlassian).

**Chromium is the CEILING on capability and the FLOOR on everything else.** A timing divergence in our
favour is not a bug — it is the point. There is nothing to regress toward.

---
# Backfill — mechanisms recovered from ticks 1–42 (pre-wiki)

## The cascade was 66% of the pipeline, superlinear — and building a `Stylist` does not mean you are USING it

On an **18,658-node** Wikipedia page: parse 18.86ms, **cascade 84.56ms**, layout 17.60ms, display-list
0.73ms, paint 6.22ms — **total 127.97ms**. Per-node cascade cost rose **0.39 → 4.53 µs/node (×11.6)** from
1.3k to 18.7k nodes, because **every element was matched against every rule** — O(nodes × rules), **no
selector index.**

**Twice. And the second time was worse and subtler:** the engine built a full Stylo `Stylist` — with its
bucketed `SelectorMap`, rule hashes and ancestor Bloom filter — **and then never used it for matching**,
borrowing only its `Device`. Wikipedia: **18,631 elements × every rule = 339ms on the UI thread on every
navigation.**

The fix in both cases is what `SelectorMap` does internally: **file each rule under its rightmost simple
selector (id → class → tag → universal)** and consult only the buckets this element can be in, plus the
universal one.

- cascade **84.56 → 31.40ms (2.69×)**, whole pipeline **127.97 → 76.44ms (1.67×)** — cascade **66% → 41%**
- and separately **339 → 199ms**
- **computed styles BIT-IDENTICAL**, box parity unchanged **72/72** — *because the index only skips rules
  that PROVABLY cannot match.* **This is a complexity fix, not a constant-factor one.**

> ⚠ **A false lead worth recording:** implementing `TElement::each_class` — which feeds Stylo's
> class-bucketed lookup and its Bloom filter — **changed the time by NOTHING AT ALL.** *Which was itself the
> finding: the fast path it feeds was never being entered.*

**And this is the same change that silently dropped every nested rule.** *The two facts belong to one
optimisation — an optimisation that makes a data structure smaller must be asked what it DROPPED.*

## `std::env::var()` inside intrinsic sizing cost real time on every page load

`std::env::var("MANUK_TRACE_INTRINSIC")` was called **once per node per probe**. It **takes a process-wide
lock and allocates a `String`.** *A debug hook nobody had enabled was on the hot path of every layout.*
Hoist to a `OnceLock`.

## Taffy's repeated intrinsic-size probing is O(n²) unless memoized per layout

Taffy probes intrinsic sizes **repeatedly** during flex/grid solving, at several available widths, and on
**nested** flex the cost **compounds per level.**

**The signature:** bbc.co.uk had **4,021 nodes and 260ms** of layout while Wikipedia had **18,630 nodes and
127ms** — **4.6× more nodes, HALF the time, ~10× worse per node** — because *Wikipedia is a document that
barely nests and bbc is deeply nested flex.* Caching max-content took bbc to **168ms** and left Wikipedia
**unchanged at 126ms — exactly as predicted.**

> ***An intrinsic is a property of the box, not of the question you asked it.***

## A single navigation ran FOUR full cascades and NINE full layouts

Two of the cascades had **byte-identical inputs**. Every stylesheet, mask and background image was re-fetched
after **every** round of dynamic scripts (`finish_loading` **8,041ms → 38ms** once deduplicated). Against a
332ms pipeline pass, one bbc.co.uk navigation took **17.5 seconds**.

> **A relayout that cannot change its own output is not conservatism; it is waste with a safety story
> attached.** When no new stylesheet arrived and nothing is dirty, **skip it.**

## First paint must not wait for images — the gap between "laid out" and "on screen" was TWELVE SECONDS

nytimes was parsed, cascaded and laid out — **everything needed to paint** — at **1.7s**, and the user saw
it at **14s**. *Twelve seconds of blank window while the article sat there waiting on tracking pixels nobody
was looking at.*

**14,000ms → 5,773ms** (then 42 images in 452ms *after* the page was up); theguardian paints at **6,488ms**
and then takes **8,006ms for 135 images the user is not waiting on.** *The reflow-on-late-arrival this
causes is exactly what an `<img>` without intrinsic dimensions does in a real browser anyway.*

## `defer`/`async`/`type=module` parsed and IGNORED means ~1MB of JavaScript blocks first paint

`Script { defer: bool, is_async: bool }` **fed no scheduling decision.** Every script blocked paint —
**including the ones whose entire purpose is to say "do not wait for me"**, and including **`type="module"`,
which is deferred by DEFAULT in every real browser and is what every Vite/Rollup bundle ships as.**

Measured on nytimes: the render pipeline is **342ms** and barely moves when scripts are stripped (**326ms**),
while the document is **1,433KB with scripts and 447KB without** — so **~1MB of JS was parsed and executed
before a single pixel.**

**The honest residual (5,773 → 5,083ms):** most of nytimes' JS is **classic blocking script**, which a real
browser must *also* run before painting. **Chromium hides that cost by painting INCREMENTALLY AS IT
PARSES** — the document above a blocking script is **already on screen** when it runs. *That is a bigger
change than honouring the attributes.*

## The first-paint checkpoint is "head-complete + render-blocking CSS" — ~113× sooner than full load

DOM construction is incremental, but the initial render is **render-blocking on CSS** (inline `<style>` +
`<link>`) **by design, to prevent FOUC.** So the checkpoint is: once `<head>` and its render-blocking CSS
are processed, **lay out and paint the DOM-so-far at `<body>` start**, then re-layout incrementally per
streamed chunk.

Measured: first paint **13.6 µs vs 1.55 ms** full load — **~113× sooner.** *Browsers additionally run a
**secondary preload scanner** over raw bytes while the main parser blocks on sync scripts/CSS.*

## Gates must measure the marginal cost of an EVENT, not the cost of a LOAD

**A load-time benchmark is structurally blind to per-event cost** — it cheerfully reported **83ms end-to-end
while the browser stuttered on every wheel event.**

- **G_ALLOC** — a counting global allocator around a **single input event** on a large DOM, asserting
  (a) **near-zero allocation when nothing is listening** — *the common case, which must be FREE, not merely
  cheaper* — and (b) that the cost of telling a page it scrolled is **sub-linear in DOM size**.
- **G_INTERACT** — asserts the **SHAPE** as well as the ceiling: **closing the 30th tab must not cost
  meaningfully more than closing the first**, because *a per-operation cost that grows with tab count is the
  real bug, and a fixed ceiling would not notice it until the user had 200 tabs open.*

**Measured with thirty REAL pages open** (an empty `Browser` measures a `Vec` and proves nothing): tab open
**0.94ms**, switch **0.02ms**, close **0.01ms**, scroll **0.01ms**, click **0.27ms** — all far under a
frame. *The only UI-thread cost a person could still feel is the page BUILD on navigation (~100ms on a large
document).*

## A freshness cache for large-scale traversal must key on the CONTENT digest, not raw HTML

**Raw bytes churn on ad tokens, nonces and timestamps while the article is identical** — which **drowns the
change signal.** Key on a digest of the **extracted content** (visible text / a11y rendering).

## Find-in-page is an OVERLAY precisely so it never triggers relayout

Chromium keeps it a browser-UI **overlay** over the renderer's text for exactly this reason. Read the
*existing* text fragments (which already carry absolute rects), match over the **document-order
concatenation of runs** — so a query can span run boundaries — and return highlight rects the compositor
draws **on top**. **Matching never mutates the DOM and never triggers a relayout.**

## Putting build output in RAM does NOT make builds faster

Say the disclaimer first: **rustc/LLVM codegen is CPU-bound**, and careful benchmarks find **near-zero
difference** between tmpfs and SSD. **What tmpfs buys is SSD-WEAR elimination.**

**The sizing insight is that `target/` is not one thing:** dependency rlibs (~10 GB, written **once**, read
forever, **zero churn**) · your own crates (~0.5 GB, rewritten every build) · **`incremental/` (~2.5 GB,
rewritten on EVERY EDIT**, thousands of small files, **and deleted by hygiene passes anyway**).

**`incremental/` is simultaneously the dominant write source and the least valuable output**, so moving
*only* it captures the great majority of the wear for ~2 GB of RAM instead of 22 GB. **Moving everything
would leave too little headroom for parallel LLVM codegen — trading a disk-wear problem for an OOM KILL,
which is strictly worse:** *ENOSPC is a build error; OOM is a machine that stops answering.*

⚠ **One trap:** `rm -rf`-ing the tmpfs root destroys the symlink *target*, and cargo then fails with
`couldn't prepare build directories … File exists (os error 17)` — **an error naming the filesystem and not
the cause.**

## The verify wall is already fast, so build-cycle "optimisations" are theatre

**181s on the worst realistic tick** (touching `engine/css` — the shared-type edit that cascades furthest)
and **57s warm.** So mold/lld, cargo-nextest and workspace-hack are **infrastructure theatre against a
target already satisfied.**

*(If it ever does become the bottleneck, the order is: **mold or lld** → **cargo-nextest** →
**cargo-hakari** → split debuginfo. And **do NOT adopt Cranelift for debug builds** — it does not support
inline assembly and is documented to break on low-level/FFI-heavy code, **which the mozjs/Stylo boundary is
by definition.**)*

## The phase that reports the budget is not the phase that spent it (tick 670)

`finish_loading` has **one budget and seven phases**, and for a long time the only thing it reported
was *which phase noticed the budget was gone*. That is the phase that ran **first after** it was
spent — never, by construction, the one that spent it.

Three consecutive ticks tried to derive the answer for `www.agoda.com` and two got it wrong:

- **t668** sorted the warnings by **count**, saw `pump_page_fetches` named once, and concluded it was
  the culprit. A cluster cannot answer a question about order.
- **t669** sorted them by **timestamp**, saw sixteen drain give-ups *before* the pump's message, and
  concluded the pump *"never ran — it is the victim, not the site."* The evidence was
  `load budget exhausted … round=1`, read as *"the first round"*. The loop is
  `for round in 0..MAX_ROUNDS`: **`round=1` is the second.** Round 0 had already run and burned the
  budget; round 1 reported it.

**An off-by-one in a log message defeats any amount of careful reasoning about ordering.**

### The instrument, and why it is four lines at one site

`phase()` already runs between every phase. It now reports what the **previous** phase cost, in the
two units that matter:

```text
  phase="external CSS"      ms=3020    gave_up=0
  phase="dynamic scripts"   ms=0       gave_up=0
  phase="subframes"         ms=0       gave_up=0     <- t669's "leading candidate"
  phase="page fetches"      ms=36061   gave_up=15    <- 36 of the 39 seconds, and every give-up
```

Emitted at `info`, so `RUST_LOG=manuk_page=info` **is** the instrument — no rebuild to ask again,
which is the property three ticks kept wishing for while re-deriving instead.

**The ledger closes with a sentinel** so the last phase reports itself and the per-phase times sum to
`elapsed_ms`. A set of parts that does not sum to its whole is this project's highest-yield instrument
(*8 of 30 process defects were caught by a number that did not add up, not by any gate*), and it
should not be possible to read this ledger without noticing a gap.

### What it found, which none of the three guesses had

Fifteen give-ups occur inside **one** round, so they are per **settled fetch**, not per round: the
pump settles a batch, each settle runs the page's JS, and the drain gives up again. Tick 667's
between-**rounds** bound cannot reach that — which is precisely why t668's re-measurement of agoda
showed no movement at all. **That result was correct and its explanation was wrong for two ticks.**

> **Make the thing being argued about report itself.** Both failure modes above are how a *derived*
> answer fails, and the cure for both is the same instrument — which was four lines, at a site that
> already existed, and would have been cheaper than either wrong tick.

## The loop that spends the budget must be the loop that checks it (tick 671)

`pump_page_fetches` checks the load budget **before each round** and **after** it. In between, it
settles that round's results:

```rust
for (id, status, body, headers) in results {
    self.resolve_fetch(id, status, &body, &headers, fonts, viewport_width);   // <- runs page JS
}
```

**Settling one result runs the page's own promise continuation, which drains the event loop.** A page
that is not converging therefore pays a full drain ceiling *per settled request*, up to
`MAX_PER_ROUND` (40) of them — and the per-round check sees none of it. **The budget was consulted
everywhere except the part that spends it.**

```text
  www.agoda.com, 12s budget      before                       after
  phase="page fetches"           ms=36061  gave_up=15   ->   ms=13190  gave_up=5
  finish_loading                          39.9s         ->            15.4s
  TOTAL                                   43.6s         ->            19.0s
```

**It costs nothing the budget was not already discarding.** Past the deadline `finish_loading` skips
images, masks and background images outright, so continuing to settle bought the page nothing and cost
it those three phases. And the outer `tokio::time::timeout` could never have done this job: a timeout
fires at an await point, and these drains are synchronous JavaScript.

### The arc, because its shape is the lesson

```text
  t666  measured 39.9s vs a 12s budget, at the page level          — right
  t667  bounded the dynamic-script ROUND loop, gated + RED-proved  — right, and not the site
  t668  re-measured agoda: no movement. Blamed pump_page_fetches   — right phase, from a COUNT
  t669  timeline said the pump "never ran"                         — WRONG: off-by-one on `round=1`
  t670  built the per-phase ledger                                 — four lines ended the argument
  t671  the settle loop, located to the line                       — 43.6s -> 19.0s
```

Two wrong inferences and one retraction, ended not by reasoning harder but by **making the subject
report itself**. The instrument was four lines at a site that already existed and was cheaper than
either wrong tick. *When two consecutive attempts to derive an answer disagree, stop deriving and
instrument.*

### A gate whose fixture could not fail

The first version used twelve settles: 1.9s with the check against 3.4s without. **No ceiling loose
enough to avoid flaking on a busy machine can straddle that**, so the gate passed either way. The
fixture was made **harder** (thirty settles → 1.90s vs 7.99s), not the threshold tighter — *vary the
mechanism, not the threshold*. This is why the RED-proof runs before a tick lands: it is the step that
tells a gate from a decoration, and here it caught one on the first try.

## The phase ledger did not sum to the load, and said it did (tick 678)

Tick 670's per-phase ledger was built on an explicit principle, quoted from its own source:

> *"the per-phase times sum to the total. A set of parts that does not sum to its whole is the
> accounting reconciliation this project rates as its highest-yield instrument."*

Measured on `playhop.com`, it accounted for **12.2s of a 27.6s load**. 56% of the navigation was
outside the ledger — because the ledger only ever covered `finish_loading`, while a navigation also
spends `load_async`.

### The ledger now spans the navigation, and it CLOSES WITH THE SUBTRACTION PRINTED

Same event shape (`load phase done`, `phase`/`ms`/`gave_up`), so `RUST_LOG=manuk_page=info` prints one
continuous ledger and every existing grep keeps working. The new closing line does the arithmetic
itself:

```text
navigation phases reconciled  total_ms=12186 accounted_ms=12182 unaccounted_ms=4
```

An instrument that leaves the subtraction to the reader is one whose parts *claim* to sum. `G_LOAD`
now asserts `unaccounted_ms * 10 <= total_ms + 500` on that line, RED-proven by removing a single
`nav_phase` mark: 1602ms of a 1786ms navigation goes unaccounted and the gate fails. A phase added to
`load_async` in future without a mark makes it red, which is the only thing that keeps an accounting
instrument honest as the code it accounts for changes.

### What the complete ledger then said about `playhop.com`

```text
load_async                       12186 ms   (accounted 12182, unaccounted 4)
  html parse                        11
  external scripts                2327
  module graph prefetch              0
  cascade+layout+blocking scripts 2106
  deferred scripts                 910
  DOMContentLoaded                   3
  subframes (pre-load)               0
  load event                      1330   gave_up=1
  initial images+masks            5495   <- the largest single phase
finish_loading                   13647 ms
  external CSS                     655
  dynamic scripts                 7697   gave_up=3
  subframes                          0
  page fetches                    5294   gave_up=2
                                 -----
                                 25833 ms of the 31626 ms the sweep measured
```

⚠⚠ **THE 12-SECOND PAGE BUDGET IS PER-CALL, NOT PER-NAVIGATION.** `load_async` and `finish_loading`
each call `load_budget()` and each start their own clock, and **both run the enhancement phases**. A
caller that does the documented pair — which is what the fidelity sweep and the shell both do — gets
**two independent 12s deadlines**, and pays for the image phase twice (5.5s in `load_async`, again
inside `finish_loading`'s `page fetches`). The budget's docstring promises *"the phases run under one
overall deadline… this is what a browser actually promises"*; measured, the navigation was busy for
25.8s under a stated 12s ceiling. That is the Bar 0 promise — *how long may a tab be busy* — and it is
a scope bug, not a tuning one.

⚠ **RESIDUE, named:** 5.8s of the sweep's 31.6s is still outside both ledgers, in the CALLER — the
document fetch (`fetch_html`), `paint`, and the PNG encode. Those belong to whoever drives the page,
not to the page, and the honest place to account for them is the caller. Stated so the next reader
does not have to re-derive the difference.

[[loop-optimization-mandate]] [[conformance-and-oracles]]

## The hang guard was firing on our own clock, and it blamed the page (tick 680)

`MAX_TASKS_PER_DRAIN` fired on real sites and said:

> *"the page is not converging (a self-rescheduling timer, **most likely**)"*

`most likely` is the tell. The engine was holding the entire pending task list and **guessing about its
contents** — a status, not a finding, and the third instance of that shape in four ticks (an anonymous
`TypeError` at t666/t675, a source called `inline.js` at t679, this).

### The instrument: name who is spinning

At the ceiling — **once per give-up, never in the steady state** — group the tasks still queued by the
source text of the page's own callback and report the top three with counts, the number due at the
current virtual instant, the delay of the soonest, and the virtual clock. Reading the *residue* rather
than counting as we go keeps the hot path at two comparisons and a splice.

One thing had to change for this to mean anything: the task the loop *runs* is `setTimeout`'s wrapper,
so grouping by it yields **a histogram of ourselves** — eight identical words, once per line. The
page's own callback is now carried on the task (`__enqueue`'s `u`), one property per task, and that is
the difference between a report and a mirror.

### What it said about `playhop.com`, first run

```text
task ceiling  count=20000  spinning=queued=1 due_now=0
              next_in_ms=86400000  vclock_ms=13822876800000
              1x function(){a.fa=Db()}
```

- **One** task queued, and **not due**. Nothing was spinning.
- `next_in_ms=86400000` — the page armed a **24-HOUR** timer. A midnight rollover, a daily reset, a
  cache expiry. Utterly ordinary.
- `vclock_ms=13822876800000` — the virtual clock had advanced **438 years**.

`__fireLoad` set `__timeBudget = Infinity`, so the clock could jump forward without bound. The loop
jumped a full day per iteration and ran that one timer **20,000 times — 54 virtual years per drain** —
burning ~1.5–2s of real CPU each time and tripping the **Bar 0 hang guard on a page that had
converged**. The page was innocent. A real browser fires a 24-hour timer **zero** times during a load,
because its clock advances at 1×.

### The fix: a horizon, which is what Chrome's `--virtual-time-budget` already is

`__timeBudget = __now + 60000`. A task due beyond the horizon does not run during load.

**The horizon may not be small, and that constraint decides the number:** `testharness.js` arms a
**10-second** harness timeout at setup, and a clock that cannot reach it makes every async WPT file
report TIMEOUT — the exact catastrophe `Infinity` was introduced to fix. 60s clears it by 6× and is
still 1,440× short of one day. `G_LIFECYCLE`'s own report is armed at 5000ms, so that gate asserts both
sides of the horizon with one fixture: a far-future task must be refused, and a merely-late one must
still run.

### Measured on `playhop.com`, before → after

```text
load event        1330ms  gave_up=1   ->   154ms  gave_up=0     (8.6x)
dynamic scripts   7697ms  gave_up=3   ->  6048ms  gave_up=0
page fetches      5294ms  gave_up=2   ->  5250ms  gave_up=0
TOTAL             31626ms             -> 27273ms
drain give-ups         6              ->      0
structural coverage  4.7%             ->    6.5%   (100 missing, was 102)
```

**Six Bar 0 hang-guard trips became zero on a page that had converged all along**, 4.4s faster, and two
more elements render. ⚠ Honestly: the site is **still unscored** — `shape_n` went 5 → 7, under the
10-sample floor either way — and its `SHAPE` reading moved 20.0% → 14.3% over a different (larger)
population. Neither number is a scored certificate term, and quoting the drop as a regression or the
rise as a win would be reading noise off a sample of seven.

⚠ **The first version of the gate could not fail.** It read a flag from inside the 5000ms report, and
tasks run in `(due, seq)` order — so the report *always* precedes an 86,400,000ms timer whether the
clock is bounded or not. It was testing ordering, and it passed with `Infinity` restored. The
observation has to happen **after** the drain: the far-future task writes to the DOM and Rust reads it.

[[loop-optimization-mandate]] [[reliability-doctrine]]

## The timeout cohort is our clock, not a regression — and it is now the largest unscored reason

The t886 corpus sweep read `timeout-150s` on **22 of 200** sites, against **3** at t875 and **2** at
t867, and dropped `scored` from 106 to 92. Three engine ticks had landed in that window, so the
attribution mattered more than the number.

**Re-measured SOLO on the current binary, quiet box (load average 1.31):**

```text
                      manuk      chromium     shape
  sip777man.site    191,825ms     11,460ms    90.5%   ← over the 150s deadline SOLO
  beb88run.xyz      172,591ms     14,114ms    81.6%   ← over it SOLO
  www.ikea.com       50,717ms     13,891ms    69.5%
  payb.jp           102,352ms     37,528ms    64.7%
```

Every one renders, and renders *well* — `sip777man` at 0.942 is one of the best shape scores in the
corpus. **We are 4–17× slower than Chromium**, sitting either side of a 150-second deadline, so a
slightly busier box flips a scored row into a timeout.

**The old-binary control settles it.** `engine/` checked out at t881 (before all three engine ticks),
rebuilt, same hour, same sites: ikea 50,717 → 46,126 ms, payb 102,352 → **105,074 ms** — the old
binary is *slower*. Not a regression.

### Why that is still the top finding and not an excuse

*"Not a regression"* is not *"not a problem."* Bar 0 counts a site over **30 seconds on our clock**,
and the sampled sites are at 50–192. The scorability ceiling is **92/132 = 69.7%**, and **22 of the 40
unscored rows are our latency** — more than every other reason combined (shell-only 8 · other 4 ·
thin-overlap 3 · render-fail 2 · css-starved 1). The board has ranked throw-killers since t777 off a
worklist where `timeout` was 3. It is now the largest single cohort by a factor of three, and closing
it is a **performance** tick, not a function one.

### The methodological residue

**Nothing in this loop records box load per sweep**, so t886 and t875 are not load-matched and the
`106 → 92` difference cannot be cleanly attributed to either engine or environment. The
composition-free comparator — mean Δshape over the 89 sites scored in BOTH sweeps — reads **−0.15
points** (7 up, 8 down by >2pt): flat. When a cohort sits *at* a deadline, the deadline is part of the
measurement, and the sweep should record the load it ran under.

## The `timeout-150s` bucket is FORCED REFLOW, and `reflow_n` splits it into two different bugs (t1236)

The drain's time budget is **exact between tasks** and blown by a *single* task. The drain already
arms `ScriptDeadline`, so the SCRIPT half of such a task is preemptible — `JS_RequestInterruptCallback`
is polled at interpreter back-edges. **There are none while the thread is inside a Rust binding**, so
the residue is native work the script triggered, and nothing could say which.

`dom_bindings::REFLOW_COST` times the inside of `force_reflow_if_stale` — the one funnel every
geometry read goes through — and the drain reports its own delta beside `elapsed_ms`:

```text
  site                 count  elapsed_ms  reflow_n  reflow_ms   reflow share
  7info.ru                 1        9324        18       9259     99.3%
  7info.ru               150        5751        22       5722     99.5%
  7info.ru               114        6616       545       5711     86.3%
  www.friulioggi.it       29        7801       233       7666     98.3%
  bhramarah.in           176       22419         1      21302     95.0%
  bhramarah.in             2       22428         1      21879     97.6%
```

**95–99% of every budget overrun is one function.** The bucket is not JavaScript, not network and not
script preemption. It is layout.

### `reflow_n` is the discriminator, and it names TWO bugs

| shape | evidence | what fixes it |
|---|---|---|
| **COST** — one pass is pathological | `bhramarah.in` `reflow_n=1`, `reflow_ms=21302` — **a single cascade+layout pass takes 21 seconds** | a specific defect in one document's layout; no frequency argument touches it |
| **FREQUENCY×COST** — many passes, each ordinary | `www.friulioggi.it` 233 × 33ms · `7info.ru` 18 × 514ms · 545 × 10ms | incremental invalidation — today a read after a one-node mutation re-cascades and re-lays-out the **whole document** |

They are not interchangeable, and a wall-clock number alone cannot tell them apart — which is the
reason this counter reports a **count** next to a duration rather than just a duration.

⚠ **Not established:** *why* one pass costs 21 s (the next profile, one level down — this counter
cannot see inside itself), and whether it is cascade or layout. And it covers **three of the eight**
timeout sites: `neutypechic.com` and `payb.jp` produced no budget-trip line at all, so their time is
somewhere else again.

### ⚠ An accounting counter that under-reports looks like GOOD NEWS

The first cut had the drain **reset** the counter on entry. Wrong twice: **drains nest**, so an inner
drain zeroed the outer one's accounting; and the reset made the counter unobservable from outside a
drain, so `G_REFLOW_ACCOUNTING` read a hard **0** for a fixture that visibly forces reflow. A zero
here does not read as "broken instrument", it reads as *"reflow is not the problem"* — and would have
aimed the next tick at the wrong subsystem with a number behind it. **It is monotonic now**, and each
drain subtracts its own entry snapshot. The gate carries a CONTROL arm (identical DOM work, no
geometry read, must report zero) because a counter incremented in the wrong place is also non-zero
after a page loads.

⚠⚠⚠ **STRUCK (t1242) — this was FALSE, and the instrument invented it.** The original text read:
*"a parse-time inline `<script>`'s geometry reads report ZERO forced reflows … a
`measure → mutate → measure` loop in a parse-time script is reading a stale snapshot. Candidate
defect."* Measured with a four-line fixture: the geometry is **fresh** (`before:0 after:100`) and the
accounting **sees** the reflow. The zero was read **while the counter was still being RESET by each
drain** — the very bug this tick went on to fix — and the parse-time case was never re-checked after
the counter was made monotonic.

> **A reading taken with an instrument you then repair does not survive the repair, and it will not
> retract itself.** When a measurement tool changes, every observation already banked from it is a
> hypothesis again.

Pinned by `G_PARSE_TIME_FORCED_REFLOW`. ⚠ This strikes **one** round off t1183-1188's *"ReflowScope
missing from 2 of 19 rounds"* — that residue still stands for the rounds this fixture does not
exercise. And it confirms `reflow_ms` has **no blind spot** on the parse-time path, so the
"95-99% of every drain overrun is forced reflow" attribution does not need re-pricing.

## …and the reflow is TWO cascades: the container-query pass is 51% of it (t1237)

t1236 left "why does one forced reflow cost 21 seconds" open. Two threshold-gated log lines answer it.

⚠ **The obvious suspect was wrong, and by three orders of magnitude.** `forced_reflow` calls
`sheets_of`, which re-parses the stylesheet set *from source* on every reflow — on a 51-sheet document
that reads like the whole story. **It is 43 ms of 21,220.** Caching it would have been a correct
optimisation worth 0.2%, shipped with a claim that the bucket was addressed.

**Level 1 — `forced_reflow` on `bhramarah.in`:**

```text
  total_ms=21220  sheets_ms=43  layout_ms=21169  rects_ms=7  publish_ms=0  n_sheets=51 nodes=23013
```

**Level 2 — inside `restyle_and_layout`:**

```text
  cascade_ms  layout_ms  container_query_ms  cq_relaid  n_sheets  nodes
        1261        519                   0      false        42  22987
        4155       1869                6421       true        47  22997
        8437       1942               10981       true        51  23013
        8179       1863               10714       true        52  23022
```

| term | share | what it is |
|---|---|---|
| `container_query_recascade` | **51%** | a **second full cascade + a second full layout** |
| `cascade_styles` | **40%** | the first cascade — **superlinear in sheet count** |
| `layout_document` | 9% | actual layout |

**Layout is not the problem inside layout. Cascade is, and it runs twice.**

### `cq_relaid` is a SWITCH, not a gradient

At 42 sheets the container-query pass costs **0 ms and returns false**. At 47 it returns **true**,
costs 6,421 ms, and returns true on every forced reflow thereafter. **One arriving stylesheet with a
container query turns every subsequent geometry read from one cascade into two, permanently** — on a
document whose node count moved 22,987 → 23,022 across those rows, a tenth of a percent.

### The cascade is superlinear in SHEET count

Sheets **42 → 51 (+21%)** takes `cascade_ms` **1,261 → 8,437 (+569%)**, with node count flat.
Whatever `cascade_styles` does per sheet is not additive. ⚠ **Why is NOT established** — the
candidates are a per-call rebuild of the structure the cascade matches against (a stylist built per
call rather than per sheet-set change) and per-sheet re-walks of the document, and they are
distinguishable by one more level of the same instrument. Measure before building: on this path the
obvious suspect was 0.2%.

### The completed chain

```text
  timeout-150s bucket (the M1 scorability cap)
    └─ 95-99% of every drain budget overrun is FORCED REFLOW           (t1236)
        └─ 99.8% of a forced reflow is restyle_and_layout              (t1237)
            ├─ 51%  container_query_recascade = cascade #2 + layout #2
            ├─ 40%  cascade_styles          ← superlinear in n_sheets
            └─  9%  layout_document
```

⚠ **This corrects t1236's ranking.** That tick called `bhramarah.in`'s single 21-second pass a layout
*defect* and the many-small-reflows sites a separate *design* problem. Right about priority, wrong
about the organ: **both named defects are in the CASCADE**, and the frequency×cost sites run the same
two-cascade path — so a fix to either moves both shapes. They were never two problems.

## …and inside ONE cascade, 43% is PSEUDO-ELEMENT MATCHING (t1238)

The instrument for this level already existed — `MANUK_CASCADE_PROFILE=1`, added during the
`PseudoIndex` work. `bhramarah.in`, one cascade, 23,001 elements, 8,208 ms:

```text
  pseudo_ms      3531.6   43.0%   <- LARGEST
  element_ms     1705.5   20.8%
  computed_ms    1481.0   18.0%
  minimal_ms      978.7   11.9%   <- MinimalCascade, running INSIDE the Stylo cascade
  has_ms          288.7    3.5%
  unattributed    211.7    2.6%
```

**Pseudo matching costs twice what matching every ordinary selector costs** — 3,531 ms against
1,705 ms over the same elements — to serve the handful that actually have a `::before`/`::after`.

⚠⚠⚠ **The fix already exists one layer up, and it only fixed half the bug.** `PseudoIndex`'s own doc
comment records that `cascade_one_element` was O(elements × rules), that **bucketing** fixed it, and
that *"the pseudo-element path never got that fix"* — so `PseudoIndex` hoisted the pseudo rule
**collection** out of the per-element loop (measured then at 9.0 s of a 19.5 s cascade on wix.com).
That hoist worked. **Matching is still a linear scan of every collected pseudo rule, per element**,
because `RuleIndex` buckets by tag/class/id and `PseudoIndex` has no such structure.

> **A fix aimed at a measured phase fixes THAT PHASE, and the sibling work in the same function keeps
> the old shape.** The profiler said "collection", collection was hoisted, the number fell — and the
> *matching* half, never separately measured, kept the very algorithm the hoist was a response to.

⚠ **Second, independent: `minimal_ms` 978 ms is `MinimalCascade` running INSIDE
`cascade_via_stylo_sized`** — a whole second cascade engine over the same DOM and sheets, on a path
that already runs twice per geometry read on any container-query page. Whether it is load-bearing
(a recovery/merge input) or vestigial is **not established**; that is a read of `cascade_element`.

### The completed chain, four levels

```text
  timeout-150s bucket — the M1 SCORABILITY CAP
   └─ 95-99%  FORCED REFLOW                                      (t1236)
       └─ 99.8%  restyle_and_layout                              (t1237)
           ├─ 51%  container_query_recascade = cascade #2 + layout #2
           ├─ 40%  cascade_styles       ← superlinear in n_sheets
           └─  9%  layout_document
               └─ inside ONE cascade:                            (t1238)
                   ├─ 43%  pseudo matching  ← O(elements × pseudo rules), UNBUCKETED
                   ├─ 21%  element matching ← bucketed, 2x cheaper for the same elements
                   ├─ 18%  computed-value conversion
                   └─ 12%  MinimalCascade, redundant inside the Stylo cascade
```

**Next: bucket `PseudoIndex` the way `RuleIndex` is bucketed** — same file, same pattern, an existing
correct implementation to copy, and the profiler already in place to prove it. ⚠ Gate the
**behaviour** (`::before`/`::after` content and specificity across a fixture with many elements and
few pseudo rules), not the timing: a bucketing bug drops a rule silently, which is worse than slow.

## Bucketing the pseudo index: a 14× narrowing that bought nothing (t1239)

t1238 specified "bucket `PseudoIndex` the way `RuleIndex` is bucketed". Done, through **one shared
`bucket_key_of`** rather than a second copy — the pseudo path was linear for 1,238 ticks precisely
because bucketing was written once, for ordinary selectors, and the sibling structure in the same
file never got it.

### ⚠ The first version narrowed NOTHING, and only a fan-out counter could say so

With the key computed exactly as `RuleIndex` computes it, `pseudo_ms` did not move (3,531 → 3,537):

```text
  FANOUT all=28 picked=28 univ=28     <- EVERY pseudo rule was in the UNIVERSAL bucket
```

**A pseudo-element is its own compound, and it is the rightmost one.** `.icon::before` reads, right
to left, `[PseudoElement(Before)]` · the `PseudoElement` combinator · `[Class(icon)]`. `sel.iter()`
stops at the first combinator, so it yields **only the pseudo** — every rule keys to `None` and the
index collapses to one universal bucket. The subject compound is one `next_sequence()` further left.

```text
  before:  all=28 picked=28   all=22 picked=22   all=34 picked=34
  after:   all=28 picked=2    all=22 picked=0    all=34 picked=8
```

### …and `pseudo_ms` still does not move: 3,531 → 3,560 ms

A fourteen-fold reduction in matching work changed nothing, which is a **measurement**: whatever
`cascade_pseudo` spends its 43% on, **it is not selector matching**. The remaining suspect is the
per-element tail — `merge_ascending` + `ServoArc::new` + `stylist.compute_for_declarations` — which
runs for every element with *any* matching pseudo rule, and a `*::before` in the universal bucket
makes that every element on the page. **Narrowing candidates cannot help a cost paid once per element
regardless of how many candidates there were.**

> **A fix that works and moves nothing has told you where the cost ISN'T — and that is worth having,
> but only if it is reported as that.** "Bucketed the pseudo index, 14× fewer rules tested" is true,
> sounds like a win, and would leave the next tick believing the 43% was addressed.

It is landed rather than reverted because it *works* (unlike t1197's inert callback, which was
reverted for not firing at all): a strict algorithmic improvement, behaviourally gated, costing
nothing, removing a known `O(elements × rules)` shape before the workload that punishes it arrives.

**Next: instrument the per-element tail of `cascade_pseudo`.** The fan-out counter is the template —
prove where the cost is before building anything.

## `to_computed_style` is 53% of the cascade, and the `content` test was two statements too late (t1240)

t1239 ruled out selector matching, leaving `cascade_pseudo`'s per-element tail. Split three ways:

```text
  pseudo_ms=3542   pseudo_tail_n=18364   merge=115ms   compute=260ms   to_computed_style=2886ms
```

⚠⚠⚠ **`to_computed_style` is 81% of the pseudo phase, and it is OUR code, not Stylo's.** Stylo's own
computed-value pipeline (`compute_for_declarations`) is **260 ms**; converting its `ComputedValues`
into our `ComputedStyle` is **2,886 ms**. The same function is another 1,483 ms on the ordinary-element
path — **one marshalling function is 53% of the entire 8,196 ms cascade.** Four ticks of attribution
walked toward Stylo and arrived at the boundary layer we wrote.

⚠⚠⚠ **`pseudo_tail_n=18364` is the bug as a number.** 18,364 elements had a full `ComputedStyle`
built for a `::before`. **Seven generate content.** One `*::before` anywhere in the page's CSS puts a
matching rule in the universal bucket for every element, and each paid a ~200-field conversion whose
result a `content` test discarded **two statements later**.

**The fix is that reorder.** `content` is read straight off Stylo's `cv` and never needed the
conversion. Semantics are provably identical — the old fallthrough was already `_ => return None` for
`Content::Normal`/`Content::None`. ⚠ `::first-letter` is excluded and keeps its early return *below*
the conversion: it re-styles text the author already wrote, so requiring `content` would drop
`p:first-letter { font-size: 200% }` — every real `::first-letter` rule on the web.

```text
                            before      after
  pseudo_tail_n              18364          7
  pseudo_ms                   3542        436      -88%
  one cascade  total_ms       8196       5163      -37%
  SLOW FORCED REFLOW        21,220     14,366      -32%
```

**The first fix in this chain that moves the real site** — said with that qualification because the
two before it did not (t1234's byte reduction moved zero of eight sites; t1239's 14× candidate
narrowing moved nothing). ⚠ `bhramarah.in` still exceeds the 150 s cap. The remaining terms are now
dominant: `element_ms` 1,665 · `computed_ms` 1,503 (**the same `to_computed_style`**, ordinary path,
untouched) · `minimal_ms` 983 (a second cascade engine inside the first).

⚠ **Next, already named by the measurement:** `to_computed_style` is 29% of what remains and **nothing
about it is conditional**, so no reorder helps. It is a ~200-field eager conversion of every element's
style, per cascade, and the cascade runs twice per geometry read on a container-query page. The shape
of the answer is to stop converting eagerly — share the `Arc<ComputedValues>` and convert on demand —
and that is a **subsystem, not a tick**. Price it before starting.

## The second cascade engine cannot be deleted — but it can be narrowed (t1241)

`MinimalCascade` runs **inside** `cascade_via_stylo_sized`, over the same DOM and the same sheets:
983 ms of every cascade, 19% of what remains after t1240. The temptation to delete it is obvious and
**it is load-bearing**, with three distinct consumers:

| where | what it supplies | why Stylo cannot |
|---|---|---|
| in-walk | `field_sizing_content`, `appearance_none` | must land **before** the presentational hints — their job is to *veto* a UA hint |
| recover | `vertical_align`, `visibility`, `mask_image`, `background_image`, `text_decoration`, `list_style` | Stylo's **servo** build does not expose them |
| fallback | `map.entry(node).or_insert(minimal)` | **shadow DOM** — Stylo's walk has no tree-scoped matching, so shadow content has no other cascade |

`visibility` alone is not optional, and the reason is written at the site: the modern web hides
dropdowns, modals and tooltips with `visibility:hidden` (animatable, unlike `display:none`), and
without it **every one of them paints on top of the page**.

⚠ **What is available** — and the code says so itself: *"Could later be narrowed to a
vertical-align-only scan to avoid the second cascade."* `MinimalCascade` computes a **full
`ComputedStyle` for every node to recover eight fields.** A property-subset pass keeps every consumer
above and drops most of the 983 ms. That is the next bounded lever, and it is **not** a delete.

> ⚠ Third time in six ticks that the fix which *looked* available was the wrong one — `sheets_of`
> (0.2% of a forced reflow), pseudo bucketing (14× narrower, inert), and now this. **The expensive
> thing and the removable thing keep turning out to be different objects.**
