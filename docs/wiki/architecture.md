# ARCHITECTURE — concurrency, process model, memory, and the REASONING

> The decisions themselves live in `docs/loop/PROCESS-MODEL.md` and STATUS.md's Settled Decisions.
> **This file records the mechanisms behind them** — the things a cold reader needs in order to
> understand *why*, and to avoid re-deriving them wrongly.

## In-process containment of a memory-corruption fault is not achievable

Not a SpiderMonkey defect — **true of every production C++ JS engine.**

- **Chromium's own model assumes V8 has such bugs** and relies on **OS process boundaries**, not on
  V8's own safety. The most security-invested browser on earth does not trust its own JS engine
  in-process.
- Google's in-process alternative, the **V8 Sandbox**, took **3+ years of dedicated work by the team
  that wrote V8** — and is **still not a declared security boundary.**

> **In-process containment is the HARDER path, not the shortcut.** A from-scratch memory-safe JS engine
> does not escape it either: **JIT-generated code safety is a separate problem regardless of
> implementation language.**

**A Rust panic can be caught at the FFI boundary; a segfault raised inside SpiderMonkey's own C++
frames cannot** — unwinding across that edge is UB. So Bar 0's containment handles *"a panic kills the
page"* and **cannot** handle *"a segfault kills the page."* That gap is closed by a process, or not at
all.

## More process granularity is not free, and Chromium says so itself

**Chromium's own documentation names Site Isolation as the primary reason Chrome uses more memory than
Firefox and Safari**, and its security team has said they are hitting the limits of what more
granularity buys — **because processes are not cheap.** That is Chromium, with vastly more budget for
process overhead than we have, naming this as a real cost.

**So per-origin Site Isolation is REJECTED, not deferred.** Adopting Chromium's own named bloat driver
works directly against a north star of being *leaner* than Chromium. Accepted trade-off, stated once: a
compromised cross-origin iframe shares its tab's process — **the same trade-off Chrome itself accepted
for a decade.**

## The memory claim is STRUCTURAL, not incremental

> **"Beat Chrome at 100 tabs" is about how many of those 100 tabs have a live process AT ALL** — not
> about making a live tab individually smaller.

With process-per-tab, a **correctly hibernated tab costs a few KB of restore-token metadata, not a
process.** Shaving a live tab by 15% is not a story; a 100-tab session that looks like a handful of
live tabs is a *different number*, not a smaller one.

**Hibernation is tiered** (freeze: pause JS/timers, process resident, instant resume · discard:
serialize a restore token and terminate the process, on **measured pressure**, not a blind timer), with
**non-negotiable exemptions**: audio/video playback, live WebSocket/SSE, anything the user would notice
stopping.

> A silent hibernation that drops something the user relies on is **the same failure class the
> no-silent-failure discipline already forbids everywhere else.** It is not a special case.

## Cross-platform is a CONSTRAINT, and the per-platform branch is deliberate

**Warm-fork (zygote) is Linux-only BY DESIGN — exactly what Chromium does.** Windows has **no `fork()`
at all**; macOS's exists but is **hazardous in a multi-threaded process** (the Obj-C runtime and system
framework state do not survive it cleanly). **Plain spawn on both is the correct,
unoptimised-but-working default — NOT a gap to close later.**

Two constraints that must not leak across the branch:

- The **fork-before-thread-init** sequencing constraint (Tokio/Rayon must start *after* fork) is
  **Linux-specific by construction** — it only exists on the fork path. It must not shape the
  spawn path's design.
- **SpiderMonkey snapshot CoW-shareability** is a *Linux-under-`fork()`* measurement. Where we never
  fork, the benefit **simply does not apply** — those platforms pay full per-process init, which is
  **expected and fine, not a bug to chase.**

**Memory-pressure detection gets a real trait with three implementations** (Linux PSI/cgroups · macOS
dispatch notifications · Windows low-memory resource notification) — *not* one platform's signal
hardcoded and "ported later."

