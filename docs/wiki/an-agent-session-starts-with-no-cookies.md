# An agent session starts with no cookies — isolation is a process property, not a flag per request

> Landed W3.4. Gate: `mcp/tests/isolation.rs` (5 tests over real stdio against the built `manuk-mcp` and a local cookie server).

## Mechanism

- `manuk_net::set_ephemeral(true)` (engine/net/src/lib.rs): the process-global cookie jar starts empty and
  `save_cookies` / `webstorage::save` become no-ops; nothing is read from or written to the profile directory.
  `manuk-mcp` turns it on unless `--profile <dir>` is given.
- `--profile <dir>`: sets `MANUK_STATE=<dir>` and takes an exclusive `File::try_lock` on `<dir>/.manuk-profile.lock`
  for the life of the process; a second process on the same dir refuses to start. Parallel tasks are separate processes.
- Storage state: `engine/net/src/storage_state.rs` exports/imports Playwright JSON (`cookies[]`, `origins[].localStorage[]`).
  MCP tool `storage_state {action: export|import, path}`; CLI `--storage-state <file>` seeds an ephemeral session.
- Per-origin grant: `--grant-origin <origin>` copies only that host's cookies out of the human's `cookies.json`
  (`--human-cookies` overrides the path) and logs the count to stderr.
- Isolation is by process, so session A's cookie value is never in session B's address space (checked via
  `/proc/<pid>/mem`, with a positive control that A's memory does hold it).

## Limits

IndexedDB, Cache Storage and the HTTP cache are not exported by storage state. Persistent profiles only keep cookies
with an explicit expiry (the jar's existing rule).
