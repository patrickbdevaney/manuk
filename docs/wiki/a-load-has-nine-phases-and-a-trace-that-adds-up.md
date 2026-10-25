# A page load has nine phases, and `MANUK_NET_TRACE` makes them add up

> Landed PERF.1. Instrument: `scripts/perf1-profile.py` (`--self-test` is its own gate). Report: `docs/sprint/PERF.md`.

## Mechanism

- `MANUK_NET_TRACE=<file>` (engine/net/src/nettrace.rs) appends one TSV line per event: `epoch`, `dns`, `connect`
  (a NEW connection: DNS + TCP + TLS, timed by `TimedConnector` around the pooled client's connector), `req`
  (start, time to headers, status, HTTP version), `body` (decoded bytes), plus `cascade` / `layout` from
  `restyle_and_layout` and `phase` from `load_async`'s `nav_phase` and `finish_loading`'s ledger. Unset, each hook is one `OnceLock` read.
- `a11y-dump` adds `mark navigate_returned` / `mark a11y_built`, so the agent's tree build is not billed to the load.
- The profiler cuts `[first request, navigate returned]` into slices and gives each slice to one phase by priority
  (cascade > layout > dns > connect_tls > doc_fetch > subres_fetch), then to the engine phase it sits in (parse, script, settle).
  The phases therefore sum to the load time exactly; startup, a11y and teardown come from independent clocks and sum to process wall.
- The cascade event is emitted at the END of the cascade, before layout starts. Emitting both after layout put the cascade
  interval on top of the layout (found by the 500 ms-injection gate before any number was kept).

## Limits

Chrome's side is Performance.getMetrics totals, not intervals, so its CPU phases are totals and its network phases are
shrunk by the CPU fraction. Compare phases to tens of ms, not single ms.
