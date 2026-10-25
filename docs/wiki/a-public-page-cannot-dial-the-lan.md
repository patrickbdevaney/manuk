# A public page could make the browser dial 127.0.0.1 — the guard lives where DNS is resolved

> Landed W3.10. Gate: `public_page_cannot_reach_private_network`, `cors_preflight_gates_non_simple_requests` (`engine/net/tests/g_private_egress.rs`).

## Mechanism

- `engine/net/src/egress.rs`: the connector's DNS resolver (`GuardResolver`) drops loopback, RFC1918, CGNAT,
  link-local (169.254/16, fe80::/10), unspecified and ULA addresses **after resolution** while the top-level
  document is public. That is what defeats DNS rebinding; landlock cannot filter by IP. IP-literal hosts never
  reach the resolver in hyper, so `send_raw` also calls `check_url_literal`.
- "Public" = `set_top_level_document` saw an http(s) URL whose host is not a private literal / `localhost` /
  `*.local` / a dotless name. A page the user opened on localhost keeps its backend.
- Embedder calls with no initiator (`manuk_net::request`, the local-LLM client) use a second, unguarded
  client via a task-local, so the agent can still reach a loopback model server.
- Opt out: `--allow-private-network` (shell) or `MANUK_ALLOW_PRIVATE_NETWORK=1`.
- CORS preflight: `cors::needs_preflight` / `preflight_response_allows`, driven by `manuk_net::cors_preflight`,
  called from the shell's `pump_fetches` before the real request is sent. Failure = network error (status 0).

## W3.10.a — raw sockets and the headless executor

- `egress::resolve_for_dial` resolves `host:port` under the same post-DNS rule; `WebSocketConn::connect` dials the
  vetted addresses (never the name), so `ws://127.0.0.1`, `ws://localhost` and `ws://[::1]` from a public page fail.
- `proxy::fetch_via_proxy` refuses a private *literal* target before dialling the SOCKS proxy. Names are resolved by
  the proxy out of our sight: a documented gap.
- `Page::pump_page_fetches` (what `manuk-mcp` and every headless consumer run via `finish_loading`) now calls
  `cors_preflight` for cross-origin requests; `Page::load_async` alone does not pump fetches.
- Gates: `public_page_cannot_open_a_websocket_to_loopback`, `public_page_cannot_target_loopback_through_a_socks_proxy`
  (net) and `g_page_fetch_preflight` (page, needs `--features spidermonkey`). Red under removing the preflight call
  or the guard in `resolve_for_dial`.

## Not covered

Proxy-resolved hostnames; WebTransport/EventSource-specific dials (none exist as separate sockets today).
