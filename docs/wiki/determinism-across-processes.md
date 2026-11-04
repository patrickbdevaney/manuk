# Determinism has to hold across processes, not just across runs

`--deterministic` (R3.1) was proven by running one process many times. Two *separate* processes
differ in things a single process never varies: a per-process id salt, the host `TZ`, the host
`LANG`/`LC_ALL`.

Found by `scripts/w311-xproc-gate.sh` (10 fresh processes x 20 pages, a11y digest + `manuk-mcp`
observation text):

* **Page ids** (`page: p40db9e:1`, in every observation) came from a salt of wall-clock nanos ^ pid,
  so 9/10 processes diverged. Under `--deterministic` and with no persisted log dir the salt is
  `determinism::seed()`. With a log dir the random salt stays (it prevents id collisions with logs a
  previous process wrote).
* **Time zone and locale**: `new Date(0).toString()` and `toLocaleString()` followed `TZ`/`LANG`
  (Tokyo+de_DE printed `GMT+0900`, `1.1.1970, 09:00:00`, `1.234.567,891`). `pin_host_environment()`
  sets `TZ=UTC` and `LC_ALL/LANG=en_US.UTF-8` before `JSEngine::init` (ICU caches its default locale),
  and `JS::ResetTimeZone()` runs right after.
* **fdlibm**: `Math.sin(1e10)` etc. were already host-independent (SpiderMonkey bundles fdlibm); the
  fixture `tests/fixtures/w311-env.html` holds that.
* **HashMap iteration**: no divergence found on the 20 pages, so no map was swapped. The gate is the
  detector if one appears.
