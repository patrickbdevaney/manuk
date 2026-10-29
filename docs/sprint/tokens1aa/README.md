# TOKENS.1.a.a — manuk vs cdp-layer (arm 3), task by task

chrome_ver=145.0.7632.116. manuk = R2.1.6 baseline transcripts (docs/sprint/r216), cdp-layer = docs/sprint/arm3/transcripts-full,
own-slice tasks, 3 runs each (120 runs per arm). Reproduce: `python3 -I docs/sprint/tokens1aa/diff.py` (writes bytes.tsv, tasks.tsv; exits 1 on a missing arm or no common task).

Same tool surface, same ToolSearch cost (1.02 vs 1.05 calls/run, ~225 bytes): the prefix is shared, as R4.1 said. The gap is elsewhere.

| per run, own slice | manuk | cdp-layer |
|---|---|---|
| in+out+cache_create tokens per row (SCOREBOARD) | 13.6k | 10.4k |
| turns (SCOREBOARD) | 6.9 | 5.0 |
| extract calls | 1.12 | 0.23 |
| Read result bytes | 1,960 | 145 |
| extract result bytes | 1,985 | 859 |
| navigate result bytes | 7,944 | 10,217 (cdp is larger) |

Cause (engine-side): `extract` text is one line. `shield.text` (agent/src/shield.rs:215) is `split_whitespace().join(" ")`, so a Wikipedia page
comes back as "78,274 characters across 4 lines" (manuk, 16 spill errors) vs "76,293 characters across 1,225 lines" (cdp-layer, 12). Claude Code spills any
extract result over the token cap to a file; the model then pages it with `Read` (line-based). On a 1-line file Read returns the whole 78k line or fails and the
model falls back to `Bash cut -c`, so each spill costs extra turns (L18 10.3 vs 9.3 turns, 92.5k vs 41.4k result bytes; L19 66.6k vs 41.7k; L24 30.6k vs 18.5k).
On cdp-layer's multi-line file Read with offset/limit returns just the wanted section. navigate is not the gap (manuk is smaller).
Also: manuk extracted more calls per run because the first `extract` after navigate is the only way to read page text it did not get from navigate's output.

Follow-up: keep block boundaries as "\n" in Shield.text (and visible_text's joining rule) so spilled extract files are line-addressable. Gate in TOKENS.1.a.a.a.
