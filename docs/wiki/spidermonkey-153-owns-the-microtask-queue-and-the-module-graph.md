# SpiderMonkey 153 owns the microtask queue and walks the module graph

*W6.J3.a. mozjs 0.15.20 (SpiderMonkey 140.17-lts, EOL 2026-09-29) became mozjs 0.26.4 (153.3.0-1).*

The bump is a port of the binding layer in `engine/js`, not an engine patch (I2 intact). Three
mechanisms changed shape; everything else was renames (`String::safe_from_jsval` ->
`from_jsval`, `to_jsval(&mut JSContext)`, `InstantiateOptions.eagerBaselineStrategy_`).

## Promise jobs: the engine holds the queue

140: the embedder implemented `enqueuePromiseJob` and parked each job in a rooted JS array
(`job_queue.rs`, with a one-slot fast path). 153: promise reactions are enqueued **inside the
engine** (`js/public/friend/MicroTask.h`); the embedder supplies only `runJobs`, which
`DequeueNextRegularMicroTask` -> `RunJSMicroTask` in the task's own realm until empty, clearing a
thrown exception so one bad job does not abort the checkpoint. The array, the slot, the pending
counter and the interrupt-queue traps are gone. `PromiseJobQueue::pending(cx)` now asks the engine
(`GetRegularMicroTaskCount`).

## Modules: one asynchronous load hook

140 had a synchronous resolve hook (return the registered module) plus a separate dynamic-import
hook. 153 has `SetModuleLoadHook`: called per request, and the embedder MUST answer with
`FinishLoadingImportedModule(referrer, request, payload, module, usePromise)` (or a `...Failed`).
`module_load_hook` resolves the specifier against the referrer's private url (`JS_GetScriptPrivate`),
takes the module from the registry or fetch+compiles it, then finishes. A `payload` that is a promise
is a dynamic `import()`: the static closure is loaded first, then `usePromise=false` links, evaluates
and settles the caller's promise synchronously.

The engine now walks the graph itself: `esm_load_graph` is a thin wrapper over
`LoadRequestedModules` (cycle-safe by the engine's own memoization). **A module must go through
`LoadRequestedModules` before `ModuleLink`**, even a leaf, or its status never leaves `New`.
The metadata hook receives the module record, not its private, so `import.meta.url` reads
`JS_GetModulePrivate`.

## What did not move

WPT JS areas, same-binary control on one tree (140 vs 153, release): dom 8080-8081/10506 on both,
html/dom 56461/59844 on both, domparsing 342/1294, url 1/7, encoding 133/767003 on both. The
committed `WPT-AREAS.tsv` baseline is older than HEAD for `dom` and `encoding` (8175 / 360564): that
drift is pre-existing and identical under 140, so it is not this bump.

Deployment: `scripts/mozjs-fetch.sh` pins the 153.3.0-1 prebuilt for linux x86_64, macOS arm64 and
windows msvc. After changing the pin, `cargo clean -p mozjs_sys -p mozjs`: the build script does not
rerun when only the archive's contents change.
