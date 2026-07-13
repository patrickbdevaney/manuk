# PROCESS — the development process is itself under test

`WEB-PATTERNS.md` tracks what the **browser** can do. This file tracks what the **process** gets wrong,
and it exists because the process has now produced more false conclusions than the engine has produced
crashes.

The rule it enforces:

> **Every number has a harness, and the harness is part of the number.**
> **A gate that has never been proven to go red is not known to work.**

## Why this file exists

In a single session (ticks 25–29), the *process* — not the code — produced **thirty** false or unusable
conclusions. Every one of them made the browser look better or worse than it is, and not one was a bug
in the browser:

| # | The process defect | What it did | The mechanism that now prevents it |
|---|---|---|---|
| 1 | Compiled while the oracle crawl ran | Contaminated the hang count; result thrown away | Part 27.1, self-reported; the crawl is the only thing on the machine |
| 2 | Widened the crawl 4 jobs → 12 to "go faster" | Hang rate read **12.5% → 49%** on the same binary in the same hour. The concurrency was mine. | `oracle-crawl.sh` **warns loudly** at a non-baseline job count |
| 3 | Reported a crawl I had **killed** at 92/265 | `STATUS.md` printed `ORACLE_HANGS: 33` as fact. An interrupted crawl always *under*-reports — the sites that hang are the ones still running when you kill it. | `status-update.sh` **refuses** to print a partial crawl |
| 4 | Stragglers wrote into a fresh run | `export -f one_site` means xargs children survive a `pkill` of the driver. Two experiments merged under one name; caught only because the two script versions happened to use *different status labels*. | Every record carries a `RUN_ID`; the crawl **refuses to start** on live workers; `status-update.sh` refuses a mixed-run directory |
| 5 | **The Bar 0 metric measured Chromium** | "73 of 265 sites HANG (27.5%)" — the headline that set the schedule for several ticks — was the oracle *process* hitting a watchdog that wraps **Chromium's render too**. Chromium is the slower engine on **84%** of this corpus. The real number is **4.4%**. | `TIMEOUT` is attributed to **nobody**; Bar 0 is computed from `manuk_ms`, our own clock |
| 6 | **The self-audit lied about itself** | Its SPA check counted *files in `tests/spa/`* (three: `apps/`, `build.sh`, `README.md`) and concluded the miner had never run. It had run against all eight frameworks and produced five engine fixes. | The check now asserts the apps exist **and** that what the miner found is pinned by a gate |
| 7 | **A gate passed while measuring nothing** | The first `G_DEDUP` called `Page::load` — the sync path, which never fetches. `FETCHES` was 0, so `assert_eq!(dupes, 0)` passed. It would have been green through the entire duplicate-fetch storm it was written to catch. | `G_DEDUP` asserts **it measured something** before asserting the thing is fine; and `scripts/falsify.sh` proves every gate can go red |
| 8 | **The dedup metric conflated cache hits with bandwidth** | "nytimes: 507 duplicate fetches" counted repeat `fetch()` **calls**, most of them free cache hits. The number that costs bandwidth — the same URL on the **wire** twice — was 4. | `NET_REQUESTS` / `NET_DUPES` count the wire; `FETCHES` counts calls; the gate asserts on the wire |
| 9 | **The falsifier needed falsifying** | On its first run `scripts/falsify.sh` reported `G_LOAD` **VACUOUS**. It was not. The mutation removed the *outer* `tokio::time::timeout`, but `finish_loading_inner` carries its own per-phase budget — so the guarantee survived, and the gate was right to stay green. **A weak mutation produces a false "vacuous" verdict**, which is a defect in the meta-gate itself. | A falsifier must mutate **the guarantee at its source**, not one implementation of it. `G_LOAD`'s now disables `load_budget()` itself, which feeds every layer. |

