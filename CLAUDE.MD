# CLAUDE.md — Browser Engine Project

Project directive for Claude Code. Read this in full before making changes. This file governs scope, architecture, and — critically — which parts of this codebase are safe to modify freely versus which require explicit human sign-off regardless of how confident the change looks.

## Current phase: shared engine core, with both front-ends live

Two front-ends now sit on one shared engine core:

1. **Headful, human-facing browser** (`shell`) — a GUI app with tabs, navigation, and rendering (headless `render` to PNG + interactive `winit`/`wgpu` `browse` window).
2. **Headless agentic browser** (`agent`) — a page driver an LLM can operate (navigate / click / scroll / screenshot / observe), driven by a provider-agnostic inference backend. This phase was explicitly authorized and is now in an initial, working form.

The common engine core (networking, parsing, CSS, DOM/JS, layout, text) is realized in the `engine/*` crates and the shared `engine/page` pipeline; the two front-ends diverge only at how they consume a rendered page (window present vs. screenshot+read). Keep that split clean — do not wire front-end-only assumptions into the parse/layout/JS layers.

`.env` holds `GROQ_API_KEY` variables for the agent phase; `.env` is gitignored. The committed agent runner uses a **single** `GROQ_API_KEY` (falling back to `GROQ_API_KEY`); the local-only development harness is a **local-only, gitignored** test tool. The agent's inference backend is decoupled behind an `InferenceBackend` trait — Groq is one implementation, not a hard dependency of the agent logic.

## Current state (implemented)

A working foundation, not a finished browser. What exists and is verified:

- **Networking** (`engine/net`): HTTP/1.1 over TLS (`hyper` + `rustls`, no OpenSSL), redirects, and a general `request` (POST) reused by the agent's Groq client. Live-fetches real sites.
- **Parse → render pipeline** (`engine/{html,dom,css,layout,text,paint}` via `engine/page`): `html5ever` → arena DOM → minimal CSS cascade (selectors/specificity/inheritance) → from-scratch block + inline layout (wrapping, centering; `taffy` for flex) → `fontdb`/`fontdue` text → `tiny-skia` CPU raster to PNG. Renders real pages (example.com and local pages) end to end.
- **JS** (`engine/js`): `JsRuntime` trait with a no-op default; **SpiderMonkey** (`mozjs`) behind `--features spidermonkey` **actually evaluates JS**. Process-global engine, shared across isolates. Modification boundary respected (see below).
- **CSS engine seam** (`engine/css`): minimal cascade default; **Stylo** behind `--features stylo` (links and builds; adapter currently delegates).
- **Compositor** (`engine/compositor`): tab render/memory tiers (focused-GPU / background-CPU / hibernated), damage tracking, scroll.
- **Headful shell** (`shell`): `render <url>` (headless PNG) and `browse <url>` (winit/wgpu window presenting the CPU raster).
- **Headless agent** (`agent`): `AgentBrowser` driver, `InferenceBackend` trait (multimodal), `run_task` loop, `GroqBackend`. Live-tested on Groq `qwen/qwen3.6-27b`: text extraction, link comprehension, link navigation, and multimodal screenshot reading all pass.
- **Conformance** (`tests/wpt`): built-in layout reftests run today; the upstream WPT runner is the growth path. 40 workspace tests pass; zero warnings.

## Mission

Build a browser engine, from first principles in Rust, that:
- Targets the **broadest possible set of real-world websites**, with the explicit exception of the smallest tier of adversarially hostile sites (aggressive bot-wall/fingerprint-gated sites like LinkedIn, X, Indeed — these are out of scope for the core engine and would be handled by narrow, separate adapters in a different phase, never by chasing generic compatibility with their defenses).
- Is verified against **Web Platform Tests (WPT)**, not against matching Chromium's specific undocumented quirks. Standards compliance is the compatibility target. Do not write code whose purpose is to replicate a Chromium-specific non-standard behavior — if a target site depends on that, it's in the excluded hostile/non-standard tier.
- Is optimized from first principles for binary size, per-tab memory, GPU-quality rendering, and click-to-navigate latency — not merely "acceptable," but genuinely disruptive relative to Chromium/Firefox on these dimensions for the sites in scope.
- Runs identically on Linux, macOS, and Windows with no platform serving as the "primary" target and others as afterthoughts.

