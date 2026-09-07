# manuk v1.1.0 — first cross-axis daily-driver capability checkpoint

Following the **v1.0.0** public ship, this release marks the point where manuk's
readiness as a *daily-driver browser* was, for the first time, **measured honestly
across all three axes at once** — visual layout (M1), JavaScript interactivity (M2),
and accessibility (a11y) — on a de-bot-walled corpus of real, Chrome-reachable sites.

v1.1.0 is a **measurement epoch**, not a "we're done" milestone. Its value is that the
numbers below are the real ones: coverage that used to flatter has a denominator that
now counts what did **not** render, and the sweep that produces these figures was itself
rebuilt after it was found to be silently mis-reporting.

## Measured capability (real sites, tri-sweep)

| Axis | What it measures | Result |
|---|---|---|
| **M1 — visual layout** | render fidelity vs a Chrome oracle | **coverage 88.5%**, **placement (SHAPE) 69.4%** |
| **M2 — JS interactivity** | perceive → address → act on page targets | **drive/addressing rate 82.0%** |
| **a11y** | accessibility tree vs Chrome (F1) | **F1 79.4%** |

Conformance substrate at this checkpoint: **~496,000 WPT subtests** tracked, **569**
internal correctness gates green.

## Honest reading — none of the three bars is met yet

- **M1 visual layout:** we place ~88% of the elements Chrome places, but **placement is
  the weak axis** — average SHAPE 69.4%, with a real tail of pages badly misplaced
  (a cluster below 40%). We render the right things; we don't yet put enough of them in
  the right place.
- **M2 JS interactivity:** the drive loop can address ~82% of real-page targets; the
  remaining ~1/5 is largely *ambiguity* (targets needing an ordinal to disambiguate), and
  there is **no end-to-end drive demo** yet.
- **a11y:** F1 79.4% overall — about half of sampled sites already clear 90%, but a
  **sub-50% tail** drags the mean down. The earlier "≥90%" figure was withdrawn when it
  was found to have been scored on an unrendered page.

## What v1.1.0 actually delivers

Not a capability jump — an **honest floor to build from**. The three gaps are now named
and quantified rather than assumed: **M1 placement**, the **a11y sub-50% tail**, and an
**M2 end-to-end drive demo**. Those are the daily-driver work items from here.

_v1.0.0 shipped; v1.1.0 is the first checkpoint where the browser measures itself the way
a user would judge it — across layout, interactivity, and accessibility together._
