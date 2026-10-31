# W4.P0 engine compass: outcome agreement, manuk vs cdp-layer

chrome_ver=145.0.7632.116 (the pinned CfT build; `MANUK_CHROME_PIN`). Model: Haiku. Script:
`bench/agent-tasks/engine_compass.py` (`--selftest`, `--write`). Table: `docs/sprint/w4p0/COMPASS.tsv`
(one row per task, with a mechanism label for each disagreement).

## Metric

- A task's **outcome** for an arm is PASS when at least 2 of its 3 runs pass (the SUCCESS.md rule).
- **Agreement** is the share of tasks where manuk and cdp-layer reach the same outcome. Same tasks, same model,
  same chrome_ver.
- **Band** is the min..max of the per-run agreement, comparing run r of manuk with run r of cdp-layer.
- Only pass/fail is compared. The trajectory (action log) is **not** compared yet. That is follow-up W4.P0.g.
- Rows: the latest row per (arm, slice, task, run). Own suite = `suite-v1.json` (hash 69903c7d). External =
  `suite-v1-external.json` live rows (hash 061ee22b). REAL = `A6-real.tsv`, suite real-v1-det.

## Result

| slice | tasks | task-outcome agreement | per-run band |
|---|---|---|---|
| own, live | 25 | 24/25 = 96.0% | 96.0% |
| own, fixture | 15 | 15/15 = 100.0% | 93.3%..100.0% |
| external, live | 10 | 6/10 = 60.0% | 30.0%..60.0% |
| REAL | 47 | 35/47 = 74.5% | 70.2%..78.7% |
| **all** | **97** | **80/97 = 82.5%** | |

The agreement is high on our own tasks and low on the external and REAL slices. That is the steering signal:
the engine's real gap lives where the agent's sites are, not in the fixtures.

Limits on the reading:
- Mechanisms below are read from answer text. None is traced in code yet, so each is labelled *suspected* unless
  it is a verifier or cdp-side finding.
- The SCOREBOARD `answer` column is cut at 300 characters (298 of 1080 manuk/cdp rows sit at the cap). REAL's is
  cut at 200. Verdicts were computed on the full answers, so some pass/fail values cannot be re-checked from the
  scoreboard alone (see X01 below).

## Disagreements by mechanism (28 rows: 17 task-level outcome flips, 11 run-level flips)

Ranked by sites x disagreeing tasks, which is the order of the fix queue.

| family | sites | tasks (disagreeing) | mechanism | class |
|---|---|---|---|---|
| click | 4 (dashdish, gocalendar, gomail, staynb) | dashdish-2, -7, -10; gocalendar-1, -3, -4; gomail-5; staynb-2 (8) | a click on a JS-handled card, tile, stepper or button does not do what cdp-layer's click does | suspected engine |
| typeahead | 1 (udriver) | udriver-1, -6, -10, -11 (4) | typing a pickup/dropoff shows only 4 fixed addresses on manuk; cdp-layer selects the same addresses | suspected engine |
| wolfram JS gate | 1 (wolframalpha) | X06, X07, X08 (3) | "Wolfram\|Alpha doesn't run without JavaScript" / no result on the results page | suspected engine |
| cdp-side | 1 (opendining) | opendining-3, -4, -5, -8, -10 (5) | cdp-layer books the right place but evals fail, or menus render empty, or the phone form rejects input | cdp-layer, unclassified |
| scroll | 1 (quotes) | L12 (1) | second quote batch never loads on manuk (3/3); cdp-layer's scroll loads it | suspected engine |
| verifier | 4 sites | X01, X02, X04, X06, F08, gomail-8 | a manuk pass on an answer that reports failure, or a correct answer that fails the exact-string check | verifier |
| site | 1 (cambridge) | X01, X02, X03 | cdp-layer hits a Cloudflare challenge and gets no content | site, not engine |
| flake | 1 (networkin) | networkin-3 | manuk r3 2/3 evals on the same post | unclassified |

### Verifier findings (not engine gaps; they move the pass count)

- **False passes, manuk.** The verifier accepts answers that say the site failed. X06 r1/r3: "the answer below is
  my own calculation, not a Wolfram result" passes on the number. X10 r1/r2/r3: "Wolfram|Alpha did not produce"
  passes on "converges". X05 r3 passes on its own calculation. gomail-8 manuk trashes 1 of 4 GitHub emails and
  passes 3/3. X01 manuk passes three times, but the stored answer (cut at 300 characters) reports that the
  translation never loaded; the expected string `efímero` is not in the stored text, so the full transcript is
  needed.
- **False fails, manuk.** F08 r3 answers "Item 45 is **amber**" (correct) and fails the exact string
  `Item 45: amber`. X04 r2 answers "Monday, 27 March 2023" (correct) and fails `27 Mar 2023`.
- **cdp-side verifier or state mismatches.** opendining-5 r1 and opendining-3 r3 say "booked" and fail evals 0/5.
  opendining-8 r1 booked the right place and fails 0/3.

These are not fixed here. SUCCESS.md forbids editing thresholds or tuning tasks after results, so the audit goes
into a new suite version (W4.P0.e).

## What the compass says about the engine

- Our own suite is not where the engine loses: manuk agrees with cdp-layer on 39 of 40 own tasks.
- The engine loses on real sites through three suspected families (click, typeahead, Wolfram JS). Each traces to
  a fix row, and each is worth a number of tasks in the REAL slice.
- Rank of fix rows: click (sites x tasks = 32) > typeahead (4) > wolfram (3) > scroll (1). The verifier and the
  cdp-side rows are not engine fixes; they are measurement fixes, and they go first in the queue because they move
  the numbers on which the other rows are judged.

## Reproduce

```
python3 bench/agent-tasks/engine_compass.py            # table + disagreements with labels
python3 bench/agent-tasks/engine_compass.py --write    # docs/sprint/w4p0/COMPASS.tsv
python3 bench/agent-tasks/engine_compass.py --selftest # mutant: a dropped label goes red
```

The mutant check is `--selftest`: it flips one manuk run that matches cdp-layer and asserts the run agreement falls
by exactly 1, and it fails if any disagreement lacks a mechanism label. Deleting one label turns it red
(verified: `SELFTEST FAIL: no mechanism label for [('real', 'udriver-1')]`).
