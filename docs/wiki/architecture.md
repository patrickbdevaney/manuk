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
