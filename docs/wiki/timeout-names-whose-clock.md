# A guard that fires on a clock cannot name a cause — and will name whoever the author had in mind

> Landed t1409. Gate: `a_site_timeout_names_whose_clock_it_burned`
> (`tests/wpt/tests/g_timeout_names_whose_clock.rs`), 4 arms, red under 3 mutations.

`timeout-150s` was the largest ENGINE-OWNED unscored bucket on the t1406 corpus sweep — 9 of 200
sites — and it is the row that decides which engine work the next ten ticks buy. One of the nine:

```text
  swiftspinus.com   our whole load, every phase         5.7 s
                    Chromium's headless screenshot      8.6 s
                    the process while it "timed out"    sleeping in hrtimer_nanosleep
                    the row we filed   timeout-150s: "this engine did not finish"
```

Nothing of ours was slow. The site-level watchdog fires on a wall clock and hard-codes the engine's
name into the reason string, for every site it kills.

⚠⚠ **The machinery to say it honestly already existed, twice.** t861 built
`Unmeasurable::OracleTimeout` for exactly this, and the per-side timing *twenty lines below the
watchdog* opens with *"time each engine separately, and attribute the cost to whoever actually spent
it."* The watchdog consulted neither.

## The fix, and the way this class of fix usually fails

`SITE_SIDE` publishes which side is running; the watchdog asks `fidelity::timeout_reason(side, secs)`.

The oracle span is closed by a **`Drop` guard**, not by a matching store — the oracle block has four
`continue` arms, and a store on the happy path alone would leave the flag reading ORACLE for the whole
of the next site. **The first mis-attribution replaced by a second** is the usual way an attribution
fix goes wrong.

```text
  swiftspinus.com  --site-budget 60  ->  oracle-timeout-60s   "the ORACLE was running"
  morikoshi.net    --site-budget 25  ->  timeout-25s          "this engine did not finish"  ← CONTROL
```

## Two rules that pull in opposite directions, and both are asserted

* **Name the oracle**, or the ranked backlog keeps buying engine ticks for a defect in the reference.
* **Keep counting the site**, or "the oracle failed" becomes a licence to launder every hard site out
  of the denominator — the `EXCLUDED-RISING` failure the fixed denominator exists to forbid.

Side `2` — scoring and probing, which is *neither* engine — is therefore filed **against us**. There
is no honest tag for the instrument's own cost yet; inventing one would let it leave the denominator.
It is named in the message so it can be measured rather than laundered.

## The session's third instrument confidently wrong about whose cost it measured

* t1405 — a live page's churn scored as engine error (chrome-vs-chrome 85.3%).
* t1407 — a stale stored row scored as this tick's work (same binary, both ways, identical).
* t1409 — a wall-clock timeout scored as the engine's, while the engine was idle.

Each was found the same way: **by asking the thing itself rather than reading its label.**
