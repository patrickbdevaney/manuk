# CONFORMANCE AND ORACLES — how we MEASURE, and what each instrument cannot see

## The differential oracle (265 real sites vs Chromium) has two structural blind spots

1. **It can only see what those sites happen to exercise.** A DOM method no site in the corpus calls
   is, to the oracle, **correct by default**.
2. **It needs Chromium to say what "right" is** — so every answer is a *diff*, and a diff cannot tell
   you whether **both** engines are wrong, or whether we are wrong in a way that happens not to move a
   box.

It is also **static and single-snapshot**: it has never observed time, interaction, scroll, session,
media, adversarial input, or network reality. **Null is not zero** — a category with no data is a
category nobody has looked at.

## WPT has neither blind spot, because the tests carry their own verdict

`assert_equals` either holds or it does not. **No oracle is required at all.** It is the difference
between *"we render this page differently from Chrome"* and *"`Node.prototype.after()` is specified to
do X and we do Y."*

### Integration mechanics (these are the sanctioned hooks, not workarounds)

- **`resources/testharnessreport.js` is the vendor hook.** WPT's own copy says so in its header:
  *"intended for vendors to implement code needed to integrate testharness.js tests with their own test
  systems."* We serve our own in its place; it registers `add_completion_callback` and writes results
  into the DOM as JSON, which the Rust side reads back with `querySelector`. Overriding it **in the
  server** rather than by writing into the checkout keeps the corpus pristine — *a runner that mutates
  its own corpus is a runner whose corpus you cannot trust.*
- **`setup({ output: false })` is required.** testharness's HTML results renderer is *page code*; any
  DOM gap it trips over throws **inside `notify_complete`**, aborting the completion-callback loop, so
  the file reports **nothing**. wptrunner passes `output: false` for the same reason.
- **Serve over real HTTP, never `file://`.** `file://` is an **opaque origin**, so every storage/origin
  test would fail *because of the harness* and be recorded as an engine failure. This project has
  already been burned by exactly that: a `file://` harness bug left *"React renders nothing"* in the
  ledger for ticks as a **React** problem.
- **`.any.js`/`.window.js` need wptserve to generate their wrappers** (~2.5% of tests). Skipped —
  **and counted, with the reason printed.** *A runner that silently drops what it cannot run is
  reporting a pass rate for a suite it did not run.*

### A hang can only be contained by a PROCESS boundary

`tokio::time::timeout` **cannot interrupt synchronous JavaScript**. A test that spins inside
SpiderMonkey never yields, so the timeout future never runs and the whole suite wedges. The runner
therefore forks a **child process per batch**; the child appends one flushed JSON line per finished
test, so when the driver kills a stalled child, **the test after the last flushed line is the one that
hung** — named, recorded, and stepped over.

> This is the same conclusion the tab process model reached (`docs/loop/PROCESS-MODEL.md`), arrived at
> independently and for the same reason: **only an OS process boundary contains a spinning C++ JIT
> frame.**

## Guard every instrument against measuring ITSELF

The runner prints a warning when >25% of files report nothing:

> *"Above ~25% this number is not measuring the engine's conformance — it is measuring whether
> testharness.js can RUN here at all."*

**It fired on the very first run (100%), and it was right.** Without that guard the honest reading of
"0%" would have been "our DOM is catastrophically broken" instead of "we never defined
`window.parent`".

**Corollary, learned three times now:** a verdict from a new instrument is a **claim**, and claims get
verified before they are believed. The first `cold-read.sh` run reported "tick 42 has no hypothesis"
about a journal entry that plainly had one — `awk` has no `\b` word-boundary escape, so the pattern
matched nothing. **The auditor was wrong, not the file.**

## THREE DIFFERENT FINDINGS MUST NEVER SHARE A NAME

The WPT runner called all of these `TIMEOUT`:

- **our** budget expiring (a *perf* finding),
- **testharness's** own status-2 verdict — an `async_test` that never completed (a *conformance* finding),
- a driver-killed **hang** (a *Bar 0* finding).

So a baseline reported **"90 Bar 0 hangs"** when the real number was **one**. The engine was fine; the
*word* was overloaded. Four columns now: `HANG`, `CRASH`, `SLOW`, `TH_TIMEOUT`.

> **The general rule: an instrument that collapses distinct findings into one label is not a coarse
> instrument — it is a WRONG one**, because the label is what gets acted on.

## A runner must account for the child that DIED, not just the one that hung

When a batch child *crashed* (rather than hanging), the driver advanced past the whole batch — **33 of
457 files silently vanished**, and the pass rate was computed over the remainder with nothing to say so.
Fixing it made **5 real crashes visible** that had been invisible from the start.

**A crash is a finding, not an accident.** Both a hang and a crash must name the test they died on and
step over it.

