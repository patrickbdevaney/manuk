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
