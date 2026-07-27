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
