# THE PROCESS MODEL — decided, scoped, and sequenced

**Status: DECIDED (2026-07-13). Sequenced AFTER the current breadth work.** This file is the whole
decision. It exists so that no future tick relitigates it, and so that no future tick *quietly attempts it
piecemeal* — it is a milestone, not a tick.

---

## 1. Process-per-tab. Decided.

**One OS process per tab, SpiderMonkey embedded in each.** This is the architecture Chrome itself shipped
**2008–2018**, before Site Isolation.

**Why there is no in-process alternative:**

- **In-process containment of a memory-corruption fault is not achievable.** Not a SpiderMonkey defect —
  **true of every production C++ JS engine.**
- **Chromium's own model assumes V8 has such bugs** and relies on **OS process boundaries**, not on V8's
  own safety. The most security-invested browser on earth does not trust its own JS engine in-process.
- Google's in-process alternative, the **V8 Sandbox**, took **3+ years of dedicated work by the team that
  wrote V8** — and is *still not a declared security boundary.* **In-process containment is the HARDER
  path, not a shortcut.**

**A from-scratch memory-safe JS engine is RULED OUT** as the escape hatch. It **would not even solve the
problem** — *JIT-generated code safety is a separate problem regardless of implementation language* — and it
**costs more than the rest of the browser combined**. It would also reverse the settled SpiderMonkey
decision. Not on the table.

**The real, scoped work** (this is the honest size of it):
1. **Process spawning** (per-platform — see §6).
2. **An IPC layer** to a coordinating process.
3. **A state-ownership redesign across that boundary.** ← *this is the hard one, and it is the reason this
   is a milestone and not a tick.* Today `Page` owns everything and the shell reaches into it.

---

## 2. Per-origin Site Isolation. REJECTED — not deferred.

Extending process-per-*tab* to process-per-*origin* is **architecturally straightforward** — the same
mechanism at a finer grain. **We are still not doing it.**

- **Chromium's own documentation names Site Isolation as the primary reason Chrome uses more memory than
  Firefox and Safari.**
- Chromium's security team has said they are **hitting the limits of what more process granularity buys** —
  because **processes are not cheap.**
- That is *Chromium*, which has far more budget for process overhead than we do, **naming this as a real
  cost.**

**Our north star is to be leaner than Chromium. Adopting Chromium's own named bloat driver works directly
against the goal.**

**Accepted trade-off, stated plainly and once:** a compromised cross-origin iframe shares its tab's
process. **This is the same trade-off Chrome itself accepted for a decade.** It is a *choice*, and it is
written here so it is never mistaken for an oversight.

---

## 3. Tiered hibernation — extracted from Chrome's real product behaviour

| Tier | Trigger | Action | Cost of resume |
|---|---|---|---|
| **Freeze** | short idle | Pause JS + timers. **Process stays resident.** | near-instant |
| **Discard / hibernate** | **memory pressure** or longer idle | Serialize a **restore token** (URL, scroll position, form state where feasible) and **terminate the process entirely.** | a real reload — **accepted** |

**Tune the idle threshold against measured usage. Do not copy a default.** Trigger discard on **measured
pressure**, not a blind timer.

### The non-negotiable exemptions

**Never hibernate, regardless of idle time:**
- audio/video **playback**
- an active **WebSocket / SSE** connection
- **anything the user would notice stopping**

> **A silent hibernation that drops something the user relies on is the same failure class the
> no-silent-failure discipline already forbids everywhere else in this project.** It is not a special case;
> it is the *same* rule.

---

## 4. The reframe that makes the memory claim structural

> **"Beat Chrome at 100 tabs" is about how many of those 100 tabs have a live process AT ALL — not about
> making a live tab individually smaller.**

Because the architecture is process-per-tab, **a correctly hibernated tab costs a few KB of restore-token
metadata — not a process.** With correct, default-aggressive hibernation, the honest target is:

**a 100-tab session's aggregate footprint looks like a handful of live tabs' worth of memory.**

That is a **structurally different number** from Chrome's, not merely a smaller one. It is also the *only*
version of the claim worth making — shaving a live tab by 15% is not a story.

