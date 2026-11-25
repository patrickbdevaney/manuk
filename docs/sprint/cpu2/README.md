# CPU.2 (NOT LANDED: apply cpu2.patch, see GRAPH CPU.2.land) — tree builds, cascades and layouts per tool call (chrome_ver=145.0.7632.116)

Counts come from `MANUK_BUILD_TRACE=1` (one stderr line per tool call: a11y-tree builds, cascades, layout installs,
process-wide counters `manuk_page::{A11Y_BUILDS,CASCADES,LAYOUTS}`); cpu-s = utime+stime of the process tree, **min of 3
fresh `manuk-mcp --deterministic --backend engine` runs** (host load is noisy; counts are exact). `gate.py` runs it,
`gate.out` is its output, `gate.json` the numbers. Fixtures `fx/`: `small.html` (button changes one text node + an input),
`div1500.html` (1500 div rows), `t1500.html` (the CPU.1 1500-row header-less table). "off" = `MANUK_NO_A11Y_CACHE=1`
(and is the pre-change tree-build count: every `tree()` rebuilt, `clickables()` rebuilt, `observe()` for content_height rebuilt).

| page | call | trees off -> on | cascades | layouts |
|---|---|---|---|---|
| all 3 | act (click, one text node) | **7 -> 1** | 2 (was 3) | 0-1 |
| all 3 | find after observe, DOM unchanged | **2 -> 0** | 0 | 0 |
| all 3 | observe | 2 -> 1 | 1 | 1 |
| small | type | 6 -> 1 | 2 | 6 |

t1500 act cpu: CPU.1 left it at 0.91; now 0.24 (cache off, hover-skip on: 0.32). Gate bars: act <= 2 trees, find <= 1,
t1500 act <= 0.45 cpu-s, every tool output byte-identical cache on vs off, observe after the click shows the new text.

What changed (engine/page): (1) `Page::a11y_tree` is cached; the key is DOM mutation_seq + a style/layout generation + a
hash of every box's node and rect + focus + zoom, so geometry changed in place (sticky, element scroll) still misses.
(2) After a click's hover/handler restyle the cascade fingerprint is re-recorded, so the settle that follows no longer
repeats an identical full cascade+layout. (3) A hover that changes no computed style (the recascade leaves the style map
equal, tree was clean before) skips its layout pass.

Red proof: `gate.py --mutant` (`MANUK_A11Y_CACHE_MUTATE=stale`, key never moves) -> exit 1, 6 RED (stale observe after click).

## Honest residual
- Cascades per act are 2, not 1: the hover recascade and the handler's restyle are separate full passes.
- observe (not find) leaves `<body>` dirty, so the NEXT apply_stylesheets re-cascades + re-lays-out the whole page once per
  observe (observe 1 cascade + 1 layout on a clean page, ~0.2 cpu-s at 1500 rows). Follow-up CPU.2.a.
