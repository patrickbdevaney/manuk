# TOKENS.1.b — where manuk loses to Playwright on tokens, per task

chrome_ver=145.0.7632.116 (`$MANUK_CHROME_PIN`), suite 69903c (own 40), haiku, 3 runs per arm.
Produced by `python3 docs/sprint/tokens1b/top10.py` (exits 1 on empty input; tested with a non-matching pin).
Output: `top10.tsv`, the 10 tasks with the largest manuk-minus-Playwright tokens.

**Sources.** Playwright = SCOREBOARD rows dated 2026-10-24 (R2.1.6). Manuk = SCOREBOARD rows dated 2026-10-25 (R2.1.6,
the same code as the baseline). Dominant bucket = tool-result bytes by tool, from `docs/sprint/tokens1/transcripts/`.
Those transcripts come from the TOKENS.1 rerun (2026-10-29), whose code change was reverted, so the buckets are a
close proxy, not the same runs. The rerun's per-task means are in the `manuk_tokens1_rerun_mean` column as a check.

**Token definition.** `tokens = input + output + cache_create`. The notes' "in+out" is not reproducible from
SCOREBOARD columns: the R2.1.6 manuk rows average 13.6k on this definition, not 31.1k as TOKENS1.md states. The
`delta_incl_cache_read` column adds cache_read, which dominates the total and moves the ranking, so read it with care.

| task | manuk | playwright | delta | dominant bucket |
|---|---:|---:|---:|---|
| L19 | 58,944 | 6,541 | +52,403 | Read (71% of result bytes): an oversized extract spilled to a file, then read back |
| L18 | 47,108 | 9,208 | +37,900 | Read (76%): same spill-and-Read pattern |
| L21 | 27,420 | 8,934 | +18,485 | Read (50%) |
| L16 | 22,016 | 5,166 | +16,850 | navigate (93%) |
| L17 | 24,157 | 7,718 | +16,439 | extract (47%) |
| L24 | 21,120 | 5,311 | +15,809 | extract (72%) |
| L22 | 20,410 | 5,624 | +14,786 | extract (72%) |
| L20 | 23,371 | 8,832 | +14,539 | Read (80%) |
| F08 | 14,093 | 5,721 | +8,372 | extract (72%) |
| L12 | 12,982 | 5,202 | +7,780 | extract (72%) |

**What it says.** The largest gaps are Read and extract results. The error text from `extract` on a 78k-character
Wikipedia page reads "Output has been saved to …", and the agent then pages through that file with Read (L19, L18),
which is the spill that R2.1.5.a.a already tracks. Run-to-run noise is large: F08 is 14.1k in the baseline and
5.7k in the rerun, so single-task deltas under ~5k are not separable from noise on 3 runs.
