measurement(render): "render-failed" is not a paint bug — the author CSS silently never arrives

t706 owed the 21 render-failed sites: the largest tractable block in the unscored 134 and the one
reason the taxonomy calls "our own bug".

IT IS NOT A PAINT BUG AND IT IS NOT DETERMINISTIC.

  superuser.com   in the t706 sweep    render-failed
  superuser.com   re-run, same binary  verdict OK  visual 62.3%  coverage 86.4%  SHAPE 15.5%

Four others (theverge, mongodb, kotlinlang, serverfault) DID reproduce, so the sweep's count is sound
and t706's certificate stands — but at least one flips, so render-failed is INTERMITTENT and a tick
that treats it as a fixed list of 21 broken pages is chasing a moving set.

WHAT THE BLANK PAGE ACTUALLY IS: THE UA STYLESHEET. Same platform, opposite outcomes:

  serverfault (blank)    html [8 8 1184x7328]   body display=Block   <- 8px is body{margin:8px}, UA default
  superuser   (renders)  html [0 0 1200x720]    body display=Flex

The blank page is the document laid out with UA defaults only. A 720px flex app-shell becomes a
7,328px unstyled scroll and the viewport screenshot catches a nearly empty region. We did not fail to
paint; we painted the wrong document, faithfully.

THREE PLAUSIBLE CAUSES, ALL WRONG:
 - "the <link>s are missing from our DOM" — they are, but so are superuser's, and superuser renders:
   the CSS loader consumes them like a <script>'s src. A red herring, and the convincing one.
 - "the origin serves us a degraded page" — no: curl with OUR UA returns the same 205KB document with
   all 9 <link>s, and parsing those identical bytes via boxes --html puts all 9 in the DOM.
 - "the load budget expires" — no: no budget message; stderr for a failing load is ONE line.

The stylesheets are fine: 200 text/css, 830KB + 311KB.

THE ACTIONABLE PART IS THE SILENCE. A render-blocking stylesheet that never applies produces no
diagnostic at all. The engine already computes blocking_css_missing(), whose own comment says
"Non-zero means the current layout is (partly) UA-default fallback, NOT this engine's rendering of
the author's page — a measurement that diffs it against a fully-styled reference is charging us for a
stylesheet that never arrived." That sentence describes this tick, and nothing prints it. So 21 sites
are booked as "failed to paint" when the honest label is "rendered without the author's CSS".

No engine change, so no gates. CLUSTER: render-failed (21 sites / 8% of corpus) — mechanism narrowed
from "we cannot paint" to "author CSS did not apply, silently, intermittently", three candidates
eliminated by measurement. Not fixed, not claimed as fixed.

WIKI: none — a diagnosis, not a behaviour; the engine does nothing today it did not do yesterday and
the wiki must not describe a fix that does not exist. [no-pattern]
