# Node pack base — the standing context every sprint node needs (digest; full docs only when this is not enough)

## North star (docs/research/WAVE3-SYNTHESIS.md §a)
Playwright MCP already ships most agent ergonomics (snapshot diffs, find, settle budget, click-interceptor naming, WebMCP
tools). What remains for manuk: **be the agent browser whose claims an outsider can check** — whole-engine determinism
(budgets, fetch order, fonts, hash seeds, libm/TZ/locale; not just a JS clock), checkpoint/fork/replay, a hardened
untrusted-content reader, and reproducible published measurements. Prefer work that makes a claim checkable.

## Pre-registered bars (docs/sprint/SUCCESS.md — never edit thresholds, never tune tasks to results)
Read from docs/sprint/SCOREBOARD.tsv, 3-run bands, one pinned chrome_ver, Haiku driving.
- Bar 1: one-line install works; >=25/40 frozen tasks pass for manuk (a task passes if >=2 of 3 runs pass); external slice
  scored and published beside own tasks; recorded public demo with committed transcript.
- Bar 2 (both, outside the band): within 10 points of Playwright MCP+Chrome on the full suite, AND one decisive win:
  hidden-instruction-followed <=5% with Playwright >=20 points worse (external cases included); OR tokens/task >=30%
  fewer at equal-or-better success; OR checkpoint/restore turns >=15% of otherwise-failed tasks into passes.
- Verdicts: Bar1+Bar2 -> launch around manuk; Bar1 only -> MCP layer over Chrome backend; neither -> post-mortem.

## Decided facts (docs/research/SYNTHESIS.md) — do not re-derive
K1 arm 3 (a manuk-style layer over Chrome CDP) is the comparator for any "decisive win". Do-now no-regret items are
independent of K1. Re-open triggers live in SYNTHESIS.md; a dependency hardening verdict is S1/W1B16.

## Constitution invariants (docs/foundation/CONSTITUTION.MD Part I) — a change violating one is rejected
I1 one core, two front-ends (shell+agent). I2 memory-safe Rust, sanctioned deps only, never patch them internally.
I3 the agent surface (DOM, style, layout, AX tree) is core, not a layer. I4 Pareto discipline: real-web breadth by usage,
verified by the differential oracle. I5 differential oracle (Manuk vs headless Chrome) is the discovery engine.
I6 page content is untrusted input, always (DOM text, AX labels, WebMCP manifests are adversarial). I7 honest walls
(no Widevine; process-per-tab; SpiderMonkey trails V8) stated plainly. I8 solo+AI-loop throughput: favor what keeps the
operator's loop fast. Non-goals are in Part IV; open docs/foundation/CONSTITUTION.MD only for the Part you are touching.

## Landing rules (the prompt has the full text)
engine/ or tests/wpt diff -> ./scripts/tick.sh (never wrapped in timeout). Otherwise cargo test touched crates + release
build touched binaries, explicit-pathspec commit, `git commit --no-verify`, secret scan on `git diff --cached`, push origin
main. Graph edits: `python3 scripts/sprint-graph.py add|set|check` (validates 9 columns). Measurement rows carry
chrome_ver=$MANUK_CHROME_PIN. No claim without a SCOREBOARD row. A gate must be shown able to go red.
