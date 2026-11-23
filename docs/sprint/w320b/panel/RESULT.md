# W3.20.b.c: WebMCP panel reference on Chrome for Testing 149

chrome_ver=149.0.7827.155 (own measurement row; never diffed against 145.0.7632.116). Binary: ~/.local/chrome-cft/149.0.7827.155/chrome-linux64/chrome.

## What 149 has
- CDP domain `WebMCP` exists (`WebMCP.enable` returns {} with `--enable-features=WebMCP,WebMCPTesting,DevToolsWebMCPSupport --enable-webmcp-testing`; without DevToolsWebMCPSupport it is "wasn't found").
- `document.modelContext` is native (a distinct object from `navigator.modelContext`); the demos register on it, no alias needed. The polyfill steps aside.
- `navigator.modelContextTesting.listTools()` returns `[]` on pizza-maker although `document.modelContext.getTools()` returns 7 tools (demo tools carry `exposedTo`).

## What did NOT work (probe.mjs, probe2.mjs, probe-headed.mjs)
- **No `WebMCP.toolsAdded` event is ever delivered**: enable before navigate, enable after load, disable+enable, Page/DOM/Network enabled first, extra blink/experimental flags, headless and headed (Xvfb), plus a tool registered live after enable. All runs: zero WebMCP.* events. The agent itself is alive (`WebMCP.invokeTool` answers "No frame for given id found" for a bad frame id). So the CDP route yields no panel list; the Application-panel view reads the same events.
- compare149.mjs (native reference = `document.modelContext.getTools()`, manuk = `manuk-mcp --backend chrome` with MANUK_CHROME=149): pages_pass=0 of 6. pizza-maker native=7, french-bistro native=1 (`book_table_le_petit_bistro`: declarative form tools ARE native on 149; on 145 native was 0). The manuk side failed with "CDP Page.navigate timed out" on pizza-maker and french-bistro under 149, and the native side threw "document.modelContext undefined" on the other four pages (cause not isolated). RESULT.tsv/transcript.json hold the raw rows.

## Verdict
No panel list captured on >=2 pages; per the gate a panel-less result is recorded as no-panel and not counted. Node BLOCKED. Follow-ups in GRAPH.tsv: W3.20.b.c.a (why no toolsAdded; try 150.0.7871.124), W3.20.b.c.b (manuk-mcp navigate timeout on 149 and the undefined document.modelContext on 4 pages).
Gate red-ability: the gate is red as measured (0 pages).
