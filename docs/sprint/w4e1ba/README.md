# W4.E1.b.a — measurable HTTP/1.1 cap wait (chrome_ver=145.0.7632.116)

The net trace now has a `q <url>` line stamped when a request starts waiting for a per-host connection slot
(engine/net/src/lib.rs, before `sem.acquire_owned()`); the `req` line's start_us is when the slot was granted.
Trace-only, no behaviour change. `scripts/w4e1b-attribute.py --cap-first` gives `cap_wait` precedence in any
subres_fetch slice where some request is queued (default mode keeps the old W4.E1.b precedence, where queueing
overlapped by other progress reads 0).

Data: 3 runs of ebanking.bdm-sa.com (all 62 subresources HTTP/1.x) with the new `a11y-dump`; `traces/`, `attribution.tsv`.
Medians: subres_fetch 3.888 s = cap_wait 2.265 + server_ttfb 0.204 + transfer 0.199 + discovered_late 1.188 (sum 3.857,
reconcile error 0.8%). Individual queue waits reach 3.3 s. Mutant (q ignored, i.e. stamped at send time):
cap_wait 0.000 (`attribution-mutant-q-at-send.tsv`). Caveat: a slice counts as cap_wait if ANY request is queued, so this is
an upper bound on the time the cap alone costs. Follow-up: W4.E1.b.a.a.
