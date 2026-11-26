# Manuk

**A browser built for agents: a Chrome MCP server you can use today, and a forkable Rust engine for running many
agents in parallel.**

Manuk gives a model a structured view of any website (role, accessible name, state, hit-test box and a stable handle
per element) instead of screenshots or brittle selectors, then acts through real input events and reports what
changed. It runs on two interchangeable backends behind one set of tools:

- **Chrome backend (default).** Drives a pinned Chrome for Testing with its sandbox on. Passed 40 of 40 tasks in all
  three runs of the project's own suite.
- **Manuk engine (`--backend engine`).** An independent browser engine in Rust with no Chrome dependency. Small
  isolated processes, quick cold start, forkable pages, recorded and replayable sessions.

## Built for parallel agents

When every agent gets its own isolated browser process, the fixed cost of that process decides how many agents fit on a
machine. The engine starts at about 31 MB. The comparison throughout is **one isolated browser process per agent**,
against pinned headless Chrome 145.0.7632.116, three runs per row (six for the real-site row).

| per agent | Manuk engine | Chrome process | ratio |
|---|---|---|---|
| memory, 1 agent (local 37 KB page) | 31 MB | 398-404 MB | 0.08x |
| memory, 10 agents (local pages) | 14.2 MB | 204-207 MB | 0.07x |
| memory, 50 agents (local pages) | 12.4 MB | 190 MB | 0.07x |
| memory, 10 agents on real sites (5 sites, 2 sessions) | 159-191 MB | 272-291 MB | 0.57-0.70x |
| cold start to first observation (local page) | 55-57 ms | 305-314 ms | 0.18x |

**Fork a loaded page.** `--zygote-after-load` loads a page once and forks a separate session from it for each agent. On
a page with a 3.9 MB DOM, 64 of 64 forked sessions acted independently, with a median fork time of 4.8 ms (p99 9.5 ms)
and a median of 8.7 MB private memory each. Every fork gets its own randomness and TLS state, and the gate goes red if
that isolation is removed. Forks share a snapshot of the loaded page.

**Warm sessions.** `--zygote` serves each connection as a fresh session with its own cookies and storage. The time to a
first page is 42-44 ms, against 67-68 ms from a cold start.

Methods, raw rows and every other comparison, including shared-browser tabs:
[`docs/sprint/FOOTPRINT.md`](docs/sprint/FOOTPRINT.md). A preregistered test of parallel agents doing real tasks, across
several model families, is designed in [`docs/sprint/CONCURRENCY-PLAN.md`](docs/sprint/CONCURRENCY-PLAN.md).

## Use it

Pre-release: build from source, or install a dispatch-built artifact ([`docs/MCP-INSTALL.md`](docs/MCP-INSTALL.md)).

```bash
./scripts/mozjs-fetch.sh                      # once: pinned SpiderMonkey prebuilt (a clean-clone build needs it)
cargo auditable build --release -p manuk-mcp  # or: cargo build --release -p manuk-mcp
claude mcp add manuk -- manuk-mcp
```

The default backend drives the pinned Chrome for Testing (under `~/.local/chrome-cft/<pin>/`, or `MANUK_CHROME`) with
its sandbox on. On Ubuntu 24.04 (and 23.10+) AppArmor blocks the unprivileged user namespaces Chrome's sandbox needs
(symptom: `No usable sandbox`); a one-time root step (a per-binary AppArmor profile, or the machine-wide sysctl
`kernel.apparmor_restrict_unprivileged_userns=0`) is in [`docs/MCP-INSTALL.md`](docs/MCP-INSTALL.md). `--backend engine` opts in to the in-process engine (`--backend chrome` is still accepted); `initialize` reports the backend as `serverInfo.title`. Thin stdio clients:
[`python/manuk_client.py`](python/manuk_client.py) (standard library) and [`ts/manuk.ts`](ts/manuk.ts) (Node
built-ins). Tool-by-backend behaviour: [`docs/sprint/BACKENDS.md`](docs/sprint/BACKENDS.md). Security policy and SBOM:
[`SECURITY.md`](SECURITY.md).

