measure(js): dynamic import() — the hook installs, every step succeeds, and the promise still rejects

TICK SHAPE: measurement / negative result. Bar 0 untouched; no ratchet floor moved; NO ENGINE SOURCE
CHANGED — the tree is HEAD plus this journal entry.

t618's audit found the map claiming `works` for `ES modules + dynamic import()` when import() was not
implemented at all. This tick went at it, got most of the way, and IS PARKED WITH A CLEAN TREE. Nothing
half-built is committed. The entry records the state reached so the next attempt starts from a narrowed
problem instead of from scratch.

WHAT IS NOW KNOWN, AND IT IS MOST OF THE WORK:

 · SetModuleDynamicImportHook / FinishDynamicModuleImport ARE available in the generated jsapi bindings
   (NOT in the published .rs — a source grep says otherwise; they are in target/*/out/build/jsapi.rs).
   Installing a hook removes the "Dynamic module import is disabled or not supported" rejection entirely.
 · The hook can reuse module_resolve_hook's specifier+base extraction VERBATIM, which it must: a dynamic
   import resolving differently from a static one would make import('./a.js') and import './a.js' name
   different modules from the same file.
 · A SYNCHRONOUS hook is the right shape. The spec's hook starts an async operation, but we have no
   synchronous network on the JS thread, so the module must already be in hand — and the page already
   pre-fetches the reachable graph. Extending scan_static_import_specifiers to also collect LITERAL
   import("…") specifiers puts them in that map (it deliberately skipped `import(`, correct while
   nothing could execute one). A computed specifier still cannot be seen by a textual scan and must
   reject, which the page's own .catch() already handles.
 · Instrumented, every step of the happy path SUCCEEDS:

     DYNIMPORT resolved=Some("http://…/app/chunk.js")  keys=[…/app/chunk.js, …]
     DYNIMPORT compiled=true linked=true evaluated=true is_obj=true in_registry=true pending_exc=false
     …and the page still sees   rejected:undefined

 · The REJECTION path already works correctly: an un-prefetched specifier rejects rather than hanging,
   which is the outcome that matters most — a promise that never settles is strictly worse than one
   that rejects.
 · Passing a NULL evaluation promise instead makes the caller's promise never settle at all, so handing
   ModuleEvaluate's promise to Finish is the correct half; the defect is elsewhere.

WHAT IS LEFT is one narrow question: why does FinishDynamicModuleImport reject when the module is
linked, evaluated, registered and exception-free? Suspects in order: the module may need to be in
SpiderMonkey's OWN module map rather than only ours; ModuleEvaluate's promise may already be settled in
a state Finish reads differently; or referencingPrivate must be the exact value the engine passed rather
than a re-rooted copy.

WHY PARKED RATHER THAN LANDED. ATOMICITY: a subsystem that does not work does not get committed, and the
scanner half is not independently useful — without a working hook it would fetch modules nothing can
import, which is pure waste. Reverted to HEAD; g_esm_page_graph re-verified green. The cost of this tick
is wall-clock; the output is that the next attempt starts at "why does Finish reject" instead of "how do
dynamic imports work here".

Gates: none — a gate for a capability that does not work would be a gate that cannot pass.
WIKI: none [forced] — nothing landed to describe.
PATTERN: [no-pattern] — no browser capability changed.

NEXT: the one question above; everything else needed is written down and reproducible in ~20 minutes.
Cheaper and still open: `document.fonts` is undefined (welt.de's remaining error), and t563's Lora
finding on an external sheet.
