# W3.20.b.a.a: is the french-bistro native tool count stable? (chrome_ver=145.0.7632.116)

Three back-to-back runs of `docs/sprint/w320b/compare.mjs` on the pinned Chrome 145 build, same binary (`target/release/manuk-mcp`), each into its own directory:

| run | french-bistro native_n | manuk_n | verdict | native names |
|---|---|---|---|---|
| repeat/run1 | 1 | 1 | PASS | book_table_le_petit_bistro |
| repeat/run2 | 1 | 1 | PASS | book_table_le_petit_bistro |
| repeat/run3 | 1 | 1 | PASS | book_table_le_petit_bistro |

Context (not part of the 3-run gate): the W3.20.b.a rerun (`docs/sprint/w320b/RESULT.tsv`, commit 475eb710) also gave native_n=1. The original W3.20.b run (commit 6a1e65a8, `docs/sprint/w320b/RESULT.md`) gave native_n=0 on the same pin. So on 145 the history is 1,1,1,1 after the change and 0 once before.

**Verdict: STABLE over the 3 repeats (native_n=1 in 3/3 runs, all PASS, chrome_ver=145.0.7632.116).**
Caveat: the single historical 0 was not reproduced in these 3 runs and its cause is not established. Whether it was a timing effect or a different page state at 6a1e65a8 is unknown. W3.20.b.b (source of the tool) is still open and is the node that would explain it.

Gate check: `sh docs/sprint/w320b/repeat/verdict.sh docs/sprint/w320b/repeat/run{1,2,3}/RESULT.tsv` prints STABLE and exits 0.
Red control: the same script on an input where run2's french-bistro row is replaced by native_n=0 prints FLAKY and exits 1.
