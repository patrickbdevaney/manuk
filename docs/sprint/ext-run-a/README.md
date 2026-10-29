# EXT.RUN.a: why manuk-mcp shows no Wolfram|Alpha result pods

chrome_ver=145.0.7632.116 (no Chrome arm here; manuk-only diagnosis). Driver: `drive.py` (stdio MCP: navigate, wait, extract).
Target: X07 query URL (`/input?i=integral+of+3e^(2x)+from+x=0+to+x=5`), release `manuk-mcp --no-confine`.

## Findings
1. **The page JS runs.** The Next.js app shell hydrates (nav, input box, "Natural Language / Math Input" buttons are
   in the observation) and the net trace (`wolfram-nettrace.log`) shows all 15 chunks, `_app`, `input-*.js` and
   `/n/v1/api/randomizer` fetched and executed. SpiderMonkey is not failing to bootstrap.
2. **The "doesn't run without JavaScript" text is a `<noscript>` leak in `extract`, not a page state.** With scripting
   on, `<noscript>` content is raw text and must not be exposed. Fixture `noscript-fixture.html`
   (JS sets `js-ran`): `extract` returns `js-ran <p>NOSCRIPT-NOTICE needs javascript</p>`. So agents read the
   page's hidden fallback as if it were visible. This is an engine/agent-layer bug (extract/a11y tree ignores the
   scripting flag for noscript).
3. **No pods because results arrive over a WebSocket that agent mode never services.** The app's `_app` chunk opens
   `new WebSocket(webSocketProtocol + fetcherDomain + results)`; there is no XHR/fetch to a results API in the trace
   (last request: `/n/v1/api/randomizer`). The engine exposes `Page::take_ws_ops` / `deliver_ws_event`, and the only
   caller outside tests is the GUI shell (`shell/src/gui.rs` `pump_websockets`). `grep -rniE 'websocket|ws_op' mcp/src`
   and `agent/src` (non-bin) find nothing: the MCP/agent backend never connects page sockets.

## Verdict
Engine cause named: agent backend (mcp/src/backend.rs path) lacks the WebSocket pump the GUI has. Gate "pods in 3/3"
is NOT met by this node (diagnosis only); it moves to EXT.RUN.a.a (pump) and EXT.RUN.a.b (noscript leak).
Not measured: that the pump alone yields pods (WA may also need cookies/captcha); that is EXT.RUN.a.a's gate.

## EXT.RUN.a.a.a: why pod bodies were absent (chrome_ver=145.0.7632.116)
WS frame dump on the X07 URL: the page sends init, newQuery and a user info; the server answers `ready` and
`noAuth` (harmless, anonymous) and then ~1.4 s later `pods` frames (Definite integral, ..., `queryComplete timing 1.41`).
The pump called the socket quiet after 400 ms of silence and stopped, so the pods frames were never delivered.
The pod bodies are `<img alt="integral_0^5 3 e^(2 x) dx = 3/2 (e^10 - 1)≈33038.">` and extract already exposes alt text.
Fix: while the page has sent more frames than the server has answered, keep servicing until WS_REPLY_BUDGET (8 s).
