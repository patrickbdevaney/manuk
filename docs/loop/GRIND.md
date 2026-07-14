# THE GRIND — the loop, made mechanical

This is not a plan. It is a **search policy that runs itself**, and it exists because every plan this
project has written has been wrong within ten ticks.

> **A rule I can recite while breaking it is a decoration.** Forty-seven process defects say the same
> thing. So nothing here is advice. Every line below is a script that runs, and a gate that refuses.

---

## Why this file exists: the loop could not see its own frame

Twice in one session this project made an order-of-magnitude leap, and **both times a human had to point
at it.** Not because the analysis was hard — because the loop was ranking items *inside* an aperture it
never questioned:

| the leap | what it was actually worth | why the loop missed it |
|---|---|---|
| "measure `html/dom`, not just `dom/`" | **+9,940 subtests in one tick** | `html/dom` (59,818 subtests) sat un-measured **in the same checkout** while ten ticks went into `dom/` (6,484) |
| "histogram the failure *messages*" | the top row was **+170 subtests in an hour** | `--show-failures` had existed for many ticks and had **never been run** |

Both are the same defect, and it has a name now:

> **The loop optimised the room it was standing in, and never opened the other doors.**
> An area you have not checked out **scores zero and is invisible**. A test you do not run **cannot
> fail**. *Absence of measurement is not evidence of coverage — it is the absence of evidence.*

So the policy begins one level up from "what should I fix?" It begins at **"what am I not looking at?"**

---

## The tick, mechanically

Every tick, in this order. None of it requires a human.

```sh
./scripts/orient.sh          # ← the whole thing. Runs 1–5 below and prints the verdict.
```

**1. BAR 0 — `scripts/orient.sh` §1.**
A hang or a crash outranks every score on the page. Tick 82 landed +9,940 subtests and *introduced one
crash*; the tick could not ship until the crash was gone. **A win beside a crash is not a win.**

**2. THE APERTURE — `scripts/blindspot.sh`. Rank apertures BEFORE mechanisms.**
Lists every WPT area upstream — including the ones never checked out, which the sparse clone can still
*see* in its index — against what we actually measure. If the largest **invisible** area is bigger than
the largest mechanism inside the visible ones, **the tick is to open the aperture**, not to write engine
code:

```sh
./scripts/wpt-expand.sh <area>   # make it real
./scripts/recon.sh               # re-map: "biggest" may now mean something else
```

*Opening an aperture is not overhead. It is the only move that can change what "biggest" means.*

**3. THE MAP — `scripts/recon.sh`.**
A cheap unbiased sample of **every** materialised area, ranked by **estimated failing subtests** — not by
percentage. *A 0%-passing area of 40 tests is a rounding error; a 21%-passing area of 60,000 is the whole
map.* Percentage answers "how are we doing here?"; failing-count answers "**where is the ground?**", and
only the second one chooses a tick.

The estimate uses the **median** subtests-per-file, not the mean — the first version put a degenerate area
at the top of the map because one generated file held a test per Unicode codepoint. *An estimator a single
file can hijack chooses wrong, confidently.* These are **scouting numbers**: they decide where to point
the expensive instrument, and they never enter the ratchet or the cadence ledger.

**4. THE WORK LIST — the failure-message histogram of the top area.**

```sh
cargo run -q -p manuk-wpt --release --features spidermonkey -- wpt <area> --show-failures 2>&1 \
  | grep -oP '^         \K.*' | sed -E 's/"[^"]*"/"X"/g; s/\b[0-9]+\b/N/g' \
  | sort | uniq -c | sort -rn | head -20
```

**Take the top row that is a MECHANISM, not an instance.** "Attribute reflection" is one tick;
"fix `input.disabled`" is four hundred. This single command is how tick 79 found `setAttributeNS` (+170)
and how tick 82 found reflection (+9,940).

**5. THE RATCHET — `scripts/ratchet.sh check`.** See below. It refuses the tick, it does not advise it.

Then: **probe first** (the most expensive thing in this loop is still guessing) → implement → gate (`G_*`)
→ **prove the gate goes red** (`falsify.sh`) → the wall → `scripts/tick.sh`, which banks the ratchet and
timestamps the tick. **Go to 1.**

---

## The ratchet — the part that makes it set-and-forget

`scripts/ratchet.sh` keeps a **high-water mark** for every invariant and **refuses the commit** if any of
them moves backwards. It only ever raises: `bank` takes `max(mark, current)`, so a regression cannot be
laundered into the baseline by re-running it.

| invariant | why it is a gate and not a hope |
|---|---|
| **WPT, per area** | Tick 82 landed +9,940 in `html/dom` and **silently lost 2 in `dom/`**. A tick that improves what you are looking at and breaks what you are not is indistinguishable from progress — *unless you measure both.* |
| **Crashes** | Zero. Not "no worse" — **zero**. Bar 0 outranks any score. |
| **Duplicate wire requests** | The same URL downloaded twice in one navigation is bandwidth, latency, and on a metered link, money. **A browser that double-downloads is not lean, however fast its layout is.** |
| **Capabilities asserted** | `G_CAPABILITY`'s claim count. An engine cannot become less capable. |
| **Gates live** | An engine cannot become **less measured**. |
| **Verify wall** | May grow 30% before it fails. The wall **taxes every future tick** — letting it drift is compounding interest charged against the loop itself. |

---

## The exhaustion rule — go broad before narrow

* While any mechanism anywhere is worth **≥500 subtests** → that is the tick, whatever area it lives in.
* After every such tick, **re-run the recon** — a fix in one area moves the others, and "biggest" changes.
* Only when **no** area holds a ≥500 mechanism does the loop go narrow: file by file, message by message,
  still ranked by count.

**A stub is worse than an absence.** Proven five times (`Range`, `TreeWalker`, `MutationObserver`, the
tokenlist half of reflection, and `createTreeWalker`'s plain-object shim). A library feature-detects a
stub, finds it, registers, and **silently never works**. If it cannot be done properly this tick,
**skip it and say so in the ledger** — never fake it.

---

## The no-handback rule

The loop does not stop and it does not ask. **A question to the user is a tick that did not happen.**
If a decision is genuinely ambiguous: take the reversible option, write down why, and keep going. The
journal is the place to be uncertain in — not the prompt.

The entire hand-off between ticks is: `STATUS.md`, the last journal entry, and `./scripts/orient.sh`.
That is by design. A tick that cannot be resumed from those three things is a tick that was never
properly recorded.

---

## What "done" means

**Near horizon — the daily driver.** Every ❌ in `WEB-PATTERNS.md` is either ✅ or carries a receipt saying
why not. `G_CAPABILITY` asserts every ✅. The 265-site oracle has zero Bar 0 residue.

**Far horizon — WPT.** Ladybird crossed **90% of all WPT subtests** in 2025 — the bar Apple uses for
alternative-engine eligibility on iOS, and therefore the only externally meaningful number. **That** is the
target. Not "most of `dom/`". Not a percentage of whatever happens to be checked out. **90% of all of it.**

Neither horizon is met. The loop continues.
