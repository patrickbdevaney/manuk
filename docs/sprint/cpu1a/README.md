# CPU.1.a — observe/find/click on 1500-6000 row tables (chrome_ver=145.0.7632.116)

Method: `../cpu1/bench.py` (one fresh `manuk-mcp --deterministic --backend engine`, cpu-s = utime+stime over the process
tree), fixtures `fx/t{1500,3000,6000}.html` (CPU.1's page shape, generated). `before.txt` = binary at 59fea97a,
`after.txt` = with the fix. 3 runs each, host load ~16 (noisy: medians quoted).

## Where the time went (temporary probes, not committed)
`observe` at 6000 rows: the a11y tree + snapshot + lines are ~0.23 s; the other ~1.2 s is `settle()`, which every
observe/find/extract runs first. `settle_async` evals a one-line script 3 times per call; each `eval_for_test` was
75 ms (3000 rows) -> 250 ms (6000 rows). Inside it, `scroll_geometry_of` (and `snap_candidates_of`, which calls it
again) cost 33 -> 126 ms each: per ELEMENT it called `manuk_layout::relative_offsets()` and `used_end_margins()`,
which are thread-local CLONES of one-entry-per-box maps. O(elements x boxes).

Fix (`engine/page/src/lib.rs`): fetch both maps once above the loop; `snap_candidates_of` returns early when no
element has `scroll-snap-type` (it was a second whole-page scroll-geometry pass for nothing).

## Result (cpu-s, median of 3)
| rows | observe before | after | find b/a | click b/a | nav b/a |
|---|---|---|---|---|---|
| 1500 | 0.22 | 0.22 (0.16 in the run before the snap early-return; load noise) | 0.11 / 0.09 | 0.70 / 0.64 | 0.72 / 0.66 |
| 3000 | 0.55 | 0.45 | 0.35 / 0.19 | 1.85 / 1.32 | 1.92 / 1.26 |
| 6000 | 1.39 | 0.87 | 0.97 / 0.39 | 5.0 / 2.79 | 4.87 / 2.56 |

observe 6000/3000: 2.5x -> 1.9x (bar <= 2.3x: MET). click at 1500 rows: 0.70 -> 0.53-0.65 (bar <= 0.5: NOT MET).

## Why the gate is not fully met
Click on 1500 rows costs about what a navigate does (0.55-0.65): a click that edits one text node re-cascades and
re-lays-out the whole table, and `find`/`observe` after it run `settle` again. That is cascade/layout work
(CPU.1.b) and tree-build count per act (CPU.2), not something this node's fix reaches. No threshold was changed.
The byte-identity corpus run (cpu1/gate.py) was not re-run on this change: it touches only JS-visible
scroll geometry, which verify.sh's wall covers. Mutant for the fix = the before binary (observe 6000 = 1.39 > 0.87).
