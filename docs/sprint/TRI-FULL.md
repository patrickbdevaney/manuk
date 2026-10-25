# TRI-FULL — full oracle-corpus tri-sweep (node R0.4.a.c)

Source: `docs/bench/tri-sweep-20261025-054452.tsv` (265 data rows, one per corpus site), run by
`scripts/tri-sweep.sh --m1-timeout 120` on `chrome_ver=145.0.7632.116`. Gate: `scripts/tri-full-gate.sh
docs/bench/tri-sweep-20261025-054452.tsv 265 145.0.7632.116` → green; mutants (wrong count, wrong pin,
one empty m2_tag) → red.

Corpus size: the node was written as "281 sites", but `docs/bench/oracle-corpus.txt` has 265 site rows
(commit f6248a4a). The sweep covers all 265.

## Where the 265 sites stop

| M1 status | sites | meaning (from the instrument) |
|---|---|---|
| OK | 162 | fidelity measured |
| LOW_SAMPLE | 2 | fewer than 10 oracle paths |
| BOTWALL | 42 | origin answered a bot wall |
| TIMEOUT | 31 | fidelity run hit the 120 s limit |
| NAVFAIL | 12 | navigation failed |
| EMPTY | 13 | no structural paths and no visual, no cause on disk |
| BLANK | 3 | origin answered an empty body |

164 reachable (OK + LOW_SAMPLE). The other 101 are tagged SKIPPED for M2 and a11y: they are never driven.

## M2 and a11y on the 164 reachable

| tag | M2 drive | a11y F1 |
|---|---|---|
| OK | 147 | 144 |
| TIMEOUT | 16 | 20 |
| CRASH | 1 | 0 |
| SKIPPED | 101 | 101 |

Headline, as the script prints it (denominator = the 164 reachable sites):

- M2 drive-rate: FULL **63.3%** (n=164, 17 non-OK scored 0); measured-only **70.6%** (n=147)
- a11y F1: FULL **64.4%** (n=164, 20 non-OK scored 0); measured-only **73.4%** (n=144)
- M1 SHAPE over reachable: **62.4%** (n=164)

If the 101 SKIPPED sites are counted as 0 over all 265 sites, the M2 rate is 63.3 × 164 / 265 ≈ **39.2%**.
That is the number to quote when the question is "how much of the corpus can an agent drive", and it is
not comparable to the 10-site numbers in TRI-BAND.tsv.

## Caveats

- One run, not a band. The cited 10-site M1 SHAPE (85.0-85.7) and the measured one (71.3-71.7) differ by about
  14 points. R0.4.a.b.a found the cause: the cited 85.5 is a 3-site mean at the 30 s timeout, and the band is a
  6-site mean at 120 s (`docs/sprint/TRI-BAND-DIFF.md`). The 85.0 and 85.7 values have no file on disk. Treat the
  full-corpus numbers above as a single sample until a 3-run full band is measured.
- M1 timeouts were set at 120 s to match R0.4.a.a; a different timeout changes the reachable set.
