# E4: where the first navigate in a pool child goes (W4.E4.a.a.b)

Measured by `scripts/w4e4-phases.py` (8 forks of `manuk-mcp --zygote`, median ms, local fixture, `MANUK_PHASE_TIMING=1`
prints `phase <name> <ms>` on stderr). chrome_ver=145.0.7632.116 (no Chrome arm).

## Baseline attribution (eager prewarm off: `--env MANUK_NO_EAGER_PREWARM=1`)

```
phase                                              nav1     nav2    delta   (median ms over 8 forks)
js_realm_prewarm                                  19.97     0.25    19.72
first_fetch (join - prewarm)                       0.59     0.42     0.17
load_async (parse/style/layout/scripts)           17.92    16.61     1.31
finish_loading                                     0.24     0.23     0.01
ws_service+other in navigate                       0.06     0.08    -0.02
settled                                            0.76     0.58     0.18
tree_seed_snapshot                                 0.05     0.04     0.01
residual (rpc, text fit, header)                   2.85     2.45     0.40
  . prewarm.engine_init                            0.00     0.00     0.00
  . prewarm.runtime_new                            9.25     0.00     9.25
  . prewarm.global_eval                            0.25     0.23     0.02
  . prewarm.precompile                             9.69     0.00     9.69
wall (client)                                     42.53    20.59    21.94
sum of attributed phase deltas = 21.39 ms; wall gap = 21.94 ms; |diff| = 0.55 ms (tol 2.0)
attribution: PASS
```

Findings:
- The nav1 - nav2 gap (~22 ms here) is the JS realm prewarm: `Runtime::new` (~9.3 ms, first navigation only) and
  `precompile` of the 14 prelude stencils (~9.7 ms). Everything else (fetch, parse/style/layout, tree) is within 1.5 ms.
- `precompile` ran on EVERY navigation (nav2 paid ~9 ms again and the stencil cache grew 14 entries per navigation).
  Fixed: it is skipped when the thread already holds stencils (nav2 29.4 -> 20.6 ms).
- Warming in the pool parent cannot move `Runtime::new` (thread-affine engine; the session runs on another thread);
  preloading a page in the parent (`--zygote-after-load`) just moved ~11 ms into `load_async`, wall unchanged (42.3 ms).
- Fix for nav1: a pool-forked session builds its runtime when its session thread starts, overlapping the agent's
  decision time (`EAGER_PREWARM`). It does not shorten connect -> navigate with zero gap.

## After (eager prewarm on, 100 ms between initialize and the first navigate)

| run | nav1 | nav2 |
|---|---|---|
| eager off, think 100 ms | 42.83 | ~20.6 |
| eager on, think 100 ms | 22.69 | 20.65 |
| eager on, think 0 | 41.6 | 20.6 |

`w4e4-gate.py` unchanged: zygote 41.9-43.5 ms vs cold 66.5-68.0 ms, isolation PASS, `--leak-mutant` FAIL (red).
