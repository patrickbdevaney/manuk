# CPU.4 — where navigate+observe CPU goes on three content pages, next to Chrome — chrome_ver=145.0.7632.116

Pages: `demo/www/pages/{mdn-css,w3c,rfc-editor}.html` (the frozen HTML), served from loopback by the scripts themselves. **Their
sub-resources are not frozen**: w3c/mdn still fetch fonts (404 on loopback) and w3c fetches ~13 real images/SVGs from www.w3.org;
every arm below loads the same live images, but the network is a source of variance and a changed remote file would shift w3c.
Host: shared, load1 8-20 while measuring (`uptime` before the attribution run: 19.8). cpu-s = `ru_utime+ru_stime` from `wait4` of the
process we spawned (Chrome: includes its reaped children), 3 interleaved rounds, medians. Binary: `target/release/manuk-mcp` and
`a11y-dump` at this commit's tree (adds only the off-by-default `cascade_pass`/`layout_pass` trace lines, `engine/page/src/lib.rs`).

## Method (no engine timers changed, perf is unavailable: perf_event_paranoid=4)
`attrib.py`: a sampler reads per-thread `/proc/<pid>/task/*/schedstat` every ~2 ms (exited threads keep their last value); the
`MANUK_NET_TRACE` lines (phase spans, plus the new one-line-per-pass `cascade_pass`/`layout_pass` with the call-site line) are laid
over that CPU curve; a phase's CPU is `cpu(end)-cpu(start)` and the cascade/layout passes inside a phase are subtracted from it and
reported on their own rows. `mcp` arm = real `manuk-mcp --deterministic --backend engine` over JSON-RPC (`navigate` then `observe`),
the trace file in the confine-writable cache dir (Landlock denies /tmp writes); the navigate/observe boundaries are the driver's reply
times. `dump` arm = `a11y-dump` (navigate + a11y build + serialise, no MCP layer). The rows cover the whole process life, so the sum
matches the `wait4` total (that is by construction; the point is that no row is a guess: 99-100% below). `compare.py` measures totals
for manuk-mcp, a11y-dump and headless Chrome (`--dump-dom`, i.e. load only, **no observe equivalent**; a warmed `--user-data-dir`
because a fresh one hung silently here) in the same rounds.


## manuk-mcp (navigate+observe) — median cpu-s of 3 rounds

| phase | mdn-css | w3c | rfc-editor |
|---|---|---|---|
| startup (before first trace event, incl. JS prewarm) | 0.053 | 0.054 | 0.053 |
| cascade | 1.034 | 0.027 | 0.048 |
| layout | 1.388 | 0.035 | 0.257 |
| parse (html) | 0.019 | 0.006 | 0.017 |
| script (JS exec + script fetch/compile) | 0.088 | 0.044 | 0.055 |
| stylesheet fetch+parse | 0.030 | 0.001 | 0.013 |
| subframes | 0.048 | 0.007 | 0.004 |
| images/masks/fetch (decode) | 0.031 | 0.613 | 0.003 |
| a11y tree (+ observe serialisation under mcp) | 0.113 | 0.009 | 0.115 |
| mcp shutdown | 0.012 | 0.013 | 0.009 |
| navigate tail after last load phase (tree seed, snapshot, settle) | 0.157 | 0.031 | 0.129 |
| **sum of the rows above** | 2.975 | 0.831 | 0.685 |
| **total (wait4 rusage)** | 2.978 | 0.833 | 0.690 |
| sum/total | 99.9% | 99.8% | 99.3% |

passes (median n / cpu-s per call site):

