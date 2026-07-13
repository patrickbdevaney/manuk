# PROCESS — the development process is itself under test

`WEB-PATTERNS.md` tracks what the **browser** can do. This file tracks what the **process** gets wrong,
and it exists because the process has now produced more false conclusions than the engine has produced
crashes.

The rule it enforces:

> **Every number has a harness, and the harness is part of the number.**
> **A gate that has never been proven to go red is not known to work.**

## Why this file exists

In a single session (ticks 25–29), the *process* — not the code — produced **sixteen** false or unusable
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

Eight of the sixteen were **found by an accounting check, not by the gate that was supposed to catch them**.

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