**WebMCP.** Tools a page registers through `document.modelContext.registerTool`, declarative `toolname` forms, and one
synthesized fill-form tool per `<form>` appear as `webmcp_<name>` MCP tools with `list_changed`; a tool whose
description holds paint-hidden text is withheld.

## Results

<!-- scoreboard:begin -->

chrome_ver=145.0.7632.116, driving model Haiku 5.5, pass count per run (3 runs), from `docs/sprint/SCOREBOARD.tsv`:

| arm | own 40 tasks, runs 1/2/3 | external 10 tasks, runs 1/2/3 |
|---|---|---|
| Playwright MCP + Chrome | 39 / 40 / 38 of 40 | 8 / 7 / 7 of 10 |
| manuk-mcp `--backend chrome` | 40 / 40 / 40 of 40 | 7 / 8 / 7 of 10 |
| manuk-mcp (in-process engine) | 39 / 39 / 38 of 40 | 7 / 5 / 7 of 10 |

<!-- scoreboard:end -->

Playwright MCP is the reference arm. The task suite and bars were fixed before any result, and every row is
published. All suites and verdicts, project status and known limits:
[`docs/RESULTS-AND-LIMITS.md`](docs/RESULTS-AND-LIMITS.md).

## Why agents benefit

- **They can check their own work.** Each action returns a receipt of the storage writes and non-safe requests it made. In the
  receipts test, verifier accuracy rose from 42% to 96% on the engine and from 38% to 83% on Chrome.
- **Data stays where it belongs.** With origin grants, hostile pages were blocked from sending a canary to an ungranted
  origin across 10 tested channels on both backends. Secrets are filled from the operator's store without entering the
  model's context. Both gates go red when the protection is removed.
- **Runs can be repeated.** `--record` and `--replay` reload an engine session with no network. In the gate, all 3
  replays gave the same accessibility tree as the live page.
- **Pages can be rewound.** `checkpoint`, `restore` and `fork` are MCP tools.
- **People stay in the loop.** `handoff` pauses the agent, lets a person act, and resumes with a diff.

## Purpose

Software agents that operate websites today either interpret screenshots or depend on DOM selectors that change with
each deployment. WebMCP offers a semantic tool surface only on sites that adopt it, and WebDriver BiDi provides
standard actuation but no accessibility surface. Because Manuk parses the DOM and solves layout itself, it derives,
without executing page JavaScript, a semantic view of any page: each element's role, accessible name, interaction
state, hit-test box and a stable address. An agent can perceive a page as structured data, act through standard
WebDriver BiDi input that dispatches real DOM events, and verify the outcome, on sites that have made no provision for
agents.

The same engine core serves three front ends: a desktop browser (`shell`), a headless agent browser (`agent`) and the
WebAssembly demonstration (`demo`).

## Capabilities

- **Rendering:** block, inline, flex, grid, table, float and positioned layout; stacking contexts; overflow clipping;
  backgrounds, gradients, shadows, transforms; the full Stylo cascade including `@layer`, container queries and `:has()`.
- **JavaScript platform:** DOM, CSSOM and events; `fetch` and XHR; ES modules with import maps; custom elements and
  shadow DOM; mutation, intersection and resize observers; Web Animations; View Transitions; the Navigation API;
  IndexedDB; Cache API; Web and Service Workers (same thread); Content Security Policy; `crypto.subtle`.
- **Media:** `<video>` playback of H.264 and AAC in MP4; Media Source Extensions partial.
- **Browser shell:** forms and uploads, tabs with hibernation, history, bookmarks, find in page, zoom, RFC 6265 cookies
  with partitioned storage, session restore, an encrypted credential store with origin-scoped autofill, streaming
  downloads.
