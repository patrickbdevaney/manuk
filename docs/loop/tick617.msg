capability(js): a module's imports resolve against the MODULE's url — welt.de goes blank -> rendered

TICK SHAPE: capability. Bar 0 untouched; no ratchet floor moved; no capability traded.

THE ARC CLOSES, AND THE NUMBER IS THE HEADLINE:

                    before          after
  COVERAGE          0.0%            94.9%   (3181 of 3182 missing -> 163 of 3226)
  SHAPE             0.0%  (n=1)     66.9%   (n=3063)
  VISUAL           17.1%            82.8%
  verdict          BELOW            ok

www.welt.de — a blank white page for four ticks — now renders its front page: navigation, hero image,
headlines, body copy.

THE BUG, AND IT IS ONE LINE OF REASONING. `fetch_external_scripts` inlines a <script src> into the node
and drops `src`. By the time `prefetch_module_graph` walks the DOM, an external module is
INDISTINGUISHABLE FROM AN INLINE ONE — and the walk resolved its imports against the document URL,
behind a comment that is TRUE: "the document URL, since an inline module resolves its relative imports
against the document". True, and applied to a case it does not cover. A MODULE'S IMPORTS RESOLVE
AGAINST THE MODULE'S URL. For an inline module that happens to be the document; for an external one it
is not. MEASURED:

  welt.de entry: /assets/bff-section/scripts/section.module.BPEBKMaY.js  imports "./chunks/react.BPdhuoKc.js"
    against the SCRIPT    /assets/bff-section/scripts/chunks/react.BPdhuoKc.js  200 ·   8,391 B · JavaScript
    against the DOCUMENT  /chunks/react.BPdhuoKc.js                             404 · 414,112 B · HTML

One directory tree too high, an SPA fallback answering with HTML, and `SyntaxError: expected
expression, got '<'`. THIS IS THE SHAPE OF EVERY VITE/ROLLUP/ESBUILD PRODUCTION BUILD — an entry module
under a hashed asset directory importing its chunks relatively — so the class is "sites that ship
modern bundled JavaScript", not one newspaper.

AND IT WAS IN TWO PLACES, THE SESSION'S SIGNATURE DEFECT FOR THE NINTH TICK RUNNING. Fixing the Rust
pre-fetch alone did NOTHING — the gate still failed. The JS side sets the root module's SpiderMonkey
private from DOC_URL unconditionally, and its comment ALSO states the rule correctly: "A root inline
<script type=module>'s base IS the document url; a FETCHED module's base is its own url — which is
exactly why the answer must live on the module, not in one per-document slot." TWO COMMENTS, IN TWO
CRATES, BOTH STATING THE CORRECT RULE, BOTH ABOVE CODE THAT DOES THE OPPOSITE. Each half is
independently RED-proven, and either one alone leaves the site blank.

⚠ THE GATE WOULD HAVE BEEN VACUOUS IN THE OTHER DIRECTION, and a diagnostic caught it. When it failed
after the Rust fix I did not assume the fix was wrong — I moved the DOCUMENT into the script's
directory, where both resolutions agree. It passed. That proved the harness could pass at all, so the
remaining failure was real rather than a broken test. A GATE THAT HAS NEVER BEEN SEEN TO PASS IS NOT
EVIDENCE THAT THE CODE IS BROKEN.

⚠⚠ `--features stylo,spidermonkey` HID A COMPILE ERROR, exactly as the standing memory says: the
shipping config built clean while `--no-default-features` did not (`script_origins` is cfg-gated but
the `Prefetched` field is unconditional). An earlier edit also split a `#[cfg]` from its item by
anchoring on a doc comment — the second half of the same hazard, in the same tick. Both configs now
compile.

RED-PROVEN, TWO INDEPENDENT MECHANISMS (either alone reproduces the blank page):
  · Rust pre-fetch resolves against the document again  -> RED `got: "-"`
  · JS root private goes back to DOC_URL                -> RED `got: "-"`
REGRESSION: both ESM graph gates, g_esm_import_map, g_esm_import_graph, g_error_subresource, g_defer,
g_globals and the whole manuk-js suite green.

HONESTY ON WHAT IS STILL BROKEN, because "renders" is not "works": welt.de's remaining errors are now
LATER AND DIFFERENT — `document.fonts is undefined` and `Dynamic module import is disabled or not
supported in this context` (an `import()` expression, which t512-516 explicitly left as residue). It
also takes 31.7s against Chromium's 8.0s, so it trips OURS-IS-SLOW and the shape number is partly a
TIMING result. 94.9% coverage is a real measurement of a real render; it is not a claim that the site is
finished.

And the arc's accounting, now that it has one: five ticks (612/613/615/616/617), five genuine capability
gaps, each justified by its own measured population — and the site moved on the fifth. Four ticks of
"still blank" were not wasted, but they were also not the win, and t616's entry says so in those words.
Naming the four honestly is what made the fifth believable.

Gate: G_MODULE_BASE_URL (engine/page/tests/g_module_base_url.rs) — a local origin serves the dependency
ONLY at the script-relative path and 404s the document-relative one with HTML, so nothing but correct
resolution can make it pass.
WIKI: docs/wiki/js-engine.md — "a module's imports resolve against the MODULE's url".
PATTERN: docs/loop/WEB-PATTERNS.md — the bundled-SPA entry module under a hashed asset directory.

NEXT: `import()` — dynamic module import is genuinely unimplemented and welt.de now REACHES it, as does
every code-splitting bundler. That is the honest next rung and it is a real subsystem, not a member.
