reliability(cert): `curl` exits 0 on a 403, so a bot wall and a document were the same thing

TICK SHAPE: reliability/measurement. Bar 0 untouched; no ratchet floor moved; no capability traded.

The board's live CO-#1 is the certification redesign, and observer t602 named the target precisely:
"do NOT jump to the full 400-site sweep — the binding constraint is the 64% unscored rate. Fix WHY 9
of 14 cannot be scored." This tick asks the instrument that question. The answer was about the
instrument.

THE HYPOTHESIS GOING IN WAS WRONG, which is why it is recorded. t606's own NEXT line said "imdb is
'no [id] elements' — the selector-path keying should not need ids at all, so this is the next keying
gap." Plausible, and the same one-rule-two-implementations shape t610 just closed. But
`capture_boxes_all_ids` has NO CALLERS AT ALL — the fidelity path has keyed on selector paths since
t532, exactly as advertised. The `[id]` story was the instrument's own STALE ERROR MESSAGE describing
a failure it no longer had, and it misdirected for five ticks.

MEASURED INSTEAD — the probe's own fetch, across the 20 HEAD sites of corpus-v2.tsv:

  200 with a body ......... 11        measurable
  403 Cloudflare .......... 5         tamildhool · mangago · supjav · fdown · quora
  202 zero bytes .......... 1         imdb.com
  transport failure ....... 3         docomo · pitc · fawanews

SIX OF TWENTY ANSWER WITH AN HTTP STATUS THE INSTRUMENT NEVER READ. The fetch checked
`out.status.success()` — curl's PROCESS exit code, not the HTTP status — and `curl -sL` without `-f`
exits 0 on a 403. So Cloudflare's "Just a moment…" interstitial came back AS THE DOCUMENT, and
imdb.com's 202-with-zero-bytes as an empty document. That is the bulk of the pilot's "9 of 14", and
it is why "find out why" was unanswerable FROM THE INSTRUMENT'S OWN OUTPUT: every distinct way of
failing to reach a page arrived at the report as the same bare `—`.

A SECOND MECHANISM A STATUS CHECK ALONE WOULD NOT FIND. A 403's body is not inert: the challenge page
ships `<meta http-equiv="content-security-policy" content="default-src 'none'; script-src 'nonce-…'">`,
and the whole probe works by INJECTING A `<script>` into the fetched HTML. Chrome parses it, refuses
to execute it, and dumps a DOM where the probe is present as TEXT and its output never existed.
`parse_seen_probe_json` fails with exactly the right words — "did Chrome run the script?" — and the
caller was `if let Ok(mut cseen) = …`, which threw that sentence on the floor. The instrument had been
telling us the answer for five ticks. The error type is now `Unmeasurable`, which an `if let Ok`
cannot quietly discard. `aparat.com` is the case that needs this and not the status check: it answers
200 with a real document and only its CSP blocks the probe.

THE SHARP EDGE. t607 landed the complementary truth for the ENGINE — an HTTP error status IS a
document, 403/404/429/500 pages must render, because the user has to see them. Both are true at once:
THE BROWSER RENDERS THE 403; THE CERTIFICATE REFUSES TO COUNT IT AS EVIDENCE ABOUT THE SITE BEHIND IT.
The harm is worse than a missing label — a challenge page is a real document both engines render, and
they AGREE, so left on the ordinary path it scores as high fidelity on a site we never reached: a gate
passing by comparing a refusal against itself. `certificate()` now skips a reasoned row before it
reads any score. Until now "unscored" was true only by ACCIDENT of control flow, and the accident was
one plausible edit away from reversing.

FOUR SILENT DROPS OUT OF THE FIXED DENOMINATOR, all `continue`, all in one loop: our fetch failed ·
our PAINT failed · chrome capture failed · probe failed. The first honest run returned `sites 17` for a
20-site corpus. The paint one is the worst and was the quietest — a render failure REMOVED the site
from the corpus instead of counting against it, so the outcome that most deserves to lower the score
was the one that could not. RED-proved by restoring one `continue`:

  with the drop      sites 1 · scored 1        <- 100% of the corpus scored
  counted            sites 2 · scored 1 — 1×bot-wall-403

