capability(text): a downloaded web font is a THIRD reason to re-lay-out — and web fonts had NO gate

TICK SHAPE: capability + measurement. Bar 0 untouched; no ratchet floor moved; no capability traded.

Surface audit #34 (t618) turned up one residue more surprising than the rest: @font-face had ZERO
occurrences in the entire gate directory while the map claimed it `works`. Web fonts drive text metrics,
which is what half the fidelity work has been chasing, so it went to the top.

THE PROBE FOUND A REAL BUG, AND EVERY LAYER IN ISOLATION WAS CORRECT. Serving the WPT Ahem face (every
glyph exactly 1em, so 5 chars at 20px is exactly 100px and no fallback can land there by accident):
measured 66.7px. Bisected downward and each layer passed — WOFF2 decode fine (7 manuk-text tests green,
incl. this fixture), manuk-text registers the face and measures XXXXX at EXACTLY 100.0, and the font was
even fetched (the local server logged GET /ahem.woff2). What was missing sat one level up:

    if count > 0 || self.dom.has_dirty() {   // external CSS arrived, or a script mutated the tree

Two reasons to re-lay-out, and there are THREE. A face that has just arrived changes the advance of
every glyph, so every line box, wrap point and content height computed with the fallback is stale. The
guard is not wrong to exist (an unconditional re-cascade is 257ms a go on bbc.co.uk); it was INCOMPLETE.
AN OPTIMISATION GUARD IS ONLY AS CORRECT AS ITS LIST OF INPUTS.

⚠ AND THEN THE MEASUREMENT DISAGREED WITH MY OWN PRICING, WHICH IS THE PART WORTH RECORDING. I expected
this to move real sites. Measured across the HEAD corpus: 10 of 16 use @font-face, and 10 of those 10
have it in an EXTERNAL sheet. Only www.welt.de has one inline — and its fidelity is BYTE-FOR-BYTE
UNCHANGED by this fix (COVERAGE 94.9%, SHAPE 66.9% n=3063, VISUAL 82.8%, before and after). Because
`count` counts ALL external stylesheets, not only ones containing @font-face, any page with a single
external sheet already took the relayout branch. The broken path needs a page whose @font-face is inline
AND which has no external stylesheet at all. MEASURED BLAST RADIUS ON THIS CORPUS: ZERO SITES.

That is t590's lesson again — a capability's usage is not its impact; measure in your own engine before
pricing — and this time it corrected me AFTER the fix rather than before. The fix stays: correct,
RED-proven, cheap. But the tick's real deliverable is not the fix.

THE DELIVERABLE IS THAT WEB FONTS ARE MEASURED AT ALL, FOR THE FIRST TIME. The map asserted `works` on
nothing for as long as the row has existed; G_WEBFONT_RELAYOUT now asserts the whole chain end to end —
fetch, WOFF2 decode, registration, family resolution, and THE DOCUMENT ACTUALLY BEING LAID OUT WITH IT —
against a number only the real face can produce. Both stylesheet paths are gated; they take different
branches.

RED-PROVEN:
  · restore the two-input guard          -> RED, 66.7px instead of 100px
  · vacuity guard inside the gate itself -> the serif fallback must NOT also measure 100px, or `== 100`
    proves nothing about which face was used

⚠ I INTRODUCED A PERFORMANCE REGRESSION AND CAUGHT IT BY READING MY OWN DIFF. `registered_webfont` was
set on every successful fetch — but fetch_and_apply_stylesheets runs again after EVERY round of dynamic
scripts, so the same font would re-register each round and force a full-document relayout each time:
precisely the waste the guard exists to prevent. Fixed with FontContext::has_webfont_face(), which skips
a family already registered. A FIX THAT REINTRODUCES THE COST OF THE THING IT FIXES IS NOT A FIX.

⚠ ONE CHANGE WAS DROPPED FOR BEING UNPROVABLE. I had also excluded registered_webfont from the
RestyleDamage::None early return. Neither gate could reach that branch — the external case answers
damage != None because the sheet is genuinely new — so it went out. An unproven change gets a gate or
gets removed, and I could not construct the case it was written for.

MAP: the web-fonts row comes off `unknown` to `partial` (not `works` — font-display, unicode-range
subsetting and FOUT/FOIT are still unasserted). map-reconcile now reports drift 1, and that one row is
the INSTRUMENT'S blind spot documented in audit #34: G_CAP_TOUCH_PROBE exists at tests/wpt/tests/, which
the reconciler does not scan. The map is correct; the reconciler under-reads it. scripts/ is
observer-owned, so this is a note, not an edit — OBSERVER: widening that scan takes drift to a true 0
and unblocks --strict.

Gates: +2 G_WEBFONT_RELAYOUT (engine/page/tests/g_webfont_relayout.rs inline ·
g_webfont_relayout_external.rs external).
WIKI: docs/wiki/text-layout.md — "a downloaded web font is a third reason to re-lay-out".
PATTERN: [no-pattern] — the fix changes no class of site on the measured corpus, and saying otherwise
because a gate went green is exactly the overclaim this entry exists to avoid.

NEXT: t563's Lora finding (Chromium resolves a webfont where we fall back to serif) is still unexplained
— that page's sheet was EXTERNAL, so this tick does not cover it.
