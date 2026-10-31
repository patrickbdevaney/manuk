# W4.C3: best-of-3 at a submit act (3 arms, Haiku judge, scripted agent)

chrome_ver=145.0.7632.116; harness `bench/agent-tasks/c3_bon.py`; data `bon.tsv` (3 runs x 8 trials x 2 fixtures per arm).
Fixture: a checkout page whose Submit does `fetch('/roll/...')`; the server lets 1 attempt in 3 succeed (seeded per run/trial,
identical across arms, so arms are paired). single = first attempt. BoN-3 = checkpoint after typing the name, 3 branches each
click Submit, `claude -p --model haiku` sees only the task and the 3 resulting page texts and names a branch.
Arms: manuk (`checkpoint`+`fork`, in-process engine), cdp-layer (same layer code, `--backend chrome`), playwright (prefix replayed by hand).
"ambiguous" fixture: the failure text is "...awaiting payment, not confirmed yet" (judge trap).

| arm | fixture | single | BoN-3 | judge correct | model calls/trial | wall s/trial |
|---|---|---|---|---|---|---|
| manuk | clear | 17% | 58% | 100% | 1 | 2.0 |
| manuk | ambiguous | 17% | 58% | 100% | 1 | 2.1 |
| cdp-layer | clear | 17% | 58% | 100% | 1 | 10.9 |
| cdp-layer | ambiguous | 17% | 54% | 96% | 1 | 10.4 |
| playwright | clear | 17% | 58% | 100% | 1 | 5.6 |
| playwright | ambiguous | 17% | 58% | 100% | 1 | 4.6 |

Per-run BoN-3 successes of 16 (runs 1/2/3): manuk 8/12/8, cdp-layer 8/11/8, playwright 8/12/8. Single: 0/4/4 on every arm.
The cdp-layer 54% is one trial that errored (row with wall_s -1, counted as a failure), not a judge miss.

## Reading (only what the numbers support)
- Gate met: manuk BoN-3 beats single-run by +41 pt on `clear` (>=5). Mutation (`MANUK_C3_MUTATE=1`, judge always picks branch 1) -> 17% = single, GATE FAIL.
- **This is a LAYER win, not an engine win.** cdp-layer and playwright (hand-replayed prefix) land inside manuk's run band
  (8/12/8 vs 8/11/8 vs 8/12/8). The kill rule applies: the engine claim is not made.
- The engine has no live-heap fork. `fork` is replay-based (mcp/src/hub.rs: "not a memory snapshot"), so manuk and cdp-layer run the same
  algorithm; manuk is faster per trial (2.0 s vs 10.9 s, 3 forks + page loads) but that is page-load speed, not a fork capability.
- Model cost of BoN-3 here: +1 Haiku call per submit (judge), plus 0 extra agent turns. Real-model branches (3 sampled actions) would cost 3x agent tokens: not measured.
- Limits: the variance source is a server roll over an unfenced GET, which every arm re-rolls; the branches run the SAME action, so this
  measures judge+fork plumbing, not whether a model's sampled alternatives help. Not the REAL x3 LLM run the node's gate names: follow-up W4.C3.a.
  Client-side nondeterminism (Math.random under a seeded engine: do the forks all roll the same?) is untested: follow-up W4.C3.b.
