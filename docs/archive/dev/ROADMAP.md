# Manuk Roadmap

Manuk's north star is **a browser an agent drives natively** — one that perceives a page through its semantic/accessibility tree and box geometry, acts on it, and verifies the result. Pixel-fidelity to Chrome is a means and a proxy metric, not the end.

## Where we are (v1.0.0)

- **Render (M1):** ~36% of a 200-site CrUX corpus reach a ≥0.75 structural match to Chrome; content coverage is far higher (~87% mean) — **the gap is placement/layout geometry, not missing content.**
- **Web-platform conformance:** 78% of active-area WPT subtests (layout + DOM + CSS).
- **Accessibility tree:** ~64% node match.
- **Interactivity (M2):** broad JS/Web-API primitives present, not yet certified on real sites.

Measured honestly, the M1 climb is **decelerating** (roughly halving each week) under a per-assertion fix method — a classic asymptote. The plan below changes the *method* (port whole algorithms) and the *definition of done* (an agent doesn't need pixels).

## M1 — climb by porting, not grinding

Stop fixing per-assertion; port whole subsystems. Source priority: **Blitz** (Taffy + Stylo — our exact stack) first, **Servo** (full reference) second. Ranked by leverage:

1. **`writing-mode` / logical geometry** — currently unimplemented; silently blocks thousands of grid failures across three areas. Biggest single unlock.
2. **Table layout** — auto algorithm, percentage columns, fixed.
3. **Floats + BFC + clear.**
4. **Absolute positioning, containing-block, stacking contexts.**
5. **Flex/grid sizing edge cases** — align Taffy usage to Blitz.

Each lands as a step-function, not a decimal.

## Accessibility tree — the agent's perception layer

Promote the a11y tree from "last phase" to a first-class output of render. **Adopt AccessKit** (the standard Rust a11y-tree crate Servo uses) as the tree output; derive role/name/state/relations from DOM + ARIA + layout. Build it alongside render — it is cheap once geometry is correct and it is what the agent consumes.

## M2 — interactivity + the agentic driving surface

- **Primitives:** event loop, hit-testing (reads M1 geometry), dispatch (capture/bubble), focus, form controls + state, scroll — port Servo's event/hit-test path.
- **Driving surface** (built on the `bidi` crate + `script.evaluate`): a WebDriver-BiDi-shaped loop — *navigate → query (DOM + a11y tree) → actuate (synthesized click/type/scroll at geometry) → observe/verify.* **This is the product.**
- **Certification:** a stratified real-site smoke suite (login, search, nav, form-submit) A/B-diffed against Chromium, gated on render scorability.

## Definition of "good enough" — the finish line

Success is an agent driving the real web, which is a lower, achievable bar than pixel-perfect Chrome for a human:

| Surface | Bar |
|---|---|
| **M1 render** | shape ≥ 0.75 on ≥ 80% of the corpus (recognizable + geometry correct for derived structures) — not 95% pixel-match |
| **a11y tree** | role + name + state match Chrome on ≥ 90% of nodes |
| **M2 interactivity** | top-5 flows pass A/B vs Chrome on ≥ 85% of sites |
| **Overall** | an agent perceives + acts on ≥ 85–90% of representative sites |

Sequence: land the big M1 ports (writing-mode, tables) to get render "good enough," stand up AccessKit + the driving loop in parallel, then gate M2 on real-site A/B.
