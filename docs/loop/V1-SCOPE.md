# v1 SCOPE — the finish line, and the test every tick must pass

**Authoritative definition: CONSTITUTION.MD PART VII.** This is the operational one-pager. v1 is **exactly
these four components, and nothing else.** Rank and select every tick against them.

## THE TEST (apply before starting any tick)
> Does this move one of the four components below toward **shippable**, or does it serve something
> **deferred**? If deferred — **it does not happen now.**

## THE FOUR COMPONENTS (the only things in scope)

| # | Component | The bar (what "done" means) | Explicitly NOT in scope |
|---|---|---|---|
| 1 | **Daily-driver rendering parity** | Reliably renders + runs the **representative real internet** (oracle corpus), GUI + headless. Rank by **real sites moved per fix** (breadth) + **depth on top sites** (x.com, Instagram, Facebook, HuggingFace, …). | A WPT %. **83%+ WPT is OUT** — WPT is a diagnostic/horizon, never a gate. |
| 2 | **Agentic automation surface** (the differentiator — most polish) | Agent fully drives the web natively: multimodal page-see; DOM/a11y/semantic tree as **first-class synchronously-queryable** state; forms/buttons/links/JS-UI interaction; tab nav+management. **"Complete" = drives the same top-N sites a human daily-drives**, measured vs the same corpus. | Anything measured separately from the real-site corpus. |
| 3 | **Reasonable security** ("good enough", NOT maximal) | Bar 0 (no crash/hang/silent-fail), **process-per-tab** containment, sound memory-unsafe-FFI handling. | Per-origin isolation, SM heap-sandbox, extension security model, best-in-class-anything. **Not a research program.** |
| 4 | **Reasonable performance** | Lean, fast, no pathological hangs / runaway resource use on the corpus. **"Feels good to use."** | Benchmark-winning targets. |

## DEFERRED — out of v1 (do NOT build toward these)
prompt-to-action layer · crypto-wallet / x402 · V8-backed speciation · kiosk / embedded / enterprise variants
· per-origin isolation · deep conformance (83%+ WPT).

**Modular seams to enable later addition = OK (architecture insurance).**
**Upfront cost / abstraction / a v1 design decision justified *only* by a deferred goal = OUT.**

## How this changes tick ranking (adds to VI.3 / the standing loop rule)
- Rank capability work by **usage-weighted real-site impact**, verified against the **oracle corpus** — not
  by raw WPT subtest count. (Tick 111's +18,245 counts because attribute reflection unblocks real sites, not
  because the number is big.)
- **Agentic-surface ticks are co-equal with rendering ticks** — the differentiator earns the most polish.
  Build the a11y/driving surface out against the top-N sites; measure "can the agent drive x.com / HF" as a
  first-class gate alongside "does x.com render."
- Security/perf ticks land only to the "good enough / feels good" bar — do not gold-plate.
- Before any tick, run THE TEST above. If the work only serves a deferred item, pick different work.