| 10 | **`G_LOAD` was vacuous for its entire life** | The gate named for the *page budget* was being protected by the *per-request timeout*. `falsify.sh` deleted `load_budget()` outright and the gate stayed **green**: three dead subresources at 1s each is 3s, comfortably under its 10s ceiling, budget or no budget. It would have been green if the budget had been deleted in production. | The ceiling is now **2× the budget** (assert the promise it is named for, not "the page eventually finished"), and the request deadline is set **longer** than the budget so the budget is the only thing that can satisfy it |
| 11 | **Two gates raced over a process-global `OnceLock`** | `g_load_budget` set `MANUK_NET_TIMEOUT_MS=5000`; the other test in the same binary set `1000`. `request_timeout()` memoises, cargo runs tests in parallel, so **whichever touched it first decided for both**. The gate's verdict depended on thread scheduling — and this is *why* #10 could not be fixed until it was found. | One test file is one binary: the tests are split (`g_load_budget.rs`, `g_load_document.rs`), each owning its own process and its own `OnceLock` |
| 12 | **A gate re-derived the constant it was checking** | `the_document_gets_a_longer_deadline_than_its_subresources` carried **its own copy of the `30`**. It was asserting a relationship between two constants it had itself written down. Change `fetch_document`'s real default to 5s and it would have passed, cheerfully, while the browser became unable to open a slow site. | `manuk_net::document_timeout()` is now the **only** derivation, and the gate calls it. Proven: `MANUK_DOC_TIMEOUT_MS=500` now makes it fail |

| 13 | **`pkill -f <pattern>` matched its own shell** | Three times this session a cleanup command killed the very shell running it, because the pattern appeared in its own command line. Once it killed a build mid-flight and reported a spurious failure. | Never `pkill -f` on a string that appears in the command; split the literal (`"oracle-cra""wl"`) or match on the binary path |

| 14 | **The falsifier POISONED the source tree** | A falsify run was killed before its EXIT trap fired, leaving `MAX_TASKS_PER_DRAIN = u32::MAX` in `event_loop.rs`. The **next** run then backed up the already-mutated file, mutated it again, and faithfully "restored" the corruption. A Bar 0 code path was now broken *in the tree*, and the next `verify.sh` hung on a genuinely broken engine — looking exactly like a real regression. **The safety tool corrupted the thing it was protecting.** | Every mutation carries a `MUTATION` marker. `falsify.sh` **refuses to start** if a target file already contains one (a previous run died), and **verifies its own restore** afterwards — a restore that silently fails is the same bug one layer down |

| 15 | **A non-compiling mutation read as "gate goes red"** | `cargo test` returns non-zero for a compile error exactly as it does for a failing assertion. I wrote a mutation calling a function that did not exist; it would have "proven" `G_FIRST_PAINT` falsifiable while testing **nothing**. The tool that certifies the gates was itself uncertified. | `falsify.sh` **builds first**, and a build failure is reported as **FALSIFIER BROKEN**, loudly — never as evidence about the gate |
| 16 | **`G_FIRST_PAINT`'s first version was vacuous** | It called `Page::load`, which has never fetched an image in its life. It would have passed before the fix, after the fix, and with the fix reverted. The images were on the paint path in exactly one place — `prefetch_document`, the function the *shell* calls — and the gate was not testing it. | The gate now drives `prefetch_document` over real HTTP, and additionally asserts the images are **still pending** (deferred, not dropped — "fast" must not mean "never loads them") |

| 17 | **Two JS tests in one binary segfault NONDETERMINISTICALLY** | `g_defer` had two `#[test]`s, each standing up a SpiderMonkey context. The leaked per-process runtime tears down messily when they co-run: the binary passed, then segfaulted, then passed. **A flaky gate is worse than a missing one** — it gets ignored, and an ignored gate protects nothing. | One test per JS gate binary, on purpose. `js_conformance` has been one giant test for exactly this reason, and the reason is now written where the next person will look |
| 18 | **A split behaviour change was applied to 2 of its 3 call sites** | `Page::load` and `from_prefetched` got the new deferred pass; `load_async` did not. **Every SPA in the suite silently stopped mounting** — a Vite bundle is a module, deferred by default, nothing ran the deferred pass, and the root element sat there correctly sized and empty. | `G_DEFER` asserts `Page::load` still runs **every** script. The rule: *every path that used to run all the scripts must still run all the scripts.* Exactly one caller may split them — the shell — because it is the only one with a human waiting |

