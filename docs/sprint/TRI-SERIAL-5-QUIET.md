# TRI-SERIAL-5-QUIET — the 5 TIMEOUT sites, serially, only when 1-min load < 4 (node R0.4.a.c.a.b.a.a.a)

Source: `docs/bench/tri-serial-5-20261103-025816.tsv`. Runner `/tmp/r0aba_a_run.sh` (not committed): before each site it
polls `/proc/loadavg` every 30 s and starts the site only when the 1-min load is below 4. Then
`timeout 180 nice -n 15 ./target/debug/drive-probe <url>`, classified by `scripts/tri-classify.sh`.
`chrome_ver=145.0.7632.116` on every row. Gate: `scripts/tri-serial-quiet-gate.sh` (green; red on a busy-host mutant and on a 2-row file).

## Result

| url | rc | tag | wall s | load1 before | rate | prior serial run (load 12.5-13.6) |
|---|---|---|---|---|---|---|
| salesforce.com | 0 | OK | 169.9 | 3.73 | 56.4 | TIMEOUT |
| time.com | 124 | TIMEOUT | 180.2 | 3.94 | NA | TIMEOUT |
| sentry.io | 124 | TIMEOUT | 180.0 | 3.69 | NA | TIMEOUT |
| tailwindcss.com | 124 | TIMEOUT | 180.0 | 3.57 | NA | TIMEOUT |
| newrelic.com | 0 | OK | 173.0 | 3.28 | 78.1 | TIMEOUT |

## Reading

- **Host load explains 2 of the 5.** salesforce.com and newrelic.com now print OK at 180 s with load below 4; both were
  TIMEOUT at load 12.5-13.6. Both finished close to the 180 s cap (170 s and 173 s), so the margin is thin.
- **Host load does not explain the other 3.** time.com, sentry.io and tailwindcss.com still TIMEOUT at 180 s with load
  3.57-3.94. TRI-FULL-M2 printed tailwindcss.com in 15 s and time.com in 20 s, standalone. Contention from our own sweep and
  host load are both ruled out for these three. The cause is a site change or a manuk regression since TRI-FULL-M2.
  This run cannot separate those. Follow-up: R0.4.a.c.a.b.a.a.a.a.
- Load was 8.2-17 for the first ~70 minutes of the wait (other tenants). The runner's first site started at 03:46 (load 3.73).

## Caveats

- One serial pass, one host state, one binary (`target/debug/drive-probe`, built 2026-11-02, not rebuilt).
- Load was sampled once per site, before the run, not during it. Load at the end of each run was not recorded.
- The runner died once at 02:54 (its launching shell exited). It wrote only a header, which was removed. The rerun used `setsid`.
  The rows above come only from the rerun.
