# RG.S1.b.b.b: TZ the real-task runner gives both arms

**Decision:** `TZ=America/Los_Angeles` for BOTH arms, set in the environment of the invocation of
`bench/agent-tasks/real_run.py` (no runner code change). The env is inherited by `claude -p`, the arm's MCP server,
and Chrome/Playwright in each arm, so the two arms get the same zone. The gocalendar evals expect PT (21:00Z = 2 pm PDT).

**Why:** the RG.S1.b.b.a stall was a zone mismatch in the site, not an engine defect (see RG.S1.b.b.a / RG.S1.b.b).
The machine is America/New_York; the site builds times with the machine zone and formats them as PT.

**Measurement:** gocalendar-1, `--judge`, 3 runs per arm, chrome_ver=145.0.7632.116, judge openai/gpt-oss-120b.

| arm | TZ | run 1 | run 2 | run 3 | TSV |
|---|---|---|---|---|---|
| manuk | America/Los_Angeles | 4/4 PASS | 4/4 PASS | 4/4 PASS | S1bbb-pt-manuk.tsv |
| playwright | America/Los_Angeles | 4/4 PASS | 4/4 PASS | 4/4 PASS | S1bbb-pt-playwright.tsv |
| manuk (control) | America/New_York | 0/4 FAIL | - | - | S1bbb-est-control-manuk.tsv |

**Red control:** one manuk run under the machine zone (America/New_York) fails 0/4 with state saved, the same
as RG.S1.b.b.a. The zone is the variable that changes the result.

**Caveat:** this is 3 runs per arm on one task at one pin. Under the 2-of-3 bar, gocalendar-1 passes for both arms.
Other tasks were not re-measured under PT, so their numbers are not comparable across this change.

## RG.S1.b.b.b.a: the runner pins TZ itself

`bench/agent-tasks/real_run.py` now does `os.environ.setdefault("TZ", "America/Los_Angeles")` before it spawns anything, so
every arm (and `claude -p`, the MCP server, Chrome) inherits PT without the invocation env. An explicit TZ in the shell wins.

| arm | shell TZ | result | TSV |
|---|---|---|---|
| manuk | unset | 4/4 PASS | S1bbba-unset-manuk.tsv |
| manuk | America/New_York | 0/4 FAIL (red control) | S1bbba-est-manuk.tsv |

chrome_ver=145.0.7632.116, judge openai/gpt-oss-120b, 1 run each. Runs from before this change on other tasks were taken with the
machine zone (America/New_York); do not diff them against runs after it.
