feat(loop tick 102): getComputedStyle exposes visibility / white-space / opacity (correct, ratchet-neutral)

computed_style_js built a fixed ~30-property snapshot and dropped several ComputedStyle fields the
cascade already computes -- visibility, white-space, opacity. getComputedStyle(el).visibility
returned undefined instead of "hidden". Exposed all three (camelCase key + white-space kebab entry
in getPropertyValue + initial values), gated by G_COMPUTED_STYLE (proven falsifiable).

Honest result: ratchet-NEUTRAL. Full sweep TOTAL 389069 unchanged, all areas at banked marks,
crashes=0. No failing WPT subtest reads these three as undefined (the scored css-ui lever is
appearance/caret-color, which need new ComputedStyle fields + Stylo extraction -- deferred).
Landed tick-97-style: strictly more correct, zero regression, real scripts read these constantly.

WIKI: docs/wiki/dom-semantics.md -- "getComputedStyle must expose the properties the cascade ALREADY computed"

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
