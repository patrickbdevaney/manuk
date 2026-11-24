# PERF.1.2.b.b.a.b: arrival-order dynamic scripts, paired A/B on puentedemando

chrome_ver=145.0.7632.116 · host load average ~32 during the runs (shared machine) · 6 paired runs, arm order alternated per run
(`paired.jsonl`). Arm A = `ArrivalScheduler` (async scripts run on arrival, `async=false` in document order); arm B = the same
binary with `MANUK_DYN_SCRIPTS_ORDERED=1` (old all-document-order barrier). Patch: `arrival-scheduler.patch`.

| arm | fetch_wait Σ median (s) | eval median (s) |
|---|---|---|
| arrival (A) | 2.330 | 0.477 |
| ordered (B) | 2.283 | 0.491 |

Drop = -2% (gate: >= 20%). Not met. The unpaired 3x20 sweep (`runs.jsonl`) read puentedemando 2.342 vs the 1.099 baseline in
perf1-dyn: that is the host load, not the change (arm B, unchanged code, reads the same 2.3 s). Sum check in that sweep was RED
(scrapethissite named 0.948) for the same reason; no claim is taken from it.

Scheduler unit tests: async arrival order, async=false document order, mixed; red under a mutant that ignores the flag (2 of 3 fail).

Conclusion: with the machine this loaded the ordered barrier is not the dominant wait; the GTM -> banner chain is serial by
dependency. The change is correct but shows no measurable win here, so it was not landed. Re-measure on a quiet host before deciding.
