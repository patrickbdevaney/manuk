# TRI-AB-3 — time.com, sentry.io, tailwindcss.com: debug vs release drive-probe (node R0.4.a.c.a.b.a.a.a.a)

Source: `docs/bench/tri-ab-3-20261103-044846.tsv`. Same source tree, one site at a time, `timeout 180 nice -n 15`, each started when 1-min load < 4 (2.03-2.68).
`chrome_ver=145.0.7632.116` on every row. Binaries: `target/debug/drive-probe` (2026-11-02 23:27) and `target/release/drive-probe` (23:21).
Gate: `scripts/tri-ab-gate.sh` (green; red on a no-difference mutant and a busy-load mutant).

| url | debug | release |
|---|---|---|
| time.com | TIMEOUT 180.2 s | OK 20.1 s |
| sentry.io | TIMEOUT 180.0 s | OK 27.1 s |
| tailwindcss.com | TIMEOUT 180.0 s | OK 16.3 s |

## Reading
- All 3 sites differ between binaries: the TIMEOUTs of R0.4.a.c.a.b.a.a.a come from running the **debug** drive-probe, not from a site change or a manuk regression. Release prints a row in 16-27 s, matching TRI-FULL-M2 (tailwindcss 15 s, time 20 s).
- Same cause as the a11y TIMEOUTs (R0.4.a.c.a.b.b). salesforce.com and newrelic.com (170/173 s on debug at load < 4) were not re-run on release here.
## Caveats
- This is debug vs release, not an older-vs-current build; no old commit was built. With release OK there is no regression to bisect.
- One pass per cell. Load sampled before each run only.
