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

#[test]
fn esm_import_graph_links_and_binds() {
    assert!(
        manuk_js::esm_import_graph_selftest(),
        "a two-module ES import graph must link, evaluate, and let the root module see a binding \
         imported across a relative specifier — proving `module_resolve_hook` resolves the specifier \
         against the importer's own url and returns the registered module. Red means the resolve hook \
         returned null (import graphs still fail at ModuleLink) or resolved the specifier against the \
         wrong base — the exact gap B2 exists to close."
    );
}
