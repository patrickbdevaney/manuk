# A rollback that only rewinds the client must say so, and a branch must not write

> Landed W3.13. Gate: `mcp/tests/world.rs` (real loopback server holding the cart) + `engine/net` `partition::tests`.

## Mechanism

- `engine/net/src/ledger.rs`: `send_raw` (every HTTP request: fetch/XHR bodies, sendBeacon, form POSTs, streaming
  requests, redirects) and `WebSocketConn::connect` call `ledger::gate`. A non-GET/HEAD/OPTIONS request is appended to a
  process ledger, or, with the fence up, pushed to a held list and refused (the page sees a network error). Embedder
  (`TRUSTED`) calls are not page sends and bypass it.
- `engine/net/src/partition.rs`: `snapshot()`/`restore()` of cookies, localStorage, sessionStorage, CacheStorage.
  Service-worker registrations are listed in the snapshot as an empty array: the engine has no SW store yet.
- `mcp/src/hub.rs`: a checkpoint carries the snapshot, the ledger position and a tag. `CLIENT` = no non-safe request had
  left the process, so restore is a full rollback; `WORLD` = the server had already been written to. `restore` replays the
  log, applies the snapshot, and reports `SERVER-DIVERGED: [POST /add]` for non-safe requests since the checkpoint (it
  cannot undo them). `fork` makes a fenced page; acts report `held=[POST /order]`; `commit` sends each held request once.

## Limits

- Replaying a `WORLD` checkpoint's log re-sends the prefix's non-safe requests (the replay is not fenced).
- A held WebSocket open cannot be replayed on commit; commit lists it as not sent.
- A checkpoint restored by another process reports divergence as unknown (the ledger is per process).
- `mcp/tests/world.rs::every_wire_path...` enumerates wire primitives in `engine/net/src` by name; a new file using one
  fails until it is gated or listed as GET-only.
