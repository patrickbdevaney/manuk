# W5.G1.b.a.a.a: cdp-layer v2 vs v3 at one tree and one pin (GATE PASS)

chrome_ver=145.0.7632.116. v2 = `cdp-layer-v2.tsv`, v3 = `cdp-layer-v3.tsv`. Same 10 tasks x 3 runs.

| task | v2 passes | v3 passes | abs(v2-v3) | v2 runs | v3 runs |
|---|---|---|---|---|---|
| dashdish-10 | 2/3 | 3/3 | 1 | 3 | 3 |
| dashdish-7 | 2/3 | 2/3 | 0 | 3 | 3 |
| fly-unified-6 | 0/3 | 0/3 | 0 | 3 | 3 |
| gomail-3 | 3/3 | 2/3 | 1 | 3 | 3 |
| gomail-5 | 1/3 | 1/3 | 0 | 3 | 3 |
| gomail-8 | 0/3 | 1/3 | 1 | 3 | 3 |
| opendining-10 | 0/3 | 0/3 | 0 | 3 | 3 |
| opendining-4 | 1/3 | 2/3 | 1 | 3 | 3 |
| staynb-2 | 2/3 | 3/3 | 1 | 3 | 3 |
| staynb-4 | 2/3 | 2/3 | 0 | 3 | 3 |
| **total** | 13/30 | 16/30 | 3 | | |

No problems: every task has 3 rows in each set.

## Reading

- Tree: the v2 and v3 sets were both run on the engine tree of e7bd3c7f..HEAD with no `engine/`, `crates/` or `tests/`
  changes in that range (`git diff --stat e7bd3c7f HEAD -- engine crates tests` is empty). `target/release/manuk-mcp` was
  built at 20:57 on 2026-11-14, before the v2 runs, and the `cargo build --release -p manuk-mcp` at HEAD was a no-op.
- Band: the same tree, same pin, same 30 (arm, task, run) keys gave 13/30 (v2) and 16/30 (v3). Per task the difference is
  at most 1 run. That is one repeat, so it is one sample of the run-to-run band, not a variance estimate.
- Attribution: the 2026-11-07 set (9/30) is compared with v2 (13/30) across the commit 77b97d40 (CDP shield). Its gap is 4
  runs, against a 3-run difference between two sets at one tree. The evidence does not separate a tree effect from noise,
  so no cause is attributed. Measuring the tree effect directly is follow-up W5.G1.b.a.a.a.a.
- Gate check: `bench/agent-tasks/w5g1_v3_compare.py` exits 1 when one set has 29 rows (a row dropped), and when the
  `chrome_ver` of the two sets differs. It exits 0 on the real files.