---
# Backfill — mechanisms recovered from ticks 1–42 (pre-wiki)

## ONE SNAPSHOT, BOTH ENGINES — and never diff against a DEGRADED oracle

**Fetching the page separately per engine compares two DIFFERENT documents and calls the difference a
bug.** The live origin injects a fundraising banner that a saved copy does not — and a metric stayed pinned
at **exactly 5,122px across four genuinely-correct fixes** because of it.

Equally: **the oracle's health check must ask what Chromium actually DREW** (elements drawn, visible text
length), **not how many elements carried an id** — otherwise a **bot wall is scored as YOUR rendering
bug.**

**Both hazards are made impossible IN CODE, rather than left as things to remember.**

## Probing only `[id]` elements makes the oracle nearly BLIND

**Most of the web does not put ids on things.** `text.npr.org` reported **ONE** probed element — and across
a 265-site corpus the oracle was about to report *"no divergences"* **with total confidence.**

Keying on a **structural path** (`div[0]/main[0]/p[3]`), computed identically by both engines from the same
snapshot, took npr **1 → 75** probed elements, lite.cnn → **226**, the Rust book → **540**.

⚠ **Getting the two path functions to agree is a precondition for the diff meaning anything.** **Chromium's
walk stops at `parentElement === null`, so `<html>` contributes NO component** — emitting `html[0]` shifts
every key by one level and reports **`<html>` and `<body>` as MISSING on every site.**

## THE SCORE GATES; THE EYEBALL DIAGNOSES — a pixel score is a poor proxy for correctness

Recorded **four separate times in one arc**:

- Wikipedia scored **75%** while being **visibly, structurally broken**.
- A massive structural repair (hidden dropdowns no longer painting over the article) moved it **81.0% →
  81.7%**.
- Restoring an **entire missing TOC sidebar** moved it **81.7% → 80.7%** — *it went DOWN.*
- **An entirely absent sidebar moved the visual score by less than ONE POINT.**

**The honest metric is COVERAGE**: *of everything Chromium renders, what fraction do we render **at all**?*
**A missing region cannot hide in COVERAGE.** Placement drift is reported **separately**, because on real
pages it is dominated by **font-metric differences** — a fidelity concern, not a correctness one.

**Corollary:** a coverage number can be **100% while the page is wrong** — coverage said every element was
present on rust-lang.org while the page was **printing its own JavaScript source down the left margin.**
*That is what a second bar is for.*

## Cluster by ROOT CAUSE, not by tag name — and rank by DISTINCT SITES

Naive differential diffing runs at **90%+ false positives** before clustering. A clusters file whose top
entries are `geometry: <div>` / `<a>` / `<body>` is **a ranking by tag name** — a restatement of *"the
oracle found divergences"* — and **cannot be worked on.**

Cluster by: **(1)** first-divergence signature, **(2)** computed-`display`-mismatch class, **(3)** the CSS
property/selector implicated. **Then rank by how many DISTINCT SITES each cluster explains** — one site with
500 `<div>`s must not outvote 200 sites with one `<iframe>`.

> **A cluster IS a website class, so the cluster registry IS the taxonomy** — empirically derived rather
> than hand-enumerated. **Crashes and hangs are a third category and outrank every visual cluster.**

**And never score timing.** A first-pass report that lumped all divergence kinds together printed
*"structural agreement: 2.8%"* for a browser rendering fine — because **`geometry` (123,796 nodes, 70% of
the total) means the node EXISTS, at the same SIZE, in a different place.** The real Bar 1 number was
**92.2%**.

## Gates must run the SHIPPING configuration

The parity harness **defaulted to the simple cascade while the shell shipped Stylo** — so parity, fidelity
and the perf bench were all validating **a cascade no user had ever seen.** Fixing it changed the numbers
**in both directions at once**: fidelity was **understated** (81.2% → 86.3%) *while simultaneously hiding a
near-total Wikipedia layout failure* that only a screenshot revealed.

Later amended: **gating without the JS engine charges the ENGINE for the absence of the SCRIPT engine.**

## A gate that CANNOT FAIL is a decoration — and they go vacuous SILENTLY

A coverage gate returned **1.0 when `probed == 0`**, and its own default URL list contained **`example.com`,
which has NO `[id]` elements at all** — so it probed nothing, **scored a perfect 100%**, and *inflated the
mean of the very gate meant to catch missing content.*

**Proven by mutation: emptying `node_rects()` so the browser rendered NOTHING still scored 100% there.**
The clickability gate had the identical shape (a browser that finds **zero links** scores as *perfectly
clickable*).

## MUTATION-TEST THE WALL — and then verify the mutation tester