- mdn-css pass[cascade@page/lib.rs:10155]: n=1, 0.155 cpu-s
- mdn-css pass[cascade@page/lib.rs:13005]: n=3, 0.429 cpu-s
- mdn-css pass[cascade@page/lib.rs:140]: n=1, 0.151 cpu-s
- mdn-css pass[cascade@page/lib.rs:7595]: n=1, 0.146 cpu-s
- mdn-css pass[cascade@page/lib.rs:9952]: n=2, 0.159 cpu-s
- mdn-css pass[layout@page/lib.rs:10155]: n=1, 0.222 cpu-s
- mdn-css pass[layout@page/lib.rs:13005]: n=3, 0.703 cpu-s
- mdn-css pass[layout@page/lib.rs:7595]: n=1, 0.222 cpu-s
- mdn-css pass[layout@page/lib.rs:9963]: n=2, 0.259 cpu-s
- w3c pass[cascade@page/lib.rs:10155]: n=1, 0.002 cpu-s
- w3c pass[cascade@page/lib.rs:13005]: n=2, 0.004 cpu-s
- w3c pass[cascade@page/lib.rs:9952]: n=1, 0.021 cpu-s
- w3c pass[layout@page/lib.rs:10155]: n=1, 0.007 cpu-s
- w3c pass[layout@page/lib.rs:13005]: n=2, 0.013 cpu-s
- w3c pass[layout@page/lib.rs:7595]: n=1, 0.006 cpu-s
- w3c pass[layout@page/lib.rs:9963]: n=1, 0.008 cpu-s
- rfc-editor pass[cascade@page/lib.rs:10155]: n=1, 0.006 cpu-s
- rfc-editor pass[cascade@page/lib.rs:13005]: n=2, 0.010 cpu-s
- rfc-editor pass[cascade@page/lib.rs:9952]: n=1, 0.030 cpu-s
- rfc-editor pass[layout@page/lib.rs:10155]: n=1, 0.049 cpu-s
- rfc-editor pass[layout@page/lib.rs:13005]: n=2, 0.079 cpu-s
- rfc-editor pass[layout@page/lib.rs:9963]: n=1, 0.137 cpu-s

## a11y-dump (navigate+a11y+serialise) — median cpu-s of 3 rounds

| phase | mdn-css | w3c | rfc-editor |
|---|---|---|---|
| startup (before first trace event, incl. JS prewarm) | 0.052 | 0.048 | 0.047 |
| cascade | 0.586 | 0.023 | 0.023 |
| layout | 0.944 | 0.026 | 0.170 |
| parse (html) | 0.023 | 0.005 | 0.013 |
| script (JS exec + script fetch/compile) | 0.022 | 0.009 | 0.008 |
| stylesheet fetch+parse | 0.028 | 0.000 | 0.010 |
| subframes | 0.038 | 0.007 | 0.003 |
| images/masks/fetch (decode) | 0.028 | 0.146 | 0.003 |
| a11y tree (+ observe serialisation under mcp) | 0.029 | 0.001 | 0.014 |
| serialise+exit | 0.029 | 0.021 | 0.033 |
| **sum of the rows above** | 1.769 | 0.283 | 0.328 |
| **total (wait4 rusage)** | 1.774 | 0.286 | 0.332 |
| sum/total | 99.7% | 99.0% | 98.8% |

passes (median n / cpu-s per call site):

- mdn-css pass[cascade@page/lib.rs:13005]: n=2, 0.297 cpu-s
- mdn-css pass[cascade@page/lib.rs:7595]: n=1, 0.145 cpu-s
- mdn-css pass[cascade@page/lib.rs:9952]: n=2, 0.147 cpu-s
- mdn-css pass[layout@page/lib.rs:13005]: n=2, 0.452 cpu-s
- mdn-css pass[layout@page/lib.rs:7595]: n=1, 0.228 cpu-s
- mdn-css pass[layout@page/lib.rs:9963]: n=2, 0.242 cpu-s
- w3c pass[cascade@page/lib.rs:13005]: n=1, 0.002 cpu-s
- w3c pass[cascade@page/lib.rs:9952]: n=1, 0.021 cpu-s
- w3c pass[layout@page/lib.rs:13005]: n=1, 0.008 cpu-s
- w3c pass[layout@page/lib.rs:7595]: n=1, 0.007 cpu-s
- w3c pass[layout@page/lib.rs:9963]: n=1, 0.011 cpu-s
- rfc-editor pass[cascade@page/lib.rs:13005]: n=1, 0.005 cpu-s
- rfc-editor pass[cascade@page/lib.rs:9952]: n=1, 0.017 cpu-s
- rfc-editor pass[layout@page/lib.rs:13005]: n=1, 0.039 cpu-s
- rfc-editor pass[layout@page/lib.rs:9963]: n=1, 0.131 cpu-s

