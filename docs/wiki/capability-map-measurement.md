# Measuring the capability map — why `unknown` is the only status that rots

*Landed tick 1393 (surface audit #81). The instrument is
`engine/page/tests/g_constellation_unknowns.rs`.*

`docs/loop/CONSTELLATION.tsv` is the loop's map of what this browser can do: ~598 capabilities, each
with a `status`, a gate and a receipt. The ratchet's `MEASURED` invariant counts the rows that have a
**verdict** — `gated`, `works`, `partial`, `missing` — and deliberately does not count `unknown`.

## The failure this page exists to record

Surface audit #80 (tick 1383) reconciled the map against Interop 2026 and Baseline, found nothing
missing, and drew the correct conclusion — ranked #2, in writing:

> *"The 49 `unknown` rows are the frontier now, not missing rows. The map is complete against both
> lists the platform maintains; what it cannot tell you is whether we have those capabilities."*

Ten ticks later the count was **still exactly 49.**

> ⭐⭐⭐ **A finding without a mechanism is a finding that expires.** The analysis was right, specific
> and ranked. It moved nothing, because nothing executed it. The `MEASURED` number was displayed on
> every single tick and stayed flat, which means **a metric nobody is obliged to move is not a
> mechanism** — it is the same prose in numeric form.

The rule that came out of it: **an audit conclusion should name the file that will enforce it.**

## The instrument

One HTML page asking **58 observable questions**, loaded through this engine and through headless
Chrome, with every answer pinned as an assertion. It is a **measure-and-pin** gate, not a conformance
gate — most of its rows assert what we do *not* have.

That inversion is the whole design:

* **Pinning an absence is what makes the absence maintainable.** The day somebody implements
  `caret-color`, the gate goes red, and the map gets updated *in the same tick* as the engine —
  instead of drifting for ten ticks the way it just did.
* **A red row here is not necessarily a regression.** It may be a capability that just arrived. The
  gate's failure message says so, and says what to do: re-measure against Chrome, update the claim and
  the map together, and never loosen the claim so the map can stay stale.

### Ask both entrances

`CSS.supports` and `getComputedStyle` are two different doors (the t1353 rule), and the probe asks
both — because the answers genuinely disagree:

| capability | `CSS.supports` | `getComputedStyle` |
|---|---|---|
| `if()`, `shape()`, `::scroll-marker` | **no** | — |
| `column-rule-*` family | **no** | `undefined` |
| `zoom`, container style queries, `@media (update:)` | **yes** | — |

An `undefined` from `getComputedStyle` is a *measurement*: the property is not modelled. It is not,
by itself, proof that nothing applies it — `field-sizing` reports nothing and works — which is why
the probe pins the reading, and the map's receipt says which reading it was.

### Presence is not sufficiency

For `clipboard events` the probe can see that `ClipboardEvent` is a global. It cannot see whether
`copy` actually fires with a `DataTransfer`. For MODULE service workers it can see
`navigator.serviceWorker` exists, not whether `{type: 'module'}` is honoured.

Both went to **`partial`**, not `works`. **EXISTENCE ≠ SUFFICIENCY** — a cheap probe stays honest by
declining the question it did not ask.

## What one sweep produced

```text
  unknown  49  ->  11        works 17 -> 24 · partial 41 -> 44 · missing 147 -> 175
```

**175 `missing` is a better headline than 147, and the rise is the point.** `unknown` was never a
claim anyone could be held to; `missing` is.

And the new absences rank themselves. Nine of them are one family — the stylo `engine="gecko"`
properties (`column-rule-color/style/width`, `text-decoration-thickness`, `image-orientation`,
`text-emphasis-style`, `paint-order`, `box-decoration-break`): **one build-configuration decision, one
shared cause, six map rows.** A single lever wearing six hats, which the map could not show while the
rows read `unknown`.

## A row goes stale by being *worked on*

`accessible NAME computation (accname)` sat at `unknown` while **eight ticks measured it** — t1349-1350
(+229 subtests), t1379, t1384, t1386 — with WPT `accname` at 91.9%.

> ⭐⭐⭐ The rot a surface audit checks for is not only *"the world moved and we didn't notice"*. It is
> equally **"we moved and did not write it down"**. On the map, a capability the loop had spent eight
> ticks on was indistinguishable from one nobody had ever looked at.

## The row where the engine is ahead of its own oracle

`contrast-color()` — a Baseline 2026 / Interop 2026 focus area — is the only one of the 58 where
**this engine says supported and Chrome says not**:

| | manuk | Chrome |
|---|---|---|
| `CSS.supports('color: contrast-color(red)')` | `true` | `false` |
| `color` on a **white** backdrop | `rgb(0,0,0)` | `rgb(0,0,0)` *(initial, by accident)* |
| `color` on a **black** backdrop | **`rgb(255,255,255)`** | `rgb(0,0,0)` |
| `color: red` then `color: contrast-color(white)` | `rgb(0,0,0)` | `rgb(255,0,0)` *(invalid decl dropped)* |

⚠⚠ **The oracle divergence this creates is correct and must not be "fixed".** The North Star makes
Chromium the **ceiling** on capability, not the floor: a diff that sees `red` vs `black` here is
looking at us having *more* of a focus area than the reference. It is pinned in the gate so a future
diff cannot quietly reverse it — the ["a gate can pin the engine to a
bug"](../loop/JOURNAL.md) hazard running backwards.

## What is left, and why a probe cannot close it

The 11 remaining `unknown` rows are all layout, text-shaping or a11y:

```text
  anonymous TABLE-CELL wrapping of non-cell content (CSS 2.1 §17.2.1)
  a CELL's min-width reaching its column's intrinsics WITHOUT constraining it (§17.5.2.2)
  inline-level flex / grid container baselines (from the child's first line box)
  scroll anchoring · CSS 2.1 §8 margin/padding (the ~280-failure cluster)
  an inline box's own content area (§10.6.1) under a FALLBACK face
  caret placement INSIDE a ligature / grapheme cluster
  cross-<svg> url(#id) references · MathML rendering · scroll/animation event ORDERING
```

None of these is a `typeof`. Each needs a fixture and an oracle — which is the method this loop
already has, and which is also, not coincidentally, the CO-#1 rendering gap wearing a different hat.
