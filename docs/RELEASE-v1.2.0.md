# manuk v1.2.0 — the agentic bar, measured on a real denominator

Where **v1.1.0** was the first honest cross-axis checkpoint, **v1.2.0** is the release
where manuk's *agentic-driving* capability — the surface an agent actually uses to
perceive and act on the legacy web — is measured on a **real denominator** for the
first time. The headline numbers below are M2 (drive/addressing) and a11y (accessibility
tree), taken on a de-bot-walled corpus of Chrome-reachable real sites.

v1.2.0 is a **measurement-integrity epoch**, not a capability jump. Its value is that the
agentic-bar numbers are, for the first time, *trustworthy* — because the instrument that
produces them was found to be silently discarding most of the corpus, and fixed.

## The instrument fix that makes these numbers real

The M2 and a11y sweeps ran ~10–20 sites under a **single shared timeout**, so the first
site to hang the driver silently wiped every site after it in the batch. The prior sweep
therefore measured M2 on **11/76** and a11y on **10/76** reachable sites — and the
survivors were the *fast* ones, biasing both headlines **up**.

Each site now runs under its own **per-site timeout**; a site that hangs the driver scores
a **tagged zero** (an undriveable site is a *failure* of the agentic bar, not a missing
data point) instead of vanishing. Measurement coverage went **11 → 60 sites (M2)** and
**10 → 65 sites (a11y)**. The headline now reports **FULL** (a hang counts as 0) beside
**measured-only** (quality where it scores), so the sampling bias is visible, not hidden.

## Measured capability (real sites, tri-sweep)

| Axis | What it measures | measured-only (quality) | FULL (real denominator) |
|---|---|---|---|
| **M2 — drive/addressing** | perceive → address → act on page targets | **78.9%** (n=60, 48% of sites ≥90) | **62.3%** (16 undriveable = 0) |
| **a11y** | accessibility tree vs Chrome (F1) | **82.0%** (n=65, **63% of sites ≥90**) | **70.1%** (11 unscored = 0) |
| **M1 — visual layout** (context) | render fidelity vs a Chrome oracle | coverage **89.9%**, SHAPE **72.1%** | — |

Reachability: **76 of 265** corpus sites yield a scored tree; the other 189 bot-wall the
*oracle Chrome itself*, so they are out of scope, not our failures. On the 76 reachable
sites, manuk scores every one.

Conformance substrate: **~496,000 WPT subtests** tracked, **570+** internal correctness
gates green.

## Honest reading

- **a11y is close and bimodal-good.** Where it scores, **63% of sites are already ≥90%
  F1** and 78% are ≥80%; the gap is a *per-site tail* of extraction bugs (e.g. a site that
  renders correctly but produces a disjoint a11y tree), not a gradual shortfall. The
  full-denominator **70.1%** is dragged by 11 sites the scorer couldn't finish and ~9
  measured sites below 50% — both tractable, named populations.
- **M2 addressing is the same shape:** a strong ≥90 cluster (48% of measured sites) and a
  low tail. The remaining fifth is largely *ambiguity* (targets needing an ordinal to
  disambiguate); there is still no end-to-end drive demo.
- **M1 placement remains the long pole.** We draw ~90% of the elements Chrome draws but
  place only ~1/3 of sites basically-right (SHAPE ≥90). Visual daily-driver parity is the
  slowest-converging axis and the honest remaining Phase-0 work after the agentic tail.

## What v1.2.0 delivers

Not a capability jump — a **trustworthy agentic-bar measurement** to grind against. The
three remaining Phase-0 work items are now named and quantified: the **M2 ambiguity/tail**,
the **a11y per-site extraction tail** (+ the 11-site scorer-timeout population), and
**M1 placement** as the visual long pole.

_v1.0.0 shipped; v1.1.0 measured all three axes at once; v1.2.0 makes the agentic-bar
measurement honest — a real denominator, with the sampling bias shown rather than hidden._
