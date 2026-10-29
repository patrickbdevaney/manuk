# Demo: Haiku completes a multi-step live-site task through manuk-mcp

- Suite task: **L21** (own slice, `bench/agent-tasks/suite-v1.json`): start `https://en.wikipedia.org/wiki/Main_Page`,
  "Use the site search to find Alan Turing's article and report his year of birth." Verifier: answer contains `1912`.
- chrome_ver=145.0.7632.116 (pin; manuk itself is not Chrome — the pin tags the measurement set).
- Steps (`L21/steps.txt`): navigate -> find "search input" -> type "Alan Turing" with submit -> find the birth date on the result page -> answer.
- Result: PASS, 6 turns, 26.8 s, $0.0068, Haiku. Answer: "Alan Turing was born in 1912 ... 23 June 1912 in Maida Vale, London."
- Full stream-json transcript: `L21/manuk_L21_r1.jsonl` (secret-scanned: no hits). Score row: `L21/row.tsv`.
- Verifier can go red: same command with `--break-verifier` -> `FAIL answer missing 'ZZ-impossible-answer-ZZ'`.
- Recording: asciinema is not installed on the build host, so the recording is the transcript plus the step log (no terminal cast).

Command:

    MANUK_CHROME_PIN=145.0.7632.116 python3 -I bench/agent-tasks/run_arm.py --arm manuk --tasks L21 --runs 1 \
        --out docs/demo/L21/row.tsv --transcripts docs/demo/L21

Single run, a demonstration, not a statistic; the 3-run numbers are in SCOREBOARD.tsv / V1.3.md.
