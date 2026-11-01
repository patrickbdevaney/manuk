# manuk-mcp: tool x backend (CB.1)

chrome_ver=145.0.7632.116. Every "works" cell is proved by `mcp/tests/backends.rs` (real stdio, one loopback
page, both backends); `cdp.rs` / `checkpoint.rs` / `world.rs` cover the deeper per-backend behaviour.
`manuk` = in-process engine; `chrome` = `manuk-mcp --backend chrome`.

| tool | manuk | chrome | gate |
|---|---|---|---|
| navigate | works | works | backends.rs `every_tool_on_the_*_backend` |
| observe (full/viewport/diff) | works | works | same |
| extract (text/markdown/table) | works | works | same |
| find | works | works | same |
| act click / check / select | works | works | same (click effect read back; checkbox + select driven) |
| act focus | works (focus accepted; the page-side effect is not read on manuk, see differences) | works (page `activeElement` poll sees the focus) | same; the page probe is read on chrome only (see differences) |
| act hover | works (`mouseover` handler observed) | works (`mouseover` handler observed) | same; mutant `MANUK_CDP_MUTATE=no-hover` makes chrome's hover a no-op -> `every_tool_on_the_chrome_backend` red at `hover did not reach the page` |
| type | works | works | same (typed value observed) |
| scroll (dy) | works | works | same |
| screenshot | works (PNG) | works (PNG, `Page.captureScreenshot`) | same; mutant `MANUK_CDP_MUTATE=no-screenshot` -> `chrome_screenshot_mutant_fails` (and the pre-CB.1 code was exactly that mutant) |
| storage_state export/import | works (all origins) | works (cookies; localStorage of every origin the session loaded; imported foreign-origin localStorage is seeded when that origin next loads) | same: seeded localStorage exported, imported cookie read back |
| checkpoint / restore / fork / commit | works | differs: replay over a fresh Chrome per page works (page state rewinds), but the checkpoint's world tag says CLIENT though Chrome's network is invisible to the write-fence | `rollback_on_the_*_backend`; tag fix = R4.1.b |

## Differences
- **Focus events (manuk).** `act focus` moves the engine's focus, but manuk fires no `focus`/`focusin` events (the JS `focus()` binding only queues the focus change). A page that reads focus from an event handler sees nothing on manuk; the test probe polls `document.activeElement` on a timer; it sees the focus on chrome and not on manuk. Follow-up CB.1.a.a.

## Differences that are not failures
- **Tree shape.** On the same page chrome's tree has more nodes: paragraphs, generic text, and a `button` node for a `<label>` (manuk omits these). Handles are therefore not comparable across backends; the page header, role and name of the interactive controls are.
- **storage_state localStorage scope (CB.1.b).** chrome exports the loaded page's localStorage live and every other origin it visited from the snapshot taken when that page was last observed; an import for an origin not currently loaded is applied by a new-document script on the next load of that origin (then removed), so it takes effect on visit, not immediately. Gate: `storage_state_two_origins_on_*_backend` + `chrome_current_origin_only_mutant_fails` in mcp/tests/backends.rs.
- **screenshot `handle`.** Ignored on both backends (viewport only), as the tool reply says.
- **Injection shield.** Both backends return a verdict; chrome computes it from DOMSnapshot paint data (R4.1), manuk from its own layout.

## Grants (W3.9.b)

Grants are the agent's permission surface, enforced in the backend-agnostic session (`mcp/src/session.rs`, `gate`)
so one policy covers `--backend manuk` and `--backend chrome`. A refused or confirm-gated call is judged before it
reaches the backend. Page-side enforcement is separate (see Gaps). Source of truth for the flags:
`parse_grants` in `mcp/src/policy.rs`; both `--flag value` and `--flag=value` forms are accepted.

| flag | meaning |
|---|---|
| `--read-origins a,b` | origins the agent may GET: navigate, follow a link, submit a GET form. Repeatable, values merge. |
| `--write-origins a,b` | origins the agent may type into or POST to. Repeatable, values merge. |
| `--confirm=submit,download,navigate-new-origin` | these actions need an explicit `confirm=true` on the call. Unknown kinds are an error. |

Origins are `scheme://host[:port]`; anything else is an error at startup. An empty set means unrestricted on that
axis. With no flags at all the gate is off (`is_open`), the headless default. `--read-origins` alone leaves typing and
same-origin POSTs unrestricted, and a cross-origin form POST is refused whenever `--write-origins` is empty.

Verdicts and the wording the agent sees (each reply ends `; nothing was sent`):

- **allow**: the call runs.
- **CONFIRM REQUIRED (`<kind>`)**: `submit`, `download` or `navigate-new-origin` need `confirm=true`. The same call
  repeated with `confirm=true` runs. `--confirm=submit` also gates clicks whose accessible name contains a risky word
  (delete, remove, pay, purchase, buy now, place order, transfer, send money, confirm order, unsubscribe, close account).
- **HANDOFF**: the target is a password, one-time-code, `cc-*`, `current-password` or `new-password` field. The agent
  never types credentials or payment data; no flag lets it. The reply asks the human to do the step.
- **REFUSED**: the origin is outside `--read-origins` or `--write-origins` (`origin not in read set` /
  `origin not in write set`), or a cross-origin form POST with no write set.

Gaps (named, not faked):

- **Chrome blocked iframe.** For a cross-origin iframe navigation Chrome may still open a bare TCP socket to the
  refused origin (no request line, 0 bytes). The loopback gate counts request lines, so it does not see this. Open as
  W3.9.a.a.
- **Frames in observe (W3.7).** On `--backend chrome`, `observe` nests each iframe's document under a
  `[hN] region "iframe <origin>"` line (same-site frames from the page snapshot, cross-site ones over their own
  DevTools session), and `type`/`act` on a handle inside it reach the child's element. Gate:
  `mcp/tests/frames.rs`; `MANUK_CDP_MUTATE=no-frames` turns the merge off. `--backend manuk` cannot do this: the engine
  has no browsing context for a frame, so it shows nothing of a frame's content. Frames from origins outside the
  read set are not yet hidden from the observation (W3.7.a).
- **Name-based judgement.** When the backend cannot describe an element, a click is judged by its name alone; the
  risky-word list is a heuristic, not a classifier.
- **WebSocket.** Upgrades are judged as writes in `manuk_net` (`engine/net/src/scope.rs`, `websocket.rs`); the chrome
  WebSocket leak is closed by W4.D1.a. Frames carry data out, so a read-set origin is not enough to open one.
- **Page-side enforcement** is in `manuk_net` on the manuk backend and in a `Fetch.requestPaused` interceptor on the
  chrome backend (W3.9.a). Gate: `mcp/tests/origins.rs`, `mcp/tests/grants.rs`.

## Follow-ups (no cell above is "missing" without one)
- CB.1.a.a: manuk fires `focus`/`focusin` events on `act focus` (engine `el_focus` binding), so the page probe passes on both backends.
- R4.1.b (existing): chrome checkpoint must not claim CLIENT.