- **Agent surface:** accessibility tree (role, accessible name, state, focus, geometry) computed without page
  JavaScript; occlusion-aware hit testing; actions that dispatch real DOM events; a provider-neutral inference
  interface; page text delivered to models only inside an untrusted-content boundary.
- **MCP tools:** `navigate`, `observe` (full, viewport or diff; a tree-shaped snapshot with list, row, card and dialog
  context and frame-merged handles), `extract(query)` returning only matching passages, `find`, `act` with an effect
  diff and a dispatch-time hit-test check, `type` with real key events, `scroll`, composite `fill_form(fields[])` and
  `act(steps[])` that stop at the first dialog or navigation, `upload`, `key`, `drag`, `dialog`, gated downloads,
  `screenshot`, `storage_state`, and `handoff` (the agent pauses, a person acts, the agent resumes with a diff of what
  changed). Per-backend behaviour is in [`docs/sprint/BACKENDS.md`](docs/sprint/BACKENDS.md).
- **Engine-only agent features:** every act and navigate reports when the page has settled, read from the engine's own
  fetch and timer queues rather than a network-idle guess; `checkpoint`, `restore` and `fork` (replay-based, including
  a cookie and storage snapshot); action receipts listing the storage writes each act made; `--zygote-after-load`
  forks sessions from an already-loaded page (64-way fork verified on a page with a 3.9 MB DOM); `--deterministic`
  runs with a logical clock and seeded randomness.
- **Security defaults (agent mode):** an ephemeral in-memory cookie and storage partition unless `--profile` is
  given; no `file:` loads from a web document; loopback, RFC 1918 and link-local addresses refused after DNS
  resolution, on fetch, WebSocket and proxy dials; a per-task origin allowlist enforced on every network channel; values
  read on one origin refused toward another (including encoded copies); secrets typed from the OS keyring through
  named handles and redacted from results; text a person cannot see is withheld from the model; SpiderMonkey's
  optimizing JIT tiers off (baseline tiers kept) and a Landlock filesystem ruleset on the headless binaries. The Chrome
  backend runs with Chrome's sandbox on; `--unsafe-no-sandbox` is the only way to turn it off. Each layer is pinned
  by a gate shown to fail without it ([`SECURITY.md`](SECURITY.md)).
