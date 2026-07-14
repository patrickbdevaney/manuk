# The wasm demo — running the engine inside someone else's browser

The engine compiles to `wasm32-unknown-unknown` and executes on a canvas at
<https://patrickbdevaney.github.io/manuk/>. This is the only artifact that lets a stranger evaluate the
project **without cloning it**, so it is held to the same standard as the engine: a claim it makes must be
one it can be caught being wrong about.

*Companion topics: [performance](performance.md) · [build & dependencies](build-and-dependencies.md) ·
[conformance & oracles](conformance-and-oracles.md).*

---

## What crosses the wasm boundary, and what cannot

`demo/` is a **separate crate**, and its dependency list is the whole design:

| In | Why |
|---|---|
| `manuk-dom`, `-html`, `-css` (Stylo), `-layout` (Taffy), `-paint` (tiny-skia), `-text` | the real engine, unmodified |
| `wasm-bindgen` (**pinned `=0.2.100`**), `js-sys`, `web-sys` | the boundary |
| `console_error_panic_hook` | a Rust panic in wasm is otherwise a bare `RuntimeError: unreachable` |

| Out | Why |
|---|---|
| `manuk-page`, `manuk-net` | they pull **tokio/mio**, which do not target wasm |
| **SpiderMonkey** | it is C++, and (see below) it cannot be linked in even in principle |

The exclusion of `manuk-page` is *why the demo has no JavaScript* — not a shortcut, a consequence.

### SpiderMonkey and wasm: the honest answer

SpiderMonkey **can** run on wasm, but **only as an interpreter**, because WebAssembly forbids runtime code
generation — and runtime code generation is precisely what a JIT *is*. (This is why the Bytecode Alliance
built a portable baseline interpreter for it.) It ships as a **separate WASI module**, not something
linkable into a Rust `cdylib`. So the demo renders `<script>`-free snapshots and **says so on its own front
page**. Stating the limit is the only thing that makes the rest of the page believable.

---

## Three things that are true on wasm and nowhere else

These each cost a debugging session; none of them are obvious from the type system.

1. **`Instant::now()` panics.** There is no clock. A single debug-only `tracing::debug!` timing a cascade
   took down the entire pipeline, surfacing as `RuntimeError: unreachable` with no stack.
2. **There is no filesystem.** `load_system_fonts()` found nothing, so a *correct* 2,526px layout
   rasterized to a **blank page**. Fixed by `include_bytes!`-ing four Liberation faces into the binary.
3. **`usize` is 32-bit.** `NodeId` packed `generation << 32` into a `usize` and silently overflowed. The
   arena is `u64` now — see [DOM semantics](dom-semantics.md).

And the clock again, one layer up: **`js_sys::Date::now()` is coarse to 1ms.** Every pipeline stage rounded
to `0ms`, which made the provenance panel — whose whole job is to show the engine working — show nothing.
Use **`performance.now()`** (via `web-sys`), which is the host's high-resolution monotonic clock.

---

## The corpus: 41 sites, and why it is bought with Chromium references

Thirteen sites is an anecdote. The corpus is **41**, baked by `scripts/demo-pages.sh` and classified by the
same three-way split the roadmap uses:

* **doc web (17)** — Wikipedia, HN, Lobsters, MDN, GNU, Apache, Python docs, W3C, RFC-Editor, Ars, BBC,
  Guardian, Craigslist, xkcd, SQLite, Postgres docs, kernel.org
* **app web (11)** — the *server-rendered* DOM of Rust-lang, Next, Svelte, Vue, React, Astro, Remix, Solid,
  Vite, Nuxt, Angular (i.e. what a framework ships **before** hydration)
* **platform web (13)** — Tailwind, Bootstrap, GitHub, Stripe docs, MUI, Chakra, Bulma, Vercel, Netlify,
  Cloudflare, Figma blog, Linear, OpenAI

Every page ships a **Chromium reference render** beside it, because a corpus you cannot disbelieve is
decoration. They are **committed, not fetched at build time**: a fidelity comparison against a *moving*
target means nothing, and a deploy that depends on 41 live sites being up is a flaky deploy.

**Snapshots strip `<script>`.** The demo has no JS engine, so those bytes could never run — shipping them
is pure waste, and on a modern site the bundles dwarf the markup. **CSS is kept, all of it**: `github.html`
is 4.5M of which **4.2M is inlined CSS**, and that CSS is exactly what Stylo is here to chew on. It is the
substance, not the padding.

---

## Measuring a browser inside a browser: every obvious way is wrong

Verifying "did the wasm engine actually run?" from a headless Chromium is where this subsystem bit hardest.
**Three separate observers reported an engine that had run as an engine that had not** (PROCESS #36):

| Flag | What it does | Why it lies |
|---|---|---|
| `--virtual-time-budget` | virtualizes the clock | CPU work advances it by **zero** — so `performance.now()` deltas are `0ms` and *every* timing reads as absent |
| `--dump-dom` | dumps at `load` | `load` does **not** wait for an async wasm boot, so it dumps the pre-boot shell **every time** |
| `--screenshot` | waits, *sometimes* | caught the render on one run and missed it on the next, on identical code. **A flaky observer is worse than none** |

All three are one defect: **the instrument was blind to the thing it was reporting as absent.** The fix is
to stop *inferring* liveness from whatever side-effect happens to be observable, and instead **ask the page
directly over the DevTools Protocol**, polling until it is genuinely done.

That is `scripts/demo-verify.py` — the **`G_DEMO_LIVE`** gate, which `demo-build.sh` cannot pass without. It
asserts the boot placeholder is gone (the module executed), the nav has its class groups (`pages.json`
loaded), the canvas has **>2 distinct colours**, and the provenance panel reports **non-zero** time.

> **Count colours, do not ask "is anything non-white".** That was the check's first version and it was
> **vacuous**: an untouched canvas is transparent *black*, which satisfies "non-white" trivially. It
> reported PAINTED for a blank demo and passed a mutation that deleted the paint call outright. An empty
> canvas has exactly **one** colour; anti-aliased text alone has dozens. *The gate written to stop an
> instrument lying was an instrument lying, in the same tick, about the same demo* — which is why no gate
> here is believed for being green, only for having been **proven to go red**.

---

## Building it

```sh
./scripts/demo-pages.sh    # bake the corpus (network; Chromium references)
./scripts/demo-build.sh    # engine → wasm → demo/www, then G_DEMO_LIVE
```

`demo/www` is a complete static site. `.github/workflows/demo.yml` deploys it to Pages on push.
