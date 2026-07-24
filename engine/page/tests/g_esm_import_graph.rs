//! **G_ESM_IMPORT_GRAPH — a multi-module `import` graph links, evaluates, and binds across files (B2).**
//!
//! B1 landed the GC-safe module registry (`g_esm_module_registry`); B2 lands the resolve hook that
//! consumes it. `module_resolve_hook` is now the SYNC half of the import-graph loader: SpiderMonkey
//! calls it once per `import` during `ModuleLink`, and it resolves the specifier against the
//! *referencing* module's own url (so `./b.js` resolves against the importer, not the document) and
//! returns the registered module for that url. Populating the registry from a fetched graph is the
//! async pre-fetch pass (B3) — mirroring how `importScripts` consumes pre-fetched worker sources,
//! because this hook cannot go to the network. What B2 proves here is that a *populated* graph
//! actually links and evaluates with cross-module bindings through SpiderMonkey's own graph walk.
//!
//! `esm_import_graph_selftest()` seeds a dependency module (`export const v = 41;`) into the registry
//! under its resolved url, compiles a root module that does `import { v } from './esm-graph-dep.js'`
//! against its own private url, links the root (the resolve hook resolves the relative specifier →
//! the dep's url → the seeded module) and evaluates it. The root writes `v + 1` to a global; a `42`
//! read back proves the imported binding resolved end-to-end.
//!
//! RED-provable: revert `module_resolve_hook` to return null → the resolve fails → `ModuleLink` fails
//! → the global is never written → red. Also red if the base url is threaded from the document slot
//! instead of the module's private → the relative specifier resolves to the wrong url → registry miss.
//!
//! ONE `#[test]` per SpiderMonkey-booting gate binary (multiple runtime inits in one process segv).

/// ONE `#[test]` per SpiderMonkey-booting gate binary — two would boot a runtime on two test threads
/// and segv. B2 (a pre-seeded graph links) and B3 (the loader POPULATES the graph itself off a root's
/// `import`s, cycle-safe) share this test, run sequentially on one thread (they reuse the parked
/// per-thread runtime), each asserted independently so a failure names which half went red.
#[test]
fn esm_import_graph_links_binds_and_populates() {
    // B2 — a pre-seeded two-module graph links, evaluates, and the root sees the imported binding,
    // proving `module_resolve_hook` resolves a relative specifier against the importer's own url.
    assert!(
        manuk_js::esm_import_graph_selftest(),
        "a two-module ES import graph must link, evaluate, and let the root module see a binding \
         imported across a relative specifier — proving `module_resolve_hook` resolves the specifier \
         against the importer's own url and returns the registered module. Red means the resolve hook \
         returned null (import graphs still fail at ModuleLink) or resolved the specifier against the \
         wrong base — the exact gap B2 exists to close."
    );

    // B3 — the graph-POPULATION walk discovers + fetches + registers a three-module graph off the
    // root's `import`s, terminating an `a ↔ b` cycle by insert-before-recurse.
    assert!(
        manuk_js::esm_graph_load_selftest(),
        "a three-module ES import graph with an a↔b cycle must be fetched + compiled + registered by \
         the population walk, then link, evaluate, and let the root see a binding computed across the \
         whole graph (total = 41). Red means esm_load_graph did not walk the root's imports to populate \
         the registry, or the cycle back-edge re-fetched instead of hitting the registry — the exact \
         gap B3 exists to close before the async page path (B3b) can wire the real fetcher."
    );

    // B3b — the REAL page module runner (`run_module`, the function `run_scripts` calls for every
    // `<script type=module>`) consumes the pre-fetched source map the async page pass seeds, drives the
    // population walk over it, links + evaluates a relative import, and clears the registry per-root.
    assert!(
        manuk_js::esm_page_module_graph_selftest(),
        "the page-path module runner must consume the pre-fetched module-graph source map: given a \
         seeded dependency (`export const answer = 7;`) and a root inline module that imports `answer` \
         from './esm-page-dep.js', run_module must fetch-from-map + compile + \
         register the dep, link, evaluate, and let the imported binding reach a global (42 = 7*6). Red \
         means run_module never drove esm_load_graph over MODULE_GRAPH_SOURCES — so a real page's inline \
         module graph still dies at ModuleLink against an empty registry, the exact gap B3b closes."
    );
}
