capability(net): `XMLHttpRequest` had the legacy half of its API and none of the EventTarget half

TICK SHAPE: capability. Bar 0 untouched; no ratchet floor moved; no capability traded.

t612 left a named residue: welt.de's adblock check advanced one rung to `TypeError: a.addEventListener
is not a function`. This tick asked WHICH OBJECT — and the answer is worth far more than the site.

FIRST, A CORRECTION TO t612'S OWN RESIDUE LIST, BECAUSE IT WAS WRONG. t612 named nodeValue, style,
classList, document.body and selectionDirection as "spec-settable and still throwing here", derived by
GREPPING `prop_guarded!(prop, ..., None)`. Probing instead:

  nodeValue  ok (and the Attr half)   data/className/textContent/innerHTML/value/checked ok
  style THROWS   classList THROWS   document.body THROWS   selectionDirection THROWS

nodeValue was ALREADY WRITABLE, from a setter on another prototype the grep never saw — an artifact of
the instrument, not a property of the engine. The same shape as the stale `[id]` message t611 deleted,
made by me one tick later. A GREP OVER A REGISTRATION MACRO CANNOT ANSWER "IS THIS WRITABLE"; ONLY A
WRITE CAN.

AND THE USAGE SCAN KILLED THREE OF THE FOUR SURVIVORS. Across the HEAD corpus (HTML + up to 12 bundles
per site): `.classList =` 0 sites, `document.body =` 0 sites, `.selectionDirection =` 0 sites.
`.style =` looked alive at 15 hits/4 sites until I read the matches — all six sampled were React PROPS
OBJECTS (`R.style=d({},t.style,{},O.style)`), not element.style. The remaining getter-only class has NO
demonstrated usage; building it off the spec would have been building for nobody.

THE REAL FIND, from probing addEventListener across object types:

  window/document/element/text/fragment/img/audio/video/form/iframe ... all present
  xhr ✗    performance ✗    screen.orientation ✗

XMLHttpRequest IS NOT AN EventTarget. addEventListener/removeEventListener/dispatchEvent are all
undefined, `xhr instanceof EventTarget` is false, the chain is `? > Object`. It has the LEGACY half
(onload/onerror/onreadystatechange/onabort/onloadend) and none of the modern half. Calling an undefined
method is a TypeError that KILLS THE CALLING FRAME — the request is never even sent.

MEASURED, and it is not a corner:

  use `new XMLHttpRequest`               8 of 16 sites   (50%)
  addEventListener within 500ch of one   4 of 16 sites   (25%)
  XHR-specific listener names            readystatechange 9 · progress 4 · loadend 3 · timeout 2

`readystatechange` and `loadend` are names essentially nothing else fires.

AND THE SIX OPEN-CODED DISPATCH SITES HAD ALREADY DRIFTED — the session's signature defect, a fifth
time. The streaming path fired `loadend`; the buffered `__deliverXhr` did NOT. So whether onloadend ran
was a function of WHETHER THE RESPONSE HAPPENED TO ARRIVE IN CHUNKS — a spec event whose delivery
depended on transport framing. Everything now routes through one `__xhrFire`, so there is no longer a
place to fire an event from. `onprogress` had the mirror bug: FIRED by the streaming path, never
DECLARED, so `'onprogress' in xhr` answered NO about an event we actually deliver.

RED-PROVEN THREE WAYS, EACH A DIFFERENT MECHANISM:
  · remove addEventListener again           -> RED, welt.de's exact TypeError
  · stop the BUFFERED path firing loadend   -> RED `once:0` — exactly the drift found above, and it
    fails while the streaming half stays GREEN, which is why both paths are gated
  · drop the duplicate-registration rule    -> RED `dupes:2` — an EventTarget degraded to an array

⚠ ONE #[test] PER JS GATE, re-learned at the cost of a run: two #[test]s in one file SIGSEGV (PageContext
is per-PROCESS). Splitting it was the right shape anyway — one file per delivery path.

HONESTY: welt.de STILL DOES NOT RENDER. Two rungs cleared, the third visible:
  t612 before: adblock: TypeError: setting getter-only property "innerText"
  t613 before: adblock: TypeError: a.addEventListener is not a function
  t613 after:  adblock: Error: Failed to execute packing script
Still open: HTMLScriptElement.supports, and a module request answered with HTML. What this tick claims
is the XHR EventTarget surface at 8-of-16 / 4-of-16 sites — not the site.

Gates: +2 G_XHR_EVENTTARGET (engine/page/tests/g_xhr_eventtarget.rs buffered ·
g_xhr_eventtarget_stream.rs streaming) — surface exists, on* and listeners BOTH run in order, duplicate
registration collapses, `once` removes before invoking, a removed listener stays removed, a throwing
listener does not block the rest, the event object is real, and both delivery paths fire the same set.
WIKI: docs/wiki/networking.md — "XMLHttpRequest had the legacy half of its API and none of the
EventTarget half".
PATTERN: docs/loop/WEB-PATTERNS.md — "AJAX set up the modern way: xhr.addEventListener, not
xhr.onload". (I first wrote [no-pattern] here, reasoning it was a second instance of t612's entry. The
hook refused, and the hook was right: t612's pattern is about a site DETECTING a broken DOM and
blanking itself, while this is a whole class of AJAX code that cannot run at all. A dual-form API
shipped in its legacy form only is not partial support — it is a TypeError on the form the ecosystem
converged on.)

NEXT: HTMLScriptElement.supports is the next rung and is trivial. performance and screen.orientation
are also not EventTargets but showed ZERO corpus usage — do not build them until they do. The
module-answered-with-HTML error is more interesting: a `<script type=module>` URL resolved to a document.
