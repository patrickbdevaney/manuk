# CPU.3 — price of opt-level (MEASURE ONLY; Cargo.toml unchanged) — chrome_ver=145.0.7632.116

Variants of `manuk-mcp` built with `CARGO_PROFILE_RELEASE_OPT_LEVEL` into a throwaway `CARGO_TARGET_DIR` (deleted after each
build; `target/release` untouched), under `systemd-run --scope -MemoryMax=8G`, `CARGO_BUILD_JOBS=16`, clean build each (`build.sh`):
`s` (today), `2`, `3`, `hot3` = `s` plus `profile.release.package.{manuk-css,dom,html,layout,a11y,page,text}.opt-level=3`.
Dependencies (incl. mozjs's C++) are not altered by the override. lto=true, codegen-units=1 for all.

## Binary size and build time (one clean build each, host loaded by other work: load1 15-21 of 24 cores)
| variant | build wall s | binary bytes | vs s |
|---|---|---|---|
| s | 194 | 53,939,648 | — |
| 2 | 282 | 59,281,376 | +9.9% |
| 3 | 289 | 60,236,960 | +11.7% |
| hot3 | 272 | 55,357,312 | +2.6% |

## 10 real pages (`pages.py`, `pages.tsv`): fresh `manuk-mcp --deterministic --backend engine`, navigate+observe, cpu-s = utime+stime of process tree
3 rounds, variant order rotated each round, pages served from `demo/www/pages` over loopback; load1 per row in `pages.tsv` (20-22). Mean of 3 (`summarize.py`):

| page | s | 2 | 3 | hot3 |
|---|---|---|---|---|
| hackernews | 0.87 | 0.83 | 0.81 | 0.81 |
| rustlang | 0.19 | 0.18 | 0.20 | 0.20 |
| mdn-css | 2.15 | 1.93 | 1.88 | 1.97 |
| github | 2.63 | 2.30 | 2.38 | 2.49 |
| lobsters | 0.21 | 0.19 | 0.19 | 0.21 |
| python-docs | 2.10 | 1.91 | 1.92 | 1.94 |
| sqlite | 0.21 | 0.20 | 0.19 | 0.20 |
| pgdocs | 4.47 | 4.23 | 4.57 | 4.48 |
| rfc-editor | 0.52 | 0.49 | 0.48 | 0.51 |
| stripe-docs | 0.53 | 0.49 | 0.47 | 0.49 |
| **sum** | 13.88 | 12.74 (-8.2%) | 13.09 (-5.7%) | 13.30 (-4.2%) |
| mean peak PSS MB (20 ms sampler) | 75.9 | 77.6 | 78.2 | 77.0 |

Per-round sums: s 13.9/14.1/13.63; 2 13.02/13.05/12.16; 3 13.59/13.03/12.64; hot3 13.58/13.06/13.26. All 120 runs ok.

## CONC Layer-1 traces (`conc.sh`, `conc-<v>.tsv`, `conc.loads`): engine-proc-full, N=1, 8 traces, 8 GB cage, 3 interleaved runs
| variant | cpu_s per run | mean | peak PSS MB | p95 ms | success |
|---|---|---|---|---|---|
| s | 4.39 4.45 4.38 | 4.41 | 214.8 148.9 149.0 | 614 554 571 | 1.0 x3 |
| 2 | 4.54 3.70 4.82 | 4.35 (-1%) | 151.3 150.2 150.4 | 660 526 622 | 1.0 x3 |
| 3 | 3.84 4.17 4.03 | 4.01 (-9%) | 152.9 152.2 152.3 | 608 534 512 | 1.0 x3 |
| hot3 | 3.89 3.90 3.82 | 3.87 (-12%) | 151.0 150.4 217.2 | 526 535 839 | 1.0 x3 |

The 215-217 MB PSS cells are first-cell/outlier noise present in both `s` and `hot3`, not a variant effect (same variant is 149 elsewhere).

## Reading
- Real pages: best case -8.2% (opt-level 2); opt-level 3 is not better than 2 (-5.7%); the hot-crate override -4.2%. Pages gain is
  below the 10% falsifier threshold for every variant. CONC cpu-s: `3` -9%, `hot3` -12%, `2` -1% (run spread for `2` is 3.7-4.8, so
  the -12% is within what this noisy, loaded host can show; ordering 2 < 3 on pages and 3 < 2 on CONC is itself a sign the
  differences are close to noise).
- Cost: +10-12% binary for 2/3 and +45-49% clean build time; `hot3` keeps size at +2.6% but gained least on pages.
- **Recommendation (falsifier applied: gain < 10% on the real-page set -> leave it): keep `opt-level = "s"`.** If the owner wants
  CPU, `opt-level = 2` on the whole profile gave the best page result (-8%) for +10% size; this needs a quiet-host remeasure
  before any decision (follow-up CPU.3.a). Size is a first-class target, so no change is made here.
- Limits: host load 15-22 during all runs, one clean build per variant (build times single-sample), cpu-s from `/proc` cpu_times.
  No SCOREBOARD row is written; nothing here is a published claim.
