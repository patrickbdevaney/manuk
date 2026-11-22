# W6.F1.a REAL-fast v2 (held-out): gate met at N=14 only, borderline and isolated; chrome_ver=145.0.7632.116

Tool: `bench/agent-tasks/fast_subset2.py` (rule in its docstring, written before the first run). Selection rounds W5.B1 x3 arms + RG.B x2 arms; 36 tasks have pooled pass rate in [0.2,0.8]; ordered by |p-0.5|. Held-out test rounds: RG.2.B manuk, RG.2.B playwright, W6.B0 cdp-layer (9 run-units).

| N | rank conc (>=0.8) | sign all pairs | paired-diff sd (need <=1.0) | gate |
|---|---|---|---|---|
| 10 / 12 | 0.765 / 0.714 | ok | 0.67 | fail |
| **14** | **0.800** | ok | 0.83 | **PASS** |
| 16 / 18 | 0.778 / 0.722 | DISAGREE | 0.77 | fail |
| 20 / 22 | 0.700 / 0.737 | ok | 0.77-0.81 | fail |
| 24-28 | 0.85-0.90 | ok | 1.19-1.46 | fail (2-task flip invisible) |
| 30-36 | 1.000 | DISAGREE | 1.24 | fail |

Self-test: mutant (36 most extreme-rate tasks) -> conc 0.786, sign DISAGREE: red.
Literal verdict per the pre-registered rule: gate met at N=14 (tasks: dashdish-10 -11 -2 -3, fly-unified-11, gocalendar-2, gomail-5, networkin-3 -4, opendining-10 -2 -4, udriver-7 -9).
Reading: NOT trustworthy as a commit gate yet. N=14 sits exactly on the 0.800 line, its neighbours N=12 and N=16 fail, only 9 held-out units, and the sd uses 3 runs per round (pstdev, biased low). Treat as a candidate; per-commit REAL stays full-102 x3 until W6.F1.a.a confirms it on fresh rounds.