## Totals, all three arms, (median of 3, all runs ok)

| arm | mdn-css | w3c | rfc-editor |
|---|---|---|---|
| manuk-mcp | 2.049 (runs 2.03, 2.10, 2.05; ok=True) | 0.630 (runs 0.56, 0.63, 0.67; ok=True) | 0.481 (runs 0.47, 0.48, 0.55; ok=True) |
| a11y-dump | 1.211 (runs 1.17, 1.28, 1.21; ok=True) | 0.223 (runs 0.22, 0.22, 0.23; ok=True) | 0.224 (runs 0.23, 0.22, 0.21; ok=True) |
| chrome | 1.348 (runs 1.30, 1.35, 1.35; ok=True) | 1.080 (runs 1.08, 1.08, 1.01; ok=True) | 1.096 (runs 1.06, 1.10, 1.11; ok=True) |

(The attribution runs read ~40% above the `totals.tsv` runs for the same binary — mdn mcp 2.98 vs 2.05 — because the host was busier
(load 20 vs 8); a quick plain/cache/trace A/B of three runs each gave 2.2-2.7 for all three variants, so tracing is not the cause.
Read the **shares**, not the absolute seconds, across the two files. Quiet-host remeasure: CPU.4.e.)

## Reading
- **mdn-css: 82% of the cpu is cascade (35%) + layout (47%)**, and they run **8 times** each in one navigate (call sites
  `page/lib.rs` 9952 x2, 10155, 13005 x3, 7595, 140), ~0.15 cpu-s per cascade and ~0.22 per layout. Page count of passes is the
  mechanism, not one slow pass. (Related existing rows, not duplicated: CPU.2.a observe-leaves-dirty, PERF.1.3.a.a, PERF.1.2.b.a.c.)
- **w3c: 74% (0.61 of 0.83 cpu-s) is `initial images+masks`, on tokio worker threads**, vs 0.15 for the same phase in a11y-dump:
  the MCP process pays ~4x for the same image set. Which half (JPEG decode, SVG raster, or spin) is not resolved here.
- **rfc-editor: layout 37%, navigate tail 19%, observe/a11y 17%.** The a11y tree under `a11y-dump` costs 0.014 but the same
  call under `observe` costs 0.115: observe is not just the tree.
- Chrome (load only): mdn 1.35 vs manuk-mcp 2.05 (1.5x); w3c 1.08 vs 0.63 and rfc 1.10 vs 0.48 (manuk-mcp lower; why Chrome sits near
  1.0-1.1 on both light pages was not attributed here — only its total was taken). No SCOREBOARD row is written; nothing here is a published claim.
- The task brief's figures (mdn 2.4, w3c 0.66, rfc 0.58) are within this host's run-to-run range of the numbers above.

## Follow-up routing (each mechanism >= 15% of a page's median total; checked by `gate.py`)
MECH mdn-css cascade -> CPU.4.a
MECH mdn-css layout -> CPU.4.a
MECH rfc-editor layout -> CPU.4.a
MECH w3c images/masks/fetch (decode) -> CPU.4.b
MECH rfc-editor navigate tail after last load phase (tree seed, snapshot, settle) -> CPU.4.c
MECH rfc-editor a11y tree (+ observe serialisation under mcp) -> CPU.4.d

## Gate
`python3 docs/sprint/cpu4/gate.py` (sum within 15% of total per page/round in both arms, a Chrome row per page, every >=15%
mechanism routed). Red proofs are in `gate.out`.
