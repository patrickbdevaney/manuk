reliability(cert): the certificate said `verdict ok` about a blank white page

t650 ranked PLACEMENT as the largest remaining gap and named agoda (shape 0.0%) as the cheapest
thread. Opening the side-by-side first — the instrument's own instruction, THE SCORE GATES; THE
EYEBALL DIAGNOSES — there was no placement bug to find. Our render of agoda is a COMPLETELY BLANK
WHITE PAGE, and the certificate reported `visual 69.9% · COVERAGE 100.0% · missing 0 · verdict ok`.

Four guards were in position and each missed it for its own reason. COVERAGE is computed against the
oracle's file:// probe, which built a 13-element shell — we had all 13, so 100% of nothing is 100%.
ShellOnly counts the oracle's SAMPLE and 13 clears the floor, so the guard written for this exact
shape of lie missed by a threshold. VISUAL scored 69.9% because most of a page is background either
way — the instrument's own caveat arriving as a false PASS. SHAPE did fire, but it reports as a
PLACEMENT number, which sends you hunting a layout bug on a page that has no layout.

They are all DOM-side measurements, and the DOM was fine. The elements exist, they have boxes, they
compare. Nothing on that side can see that the paint is empty.

The fix is the other population, which §6.4 already asks for and which was sitting unused: both
screenshots are in hand when compare() runs. INK — the fraction of blocks differing from the image's
own modal block colour — is self-relative on purpose, so it asks "did this engine draw anything"
rather than "is this page white". Thresholds MEASURED, not chosen: agoda 0.00% against every genuine
render at >=1.07%.

The two guard cases carry the design. The ORACLE must clear 10% ink for our 0% to mean anything
(comix/naukri are Chrome-side shells; blaming our renderer would be a false accusation). And a page
rendered BADLY is not a page rendered NOT AT ALL — keirin at 8.11% ink / 2.2% shape stays SCORED,
because excusing our worst real renders as unmeasurable is how a certificate launders its own
failures: the same disease as the false pass, pointed the other way.

One certificate term moves in the honest direction: scored 5 -> 4 on HEAD-20, because agoda's row was
never a measurement. No ratchet floor was moved to accommodate it.

Also measured and NOT fixed here, named so it is not lost: agoda is blank because a webpack chunk
fetch TIMES OUT, the app never converges (task ceiling x10, ~2.2s each), and ~22s of spin eats the
12s load budget. Two hypotheses were tested and FALSIFIED rather than assumed — no dynamic script
fetch failed, and script.src DOES reflect to the attribute. That is the next tick, and it is engine.

Gates: G_BLANK_RENDER_CANNOT_SCORE, RED-proven by mutation and verified end-to-end on the real pairs
(agoda ok -> UNMEAS[render-failed]; ikea unchanged at SHAPE 52.0%, n=698).

WIKI: docs/wiki/conformance-and-oracles.md — "100% of nothing is 100%".
