# TOKENS.1.a — which lever moved tokens? (NEGATIVE: neither variant kept)

chrome_ver=145.0.7632.116, suite 69903c (own 40), manuk arm, haiku, 3 runs each. Binaries were built from the working tree with one
change each (code reverted; nothing in mcp/ is changed by this node). Summary: `python3 -I docs/sprint/tokens1a/summarize.py docs/sprint/tokens1a/desc docs/sprint/tokens1a/cap`
(exits 1 on a missing/empty rows file).

| variant | pass per run | in+out tokens per run | avg | ToolSearch calls (120 runs) |
|---|---|---|---|---|
| baseline R2.1.6 | 39 / 39 / 40 | 31.1k avg | 31.1k | (not counted) |
| TOKENS.1 both changes | 38 / 37 / 37 | 33.6k / 36.7k / 32.3k | 34.2k | 122 |
| ONLY shortened descriptions (navigate cap 8000) | 37 / 37 / 39 | 34.0k / 33.9k / 32.8k | 33.5k | 111 |
| ONLY navigate cap 4000 (descriptions unchanged) | 38 / 39 / 38 | 34.9k / 31.2k / 31.8k | 32.6k | 124 |

Gate (keep a variant only if tokens fall and pass stays in 39-40): neither met. Both variants raised tokens vs 31.1k and lowered pass;
shortened descriptions alone cost 2 pass at the low end (37/37), the 4000 cap alone is closest (32.6k, 38-39) but still above baseline
and the per-run band (31.2k-34.9k) overlaps baseline's. Neither change is the token lever; ToolSearch calls stay ~1 per run (111-124)
regardless, so they come from the harness's deferred tool loading, not from description length.
Caveat: binaries included the uncommitted agent/src/lib.rs WebSocket-pump WIP of another node (not tokens-related).
