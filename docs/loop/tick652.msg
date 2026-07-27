capability(scripts): we ran the injected script and never told the page

t651 left agoda diagnosed but unfixed — a webpack chunk load ends in ChunkLoadError. It does not
fail. The chunk fetches and executes perfectly, and we fire NOTHING: no `load`, no `error`, on the
script element or anywhere else. Probed with the answer forced into a box width — ran 200px (the
chunk DID execute), onload 10px, onerr 10px on a 404.

There was no symptom to find: no error, no log, no failing gate, because the script itself had run
flawlessly. That event IS the contract of a script loader. webpack's is literally
`script.onerror = script.onload = onScriptComplete` plus a timer; with neither event it falls through
to the timer and rejects. The class is far wider than bundlers — every loadScript() helper resolves
its promise here: analytics, tag managers, ad tags, reCAPTCHA, payment and map SDKs. A loader with no
completion signal does not fail loudly. It waits.

TWO defects, and the second is the instructive one. Dispatching the event fixed only half, and the
wrong half: addEventListener('load') began firing while `script.onload = ...` still did nothing.
__dispatchEvent — the ONE element dispatch point — walked __listeners and never invoked the
`on<type>` PROPERTY handler, so the listener form worked and the property form did not, which is
exactly backwards from what real loaders use. This file already states the rule in __xhrFire: "ONE
dispatch point, so the `on*` handler and the listener list can never drift apart." XHR had been fixed
that way; the element path was still drifted.

The double-fire risk was MEASURED, not argued: `typeof b.onclick` is NOT a function for
<button onclick=...> and the inline handler fires exactly once, because inline handlers compile onto
a marked expando and register via addEventListener. The gate asserts once:1 rather than trusting it.
`load` fires even when the script THREW — the event reports that fetching and running happened, not
that the code succeeded; gating it on success would strand every loader whose chunk has a benign
error.

⚠ AND IT DID NOT FIX AGODA, which is the point of saying so. The events now fire, but TOO LATE: our
dynamic-script phase runs inside finish_loading, after the page's own event loop has spun to its
20,000-task ceiling and webpack's timeout has already fired. A real browser fetches an injected
script CONCURRENTLY with the event loop. That is a scheduling defect and a different tick. A fix that
is real and insufficient is worth exactly its own size, and no corpus claim is made here.

Gates: G_SCRIPT_LOAD_EVENT (engine/page/tests/g_script_load_event.rs) — IN THE WALL. RED-proven
TWICE, once per defect: disabling the property invocation leaves only `ael-load:true` (the exact
pre-fix asymmetry); disabling the dispatch leaves #out at `-` (the exact pre-fix silence). Hermetic,
served from a local socket, so it cannot false-RED on the network.

WIKI: docs/wiki/frameworks.md — "we ran the script and never told the page".
