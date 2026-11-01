# W4.E1.b — where subres_fetch goes (chrome_ver=145.0.7632.116; manuk-only, no Chrome arm)

Data: 3 runs x 4 URLs of `/tmp/a11y-dump.head` (the pre-W4.E1 HEAD build used by W4.E1.a, built Oct 26) with
`MANUK_NET_TRACE`; traces in `traces/`, table in `attribution.tsv`, method in `scripts/w4e1b-attribute.py`
(`--self-test` checks a known synthetic answer and that dropping a bucket breaks the reconcile; `--drop transfer` on the
real traces exits 1). Medians over 3 runs, seconds. Bucket sums reconcile with perf1's exclusive subres_fetch within 0.1-2.5%.

| URL | subres_fetch | cap_wait | server_ttfb | transfer | discovered_late | (late: ttfb / transfer) | reqs (late, http1) |
|---|---|---|---|---|---|---|---|
| comix.to | 2.47 | 0 (unobservable) | 0.13 | 0.01 | 2.33 | 2.07 / 0.26 | 210 (207, 0) |
| mobile.bg | 4.65 | 0 (unobservable) | 0.00 | 0.41 | 4.12 | 2.66 / 1.34 | 101 (91, 0) |
| puentedemando | 5.68 | 0 (unobservable) | 0.10 | 0.10 | 5.47 | 4.58 / 2.12 | 130 (129, 4) |
| ebanking | 3.84 | 0 (unobservable) | 0.21 | 0.24 | 3.34 | 0.84 / 2.50 | 61 (60, 61) |

Reading: on all four, 85-96% of subres_fetch sits in requests that START >100 ms after the first subresource request
(discovered after the parse-time batch: CSS url() images/fonts, script-inserted resources, layout-time images), not in the
parse-time batch. So a parse-time scanner cannot reach it - consistent with W4.E1.a's null result. Inside that late time,
the three h2 sites are mostly waiting for first byte (comix 2.07 of 2.33 s, mobile.bg 2.66 of 4.12, puentedemando 4.58 of
5.47); ebanking (HTTP/1.1, all 61 requests, concurrency peaks at exactly 6) is mostly body transfer (2.50 of 3.34 s).

Limits: (1) cap_wait is 0 by construction - the trace's `req start` is stamped after the connection is acquired, so queueing
behind the 6-conn cap is invisible; ebanking hitting max concurrency 6 with 61 requests suggests it is real there but
unmeasured (follow-up W4.E1.b.a). (2) server_ttfb includes connect+TLS handshake on new hosts. (3) "late" is a 100 ms
threshold from the first subresource start, a stated heuristic, not discovery timestamps (the trace has none).
