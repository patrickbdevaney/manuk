# CONCURRENCY-PLAN: parallel agents, with the browser separated from the model

_Written 2026-11-24, before any result exists. Thresholds are not edited after the first row. Every row carries
chrome_ver, commit, arm, N, model and host load. Results are published whether or not they favour manuk._

## Question
With many agents working at once on one machine, what does each browser cost (memory, CPU, time), and how many tasks
succeed? How much of any difference comes from the browser rather than from the model?

## Arms
| arm | isolation | why it is here |
|---|---|---|
| engine-proc | one `manuk-mcp --backend engine` per agent | the lightweight default |
| engine-fork | sessions forked with `--zygote` / `--zygote-after-load` | the density case |
| chrome-mcp | one `manuk-mcp --backend chrome` per agent | the shipped product |
| pw-proc | one Playwright MCP `--isolated` per agent | how a separate MCP server per agent runs |
| pw-shared | one Chrome, one isolated context per agent | Chrome's cheapest arrangement; must be included |

## Layer 1: no model (the browser on its own)
A scripted driver replays fixed action traces (own suite, plus frozen snapshots of real pages) through each arm, at
N = 1, 5, 10, 25, 50, 100 agents, 3 runs, arms in randomised blocks. No model, so no model limits or token costs.
- Measured: task success, successful tasks per minute, p50/p95 latency per action, peak PSS of the whole tree, CPU
  seconds, and cold start. Each trace must be shown to be feasible on at least one arm before it counts.
- Fixed budget test: an 8 GB cgroup (`MemoryMax`, `MemorySwapMax=0`). The largest N that keeps success within 5 points
  of that arm's N=1 is its **agent capacity**.

## Layer 2: many models (the browser seen through different drivers)
Same tasks, prompt and tool budget on every arm. Models are **blocks**: arms are interleaved within a model, so quota
and time-of-day effects hit every arm. Drivers: Sonnet, Opus (interleaved) and Haiku through `claude -p`, Gemini through
`agy`, one GroqCloud model, and at least two OpenRouter free models. Free tiers have rate limits, so this layer runs at
low N and makes no concurrency claim.
- Primary: paired difference in success, arm minus pw-proc, **within each model**, over all tasks, with a bootstrap 95%
  interval over tasks. Also reported: the model x arm interaction, tokens, turns, dollar cost and wall time.
- Secondary: the same over tasks whose Layer 1 trace passes on that arm. Tasks are never removed because a model failed
  them.
- A browser effect is claimed only if its sign agrees in at least 4 model blocks and the pooled interval excludes 0. An
  effect in only one model is reported as a model interaction.
- Rate-limit and provider errors are infrastructure failures: re-run on every arm of that block, never scored as task
  failures on one arm.

## Bars, fixed now
- **Capacity win:** agent capacity of engine-proc or engine-fork is at least 1.5x the best Chrome arm, **including
  pw-shared**, on real-page traces.
- **Cost win:** peak PSS per successful task at N=50 is at least 30% lower than the best Chrome arm.
- **Success:** within 5 points of the best Chrome arm at the same N. A capacity or cost win without this does not count.
- Against one Chrome process per agent the same numbers are reported separately and labelled that way.

## Rules
- Keys: `GROQ_API_KEY` and `OPENROUTER_API_KEY` come from the owner's environment for the run. A missing key is a refusal
  (exit 3, no rows). No agent looks for keys, reads `.env`, prints a key, or reuses the judge's key. The GroqCloud block
  does not run while a judged batch is using that quota.
- Every run is caged with `MemoryMax` and runs at >= 20 GB available. Chrome keeps its sandbox on.
- Fix or explain the dashdish memory growth (DASH.MEM.1) before any real-page result is quoted.
- The host is shared. Load is recorded, and arms are interleaved rather than run on different days.
