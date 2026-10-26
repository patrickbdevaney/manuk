# Manuk

Manuk is an independent web browser engine written in Rust that renders the existing web for people and exposes each
page to software agents as a structured accessibility surface. It is not a Chromium fork or a wrapper around another
browser; it composes established Rust web-platform components with its own DOM, layout, paint and accessibility code.

> [!IMPORTANT]
> **Pre-release (Phase 0 of the roadmap).** Manuk renders a broad share of real websites and runs their scripts, but
> it is not yet suitable for daily browsing or for production automation. Visual placement fidelity, function
> certification and accessibility-tree accuracy are below the project's own exit thresholds (see [Status](#status)).
> Current release: v1.2.0.

[Run the engine in your browser](https://patrickbdevaney.github.io/manuk/) (WebAssembly build: layout, paint and the
accessibility tree on page snapshots; no JavaScript, see [Known gaps](#known-gaps-and-non-goals)).

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

## Status

Measured for the v1.2.0 release on a corpus of real sites rendered by both Manuk and Chromium from identical
snapshots. 76 of 265 corpus sites were reachable by the reference Chromium (the remainder are blocked by bot
protection for the reference itself) and all 76 are scored. "Full" counts a site that hangs the driver as zero;
"measured-only" reports quality where a score was obtained. Method and per-site data:
[`docs/RELEASE-v1.2.0.md`](docs/RELEASE-v1.2.0.md).

| axis | measures | measured-only | full | exit threshold |
|---|---|---|---|---|
| M1 render | node placement relative to Chromium (shape ≥ 0.75), no overlap or overflow | coverage 89.9%, shape 72.1% | — | ≥ 95% of in-scope sites |
| M2 drive | perceive → address → act on page targets | 78.9% (n = 60) | 62.3% | ≥ 95% render ∧ function |
| a11y | accessibility tree role/name/state agreement with Chromium (F1) | 82.0% (n = 65) | 70.1% | ≥ 90% of nodes |

**Web Platform Tests.** About 496,000 WPT subtests pass in the project's internal harness (`tests/wpt`), tracked as a
monotonic ratchet across releases. Results have not been submitted to wpt.fyi, and the figure is an internal
measurement, not a published conformance claim. JavaScript conformance is provided by SpiderMonkey; Test262 results
are not separately reported.

**Release history:** v1.0.0 (2026-08-21, [notes](docs/archive/dev/RELEASE-v1.0.0.md)), v1.1.0
([notes](docs/RELEASE-v1.1.0.md)), v1.2.0 ([notes](docs/RELEASE-v1.2.0.md)). Live, generated project state:
[`STATUS.md`](STATUS.md).

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

## Known gaps and non-goals

Gaps on the Phase-0 path: placement fidelity (mean parent-relative shape about 0.69); roughly one in five in-scope
sites not yet reaching a stable, scorable DOM; rich editing (`contenteditable`, `Selection`) and IME composition;
WebAuthn; the M2 function certificate; adoption of a platform accessibility bridge. SpiderMonkey faults inside its
own frames cannot be contained in-process until process-per-tab isolation lands. The WebAssembly demo has no
JavaScript because SpiderMonkey does not target WebAssembly.

Out of scope by decision: Encrypted Media Extensions and DRM, WebRTC, WebGPU and heavy WebGL applications, HTTP/3.
Manuk does not attempt fingerprint evasion or bot-detection circumvention.

## How it is developed

Manuk is developed largely by an autonomous AI coding loop operating under human direction. Each change must pass a
single all-or-nothing verification wall (`scripts/verify.sh`): build, WPT-derived and purpose-built correctness gates
(about 570), box-geometry probes within ±3 px of headless Chromium, real-site fidelity, JavaScript conformance and
performance floors. A capability counts only once a gate has been shown to fail without it, and any measured
regression is reverted. Work is prioritized by a differential oracle that renders 265 real sites in Manuk and Chromium
and clusters divergences by root cause. The method is documented in [`docs/loop/`](docs/loop/) and
[`CONSTITUTION.MD`](CONSTITUTION.MD). Responsibility for every change rests with the repository owner.

## Contributing and security

External contributions are not being accepted during Phase 0. A private vulnerability-reporting channel has not been
set up yet and will be published before a stable release; do not report vulnerabilities in public issues.

## License

Mozilla Public License 2.0, as declared in the workspace manifest. Vendored and linked components remain under their
own licenses, as listed in [Architecture](#architecture).
