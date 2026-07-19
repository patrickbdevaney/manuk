probe(loop tick 241): measure-and-pin the unknowns — one is a DEAD-END WIRE, worse than absent

Board lever (E), and process rule 2: a cheap re-probe MUST precede any build tick aimed at an
unknown. Six constellation rows carried `unknown`; four are now pinned by measurement and two
remain, honestly named as JOBS rather than mysteries. An `unknown` is not a neutral cell — it
reads as "maybe fine", which is the optimistic direction of the same failure tick 238 caught in
the pessimistic one.

THE FINDING WORTH THE TICK — QUIRKS-MODE IS A DEAD-END WIRE. html5ever detects quirks correctly
and engine/html/src/sink.rs:199 set_quirks_mode stores the verdict in self.quirks — a field that
is WRITTEN AND NEVER READ (decl :54, init :63, setter :200, ZERO readers). Every Stylo call site
then hard-codes QuirksMode::NoQuirks (stylo_engine.rs:105/259/275/862/931, stylo_traits.rs:151,
stylo_probe.rs:78/106/111, stylo_dom.rs:537). So the parser knows the document is quirks and the
style system is never told. document.compatMode (dom_bindings.rs:5222) is hard-coded 'CSS1Compat'
behind a comment asserting "our documents are never quirks-mode" — which the parser contradicts
on every doctype-less page.

AND THAT IS WHY IT IS A FINDING RATHER THAN A FIX IN THIS TICK: flipping compatMode ALONE would
be a regression risk. Reporting BackCompat while still rendering standards makes sites take
quirks code paths we do not honour — a lie in the other direction, and a worse one, because it is
actionable by the page. The wire is SHORT (thread sink.quirks -> Dom -> the ~10 Stylo call sites;
Stylo already implements the quirks themselves, so this is plumbing, not layout math) but the
rendering and the reporting must land TOGETHER or neither. Scoped, not smuggled in here.

CSP (row 61) AND WEBTRANSPORT (row 95): ZERO HITS, ABSENT BY CONSTRUCTION. A grep fact, which is
stronger than a failed fixture — a fixture cannot distinguish a missing feature from a bad case.
CSP's consequence is two-sided and worth stating plainly: the security property is absent, AND we
do not MIS-enforce a policy we never parsed. WebTransport depends on HTTP/3, which V1-SCOPE.md
explicitly skips, so it is a deliberate deferral; keep it honestly absent (no stub) so feature
detection fails cleanly.

ROW 111 WAS STALE ABOUT OUR OWN INSTRUMENTS. It claimed "F1/F2 don't cover a big single DOM".
They do: the `large` fixture is 357.2KB / 8808 nodes — which brackets GitHub's 1k-5k-line blobs —
at ~150ms (parse 6.6 / cascade 6.5 / layout 98 / dlist 6.4 / paint 30). THE SHAPE IS THE RESULT:
layout is 65% of the pipeline and scales ~LINEARLY (1208->8808 nodes is 7.3x; 13->98ms is 7.5x),
so there is no accidental O(n^2) hiding there — which is the thing that would have made big blobs
hopeless. The genuine remaining unknown is not throughput but INTERACTIVITY on such a page
(scroll, selection, re-layout after an edit), which F1/F2 do not model. A narrower and more useful
question than the row was asking.

THE TWO LEFT UNPINNED, NAMED AS WORK RATHER THAN LEFT AS '?': the 100-tab RSS budget (row 79 —
defined, never run; this is MEM-HARNESS, and it can FALSIFY our own memory positioning, which is
exactly the point of running it) and test262 (row 100 — never run; a local suite exists at
v8/test/test262, 296K, so the missing piece is a RUNNER against our embedded SpiderMonkey, not
the corpus).

TICK SHAPE: probe the remaining UNKNOWNs and pin each with a measurement.

Unknowns 6 -> 2. No capability was claimed that a gate does not hold, and nothing in this tick
changes engine behaviour — the diff is the constellation and the journal.

[no-pattern] — this tick unlocks no class of the web; it corrects what we BELIEVE about four
capabilities. The quirks-mode wiring it scopes would unlock one, and that is a separate tick.

WIKI: none — a measurement tick with no engine change; the mechanism worth retrieving (the
dead-end quirks wire and why compatMode must not flip alone) is captured in the CONSTELLATION
receipt and JOURNAL entry, which is where the next builder of that row will look.
