# Manuk

[![CI](https://github.com/patrickbdevaney/manuk/actions/workflows/ci.yml/badge.svg)](https://github.com/patrickbdevaney/manuk/actions/workflows/ci.yml)
[![demo](https://github.com/patrickbdevaney/manuk/actions/workflows/demo.yml/badge.svg)](https://github.com/patrickbdevaney/manuk/actions/workflows/demo.yml)

## The browser that renders the whole web *into the surface an agent actually wants*

Every AI agent driving the web today is bolted onto a browser built for human eyes. It
either **screenshots** the page (general, but ~$0.40 and 2–3 seconds per step, and the
model still has to *find* the button in pixels) or it **scrapes the DOM / guesses CSS
selectors** (brittle, and a third of the tokens are markup noise). The emerging clean
answer — **WebMCP** (`navigator.modelContext`), a semantic tool surface with no scraping
and no selectors — only exists on the handful of sites that adopt it. The cross-browser
automation standard — **WebDriver BiDi** — can move the mouse but exposes **no
accessibility surface at all**. So on the real, legacy web an agent is still squinting at
screenshots or clinging to selectors that break on the next deploy.

**Manuk's unclaimed opening: a browser whose *native* output is the agent's ideal surface —
for every site, with no cooperation from the site.** Because the engine already parses the
DOM and solves the layout, it computes — with **no JavaScript** — a clean semantic view of
any page: every element's **role + accessible name + interaction state + a click box + a
stable address**. That is a WebMCP-quality surface synthesized for the *entire* web, paired
with standard BiDi actuation that fires the *real* DOM events. An agent perceives the page
as structured meaning (cheap, fast, robust), acts by semantic reference (not a screenshot,
not a `div.x7f3`), and verifies the result — on any site, legacy included.

Manuk is that browser, built from scratch in Rust — **and** a genuine, human-usable visual
browser (Stylo cascade, real JavaScript, `<video>` playback, forms, tabs), not a scraping
shell. One engine core, three front-ends: a headful GUI (`shell`), a headless agentic
browser (`agent`), and a WebAssembly demo. Chromium parity across the whole web platform is
the work of a large team; what follows is an honest, *measured* account of how far this has
actually got — not what it aspires to.

### ▶ **[Run the engine in your own browser →](https://patrickbdevaney.github.io/manuk/)**

Not a screenshot, not a video. That link compiles this engine to WebAssembly and executes
it **in your browser** — **Stylo**'s cascade, **Taffy**'s flex/grid, **tiny-skia**'s
rasterizer, and this engine's own DOM / layout / paint / accessibility tree, rendering real
page snapshots onto a canvas. Three views:

- **Render** — the pixels. Scroll re-renders (it does not pan a bitmap); hover is a real
  hit-test against the laid-out boxes.
- **Agent view** — **the thesis made visible**: the accessibility tree an LLM drives through
  — role + accessible name + interaction state + a click box, computed with no JavaScript
  from the parsed DOM and the solved layout. Hover a node to ground it on the page. This is
  the channel the headless agent gets *instead of* a screenshot.
- **Boxes** — every rectangle the layout engine solved, coloured by nesting depth.

And **compare with Chromium** puts our render beside Chromium's of the same document, so none
of it is taken on trust. *(The wasm demo has no JavaScript and no live fetching — SpiderMonkey
is C++ and can't be linked into a wasm binary, and the demo says so on its own front page. The
Agent view needs no JS: an a11y tree is a pure function of the DOM and the geometry.)*

---

A browser engine built from scratch in Rust — **~118k lines of our own source across 18
crates** (vendored Stylo and SpiderMonkey are not counted) — with one shared engine core
behind all three front-ends. The goal is a **daily driver**: fast, lean, and correct enough
on the *breadth* of the real web to actually use — and, uniquely, **agent-native**: the same
core exposes a structured surface an automation framework or an LLM operates directly.

---

## Where this actually is

Two public checkpoints have shipped — **v1.0.0** (2026-08-21) and **v1.1.0** (the first
cross-axis capability measurement) — and **Phase 0 (daily-driver capability) is in
progress**. The engine is broad in *coverage* of the real web; *placement fidelity*,
*function certification*, and the *accessibility bar* are the open gaps to the exit
certificate below.

**The first principle is a ratchet.** Every unit of work must leave the engine strictly more
capable than it found it, and nothing that worked before may work less after — across
**capability**, **performance**, and **instrument fidelity**. A capability is *banked* only
once a gate proves it can go red; that proven gate is the ratchet tooth. Progress turns one
way. Everything below is machine-generated into [`STATUS.md`](./STATUS.md) from the
filesystem, git, and the verify receipt — never hand-asserted.

Three bars, never conflated:

| | | |
|---|---|---|
| **Bar 0** | *Does the engine ever take the browser down?* | The floor — checked before correctness is even asked. |
| **Bar 1** | *Is the page legible, navigable, functional?* | The near-term target. |
| **Bar 2** | *Is it pixel-exact?* | **Deliberately deferred** — breadth beats depth until Bar 1 is real. |

The **north star**: *Chromium is the **ceiling** on capability and the **floor** on
everything else.* Match what a page can do in Chrome (scripts run, layout resolves, forms
submit, media plays); **beat** Chrome on speed, stability, and resource use. A structural
divergence is a bug; a timing divergence in our favour is the point.

### The Phase-0 exit is a certificate, not a percentage

**Phase 0 is done when, on a statistically-representative sample of the real web, ≥95% of
in-scope sites both RENDER acceptably *and* FUNCTION on the capabilities they actually use**
— every term adversarially falsified, every denominator reconciled, only named exceptions
failing. It decomposes into three measured legs:

- **M1 — RENDER.** A site passes when its **parent-relative shape ≥ 0.75 on ≥95% of nodes**
  (they land where Chromium puts them, within tolerance) **and** it is **jarring-clean** (no
  overlap, no horizontal overflow, reading order preserved, no dead click-targets). Bar:
  **≥95%** of in-scope sites.
- **M2 — FUNCTION.** The capabilities a site actually uses **exercise green for that site**,
  driven over **WebDriver BiDi** and A/B-diffed against Chromium. Bar: **≥95%** render∧function.
- **a11y.** The agent's perception surface: **role + name + state match Chrome on ≥90% of
  nodes.**

**The honest current picture, and the binding constraint.** Coverage is the strength — the
engine draws the box Chromium draws on **~90%** of nodes, is **faster than Chromium on ~92%**
of the corpus (median ~2×), and **crashes zero times** (a panic kills the page, not the
browser). The open gaps: **placement fidelity** (mean parent-relative shape ≈ **0.69** — right
boxes, not yet all in the right place) and, decisively, a **scorability ceiling**: roughly a
**fifth of in-scope sites don't yet reach a stable, scored DOM** (an app-halting script error,
a missing boot-critical API), and an unscorable site fails RENDER by construction. That ceiling
— not layout polish — is what currently caps the exit number, and lifting it (fixing the shared
function/JS defects) is the main line of work; it raises M1 *and* unblocks the M2 function
certificate at once. Live figures are in [`STATUS.md`](./STATUS.md); the plan is in
[`docs/loop/PHASE0-COMPLETION-RESEARCH-AND-PLAN.md`](./docs/loop/PHASE0-COMPLETION-RESEARCH-AND-PLAN.md).

Development runs on a **differential oracle**: **265 real sites across 15 design-pattern
classes** rendered by *both* Chromium and this engine from one snapshot, diffed by structural
path, clustered by root cause. The cluster ranking — by *distinct sites explained*, not hit
count — **is** the priority ledger. No feature is picked by taste.

---

## What works

**Rendering.** Real sites over HTTPS: block / inline / flex / grid / table / float /
positioning, the box model, stacking contexts and `z-index`, `overflow` clipping,
`border-radius`, gradients, backgrounds, shadows, transforms. The **live cascade is Stylo**
(Firefox's) — `@media` / `@supports` / `@layer`, custom properties, container queries,
`:has()`, CSS Level-4 math. Stylo and SpiderMonkey are embedded as sanctioned FFI
dependencies — never patched internally.

**JavaScript (native).** SpiderMonkey with real DOM/BOM/CSSOM: event dispatch with
capture/bubble, `fetch` / `XHR`, timers, promises + microtasks (spec ordering), custom
elements + shadow DOM, `MutationObserver` / `IntersectionObserver` / `ResizeObserver`,
**ES modules** (import maps + a real import graph, so CDN no-bundler apps boot),
`<canvas>` 2D, the **Web Animations API**, **View Transitions**, the **Navigation API**,
**IndexedDB**, the **Cache API**, **Web / Service Workers** (same-thread), **CSP**, `Blob`,
the **Sanitizer API**, popover, `scheduler.postTask`, `crypto.subtle` on a real CSPRNG.
Eight real SPA framework bundles (React, Vue, Svelte, Solid, Preact, Lit, Vanilla — Vite
production output) mount and render. **~496,000 Web Platform Test subtests pass**, tracked as
a monotonic ratchet (the build instrument, never the certificate).

**Media.** `<video>` **plays**: MP4 demux → **H.264** + **AAC** decode → frames composited
into the page → a real playback clock (`timeupdate` / `ended`, true seeks, tracks / captions).
MSE `SourceBuffer` is partial. EME / Widevine is **permanently out of scope** (a licensed
proprietary CDM), so Netflix / Spotify are unreachable — stated once, not relitigated.

**Interactivity & shell.** Click links, focus and type into fields, submit forms (POST
navigation, multipart `FormData` uploads, constraint validation), toggle controls, scroll,
tabs (open / close / switch, **hibernated** background tabs), history, bookmarks,
find-in-page, zoom, cookies (RFC 6265, public-suffix-aware, `SameSite` + `__Host-` /
`__Secure-`), partitioned storage, session restore, a password **vault + origin-scoped
autofill**, and streaming **download-to-disk** (multi-GB, no OOM). The request path carries
headers and bodies faithfully with CORS enforcement.

**Agent-native (the thesis).** The same engine core, headless: an **accessibility tree**
(roles, accessible names, interaction state, focus, geometry) as the agent's observation
channel — computed with no JS — plus in-process automation (selectors / wait / assert),
**WebDriver BiDi**, occlusion-aware hit-testing, and an `InferenceBackend` trait so any
provider (local `llama-server`, Ollama, or a hosted endpoint) drives it identically. Agent
actions fire the *real* DOM events (`input` / `keydown` / `focus` / `blur` / `change`), so
React- and Vue-controlled inputs actually update. Page text reaches a model only through an
untrusted-content fence.

## What doesn't (yet), stated plainly

A README that only lists wins is marketing.

- **The Phase-0 exit itself** — ≥95% render∧function — is not met: placement fidelity (~0.69
  mean shape) and the ~1-in-5 scorability ceiling are the live gaps; the M2 function
  certificate is not yet stood up (the BiDi `script.evaluate` leg is stubbed); the a11y tree
  is short of its ≥90% node-match bar and the AccessKit platform bridge is not yet adopted.
- **Rich editing** — `contenteditable` + `Selection` + editing commands (Gmail-compose /
  Notion class) and **IME / composition** (CJK typing) — remaining Tier-1 work.
- **WebAuthn / passkeys** — passkey-only sites are hard walls (TOTP fallback covers the rest).
- **Pixel precision (Bar 2)** is deferred, not achieved.
- **No JavaScript in the wasm demo** — SpiderMonkey is C++ and doesn't target wasm.
- **SpiderMonkey can fault inside its own C++ frames**, uncatchable in-process — an open Bar-0
  residual whose real fix is one OS process per tab (a decided architecture, sequenced into
  Phase-1 security work).
- **Out of scope by decision** (feature-detected cleanly, not half-built): **EME/DRM**,
  **WebRTC**, **WebGPU + heavy-WebGL creative apps** (Figma / Canva tier), a niche modern-CSS
  tail, and HTTP/3 / QUIC. Each is a named exception with a reason, not a silent gap.

---

## How it is developed

> **Three instruments, seeing different things.** The **differential oracle** (265 real sites
> vs Chromium) finds what real pages do. **Web Platform Tests** finds what the *spec* says,
> needs no oracle, and surfaces adversarial cases no real site generates. The **fidelity
> instrument** (parent-relative shape scoring + the four jarring invariants) is the Phase-0
> exit gate. Findings are captured by topic in **`docs/wiki/`**.

**Select by the binding constraint, one capability per tick.** The engine is built by an
autonomous loop that lands exactly one verify-gated capability per commit. Work is chosen by
the term *arithmetically capping the exit certificate* — currently the scorability/function
ceiling — attacked as a **whole subsystem or shared root cause**, never a per-assertion
decimal (the method that took Servo and Ladybird to conformance, and that our own data showed
is the only path off the mid-40s asymptote). A subsystem-sized lever is decomposed into
independently landable bricks. The ratchet is absolute: a Bar-0 crash or *any* measured
regression is reverted, never traded for a feature.

**The gates** run as one wall (`scripts/verify.sh`, ~8 min) and are all-or-nothing — build,
`parity` box-geometry probes within ±3px of headless Chrome, real-site fidelity, JS
conformance, clickability, plus ~570 purpose-built `G_*` gates each born from a user-visible
failure every existing gate slept through (`G_CONTAIN` Bar-0 containment, `G_HANG` counted
timeouts, `G_ALLOC` per-event allocation, `G_CLEAN_EXIT`, cascade ≤40ms / pipeline ≤125ms
perf floors, …).

**Compliance is mechanical, not remembered.** The **gate receipt** records the git *tree*
verified and a pre-commit hook refuses a commit whose staged tree differs; the **journal is
enforced** (no commit without a tick entry, written before the work as a hypothesis); a
pattern-class fix must **name the oracle cluster** it closes; the self-audit is unavoidable
past 10 ticks overdue. Every one of these has refused its own author at least once — that is
the mechanism working.

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
bidi/          WebDriver BiDi surface — the standard, pinnable automation ingress (M2 is measured over it)
store/         local encrypted password store + origin-scoped autofill
demo/          the engine compiled to wasm — the in-browser demo (this repo's GitHub Pages site)
tests/wpt/     Web Platform Tests harness + results tracking
docs/          wiki (findings by topic), the loop's methodology + status, sample pages
```

## The stack, layer by layer

**Reuse** = a mature upstream crate wired in; **Build** = written from scratch, verified
against WPT and the oracle.

| Crate | Role | Kind |
|---|---|---|
| `engine/net` | `fetch` + `request` over pooled hyper + rustls, HTTP/2, gzip/br, streaming | Reuse |
| `engine/html` | `parse(html) -> Dom` via html5ever; incremental `StreamParser` | Reuse |
| `engine/dom` | arena DOM tree, the mutable Web API surface — **no JS dependency** by design | Build |
| `engine/css` | `ComputedStyle` via **Stylo** (the live cascade) + a minimal fallback | Reuse (Stylo) |
| `engine/layout` | block / inline / float / table / positioning / stacking; flex + grid via Taffy | Build (+Taffy) |
| `engine/text` | font discovery, shaping, glyph raster (complex-script is the frontier) | Reuse |
| `engine/js` | `JsRuntime` trait; SpiderMonkey behind `--features spidermonkey` | Reuse (mozjs) |
| `engine/media` | container demux + H.264 / AAC decode → frames | Build (+symphonia/openh264) |
| `engine/a11y` | role + accessible-name + state tree — screen-reader source **and** the agent channel | Build |
| `engine/paint` | display list → CPU raster (tiny-skia) → PNG / RGBA | Build |
| `engine/compositor` | per-tab tiers (focused-GPU / background-CPU / hibernated), damage, scroll | Build |
| `engine/page` | the shared pipeline — headful and headless share this core | Build |
| `shell` | headful GUI: `render` + `browse` (winit/wgpu), tabs, session, downloads | Build |
| `agent` | headless agentic browser + `InferenceBackend` (hosted / local llama / BYO) | Build |
| `bidi` | WebDriver BiDi — the standard remote-control protocol; M2 is measured over it | Build |
| `store` | encrypted password vault + origin-scoped autofill | Build |

### The JS-engine modification boundary

`engine/js` **configures and binds to** SpiderMonkey (`mozjs`, the Servo path — not V8). It
never patches SpiderMonkey's JIT or GC internals, nor the sandbox — a deliberate boundary,
because JIT miscompilation is historically the largest source of exploitable browser RCE.
Where a vendored dependency's *build flag* leaves us behind Firefox, the capability wins via a
named, minimal, guarded delta — never by forking an engine's algorithms, never by copying
Blink/Gecko code.

### The agentic browser — the ingress for Phases 2–4

Layered so the pieces are independently testable and swappable:

- **`AgentBrowser`** — headless page driver over `engine/page`. Knows nothing about LLMs:
  `navigate`, `scroll_by`, `screenshot_png`, and `observe` (the a11y tree — role + name +
  state + a click point per element, a far less injection-prone channel than raw text + a
  screenshot).
- **`InferenceBackend`** — the provider-agnostic, object-safe, multimodal model trait.
  Backends exist for a hosted OpenAI-compatible endpoint (posting *through* `engine/net`), a
  keyless local `llama-server`, and a bundled small gguf; point it at your own endpoint.
- **`run_task`** — the observe → decide → act loop, naming neither provider nor harness.
  Actions are a small permission-gated JSON protocol (`navigate` / `click` / `scroll` /
  `finish`, plus tab control).

This is the seed of the unclaimed opening: the same surface is the ingress a dev automation
framework drives, the thing the default LLM harness sits on, and the thing an in-browser
consumer chat bar would drive.

---

## Roadmap

A finite, phase-ordered plan (full version:
[`docs/loop/HORIZON.md`](./docs/loop/HORIZON.md)). Thesis: **a Rust, from-scratch,
memory-safe, agent-native browser** — one a human daily-drives *and* that exposes a unified
surface for agents to drive the whole web, with an optional in-browser "Claude Code for
browsers" prompt-to-action layer.

0. **Daily-driver capability** *(in progress)* — render + JS-platform + media + forms + shell
   parity for the "document + download + un-gated-SPA" web, plus the a11y perception surface.
   **Exit = the ≥95% render∧function certificate above** (a11y ≥90% node-match); the binding
   constraint today is the scorability ceiling. The marquee proof is **YouTube plays**.
1. **UI/UX browser features** — tab-set restore, lean tab ops, mute, pin-to-stay-warm, the
   hibernate-vs-keep-warm decision (also where **process-per-tab** isolation and DevTools land).
2. **Agentic automation API surface** — the stable, pinnable **ingress** any framework or
   pipeline drives; seeded already by the Phase-0 a11y tree, BiDi, and actuation.
3. **Default agent harness** — "Claude Code for browsers": the LLM tool-loop over that surface
   (context, multi-step steering, skill/tool exposure), in three deployment modes.
4. **Consumer prompt-to-action GUI** — an optional in-browser chat bar driving the browser via
   a bundled small gguf *or* a user-configured endpoint, reusing the Phase-2 surface.
5. **Performance.** · 6. **Security** (builds on the Phase-0 capability scoping + anti-injection
   fence).

Each phase after 0 opens with a deep-research sweep updated against the layer beneath it, held
to the same verify-gated ratchet. Fine-tuning a model to the surface is an explicit,
owner-gated track *outside* this loop — the bring-your-own-endpoint path gives full capability
with no tune.

## License

MPL-2.0 (see workspace manifest).
