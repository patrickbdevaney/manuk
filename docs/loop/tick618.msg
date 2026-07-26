reliability(map): SURFACE AUDIT #34 — the map claimed `works` for a capability that does not exist

TICK SHAPE: reliability/measurement (the due surface audit). Bar 0 untouched. Four marks lowered with
cause and evidence; four raised by real work.

THE INSTRUMENT WAS WRONG FIRST, TWICE, AND BOTH TIMES IN MY FAVOUR TO BELIEVE IT. The gate-vs-map diff
reported 262 of 311 gate files unreferenced — entirely my own check matching FILENAMES against a map
that cites G_* CONSTANTS. Keyed correctly: 6, and all six are THIS SESSION'S OWN new gates, added
across five ticks and never mapped. Audit #31's standing rule held perfectly; the drift was mine.

Then map-reconcile reported G_CAP_TOUCH_PROBE as a DANGLING gate. IT IS NOT — it lives at
tests/wpt/tests/g_cap_touch_probe.rs and the reconciler scans only engine/page/tests/. I was one edit
from deleting a real gate's claim on an instrument's say-so, and only re-verified because this session
has been burned by exactly this four times. Every dangling report was then re-checked with a repo-wide
grep; six of the seven are genuinely absent.

THE HEADLINE: `ES modules + dynamic import()` claimed `works`, with NO gate, and half of it does not
exist. Static ESM is real and now gated four ways. Dynamic import() is NOT IMPLEMENTED — SpiderMonkey
says "Dynamic module import is disabled or not supported in this context" and a direct probe confirms
the module never completes. ONE ROW ASSERTED TWO CAPABILITIES AND REPORTED THE STRONGER ONE'S VERDICT
FOR BOTH. Split, with import() at `missing`.

A second row failed the same way: `fetch / XHR / AbortController — works`, no gate — while
xhr.addEventListener was a TypeError for as long as anyone had looked (t613: 8 of 16 HEAD sites
construct an XHR). A BARE `works` IS NOT A WEAK CLAIM, IT IS AN UNFALSIFIABLE ONE.

DRIFT 26 -> 0 (the observer's live ask since t601): 11 rows given a REAL backing gate, each verified to
ASSERT the claim rather than merely to have a plausible filename (pushState appears 4x in
G_DOCUMENT_LOCATION; contrast-color 12x in G_CONTRAST_COLOR). 14 set to `unknown` — @font-face has ZERO
occurrences in the entire gate directory, and ResizeObserver appears only in presence lists, where "the
global exists" is not "the observer fires". 1 row split. 5 new rows for this session's gates.
`unknown` rather than `missing` because `missing` would be A NEW LIE IN THE OPPOSITE DIRECTION — most
of these probably do work, nobody has measured them. G_CONSTELLATION_WELLFORMED rejected my first
vocabulary (`unmeasured`) and was right to.

⚠ FOUR RATCHET MARKS LOWERED, DELIBERATELY: CONST:doc 24->22 · CONST:media 11->10 · CONST:cross 17->16 ·
MEASURED 362->356. Those marks counted capabilities as GATED whose named gate has NO TEST FILE ANYWHERE
(G_AVIF_PAINT, G_AUDIO_PUMP, G_AUDIO_JOIN, G_TAB_DISCARD_RELEASES_TO_OS, G_FIDELITY, G_RATE). A gated
count containing phantoms was never a measurement of gating. The ratchet's own message provides for this
— "explain why the mark itself was wrong and lower it deliberately" — and the test applied first: WOULD
I MAKE THIS CHANGE IF IT DID NOT HELP ME LAND? Yes. Keeping a phantom gate to protect a number is the
worst available reason, and this is the same session in which I nearly had a WALL mark loosened over a
build artifact (t610), so the bar for touching a mark is one I have already failed once.

In the same pass four marks went UP — app 80->81, dom 19->20, js 1->3, net 5->7 — this session's real
gates. The correction is not a net retreat; it is a truer number in both directions.

RED-PROVEN: n/a, no engine source changed. Both instruments are themselves falsify-proven:
map-reconcile --self-test passes (fires on bare + dangling fixture rows, does not false-fire on a valid
one), and G_CONSTELLATION_WELLFORMED caught my invalid status vocabulary before it reached the map.

Gates: none added — this tick IS the audit.
WIKI: none [forced] — no engine mechanism changed; the finding's home is the audit ledger.
PATTERN: [no-pattern] — no browser capability changed.

NEXT: import() is now correctly `missing` and is the honest next capability — a subsystem (host hook +
async graph fetch), and welt.de reaches it. @font-face having NO GATE AT ALL is the most surprising
residue: web fonts drive text metrics, which is what half the fidelity work has been chasing.