| 19 | **A capability audit run from the wrong ORIGIN reported the browser as broken** | The first `CAPABILITIES.md` opened with *"`localStorage` — 27% of the web — THROWS. Not a gap, an outage."* **False.** A real, persisted, per-origin `localStorage` had existed for ages; it threw because the probe was a `file://` URL — an opaque origin, which gets no storage *in every browser*. **I had already written a replacement shim.** One more step and I would have shipped a worse duplicate of a working feature and reported a 27%-of-the-web win that did not exist. | The capability probe is **served over real HTTP** (`docs/loop/capability-probe.html`), never opened from disk. Support numbers are measured from a real origin or they are not measured |

| 20 | **I implemented a duplicate of a working feature. Twice, in two ticks.** | `localStorage` (tick 33) and `FormData`/`URLSearchParams` (tick 34) both already existed and both worked. I wrote replacements for both. The second time the shim was dead on arrival — guarded by `typeof === 'undefined'` — and I only noticed because **the behaviour did not change when I "fixed" it**. | The cause was never carelessness about the code: it was **trusting a capability probe that did not test the capability**. The probe is the AUTHORITY. If it does not test something, that capability's status is **UNKNOWN** — and *"unknown" must never be silently read as "missing"*, which is exactly how you ship a worse copy of working code. `capability-probe.html` now tests everything before I touch it. |