---

## 5. Predictive pre-warming — a HYPOTHESIS, not a feature

Worth **prototyping** (Chrome keeps a warmed spare renderer — informal precedent). It has a **real cost
when the prediction is wrong.**

**Required before any go/no-go:** instrument **prediction accuracy** and the **net resource cost of false
positives** against the **latency win of true positives**. **No default-on without that measurement.**
This is the same discipline as everything else here: *measure before implementing.*

---

## 6. CROSS-PLATFORM IS A CONSTRAINT ON ALL OF THE ABOVE

**Linux, macOS and Windows are all north-star targets.** **Nothing in this file may be built Linux-only
without an explicit, working fallback for the other two.**

### Process spawning — a deliberate per-platform branch, not a gap

| Platform | Mechanism | Why |
|---|---|---|
| **Linux** | **warm-fork (zygote-style)** — a real optimisation | CoW from a pre-warmed template saves Chromium **~6–8 MB/process** |
| **macOS** | **plain spawn** (`std::process::Command`-equivalent) | `fork()` exists but is **hazardous in a multi-threaded process** — the Obj-C runtime and system framework state do not survive it cleanly. **Broadly discouraged.** |
| **Windows** | **plain spawn** | **No `fork()` at all.** `CreateProcess` — no parent-state sharing. |

**This is exactly what Chromium does** — it zygote-forks on Linux and `exec`s renderers fresh on Windows
and macOS. **The spawn path is the correct, unoptimised-but-working default — NOT a gap to close later.**

**Two constraints that must not leak across the branch:**

- **The fork-before-multithread-init sequencing constraint** (Tokio/Rayon must spin up **after** fork, not
  before) is **Linux-specific by construction** — it only applies to the fork path. **Do not let it shape
  the macOS/Windows spawn path's design.** Those paths never fork; the hazard does not exist there; each
  process just initialises normally from a cold start.
- **SpiderMonkey snapshot CoW-shareability must be VERIFIED per platform, not assumed to generalise.** The
  Chromium figures are **Linux-specific measurements of V8's snapshot behaviour under `fork()`.** Since
  macOS and Windows **never fork in this design, the CoW benefit simply does not apply there** — those
  platforms pay full per-process init. **That is expected and fine, not a bug to chase.**

### Memory-pressure detection — a real trait, not a Linux assumption ported

Build the hibernation trigger (§3) against **a small internal trait with three real implementations.** Do
**not** hardcode one platform's signal and assume porting is trivial.

| Platform | Signal |
|---|---|
| Linux | cgroups / **PSI** |
| macOS | dispatch-based **memory-pressure notifications** |
| Windows | memory-status + **low-memory resource notification** APIs |

### Verify **mozjs**, not just SpiderMonkey

SpiderMonkey itself ships cross-platform — it is Firefox's engine and Firefox runs everywhere. **But Rust
FFI binding-crate maturity can lag or vary by platform independently of the underlying C++ library's own
portability.** **Confirm `mozjs` actually builds and runs on macOS and Windows** before treating
cross-platform SpiderMonkey support as *settled* rather than *assumed*.

---

## 7. The standing 100-tab RSS benchmark — prove the claim, don't assert it

**Extend the differential-oracle infrastructure** to:

1. Open **N tabs (20 → 100)** in **both Manuk and real Chromium**.
2. Drive to a realistic **idle / backgrounded steady state**.
3. Measure **aggregate RSS across all processes** for each.
4. Track as a **standing metric** alongside the existing performance floors.

**Record per-platform status EXPLICITLY.** **Linux-validated is not the same claim as
cross-platform-validated**, and STATUS.md must reflect the difference **honestly** rather than let a Linux
measurement stand in for all three targets. Until the benchmark runs on all three, the gap is **tracked as
a known gap**, labelled as such — *development stays Linux-first, but the claim does not.*

---

## Why this file exists

Every item above is a decision that a future tick would otherwise **re-derive from scratch, wrongly, under
compaction** — and two of them (§2 and the ruled-out JS engine) are decisions where the *obvious* engineering
instinct is the wrong answer. **The mechanisms are the memory.**
