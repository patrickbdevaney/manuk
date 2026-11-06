# A loaded page can be forked, if the process has one thread when it forks

R3.4 experiment (`scripts/r3-4-fork-gate.sh`, probe `agent/src/bin/fork-probe.rs`). One process loads a
page, quiesces, then `fork()`s children; each child acts differently and reports its a11y digest.

What broke, in the order it was found:

* **Stock SpiderMonkey hangs every child.** It keeps its own "JS Helper" threads; `fork()` copies only
  the caller, so the first helper task a child waits on never finishes (naive fork: 4/4 and 8/8 hung).
* **`JS::SetHelperThreadTaskCallback` fixes it, but only after `JS_Init`.** Called before, it segfaults
  (the helper-thread state does not exist yet). `forkpool::install()` runs right after `JSEngine::init`.
  The pool is ours, so `forkpool::quiesce()` drains and joins it (and the script watchdog) before the
  fork and `resume()` restarts both in the parent and in every child. Opt-in: `forkpool::enable()`.
* **`Math.random` replays the parent's stream in every child.** The xorshift state is heap data the
  fork duplicated. `JS::ResetRealmMathRandomSeed` did NOT help (siblings still drew the same value),
  so the child installs a fresh `getrandom`-seeded xorshift128+ as `Math.random` at its first script
  entry. Not covered: other realms (iframes), pages that cached `Math.random` earlier.
  `crypto.getRandomValues` (OS) was already distinct.
* **The watchdog thread is not inherited.** It used a `OnceLock` "spawned" flag, which reads true in a
  child that has no such thread, so scripts would run with no deadline. It is now a joinable handle:
  `quiesce` takes it, `resume` and `ScriptDeadline::arm` start it again.
* **Sockets.** Pooled connections must be gone before the fork or children share TLS state. The probe
  loads on a current-thread runtime and drops it (closing the page's connections) before forking.
  `net::runtime()` has no callers; the engine runs on the caller's runtime.

Measured (N=64, fixture of ~300 list items, so ~14 MB RSS): see `docs/bench/r3-4-fork-*.tsv`. These are
numbers for a small page; a large SPA's per-child Private_Dirty and fork time are not measured yet.
