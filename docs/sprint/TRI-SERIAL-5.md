# TRI-SERIAL-5 — do the 5 sites that still TIMEOUT at 180 s with 2 jobs print when run one at a time? (node R0.4.a.c.a.b.a)

Source: `docs/bench/tri-serial-5-20261103-003801.tsv`, from `/tmp/r0abab_run.sh` (not committed as a script): for each
site in turn, `timeout 180 nice -n 15 ./target/debug/drive-probe <url>` with the classifier from
`scripts/tri-classify.sh`. Load recorded from `/proc/loadavg` (1-min) immediately before each site. `chrome_ver=145.0.7632.116`
on every row. Sites: salesforce.com, time.com, sentry.io, tailwindcss.com, newrelic.com (the 5 TIMEOUT rows of
`TRI-LOAD-SLOW-16.md`).

## Result

| url | rc | tag | wall s | load1 before | rate | standalone in TRI-FULL-M2 |
|---|---|---|---|---|---|---|
| salesforce.com | 124 | TIMEOUT | 180.1 | 12.53 | NA | 78 s |
| time.com | 124 | TIMEOUT | 180.2 | 13.10 | NA | 20 s |
| sentry.io | 124 | TIMEOUT | 180.0 | 13.29 | NA | 29 s |
| tailwindcss.com | 124 | TIMEOUT | 180.0 | 13.49 | NA | 15 s |
| newrelic.com | 124 | TIMEOUT | 180.0 | 13.64 | NA | 50 s |

5 of 5 TIMEOUT at 180 s, run serially, with no manuk job of this sprint running alongside (the 2-job sweep had ended).
Per the node's rule, a site counts as contended only if it prints OK serially at 180 s; none did.

## Reading

- **Contention from our own 2-job sweep does not explain these 5.** Serially they still do not finish in 180 s.
- **Host load is high and was not controlled.** It ran 12.5 to 13.6 on 16 cores, from other tenants on the box, and rose across the five runs.
  The standalone runs in TRI-FULL-M2 printed in 15-78 s, so these sites printed when the box was quieter (not measured here).
  The cause is therefore one of: (a) host load from other tenants, (b) the sites' own response time changed since TRI-FULL-M2,
  (c) a manuk regression since TRI-FULL-M2. This run cannot separate these. It only rules out "2 jobs of our own sweep".
- The M2 gap on these 5 sites is not a budget artefact under the current host state. Raising the timeout further is not justified by this data.

## Caveats

- One serial pass, one host state. Load was not controlled and was high throughout.
- The binary is `target/debug/drive-probe` built 2026-11-02, the same binary as the 20261103-001932 sweep. It is not rebuilt here.
- No A/B against the standalone run at the same load, so the "standalone" column is cited from TRI-FULL-M2, not re-measured.

## Follow-up

R0.4.a.c.a.b.a.a: re-run the same 5 sites serially at 180 s only when 1-min load is below 4, to test whether the host load drives the TIMEOUTs.
