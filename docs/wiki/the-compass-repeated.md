# The compass, repeated

*Tick 1496. The loop has been steering by one sweep for eleven ticks. It has now been run twice.*

## Why

Surface audit #91 ranked this first against itself: `SWEEP-t1485-refusal-tags.tsv` — the attribution
that says the observer's mandate aims at a term worth six sites in two hundred — **had never been
repeated**, and t1410's rule is explicit that *one run refuses nothing*. A compass the loop steers by
and has measured once is a compass on trust.

## The two runs

```text
                        t1485        t1496        spread
  sites                   200          200
  SCORED                  113          118          +5      (56.5% -> 59.0%)

  ORIGIN                   58           60          +2
      bot-wall             36           37
      unreachable          16           17
      http                  5            5
      empty                 1            1
  METHOD                   23           17          -6
      probe-blocked         8            4
      tree-divergence       7            5
      shell-only            4            4
      oracle-module-shell   3            3
      thin-overlap          1            1
  ENGINE                    6            5          -1
      css-starved           5            3
      render-failed         1            2
```

## What holds

**ENGINE is 6 and 5.** The term the mandate calls *the binding constraint* — the shared function/JS/DOM
defects that make sites unscorable — is worth **five or six sites in two hundred**, on two independent
runs a day apart. That is no longer an observation; it is a measurement with a band.

**ORIGIN is 58 and 60, and dominates.** Bot-walls (36/37) plus unreachable (16/17) are two thirds of
every refusal and are out of any engine's reach.

**In-scope scorability** (bot-walls excluded per `DAILY-DRIVER-CERTIFICATION.md` §3) is **113/164 =
68.9%** and **118/163 = 72.4%** — both well below the mandate's quoted ~82%, and the gap is still not a
function gap.

## What moved, and one third of it is this loop's own work

**METHOD fell 23 → 17**, the widest spread of the three, and `probe-blocked` alone went **8 → 4**.
That is what t1486 and t1487 did: the engine and the instrument both learned to follow
`<meta http-equiv="refresh">`, and three redirect-stub sites moved from refused to scored. The
category the audit ranked as the largest addressable term is measurably smaller than when it was
ranked.

⚠ The rest of METHOD's movement is not attributed. `tree-divergence` 7 → 5 is inside what a live-network
sweep can do on its own.

## ⚠ And the first attempt at this sweep was corrupt

The runner was started twice — the first launch failed on a `pid`-file write and was assumed dead —
and the two instances appended to one file. It was caught by the row count: **269 rows for a 200-site
corpus.**

*A row count that can exceed its own corpus is the cheapest possible integrity check*, and it is the
only reason the contaminated run did not become the second data point. Discarded, processes killed by
explicit PID (`pgrep -f` on the runner's own path self-matches the wait loop — t1197's trap), and
re-run once with the top-level runner's PID confirmed.