## Architecture — the stack

Reuse mature components where reinventing them adds security/correctness risk with no differentiation value. Build from scratch, verified against WPT, where the target site set is bounded and a from-scratch build is both tractable and gives real performance upside. Do not blur this line without discussion — see "Modification boundaries" below.

| Layer | Approach | Components |
|---|---|---|
| Networking | Reuse | `hyper` (HTTP/1.1, HTTP/2), `quinn` (HTTP/3/QUIC), `tokio-tungstenite` (WebSocket), `rustls` (TLS — pure Rust, no OpenSSL system dependency), `tokio` async runtime |
| HTML parsing | Reuse | `html5ever` |
| CSS parsing + cascade | Reuse | Stylo (Servo's/Firefox's production CSS engine) |
| Layout | Reuse + build | `taffy` for flexbox/grid; custom block, inline, table, positioned, and float layout built from scratch, verified against WPT layout reftests |
| DOM + Web API surface | Build | Custom, spec-driven (WebIDL), verified per-API against its WPT test suite. This is the largest-volume component — budget accordingly |
| JS engine | Reuse, do not reimplement | SpiderMonkey, embedded via `mozjs` / `mozjs-sys` Rust bindings (the same integration path Servo uses). **Not V8.** SpiderMonkey is C++ and Rust (Rust in the parser layer only — the JIT and GC are C++); this is not a "pure Rust" engine, it is a mature C++ engine with a proven Rust FFI layer |
| Text shaping | Reuse | Parley + swash + fontdb |
| Paint / rasterization | Reuse | Vello — GPU-compute renderer via `wgpu`, with Vello CPU / Vello Hybrid as lighter fallback tiers for background/hibernated tabs. **Currently alpha upstream** — prototype against actual target rendering needs (gradients, blur, complex clips) before committing further |
| GPU abstraction | Reuse | `wgpu` — Vulkan (Linux/Windows), Metal (macOS), DX12 (Windows). No Linux-desktop or specific graphics-package assumptions |
| Windowing / event loop | Reuse | `winit` |
| Compositor | Build | Layer compositing, scroll, damage tracking, animations — built on Vello + wgpu |
| Concurrency/process model | Build | Isolate/compartment-per-tab (SpiderMonkey compartments) multiplexed across a small number of OS processes — not one OS process per tab. Share startup snapshots across isolates to cut per-tab baseline memory |

## Compatibility verification methodology

- Ground truth is **WPT (Web Platform Tests)** — the cross-browser conformance suite used by Chromium, Firefox, WebKit, and Servo themselves. Every layout, CSS, and DOM/API feature you implement should be checked against the relevant WPT subset before being considered done.
- Do not hand-roll a compatibility test corpus from scratch as the primary verification method — WPT already exists and is comprehensive. Use targeted real-site screenshot diffing (against a real Chrome render) as a secondary check for visual fidelity, not as the primary conformance signal.
- When a feature is ambiguous or under-specified even in WPT (this happens — floats and table layout are notorious), document the interpretation chosen and why, rather than silently picking one.

## Modification boundaries — read before touching the JS engine

This is the most important section in this file. Violating it is the one mistake in this project with real security consequences.

**Safe to modify, verify by benchmark or binary size:**
- SpiderMonkey build configuration and feature flags (stripping unused WASM support, the debugger/inspector protocol, unused ICU locale data — all legitimate binary-size wins)
- The FFI/binding layer between Rust and SpiderMonkey (`mozjs` usage, custom bindings) — this is well-specified, low-blast-radius, and the correct place for iteration
- Startup snapshot tuning (shared heap contents for fast isolate startup)

**Do not modify without dedicated fuzzing infrastructure and explicit sign-off:**
- SpiderMonkey's JIT compiler internals (WarpMonkey/IonMonkey) or garbage collector internals
- Any change to how untrusted JavaScript is sandboxed or isolated