- **Agent grants:** `manuk-mcp` takes `--read-origins` and `--write-origins` (origin sets for navigation, reads, typing
  and POSTs), and `--confirm=submit,download,navigate-new-origin` (actions that need `confirm=true`). Credential and
  payment fields are never typed by the agent (HANDOFF), and out-of-set origins are REFUSED on both the in-process and
  Chrome backends (the Chrome blocked-iframe socket residual is listed in the gaps). Flags, verdicts and open gaps: [docs/sprint/BACKENDS.md](docs/sprint/BACKENDS.md#grants-w39b).

## Architecture

| component | crate | implementation | upstream (license) |
|---|---|---|---|
| Networking | `engine/net` | reused | hyper, rustls, tokio (MIT / Apache-2.0) |
| HTML parsing | `engine/html` | reused | [html5ever](https://github.com/servo/html5ever) (MIT / Apache-2.0) |
| DOM | `engine/dom` | own | — |
| Style | `engine/css` | reused, embedded unmodified | [Stylo](https://github.com/servo/stylo), the cascade used by Servo and Firefox (MPL-2.0) |
| Layout | `engine/layout` | own (block, inline, float, table, positioning); flexbox and grid via Taffy | [Taffy](https://github.com/DioxusLabs/taffy) (MIT) |
| Text | `engine/text` | reused | parley, swash (MIT / Apache-2.0) |
| JavaScript | `engine/js` | reused, bound through a `JsRuntime` trait | SpiderMonkey via [mozjs](https://github.com/servo/mozjs) (MPL-2.0) |
| Media | `engine/media` | own demux; decode via upstream | symphonia, openh264 |
| Accessibility | `engine/a11y` | own | — |
| Paint | `engine/paint` | own display list; rasterization via tiny-skia | tiny-skia (BSD-3-Clause) |
| Compositor | `engine/compositor` | own | — |
| Pipeline | `engine/page` | own | — |
| Automation | `bidi` | own WebDriver BiDi endpoint | — |
| Agent | `agent` | own | — |

SpiderMonkey and Stylo are configured and bound, never patched internally. The JIT, garbage collector and sandbox are
upstream code. All page content runs in a single process today; one OS process per tab is a decided design scheduled
for the security phase. The breakdown of each crate, and the boundary with upstream code, is in
[`docs/loop/HORIZON.md`](docs/loop/HORIZON.md) and [`docs/SPEC.md`](docs/SPEC.md).

## Automation protocol support

Manuk implements WebDriver BiDi, the W3C cross-browser automation protocol. It does not implement the Chrome DevTools
Protocol, which is specific to Chromium and not a standard.

| BiDi module | implemented commands and events | state |
|---|---|---|
| `session` | `new`, `end`, `status`, `subscribe`, `unsubscribe` | implemented |
| `browsingContext` | `create`, `close`, `getTree`, `navigate`, `traverseHistory`, `setViewport`, `captureScreenshot`; events `contextCreated`, `contextDestroyed`, `navigationStarted`, `domContentLoaded`, `load` | implemented |
| `input` | `performActions` | implemented |
| `script` | `evaluate`, `callFunction` | partial |
| `network`, `storage`, `log`, `emulation`, `permissions` | — | not implemented |

Client libraries (Puppeteer, Selenium, Playwright's BiDi channel) have not yet been tested against this endpoint.

## Getting started

Requirements: a Rust toolchain; SpiderMonkey's build prerequisites for the `spidermonkey` feature. Linux x86_64 is the
verified platform; macOS and Windows build in CI but are not verified on hardware.

```bash
# Render a page to PNG, headless (no GPU or display required)
cargo run -p manuk-shell --no-default-features -- render https://example.com/ -o out.png --width 800

# Desktop browser window (winit + wgpu)
cargo run -p manuk-shell -- browse https://example.com/

# Headless agent browser (configure a model endpoint in .env, or use a local llama-server)
cp .env.example .env
cargo run -p manuk-agent --bin agent-run -- "What is this page's main heading?" https://example.com/

# JavaScript through SpiderMonkey and the live Stylo cascade. First fetch the pinned SpiderMonkey prebuilt:
# the script checks its sha256 against the pin and, when `gh` is authenticated, runs `gh attestation verify`.
./scripts/mozjs-fetch.sh
cargo test -p manuk-js --features spidermonkey
cargo test -p manuk-css --features stylo

# Tests and the internal WPT harness
cargo test --workspace
cargo run -p manuk-wpt
```

## How it is developed

Manuk is developed largely by an autonomous AI coding loop operating under human direction. Each change must pass a
single all-or-nothing verification wall (`scripts/verify.sh`): build, WPT-derived and purpose-built correctness gates
(about 570), box-geometry probes within ±3 px of headless Chromium, real-site fidelity, JavaScript conformance and
performance floors. A capability counts only once a gate has been shown to fail without it, and any measured
regression is reverted. Work is prioritized by a differential oracle that renders 265 real sites in Manuk and Chromium
and clusters divergences by root cause. The method is documented in [`docs/loop/`](docs/loop/) and
[`CONSTITUTION.MD`](CONSTITUTION.MD). Responsibility for every change rests with the repository owner.

## Contributing and security

External contributions are not being accepted during Phase 0. Vulnerability reporting and the supply-chain notes are
in [`SECURITY.md`](SECURITY.md); do not report vulnerabilities in public issues.

## License

Mozilla Public License 2.0, as declared in the workspace manifest. Vendored and linked components remain under their
own licenses, as listed in [Architecture](#architecture).
