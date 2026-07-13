# WPT BASELINE — the first honest conformance number this project has ever had

**Measured 2026-07-13 (tick 43), release build, `dom/` subset, WPT @ sparse clone of `main`.**

```
FILES 457    subtests 1429 / 6284  =  22.7%
NO_REPORT 0                            ← every file reports. This is the number that makes the rest real.
HANG/TIMEOUT 90                        ← Bar 0. Outranks every failing assertion below.
```

## Why `NO_REPORT 0` is the load-bearing figure

On the **first** run, **100% of files reported nothing** — and the runner's own guard said so rather
than printing "0%" and letting it be read as a conformance catastrophe:

> *"Above ~25% this number is not measuring the engine's conformance — it is measuring whether
> testharness.js can RUN here at all."*

It was right. Four engine defects stood between us and a *readable* score, and none of them move a box
(which is why forty ticks of Chromium box-diffing never saw them):

1. **`window.parent` was undefined** → `testharness.js` walks off the end of the window tree on its
   first action. **100% of WPT.**
2. **`DOMContentLoaded` and `load` were never dispatched** → testharness gates completion on
   `window.load`, so every file timed out.
3. **`setTimeout` threw its delay away** → testharness's own 10s harness timeout fired *before* the
   tests it guards.
4. **`insertAdjacentText` was missing** → testharness's results renderer threw, aborting the
   completion-callback loop → **29 of the first 40 files reported nothing.**

## Per-directory — this is the WORK LIST, and it is the point

| Pass | Subtests | Files | Area |
|---|---|---|---|
| **56.6%** | 107/189 | 5 | `dom/lists` |
| **53.7%** | 66/123 | 8 | `dom` |
| **22.4%** | 1147/5125 | 210 | `dom/nodes` |
| **22.1%** | 81/367 | 68 | `dom/events` |
| **20.8%** | 11/53 | 17 | `dom/traversal` |
| **8.3%** | 2/24 | 16 | `dom/nodes/insertion-removing-steps` |
| **6.2%** | 3/48 | 9 | `dom/collections` |
| **5.6%** | 1/18 | 6 | `dom/events/scrolling` |
| **3.9%** | 2/51 | 38 | `dom/ranges` — **`Range` is inert** |
| **2.8%** | 3/106 | 45 | `dom/nodes/moveBefore` |
| **0%** | 0/149 | 15 | `dom/ranges/tentative` |
| **0%** | 0/2 | 2 | `dom/abort` |
| **0%** | 0/15 | 15 | `dom/nodes/Document-contentType` |

**The single largest lever is `dom/nodes` (5,125 subtests, 22.4%).** `dom/ranges` at 3.9% is not a
conformance problem — **`Range` is one of the ~70 inert stubs**, so it exists and does nothing. That is
the honest state, and it is now *visible* rather than assumed.

## Skipped, counted, and reported — never silently dropped

| Count | Why |
|---|---|
| 88 | needs `testdriver.js` (synthetic input) |
| 67 | not a testharness test |
| 63 | `.any.js`/`.window.js` — wptserve generates the wrapper at request time |
| 4 + 3 | reftests + references — **Bar 2 (pixel), deliberately deferred** |

> *A runner that silently drops what it cannot run is reporting a pass rate for a suite it did not run.*

## The 90 hangs are the most valuable line in this file

**A hang is Bar 0 and it outranks every failing assertion.** The first one found (`ChildNode-after`)
was an **infinite loop in our own `insert_before`**: inserting a node *before itself* built a
self-cycle in the sibling list, because we never implemented the DOM spec's *"if referenceChild is
node, set referenceChild to node's next sibling."*

**No real site inserts a node before itself.** The 265-site differential crawl could never have found
it. **WPT found it in the first 25 tests.** The other 89 are not yet triaged — they are the next tick.

## Reproduce

```bash
./scripts/wpt-setup.sh
export WPT_DIR=$HOME/wpt
cargo run --release -p manuk-wpt -- wpt dom --show-failures
```