`falsify.sh` installs, for each gate, **the exact bug that gate exists to catch**, and asserts it goes
**RED**. Its first run found **five** defects, including a **Bar 0 gate — the one between the user and a
frozen tab — that was VACUOUS**: deleting the page-budget function outright left it **green**, because it
was being protected by an unrelated per-request timeout.

**Three further traps, all real:**

1. **A mutation that fails to COMPILE returns non-zero exactly like a failing assertion** — so a typo
   *certifies the gate by nothing*. The falsifier must **BUILD first** and report a build failure as
   **FALSIFIER BROKEN**, never as evidence about the gate.
2. **Two gates racing over a process-global `OnceLock`** made the verdict depend on **thread scheduling**.
   (`request_timeout()`/`load_budget()` memoise process-wide: **the first caller to read them wins,
   forever.** One test file = one binary = its own `OnceLock`.)
3. **A killed run left a mutated constant in the tree** (`MAX_TASKS_PER_DRAIN = u32::MAX`, in a Bar 0
   path), which the next run then **"backed up" and "restored" as if it were the original.**

> **A "VACUOUS" verdict is a CLAIM ABOUT THE GATE. Verify it before believing it.** Six times the verdict
> was false and *the gate was right while the mutation was wrong* (aimed at a dead function, an unscanned
> file, the wrong score axis). **The tool that checks the instruments is an instrument.**

**And a linker OOM is not evidence about your code:** `ld terminated with signal 9` made the harness report
FALSIFIER BROKEN for two perfectly good mutations. Retrying at `CARGO_BUILD_JOBS=2` proved both.

## "The probe didn't say yes" is NOT "the probe said no"

Made **three times in four ticks**. `localStorage`, `FormData`/`URLSearchParams` and `position: sticky` were
each recorded as **"❌ missing"** and each **already worked** — **twice the replacement was written before
anyone noticed.**

One read as missing only because **the capability probe was served from a `file://` URL — an OPAQUE ORIGIN,
which correctly answers `QuotaExceededError` in EVERY browser.**

> **Serve capability probes over real HTTP, through the real pipeline. And if the probe does not test it,
> its status is UNKNOWN — which is not "missing".**
> **An absent measurement is not a negative measurement.**

## Corpus BREADTH, not verification throughput, surfaces class bugs

- **3 sites** reported COVERAGE 99.7% and *"everything is fine."*
- **20 sites** found that a page was **printing its own JavaScript**, that `:checked` never matched
  anywhere, that checkboxes were invisible, and that docs.python.org rendered **entirely dark**.
- **265 sites** found a **SIGSEGV core dump** (apple.com) and the whole hang class.

> **A three-site sample is not a benchmark; it is an anecdote that confidently reports that a bug on one of
> those three is the most important bug on the web.** *The bugs a corpus cannot find are exactly the ones no
> corpus site happens to use.*

## Every number has a HARNESS, and the harness is part of the number

- **Job count is part of the measurement.** 4 jobs → 11 hangs/88 sites (**12.5%**); 12 jobs → 22/45
  (**49%**) — **same binary, same corpus, same hour.** (Twelve parallel oracle runs meant **189 concurrent
  Chromium processes**, and the watchdog fired on *manufactured contention*.)
- **`export -f` + xargs workers SURVIVE the death of their driver** — a previous crawl's workers kept
  writing into the new run's results directory. Caught **only by luck** (the two script versions used
  different labels). Every record now carries a **`RUN_ID`**, and the crawl **refuses to start on live
  workers**.
- **An interrupted crawl always UNDER-reports**, because *the sites that hang are the ones still running
  when you kill it.* The status script **refuses to print a partial run.**
- **A benchmark that shares a machine with a compile is not a benchmark** — and **RAM, not cores, was the
  binding constraint.**

## Residual-bug estimation must use a REMOVAL model, because discovery is SERIAL

Each tick's fix changes the codebase, so this is **not** independent sampling of a frozen artifact — a naive
Lincoln-Petersen estimator will **UNDERESTIMATE** the residual. Use a **removal model (Zippin/Moran)**: fit
the declining discovery-rate curve; the x-intercept estimates the total population.

**Report it as a LOWER BOUND, scoped to the current capability surface — and EXPECT the estimate to GROW as
the crawl frame expands.** *A rising number from better instrumentation is the method working, not
failing.*

## Read Blink/Gecko for the ALGORITHM; never copy the CODE — and know what that buys

For any ambiguous, edge-case-heavy behaviour (margin collapsing, line breaking, float/BFC interaction, event
dispatch order, IDL reflection), read the reference source **first** and extract the *algorithm and its
edge-case list*, **citing the file/function in the commit.**

**Stated ceiling, so it is not over-extrapolated:** this compresses **DISCOVERY, not IMPLEMENTATION** — the
Rust still has to be written — and it does **nothing** for external-integration problems (codec licensing,
GPU drivers, DRM), which are not algorithm-discovery problems.

