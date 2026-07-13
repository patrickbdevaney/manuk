# CSS AND THE CASCADE — Stylo realities and quirks actually encountered

## Stylo's *servo* build hardcodes `parse_has() -> false`

A selector containing `:has()` therefore **fails to parse**, and CSS error-recovery **discards the
whole rule** — its declarations never reach the cascade at all. (Gecko's build returns `true`; this is
a *build default*, not a capability limit.) **13% of the corpus uses `:has()`.**

**`./stylo` in this repo is a REFERENCE CHECKOUT THAT NOTHING BUILDS.** The workspace depends on
`stylo = "0.19"` **from crates.io**. Editing the local checkout changes nothing — a fact that cost a
tick to discover and that **re-prices every "just flip the flag" idea** into "vendor Stylo and pay the
tax on every bump."

**The resolution — and the ladder it established:** extend the selector engine we *already own* (the
one behind `querySelectorAll`) with `:has()`, and apply the rules Stylo discarded as a **second cascade
pass** ordered by `(specificity, source order)`. Contained, no fork.

> **The ladder, now settled:** pref → minimal flag delta → **hand-rolled supplement** → hand-rolled
> module. **Never: give up the capability.** (But never copy Blink/Gecko *code*, and never fork an
> engine's *algorithms*.)

**Known, bounded inaccuracy, stated rather than discovered later:** a low-specificity `:has()` rule
cannot currently *lose* to a higher-specificity normal rule, because Stylo does not report which rule
won each property. Strictly better than the rule not existing — and written down.

## An optimisation that makes a data structure smaller must be asked WHAT IT DROPPED

`RuleIndex` (a tick-14 cascade optimisation, 339ms → 199ms) walked each stylesheet's rules, read each
`StyleRule`'s `selectors` and `block`, and **never looked at its `rules` field** — the **nested** rules.
Stylo parses them correctly and always had. We threw every one of them away.

**≥41% of the corpus uses CSS nesting** in its inline `<style>` blocks *alone* (external sheets were not
even scanned, so that is a **floor**). It was the single largest cause of both real rendering
divergences the oracle found: *"we lose flex/grid on this node"* (11,324) and *"we show what Chrome
hides"* (2,433 — a nested `display: none` never applied, so menus and modals rendered on top of the
page).

> **A gate comparing boxes could not see it**, because the boxes it produced were internally consistent
> — they were just consistently wrong.

## `<body>`'s background propagates to the CANVAS

If it does not, **every dark-themed site is a dark box floating in a white void.** Found via an
iframe, because *a child document is "a page shorter than its viewport"* — the same condition, made
obvious.

## `visibility` and `opacity` interact with animation

An element with `opacity: 0` that *specifies an animation* is not hidden — it is **about to be shown**.
Treating the computed value as final hid ~a fifth of the web's content.
