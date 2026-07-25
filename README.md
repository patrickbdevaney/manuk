# Manuk

[![CI](https://github.com/patrickbdevaney/manuk/actions/workflows/ci.yml/badge.svg)](https://github.com/patrickbdevaney/manuk/actions/workflows/ci.yml)
[![demo](https://github.com/patrickbdevaney/manuk/actions/workflows/demo.yml/badge.svg)](https://github.com/patrickbdevaney/manuk/actions/workflows/demo.yml)

### ▶ **[Run the engine in your own browser →](https://patrickbdevaney.github.io/manuk/)**

Not a screenshot, not a video. That link compiles this engine to WebAssembly and executes it **in your
browser** — **Stylo**'s cascade, **Taffy**'s flex/grid, **tiny-skia**'s rasterizer, and this engine's own
DOM / layout / paint / accessibility tree, rendering real page snapshots onto a canvas. It has three views:

- **Render** — the pixels. Scroll it and scrolling **re-renders**; it does not pan a bitmap. Hover it and
  that is the real hit-test against the laid-out boxes.
- **Agent view** — the **accessibility tree an LLM drives it through**: role + accessible name + interaction
  state + a click box, computed with no JavaScript from the parsed DOM and the solved layout. Hover a node
  to ground it on the page. This is the observation channel the headless agent gets *instead of* a
  screenshot, and it is the whole thesis of the project made visible.
- **Boxes** — every rectangle Taffy and our inline/float/table code solved, coloured by nesting depth.

And **compare with Chromium** puts our render next to Chromium's of the same document, so none of it has to
be taken on trust.

*What is **not** real in the wasm demo (the native browser does far more — see [What works](#what-works)),
said in-product too:* **no JavaScript** and **no live fetching** (bundled snapshots). SpiderMonkey *can*
target wasm, but only as an **interpreter** — WebAssembly forbids the runtime code generation a JIT is — and
it ships as a separate WASI module that cannot be linked into this binary. Saying so is the only thing that
makes the rest believable. (The Agent view needs no JS: an a11y tree is a pure function of the DOM and the
geometry.)

---

A browser engine built from scratch in Rust — **~129k lines across 18 crates** — with one shared engine
core driving three front-ends: a **headful GUI browser** (`shell`), a **headless agentic browser** (`agent`)
an LLM can drive, and the **wasm demo** above.

The goal is a **daily driver**: fast, lean, and correct enough on the *breadth* of the real web to actually
use — and, uniquely, **agent-native**: the same core exposes a structured surface an automation framework or
an LLM can operate directly. Chromium parity across the whole web platform is the scope of a large team;
what follows is an honest account of how far this has actually got, measured rather than asserted.

---

## Where this actually is

**Phase 0 (daily-driver capability) is in progress and near its exit.** Development runs on a differential
oracle: **265 real sites across 15 design-pattern classes** are rendered by *both* Chromium and this engine
from **one snapshot**, diffed by structural path, and the divergences **clustered by root cause**. The
cluster ranking — ranked by *distinct sites explained*, not by hit count — **is** the priority ledger. No
feature is picked by taste.

**The first principle is a ratchet.** Every unit of work must leave the engine strictly more capable than it
found it, and nothing that worked before may work less well after — across **capability**, **performance**,
and **instrument fidelity**. A capability is only *banked* once a gate proves it can go red; that proven gate
is the ratchet tooth. Progress only turns one way.

Three bars, and they are never conflated:

| | | |
|---|---|---|
| **Bar 0** | *Does the engine ever take the browser down?* | The floor. Checked before correctness is even asked. |
| **Bar 1** | *Is the page legible, navigable, not visibly broken?* | The near-term target. |
| **Bar 2** | *Is it pixel-exact?* | **Deliberately deferred.** Breadth beats depth until Bar 1 is real. |

The **north star**, one sentence, decides what "done" means: *Chromium is the **ceiling** on capability and
the **floor** on everything else.* Match what a page can do in Chrome (scripts run, layout resolves, forms
submit, media plays); **beat** Chrome on speed, stability, and resource use. A structural divergence is a
bug; a timing divergence in our favour is the point.

On the last full oracle run the engine rendered the node Chromium renders **~92% of the time**, was **faster
than Chromium on ~92% of the corpus** (median roughly **2×**), and **crashed zero times** (a panic kills the
page, not the browser). The live figures — tick number, hang count, cluster count — are generated into
[`STATUS.md`](./STATUS.md); it is machine-written from the filesystem, git, and the verify receipt, never
hand-edited.

**The Phase-0 exit is a certificate, not a percentage.** It is not `ready_pct` (retired) and not a WPT
count: it is a fidelity certificate measured by a rebuilt instrument on a stratified corpus — Bar 0 clean,
the four *jarring* invariants (overlap / horizontal-overflow / reading-order / dead-click-target) ≥ 95%,
parent-relative shape ≥ 0.75 on ≥ 95% of nodes, interactivity ≥ 95%, and only **named** exceptions. That is
the finish line, and it is finite (see [Roadmap](#roadmap)).

---

## What works

**Rendering.** Real sites over HTTPS: block / inline / flex / grid / table / float / positioning, the box
model, stacking contexts and `z-index`, `overflow` clipping, `border-radius`, gradients, backgrounds,
shadows, transforms. The **live cascade is Stylo** (Firefox's) — `@media` / `@supports` / `@layer`, custom
properties, container queries, `:has()`, and CSS Level-4 math (`round()` / `mod()` / `abs()` resolve to
exact used values). Stylo and SpiderMonkey are embedded as sanctioned FFI dependencies — never patched
internally.

**JavaScript (native).** SpiderMonkey with real DOM/BOM/CSSOM: event dispatch with capture/bubble,
`fetch` / `XHR`, timers, promises + microtasks (spec ordering), custom elements + shadow DOM,
`MutationObserver` / `IntersectionObserver` / `ResizeObserver`, **ES modules** (import maps + a real import
graph, so CDN no-bundler apps boot), `<canvas>` 2D (paths, gradients, patterns, `getImageData` / `toDataURL`,
`createImageBitmap`), the **Web Animations API** (`element.animate`), **View Transitions**, the **Navigation
API**, **IndexedDB**, the **Cache API**, **Web / Service Workers** (same-thread), **CSP**, `Blob`, the
**Sanitizer API**, popover, `scheduler.postTask`, and `crypto.subtle` on a real CSPRNG. Eight real SPA
framework bundles (React, Vue, Svelte, Solid, Preact, Lit, Vanilla — Vite production output) mount and
render.

**Media.** `<video>` **plays**: MP4 demux → **H.264** video decode and **AAC** audio decode → decoded frames
composited into the page → a real playback clock (`timeupdate` / `ended`, `currentTime` writes are true seeks
firing `seeking` / `seeked`, `played`, `durationchange`), plus tracks / captions. MSE `SourceBuffer` is
partial. EME / Widevine is **permanently out of scope** (a licensed proprietary CDM), so Netflix / Spotify
are unreachable — stated once, not relitigated.

**Interactivity & shell.** Click links, focus and type into fields, submit forms (`<form method=POST>`
navigation, multipart `FormData` uploads, constraint validation), toggle controls, scroll, tabs (open /
close / switch, **hibernated** background tabs), history, bookmarks, find-in-page, zoom, cookies (RFC 6265,
public-suffix-aware, `SameSite` + `__Host-` / `__Secure-` prefixes), partitioned storage, session restore, a
password **vault + origin-scoped autofill**, and streaming **download-to-disk** (multi-GB files, no OOM). The
request path carries headers and bodies faithfully with CORS enforcement.

**Agent-native.** The same engine core, headless: an **accessibility tree** (roles, accessible names,
interaction state, focus, geometry) as the agent's observation channel, in-process automation
(selectors / wait / assert), **WebDriver BiDi**, occlusion-aware hit-testing, and an `InferenceBackend`
trait so any provider — local `llama-server`, Ollama, or a hosted endpoint — drives it identically. Agent
actions fire the *real* DOM events (`input` / `keydown` / `focus` / `blur` / `change`), so React- and
Vue-controlled inputs actually update. Page text reaches a model only through an untrusted-content fence.

## What doesn't (yet), stated plainly

A README that only lists wins is marketing.

- **Rich editing** — `contenteditable` + document `Selection` + editing commands (Gmail-compose / Notion
  class), and **IME / composition** (CJK typing) — are Tier-1 remaining work, not yet done.
- **WebAuthn / passkeys** — passkey-only sites are still hard walls (TOTP fallback covers the rest).
- **Pixel precision (Bar 2)** is deferred, not achieved.
- **No JavaScript in the wasm demo** — SpiderMonkey is C++ and does not target wasm; the in-browser demo is
  render-only (and says so on its own front page).
- **SpiderMonkey can fault inside its own C++ frames**, uncatchable in-process — an open Bar-0 residual whose
  real fix is one OS process per tab (a **decided** architecture, sequenced into Phase-1 security work).
- **Out of scope by decision** (feature-detected cleanly, not half-built): **EME/DRM**, **WebRTC**,
  **WebGPU + heavy-WebGL creative apps** (Figma / Canva tier), a niche modern-CSS tail (subgrid, `@scope`,
  anchor positioning, scroll-driven animations, `text-wrap: balance`, JPEG-XL, WebCodecs), and
  HTTP/3 / QUIC. Each is a named exception with a reason, not a silent gap.

---

## How it is developed

> **Three instruments, and they see different things.** The **differential oracle** (265 real sites vs
> Chromium) finds what real pages do. **Web Platform Tests** finds what the *spec* says, needs no oracle,
> and sees the adversarial cases no real site generates — its first run found a Bar-0 hang
> (`child.after(child)`) no crawl could surface, and that `DOMContentLoaded` / `load` had never been
> dispatched. The **fidelity instrument** (parent-relative shape scoring + the four jarring invariants) is
> the Phase-0 exit gate. Cumulative findings are captured by topic in **`docs/wiki/`**.

**One capability per tick, highest-leverage first.** Each commit lands exactly one verify-gated capability;
the loop attacks the largest same-root-cause cluster, weighted toward *daily-driver* leverage rather than
raw WPT count. A subsystem-sized lever (media, ESM graph) is **decomposed** into independently landable
bricks rather than stalling the loop. The ratchet is absolute: a Bar-0 crash or *any* measured regression is
reverted, never traded for a feature.

**The gates** run as one wall (`scripts/verify.sh`, ~60–190s) and are all-or-nothing — build, `parity`
box-geometry probes within ±3px of headless Chrome, real-site fidelity, JS conformance, clickability, plus
purpose-built gates each born from a user-visible failure every existing gate slept through:

| | |
|---|---|
| `G_CONTAIN` | **Bar 0** — a panic kills the page, not the process |
| `G_HANG` | every crawled site under a watchdog; a timeout is a hard, counted, *attributed* failure |
| `G_ALLOC` | per-input-event allocation rate (born from a scroll freeze every other gate called green) |
| `G_LOAD` | a dead subresource cannot hold the document hostage |
| `G_INTERACT` | tab open / switch / close stay under one frame — with real pages in 30 tabs |
| `G_SILENT_FAIL` | an error on the load / render / script path that is swallowed |
| `G_CLEAN_EXIT` | a process that ran JavaScript exits 0 |
| `F1` / `F2` | cascade ≤ 40ms, full pipeline ≤ 125ms — asserted, not eyeballed |

**Compliance is mechanical, not remembered.** A long session degrades on exactly the clauses that depend on
being recalled, so they were moved into tooling: the **gate receipt** records the git *tree* verified and a
pre-commit hook refuses a commit whose staged tree differs; the **journal is enforced** (no commit without a
tick entry, written *before* the work as a hypothesis); a tick claiming a pattern-class fix must **name the
oracle cluster** it closes; the **self-audit is unavoidable** past 10 ticks overdue. Every one of these has
refused *its own author* at least once — that is the mechanism working.

---

## Quick start

```bash
# Headful engine ------------------------------------------------------------
# Headless render to PNG (no GPU/display needed):
cargo run -p manuk-shell --no-default-features -- render https://example.com/ -o out.png --width 800
# Interactive GPU window (winit + wgpu; needs a display):
cargo run -p manuk-shell -- browse https://example.com/

# Agentic browser (needs a provider API key, or a local llama-server) -------
cp .env.example .env            # then set your provider credential in it
cargo run -p manuk-agent --bin agent-run -- "What is this page's main heading?" https://example.com/

# JavaScript via SpiderMonkey; live cascade via Stylo (heavy features) -------
cargo test -p manuk-js --features spidermonkey
cargo test -p manuk-css --features stylo

# The in-browser wasm demo (static site → demo/www) -------------------------
./scripts/demo-build.sh

# Conformance + tests -------------------------------------------------------
cargo run -p manuk-wpt          # built-in layout reftests
cargo test --workspace
```

## Repository layout

```
engine/
  net/         HTTP(S) fetch + general request (hyper, rustls, tokio)
  html/        HTML parsing (html5ever) -> DOM
  dom/         arena DOM tree (shared core; no JS dependency)
  css/         style engine: Stylo cascade (live) + a minimal fallback
  layout/      from-scratch block/inline/float/table layout (+ taffy for flex/grid)
  text/        font discovery + shaping + rasterization
  js/          JsRuntime trait + no-op default (+ SpiderMonkey behind a feature)
  media/       container demux + H.264/AAC decode -> decoded frames for <video>
  a11y/        accessibility / semantic tree over the DOM (screen readers + the agent channel)
  paint/       display list + CPU raster tier (tiny-skia) -> PNG
  compositor/  tab tiers / hibernation, damage tracking, scroll
  page/        the shared pipeline: bytes -> DOM -> style -> layout -> paint
shell/         headful GUI: `render` (headless PNG) + `browse` (winit/wgpu window), tabs, session
agent/         headless agentic browser: driver + backend-agnostic loop + inference backends
store/         local encrypted password store + origin-scoped autofill
demo/          the engine compiled to wasm — the in-browser demo (this repo's GitHub Pages site)
tests/wpt/     Web Platform Tests harness + results tracking
docs/          wiki (findings by topic), the loop's methodology + status, sample pages
```

## The stack, layer by layer

Every crate is present and builds. **Reuse** = a mature upstream crate wired in; **Build** = written from
scratch, verified against WPT and the oracle.

| Crate | Role | Kind | State |
|---|---|---|---|
| `engine/net` | `fetch` + general `request` over pooled hyper + rustls (pure-Rust TLS), HTTP/2, gzip/br/deflate, streaming | Reuse | live HTTPS, connection pooling, streaming first-paint |
| `engine/html` | `parse(html) -> Dom` via html5ever; incremental `StreamParser` | Reuse | full error recovery; streaming above-the-fold |
| `engine/dom` | arena (`Vec`-indexed) DOM tree, the mutable Web API surface | Build | the shared core; **no JS dependency** by design |
| `engine/css` | `StyleEngine` producing `ComputedStyle`; **Stylo is the live cascade** | Reuse (Stylo) | cascade / specificity / inheritance / `@`-rules / container queries / `:has()` |
| `engine/layout` | block / inline / float / table / positioning / stacking; flex + grid via Taffy | Build (+Taffy) | wrapping, floats, tables, abs/fixed, margin collapse |
| `engine/text` | font discovery, shaping, glyph raster | Reuse | Latin measure / shape / raster; complex-script is the frontier |
| `engine/js` | `JsRuntime` trait; SpiderMonkey behind `--features spidermonkey` | Reuse (mozjs) | DOM bindings, event loop, ESM, the platform APIs listed above |
| `engine/media` | container demux + H.264 / AAC decode → decoded frames | Build (+symphonia/openh264) | `<video>` plays; MSE partial; EME out of scope |
| `engine/a11y` | role + accessible-name + state tree over the DOM | Build | screen-reader source **and** the agent observation channel |
| `engine/paint` | display list → CPU raster (tiny-skia) → PNG / RGBA | Build | backgrounds, text, images, borders, gradients, clips |
| `engine/compositor` | per-tab tiers (focused-GPU / background-CPU / hibernated), damage, scroll | Build | tier transitions, damage union, scroll clamp |
| `engine/page` | the shared pipeline — headful and headless share this core | Build | load / relayout / paint / links / text |
| `shell` | headful GUI: `render` (PNG) + `browse` (winit/wgpu), tabs, session, downloads | Build | the human front door |
| `agent` | headless agentic browser + `InferenceBackend` (Groq / local llama / BYO) | Build | the LLM front door — see below |
| `store` | encrypted password vault + origin-scoped autofill | Build | crypto core done; UX is Phase-0 polish |

### The JS-engine modification boundary

`engine/js` **configures and binds to** SpiderMonkey (`mozjs`, the Servo path — not V8). It never patches
SpiderMonkey's JIT (Warp/Ion) or GC internals, nor the sandbox — a deliberate boundary, because JIT
miscompilation is historically the largest source of exploitable browser RCE, and the reason SpiderMonkey is
trustworthy is years of adversarial fuzzing this project has no equivalent of. Where a vendored dependency's
*build flag* leaves us behind Firefox, the capability wins via a named, minimal, guarded delta — never by
forking an engine's algorithms and never by copying Blink/Gecko code.

### The agentic browser

The agent side is layered so the pieces are independently testable and swappable — the agent logic is
decoupled from both the harness driving it and the inference backend:

- **`AgentBrowser`** — headless page driver over `engine/page`. Knows nothing about LLMs: `navigate`,
  `scroll_by`, `screenshot_png`, and `observe` (the a11y tree — role + name + state + a click point per
  element, a far less injection-prone channel than raw text + a screenshot).
- **`InferenceBackend`** — the provider-agnostic, object-safe, multimodal model trait. Backends exist for a
  hosted OpenAI-compatible endpoint (posting *through* `engine/net` — no separate HTTP client, no OpenSSL), a
  keyless local `llama-server`, and a bundled small gguf; a user can point it at their own endpoint.
- **`run_task`** — the observe → decide → act loop, taking `&dyn InferenceBackend` + `&mut AgentBrowser` and
  naming neither a provider nor a harness. Actions are a small permission-gated JSON protocol
  (`navigate` / `click` / `scroll` / `finish`, plus tab control).

This is the seed of Phases 2–4: the same surface is the ingress a dev automation framework drives, the thing
the default LLM harness sits on, and the thing an in-browser consumer chat bar would drive.

---

## Roadmap

A finite, phase-ordered plan. The full version — with falsifiable good-enough bars and the self-executing
research→implement cascade — lives in [`docs/loop/HORIZON.md`](./docs/loop/HORIZON.md) and
[`docs/loop/PHASE0-BOUNDED-REMAINDER.md`](./docs/loop/PHASE0-BOUNDED-REMAINDER.md). The thesis: **a Rust,
from-scratch, memory-safe, agent-native browser** — one a human daily-drives *and* that exposes a unified
surface for agents to drive, with an optional in-browser "Claude Code for browsers" prompt-to-action layer.

0. **Daily-driver capability** *(in progress, near exit)* — render + JS-platform + media + forms + shell
   parity for the "document + download + un-gated-SPA" web. Exit = the fidelity certificate above. The
   bounded remainder is sized and has a named cut line; the marquee proof is **YouTube plays**.
1. **UI/UX browser features** — tab-set restore (toggleable), lean tab ops, mute/unmute, pin-to-stay-warm,
   and the hibernate-vs-keep-warm decision. (Also where **process-per-tab** isolation and DevTools land.)
2. **Agentic browser-automation API surface** — the stable, pinnable **ingress** any automation framework or
   async pipeline drives. Seeded already by the Phase-0 a11y tree, BiDi, and actuation.
3. **Default agent harness** — "Claude Code for browsers": the LLM tool-loop over that surface (context,
   multi-step steering, skill/tool exposure). Three deployment modes against one surface — consumer chat,
   dev/enterprise headless automation, and bring-your-own agent framework.
4. **Consumer prompt-to-action GUI** — an optional in-browser chat bar driving the browser via a bundled
   small gguf *or* a user-configured endpoint, reusing the Phase-2 surface.
5. **Performance.** · 6. **Security** (builds on the Phase-0 capability scoping + anti-injection fence).

Each phase after 0 opens with a deep-research sweep updated against the implemented layer beneath it, and
every step is held to the same verify-gated ratchet. Fine-tuning any model to the surface is an explicit,
owner-gated track *outside* this loop — the bring-your-own-endpoint path gives full capability with no tune.

## License

MPL-2.0 (see workspace manifest).
