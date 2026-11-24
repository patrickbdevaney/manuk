# CPU.1 — a11y tree cost was quadratic in table size (chrome_ver=145.0.7632.116)

Fix: `table_is_data` (a whole-table walk) was called by `demote_layout_table` for every row and cell; its verdict is now
memoised per table for one `build_tree_inner` call (thread-local, cleared on entry and on drop, so a DOM change between
builds cannot read a stale verdict). `engine/a11y/src/lib.rs`.

Method: `bench.py` = one fresh `manuk-mcp --deterministic --backend engine` (the arm the CONC harness measures; the default
in-process backend does not show it), cpu-s = utime+stime summed over the process tree, 3 runs per size.
Pages: `fx/t{400,800,1500}.html` are the CONC `/heavy` page shape (h1, script, onclick buttons, header-less table).
`before.txt` = binary built at e6cc95fd; `after.txt` = with the memo. Server: `python3 -m http.server` over `fx/`.

| rows | observe before | observe after | find b/a | click b/a | navigate b/a (mean of 3) |
|---|---|---|---|---|---|
| 400  | 0.18 | 0.06 | 0.14 / 0.04 | 0.58 / 0.19 | 0.34 / 0.25 |
| 800  | 0.59 | 0.12 | 0.53 / 0.07 | 2.06 / 0.39 | 0.84 / 0.44 |
| 1500 | 1.97 | 0.28 | 1.84 / 0.16 | 6.46 / 0.91 | 2.36 / 1.00 |

Gate (`gate.py`, output `gate.out`): observe(1500) 0.280 <= 0.3; 800->1500 growth 2.27x <= 2.5x (before: 3.3x);
a11y-dump JSON byte-identical before/after on 39 pages (6 fixtures incl. 19/20/21-row boundary, nested tables, a
header table, a 403-row archive-shaped table, + 31 `demo/www/pages` real pages incl. hackernews, rustlang, mdn);
`w3c.html` is excluded because the BASELINE differs from itself run to run (pre-existing nondeterminism, noted not fixed).

Red proofs: memo bypassed (`if false`) -> observe(1500) 2.07 / 2.22 cpu-s (>0.3, gate red); an after-binary that appends a
byte -> 39 DIFF, exit 1. Falsifier not triggered: observe fell 7x (2.0 -> 0.28), the suspect was right.

## Honest residual
Still super-linear OUTSIDE the a11y tree: at 3000/6000 rows observe 0.72/1.97, navigate 2.5/6.9 cpu-s, click 2.6/7.0
(a11y build itself, timed with a temporary probe: 9.8 ms at 3000 rows, 41 ms at 6000 — linear). Navigate (cascade/layout)
at 1500 rows is 1.0 vs 0.23 for the same rows as divs. Follow-ups CPU.1.a (observe/find/click path beyond a11y) and
CPU.1.b (navigate: cascade/layout of tables) are in GRAPH.tsv.
