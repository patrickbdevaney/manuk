# THE GRIND — how to cover the most ground in the fewest ticks, forever, without asking

This is the standing operating procedure for the perpetual loop. It replaces intuition about *what to work
on next* with a mechanical rule, because intuition has now been wrong six times in a way the ledger caught
and twice in a way it did not.

**It is not a plan. It is a search policy.** Run it every tick, forever, until both horizons are met.

---

## The one law

> **Fix the MECHANISM with the largest failure count. Never the instance.**

Every capability tick answers one question and it is always the same question:
*which single mechanism, if implemented correctly, retires the most failing subtests?*

The answer is never guessed. It is **measured, in one command**, and the loop had it for many ticks before
it thought to run it (PROCESS #45).

---

## The tick, mechanically

```sh
# 1. MEASURE EVERY AREA — not just the one you worked on last.
for d in dom html/dom css/selectors css/css-flexbox css/css-grid cssom domparsing url encoding; do
  cargo run -q -p manuk-wpt --release --features spidermonkey -- wpt "$d"
done

# 2. HISTOGRAM THE FAILURE MESSAGES of the biggest area. This is the work list.
cargo run -q -p manuk-wpt --release --features spidermonkey -- wpt html/dom --show-failures 2>&1 \
  | grep -oP '^         \K.*' \
  | sed -E 's/"[^"]*"/"X"/g; s/\b[0-9]+\b/N/g' \
  | sort | uniq -c | sort -rn | head -20
```

**Rank by count. Take the top row whose fix is a mechanism rather than a special case. Implement it
generically. Gate it. Falsify it. Land it. Repeat.**

Every step of that is already built. None of it needs a human.

---

## Why the histogram and not the score

A per-directory pass rate tells you *where* you are losing. It cannot tell you *what* is losing, and those
are different questions with wildly different answers:

* `dom/` was at 36.8% after **ten ticks** of careful, correct work. It is **6,484 subtests**.
* `html/dom` was at 21.0% and had **never been measured**. It is **59,818 subtests** — nine times larger.
* And **80% of its failures were a single mechanism** (attribute reflection), worth **~38,000 subtests**.

Ten ticks were spent on 9% of the ground that was already checked out, because nobody ran the loop over
the other areas. **The score tells you how you are doing. The histogram tells you what to do.**

---

## The ranking rule, when counts are close

Prefer, in order:

1. **A mechanism over an instance.** "Reflection" beats "fix `input.disabled`". One is a tick; the other is
   four hundred ticks.
2. **Both horizons over one.** `Range`, `TreeWalker`, `MutationObserver`, reflection — every one of these
   is *also* how real pages work. A WPT win that no page would ever notice is worth less than the same
   number of subtests behind something a page actually calls. The near horizon (doc/app/platform web) and
   the far horizon (WPT) are **nearly orthogonal** (measured, tick 70) — but the overlap is where the
   cheapest tick lives, and it is where the last ten capability ticks came from.
3. **A real fix over a stub.** *A stub is worse than an absence* — proven four times (`Range`,
   `TreeWalker`, `MutationObserver`, and the tokenlist half of reflection). A library feature-detects a
   stub, finds it, registers, and silently never works. **Skip it and say so** rather than fake it.
4. **Bar 0 over everything.** A hang or a crash outranks any score. Tick 82 introduced a crash worth
   −1 file while landing +9,940 subtests, and the tick could not have shipped until it was gone.

---

## The exhaustion rule (go broad before narrow)

**Keep taking the largest mechanism until there is no mechanism left**, and only then start on
special cases. Concretely:

* while the top histogram row is **≥ 500 subtests** and is a *mechanism* → it is the tick, whatever area
  it is in;
* when the top row falls below that, **re-measure every area** (a fix in one moves the others) and check
  whether a previously-second-place area is now the biggest;
* only when *no* area has a ≥500 mechanism left does the loop go narrow — file by file, message by
  message, still ranked by count.

A mechanism you have not looked for cannot appear in your ranking. **Widen the checkout before you narrow
the work**: `scripts/wpt-setup.sh` controls which areas exist at all, and an area that is not checked out
scores zero and is invisible.

---

## The no-handback rule

The loop does not stop, and it does not ask. Concretely, every tick:

1. reads `STATUS.md` and the last journal entry (that is the whole hand-off);
2. runs the histogram above;
3. picks the top mechanism;
4. **builds the probe first** — the most expensive thing in this loop is guessing, and it is *still* the
   most expensive thing (PROCESS #45);
5. implements, gates (`G_*`), **proves the gate goes red** (`falsify.sh`), runs the wall;
6. lands with `scripts/tick.sh`, which timestamps the tick into `CADENCE.tsv` automatically;
7. goes to 1.

There is no step that requires a human. **A question to the user is a tick that did not happen.** If a
decision is genuinely ambiguous, take the reversible option, write down why, and keep going — the journal
is the place to be uncertain in, not the prompt.

---

## What "done" means

**Near horizon — the daily driver.** Every ❌ in `docs/loop/WEB-PATTERNS.md` is either ✅ or carries a
receipt saying why not; `G_CAPABILITY` asserts every ✅; the 265-site oracle has zero Bar 0 residue.

**Far horizon — WPT.** Ladybird crossed **90% of WPT subtests** in 2025 — the bar Apple uses for
alternative-engine eligibility on iOS, and therefore the only externally meaningful number. That is the
target. Not "most of dom/". Not a percentage of what happens to be checked out. **90% of all of it.**

Neither horizon is met. The loop continues.