**And verify `mozjs`, not just SpiderMonkey.** SpiderMonkey ships everywhere (it is Firefox's engine).
**Rust FFI binding-crate maturity is a separate question from the C++ library's portability.**

## SpiderMonkey has no V8-style deployable startup snapshot

It uses **compartments** for per-tab isolation and **realms** for globals, loading self-hosted code once
from a single compressed stream at init. Any plan that says *"share startup snapshots across isolates"*
is describing V8, not SpiderMonkey.

---
# Backfill — mechanisms recovered from ticks 1–42 (pre-wiki)

## The HOST-QUEUE + RE-ENTER pattern is the universal shape for every async JS surface

`window.open`, `history.pushState`, `postMessage`, `fetch`/XHR and downloads **all use the same structure**:

> **The JS native does no I/O — it enqueues a record on a thread-local queue and returns. The host drains
> the queue, performs the operation, and RE-ENTERS JS to deliver the result.**

This keeps `manuk-js` **decoupled from the network stack** (production injects a fetcher closure; tests
inject a mock) and makes each surface **testable without a socket**.

⚠ **NEVER add a parallel queue. That mistake cost a rework in tick 2**, and the instruction was carried
forward as a standing rule.

**The exception that proves the rule:** `MutationObserver` needs **no host round-trip** — it is
same-document and synchronous, delivered via **`queueMicrotask`**, which is exactly the spec's batching.
Any same-document JS surface **must deliver via a microtask** so it runs *before* the enclosing
dispatch/load/fetch call returns, since all of those drain microtasks. *Getting this wrong means records
queued during a click handler are delivered after the caller already observed the DOM.*

## A single `Rc` held across an `.await` is what pinned page construction to the UI thread

`Page` borrows `FontContext` (which holds `Rc<fontdue::Font>`, hence `!Send`), so page *construction*
genuinely cannot move off the UI thread. **But the FETCHING can** — and one detail was blocking that too:
**`fetch_images` held an `Rc<DecodedImage>` across an `.await`, which makes the entire future `!Send`.**

*That single `Rc` is why images, and with them the whole build, had to happen on the UI thread.* The fix is
an **owned, genuinely-`Send` fetch returning plain data** (`Prefetched { dom, final_url, css, images }`),
applying the `Rc` wrapper afterwards. **UI-thread `block_on` calls: 4 → 1**, and the user-reported "refresh
lag" disappeared.

## Snapshots handed into the JS world must be BORROWED, not cloned — and ASK whether the page is listening

Publishing `LAYOUT_RECTS` and `STYLES` as **owned copies** meant an **18,630-entry rect map and 18,630
`ComputedStyle` structs** (each with a heap-allocated font list and boxed pseudo-element styles) were cloned
on **EVERY JS re-entry** — **4.5ms per wheel event**, dozens of events per frame, on the UI thread, **to
tell a page something it usually is not even listening for.**

**Three compounding fixes:** **borrow** for the duration of the call · **ask first** (`wants_view_events` —
*most pages register no scroll listener and no observer*) · **coalesce to one notification per painted
FRAME**, not one per input event. Result: **scroll 4.5ms+ → 0.01ms**, click 1.32ms.

## Runtimes and pools are PER-PROCESS, not per-navigation

**Exactly one Tokio runtime and one Rayon pool per process.** Spawning them per action is *the same class of
bug as re-fetching a resource per subsystem, one layer down the stack* — and it is **invisible until
something counts them** (`G_RUNTIME_COUNT`).

**Tokio owns I/O; Rayon owns CPU — and nesting them without `spawn_blocking` can DEADLOCK BOTH POOLS.**
Tokio's workers assume tasks yield frequently at `.await`; Rayon's assume threads run CPU work to
completion. Calling `par_iter` from inside a Tokio worker risks starvation, and if a Tokio worker blocks on
Rayon work that is itself waiting on a Tokio-owned resource, **both stall.** Hand off via
`tokio::task::spawn_blocking`.

**Two-tier concurrency:** network fetch is I/O-bound (thousands of concurrent requests are feasible); JS/parse
is CPU-bound (ceiling near core count). **A single shared limit either throttles the network down to the JS
ceiling or thrashes the CPU up to the network ceiling** — use **two independent semaphores**
(net ≈ 16×cores, js ≈ cores).

## Three times in three ticks, the mechanism EXISTED and was wired to nothing that draws pixels

- **`Dom::flat_children`** — correct, tested, used by the HTML crate. Layout and the cascade walked
  `children()`. → *every web component on the web produced zero boxes.*
- **`NodeData::Comment`** — a real comment node type. `createComment()` returned a **text** node. → *lit-html
  found zero parts.*
- **`NodeData::Fragment`** — documented in our own source as *"a `<template>`'s contents"*.
  `createDocumentFragment()` returned a **`<div>`**.
- **`serialize_node`** (i.e. `outerHTML`) — had existed since the parser was written and was **unreachable
  from JS**.
- **A `<style>` inside a shadow root** — parsed and stored, while `collect_style_sources` walked the **light**
  tree.

> **That is ONE gate-shaped hole, not five bug-shaped ones.** No gate was asking whether a line existed
> between *"the code exists"* and *"the pixels change."*
>
> **A codebase that does the right thing NEXT TO the wrong thing is telling you the wrong thing is an
> oversight, not a design.**

## Isolation that FALLS OUT of the architecture beats isolation you enforce

An `<iframe>` is a **replaced element with a nested document**: fetch the child, build a whole `Page`, blit
its display list through the replaced-element path, translated and clipped to the iframe's rect.

**Because a `PageContext` is per-`Page`, a child's script has NO PATH to the parent's DOM — it cannot reach
it because it does not HAVE it.** *"It happens to be true" and "it is guaranteed" are different claims, and
only one survives a refactor* — so a hostile child
(`parent.document.body.setAttribute('data-pwned','yes')`) is **asserted against.** *A payment iframe the
host page can rewrite is not a payment iframe.*

The embed is **honestly limited** (it is a bitmap: it renders, it does not scroll and it does not update)
and the child document is fetched **after first paint**, because *an embed is the single most likely thing
on a page to be slow.*

## The Blitz LAYOUT CONTRACT: Taffy owns containers, the host owns leaf measurement

The prior design built a **fresh `TaffyTree` per flex container**, extracted slots, re-laid-out each child
as a block, and passed Taffy only fixed `Px`/`None` item sizes — so there was **no measure function at
all**, auto/intrinsic sizing of flex/grid items was simply **wrong**, and nested contexts re-solved in
**separate throwaway trees**.

**The contract:** Taffy drives flex/grid/block-container geometry; **every inline/replaced/float-containing
subtree is a MEASURED LEAF formatting context** the host sizes via `MeasureFunction`.

⚠ **The one delicate rule, stated so it is not rediscovered: Taffy has NO float/BFC model, so any container
establishing a BFC with floats must be a HOST-OWNED LEAF, not a Taffy container.**

Implementing `CacheTree` also buys Taffy's per-node **9-slot cache** (1 final layout + 8 measure slots) for
free. **Taffy also supports incremental relayout natively** — a per-node cache plus `mark_dirty()`
invalidating a node and its ancestors.

## The incremental-layout machinery is THREE data structures, not one dirty bit

1. **Double dirty-bit** — a node `IS_DIRTY` **plus an ancestor summary bit `HAS_DIRTY_DESCENDANTS`**, so a
   traversal skips any subtree whose summary bit is clear.
2. **RestyleDamage** — diff old vs new computed style into levels
   (`None < Repaint < Reflow < Rebuild`: `display` → Rebuild, geometry → Reflow, paint-only → Repaint).
3. **Invalidation, to skip MATCHING entirely** — a `RuleFeatureSet` built at stylesheet-parse time maps each
   key (class/id/attribute) to descendant vs sibling `InvalidationSet`s, **with a whole-subtree fallback for
   complex selectors** — plus a matched-properties cache and sibling style sharing, honouring the
   **sharing-breakers** (id, inline style, container units, registered event handlers).

**Salsa** (the framework rust-analyzer is built on) is the explicit adopt-vs-build alternative: it produces
the same invalidation *outcome* generically and **removes an entire class of stale/over-broad dirty-bit
bugs.**

## Deterministic CPU raster is what makes an agent's screenshots REPRODUCIBLE

**GPU rendering and layout are documented sources of replay non-determinism** (WebKit's "Web Replay"; GPU
kernel autotuning), so a GPU-backed agent **diverges on replay across machines and drivers.** A CPU raster
(tiny-skia) gives **stable screenshot digests** — which is what makes exact replay achievable.

**Record only what is genuinely non-deterministic** — the model's raw replies (same prompt, different
sampling) and the network — **and replay everything else.** *A green strict replay IS the reproducibility
proof, and the same machinery doubles as a regression harness.*

