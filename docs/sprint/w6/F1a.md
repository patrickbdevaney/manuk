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

## W6.F1.a.a verdict: fresh rounds, N=14 FALSIFIED (chrome_ver=145.0.7632.116)

Gate run: `python3 -I bench/agent-tasks/f1aa_verify.py` on the fresh rounds (w6f1aa: manuk x3 and playwright x3, all 102 judged tasks each, primary judge `openai/gpt-oss-120b`). Subset = the same 14 tasks as the selection rule: dashdish-2, -3, -10, -11; fly-unified-11; gocalendar-2; gomail-5; networkin-3, -4; opendining-2, -4, -10; udriver-7, -9.

| clause | pre-registered bar | measured | result |
|---|---|---|---|
| (a) rank concordance of subset vs full-102, 6 units | >= 0.8 | 0.727 (manuk 51/46/46 full, subset 10/9/7; playwright 51/53/50 full, subset 10/10/11) | RED |
| (b) sign of manuk - playwright | agree | full -3.67, subset -1.67 | ok |
| (c) per-round sd of subset totals | <= 0.7 | manuk 1.25, playwright 0.47 | RED |

Verdict: the falsifier fires. N=14 was a lucky pick on the selection rounds; per-commit REAL stays full-102 x3, as W6.F1.a already states.

**Arm-file sensitivity.** `f1aa_verify.py` reads the manuk arm from `bench-queue/out/w6f1aa-manuk.tsv`. The w6f1aa manuk rows were produced across more than one build, so an owner-approved single-build re-run exists (`w6f1ab`, manuk-mcp f58a3334; `bench-queue/out/w6f1ab-manuk.tsv`). The script path was left unchanged to keep the pre-registered arm. Swapping in the w6f1ab manuk arm gives concordance 0.636 (RED), sign ok, manuk sd 1.25 (RED), with manuk full-102 47/50/45. The verdict is RED under either manuk file. The playwright arm of w6f1aa was run on a different day (2026-11-25 02:05) from the manuk arm, which is a caveat on the paired comparison, not on the subset verdict.

**Self-test (can go red):** `f1aa_verify.py --self-test` GREEN: mutant subset (14 most extreme-rate tasks) is RED (conc 0.000, sign DISAGREE); a manuk table with one row dropped is REFUSED (exit 2). Thresholds unchanged.

Consequence: the N=14 subset is not used as a commit gate. The 9-unit held-out evidence of W6.F1.a was not enough; the fresh-round result contradicts it.
