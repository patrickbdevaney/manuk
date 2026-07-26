capability(net): a 404 page is a document to RENDER, not JavaScript to EXECUTE

TICK SHAPE: capability. Bar 0 STRENGTHENED (an injected error page was a SyntaxError in a frame, and
t612/t615 established what a throw costs). No ratchet floor moved; no capability traded.

t615 named the last visible rung on welt.de and called it the only mechanism-shaped one:
`a page module failed — SyntaxError: expected expression, got '<'`. Chasing it found a real defect —
though, as it turns out, NOT welt.de's (see HONESTY).

t607 WAS RIGHT, AND IT LEFT A HAZARD ITS OWN COMMENT PREDICTED. t607 stopped manuk_net::fetch failing
on `status >= 400`, because a 404/403/429/500 arrives with a real body and every browser renders it.
Correct. And it wrote: "The status is not swallowed: it rides on Response::status for every caller that
cares." NONE OF THE SIX SUBRESOURCE CALLERS CARED. Every one was the same line:

    let text = manuk_net::fetch(&url).await.ok().map(|r| r.decoded_text());

reading "the request completed" as "the request succeeded". So a <script src> that 404s had its error
page INJECTED INTO THE SCRIPT NODE AS INLINE JAVASCRIPT, a <script type=module> had it COMPILED AS A
MODULE, and external CSS took it as a stylesheet. Six consumers: two script paths, two CSS paths, two
module-graph paths — the sixth consecutive instance of §VI.3's fourth clause, and this time all six
copies were wrong the SAME way, which is its own lesson: when the copies AGREE, the rule can still be
absent.

THE DISTINCTION IS THE TICK, AND IT IS ONE FACT ANSWERED THREE WAYS. A 403 challenge page is:
  · a DOCUMENT to the navigation path  — render it            (t607)
  · NOT EVIDENCE to the certificate    — refuse to score it   (t611)
  · NOT CODE to a subresource loader   — do not execute it    (t616)
Getting one right does not settle the others — which is precisely how a correct tick introduced this.
The fix is ONE function, `subresource_text`, that all six sites route through, because patching six
sites is how you get a seventh. `< 400` rather than `2xx` on purpose: redirects are already followed by
the time a caller sees a response, and a revalidated 304 comes back as the STORED entry (status 200),
so this rejects exactly the error statuses and cannot regress a working path.

⚠ THE GATE'S FIRST DRAFT WAS VACUOUS — the third this session, and again only the RED probe knew. I
wrote it against `Page::load`, WHICH DOES NOT FETCH AT ALL: the sync constructor never touches the
network, so every assertion was true by construction and the gate passed WITH THE FIX DISABLED. Moved
to `Page::load_async` — the path the shell and renderer actually take — and the same probe then goes
red. Three vacuous first drafts in one session is not bad luck; it is the cost of writing the assertion
AFTER believing the fix.

RED-PROVEN, TWO MECHANISMS:
  · let error bodies be content again     -> RED, the 404 page's HTML is back in the document
  · refuse the body but drop `src` anyway -> RED, the "nothing to run" marker is lost, so the node
    stops looking external and the pipeline would try to run an empty script
REGRESSION: g_error_document (t607's own gate) still green — the navigation rule is untouched. Also
green: both ESM graph gates, g_csp, g_dedup, g_first_paint, g_load_budget, g_conn_cap.

⚠⚠ HONESTY, AND IT CUTS AGAINST THIS TICK'S OWN ORIGIN STORY: THIS DOES NOT FIX WELT.DE, AND WELT.DE IS
NOT AN INSTANCE OF IT. After the fix welt.de reports the SAME module SyntaxError, and `subresource_text`
logged NO rejection — nothing it fetched answered 4xx at all. welt.de's module URL therefore returns 200
WITH AN HTML BODY, a different cause: either our specifier resolution produces a document URL, or the
origin serves an SPA fallback for the module path. The bug I FOUND is real, gated and RED-proven; the
bug I WENT LOOKING FOR is still open. Recording that plainly matters more than the fix does — a
chain-peeling arc that lets "I was investigating X" become "I fixed X" is how a sunk-cost march starts,
and this is the fourth tick in a row on the same site.

Gate: G_ERROR_SUBRESOURCE (engine/page/tests/g_error_subresource.rs) — a local origin 404s a script, a
stylesheet and a module; the error body appears nowhere in the document, the script node keeps `src` so
the pipeline still reads "nothing to run", and the page itself still renders.
WIKI: docs/wiki/networking.md — "a 404 page is a document to render, not code to execute".
PATTERN: docs/loop/WEB-PATTERNS.md — a subresource that 404s to an HTML error page.

NEXT: welt.de's module URL answers 200-with-HTML — find WHICH url, and whether we resolved it or the
origin served it. One tracing line away. Standing for the observer since t611: manuk-wpt's tests,
including G_CERT_FALSIFIABLE, are not in the wall.