**Measured in-process advantage over a socket protocol:** **24.6 µs/command in-process vs 294.9 µs over
CDP-style JSON+TCP (~12×)**. ⚠ *An early run without `TCP_NODELAY` read a misleading **40ms**/command — a
**Nagle/delayed-ACK stall**, not real transport cost.*

## Extensions-as-a-runtime is a scope trap; the valuable subset is NATIVE

The full WebExtensions surface is hundreds of APIs **plus content scripts (arbitrary JS injection)** — a
major attack surface. **But the #1 high-value case, ad/tracker blocking, has a declarative, code-free
path:** Manifest V3's `declarativeNetRequest` already replaced runtime `webRequest` interception, and
Brave's **`adblock`** crate is mature, consumes standard EasyList/uBO lists, does tokenized matching, and is
**designed to run inside a native browser core.**

> **Content blocking ships natively at the request layer with NO extension runtime at all.**

## Chromium splits tab dormancy into two mechanisms, and only ONE reclaims RAM

- **Freeze** (Energy Saver): after hidden+silent ~5 min, stop JS/CPU and throttle DOM timers to ~1/min
  ("intensive wake-up throttling"), dispatch `freeze`/`resume` — **but it KEEPS FULL RAM.** Audio/WebSocket/
  RTC tabs exempt.
