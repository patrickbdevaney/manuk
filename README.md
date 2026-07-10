# Manuk

A browser engine, built from first principles in Rust, per the directive in
[`CLAUDE.md`](./CLAUDE.md). One shared engine core drives two front-ends:

- a **headful, human-operator GUI browser** (`shell`), and
- a **headless agentic browser** (`agent`) that an LLM can drive.

This is a **working foundation**, not a finished browser — a standards-complete
engine is the scope of Servo/Chromium. What exists is the full crate architecture
the directive mandates, real dependencies wired in, and two working vertical slices
that load real pages over HTTPS and render them.

> **Snapshot (kept current):** 13 workspace crates · **40 tests pass, 0 warnings** ·
> renders real sites to pixels · SpiderMonkey evaluates real JS behind a feature ·
> the agent is live-tested end-to-end against Groq `qwen/qwen3.6-27b`.
> This README documents the **entire stack as it actually is**, and is updated on
> every major change (see [Maintenance](#maintenance)).

```
                          ┌── shell → winit/wgpu window       (headful)
 net → html → dom → css → layout → text → paint ──┤
                          └── agent → screenshot + LLM loop   (headless)
```

Rendering `https://example.com/` and a local test page (the CPU raster tier —
the same rasterizer the agent screenshots):

![Rendered example.com](docs/example.png)

![Rendered local sample page](docs/sample.png)

---

## Quick start

```bash
# Headful engine ------------------------------------------------------------
# Headless render to PNG (no GPU/display needed):
cargo run -p manuk-shell --no-default-features -- render https://example.com/ -o out.png --width 800
# Interactive GPU window (winit + wgpu; needs a display):
cargo run -p manuk-shell -- browse https://example.com/

# Agentic browser (needs a Groq API key) ------------------------------------
cp .env.example .env            # then put GROQ_API_KEY=... in it
cargo run -p manuk-agent --bin agent-run -- "What is this page's main heading?" https://example.com/

# JavaScript via SpiderMonkey (heavy feature) -------------------------------
cargo test -p manuk-js --features spidermonkey

# Conformance + tests -------------------------------------------------------
cargo run -p manuk-wpt          # built-in layout reftests
cargo test --workspace          # 40 tests, all crates
```

## Repository layout

```
engine/
  net/         HTTP(S) fetch + general request (hyper, rustls, tokio)
  html/        HTML parsing (html5ever) -> DOM
  dom/         arena DOM tree (shared core; no JS dependency)
  css/         style engine: minimal cascade (+ Stylo behind a feature)
  layout/      from-scratch block/inline layout (+ taffy for flex)
  text/        font discovery + shaping + rasterization (fontdb, fontdue)
  js/          JsRuntime trait + no-op default (+ SpiderMonkey behind a feature)
  paint/       display list + CPU raster tier (tiny-skia) -> PNG
  compositor/  tab tiers / hibernation, damage tracking, scroll
  page/        the shared pipeline: bytes -> DOM -> style -> layout -> paint
shell/         headful GUI: `render` (headless PNG) + `browse` (winit/wgpu window)
agent/         headless agentic browser: driver + backend-agnostic loop + Groq
tests/wpt/     Web Platform Tests harness + results tracking
docs/          sample page + rendered screenshots
```

---

## The stack, layer by layer

Every crate below is present and builds. **Reuse** = a mature upstream crate wired
in; **Build** = written from scratch, to be verified against WPT.

### Engine core

#### `engine/net` — networking · *Reuse*
- **Does:** `fetch(url)` (GET + redirect following) and a general
  `request(method, url, headers, body)` (POST etc.). Returns status, headers, a
  (Content-Encoding-decoded) `Bytes` body, and the negotiated `HttpVersion`.
- **Uses:** a process-global pooled `hyper-util` `legacy::Client` over a
  `hyper-rustls` `HttpsConnector` (ALPN `h2,http/1.1`), `rustls` 0.23 (pure-Rust TLS
  via `ring` — no OpenSSL), `webpki-roots`, `async-compression` (gzip/deflate/br)
  over the streaming body, `tokio`.
- **Works:** live HTTPS fetches of real sites; **HTTP/2 auto-negotiated** (verified
  on example.com); **connection pooling** (sequential same-origin fetches reuse the
  socket); **gzip/br/deflate decoding** (verified on httpbin.org/gzip); redirects;
  the Groq client reuses the same stack for outbound LLM calls.
- **External subresources:** `<link rel=stylesheet>` is fetched and applied before
  paint (render-blocking; `Page::fetch_and_apply_stylesheets`, wired into shell
  `render`); `<script src>`/`<img src>` are enumerated with `defer`/`async` semantics
  (`Page::subresources`) — script execution + image rendering are follow-ons.
- **Not yet:** HTTP/3/QUIC (`quinn`) is a target, not yet a dependency; speculative
  preconnect on hover; cookies/cache.

#### `engine/html` — HTML parsing · *Reuse*
- **Does:** `parse(html) -> Dom`, walking `html5ever`'s spec-compliant tree builder
  (via `markup5ever_rcdom`) into our arena DOM.
- **Uses:** `html5ever` 0.39, `markup5ever_rcdom`.
- **Works:** full error recovery, implied tags, malformed input.
- **Streaming:** `StreamParser` drives html5ever incrementally (feed chunks,
  snapshot the parsed-so-far tree) → `Page::load_streaming` first-paints `<head>` +
  above-the-fold before the tail arrives (~113x sooner than full-load in the bench).
- **Not yet:** encoding sniffing at the parser layer (UTF-8 assumed; the net layer
  does WHATWG charset decode); a chunked-body fetch to feed the stream from a real
  socket (`fetch()` buffers today).

#### `engine/dom` — the DOM tree · *Build*
- **Does:** an arena (`Vec`-indexed `NodeId`) tree of Document/Doctype/Element/
  Text/Comment, with sibling/child links, attributes, class/id helpers, pre-order
  descendants, and text-content extraction.
- **Uses:** `smallvec`. **No JS dependency** — deliberately (see [deviations](#deviations)).
- **Not yet:** the mutable Web API surface (`appendChild`, ranges, live
  collections) — that is the large-volume follow-on.

#### `engine/css` — style engine · *Reuse target (Stylo), Build (fallback)*
- **Does:** a `StyleEngine` trait producing a `ComputedStyle` per node. Default
  `MinimalCascade` parses `<style>` + inline `style=""`, matches tag/id/class/`*`
  selectors and the descendant combinator, applies specificity + source-order +
  `!important`, inherits inherited properties, and resolves a UA default sheet
  (block/inline/none, default margins, headings, bold). Property support covers
  the box model + text (color, background, font, margins/padding, width/height,
  text-align, white-space, line-height). `cssparser` handles length/number tokens.
- **Uses:** `cssparser` 0.34; **Stylo** 0.19 behind `--features stylo`.
- **Works:** cascade/specificity/inheritance/descendant matching (6 tests).
- **Not yet:** Stylo's real cascade (the feature links + builds; the adapter
  currently delegates to `MinimalCascade`); combinators `> + ~`, attribute/pseudo
  selectors, `@media`/`@font-face`, most shorthands, calc, custom properties.

#### `engine/layout` — layout · *Build (+ taffy for flex)*
- **Does:** builds a fragment tree of absolutely-positioned boxes from DOM +
  computed styles. Implements **block** formatting (normal-flow vertical stacking,
  the box model, `auto` width fill, `auto`-margin centering, **adjacent-sibling
  margin collapsing**), **inline** formatting (greedy line-breaking with real font
  measurement, per-line vertical metrics, text-align, **flow-around floats**),
  **floats** (a BFC-aware `FloatContext`: left/right placement, horizontal
  stacking + band-drop, `clear`, shrink-to-fit), **positioning**
  (`relative`/`absolute`/`fixed` against the containing-block chain), and **tables**
  (`display:table` with fixed/auto column algorithms + `border-spacing`).
  `display:flex` routes through `taffy`.
- **Uses:** `taffy` 0.12; consumes `manuk-text` for measurement.
- **Works:** stacking, wrapping, centering, document height, flex rows, margin
  collapse, float placement/clear/flow-around, relative/absolute/fixed positioning,
  fixed/auto table layout (21 tests + an end-to-end table PNG render).
- **Documented simplifications:** margin collapse is adjacent-sibling only
  (no parent↔child); positioning has no `sticky`, no `z-index` ordering, and leaves
  inset-less (static-position) abs boxes unplaced; tables use the separated model
  (no `colspan`/`rowspan`, `border-collapse`, or captions); percentage heights only
  against definite containers; inline is Latin/LTR with an inter-word space between
  adjacent tokens.
- **Not yet:** `sticky`, grid (native), writing-modes, bidi, `border-collapse`,
  table spanning.

#### `engine/text` — text · *Reuse*
- **Does:** `FontContext` discovers system fonts, resolves faces by
  family/weight/style, returns line metrics, measures runs, shapes glyphs (pen
  positions), and rasterizes glyphs to 8-bit coverage bitmaps.
- **Uses:** `fontdb` 0.23 (discovery), `fontdue` 0.9 (metrics + raster) — the
  lower layers of the Parley/swash family named in the directive.
- **Works:** Latin measurement, shaping, and rasterization.
- **Not yet:** complex-script shaping, bidi, ligatures/kerning, `@font-face`
  loading — Parley's remit.

#### `engine/js` — JavaScript · *Reuse (feature-gated)*
- **Does:** a `JsRuntime` trait. Default `NoScriptRuntime` is a no-op (correct for
  the no-JS default). **SpiderMonkey** (`mozjs`) behind `--features spidermonkey`
  boots a process-global engine (shared across isolates), evaluates script, and
  returns typed values.
- **Uses:** `mozjs` 0.18 / `mozjs_sys` 140 (the Servo integration path — **not
  V8**). In this environment it builds from a prebuilt in seconds.
- **Works:** real evaluation under the feature (`40+2 == 42`, etc.). **DOM bindings**
  — a hand-written jQuery-core subset over the arena DOM on a thin safe helper layer
  (reserved-slot `NodeId`, the Servo mechanism, no `Dom<T>`). **Methods:**
  `document.getElementById`/`querySelector`/`querySelectorAll`/`createElement`,
  `element.appendChild`/`setAttribute`/`getAttribute`/`querySelector`/
  `querySelectorAll`. **Accessor properties:** `textContent` (get+set), `tagName`,
  `id` (get+set), `className` (get+set), `innerHTML` (get+set, re-parses a
  fragment into the arena DOM). `querySelectorAll` returns a real JS array
  NodeList. **Events:** `addEventListener`/`dispatchEvent` (synchronous dispatch +
  dispatch scheduled through the event loop). **Event loop:** a real I/O loop — `setTimeout`
  macrotasks + `queueMicrotask` microtasks (spec-correct ordering) + **`fetch`
  (thenable) and `XMLHttpRequest`**, whose network I/O the loop performs via an
  injected fetcher (production wires `manuk-net`; `engine/js::event_loop`). JS mutates the real arena DOM; validated end-to-end
  (isolated tests).
- **Boundary:** this crate only *configures and binds to* SpiderMonkey — never
  patches JIT/GC or the sandbox. See [the modification boundary](#the-js-engine-modification-boundary).
- **Not yet:** an `Event` object + capture/bubble propagation, `fetch` response
  streaming/headers, and native-`Promise`-queue integration (blocked on a crashing
  mozjs-0.18 wrapper) — the documented next tranches.

#### `engine/paint` — rasterization · *Reuse target (Vello), Build (CPU tier)*
- **Does:** flattens the fragment tree to a `DisplayList`, then a `Painter` renders
  it. The **CPU tier** (`CpuPainter`) fills rects with `tiny-skia` and alpha-blits
  `fontdue` glyph coverage, producing a `Canvas` that encodes to PNG (and to RGBA
  bytes for GPU upload). Supports a scroll offset.
- **Uses:** `tiny-skia` 0.12, `fontdue`.
- **Works:** backgrounds, text, PNG output; deterministic and headless (3 tests).
- **Not yet:** the Vello GPU-compute tier (the directive's quality lever) — it
  slots behind the same `Painter` trait; borders/gradients/blur/clips/images.

#### `engine/compositor` — composite policy · *Build*
- **Does:** the per-tab memory model — `TabManager` assigns tiers (focused-GPU,
  background-CPU, hibernated/evicted) with LRU eviction beyond a budget; `Damage`
  accumulates dirty rects and unions them; `Viewport` clamps scroll and marks
  damage.
- **Works:** tier transitions, damage union, scroll clamping (4 tests).
- **Not yet:** the actual GPU layer compositing (lives in `shell`); tile caching.

#### `engine/page` — the shared pipeline · *Build*
- **Does:** ties the core together: `Page::load` (fetch→parse→cascade→layout),
  `relayout` (new width), `paint`/`paint_scrolled`, `links()` (anchors with hrefs
  resolved absolute), `visible_text()`. `fetch_html` supports `http(s)`/`file`/
  local paths. **This is the concrete "headful and headless share the core."**
- **Works:** load, layout, link/text extraction (2 tests).

### Front-ends

#### `shell` — headful GUI · *Build*
- **`render <url> -o out.png [--width N] [--height N]`** — headless: runs the full
  pipeline and writes a PNG. No GPU/display needed.
- **`browse <url>`** (feature `gui`, on by default) — opens a `winit` 0.30 window
  and presents the CPU raster as a `wgpu` 27 fullscreen textured quad; mouse-wheel
  scrolls, resize reflows. Compiles here; running needs a display.
- **Tabs:** a `Browser`/`Tab` model over the compositor's `TabManager`.

#### `agent` — headless agentic browser · *Build*
See [The agentic browser](#the-agentic-browser).

### Conformance

#### `tests/wpt` — Web Platform Tests harness
- **Does:** `run_layout_suite` runs built-in layout reftests (expressed against the
  real engine) and reports pass/fail/skip as text + JSON (`Report`).
  `find_wpt_checkout` (via `$WPT_DIR`) is the hook for the upstream runner.
- **Runner:** `cargo run -p manuk-wpt` (5 reftests pass today).

---

## Data flow

**Page load (both front-ends):**

```
URL ─fetch_html─▶ HTML bytes ─html5ever─▶ DOM ─MinimalCascade─▶ ComputedStyle map
      │                                        │
      └───────────────── layout_document ◀─────┘
                              │
                     fragment tree (absolute rects + text runs)
                              │
                    ┌─────────┴──────────┐
              CpuPainter → PNG      DisplayList → (Vello GPU: future)
```

**Agent loop (`run_task`):**

```
observe (url, title, links[i], text, +screenshot PNG)
      │
      ▼
InferenceBackend.complete(messages)  ──►  Groq (qwen/qwen3.6-27b)
      │                                        │ strips <think>…</think>
      ▼                                        ▼
parse last JSON action  ◀──────────────  assistant reply
      │
      ├─ navigate{url} / click{index} → AgentBrowser.navigate
      ├─ scroll{dy}                   → AgentBrowser.scroll_by
      └─ finish{answer}               → done
```

## Feature flags

| Crate | Feature | Default | Effect |
|---|---|---|---|
| `manuk-js` | `spidermonkey` | off | Real JS via `mozjs`; else no-op runtime |
| `manuk-css` | `stylo` | off | Links Stylo; adapter present (delegates for now) |
| `manuk-shell` | `gui` | **on** | The `winit`/`wgpu` window; off = headless `render` only |

## The agentic browser

The agent side is layered so the pieces are independently testable and swappable —
and, per the brief, **the agent logic is decoupled from both the harness driving it
and the inference backend**:

- **`AgentBrowser`** — headless page driver over `engine/page`. Knows nothing about
  LLMs: `navigate`, `scroll_by`, `screenshot_png`, `observe` (URL, title, links by
  index, visible text, scroll position). Renders via the CPU tier — display-free
  and deterministic.
- **`InferenceBackend`** — the provider-agnostic model trait
  (`async fn complete(&[Message]) -> String`), object-safe and multimodal
  (`Content::Text` + `Content::ImagePng`). `GroqBackend` is one implementation; it
  posts to Groq's OpenAI-compatible endpoint **through `engine/net`** (hyper +
  rustls — no separate HTTP client, no OpenSSL) and strips `<think>…</think>`
  reasoning blocks that qwen/DeepSeek-style models emit.
- **`run_task`** — the observe→decide→act loop. Takes `&dyn InferenceBackend` +
  `&mut AgentBrowser`; never names a provider or a harness. Actions are a small JSON
  protocol: `navigate` / `click` / `scroll` / `finish`, robustly extracted from the
  model's reply (reasoning stripped, last balanced `{…}` parsed).

Default model: `qwen/qwen3.6-27b` (multimodal, Groq), overridable via `GROQ_MODEL`.

### Runners and keys

- **`agent-run`** (committed) drives the agent with a **single** `GROQ_API_KEY`
  (falls back to `GROQ_API_KEY` from `.env`).
- **`agent-run`** is a **local-only** capability harness that cycles
  `GROQ_API_KEY..N` (one key per test, to spread rate limits) across the full
  capability set. It is **gitignored** — only the single-key runner is committed.
  It reuses the exact same `run_task`; it is just a driver.
- `.env` (holding keys) is gitignored and never committed; see `.env.example`.

Live capability run (qwen/qwen3.6-27b, screenshots rendered by our own engine):

```
[1/4] text-extraction        PASS  answer: Example Domain
[2/4] link-comprehension     PASS  answer: Example Domain site
[3/4] link-navigation        PASS  answer: Example Domain     (real network hop)
[4/4] multimodal-screenshot  PASS  answer: light              (read our engine's PNG)
4/4 capabilities passed
```

## The JS-engine modification boundary

Per CLAUDE.md's most important section: `engine/js` **configures and binds to**
SpiderMonkey (`mozjs`, the Servo path — not V8). It never patches SpiderMonkey's
JIT (Warp/Ion) or GC internals, nor the sandbox — a "come back to the human"
boundary, because JIT miscompilation is historically the largest source of
exploitable browser RCE and the reason SpiderMonkey is trustworthy is years of
adversarial fuzzing this project has no equivalent of.

## Performance posture

Hooks aligned with the directive's targets are in place; measurement is the ongoing
work (see [CLAUDE.md frontiers](./CLAUDE.md)).

- **Binary size:** release profile is `opt-level="s"`, `lto=true`,
  `codegen-units=1`, `panic="abort"`, `strip=true`. `.cargo/config.toml` adds
  static-CRT (Windows) / musl (Linux) target flags for fully-static binaries,
  opt-in per target.
- **Per-tab memory:** the compositor models isolate-per-tab tiers (focused GPU +
  active JS; background CPU + frozen JS; hibernated/evicted); SpiderMonkey is a
  process-global engine shared across isolates.
- **Latency:** `Bytes`-based response bodies and html5ever's streaming sink are
  positioned for incremental parse/layout (not yet wired).

## Testing & conformance

- `cargo test --workspace` — **40 tests**, zero warnings.
- Feature builds verified: `--features spidermonkey` (JS eval), `--features stylo`
  (links), `--all-features` (all together).
- `cargo run -p manuk-wpt` — built-in layout reftests; `$WPT_DIR` is the hook for
  the upstream WPT runner.
- **CI** (`.github/workflows/ci.yml`): build+test on Linux/macOS/Windows, fmt+clippy,
  and static-release binaries (musl / static-CRT / macOS framework) that each smoke-
  render a PNG to prove they *run*. Cross-platform verification status is tracked in
  [`PLATFORM.md`](./PLATFORM.md) — Linux is verified locally; macOS/Windows are
  engineered-for-portability and gated in CI (await first green run).

## Deviations

- **`engine/dom` is its own crate.** The directive groups "DOM + Web API surface"
  under `/engine/js`; but the DOM *tree* is consumed by html/css/layout, none of
  which should depend on the JS engine. So the tree lives in `engine/dom` and
  `engine/js` holds the *bindings*. Keeps the JS feature gate off the parse/layout
  path.
- **`engine/page`** is the concrete realization of "headful and headless share the
  core, diverge at consumption."

## Maintenance

**This README and `CLAUDE.md` are updated on every major change** — a new crate, a
new capability, a wired feature seam, or a changed public API. The README documents
the stack *as it actually is* (no aspirational claims in the per-crate status); the
directions of improvement live in [`CLAUDE.md`](./CLAUDE.md) and are reviewed
periodically.

## Roadmap

The full, objective-organized roadmap (performance, web-traversal versatility,
lightweightness, agent capability, correctness, portability, security) lives in
[`CLAUDE.md` → Directions of improvement](./CLAUDE.md). Nearest term: layout
floats/tables/positioning, external stylesheet + image loading, the real Stylo and
Vello integrations, and viewport-clipped agent observation.

## License

MPL-2.0 (see workspace manifest).
