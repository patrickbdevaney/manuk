# W4.P0.c.a.a.a.a — F1 cascade/mid spread under load

Runner: `f1-spread.sh` (same bench command, corpus, field parse and 0.55 floor as `scripts/verify.sh` F1; verify.sh unchanged).
Data: `F1-spread.tsv` (chrome_ver=145.0.7632.116). Each row is one verify-equivalent bench invocation (median of 5 inside).

| condition | runs | cascade/mid ratios | exceed 0.55 |
|---|---|---|---|
| ambient-load8.8 (load1 ~8.8 on 24 cores; the 600 s quiet wait timed out, see ambient-wait.log) | 5 | 0.50, 0.62, 0.43, 0.52, 0.81 | 2/5 |
| loaded-busy24 (ambient + 24 `yes` spinners; load1 10.9 to 17.8) | 5 | 0.41, 0.23, 0.54, 0.26, 0.75 | 1/5 |

Quiet condition NOT reached: this host is shared, and load1 stayed at 7.9 to 8.8 for the whole wait.
So no row here is labelled "quiet". The quiet measurement is a follow-up (W4.P0.c.a.a.a.a.a).

Finding: F1 flakes. 3 of 10 invocations exceed 0.55, under the same code. The floor is not edited (SUCCESS-style
discipline: the floor is an owner decision, not a tuning target).
