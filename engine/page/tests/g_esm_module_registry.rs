//! **G_ESM_MODULE_REGISTRY — the ES-module rooting harness survives a compacting GC (B1).**
//!
//! The import-graph loader (the multi-module `import {x} from './y.js'` resolve, cell 169) is a
//! bounded subsystem whose one sharp edge is GC rooting: the specifier→module cache must hold
//! `*mut JSObject` module handles alive AND *relocatable* across a compacting GC — the exact "raw
//! pointer outlives a GC" class this repo keeps flagging, and one that fails as a use-after-free
//! crash, not a wrong answer. Brick B1 lands that cache's rooting primitive ALONE and proves it here
//! before any import resolution (B2) depends on it.
//!
//! `esm_registry_gc_selftest()` compiles a trivial self-contained module, stashes a url as its
//! SpiderMonkey private, registers it in the module registry, drops every stack root, forces a full
//! non-incremental GC, then reads the module back *through the registry* and asserts its private
//! still round-trips the url. If the registry value type were a bare `*mut JSObject` (or a `Heap`
//! not registered with mozjs's `RootedTraceableSet`), the module would be collected or left dangling
//! across that GC and this would go red — which is precisely the failure it exists to forbid.
//!
//! ONE `#[test]` per SpiderMonkey-booting gate binary (multiple runtime inits in one process segv).

#[test]
fn esm_module_registry_survives_gc() {
    assert!(
        manuk_js::esm_registry_gc_selftest(),
        "a module held ONLY by the ES-module registry must survive a full compacting GC with its \
         private (its resolved url) intact — the RootedTraceableBox<Heap<*mut JSObject>> rooting the \
         import graph (B2) depends on. Red here means a registered module was collected or left \
         dangling by a moving GC: a UAF waiting for the resolve hook, not a wrong answer."
    );
}