That is §0's cause #1 — "dropping the hard sites is what made every past reading optimistic" —
reproduced live, INSIDE the instrument built to prevent it, 28 ticks after the rule was written. A
rule enforced at one layer is not enforced. Third session running for this class (t591 @supports, one
of two lists; t610 run_with_fetcher, one of two drains).

MEASURED, 20 HEAD sites, after all four:

  sites 20 · scored 5 · shape >=0.75 on 0 (0.0%)
  15 of 20 UNSCORED — 5×bot-wall-403, 4×unreachable, 1×empty-202, 1×probe-blocked
  4 of those have NO recorded reason — an instrument gap, not a result
  MEAN VISUAL 59.2% · MEAN COVERAGE 62.8% · MEAN SHAPE 42.8%

`sites 20`, where the same corpus read `sites 17` an hour earlier and `sites 14` at the pilot. The
denominator is whole for the first time, and the headline got WORSE by exactly the amount it had been
quietly flattering itself.

AND ONE MORE LIE, FOUND BY aparat: it printed `99.9% … ok` on the old code, because the per-row
verdict falls back to the PIXEL score when there is no structural score. The aggregate was already
correct, which is what makes the shape dangerous — THE NUMBER A HUMAN READS SAID THE OPPOSITE OF THE
NUMBER THE GATE USED. An unmeasurable row now reads `UNMEAS` and is excluded from MEAN VISUAL too;
otherwise two headlines three lines apart are computed over two different populations.

RED-PROVEN SIX WAYS, EACH A DIFFERENT MECHANISM (t610's lesson: vary the MECHANISM, not the threshold):
  · make `classify_fetch` status-blind again   -> RED, the 202/403/404 classifications collapse
  · drop the reason column from the row file   -> RED, the reason dies at the chunk boundary
  · hide the unexplained-residue shortfall     -> RED, an unexplained site reads as explained
  · let a reasoned row into the scoring loop   -> RED, a bot wall meets the placement bar
  · restore the `continue` (the silent drop)   -> RED, `sites 2` becomes `sites 1 · scored 1`
  · add a variant with no `from_tag` reader    -> RED, the tag writes but never reads back
The fourth went red BEFORE its fix was written — the assertion is how the scoring exclusion came to
exist rather than being assumed.

RESIDUE, NAMED NOT SMOOTHED: the 4 unscored-without-reason (comix, welt, naukri, ebay) all REACHED the
scorer and fell below CERT_MIN_SHAPE_SAMPLE — comix probed 3 paths, naukri ONE. A different class
entirely, and deliberately NOT given an `Unmeasurable` variant, because inventing a label for a cause
I have not measured is how the `[id]` message this tick deleted came to exist. Also: the counted-row
invariant is gated at `certificate()` but NOT at the sweep's call site (main.rs has no test harness) —
proven by RED probe 5, not held by a gate.

⚠⚠ OBSERVER: `manuk-wpt`'s TESTS ARE NOT IN THE WALL. verify.sh's `_crate_suite` covers manuk-css
manuk-layout manuk-paint manuk-dom manuk-net manuk-agent manuk-shell — there is no manuk-wpt and no
`_launch` line. So G_CERT_FALSIFIABLE, whose own doc comment says "this is the proof, re-run on every
wall", HAS NEVER RUN ON A WALL, nor has the vacuous-pass guard, nor this tick's gate. They are green
(`cargo test -p manuk-wpt` -> 54 passed in 14s) and the RATCHET's GATES count has been counting them
as coverage. scripts/ is observer-owned; one line adds them.

Gate: G_UNMEASURABLE_REASON (tests/wpt/src/fidelity.rs, `an_unscored_site_must_name_its_cause`).
WIKI: docs/wiki/conformance-and-oracles.md — "`curl` exits 0 on a 403, so the certificate could not
tell a bot wall from a document".
PATTERN: [no-pattern] — no browser capability changed; this is the instrument.
