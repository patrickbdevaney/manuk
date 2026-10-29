# TOKENS.1 — navigate cap + shorter tool descriptions: NEGATIVE result (reverted)

chrome_ver=145.0.7632.116, suite 69903c (own 40), manuk arm, 3 runs, haiku; before = R2.1.6 rows, after = docs/sprint/tokens1/rows.tsv.

Baseline attribution (R2.1.5.a, 29 tasks): navigate 8.1 KB/call = 21.8% of input-side tokens; schema prefix 59%.
Change tried: navigate budget 8000 -> 1500 tokens, tool descriptions trimmed (mcp/src/server.rs, session.rs).

| | pass per run | in+out tokens per run |
|---|---|---|
| before (R2.1.6) | 39 / 39 / 40 | 31.1k avg |
| after | 38 / 37 / 37 | 34.2k avg |

Navigate result bytes fell (2.5 KB/call avg, 142 calls) yet tokens rose and pass fell: L12, F08 fail every run, F10/F11 once; the agent
compensates with more extract/Read calls (extract 152 calls, 274 KB; Read 20 calls, 486 KB). Gate (tokens down, pass in band) NOT met,
so the code change was reverted. Tool descriptions and the cap were changed together; their effects are not separated.

Table (after):

### manuk: 120 runs

| bucket | calls | tool-result bytes | attributed cache_create tokens | attributed input-side tokens | share |
|---|---:|---:|---:|---:|---:|
| prompt+system+tool schemas |  |  | 269303 | 5904302 | 33.8% |
| navigate | 142 | 360244 | 202486 | 3595664 | 20.6% |
| ToolSearch | 122 | 24496 | 66889 | 3079310 | 17.6% |
| model output |  |  | 40080 | 1436629 | 8.2% |
| extract | 152 | 274289 | 137984 | 1252438 | 7.2% |
| act | 104 | 65946 | 43323 | 688394 | 3.9% |
| type | 40 | 55071 | 31917 | 473474 | 2.7% |
| Read | 20 | 485550 | 198669 | 330310 | 1.9% |
| find | 54 | 21379 | 17315 | 290702 | 1.7% |
| observe | 14 | 38260 | 21440 | 159872 | 0.9% |
| scroll | 25 | 2922 | 3783 | 151221 | 0.9% |
