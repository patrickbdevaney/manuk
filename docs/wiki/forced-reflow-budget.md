# A budget that can only be checked where the overrun is not

> Landed t1408. Gate: `a_thousand_forced_reflows_in_one_task_cannot_run_forever`
> (`engine/page/tests/g_forced_reflow_budget.rs`), 3 arms, red under 4 mutations.

`timeout-150s` was the largest engine-owned bucket on the t1406 corpus sweep (9 of 200 sites), and an
unscored site is a **zero** — the M1 ceiling rather than a point on it. `morikoshi.net`:

```text
  load phase "cascade+layout+blocking scripts"   191,235 ms    (every other phase: seconds)
  event loop hit its TIME budget: count=1 elapsed_ms=125235 budget_ms=5000
                                  reflow_n=1054  reflow_ms=124477   ← 99.4% of ONE task
  89 x SLOW FORCED REFLOW ~2,600 ms each   cascade_ms ~550  layout_ms ~2,000  nodes=4375
```

## ⭐⭐⭐ Two guards, both correct, both blind

* The **drain's time budget** is read on a **task boundary**. This is one task.
* The **`ScriptDeadline` watchdog** interrupts **JS**. This time is spent in **Rust**, inside the
  native geometry read, where no interrupt point exists.

> **A budget that can only be checked where the overrun is not is not a budget.** When a cost crosses a
> boundary — JS → Rust, task → within-task — ask which side each guard lives on.

The forced reflow now carries the drain's OWN number (`manuk_js::drain_budget_ms`, shared rather than
re-invented, exported for both build configurations) as a cumulative per-script-round budget. Past it,
a geometry read is answered from the layout already published.

```text
  morikoshi.net                     before       after
  phase cascade+layout+scripts  191,235 ms   11,185 ms    17x
  elapsed at the load event     210,019 ms   27,794 ms
  reflow_ms in that task           124,477       5,805
  the run                        TIMED OUT   COMPLETED
```

⚠ **Not performance bought with capability.** These pages score zero today — no geometry, no paint, no
DOM. The bound turns *"no answer after 150 s"* into *"an answer from a layout a few mutations old"*,
which is the doctrine the drain already states everywhere else. A page inside the budget is
bit-identical.

## ⭐⭐⭐ The gate passed under four mutations first, including deleting the fix

The first version drove a tight `measure → mutate → measure` loop at the production budget and went
green under all four mutations. It was measuring the `ScriptDeadline`: **reflow time is a subset of
script time and both budgets are the same number**, so a JS loop with frequent interrupt points is
always terminated by the deadline first. The real site reaches this bound because its task spends its
time in Rust — which a fixture cannot arrange.

> **A fix whose only witness is a real site is not yet gated.** `MANUK_REFLOW_BUDGET_MS` exists so the
> gate can set the budget small and the mutations bite. The seam is the deliverable, not a convenience.

The control arm needed the same treatment: `reads:6` still passed with an over-eager budget, because
six answers from one stale layout are still six answers. Each iteration now changes the padding and the
arm asserts **`distinct:6`** — six reads, six different heights.
