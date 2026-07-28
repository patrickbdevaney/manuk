instrument(fidelity): three "render-failed" sites were a bot wall wearing HTTP 200

t708 owed a byte-level diff of our fetch against another client's.

FIRST, THE INSTRUMENT — BECAUSE t708 HAD NONE. t708 concluded "the origin serves our engine a
stripped document" from <head> CHILD COUNTS: 9 live vs 49 via curl. That comparison is not sound and
I should not have shipped it — the live number is read AFTER scripts run and AFTER our CSS loader
consumes the <link> elements (t707 established that), so a different document and a document
rewritten by its own scripts look identical through it. Added `boxes --dump-html FILE`: the bytes our
net stack received, before any parser or script. Same lesson the bad comparison broke — a stand-in for
the subsystem under test IS the instrument, and curl was never our net stack.

THE ANSWER (t708's conclusion survives its evidence):

                    OUR net stack        another client (curl)
  serverfault.com   5,491 B  1 script    205,345 B  43 scripts  9 <link>
  <title>           "Just a moment..."   "Newest Questions - Server Fault"
  body contains     challenges.cloudflare.com

It is a Cloudflare bot challenge and it arrives as 200 OK. Every bot-wall rule keys off a STATUS —
401/403/429, or a 5xx carrying a challenge marker — so a 200 interstitial fell through to
"measurable", painted its near-empty spinner, and was booked render-failed: the one reason documented
as "our own bug rather than a property of the origin". It is the precise opposite. Headers were
eliminated first: our exact Accept / Accept-Language / Accept-Encoding set replayed through curl still
returns the full 205KB, and we decode gzip/br/deflate.

render-failed HELD TWO POPULATIONS AND ONLY ONE IS OURS:

  serverfault · askubuntu · mathoverflow    5,489-5,492 B "Just a moment..."   <- NOT ours
  theverge 928KB · vox 963KB · mongodb 956KB · kotlinlang 250KB · notion 417KB <- OURS, real

This also closes t707's loose end: superuser.com scored render-failed in the sweep and ok on re-run
because it is sometimes served the challenge and sometimes the page. The intermittency was never in
our renderer.

classify_fetch now books a 2xx carrying an INFRASTRUCTURE challenge marker as BotWall(200). The 2xx
arm tests challenges.cloudflare.com / cf-browser-verification ONLY — never the prose markers "Just a
moment" / "Attention Required", which are English sentences a real page may contain. Mislabelling a
genuine render failure as a bot wall would EXCUSE our own bug, the expensive direction of this error.
The 5xx path keeps the wider list.

Gates: bot_challenge_tests 3/3 — mechanism, over-correction guard, 5xx path — RED-proven by dropping
the 2xx arm.

WIKI: none [forced] — a change to the fidelity instrument's classifier, not to the browser. Nothing
new is true of the engine; what changed is who gets billed for a page it never received. [no-pattern]
