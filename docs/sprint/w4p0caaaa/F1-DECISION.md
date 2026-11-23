# F1 statistic decision (W4.P0.c.a.a.a.a.b)

Question: should `scripts/verify.sh` F1 (cascade/mid <= 0.55) take the median of 3 invocations instead of one?

**Decision: KEEP the single-invocation statistic and the 0.55 floor. `scripts/verify.sh` is unchanged.**
(chrome_ver=145.0.7632.116; data: `F1-spread.tsv`.)

Reasoning, from the measured table only:
- Observed exceed rate under load: 3/10 invocations (p_hat = 0.30; ambient 2/5, loaded 1/5).
- If invocations are independent at that rate, a median-of-3 fails when >=2 of 3 exceed: 3p^2 - 2p^3 = 0.216.
  That is 0.30 -> 0.22: it triples the bench cost of the wall for a small gain, and the flake stays.
- The exceed values (0.62, 0.81, 0.75) are not marginal, so the noise is not "near the line"; a median
  statistic does not address it. The cause is the host (load1 6.5-17 from other tenants, see
  `ambient-wait.log` and the BLOCKED node W4.P0.c.a.a.a.a.a), not the cascade.
- No quiet-host row exists (0 rows at load1 < 2.0), so whether F1 flakes on an idle machine is UNMEASURED.
  Changing a binding floor's statistic on loaded-host data would tune the instrument to the noise.
- The mutant requirement applies only if the rule changes; no rule change, so no verify.sh mutant.

Re-open condition: if W4.P0.c.a.a.a.a.a lands >=5 quiet rows (load1 < 2.0) and any exceed 0.55, F1 is
flaky on the code itself and a statistic change (with a deliberately flaky mutant going red) is warranted.
If all quiet rows pass, the flake is environmental and a wall failure on a loaded host should be re-run on
a quiet one, not "fixed" in verify.sh.

## Quiet-host result (W4.P0.c.a.a.a.a.a, 2026-11-23T04:33Z, chrome_ver=145.0.7632.116)

5 runs labelled `quiet-load1<2` in `F1-spread.tsv`. Every run started only after its own start-of-run load1 was
below 2.0 (`QUIET_EACH=1` in `f1-spread.sh`; load1 1.89-1.90, load5 3.24-3.26). cascade/mid =
0.37, 0.37, 0.37, 0.50, 0.37: **5/5 PASS** (ambient load ~8.8: 0.43-0.81, 2/5 exceed).
Large cascade 11.3-15.4 ms on the quiet host, against 15.2-25.8 ms at ambient load.
So the re-open condition resolves to "environmental": the decision above stands. A red F1 on a loaded host
should be re-run on a quiet one; verify.sh is unchanged. Caveat: load5 was still ~3.2, so this is a
quiet-ish window rather than a fully idle host. The binary was target/release/manuk-wpt built 2026-11-22 06:45,
the same binary as the loaded rows, so the comparison is like-for-like.