## Three gates that exist because green gates coexisted with real bugs

- **G_ALLOC** — every perf floor stayed green through a clone-per-wheel-event regression, because **a
  load-time bench measures throughput on an idle queue, not the marginal cost of an EVENT.**
- **G_TEARDOWN** — forbids `libc::_exit()` or any process-exit path bypassing Rust `Drop`. *A workaround
  that hides a crash is a data-loss bug wearing a disguise.*
- **G_SILENT_FAIL** — a swallowed script exception hid two missing IDL properties that were killing
  navigation on **every mdbook site**. *A caught error that is not logged or surfaced is a gate violation,
  not defensive coding.*

## THREE anchors of parity scope, and each sees what the others cannot

1. **The differential oracle** (265 real sites vs Chromium) — *what real pages do*; needs Chromium to say
   what's right; blind to anything that does not move a box.
2. **WPT** (`docs/wiki/wpt-horizon.md`) — *what the spec says*; carries its own verdict, no oracle needed;
   sees adversarial cases no real site generates; blind to which spec features the real web actually uses.
3. **The doc/app/platform-web capability roadmap** (`PARITY-LEDGER.md`) — *which class of the web works
   end-to-end*.

**No one of them is sufficient.** The oracle found the cascade dropping 41% of real sites' nested rules;
WPT found `insert_before(X,X)` looping forever and `load` never firing; the roadmap is what says iframes and
lazy-load come before media. **Feed all measured WPT categories into the priority ledger with the same
`usage × divergence` formula the oracle's cluster ranking uses — one ledger, not three.**

## The async CI lane is redundant verification you never wait on

`.github/workflows/ci.yml` runs the full wall on every push, in parallel, at zero cost to the local loop —
a regression it finds is an ordinary gate failure at the next check-in, never an interrupt. Split into a
**badge-bearing Linux lane** (shipping config, must be green) and a **cross-platform known-gap lane**
(`continue-on-error`, promoted into the badge when a platform goes green). *A green badge that has stopped
meaning anything is worse than a red one from a real regression.*

## The pattern ledger, and why it is now executable

`docs/loop/WEB-PATTERNS.md` decides what this project builds next. It is the most load-bearing instrument
in the loop, and for a long time it was **the least verified file in the repo**.

At tick 65 every `❌` in it was probed. The result:

| The ledger said | The truth |
|---|---|
| *"~1 site in 4 **hangs** — Bar 0. Nothing else matters at this ratio."* | **4 sites in 265** (1.5%). Off by 16×, and it was steering the roadmap. |
| *"React committing its render — ❌ still silent. Renders nothing."* | **React renders.** `#root` gets its children, the app's text, zero errors. |
| *"`append`/`prepend`/`before`/`after`/`replaceWith` ❌"* | **All five work.** So do `insertAdjacentHTML` and `remove`. |
| *"`outerHTML`, `innerText` ❌"* | **Both work.** |
| *"`Blob`/`File`/`FileReader` ❌"* | **All three work.** |
| *"`getSelection`/`Range` ❌"* | Both **exist**; only `document.createRange()` is missing. |
| *"CSS `transform` — not in computed style, a real gap"* | The transform **is applied** — the box really moves. Only the *computed-style read-back* is missing. |

**Three of its top three priorities were phantoms.** The loop had been aiming at ghosts.

### The mechanism

The lesson — *an absent measurement is not a negative measurement* — had been written down **five times**
(PROCESS #19, #20, #21, #35, #41) and did not hold. A rule you can recite while breaking it is a
decoration. So it stopped being a rule:

> **`G_CAPABILITY` runs the ledger's claims as assertions**, on every wall. 42 of them. A `✅` that stops
> being true **fails the tick** — which is the RATCHET (*never regress capability*) made mechanical. And
> every `❌` prints a **receipt** from the same run, so the next person reads a measurement instead of
> inheriting a rumour.

The ledger cannot drift from reality, because reality is what runs.

### The gaps that are real (with receipts, tick 65)

* **`<canvas>` 2D draws nothing.** Not absent — a *stub*: `getContext('2d')` returns a context, `fillRect`
  is a function, and filling the canvas red then reading a pixel gives `0,0,0,0`. It is deliberate (a
  blank chart beats a `TypeError` that takes the whole bundle down) and it warns in-product. But a page
  that feature-detects canvas is told **yes** and renders nothing.
* **`scrollTop` lies.** Reading gives `undefined`; writing silently creates a plain JS property that
  scrolls nothing. A virtualised list sets it, reads it back, and believes it worked.
* `getComputedStyle().transform` → `undefined` (the transform itself works).
* `display: contents` → reports `inline`.
* `document.createRange`, `document.createEvent`, `URL.createObjectURL` → absent.
