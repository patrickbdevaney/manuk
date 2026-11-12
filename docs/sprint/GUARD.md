# Human-view guard rule (GUARD.R4 and later)

Set by node GUARD.BAND (owner decision 2026-11-06). Implemented in `scripts/guard-band-check.py`; no SUCCESS.md threshold is changed. The old fixed GUARD.R0 min/max bands (`scripts/guard-r3-check.py`) stay as the historical record only.

Run: `python3 -I scripts/guard-band-check.py BASE_ROWS CAND_ROWS` with comma lists of GUARD.tsv row names, e.g. `run1,run2,run3 R3run1,R3run2,R3run3`. Both sets need >=3 same-binary runs on one chrome_ver (the script refuses mixed `chrome_ver`).

1. **Band.** Per metric (cov, shape, m2_full, m2_meas, ax_full, ax_meas): lower edge = min(baseline runs) - max(baseline spread, candidate spread), spread = max-min over a set's own runs. The band is therefore never narrower than the observed run-to-run spread. One-sided: values above are not flagged. The script prints each band next to both spreads and the old fixed lower edge.
2. **Paired criterion.** Sites OK in both sets, per-site values averaged over each set's runs, for cov, shape, m2_rate, a11y_f1: mean paired delta >= -1.0 point. Every site moving >= 10 points on a11y F1 needs a row in `docs/sprint/guard-classes.tsv` (INSTRUMENT / ORIGIN / ENGINE + evidence); an unclassified mover, or an ENGINE mover with a negative delta, is RED.
3. **Verdict.** A band breach is discharged only if its paired metric passes (set metric -> per-site column: cov->cov, shape->shape, m2_*->m2_rate, ax_*->a11y_f1). A paired failure is always RED. Negative >=10-point movers on non-a11y metrics print as WATCH (not RED); each gets a node.
