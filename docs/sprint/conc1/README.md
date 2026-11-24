# CONC.1: Layer 1 harness (no model)

Plan and fixed bars: `docs/sprint/CONCURRENCY-PLAN.md`. This node builds and exercises the harness. It makes **no capacity or
cost claim**; the N=1 and N=5 rows are a smoke run that shows every arm runs and every trace is feasible somewhere.

- Code: `bench/conc/harness.py` (driver), `bench/conc/fixtures.py` (own-suite pages and 3 frozen real pages from `demo/www/pages`).
- Gate: `tests/sprint/test_conc1_harness.sh` (self-test green; mutant `share-proc` drops engine-proc isolation, mutant `skip-failed`
  drops failed traces from the results; both turn it red).
- Arms: engine-proc (`--agent-profile=full` and `=lean`, two rows), engine-fork (`--zygote`), chrome-mcp (`--backend chrome`),
  pw-proc (`@playwright/mcp@0.0.83 --isolated --sandbox`, one process per agent), pw-shared (one `@playwright/mcp --isolated --port`
  server: one Chrome, one browser context per HTTP client). Chrome keeps its sandbox on in every arm.
- Cage: each cell runs in its own `systemd-run --user --scope -p MemoryMax=8G -p MemorySwapMax=0`; the harness waits for
  MemAvailable >= 20 GB; agent processes get `oom_score_adj=500`. `caged` in a row is true only if memory.max is 8 GiB and swap 0.
- Traces (each agent runs all, in one session): iso, form, late, walk, heavy, rfc, w3c, mdn. `iso` is the isolation probe: every agent
  writes `localStorage` on a shared origin, waits at a barrier, reads it back, and must see its own id. A trace passes only if every
  step's expected token is seen (a click's token is polled on later snapshots for up to 6 s).
- Driver rule, fixed before the arm rows: on the manuk surface, a target missing from a token-budgeted observation is looked up with
  `find` (the surface's own answer); Playwright's snapshot is complete, so it has no such step.
- Measured per cell: success (traces passed / expected), successful traces per minute (clock starts when all agents are connected),
  p50/p95 per tool call, cold start (connect + initialize) p50, peak PSS of the process tree (sampled 0.5 s, 1 s at N>=25), CPU seconds
  (cgroup usage minus the harness's own), `oom_kill` delta, isolation. Cells the harness did not survive go to `<out>.infra`, never to success.
- `capacity --arm A`: ladder 1,5,10,25,50,100 then bisection; capacity is the largest N whose mean success is within 5 points of the arm's
  N=1. Its logic is checked in the self-test on a synthetic curve; it has not been run against real arms here.
- Smoke data: `layer1-n1-n5.tsv` (3 runs x N in {1,5} x 6 arm rows, chrome_ver 145.0.7632.116).
