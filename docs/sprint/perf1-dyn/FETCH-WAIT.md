# PERF.1.2.b.b.a: what `dynscript_wait` waits on

chrome_ver=145.0.7632.116 · traces: `docs/sprint/perf1-dyn/traces/` (MANUK_NET_TRACE, 3 runs). Read by hand, not a new sweep.

## Findings

1. **Not the per-host slot.** In every wait window checked, the awaited fetches show `q` -> `dns` -> `connect` -> `req` -> `body` with no idle gap attributable to a slot queue; the delay is dns/connect/TTFB of third-party hosts.
2. **mobile.bg: one third-party script bounds the phase.** The 2.17 s first wait (run 1) is `gabg.hit.gemius.pl/xgemius.min.js`: dns 0.17 s, connect 1.24 s (0.68 s / 0.71 s in runs 2/3), body at 5.40 s. Nothing else in the round is late (cookie-script body 3.52 s, pubads 3.59 s). The phase cannot end before this script arrives unless the engine stops waiting for it; reordering alone does not shorten it.
3. **puentedemando: the ordered round barrier is visible.** Round 3 (run 1) waits 0.31 s on `www.clarity.ms/tag/...` (queued first, body 6.47 s). `gtag/js` arrived at 6.01 s and cookieyes `script.js` at 6.14 s, with `banner.js` fetchable immediately after, but both run only after clarity arrives because `FuturesOrdered` holds document order. Round 1 (0.27 s) is the GTM `gtm.js` fetch itself (genuine chain head, 0.37 s body).
4. **Why order is held.** The page crate cannot tell a script the page set `async = false` on (must stay ordered) from a default-async injected script (browsers run these in arrival order): `async` is not in the reflect table, so it is not visible on the DOM node. Switching to arrival order without that flag could reorder scripts a loader meant to be ordered.

## Verdict

The >= 30% gate is not reachable with a safe engine change today: mobile.bg is bounded by one slow third-party fetch (network variance 0.45-1.24 s on the same host), and puentedemando's removable part is the ordered barrier (~0.3 s per affected round), which needs the async flag first. No engine change landed. Follow-ups: PERF.1.2.b.b.a.a (expose the force-async flag), PERF.1.2.b.b.a.b (arrival-order execution for async scripts).
