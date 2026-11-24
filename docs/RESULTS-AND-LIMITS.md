# Full results, project status and known limits

The README presents what manuk does. This page holds the complete record: every verdict, the suites where each arm
stands, and the known gaps. Text below was moved from the README without changing its numbers.

## Verdict and REAL results

Verdict ([`docs/sprint/VERDICT.md`](sprint/VERDICT.md)): Bar 1 met, Bar 2 not met, which SUCCESS.md reads as
"a useful agent tool, but the engine is not the reason". Bar 2 failed on every branch of the decisive-win test: the
hidden-instruction rate is 0 for the in-process engine and also ~0 for Playwright; on tokens the in-process engine
uses more than the best Playwright arm (727 vs 517 in+out per task, W4.B4); the rollback branch measured 0 flips
(0 of 44 failed tasks on the REAL deterministic subset). The Chrome backend therefore ships as the product, and the
in-process engine as the deterministic, forkable research backend. On the REAL deterministic subset (47 tasks) the
in-process engine passes 10-11 tasks, the Chrome backend 15-18 and Playwright 23-25 (W4.A6); the engine cannot yet
drive those forms. On all 102 REAL tasks, with the LLM-judged evals scored by an owner-provided judge
(`openai/gpt-oss-120b`, validated against positive, negative and agreement controls, W3.18.a.a.b), the tasks passed in
at least 2 of 3 runs are: Playwright 52, the Chrome backend 35, the in-process engine 32 (runs 46/49/52, 35/36/40 and
35/31/30; [`docs/sprint/w318/real-judge-report.tsv`](sprint/w318/real-judge-report.tsv)).

Held-out live-web set (30 tasks, 3 runs): engine 28, Chrome backend 29, Playwright 30
([`sprint/w6/V1.md`](sprint/w6/V1.md)). That document rules out a parity claim, and this project makes none.

## Footprint: every comparison

Measured against the pinned Chrome for Testing (chrome_ver=145.0.7632.116), 3 runs per row, local fixture pages so the
numbers are not shared-cache artefacts. Method, raw rows and what is not claimed:
[`docs/sprint/FOOTPRINT.md`](sprint/FOOTPRINT.md).

| axis | in-process engine | Chrome | reading |
|---|---|---|---|
| memory, 1 page (PSS) | 31 MB | 399-407 MB | 0.08× |
| memory per page, 50 pages | 12.4 MB (one process each), 4.9 MB (forks of one checkpoint) | 20.1 MB (tabs in one browser) | 0.62×, 0.24× |
| new page from a checkpoint | 36-43 ms (`fork`, replay-based) | 65-70 ms (new context and navigate) | — |
| cold start to first observation | 55-57 ms | 305-314 ms | 0.18× |
| real sites, per page (5 paired URLs) | — | — | PSS 0.30-0.31×; wall clock slower (1.66-1.96× this run, not settled) |

The memory gap narrows as pages accumulate, because Chrome tabs share most of their cost. On live sites the engine is
smaller, but its single-page load time is behind Chrome's, mostly in subresource fetch
([`docs/sprint/PERF.md`](sprint/PERF.md)).

Read this table with [`sprint/FOOTPRINT.md`](sprint/FOOTPRINT.md), which later measured real pages held open together.
Against one isolated Chrome process per page the engine holds 0.57-0.70x the memory (10 pages, 6 runs). Against one
shared Chrome with the pages in tabs it holds 1.60-1.90x (10 pages) and 1.21-1.30x (5 unique pages). The 0.30-0.31x row
above is a median of pages loaded one at a time and is not the concurrent figure.

## Status

Measured for the v1.2.0 release on a corpus of real sites rendered by both Manuk and Chromium from identical
snapshots. 76 of 265 corpus sites were reachable by the reference Chromium (the remainder are blocked by bot
protection for the reference itself) and all 76 are scored. "Full" counts a site that hangs the driver as zero;
"measured-only" reports quality where a score was obtained. Method and per-site data:
[`docs/RELEASE-v1.2.0.md`](RELEASE-v1.2.0.md).

| axis | measures | measured-only | full | exit threshold |
|---|---|---|---|---|
| M1 render | node placement relative to Chromium (shape ≥ 0.75), no overlap or overflow | coverage 89.9%, shape 72.1% | — | ≥ 95% of in-scope sites |
| M2 drive | perceive → address → act on page targets | 78.9% (n = 60) | 62.3% | ≥ 95% render ∧ function |
| a11y | accessibility tree role/name/state agreement with Chromium (F1) | 82.0% (n = 65) | 70.1% | ≥ 90% of nodes |

**Web Platform Tests.** About 496,000 WPT subtests pass in the project's internal harness (`tests/wpt`), tracked as a
monotonic ratchet across releases. Results have not been submitted to wpt.fyi, and the figure is an internal
measurement, not a published conformance claim. JavaScript conformance is provided by SpiderMonkey; Test262 results
are not separately reported.

**Release history:** v1.0.0 (2026-08-21, [notes](archive/dev/RELEASE-v1.0.0.md)), v1.1.0
([notes](RELEASE-v1.1.0.md)), v1.2.0 ([notes](RELEASE-v1.2.0.md)). Live, generated project state:
[`STATUS.md`](../STATUS.md).

## Known gaps and non-goals

Gaps on the Phase-0 path: placement fidelity (mean parent-relative shape about 0.69); roughly one in five in-scope
sites not yet reaching a stable, scorable DOM; rich editing (`contenteditable`, `Selection`) and IME composition;
WebAuthn; the M2 function certificate; adoption of a platform accessibility bridge. SpiderMonkey faults inside its
own frames cannot be contained in-process until process-per-tab isolation lands. The WebAssembly demo has no
JavaScript because SpiderMonkey does not target WebAssembly.

Out of scope by decision: Encrypted Media Extensions and DRM, WebRTC, WebGPU and heavy WebGL applications, HTTP/3.
Manuk does not attempt fingerprint evasion or bot-detection circumvention.

Retry-from-checkpoint and best-of-3 forking help on flaky forms, but the same layer over Chrome matches them (W4.C2,
W4.C3). Action receipts help verifiers equally on the Chrome backend (W4.C4).

WebMCP: against Chrome's own registry on 6 demo pages the in-process engine matched tool names on 4 and names plus
schemas on 1 (W3.20.b, gate not met; see `sprint/JOURNAL.md`).
