# TRI-A11Y-TIMEOUT — why a11y-score TIMEOUTed on 14 of 15 sites in the 16-site run (node R0.4.a.c.a.b.b)

Cause: `scripts/tri-sweep.sh` defaulted `AX` to `target/debug/a11y-score`. The debug build is CPU-bound:
`a11y-score https://github.com/` at 90 s printed no row (user 86.5 s, rc timeout) at load ~13.1; the release build on the same
site, same load, printed F1 89.5% in 17.3 s (user 12.0 s). Default changed to `target/release/a11y-score`.

Re-run: `scripts/tri-sweep.sh --corpus docs/sprint/TRI-LOAD-SLOW-16.corpus --jobs 2 --m1-timeout 120 --no-m2`, load 13.7-14.5,
`docs/bench/tri-sweep-20261103-011404.tsv`: a11y 13 of 13 reachable OK at 90 s (F1 FULL 75.6%), 0 TIMEOUT. M1 reached 13 of 16
this time (time.com among the non-reachable), so the "of 15" in the gate is 13 reachable; threshold 12 met.
Gate: `scripts/tri-a11y-gate.sh` (green 13>=12; red on tri-sweep-20261103-001932.tsv with 1 OK, red on a mutant with all OK -> TIMEOUT).
Caveat: wikihow.com a11y F1 is 0.4% (OK tag, a real low score, not a timeout). `a11y-score` release binary dated Nov 1.
chrome_ver=145.0.7632.116
