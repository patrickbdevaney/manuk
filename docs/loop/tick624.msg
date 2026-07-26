capability(js): dynamic import() works — and the thing that parked it was a LIFETIME, not the hook

TICK SHAPE: capability. Bar 0 untouched; no ratchet floor moved; no capability traded — the registry's
navigation-scoped lifetime is unchanged, only the MOMENT it is dropped.

t618's audit found the map claiming `works` for import() when it was not implemented; t620 built the
hook, got every step succeeding, and parked with the promise still rejecting. The parked question is
answered and the capability is in.

THE ANSWER WAS NOT THE HOOK — IT WAS WHEN THE MODULE STOPPED EXISTING. `run_module` cleared the compiled
-module registry the instant the ROOT was linked and evaluated, on reasoning that is correct FOR STATIC
IMPORTS: SpiderMonkey's own module records keep a linked graph alive, so the registry's roots stop being
load-bearing. FALSE FOR DYNAMIC ONES. FinishDynamicModuleImport completes the caller's promise in a
LATER MICROTASK, and the module must still be returned by the resolve hook at that point — which the
API's own header says, in a sentence I had read as being about registration rather than about LIFETIME:

  "If successful, AFTER CALLING FinishDynamicModuleImport() the module should be returned by the
   resolve hook when passed |referencingPrivate| and |moduleRequest|."

So the module was compiled, linked, evaluated, registered, exception-free — and deleted between the hook
returning and SpiderMonkey asking for it. The clear now happens at the end of the SCRIPT PASS, beside
where MODULE_GRAPH_SOURCES is dropped, preserving the contract that actually matters (the registry never
outlives the NAVIGATION, so nothing pins a dead realm's modules) and simply being later than the last
moment a dynamic import can need it.

AND ONE FAILURE ON THE WAY BACK IN, WORTH RECORDING BECAUSE IT COST A FULL WRONG ANSWER. Re-applying
t620's hook I installed it at ONE of the two SetModuleResolveHook sites. The page path uses the SECOND.
The first run reported "Dynamic module import is disabled" — the PRE-HOOK symptom — and I nearly
concluded the re-application had failed. A HOOK INSTALLED AT ONE OF TWO SITES LOOKS EXACTLY LIKE A HOOK
NOT INSTALLED AT ALL. §VI.3's fourth clause from the other direction: not "two implementations of one
rule disagree" but "one implementation registered in one of two places". The gate's assertion message
now names this, because the symptom is indistinguishable from the original bug.

RED-PROVEN THREE WAYS, each reproducing a DIFFERENT real symptom rather than three flavours of red:
  remove the hook          -> "Dynamic module import is disabled or not supported in this context"
  restore the early clear  -> "rejected:undefined"       <- t620's exact parked symptom
  revert the scanner       -> the promise never settles  <- the miss case
The un-prefetched specifier REJECTS rather than hanging, asserted in its own right: a computed specifier
cannot be seen by a textual pre-scan, and a promise that never settles is strictly worse than one that
rejects — the page's .catch() is what such a page already relies on.

MEASURED on www.welt.de, which reaches import():
                before (t617)      after
  VISUAL         82.8%             91.1%
  COVERAGE       94.9%             95.7%   (163 -> 136 missing)
  SHAPE          66.9%             67.2%
Modest against t617's 0.0->94.9, and real. The CLASS is larger than the number: every code-splitting
bundle — a lazy route, an on-demand component, a deferred polyfill — is import().

REGRESSION: all six ESM gates green (g_esm_page_graph, g_esm_prefetched_graph, g_esm_import_map,
g_esm_import_graph, g_esm_module_registry, g_module_base_url), plus g_defer, g_globals and the whole
manuk-js suite. Both feature configurations compile.

Gate: G_DYNAMIC_IMPORT (engine/page/tests/g_dynamic_import.rs) — resolution AND honest rejection, with
the assertion messages naming all three failure symptoms by name.
WIKI: docs/wiki/js-engine.md — "a dynamic import's module must outlive the hook that made it".
PATTERN: docs/loop/WEB-PATTERNS.md — the code-split bundle.

NEXT: document.fonts is undefined (1 of 16 measured sites, welt.de's last named error). Observer items
unchanged: manuk-wpt tests not in the wall; map-reconcile scanning only engine/page/tests/.
