fetch(css): a deadline-cut stylesheet now counts — and the real cause is that our FETCH gets a different page

t707 owed "make the UA-default fallback loud". failed_stylesheet_fetches() exists so a measurement can
refuse to score a page we never styled, and it had ZERO callers.

THE FIX. collect_before_deadline returns the futures that FINISHED; the ones the deadline cut off are
absent from the result, so they never reach the `for (url, text) in fetched` loop that owns BOTH the
logging and the failed_css bookkeeping. A page whose CSS was cancelled rendered in UA-default fallback
with no STYLESHEET FAILED line, no failed_css entry, and failed_stylesheet_fetches() answering 0 — the
counter's blind spot was the common case. Now the requested set is diffed against the settled set,
every cut URL is recorded, and ONE warning names the count and the URLs (the failure is the DEADLINE,
a single event, so N lines would read as N faults). Gated by
a_stylesheet_the_deadline_cut_off_is_counted_as_failed with the deadline set in the PAST so the cut is
deterministic and needs no network. RED-proven: delete the bookkeeping and it reports 0.

AND IT DOES NOT FIRE ON serverfault.com:

  load phase done  phase="external CSS"  ms=0  gave_up=0  elapsed_ms=0

The external-CSS phase found nothing to fetch. No sheet was cut because none was requested. The fix is
correct and stays; it is not this site's bug.

THE CORRECTION. t707 ruled out "the origin serves us a degraded page" by fetching with curl -A "<our
UA>" and getting the full 205KB document with all 9 <link>s. curl is NOT our net stack, and standing
one in for the other is the whole error:

  serverfault.com <head> element children
    our engine, live fetch      9      <- and not one <link> among them
    the same URL via curl      49

Same URL, same User-Agent, same moment. The document our engine receives is stripped of its head,
which is why collect_style_sources finds no external stylesheets, why the CSS phase costs 0ms, why the
page lays out under body{margin:8px} at 7328px instead of a 720px flex shell, and why the sweep books
it render-failed. t707's elimination #2 is RETRACTED. Suspect the instrument before the subject — and
a stand-in for the subsystem under test IS the instrument.

Gates: a_stylesheet_the_deadline_cut_off_is_counted_as_failed (RED-proven). Page lib 21 passed / 1
pre-existing failure (static_import_scanner..., present at HEAD since before t704).

WIKI: none [forced] — net/load behaviour, not a rendered construct: docs/wiki topics map the engine's
RENDERING mechanisms, and the class this belongs to (a render-blocking stylesheet that never arrives)
is recorded in WEB-PATTERNS.md with the full measurement. Writing a wiki topic for a cause that is
localised but NOT yet fixed would describe a behaviour the engine does not have.
