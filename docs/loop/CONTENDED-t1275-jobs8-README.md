# CONTENDED-t1275-jobs8-rows.tsv — a POISONED sweep, kept as evidence and deliberately un-named

**Do not read this file as a trend point, and do not let `progress-metric.sh` pick it up.** It is
renamed out of the `SWEEP-t*-rows.tsv` glob on purpose.

It was produced at `--jobs 8` because the sweep at `--jobs 2` was projecting 3+ hours and the board's
THROUGHPUT lever sanctions parallelising it. The result reads as a catastrophic engine regression and
is nothing of the kind:

```text
   reason           t1268 (--jobs 2)   t1275 (--jobs 8)
   scored                   107                54
   unreachable               14                35     (+21)
   timeout-150s              11                34     (+23)
   css-starved-*              0                24     (new bucket entirely)
   bot-wall-403              36                30
   ⇒ scorability           ~87%              43.9%
```

`unreachable`, `timeout-150s` and `css-starved` are **precisely** the three failure modes eight
concurrent Chrome+manuk pairs manufacture: saturated network (the fetch never completes), saturated
CPU (the render exceeds the 150s budget), and stylesheets that do not arrive before the render is
sampled. Nothing about the engine changed between the two runs except three landed ticks whose WPT
deltas were all positive.

> **This is `STATUS.md` Lesson 4 firing for the FOURTH time, and the fourth instance is in its own
> words:** *"I widened the crawl from 4 jobs to 12 to make it finish sooner and watched 'the hang
> rate' go from 12.5% to 49% on the same binary in the same hour."* The lesson closes *"it is a
> lesson I could recite while breaking it"* — and it was recited in this very session, in the
> constitution check written an hour earlier, and then broken.

**What IS salvageable from this run** is the one statistic built to survive exactly this: the
drift-robust **common-set band** over the 53 sites scored in BOTH sweeps. Contention removes sites
from the population; it does not systematically bias the ones that still rendered. That band reads
`mean Δshape -0.0063`, `mean Δsite_score -0.0090` (6 up / 11 down, and 7 up / 13 down) — **flat,
inside the noise**, which is a real and unwelcome finding about ~30 ticks of WPT work and is
independent of the concurrency.

**The rule, stated so the fifth instance costs less than this one:** a sweep's `--jobs` is part of
its number. A run at a different job count is a different instrument and may only be compared to
itself. Bank corpus-level headlines from `--jobs 2` only.
