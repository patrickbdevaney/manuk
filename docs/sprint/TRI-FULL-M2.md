# TRI-FULL-M2 — why 17 of 164 reachable sites scored 0 in M2 FULL (node R0.4.a.c.a)

Sweep: `docs/bench/tri-sweep-20261025-054452.tsv` (M2 timeout 90 s, 4 jobs, `nice 15`). Its per-site stderr
logs were lost: `scripts/tri-sweep.sh` writes them to a fixed scratch dir that later sweeps overwrote (0 logs
newer than the sweep survive). So each of the 17 sites was **re-driven once** with stderr kept
(`docs/sprint/TRI-FULL-M2-evidence.txt`, same binary, same 90 s limit, 4 in parallel, chrome_ver=145.0.7632.116).

Result: **16 of 17 TIMEOUTs did not reproduce**; each printed its row in 10-85 s with exit 0 and no hang. They are
tagged `LOAD-SLOW`: the sweep's 90 s budget under 4 jobs plus concurrent M1/a11y work is too tight for them.
The re-drive cannot say whether the time went to fetch or to layout/JS (no stage timing is printed), so that split
is **unknown**; rottentomatoes (85 s), salesforce (78 s) and wix (61 s) sit closest to the limit.
**1 of 17 is a real engine fault**: wired.com aborts with a stack overflow (exit 139), reproduced.

Consequence for the 63.3 vs 70.6 gap: 16 of the 17 zeros are a timeout-setting/load artefact, not an engine bug;
one (wired.com) is an engine bug. This is one re-run, not a band.

| url | sweep tag | re-drive tag | cite (verbatim line in TRI-FULL-M2-evidence.txt) |
|---|---|---|---|
| https://github.com/ | TIMEOUT | LOAD-SLOW | https://github.com/ 0 54 |
| https://nasa.gov/ | TIMEOUT | LOAD-SLOW | https://nasa.gov/ 0 32 |
| https://reactjs.org/ | TIMEOUT | LOAD-SLOW | https://reactjs.org/ 0 18 |
| https://rottentomatoes.com/ | TIMEOUT | LOAD-SLOW | https://rottentomatoes.com/ 0 85 |
| https://bonappetit.com/ | TIMEOUT | LOAD-SLOW | https://bonappetit.com/ 0 37 |
| https://salesforce.com/ | TIMEOUT | LOAD-SLOW | https://salesforce.com/ 0 78 |
| https://sentry.io/ | TIMEOUT | LOAD-SLOW | https://sentry.io/ 0 29 |
| https://berkeley.edu/ | TIMEOUT | LOAD-SLOW | https://berkeley.edu/ 0 10 |
| https://newrelic.com/ | TIMEOUT | LOAD-SLOW | https://newrelic.com/ 0 50 |
| https://wikihow.com/ | TIMEOUT | LOAD-SLOW | https://wikihow.com/ 0 15 |
| https://time.com/ | TIMEOUT | LOAD-SLOW | https://time.com/ 0 20 |
| https://pagerduty.com/ | TIMEOUT | LOAD-SLOW | https://pagerduty.com/ 0 14 |
| https://newyorker.com/ | TIMEOUT | LOAD-SLOW | https://newyorker.com/ 0 37 |
| https://tailwindcss.com/ | TIMEOUT | LOAD-SLOW | https://tailwindcss.com/ 0 15 |
| https://wired.com/ | CRASH | ENGINE-STACK-OVERFLOW | fatal runtime error: stack overflow, aborting |
| https://gizmodo.com/ | TIMEOUT | LOAD-SLOW | https://gizmodo.com/ 0 20 |
| https://wix.com/ | TIMEOUT | LOAD-SLOW | https://wix.com/ 0 61 |