| 21 | **A third untested assumption in three ticks** | `CAPABILITIES.md` said `position: sticky` was "laid out, does not stick". **It was never tested.** `apply_sticky` had existed all along and works — a sticky header pins correctly at scroll 500. Same shape as `localStorage` (#19) and `FormData` (#20). | The pattern is now unmistakable and it is one rule: **if the probe does not test it, its status is UNKNOWN — and I must go and test it before writing a status, not after.** Three ticks in a row I wrote "❌ missing" where the truth was "✅ works, untested". |

| 22 | **The crawl report announced "coverage 2.8%" for a browser that renders fine** | I wrote `crawl-report.sh` to make the speed/coverage rule mechanical — and on its **first run** it lumped all three divergence kinds together and called the result Bar 1. `geometry` (123,796 of them) means *the node exists, at the same size, in a different place* — that is **Bar 2, deferred by settled decision**, not a rendering failure. The real Bar 1 number is **92.2% node presence**. I nearly reported a catastrophic regression that did not exist. | The report now separates `missing` (the node is not there → Bar 1), `display` (there, shown differently → Bar 1) and `geometry` (there, same size, moved → **Bar 2, reported, never scored**). The instrument built to stop me trusting bad numbers produced one on its first run — which is the fourth time an instrument has done that here, and the reason none of them get to be trusted on sight. |

| 23 | **The pre-commit hook refused a tick for a stale entry from a DIFFERENT EPOCH** | The tick counter has been reset at least once, so `## Tick 38` exists twice — once from 2026-07-12 and once from now. The hook took the **first** match, handed itself an entry from a numbering scheme that predates the TICK-SHAPE rule, found no shape, and **blamed the author**. | The journal is **append-only**, so the current entry for tick N is the **most recent** one — always. The hook now takes the last match. *A check that fails closed on its own bug is still a broken check* — and this is the fourth time this specific hook has refused its own author. |
| 24 | **An optimisation was never asked what it DROPPED** | `RuleIndex` (tick 14) made the cascade 1.7× faster by bucketing rules by their rightmost selector. It read each `StyleRule`'s `selectors` and `block` and **never looked at its `rules` field** — the nested rules. **41% of the corpus** (a floor) had every nested rule silently thrown away. It shipped, was measured for speed, and passed every gate for 25 ticks. | **An optimisation that makes a data structure smaller must be asked what it dropped.** No gate could see this: every gate compared *boxes*, and the boxes were internally consistent — they were just consistently wrong. `G_SELECTOR` now asserts a nested rule applies AND that every selector that already worked still does. |
| 25 | **A fifth "✅ untested" in the capability ledger** | `CAPABILITIES.md` said `:has()` ✅ "(Stylo)". It was never tested. Stylo's *servo* build hardcodes `parse_has() -> false`, so every `:has()` rule is **dropped as unparseable** — 13% of sites. | Same rule, fifth instance: **if the probe does not test it, its status is UNKNOWN.** The selector conformance probe now covers `:has()`, nesting, `:is`/`:where`/`:not`, attribute selectors and combinators. |
| 26 | **The self-audit checked 4 of 12 gates and reported "all proven"** | Its falsifier check **hardcoded** `G_DEDUP G_LOAD G_RUNAWAY G2` — the four gates that existed when it was written. Six more had shipped since and it knew about none of them. It was reporting the wall as certified while checking **a third of it**, and would have said so forever. | The gate list is now **derived from `verify.sh`**, never carried as a copy. Same defect as a test re-deriving the constant it checks (#12) and a ledger whose ✅ was never tested (#19/#20/#21/#25): **a check that keeps its own copy of the list it is checking will drift from reality, silently.** |
| 27 | **`G1` — the visual-fidelity gate — could not fail** | Its floor is applied to the **structural** score, and `coverage` returned **1.0 when `probed == 0`**. `example.com` was in G1's **default URL list** and has **no `[id]` elements at all** — so it probed nothing, scored a perfect 100%, and inflated the mean of the gate whose entire job is catching missing content. Mutation-testing found it: emptying `node_rects()` so the browser renders **nothing** still scored 100% there. | `coverage` is `NaN` when nothing was probed, `report` **fails loudly** on it, and G1's URL list now contains only pages it can actually measure. |
| 28 | **`G6` — clickability — could not fail either** | `MISSED` is 0 when the page has **no links at all**, so a browser that finds **nothing** scored a perfect clickability. Found by the same mutation pass. | The gate now **refuses fewer than 50 links as vacuous** ("fix the harness, not the threshold") — the same "did it measure anything?" guard `G_DEDUP` needed (#7). It now reports *3 unclickable of 484 links*. |
| 29 | **Three more weak mutations, three more false "VACUOUS" verdicts** | Falsifiers aimed at `Page::links()` (G6 uses the DOM directly), a dead `_mutation_stall()` nobody calls (G_INTERACT), `shell/src/tab.rs` (G_TEARDOWN scans only the shipping exit paths), and a black canvas (G1's floor is structural, not visual). Each time the gate was **right** and the mutation was **wrong**. | Restated, because it keeps recurring: **mutate the guarantee at its SOURCE, and confirm the mutation reaches the code the gate drives.** A "VACUOUS" verdict is a claim about the gate — verify it before believing it, exactly as you would any other measurement. |
| 30 | **I "fixed" a file that is not compiled** | Flipped `parse_has() -> true` in `./stylo/style/servo/selector_parser.rs` and rebuilt clean — and `:has()` rules were **still dropped**. The workspace depends on **`stylo = "0.19"` from crates.io**; `./stylo` is a *reference checkout* that nothing builds. The edit was inert. | **Before editing a vendored directory, confirm the build actually uses it** (`grep` the Cargo.toml for a `path =` or `[patch.crates-io]`). A source tree sitting in the repo is not evidence that it is the source. This also **re-prices the `:has()` decision**: it is not a one-line flag delta, it is *vendoring Stylo* — a genuine fork — or a hand-rolled supplement. |

Eight of the twenty-three were **found by an accounting check, not by the gate that was supposed to catch them**.

**Defect #20 is the one with a general shape worth naming**, because it is the same shape as #5 (the Bar 0
metric), #16 (the vacuous gate) and #19 (the file:// probe):

> **An absent measurement is not a negative measurement.** "The probe did not say yes" and "the probe
> said no" are different facts, and treating the first as the second is how a project spends a tick
> rebuilding something it already had — or, worse, reports the rebuild as a win.

Defect #14 is the one to be frightened of. Every other entry here produced a *wrong number*. This one
produced **wrong code, in the working tree, in a Bar 0 path**, and the failure it caused was
indistinguishable from a real regression — I would have gone looking for a bug in the event loop that I
had put there myself, with a tool whose entire purpose is safety.

> **A tool that can leave the tree in a worse state than it found it must be able to prove it did not.**
> Not "should be careful". *Must be able to prove it* — a marker it can look for, and a check it runs on
> the way out.
That is the signal this file exists to act on.

Defect #9 is the one worth staring at: **the tool built to detect vacuous gates was itself, on its first
run, wrong in the same way.** It reported a *false positive* — a working gate declared vacuous — because
its mutation was too weak to actually remove the guarantee. There is no bottom to this: the instrument
that checks the instrument is also an instrument. What makes it tractable is not paranoia but a rule
about *where* to cut:

> **Mutate the guarantee at its source, not one implementation of it.** If the promise is "a dead
> subresource cannot hold the document hostage", then break `load_budget()` — the thing every layer
> reads — not one of the two places that happens to call it. A mutation that a redundant code path can
> survive is testing the redundancy, not the promise.

## The mechanisms, and what each one is for

| Mechanism | Catches |
|---|---|
| `scripts/falsify.sh` | **a gate that cannot go red** — mutation-tests the wall against itself |
| `scripts/verify.sh` gate receipt | verifying one version of a diff and committing another |
| `scripts/hooks/pre-commit` | a tick with no journal entry, no cluster ID, an overdue audit, an untouched pattern ledger, a credential shape |
| `scripts/self-audit.sh` | anything the methodology **prescribes** that has never been **built** |
| `scripts/status-update.sh` | a partial crawl, a mixed-run crawl, a hand-narrated status |
| `scripts/oracle-crawl.sh` | a non-baseline job count, a crawl started on top of live workers |
| `STATUS.md` Lessons | a lesson that recurred and therefore should have been a gate |

## The rule that generated most of them

Every entry above is one sentence, stated twice:

> **The instrument is part of the experiment, and it is the part that lies to you.**
> A measurement harness gets the same scrutiny as the code under test — *more*, because nothing is
> watching **it**.

## The falsifiability rule (Part 33)

A gate is not "a test that passes". A gate is **a test that is known to fail when the thing it protects
is broken.** Those are different claims, and only one of them is worth anything.

`G_DEDUP` passed for its first ten minutes of life while measuring **nothing**. `G_CONTAIN` has always
been trustworthy for exactly one reason: it *deliberately panics a build* and asserts the page dies and
the process does not. It proves itself.

So: **every gate declares how to break it**, and `scripts/falsify.sh` actually breaks each one and
asserts it goes red. A gate that stays green under its own mutation is **vacuous**, and vacuous is worse
than absent, because absent is honest.

This runs on the self-audit cadence, not every tick — it is expensive by construction, since it builds
a deliberately broken engine once per gate.

**On its first run it found three defects (#10, #11, #12), all in one gate**, and the gate in question was
`G_LOAD` — a *Bar 0* gate, the one standing between the user and a frozen tab. It had never once tested
the thing it was named for. Nothing else in the process was going to find that: it passed every tick, it
looked right, and it was written by someone who believed it.

The pattern underneath all three is one sentence:

> **A test that can pass without the code it protects is not a test.** Not a weak test — *not a test*.
> The only way to know is to take the code away and watch it fail.

## How to add to this file

When the *process* produces a false conclusion — a number you believed and shouldn't have, a check that
passed and shouldn't have, a measurement you had to throw away — it goes here **before** the fix does.
The row is: what it did, and what now makes it impossible. If you cannot name the mechanism, you have
not finished.

A defect that only produces a lesson is not closed. A lesson you can recite while breaking it is a
decoration — and that has been demonstrated, in this project, three times in one day.