- **Discard** (Memory Saver): unload the tab entirely — resident drops from **~80–300 MB active to ~5–10 MB**
  of metadata. **That is where RAM is actually reclaimed.**

## Retained-heap accounting is NOT RSS, and saying so is the whole point of the metric

A per-tab `estimated_bytes` is a **retained-heap proxy** (what a discard would reclaim); process RSS
(`/proc/self/status` `VmRSS`) is the OS's real figure. **They do not sum to one another**, and RSS reclaim
additionally depends on **the allocator returning freed pages to the OS.**

Because tabs share one process in the isolate model, **true per-tab RSS is not directly separable** — and
per-tab *JS heap* would require SpiderMonkey's per-compartment memory reporters (Firefox's `about:memory`
does exactly this, **and still lands 30–45% in "heap-unclassified"**).

> **Printing a fabricated per-tab number is the dishonesty the feature exists to prevent. Report "not
> reported."**

## Two design decisions recorded as DECIDED-BUT-UNDONE

- **The DOM bindings are string-`eval` bindings.** Large parts build JS source strings and `eval` them (the
  identity cache, the listener registry, event dispatch, `getComputedStyle`, `getBoundingClientRect`, the
  promise job enqueue). Three consequences were named: **slow** (parse+compile per op), **fragile to any page
  that shadows a `__`-prefixed global or `Array.prototype.push`**, and **a latent injection surface** — and
  it **blocks a real lifetime story, because you cannot trace GC edges that live inside eval-string state.**
  The decision was *"kill the string-eval bindings, use direct JSAPI."* **Still undone.**
- **Methods are defined PER-INSTANCE** (N nodes × M methods) rather than hung off **one shared prototype per
  interface** — *which is what breaks the `instanceof`/`constructor` semantics pages test for.*