Reasoning, so this isn't just an arbitrary rule: JIT miscompilation bugs — incorrect type or escape-analysis assumptions in the optimizing compiler — are historically the largest single source of exploitable browser RCE vulnerabilities. The reason SpiderMonkey and V8 are trustworthy today isn't just that the code is correct, it's that the code has survived years of adversarial fuzzing (OSS-Fuzz/ClusterFuzz) that this project has no equivalent of. Faster code generation does not substitute for that calendar-bound adversarial exposure. If a task seems to require touching this layer, stop and flag it rather than proceeding — this is a "come back to the human" boundary, not a "do it carefully" one.

## Performance targets

- **Binary size**: static linking + LTO + stripped symbols; `panic = "abort"` in release profile; musl target on Linux for a fully static binary with no dynamic-link bloat; static CRT linking on Windows to avoid requiring a redistributable runtime.
- **Per-tab memory**: isolate-based tabs, not process-based. Background/hibernated tabs should freeze JS execution and evict to disk, dropping to Vello CPU or no active renderer at all — full Vello GPU + active JS is reserved for the focused tab only.
- **Click-to-navigate latency**: streaming/incremental parse and layout (begin building the DOM and layout tree as bytes arrive, don't wait for the full response); zero-copy handoff from socket buffer to tokenizer where feasible; speculative preconnect/DNS-prefetch on link hover.
- **Rendering quality**: Vello + wgpu is the graphics-quality lever — this is a from-scratch, GPU-compute-first architecture, not a CPU/GPU hybrid retrofit. Treat visual fidelity and frame timing as first-class, measured metrics, not an afterthought to functional correctness.

## Portability

- Default async I/O path (`tokio`'s standard reactor) must work identically on Linux, macOS, and Windows. Do not introduce Linux-only dependencies into the core engine.
- `io_uring`-based optimization (`tokio-uring` or similar) is Linux-only and is a *future, optional* fast path relevant to the eventual agent/headless phase, not this one. Do not add it to the human-operator GUI codebase now — the concurrency profile doesn't need it here.
- No Linux-desktop assumptions (no GTK/Qt bindings, no specific display-server assumptions) — `winit` and `wgpu` already abstract this; keep it that way.

## Non-goals

- No adapters for LinkedIn, X, Indeed, or other bot-walled sites; no bot-evasion in the core engine.
- No pixel-parity chase with Chromium beyond what WPT compliance already gets you.
- No DRM/EME.
- No horizontal multi-node scaling concerns — this is single-binary, single-machine scope.
- The agent must not couple to a specific inference provider — Groq is one `InferenceBackend`, not a dependency of the agent logic. Do not commit the local-only development harness or any API keys.

## Next frontiers

Roughly in priority order, each behind an existing seam:

- **Layout**: floats, tables, absolute/relative positioning, margin collapsing — the from-scratch WPT-verified work called out above.
- **Loading**: external `<link rel=stylesheet>` + image fetching, gzip/br decoding, encoding sniffing, HTTP/2+3 activation (ALPN `h2`, `quinn`).
- **Real reuse integrations**: wire Stylo's cascade behind the existing `StyleEngine` adapter; wire Vello GPU paint behind the existing `Painter` trait for the focused tab.
- **JS/DOM**: grow the Web API binding surface (`engine/js::bindings`) projecting the DOM into SpiderMonkey, WebIDL-driven, WPT-verified per API.
- **Agent**: viewport-clipped text observation (so scrolling matters to the text channel, not just the screenshot), richer actions (typing, form submit, back/forward), and multi-tab agent sessions.
- **Conformance**: the upstream WPT runner (`$WPT_DIR`), folding real `css`/`dom` results into the existing `Report`.

---

The above is the near-term list; the comprehensive, objective-organized roadmap
follows. Reviewed and re-prioritized periodically. The north star is a
**disruptively good single-machine browser**: fast, versatile across the real web,
and lightweight — measured, not merely "acceptable." Each item sits behind an
existing seam so it is additive.

### 1. Performance (single machine)
- **Click-to-navigate latency:** wire the incremental path already positioned —
  stream socket bytes into the html5ever sink, build DOM/layout as bytes arrive
  instead of after full buffering; speculative preconnect/DNS-prefetch on link
  hover; connection reuse/pooling (currently one connection per request).
- **Frame timing + rendering quality:** land the **Vello** GPU-compute tier behind
  the existing `Painter` trait for the focused tab; treat frame time and visual
  fidelity as first-class measured metrics.
- **Per-tab memory:** realize hibernation for real — freeze background JS, evict
  rasterized tiles to disk, drop background tabs to Vello-CPU/no-renderer; share
  SpiderMonkey startup snapshots across isolates.
- **Incrementality:** damage-driven relayout + repaint (the compositor already
  tracks damage) instead of full-page passes; dirty-subtree style invalidation.
- **Caching:** HTTP cache, decoded-image cache, shaped-run cache.

**Research dispositions (2026-07, primary sources cited inline):**
- **B1 streaming parse→layout.** *Finding:* html5ever's `parse_document` returns a `Parser: TendrilSink` already exposing `process(chunk)`/`finish()` (push-incremental — docs.rs/html5ever); browsers additionally run a *secondary preload scanner* over raw bytes for `<img>/<link>/<script>` while the main parser blocks on sync scripts/CSS, and CSS is render-blocking so first paint is gated on `<head>` CSS (web.dev/preload-scanner; MDN speculative parsing). *Disposition:* **reuse** html5ever's incremental sink; **hand-roll** the first-paint checkpoint (small engine glue); **defer** the preload scanner until external-resource fetch (D4) exists. Prereq: a chunked-body stream in `engine/net` (today `BodyExt::collect` buffers the whole body). *Roadmap:* the §1 "Click-to-navigate latency" bullet's "stream socket bytes into the html5ever sink" resolves to: net yields body chunks → `parser.process()` per chunk → first-paint checkpoint after `<head>` CSS → incremental body layout. No WPT target (perf/architecture).
- **B2 connection reuse / preconnect / DNS-prefetch.** *Finding:* `hyper_util::client::legacy::Client` already provides a keep-alive connection **pool** (default 90s idle), and its `HttpConnector` does **Happy Eyeballs** (RFC 6555/8305, 300ms default), threaded DNS, and `SO_KEEPALIVE` (docs.rs/hyper-util). Chrome reaps idle *preconnected* sockets at ~10s and warns that over-preconnecting saturates the pool; preconnect leaks navigation intent (DNS+TCP+TLS) so it must be user-initiated and same-origin-biased, with `dns-prefetch` as the cheaper maybe-origin fallback (developer.chrome.com/Lighthouse, web.dev/resource-hints, MDN). *Disposition:* **reuse** hyper-util `legacy::Client` + `HttpConnector` for pooling/happy-eyeballs/DNS, wrapped with a rustls TLS connector (**reuse** `hyper-rustls`, small dep, or a tiny custom `tower::Service`) — replaces the current per-request `conn::http1::handshake`; this is also compatible with B1's streaming body (`Incoming`). **Hand-roll** the speculative preconnect trigger in the shell (hover/pointerdown → warm a pooled socket, ~10s idle cap, bounded concurrency), gated user-initiated; **defer** it until the pooled client + a shell hover signal exist. *Privacy constraint:* no unsolicited cross-origin preconnect without policy. *Roadmap:* resolve the §1 "connection reuse/pooling" and "speculative preconnect/DNS-prefetch" clauses to the above. No WPT target.
- **C1 realize hibernation.** *Finding:* Chromium splits it into two mechanisms that map onto our tiers — **freeze** (Energy Saver: after a tab is hidden+silent ~5min, stop JS/CPU and throttle DOM timers to ~1/min via "intensive wake-up throttling"; **keeps full RAM**; dispatches `freeze`/`resume`; audio/WebSocket/RTC tabs exempt) and **discard** (Memory Saver: unload the tab — resident drops from ~80–300MB active to ~5–10MB metadata/favicon, reloads on activation; this is the real RAM reclaim) (chromium.org tab-discarding, developer.chrome.com freezing/energy-saver). SpiderMonkey uses **compartments** for per-tab isolation (Firefox's own model) and **realms** for globals, loading self-hosted code once from a single compressed stream at init — there is **no V8-style deployable startup snapshot** (firefox-source-docs, spidermonkey.dev). *Disposition:* **hand-roll** the freeze/discard actions in the compositor (tiers exist, actions don't): background-CPU tier = *freeze* (throttle timers/JS task queue, keep DOM+layout, shrinking GC), hibernated tier = *discard* (drop fragment tree + rasterized tiles + the tab's realm, retain URL/DOM-source, relayout/re-fetch on wake — where RAM is actually reclaimed). **Reuse** SpiderMonkey compartment/realm-per-tab over the shared process-global runtime for low per-isolate baseline. *Reframe:* the §1 phrase "share SpiderMonkey startup snapshots across isolates" is inaccurate — replace with "compartment/realm-per-tab over one process-global runtime (SpiderMonkey has no V8 snapshot; self-hosted code loads once at init)." *Prereq:* per-tab baseline-RSS measurement (§8) to prove the freeze/discard deltas. No WPT target.
- **A1 Vello GPU paint tier.** *Finding:* the tier crates exist — full `vello` (GPU compute) and the "sparse strips" `vello_cpu` / `vello_hybrid` — but as of early 2026 sparse strips is **0.0.7, experimental, no API-stability guarantee** (any release likely breaking). `vello_cpu` is the most mature (near-beta, fastest Rust CPU renderer, now has Gaussian blur / drop-shadow / flood, non-layer clipping, gradients, opaque-image fills); `vello_hybrid` (GPU) and full `vello` remain alpha with known rough edges (complex-paint glyphs, high-winding paths, WebGL-on-Adreno, conflation). Even **Servo adopted Vello only for the bounded `<canvas>` surface, keeping WebRender for page compositing** (linebender.org TMIL blogs; github.com/linebender/vello releases; doc.servo.org/vello_cpu). *Disposition:* **defer** the focused-tab GPU tier (`vello`/`vello_hybrid`) on API instability + alpha rough edges — the existing `Painter` trait isolates the swap, so waiting is cheap and correctness-first. **Keep `tiny-skia`** as the CPU tier now; **track `vello_cpu`** as the eventual *richer* CPU tier (adds blur/shadow/clip we lack) and **reuse** it at its first stable (≥0.1) release; if prototyping the GPU tier sooner, pin an exact 0.0.x. *Roadmap:* §1 "Frame timing + rendering quality" resolves to — `Painter` seam ready → adopt `vello_cpu` at first stable release → GPU `vello_hybrid` after API stability + glyph fixes; `tiny-skia` correctness-first meanwhile. No WPT target (rendering quality → visual reftests, not WPT layout/DOM).

### 2. Web-traversal versatility (compatibility breadth)
Broadest real-world site set (excluding the hostile/bot-walled tier), WPT-verified.
- **Layout:** floats, tables, absolute/relative/fixed/sticky positioning, margin
  collapsing, CSS grid (via `taffy`), overflow/scroll containers, writing-modes, bidi.
- **CSS depth:** wire **Stylo**'s real cascade behind the `StyleEngine` adapter;
  media queries, `@font-face`, the full selector grammar, custom properties,
  `calc()`, transforms, transitions/animations, gradients.
- **JS + DOM:** grow the Web API bindings (`engine/js::bindings`) projecting the DOM
  into SpiderMonkey, WebIDL-driven, WPT-verified per API; event loop + timers;
  `fetch`/`XHR`; events; DOM mutation.
- **Loading:** external stylesheets/scripts/images; gzip/br/deflate decoding;
  charset sniffing; data/blob URLs.
- **Protocols:** activate **HTTP/2** (ALPN `h2`, already compiled), add **HTTP/3**
  (`quinn`), WebSocket (`tokio-tungstenite`), cookies.
- **Forms + navigation:** form submission, history (back/forward), fragment nav.

**Research dispositions (2026-07):**
- **B3 HTTP/2, HTTP/3, compression.** *Finding:* **h2** negotiates over TLS ALPN — advertising `h2` in the rustls connector lets hyper/hyper-util pick it automatically; multiplexing's win is real only with many same-origin **subresources**, so it is gated on external-resource loading (D4) — top-level-document-only fetch gains little over h1 keep-alive (B2). **h3** = `quinn` + hyperium `h3`/`h3-quinn` (h3-quinn still 0.0.x); browsers discover h3 via **Alt-Svc**/HTTPS-RR then **fall back to h2** if UDP/certs fail — so it needs an initial h1/h2 connection, Alt-Svc caching, a UDP path, and fallback logic (crates.io/h3-quinn, RFC 9114). **Compression**: `async-compression` exposes feature-gated tokio `AsyncRead` decoders for gzip/deflate/br/zstd (wrapping miniz_oxide/brotli), composing with B1's streaming body (github.com/Nullus157/async-compression). *Disposition:* **reuse** — (1) flip ALPN to `h2,http/1.1` once B2's pooled client lands, **defer the payoff** to D4; (2) **reuse** `quinn`+`h3` but **defer** h3 behind Alt-Svc discovery + h2 fallback (marginal win, pre-1.0); (3) **reuse** `async-compression` for `Content-Encoding` decode now (send `Accept-Encoding: gzip, deflate, br`), higher priority than h3 since real sites gzip/br regardless — brotli's static-dictionary size is tracked under C3. *Roadmap:* §2 "Protocols" resolves to h2-first (ALPN flip, D4-gated payoff) then deferred h3; add compression to §2 "Loading". No WPT target.

### 3. Lightweightness (binary size + footprint)
- Verify + measure the **musl static** Linux and **static-CRT** Windows builds;
  report binary size before/after size-affecting changes.
- Strip unused SpiderMonkey features (WASM, debugger, unused ICU locale data) via
  build config — size wins that do **not** cross the JS modification boundary.
- Dependency hygiene: the default engine/agent graph is already lean (0 `wgpu`/
  `winit` — correctly gated behind `shell`'s `gui` feature); remaining duplicates
  are transitive (see C3). Prefer `default-features = false`; audit with
  `cargo tree --duplicates` / `cargo bloat`.
- Minimize per-tab baseline resident memory; measure it.

**Research dispositions (2026-07):**
- **C2 SpiderMonkey build-size reduction (config-only, inside the boundary).** *Finding:* the real lever is **ICU locale data**, not code — `--without-intl-api` drops `Intl`+ICU entirely, but ICU *data* dominates size and en-US-only data is ~3–4MB vs the full-locale set (bugzilla 1402379, firefox-source-docs/intl). Crucially, `mozjs` **links a prebuilt archive by default** (why it "builds" in seconds here) that is Servo's fixed config **with** Intl+ICU; custom configure flags only apply when `MOZJS_FROM_SOURCE=1` forces the ~30-min from-source build (github.com/servo/mozjs README). *Disposition:* **defer, blocked on** (a) a `MOZJS_FROM_SOURCE=1` build path + patched SpiderMonkey configure invocation, and (b) per-flag binary-size measurement (§8) — the default prebuilt hides the win and cannot be trimmed as-is. When unblocked: baseline `--enable-optimize --disable-debug`, **trim ICU locale data** (keep `Intl` — real sites use `Intl.*`; never `--without-intl-api`), treat **WASM disable** as a measured compat tradeoff (increasingly used → default keep). Strictly build/config; never JIT/GC/sandbox. No WPT target.
- **C3 dependency-graph hygiene.** *Finding (own `cargo tree`, 2026-07):* the earlier "two `tiny-skia`/`cssparser` versions" note was inaccurate. In the **default** build there is **one** `cssparser` (0.34); the second (0.37) appears **only** under `--features stylo` (Stylo brings its own). The duplicate `tiny-skia 0.11.4` is a **transitive** of `winit → sctk-adwaita` (Wayland client-side decorations, `gui`+Linux only) — not a competing copy of our paint's 0.12. The default **engine + agent** graph pulls **0** `wgpu`/`winit`/`sctk` (all correctly gated behind `shell`'s `gui` feature); remaining dupes (hashbrown/foldhash/getrandom/linux-raw-sys/phf) are deep transitive churn with low payoff. *Disposition:* **no action needed structurally** — gating is already correct. Optional low-priority trim: drop winit's `sctk-adwaita` default feature if we forgo Wayland CSD (removes the 0.11.4 `tiny-skia`), a measured UX tradeoff. Adopt `cargo tree --duplicates` + `cargo bloat` + `cargo-udeps` as **standing** measurement tooling (§8) rather than one-off audits. No crate to add, nothing to hand-roll. No WPT target.

### 4. Agentic capability (headless phase)
- **Observation:** viewport-clipped text; an accessibility/semantic tree; element
  bounding boxes so the agent can click by coordinate, not only by link index.
- **Actions:** typing, form submit, back/forward, wait-for-load, scroll-to-element,
  tab management.
- **Reliability/cost:** structured tool-calling where supported; retry/rate-limit
  handling; screenshot/response caching; token budgeting; deterministic replay.
- **Backend breadth:** the `InferenceBackend` trait is provider-agnostic — add local
  (llama.cpp/Ollama) and other cloud backends without touching `run_task`.

### 5. Correctness & conformance
- Stand up the **upstream WPT runner** (`$WPT_DIR`); grow the built-in reftests.
- Secondary real-site screenshot diffing vs a real Chrome render.
- Property/fuzz testing of parsers + layout — but **never** the JIT/GC fuzzing that
  would tempt crossing the SpiderMonkey boundary; that stays vendored.

### 6. Portability
- Keep Linux/macOS/Windows co-equal: verify builds + rendering on all three (only
  Linux is exercised so far); no platform-primary; `io_uring` stays out of the GUI.

### 7. Security & process model
- Move toward isolate/compartment-per-tab multiplexed over a small number of OS
  processes (not one-per-tab); site isolation for untrusted content; keep untrusted
  JS inside the vendored SpiderMonkey sandbox; TLS stays `rustls`.

### 8. Measurement (cross-cutting)
The targets are only real if measured. Stand up benchmarks/CI for binary size,
per-tab baseline memory, click-to-navigate latency, and frame timing — so every
change can report before/after numbers as the working agreements require.

## Suggested crate layout

```
/engine
  /net        — hyper, quinn, tokio-tungstenite, rustls integration
  /html       — html5ever integration
  /css        — Stylo integration
  /layout     — taffy + custom block/inline/table/positioned/float layout, WPT-verified
  /js         — mozjs bindings, DOM/Web API surface, WPT-verified per API
  /text       — Parley, swash, fontdb
  /paint      — Vello integration (GPU/CPU/Hybrid tiers)
  /compositor — layer compositing, damage tracking, hibernation logic
/shell        — winit-based GUI: window, tabs, navigation, chrome
/tests
  /wpt        — WPT test harness and results tracking
```

## Working agreements

- Any change touching `/engine/layout`, `/engine/css`, or `/engine/js` should be checked against the relevant WPT subset before being considered complete — cite which tests were run.
- Any change touching binary size or per-tab memory should report before/after numbers.
- Treat SpiderMonkey as a vendored dependency you configure and bind against, not a codebase you patch internals of — see Modification boundaries above.
- When in doubt about whether a site belongs in the "broadest possible, excluding hostile" target set or the excluded tier, default to excluding rather than building bot-evasion logic into the core engine.
- **Documentation is part of "done."** On every major change — a new crate, a new capability, a wired feature seam, or a changed public API — update **both** `README.md` and this file in the same change. `README.md` must describe the stack *as it actually is* (no aspirational claims in the per-crate status; move aspirations to "Directions of improvement" below). Re-read and re-prioritize the "Current state" and "Directions of improvement" sections here periodically.
- The agent's inference is provider-agnostic: keep it behind the `InferenceBackend` trait (Groq is one implementation). Never commit API keys, `.env`, or the local-only development harness.