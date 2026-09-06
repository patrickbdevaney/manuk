# Nineteen of five hundred and sixty-seven

`g_constellation_unknowns` was **red on the clean tree for four consecutive ticks** while every wall
run in that window reported green. A gate and the wall disagreeing about the same tree is an
instrument failure whichever one is lying, so the fifth tick asked which.

Neither. **The wall never ran it.**

```text
  gate files in engine/page/tests/           567
  launched by name in verify.sh               19
  manuk-page in the crate-suite loop          NO
```

The loop is `for c in manuk-css manuk-layout manuk-paint manuk-dom manuk-net manuk-agent
manuk-shell` — `manuk-page` is not in it, and the one `manuk-page` line above it is a *build*
warm-up, not a test run. So 548 of the 567 gates in the project's largest gate directory are
executed by nothing on the per-tick path.

⚠ This is a **recurrence with a number**. t1403 already recorded *"a count of GATE FILES is not a
count of EXECUTED gates"* after finding two gates red for three ticks under a green `GATES 534`.
That was the same defect at a smaller scale, and it was not closed.

## What was actually red

Running all of them found **two**, and both are the same shape — the engine got *more correct* and
the gate held the old claim:

| gate | pinned | measured now | Chrome |
|---|---|---|---|
| `g_constellation_unknowns` | `img.sizes-auto=undefined` | `auto` | `auto` |
| `g_iface_surface_2` | `overclaimed:none` | `overclaimed:ToggleEvent` | `ToggleEvent` exists |

The first even carried its own answer in a trailing comment — `// Chrome: auto` — and its header
says exactly what to do: *"a red row here is not necessarily a regression — it may be a capability
that just ARRIVED. Re-measure the row against headless Chrome, update this claim, and update the
matching row in `CONSTELLATION.tsv` in the SAME tick."*

The second is subtler and is worth stating carefully. `g_iface_surface_2` keeps a list of
capabilities the engine does **not** have, so `'X' in window` must answer `false` and a page can
route around us — t772's half-installed-API trap. `ToggleEvent` was on that list, and it now works:

```text
  Chrome  typeof=function | ctor=ok old=closed new=open type=toggle | isEvent=true
          dispatched=ToggleEvent/old=closed/new=open
  Manuk   identical
```

**Leaving the name on the list asks the engine to lie in the other direction** — which is the same
defect the gate exists to catch, mirrored.

## After the repairs

```text
  cargo test -p manuk-page --features stylo,spidermonkey --no-fail-fast
    570 binaries ok, 595 tests, 0 failures
```

⚠ And `--no-fail-fast` is load-bearing for this measurement: without it cargo stops at the first
failing binary, so the first sweep ran **230 of 568** and reported a clean-looking tail that did not
exist. A green count from a fail-fast run is a count of the binaries before the first red.

⚠ One gate flaked under the full-suite load and passed alone (`g_forced_reflow_budget` — a timing
gate, starved by 230 concurrent binaries). Re-run a timing failure alone before believing it.

## The standing rule this leaves

`scripts/verify.sh` is harness and observer-owned; this is reported, not fixed. But the *practice* is
agent-side and costs nothing: **before landing a gate in `engine/page/tests/`, run the package
sweep** — the wall will not do it for you.

See also [[a-count-of-gate-files-is-not-a-count-of-executed-gates]].
