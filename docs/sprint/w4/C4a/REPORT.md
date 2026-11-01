# W4.C4.a: action receipts on vs off, REAL-47 (Haiku, 3 runs/task)

chrome_ver=145.0.7632.116; harness `bench/agent-tasks/c4a_report.py`; off-arm data `real-runs-*-no-receipts.tsv` (`MANUK_NO_RECEIPTS=1`, same `manuk-mcp` binary);
on-arm data = `docs/sprint/w4/C3/real-runs.tsv` (the receipts-on manuk / cdp-layer rows; same pin, same binary, runs taken earlier the same day, not interleaved). Output: `report.txt`.

| layer | arm | per-run pass of 47 | band | tasks passing >=2/3 | failed-without-error runs |
|---|---|---|---|---|---|
| manuk | receipts on | 12,11,11 | 11-12 | 13 | 107/141 |
| manuk | receipts off | 10,11,11 | 10-11 | 11 | 109/141 |
| cdp-layer | receipts on | 19,18,18 | 18-19 | 19 | 82/141 |
| cdp-layer | receipts off | 18,18,19 | 18-19 | 17 | 80/141 |

Gate (on band wholly above off band): **RED** on both layers; bands overlap. Mutations: swapped on/off -> RED; off pass zeroed (broken input) -> GREEN, so the gate can fire.
Reading: on REAL's end-state-graded tasks receipts show no measurable change in pass count or in "finished without error but wrong end state" runs. The +54 pt of W4.C4 holds only on the synthetic silent-failure fixture; it does not carry to REAL. Caveat: the on/off arms were not interleaved in time.
