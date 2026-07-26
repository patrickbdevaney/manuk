# JS ENGINE — SpiderMonkey/mozjs integration realities

## The window is a browsing-context TREE, and its self-references are load-bearing

At the top level the spec requires:

```
window.parent === window
window.top    === window
window.frames === window
window.self   === window
window.opener === null      // null, NOT undefined
```

**These are not niceties. The self-reference IS how a page knows it is the top.** The universal idiom
for walking to the top window is:

```js
var w = self;
while (w != w.parent) { w = w.parent; }
```

That loop terminates **because the top is its own parent.** With `parent` undefined it does not fail
to terminate — **it walks straight off the end**: `w` becomes `undefined`, and the next `w.parent`
throws a `TypeError`.

**We defined `window` and `self` and not the other four.** The result: `testharness.js` throws on its
*first* action (`_forEach_windows`), so **100% of Web Platform Tests failed before a single assertion
ran** — and the failure presented as *"our JS engine cannot run testharness.js"*, which is a far
scarier and far wronger diagnosis than *"we never defined `window.parent`."*

> **Generalisable:** when a spec defines an object as self-referential, that identity is usually
> load-bearing for a *termination condition* somewhere. Omitting it does not degrade behaviour, it
> creates an infinite walk or a null deref.

## An inert stub will silently DISABLE a real implementation if it installs first

We install a surface of ~70 **inert** named constructors so that referencing an unimplemented
interface is not a `ReferenceError` (a `ReferenceError` aborts the whole `<script>`; an inert object
that answers `false` is survivable — every library on the web is written to survive it, because real
browsers produce exactly that behind captive portals and in private windows).

The guard is `if (typeof globalThis[n] === 'undefined') { install stub }`. **Order decides what that
guard MEANS.** `AbortSignal` was in the inert list, and the list installed *before* the real
`AbortSignal` (defined a few hundred lines below, with a working listener array). By the time the real
one asked `typeof globalThis.AbortSignal === 'undefined'`, **its own stub was already sitting there.**
The real implementation never installed, and every `new AbortController().abort()` threw.

**The mechanism, and the fix:** install the inert surface **LAST**, so the `undefined` guard means what
it was always supposed to mean — *"fill in only what nobody actually implemented."* Ordering is now
the mechanism, so it cannot recur when someone later adds a real implementation for a name on the list.

> **And the gate could not see it.** `G_GLOBALS` asserts `typeof X !== 'undefined'`, which an inert
> stub satisfies **perfectly**. *Existence was never the property worth asserting — behaviour was.*

## A throwing task must NOT kill the event loop

Our macrotask runner called the callback bare, so an exception propagated out of the eval, the Rust
`?` aborted the loop, and **every task queued after the throwing one never ran.** One bad `setTimeout`
callback silently stopped the page's clock.

**The spec says: report the exception, then keep going.** The loop is not allowed to care. A real
browser fires `window.onerror` / an `error` event and takes the next task.

Errors are now collected in `globalThis.__errors` — which is also the storage the **unhandled-error
harvester** wants: a page's silent breakage becomes something that can be *read out* rather than
guessed at.

## `setTimeout`'s delay is not decoration, and a virtual clock must not outrun the lifecycle

**We threw the delay away.** Every timer was a bare FIFO push, so `setTimeout(f, 10000)` ran *before* a
`setTimeout(g, 0)` queued after it. Insertion order, not time order. That silently mis-orders every
debounce, throttle, retry-backoff and staged animation on the open web — **and none of it errors**, it
just happens in the wrong order, which is exactly the class of bug a box-diff against Chromium cannot
see.

The fix is a real timer queue ordered by `(due, seq)` over a **virtual clock**: time jumps forward to
whatever is due next, and we never actually sleep. A headless load must not take ten real seconds
because the page armed a ten-second timer; it must only run that timer **last**. *Ordering is the
property that matters; waiting is not.*

**But a virtual clock has a trap, and it is subtle:** while the page is still loading, the only task
left is often a *long* timer — so the clock leaps to it and fires it **before `load` ever happens.**
`testharness.js` arms a 10-second harness timeout at setup; our loop drained everything else, jumped
to 10s, fired the timeout, and testharness declared TIMEOUT — *and only then* did we fire `load`, into
a page that had already given up.

> **The rule: the virtual clock may not run ahead of the document's lifecycle.** During load the time
> budget is 0 — only tasks due *now* may run, which is what a real browser does anyway, since real time
> has barely advanced. **`load` opens the budget**, and the delayed timers then run in correct order
> *behind* the event they were always meant to follow.

## `JS::JobQueue` must be installed with `SetJobQueue`, not `UseInternalJobQueues`

`mozjs::rust::Runtime::create` calls `JS::InitSelfHostedCode` **unconditionally**, and SpiderMonkey
requires `js::UseInternalJobQueues(cx)` to be called **before** it (the promise machinery captures the
queue at that point). mozjs exposes no hook in between, so the call **always arrives too late** — a
newer mozjs cannot fix this; the wrapper signature was never wrong.

**The answer is the one browsers use:** do not use the internal queue at all. Provide an embedder
`JS::JobQueue` and install it with `SetJobQueue`, which has **no ordering constraint** — the same hook
Gecko and Servo use. No JIT/GC/sandbox is touched and SpiderMonkey needs no patch.

**Rooting hazard:** an enqueued job is a `JSObject*` that must survive until it runs. Rather than root
it Rust-side, push jobs onto a **JS array held by the global**, which the GC traces already.

## Missing `JSAutoRealm` compiles fine and SIGSEGVs at runtime

Raw jsapi per-interface work is realm/rooting-error-prone in a way the compiler cannot help with. Build
the thin safe binding-helper layer **once** (reflector creation, reserved-slot accessors, native-fn +
realm/rooting wrappers) rather than writing raw jsapi at each interface.

---
# Backfill — mechanisms recovered from ticks 1–42 (pre-wiki)

## `import.meta` needs an embedder module-metadata hook, and its absence killed every Vite app

SpiderMonkey will **not evaluate an ES module that touches `import.meta`** unless the embedder installs a
module-metadata hook; without it the module throws *"Module metadata hook not set"* **at its own top
level**, where a warning path that only watches *script* errors never sees it. **Vite, Rollup and esbuild
all emit `import.meta.url` unconditionally**, whether the app uses it or not — so one missing embedder
callback made **every bundler-produced app on the internet** fail silently, mounting an empty
`<div id="root">` and throwing zero visible exceptions.

**`import.meta.url` must be sourced PER-MODULE, not from the document (tick 512, ESM import-graph B1).**
The first cut of the metadata hook returned the one-per-document `DOC_URL` thread-local — correct only
because the sole module was the page's root inline `<script type=module>`, whose base *is* the document
URL. A real import graph breaks that: a fetched `./b.js` evaluating `import.meta.url` must get **`b.js`'s
own resolved URL**, not the top document's. The fix is the mechanism the spec already implies — stash each
module's base URL as its **SpiderMonkey private** at compile time (`SetModulePrivate`, via
`set_module_private_url`), and have the metadata hook read the `private_value` it is *handed* (the
referencing module's private) rather than a global. SpiderMonkey traces the private off the module object,
so a JS string parked there is rooted for the module's life; the hook keeps a `DOC_URL` fallback so a
private-less module never reports an empty URL. This is the per-module URL thread the resolve hook (B2)
extends: every fetched module gets the same private set to *its* resolved URL. RED-provable via the
`esm_registry_gc_selftest` round-trip — neuter `SetModulePrivate` and the private reads back `undefined`.

## A raw `*mut JSObject` cached across a GC boundary is a use-after-free, not an optimisation

`DOC_REFLECTOR` was a `Cell<*mut JSObject>` — an unrooted bare pointer into the JS heap. Nothing kept the
document alive or updated the pointer when the collector **moved** it, so after enough allocation
`document`/`ownerDocument` returned **whatever object now occupied that address**. In the failing React run
it returned one of *our own* `MutationRecord` objects — on which `createElement` is indeed not a function.
**React's error message was TRUE and pointed at nothing wrong with React.**

The correct discipline (keep the reflector in a JS-side structure the GC traces through the global) was
**already applied to every DOM node ten lines above**, and just not to the document.

> The regression test has to **allocate 60,000 objects to force a collection**. *A test that does not
> allocate cannot see this bug at all* — which is why it survived several ticks.

**The ES-module registry is where this exact hazard reappears — and B1's whole job was to solve it once
(tick 512).** The import-graph resolve hook (B2) must cache `specifier → module` and return the *same*
module object every time a specifier is re-requested (the spec requires it; it is also how cycles
`a→b→a` terminate). That cache holds `*mut JSObject` module handles across `ModuleLink` **and across a
compacting GC** — precisely the "raw pointer outlives a GC" trap above, except a compacting collector
also *moves* the handle, so even a still-live module is at a stale address. The correct value type,
proven in isolation before any resolution depends on it, is
**`RootedTraceableBox<Heap<*mut JSObject>>`**, and both halves are load-bearing: `Heap` carries the
store/post-write barrier a bare pointer lacks, and the `RootedTraceableBox` (a) heap-pins the `Heap` so a
`HashMap` rehash cannot invalidate its store-buffer slot, and (b) registers it with mozjs's
`RootedTraceableSet`, which the runtime's already-installed extra-GC-roots tracer marks **and relocates**
every collection. The `esm_registry_gc_selftest` gate registers a module, drops every stack root, forces
a full `JS_GC`, and reads it back through the registry with its private intact — a bare-pointer or
untraced-`Heap` registry goes red (collected or dangling) exactly there. Same lesson as `DOC_REFLECTOR`:
**keep the handle in a structure the GC traces; never a bare `Cell`/`HashMap` value.**

**The resolve hook is the SYNC half, and it must NOT fetch — because this engine has no synchronous
network (tick 513, ESM import-graph B2).** SpiderMonkey calls `module_resolve_hook` once per `import`
during `ModuleLink`, which runs synchronously on the JS thread. The obvious design — "fetch+compile the
module right here" — is wrong for the same reason `importScripts` is pre-scanned rather than fetched
on-demand: there is no blocking `fetch` in this engine (`manuk_net::fetch` is async; the page and workers
pre-fetch on the async side and the sync path consumes a source/module map). So B2's hook does exactly
three synchronous things and nothing else: read the specifier (`GetModuleRequestSpecifier`), take the
base URL from the **referencing module's private** (the per-module URL B1 threaded — so a relative
`./b.js` resolves against the *importer*, not the document; `DOC_URL` fallback), `url::Url::parse(base)
.join(specifier)`, and return `esm_registry_get(resolved)`. A miss or a bad URL returns null → a graceful
`ModuleLink` failure (the same shape as the old always-null hook, never a crash), which is also how a bare
specifier (`import 'react'`) fails loud-but-safe until a resolver exists. **This is what makes a
*populated* graph work:** the registry returning the SAME `*mut JSObject` per URL is the memoization
SpiderMonkey's own graph walk needs — cross-module bindings resolve and a cycle `a↔b` re-enters the
existing module record instead of looping. **Populating** the registry from a fetched graph is deliberately
NOT here; it is the async pre-fetch pass (B3), mirroring `importScripts`. Gate `g_esm_import_graph`
(`esm_import_graph_selftest`) seeds a dep module into the registry, compiles a root that imports it across
a relative specifier, links+evaluates, and asserts the imported binding reached the root
(`globalThis.__esm_graph_r === 42`); revert the hook to null and it goes red at `ModuleLink`. One catch
proven the same tick: the resolve+metadata hooks are per-**runtime**, installed by `run_scripts` on the
page path — a self-test that boots its own global must re-`SetModuleResolveHook`, or `ModuleLink` reports
*"Module resolve hook not set"* and never reaches the hook under test.

**B3 is the population WALK that fills the registry the resolve hook reads (tick 514, ESM import-graph
B3).** B2's hook is only ever as good as what is already in the registry; on an empty registry every
`import` resolves to null and the graph dies at `ModuleLink`. `esm_load_graph(cx, url, module, fetch,
depth)` is the loader that fills it: given a *compiled, registered* root, it walks
`GetRequestedModulesCount` / `GetRequestedModuleSpecifier`, resolves each specifier against **that
module's own url** (so `./b.js` is relative to the importer), and for each not-yet-registered dependency
fetches its source through an **injected `fetch` seam**, compiles it, stashes its private url, inserts it,
and recurses. Two invariants make it correct: (1) **insert BEFORE recurse** — the dependency is in the
registry before the walk descends into it, so a back-edge (`a.js`→`b.js`→`a.js`) is a registry hit and
the cycle terminates instead of re-fetching forever (a depth cap backstops the pathological case); (2)
**a miss is skipped, never fatal** — an unresolvable bare specifier or a fetch miss simply never lands,
and its importer fails gracefully at `ModuleLink` (loud-but-safe), exactly like the resolve hook.

The `fetch` seam is injected, not a direct `manuk_net` call, for a hard reason: `ModuleLink` is
synchronous on the JS thread, but the render path runs **inside** the tokio runtime, so a `block_on`
inside the link would panic *"cannot start a runtime from within a runtime"*. So the real page path (B3b)
must **pre-fetch the whole graph asynchronously** — the way `fetch_external_scripts` pre-fetches classic
scripts — and hand `esm_load_graph` a fetcher that reads from that pre-fetched map, mirroring how
`importScripts` consumes pre-fetched worker sources. Keeping `fetch` injectable is also what makes the
walk hermetically testable with zero network. Gate `g_esm_import_graph` (`esm_graph_load_selftest`, run
in the SAME `#[test]` as B2 — two SpiderMonkey-booting tests in one binary segv) loads a three-module
graph with an `a ↔ b` cycle through an in-memory fetcher, links+evaluates, and asserts a binding computed
across the whole graph reached the root (`total === 41`) plus that the cycle back-edge saw a function
export mid-cycle; neuter the walk and it goes red at `ModuleLink`. Only exported **function
declarations** cross the cycle in the fixture — they are hoisted and initialised during instantiation, so
reading them at another module's top level is safe mid-cycle where a `const` would hit the temporal dead
zone.

**B3b wires the population walk into the REAL page module runner, and clears the registry per-root
(tick 515, ESM import-graph B3b).** B3 proved `esm_load_graph` fills a registry off an in-memory fetcher;
B3b makes the actual page entry point use it. `run_module` — the function `run_scripts` calls for every
`<script type=module>` — now, after compiling the root and stashing its private url, reads the
per-document `MODULE_GRAPH_SOURCES` map (resolved-url → source, seeded by the async pre-fetch pass) and
drives `esm_load_graph` over it *before* `ModuleLink`, so every `import` the root reaches is already in
the registry when the resolve hook fires. An empty map is a no-op: a self-contained module links exactly
as it did before the loader existed. The pre-fetched sources cannot be fetched inside the link (the
`block_on`-in-`ModuleLink` panic above), so the async page pass fills the map first — B3b lands only the
*consumption* half; the producer (the async scanner + graph fetch) is B3b-ii.

The registry is cleared **at the end of each `run_module` call**, not on a page-teardown hook. Once
`ModuleLink` has run, SpiderMonkey's own module records keep the linked graph alive through the
still-rooted root object, so the registry's `RootedTraceableBox` roots are no longer load-bearing —
dropping them there means the registry never outlives the call. That is the B1 GC-safety contract (a
root must never outlive its realm) satisfied *by construction* rather than by a teardown hook a future
navigation path could forget to call; the only cost is that two roots sharing a dependency re-compile it,
which is correct and rare. Gate: `esm_page_module_graph_selftest` seeds one dependency
(`export const answer = 7;`) plus a root that imports it, runs the **real** `run_module`, and asserts the
imported binding reached a global (`42 === 7 * 6`); delete the `esm_load_graph` call from `run_module` and
only that assert goes red at `ModuleLink` (B2/B3, which drive the walk directly, still pass) — proving the
gate watches the page-path seam specifically.

**B3b-ii is the async PRODUCER that fills `MODULE_GRAPH_SOURCES` on the real page path (tick 516, ESM
import-graph B3b-ii).** B3b-i built the consumer; nothing filled the map, so a real inline
`<script type=module>` importing a relative graph still died at `ModuleLink`. The producer lives in
`manuk-page`'s `load_async`, right after `fetch_external_scripts` (which has already inlined any external
`type=module src`), and it does two things: `scan_static_import_specifiers` — a lightweight **textual**
pre-scan of each module root's source that pulls the specifier out of every static `import … from 'm'` /
`import 'm'` / `export … from 'm'` (skipping comments, string bodies, dynamic `import(` and
`import.meta`) — and `prefetch_module_graph`, a breadth-first walk that resolves each specifier against
its importer's URL with the **same `Url::join`** the resolve hook uses, fetches each not-yet-seen module
off the UI thread with `manuk_net::fetch`, scans *its* imports, and recurses (a visited set = the map's
keys is the diamond/cycle guard; a 512-node cap backstops adversarial graphs). The resolved-url → source
map is handed to `manuk_js::set_module_graph_sources` **immediately before** `run_deferred_scripts` (no
`.await` between, so both stay on the JS thread that reads the thread-local) and dropped with
`clear_module_graph_sources` the instant that pass returns — one document's graph can never resolve the
next's imports.

Two deliberate design choices. **The scanner is a superset-or-miss heuristic, not a parser**, and that is
sound because it only decides what to *fetch*: `esm_load_graph` remains the authoritative walk (it reads
SpiderMonkey's real `GetRequestedModuleSpecifier` after compiling), so an over-match fetches a URL nothing
imports (harmless) and a miss leaves that dependency out of the map, where its importer fails loud-but-safe
at `ModuleLink` — never a crash. **The producer resolution mirrors the loader's exactly** (`Url::join`
against the importer), so a key the pre-fetch stores is the key the resolve hook later looks up. Gate
`g_esm_page_graph` stands up a localhost origin serving a **two-level** graph (inline root →
`/esm-a.js` → `/esm-b.js`, the transitive dep proving recursion), loads it through the real
`Page::load_async`, and asserts the cross-graph binding `answer` (42) reached a DOM node; neuter
`prefetch_module_graph` to return an empty map and `#out` stays `-` (`ModuleLink` can't resolve
`./esm-a.js`). A unit test pins the scanner's edge cases (minified, comment/string/dynamic-import skip)
directly. B3b-ii wires the producer into the `load_async` path — the streaming/headless/agent render path
(`fetch_streaming_page` → `load_async`); B3b-iii (below) wires the interactive shell.

**B3b-iii wires the producer into the SHELL path and unifies the seed seam on a page field (tick 517, ESM
import-graph B3b-iii).** B3b-ii ran the pre-fetch only on `load_async`; the interactive shell navigates
through `prefetch_document` → `from_prefetched_blocking_only` → (paint) → `run_deferred_scripts`, an
off-thread DEBT-1 path that never saw the map — so a human browsing a native-ESM site in the window still
got nothing. B3b-iii closes it. `prepare_prefetched` (already async, off-thread, holding the dom with its
external `type=module src` roots inlined) now calls `prefetch_module_graph` and carries the resolved-url →
source map on a new `Prefetched.module_graph_sources` field. The map then rides onto a new
`Page.module_graph_sources` field in `from_prefetched_inner`, and `run_deferred_scripts` seeds it into the
JS layer (and clears it after) right where it runs the deferred/module pass — next to `set_scroll_geometry`
/ `set_snap_candidates`, the same publish-geometry-then-run pattern. **The seam is now a page field, not an
external thread-local set**, which is the point: the shell runs its deferred pass much *later* than it
built the page and possibly on a different worker thread, so the map has to survive on the page across the
blocking→paint→deferred gap rather than sit in a thread-local set by whoever fetched it. `load_async` was
refactored to set the same field instead of its own external `set_module_graph_sources` call, so both entry
points now flow through one seam. Gate `g_esm_prefetched_graph` drives the exact shell sequence
(`prefetch_document` → `from_prefetched_blocking_only` → `run_deferred_scripts`) over the same localhost
two-level graph and asserts `answer` (42) reached the DOM; drop the `page.module_graph_sources =
module_graph_sources` carry in `from_prefetched_inner` and it goes red (`#out` stays `-` — the map was gone
by the deferred pass). **With both paths live the class is genuinely unlocked** — native-ESM / no-bundler /
Vite-dev import-graph apps render in the agent AND in the window. Residue: dynamic `import()` uses its own
lazy hook.

**Bare specifiers resolve through an import map, and ONE resolver governs both paths (tick 520).** A bare
specifier (`import 'react'` — not `./ ../ /`, not a URL) has no built-in resolution here, so before this it
failed at `ModuleLink`; that is exactly how a CDN-pinned no-bundler app (`import {h} from 'preact'` mapped
to `https://esm.sh/preact`) fails to boot. A `<script type=importmap>` closes it. The page parses the map's
flat `imports` object (`extract_import_map`, serde_json — the FIRST map wins, malformed JSON → empty →
loud-but-safe) and carries it on `Page.import_map` / `Prefetched.import_map` beside the graph sources,
seeding it into the JS-layer `IMPORT_MAP` thread-local in `run_deferred_scripts`. The key move is a single
`resolve_module_specifier(base, spec)` that BOTH `module_resolve_hook` and `esm_load_graph` now call, so
pre-fetch and link never disagree: a relative specifier resolves against `base` (the importer), a bare
specifier is looked up in the map — **exact key first, then the longest trailing-slash PREFIX key**
(`"utils/"` maps `utils/num.js` → its target + `num.js`), the two standard import-map forms — and its
target resolved against the DOCUMENT url (import-map targets are document-relative, not importer-relative);
an unmapped bare specifier returns null (the link fails there, unchanged). The page-side `prefetch_module_
graph` mirrors the identical resolution (`resolve_page_specifier`) so the mapped urls get fetched. Gate
`g_esm_import_map` serves a document whose importmap declares both an exact key (`greeter`) and a prefix
key (`utils/`), drives it through `load_async`, and asserts `hi:42` (greet() + six*7) reaches the DOM;
neuter `extract_import_map` to empty and it goes red (`#out` stays `-`). Residue: import-map `scopes`
(per-path overrides) not yet honoured — a bounded follow-up; the flat `imports` covers the common case.

## An unhandled promise rejection is where every framework's failure goes to die

**Every modern framework renders inside an `async` function**, so a throw during render is a *rejected
promise*, not a synchronous error any catch site sees. With no rejection tracker the engine reported a
clean load of an empty page — and for several ticks the ledger recorded *"React mounts, schedules, throws
nothing, renders nothing"* as a **React** bug. React was throwing truthfully and **nobody was listening**.
`SetPromiseRejectionTrackerCallback` closes it. The moment it existed, Lit and Svelte stopped being
mysteries and became error messages.

This was the **third** distinct place errors were being discarded: empty `catch` blocks, swallowed
exception messages, and rejections.

## Custom-element upgrade has four traps, and each hides the next

1. Per ES semantics, **`HTMLElement`'s constructor must RETURN the element under upgrade**, so the derived
   class's `this` becomes the real element and `constructor(){super(); this.attachShadow(…)}` works.
2. **Copying only the class's OWN prototype is wrong** — real libraries are deep
   (`MyElement extends LitElement extends ReactiveElement extends HTMLElement`) and the machinery lives on
   the base.
3. **`el[k] = proto[k]` *reads* the property** — an accessor's getter runs with `this` bound to the
   *prototype* and its result is frozen as a plain value. **Copy descriptors, not values.**
4. **`this.constructor` must be the custom class**, because libraries read static config through it
   (`elementProperties`, `observedAttributes`, `styles`).

## Svelte 5 lifts accessors off `Node.prototype` — so reflectors need a prototype bridge

Svelte avoids per-node lookup by doing `get_descriptor(Node.prototype, 'firstChild').get` **once** at
startup and then `.call(node)`-ing the raw accessor on every node it walks. With reflectors whose members
are **own properties** and no shared prototype, `Node.prototype` was an empty object, `get_descriptor`
returned `undefined`, and `.get` threw.

The fix is a **prototype accessor bridge**: each prototype accessor looks up the OWN descriptor of whatever
`this` it is handed and delegates to it. *Reading the **descriptor** rather than the property is what stops
it recursing.*

## `Symbol.hasInstance` answers framework `instanceof` checks without a real prototype chain

`node instanceof HTMLIFrameElement` throws *`invalid 'instanceof' operand`* when the constructor is
`undefined` — **not a false answer, a thrown one.** Defining the interface constructors with
`Symbol.hasInstance` answers the question frameworks actually ask, with no need to build a real IDL
hierarchy.

⚠ **The shims must EXTEND what exists, never clobber it.** Replacing an already-present, load-bearing
`HTMLElement` broke every custom element and every `attachShadow` within a minute.

## `libc::_exit()` to dodge a shutdown crash is a data-loss bug in disguise

The shell called `libc::_exit()` to skip a SIGSEGV during SpiderMonkey shutdown (exit code 139, *after*
`main` returned, with perfect output). **`_exit` skips every exit handler — and in a browser those handlers
flush the user's profile.** Running `JS_ShutDown()` in order surfaced the real crash so it could be fixed.
`G_TEARDOWN` now forbids any process-exit path bypassing Rust `Drop`.

> *A workaround that hides a crash is a data-loss bug wearing a disguise.*

## A panic that travels through SpiderMonkey's C++ frames does not unwind — it ABORTS

Which is why layout panics on apple.com produced a **core dump** rather than a caught error. Related:
`panic = "abort"` in the release profile makes `catch_unwind` **impossible**, so "a bad page kills the tab,
not the browser" is unreachable *by construction* until the profile says `unwind`.

## A runaway task loop needs a hard drain ceiling — and the ceiling must assert the page RENDERED

An event loop that "drains to quiescence" **never returns** once a page schedules work that reschedules
itself. `setInterval(fn, 0)` is one line of JavaScript and it is on real carousels, clocks, pollers and
progress bars. So `setInterval` cannot exist before a `MAX_TASKS_PER_DRAIN` ceiling does — **and the
ceiling must also assert the page rendered**, because a ceiling that returns a blank page has swapped a
hang for a different nothing.

## The SpiderMonkey-vs-V8 "capability gap" is mostly a myth. The real gap is ONE API family.

Sites broken on Firefox and working on Chromium are overwhelmingly explained by **intentional
browser-sniffing** and untested library assumptions, not JS-engine conformance. The "V8 is just faster"
narrative traces to a retired, V8-over-tuned synthetic benchmark.

**The one real, documented gap:** `Error.captureStackTrace` / `Error.prepareStackTrace` /
`Error.stackTraceLimit` — **non-standard V8-only APIs** that popular libraries feature-detect (now a TC39
proposal *precisely because* it became a web-compat problem). Fix with a **shim in JS-environment setup**,
never a SpiderMonkey patch. Smaller sibling: V8 parses some non-ISO-8601 date strings more leniently.

## Lean JS engines (QuickJS, Hermes, JerryScript) are ruled out by the BAR, not by taste

They are well-built, and they target IoT/mobile/embedded scripting: they trade away modern **JIT tiering**,
debugger/profiler support, and spec/API depth for footprint. A stated bar of Chromium-grade capability
requires a browser-grade JIT engine, which leaves exactly two candidates — and **mozjs is the most mature
browser-grade JS-engine binding in the Rust ecosystem**, more proven than V8's Rust embedding path (which
has documented gaps around host objects and `ExternalArrayBuffer`).

## The SpiderMonkey modification boundary exists because of ADVERSARIAL-FUZZING CALENDAR TIME

**JIT miscompilation bugs are historically the largest single source of exploitable browser RCE.**
SpiderMonkey and V8 are trustworthy not merely because the code is correct, but because it has **survived
years of adversarial fuzzing (OSS-Fuzz/ClusterFuzz)** that an embedding project has no equivalent of.
*Faster code generation does not substitute for calendar-bound adversarial exposure.*

Therefore: **build config, feature flags and the FFI/binding layer are freely modifiable; JIT/GC internals
and the sandbox are a "come back to a human" boundary — not a "do it carefully" one.**

## Two SpiderMonkey contexts in one test binary segfault nondeterministically

The per-process runtime is leaked and tears down messily. The gate passed, then segfaulted, then passed.
**A flaky gate is worse than a missing one, because it gets ignored — and an ignored gate protects
nothing.** So JS gates are **one giant test per binary, on purpose.**

## `catch_unwind` AROUND an `extern "C"` fn does NOTHING. The catch must be INSIDE it.

`extern "C"` is **`nounwind`**. A Rust panic inside such a function is *"panic in a function that cannot
unwind"* → **abort/SIGSEGV** — and it aborts at **that function's own boundary**, *before any outer
`catch_unwind` is ever reached.

**So wrapping the native from the outside compiles cleanly, looks correct, and contains nothing.** (Done
here first; the gate still died.)

**The working shape:** the native is a **plain Rust `unsafe fn`**, and a **generated trampoline is the only
`extern "C"` frame** — with the `catch_unwind` *inside* it:

```rust
unsafe extern "C" fn trampoline(cx, argc, vp) -> bool {      // the ONLY nounwind frame
    match catch_unwind(AssertUnwindSafe(|| real_native(cx, argc, vp))) {
        Ok(ok) => ok,
        Err(_) => { error!(...); *vp = UndefinedValue(); true }
    }
}
```

**Return `true`, not `false`.** `false` tells SpiderMonkey *"an exception is pending"* — and there isn't
one. **That trades a segfault for an assertion failure.**

**And it must be LOUD.** *A crash you made survivable and invisible becomes a permanent, unexplained "this
site just doesn't work."*

> **This also requires `panic = "unwind"` in the profile.** Under `panic = "abort"`, `catch_unwind` cannot
> exist and per-page containment is **unreachable by construction** — a build-profile decision *before* it
> is a code decision.

## Shutting SpiderMonkey down — and the rule that makes teardown work

For sixty ticks the engine carried an open Bar 0 residual: a binary would boot SpiderMonkey, run
JavaScript perfectly, print correct output, and then **SIGSEGV after `main` returned**.

```
mozilla::detail::MutexImpl::~MutexImpl: pthread_mutex_destroy failed: Device or resource busy
process didn't exit successfully (signal: 11, SIGSEGV: invalid memory reference)
```

SpiderMonkey requires **`JS_ShutDown()` before the process exits**. Skip it and its C++ static
destructors run against a still-initialized engine and die inside `__run_exit_handlers`.

**This is not cosmetic.** A crash in the exit handlers *aborts the handlers that follow it* — and that is
precisely where a browser flushes its cookie jar and `localStorage` to the profile (ADR-009). The
user-visible bug is **silent data loss on quit**: you close the window, and your session is gone.

### The workaround that wasn't

The old answer was a convention: *"every binary must call `manuk_js::shutdown()` last."* `g_runaway`,
`g_alloc`, `g_load_budget` and the shell remembered. `g_globals` and `g_dedup` did not — and crashed,
every run, for sixty ticks. **A convention that half the callers forget is not a fix; it is a list of the
places you have not been bitten yet.**

### The ordering trap (this is the part worth remembering)

The obvious fix is to put the `Runtime` and the `JSEngine` in one struct, in one thread-local, with a
`Drop` that tears them down in the one correct order — context first, then `JS_ShutDown()`. **It does not
work**, and it fails in a way that teaches the actual rule:

> **Thread-local destructors run in REVERSE order of registration.** And mozjs keeps thread-locals of its
> own: `Runtime::drop` → `DestroyContext` → `finishRoots` → **`trace_traceables`**, which is a mozjs
> thread-local that does not exist until the **first `rooted!`** — i.e. it is registered lazily, *during
> the first eval*.

Our state has to be initialized *before* any of that (the engine must be parked somewhere the instant
`JSEngine::init()` returns), so it registers **first**, so it is destroyed **last** — by which time mozjs's
thread-local is already gone. Teardown then dies with `cannot access a Thread Local Storage value during
or after destruction`, inside a `nounwind` frame, which is an instant abort. **One exit crash traded for
another.**

`atexit` does not save you either: glibc's `exit()` runs `__call_tls_dtors()` **before** it walks the
atexit list, so an atexit handler sees an even deader world.

### The shape that works

Split the **state** from the **trigger**:

* `ENGINE` and `RUNTIME` are thread-locals holding `ManuallyDrop`, which has **no drop glue** — so they
  register *no destructor at all*, are never torn down by TLS, and stay readable at any point during
  shutdown.
* `TeardownGuard` is an empty struct whose only content is its `Drop`. It is first touched **after the
  first eval has run** — therefore registered *after* mozjs's lazy thread-locals, therefore destroyed
  *before* them, while everything it needs is still alive.

Teardown then runs in the only correct order: drop the `JSContext`; clear the published engine handle (a
cached handle is an *outstanding* handle, and `JSEngine::drop` asserts on those); call `JS_ShutDown()`;
and set a flag so a late request for JS gets an honest error instead of a crash (SpiderMonkey may not be
re-initialized in a process that has shut it down).

> **To run first at teardown, register last.**

That is the whole rule, and it generalises well past SpiderMonkey — it applies to any C library with lazy
thread-local state that you must outlive.

`manuk_js::shutdown()` still exists and is still called by the shell, because a *browser* wants to choose
the moment it stops running JavaScript (before it flushes the profile), rather than inherit whatever
moment the runtime picks. It is now an optimization, not a requirement. **`G_CLEAN_EXIT`** holds the line:
it re-executes the test binary as a child that runs real JavaScript and then simply returns from `main`,
and demands exit code 0.

## DOM reflectors: the prototype chain, and the two bugs hiding in "it works"

For sixty ticks every DOM method was defined as an **own-property of every element** — all 116 of them,
one `JS_DefineProperty` per node. Elements answered `div.setAttribute(...)` correctly, so it looked fine.
It was wrong in three ways at once, and two of them were invisible.

**1. The interfaces were empty.** `Element.prototype.setAttribute` was `undefined`. So was
`Node.prototype.appendChild`. `EventTarget` did not exist at all — a bare `ReferenceError`. Feature
detection (`'matches' in Element.prototype`) and borrowed methods (`Element.prototype.setAttribute.call(el, …)`)
both failed.

**2. Patching a prototype SILENTLY DID NOTHING — this is the one that matters.**

```js
const real = Element.prototype.setAttribute;
Element.prototype.setAttribute = function (n, v) { track(n, v); return real.call(this, n, v); };
```

That is *the* way the web instruments the DOM: Sentry and every error tracker, ad-blockers, polyfills,
framework internals, React DevTools. The assignment succeeded. Nothing threw. And the element's **own**
property shadowed the patched prototype, so the wrapper was never called. **The library believes it is
installed and it is not.** A loud failure gets fixed; a silent one ships.

**3. It was slow, per element.** 116 property definitions *and two full JS compiles* per node — the
identity cache (`__nodes[id]`) was read and written by `eval`ing a formatted source string. Creating
5,000 divs took **124ms**. Every React/Vue/Angular render pays that.

### The shape now

```text
element → HTMLElement.prototype → Element.prototype → Node.prototype → EventTarget.prototype
document → Document.prototype   → Node.prototype    → EventTarget.prototype
```

Built once per global (`dom_bindings::dom_protos`), cached on the global so it is GC-reachable. Every
member is defined **once**. The identity cache is a real object read with `JS_GetElement`, not a compile.
Reflectors carry **one** own property (`__nodeId`) instead of 116.

**Result:** `createElement` ×5,000 went **124ms → 2ms** (~60×), and `Element.prototype.setAttribute = wrapper`
now actually runs. `G_PROTOTYPE` holds both, and is proven to go red when the members go back on the
instance.

### Two traps worth knowing

* **The prototypes are `NODE_CLASS` objects with unset reserved slots**, on purpose. `node_and_dom()`
  checks `is_int32()` and returns `None`, so calling `Element.prototype.tagName` with `this` *being the
  prototype* yields `undefined` — instead of reading reserved slots off an object that has none, which is
  UB and in release is a garbage pointer dereference.

* **A raw `*mut JSObject` held across ANY allocation is a dangling pointer.** The first version cached the
  `__nodes` object pointer, then called `dom_protos()` — which defines ~116 properties, any one of which
  can trigger a **moving** GC. It segfaulted on the first page. Rust's type system cannot see this: to it,
  a `*mut JSObject` is a number. **Root immediately, always.**

### The stated limit

The members are own-properties of `Node.prototype` rather than distributed across the Node / Element /
HTMLElement tiers, because this engine's member list does not yet distinguish them (`appendChild` and
`setAttribute` live in one list). So `Element.prototype.hasOwnProperty('setAttribute')` is `false` where
the spec says `true`. Everything that *resolves* through the chain is correct; the ownership tiering is a
later tick. Saying so beats pretending.

### And it moved WPT not at all

It was tempting to bank `dom/nodes`' rise against this. A/B on the same tree — the change mutated out —
gives **1736/6418, identical to the subtest**. *A number you cannot attribute is not a result.*

## `<canvas>` 2D — from a stub that drew nothing to a real rasterizer

For sixty ticks `getContext('2d')` returned a context object whose every drawing operation was a `noop`.
That was a **deliberate and honest trade** for its time, and worth understanding before replacing it: the
alternative was `getContext` being `undefined`, which made `ctx.fillRect(...)` on the next line a
`TypeError` that took the whole bundle down. **A blank chart on a working page beats an exception**, and
it even warned in the console.

But it is the worst *shape* a failure can take while still counting as "working": a page feature-detects
canvas, is told **yes**, draws its chart, and nothing appears — with no error anywhere. `G_CAPABILITY`
measured it exactly: fill the canvas red, read the pixel back, get `0,0,0,0`.

### How it reaches the screen — with no new machinery

This is why canvas took one tick rather than five. The painter **already** scales a
`manuk_paint::DecodedImage` into a replaced element's content box, keyed by `NodeId` — that is how `<img>`
works, and how an `<iframe>` is composited. **A canvas is simply an image the page draws into.** So:

* each `<canvas>` owns a `tiny_skia::Pixmap` (`engine/js/src/canvas.rs`);
* the JS context draws into it;
* `Page::drain_canvases()` moves the finished, *dirty* pixmaps into the same image map an `<img>` lands
  in, and **the painter never learns that a canvas exists**.

### Where the state lives, and why it is split

The **state machine** — `fillStyle`, `strokeStyle`, `lineWidth`, `globalAlpha`, the transform stack, the
current path — stays in **JavaScript**, where colour strings, `save()`/`restore()` and method chaining are
cheap. Only **rasterization** crosses into Rust, with the colour and transform already resolved.

A path crosses as **one flat `[op, args…]` array**, not one call per segment: a chart with 10,000 points
must not pay 10,000 FFI crossings. Every read of that stream is bounds-checked, because a truncated array
would index off the end — and **a panic inside a JSNative is `nounwind`, so it aborts the browser rather
than throwing** (PROCESS #34).

### Done, and honestly not done

**Works:** `fillRect`, `strokeRect`, `clearRect` (to *transparent*, not white), paths — `moveTo`,
`lineTo`, `quadraticCurveTo`, `bezierCurveTo`, `rect`, `arc` (flattened to line segments, sub-pixel error),
`fill`, `stroke` — the full transform stack (`save`/`restore`/`translate`/`scale`/`rotate`/`setTransform`),
CSS colour parsing (`#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()`, `rgba()`, named), `globalAlpha`, **real**
`getImageData` (non-premultiplied, as the spec hands JS), and **real** `toDataURL` PNG.

**Honest no-ops, named rather than hidden:** `fillText`/`strokeText` (but `measureText` returns a real
shape — layout code multiplies by `.width`, and `undefined * n` is `NaN`, which poisons every coordinate
downstream), `drawImage`, `clip`, `putImageData`. Gradients return a real object and are approximated by
their last stop — **a bar drawn in the gradient's end colour beats a bar that is not drawn.**

### The bug that hurt most, and it was not the rasterizer

`canvas.width` and `canvas.height` **did not exist as JS properties**. So `el.width` read `undefined`, the
backing store fell back to the spec default of 300×150 — and the drawing was then *perfectly correct
inside a 300×150 surface*, which the painter dutifully scaled down into the element's real box. A chart
drawn at its true size came out as a smudge in the corner.

> **The pixels were right and the surface was wrong** — which is far more confusing than a blank canvas,
> because `getImageData` agrees with you the entire way down.

They are IDL attributes reflecting the content attributes now, and assigning either one resizes **and
clears**, which is the spec and is the idiomatic way to erase a canvas.

## Reflecting a NUMBER attribute: four rules the naive getter gets wrong (tick 117)

An integer-reflecting IDL attribute (`maxLength`, `tabIndex`, `colSpan`, `width`, …) is a view over a
content attribute with the HTML spec's coercion in between. Getting one is *not* `parseInt` — it is the
HTML "rules for parsing integers" **plus** a per-type range/default rule **plus** the WebIDL numeric type,
and each of those layers hides a subtle failure. The ground truth is WPT's own `html/dom/reflection.js`
`domExpected` functions — read those, do not re-derive the spec prose.

1. **`-0` is `+0`.** The HTML integer rules accumulate a magnitude and return a bare `0` when it is zero,
   *sign discarded*. JS `parseInt("-0", 10)` returns `-0`, and `assert_equals` is `Object.is`-based, so a
   leaked `-0` fails **every** `setAttribute() to "-0"` case — one per numeric attribute per element, the
   single biggest cluster (143 subtests). Normalise at the parse seam: `n === 0 ? 0 : n`.

2. **Overflow FALLS BACK — it does not wrap.** `tabindex="2147483648"` is outside the signed-32 range, so
   a plain `long` reflects the **default** (`0`), not `-2147483648`. The tempting "fix" is `n | 0` (ToInt32),
   and it is exactly wrong: no browser wraps here, because the reflection algorithm's range check fires
   *before* any IDL conversion. So a plain `long` must range-check `[-2^31, 2^31−1]` and fall back outside
   it — not only the unsigned family.

3. **The default is per-type, not always `0`.** `limited long` (`maxLength`/`minLength`, "non-negative")
   defaults to **`-1`**; `limited unsigned long` (`size`, "> 0") defaults to **`1`**. An invalid *or*
   out-of-range value returns that type default, table-overridable via a `d` field.

4. **`clamped unsigned long` CLAMPS; it does not fall back.** `colspan` of a billion is `1000` (the max),
   `colspan="0"` is `1` (the min) — out-of-range saturates to the bound. Only a parse failure / negative
   returns the default. The bug to avoid: applying the plain-unsigned `> 2^31 → default` cutoff *before*
   the clamp, which turns a huge colspan into the default instead of the max.

**Reach.** Only attributes whose IDL name is *not* already on the prototype chain route through the generic
reflector (`if (idl in proto) return;` — a native binding always wins). So `li.value`/`ol.start`/`pre.width`
are natively shadowed and untouched by this; `tabIndex`/`maxLength`/`colSpan`/`width` are reflected and
were the ~380-subtest lever. Gated by `G_REFLECT_NUMERIC`, kept in its own test binary because two
SpiderMonkey `Page::load`s in one process reuse the runtime and can trip the reflector-teardown UAF.

## dispatchEvent validity, and the native seam that swallowed the throw (tick 118)

DOM §`dispatchEvent` requires `InvalidStateError` in two states: the event's **dispatch flag** is set
(you are re-dispatching the same event object from inside one of its own listeners) or its **initialized
flag** is not set (a `document.createEvent("Event")` event that was never `initEvent`-ed). The rule is one
line; the trap is where the throw has to *survive*.

`el.dispatchEvent` is a **native** (`el_dispatch_event`) that hands the event to a JS helper
(`__dispatchEvent`) via `eval_in_current_global`. That helper returning an error was `unwrap_or(false)`'d —
so an `InvalidStateError` thrown inside it became a benign `false`, and `assert_throws_dom` saw **no throw**.
The generalisable lesson (the swallowed-error class this project keeps rediscovering): **a native that
coerces a JS exception into a return value erases it.** The fix is to let `eval` return `None`, check
`JS_IsExceptionPending`, and return `false` from the native *with the exception still pending* so
SpiderMonkey propagates it to the JS caller.

The flags live on the event object as `__initialized` / `__dispatchFlag`:
- `createEvent` sets `__initialized = false`; `initEvent` clears it back to `true`.
- A **constructed** event (`new Event()`) leaves `__initialized` `undefined` — deliberately not `=== false`
  — so it dispatches normally without any per-constructor bookkeeping. Only createEvent-without-init is
  uninitialized.
- `__dispatchEvent` sets `__dispatchFlag = true` for the duration of the walk and clears it at the end, so
  a listener that re-dispatches the same object throws, but the object is dispatchable again afterward.

## A node id is unique only WITHIN its arena — so a reflector must resolve against its OWN document

This is the lesson that made `iframe.contentDocument` possible, and it is a trap every second-document
feature will hit. A DOM reflector stores its node as a bare `i32` in a reserved slot, and for one document
that is fine: the id indexes the one arena. **The moment a second arena exists — an `<iframe>`'s child
document — node #7 exists twice**, and a reflector that resolves its id against the *one* thread-local
`CURRENT_DOM` reads the parent's node #7 for a child reflector: a different element, in a different
document, with total confidence. Nothing throws; the wrong node is simply returned.

The fix has three parts, and all three are load-bearing:

1. **The reflector carries its arena** in a second reserved slot (`SLOT_DOM`), written at creation, and
   `node_and_dom` resolves against *that*, not `CURRENT_DOM`.
2. **A registry of live arenas** (`LIVE_DOMS`, a thread-local `HashSet<usize>` of arena addresses)
   makes that safe: a reflector held by a script after its `Page` dropped points at freed memory, and
   `is_alive()` cannot save you — it validates a node id *within* an arena, and the arena itself is what
   went away. So a `Page`'s `Drop` **unregisters its arenas before they free**, and an unregistered
   pointer resolves to `None` (a JS `null`), which is a correct answer where a dereference is a crash.
3. **`SLOT_DOM` holds a `PrivateValue`, and a `PrivateValue` IS a double.** Reading it back, a guard
   written `if v.is_double() { reject }` throws away every legitimate arena pointer — silently, because it
   *looks* like the feature simply not working. The only value to reject is `undefined` (the prototype
   objects' empty slots); everything else is validated by the registry.

> The transferable rule: **any per-document state keyed by node id is wrong the instant a second document
> exists.** Reflect against the arena the node came from, and gate that arena's liveness through a
> registry, not through the node-generation check.

## A per-arena identity cache must not CLOBBER the shared `__nodes` — it breaks event dispatch silently

`a.firstChild === b` requires one wrapper per node, so reflectors are memoised in a JS-side map. With two
documents that map must be **per-arena** (parent node #7 and child node #7 are different objects), keyed by
the arena address: `__nodes_<addr>`. The trap: the **main** document's cache is the global `__nodes` that
`install` seeds the `document` reflector into — and the first cut created a fresh `__nodes_<addr>` for the
main document *too*, and pointed `__nodes` at it. That fresh map does not contain the seeded `document`, so
`__nodes[0]` became `undefined`, so `document.dispatchEvent(ev)` — which does `target = __nodes[nid]` —
found nothing and **stopped reaching document-level listeners** (`DOMContentLoaded`, delegated clicks).
The symptom appeared only after a script touched `document.body` (the access that first built the bad
cache), and `G_LIFECYCLE` caught it as `seen: dcl-win, load` — missing `dcl-doc`. **The main document's
cache IS `__nodes`, looked up and reused, never replaced; only child documents get their own map.**

## Mass reflector access + the reflection layer can overflow the C stack, and SpiderMonkey won't catch it

`document.querySelectorAll('*')` and reading a property on every element — an ordinary thing for a
framework or a polyfill to do — forces a reflector for the whole tree, and with the HTML-attribute
reflection layer installed that mass access tripped an **infinite JS recursion** (a reflected accessor
re-entering `getAttribute`/`setAttribute` through the mutation-observer wrappers) that overflowed the C
stack into a **SIGSEGV** — Bar 0. The nasty part: SpiderMonkey's `JS_SetNativeStackQuota` is supposed to
turn that into a catchable *"too much recursion"*, but the quota is an **absolute address computed at
`Runtime::new`**, which in an async/tokio embedding runs buried deep in the call stack — so the limit sits
past the real stack bottom and never fires. Re-anchoring the quota per call did not reliably help
(headroom varies by call depth). **The durable fix was structural: do not iterate the whole tree in JS.**
A native (`__inlineHandlerNodes`) finds the handful of elements that actually need wiring by a single
arena walk in Rust, so JS never touches every reflector. The latent recursion (reflection + mass access)
is real and un-fixed; the engine simply never triggers it.

> Two rules fall out. **(a)** A getter that needs computed style or geometry already has it — check
> `STYLES_PTR` / the view maps before marshalling anything new across the FFI. **(b)** When a JS operation
> must touch "every element," ask whether Rust can answer it from the arena instead; the arena walk is
> O(n) and allocation-free, the JS reflector sweep is neither, and it is one reflection bug away from a
> stack overflow the engine cannot report.

**This recursion is a CONCRETE BLOCKER for reflection expansion, proven by ARIA.** Adding the ~44 ARIA IDL
accessors (`el.role`, `el.ariaLabel`, …) to `Element.prototype` — correct, tested, working in isolation —
made a *different* `html/dom` file tip the same recursion into a SIGSEGV in the full-suite run (0 crashes
without ARIA, 1 with; the extra accessors deepen the mass-access recursion past the C-stack limit).
Enumerable-vs-not made no difference: it is the accessor *count on the mass-access path*, not enumeration.
So ARIA reflection — and any further reflection-surface growth — is gated on making SpiderMonkey's native
stack quota **effective** first, so the deep recursion throws `InternalError: too much recursion` (which a
WPT test survives) instead of segfaulting. The quota is set once in `Runtime::new` relative to a
stack-pointer buried deep in the async call stack; the durable fix is to set `JS_SetNativeStackQuota` from
the **actual thread-stack bounds** (`pthread_getattr_np` + `pthread_attr_getstack`, minus a safety
margin), so the limit is real regardless of call depth. **That is the prerequisite tick; ARIA rides on
it.** Reverted rather than shipped, because a Bar 0 crash is never a trade for a capability.

## A reused SpiderMonkey runtime across many pages SIGSEGVs — a cross-file reflector/rooting UAF (open Bar-0)

The WPT batch harness runs many files in ONE process, reusing the process-global runtime (thread-local
`ENGINE`/`RUNTIME`, `ManuallyDrop`, tick-62) while making a fresh `Page`/PageContext per file. After
~20–40 accumulated pages, a later page (repro: `css/css-flexbox/stretched-child-shrink-on-relayout.html`,
which exercises the incremental **relayout** path) **SIGSEGVs (exit 139)**. It is clean in isolation
(fresh runtime) — so this is **cross-file heap corruption: a dangling reflector / unrooted `*mut JSObject`
that survives one page's teardown and is touched during the next page's GC or layout.** This is the
H0.4 "largest unsafe surface" (GC rooting across FFI), the same class as the tick-84 saga.

**Properties that decide how to fix it:** it is a **Heisenbug** — heap-layout/allocator-timing sensitive,
reliably reproducing only under memory pressure, and **it disappears under `gdb`** (perturbed heap), so
gdb yields no usable backtrace. **Use ASAN** (`-Zsanitizer=address` + an ASAN mozjs, or `valgrind`) which
catches the free-at-source regardless of layout — not gdb. Reproduce: `manuk-wpt wpt css/css-flexbox
--child --out /tmp/x --limit 40` (exit 139) vs `--limit 20` (exit 0); bisect predecessors to a minimal
pair, then find the reflector whose lifetime outlives its arena node across the per-page teardown.

**Until fixed, the sweep tolerates it HONESTLY via isolation-retry** (the culprit is re-run alone and
counted as `ACCUM`, not a per-page `CRASH` — see [[conformance-and-oracles]]); the UAF itself stays an
open Bar-0. Do NOT start this fix at the tail of a maxed context. [[interactive-js-architecture]]

## A second, DETERMINISTIC C-stack overflow (html/semantics) — NATIVE recursion, not the stack-quota class

> **CORRECTION (tick 106): this is NATIVE recursion, not a JS-stack-quota crash — proven by building the
> quota fix.** Tick 105 hypothesised the stack-quota class; tick 106 implemented the effective quota
> (`libc`/`pthread_getattr_np` → real thread bottom, set at `lib.rs` `with_runtime` where the page-eval
> `RUNTIME` is created) and the file **still SIGSEGVs on the main thread with a 7 MB-headroom quota**. A JS
> stack quota only guards the C stack at JS call checkpoints; this crash overshoots them, so the recursion
> is in **our own Rust** — the `<script>.textContent` setter re-preparing the script / re-evaluating CSP,
> re-entering itself between checkpoints. **Fix = a "script is already started/prepared" guard to break the
> re-entrant loop** (HTML spec's "already started" flag), found with a debug build — NOT the quota. The
> quota fix was reverted (it doesn't hit this gate; its real value is small-worker-thread JS recursion,
> which is un-gateable on the main thread and needs a worker-thread repro + full-sweep pass to land).

Opening the aperture (tick 104) surfaced `html/semantics/scripting-1/the-script-element/`
**`script-text-modifications-csp.html`**, which **SIGSEGVs (exit 139, core dumped) in ISOLATION** — a
*deterministic* single-file crash, unlike the flexbox Heisenbug ([[interactive-js-architecture]]). The
gdb backtrace is a tight repeating 3-address cycle over NaN-boxed JS values (0xfff8/0xfff9/0xfffe tags)
= **deep JS recursion overflowing the C stack** — a SIGSEGV where SpiderMonkey should throw *"too much
recursion"*. This is the same **stack-quota mis-anchoring** documented above: mozjs 0.18's `Runtime::new`
calls `JS_SetNativeStackQuota(cx, STACK_QUOTA, …)` with the limit computed from `nativeStackBase` captured
at `JS_NewContext` — buried deep in the tokio `block_on`, so the guard sits past the real stack bottom.

The trigger looks benign — `t.step_timeout(changeScriptText, 500)` self-scheduling, which a real browser
DEFERS (no recursion). Our `setTimeout` correctly defers (macrotask FIFO, `event_loop.rs`), so the
recursion is elsewhere — most likely re-entry through the `<script>.textContent` setter + CSP re-eval, or
the harness event-loop drain. **Needs a symboled/debug build to pinpoint** (the stripped release
backtrace is addresses only; because it is DETERMINISTIC, gdb WILL catch it on a debug build — the
Heisenbug would not). Because it is deterministic, this is the **better first target than the flexbox
UAF** for the fresh-context stack-quota tick: fix it (effective quota from real thread-stack bounds via
`pthread_getattr_np`, OR the specific script-text/CSP recursion) → gate that this file throws instead of
crashing → then html/semantics (~8,879 failing, the biggest mass) can join the sweep. It is currently
**held out of `AREAS`** precisely because of this crasher (and one more in the same tree). [[conformance-and-oracles]]

## Web Crypto entropy: `crypto.getRandomValues` / `randomUUID` (tick 160)

SpiderMonkey (via mozjs 0.18) does **not** expose a `crypto` global, so the boot prelude
(`event_loop.rs`) installs one. JS on its own has **no path to a CSPRNG** — `Math.random()` is the only
entropy primitive in the language, and it is explicitly *non*-cryptographic — so a correct `crypto`
requires a **host call**. The native `__cryptoRandomHex(n)` (`dom_bindings.rs`) fills `n` bytes from the
OS CSPRNG (`getrandom` crate → `getrandom(2)`/`/dev/urandom` on Linux, `BCryptGenRandom` on Windows) and
returns lowercase hex; `n` is clamped to WebCrypto's 65536-byte per-call quota. `getrandom` is an
**optional dep gated to `_sm`** — it is only reachable from the SpiderMonkey native, so the JS-less build
never pulls it.

Two non-obvious shaping rules the shim must enforce, because getting either wrong is a *silent* bug:

- **Fill through a BYTE view, not per element.** `getRandomValues` takes any integer typed array. Writing
  `a[i] = random & 0xff` fills a `Uint32Array` element with only its low byte — 24 bits always zero. The
  fix writes through `new Uint8Array(a.buffer, a.byteOffset, a.byteLength)` so every *byte* of every
  element is random, whatever the element width (`Uint8`…`BigUint64`). The type guard rejects
  Float*/DataView/plain arrays (`TypeMismatchError`) and `>65536` bytes (`QuotaExceededError`).
- **Stamp the RFC 4122 bits for `randomUUID`.** Draw 16 CSPRNG bytes, then `b[6]=(b[6]&0x0f)|0x40`
  (version = 4) and `b[8]=(b[8]&0x3f)|0x80` (variant = 10xx) before hex-formatting `8-4-4-4-12`. Skipping
  the variant nibble emits strings that *look* like UUIDs but fail strict v4 validators (index-19 nibble
  not in `[89ab]`).

The general lesson, which recurs across this shim layer ([[js-engine]] boot globals): **a security
primitive that "works" (returns a value, no throw) can still be catastrophically wrong.** `Math.random()`
tokens pass every functional test and every guessable one. The gate (`g_crypto.rs`) therefore asserts the
*statistical* consequences a correct CSPRNG must have (full-width fill, independent draws differ) and not
merely that the call returns — the same "construct AND answer honestly" bar the missing-globals gate
holds, extended to "answer *securely*". `crypto.subtle` (SubtleCrypto) stays **undefined** — the honest
"cannot" (browsers expose it only in secure contexts), a separate larger tick if a page class needs
`subtle.digest`.

## HTML Constraint Validation, on the shared prototype (tick 161)

The form-validity API (`el.validity`, `el.checkValidity()`, `el.willValidate`, `setCustomValidity`, the
`invalid` event) is installed as a **prelude shim on `__protoHTMLElement`** — the real HTMLElement
prototype built in Rust (`dom_bindings::dom_protos`), which every element instance has in its
`[[Prototype]]` chain. This is the correct attach point precisely because our reflectors carry their DOM
members as OWN properties with no shared prototype: a NEW method (one not already an own-property
reflector) must go on the shared proto or no instance will find it. Defining `validity`/`checkValidity`
there gives every `<input>`/`<select>`/`<textarea>`/`<form>` the API for free.

The shim is **pure JS over existing reflectors** — it reads `el.required`, `el.pattern`, `el.type`,
`el.min`/`el.max`, `el.minLength`/`el.maxLength` (all live via G_REFLECT, numeric ones defaulting to
`-1` per G_REFLECT_NUMERIC so an unset `maxLength` correctly imposes no limit) and the current `el.value`,
and computes the eight `ValidityState` flags. Two spec subtleties worth keeping: (1) an element **barred
from constraint validation** (a `type=hidden`/submit/reset/button/image input, or a disabled/readonly
control) has `willValidate === false` and `checkValidity()` short-circuits to `true` — it is not "valid",
it is "not a candidate"; (2) `checkValidity()` must fire a **cancelable `invalid` event** at each failing
control *before* returning, and `form.checkValidity()` fires one per failing descendant control — which
is how native validation lets a page `preventDefault` and show its own message.

What is deliberately NOT here: the `:valid`/`:invalid` **CSS pseudo-classes**. Those require Stylo to
match a pseudo keyed on the element's live validity (a restyle on every value change), which is a cascade
tick, not a JS-shim tick. Leaving them unwired is honest — a page's *script* validation works; only the
*CSS-driven* red-border styling is absent, and that degrades to "no styling" rather than to a throw.

## crypto.subtle.digest — async surface over a sync host hash (tick 162)

`crypto.subtle.digest` shows the general recipe for exposing an **async Web API backed by a synchronous
host computation**. The digest itself is instant (a few KB through SHA-256), but WebCrypto's signature is
`Promise<ArrayBuffer>`, and real code always `await`s it. So the native `__subtleDigestHex` computes
synchronously (RustCrypto `sha2`/`sha1`, hex string-in/string-out — the same single-function FFI shape as
`__cryptoRandomHex`), and the JS shim wraps the result in `Promise.resolve(buffer)`. Errors become
`Promise.reject` (an unknown algorithm → `NotSupportedError`, a non-BufferSource → `TypeError`), never a
synchronous throw — matching what `.then(ok, err)` / `.catch` expect.

Two things worth reusing:

- **The microtask queue drains during `Page::load`.** A gate that reads its result from a `.then` (even a
  chained `Promise.all([...]).then`) works, because load pumps the job queue to empty before returning —
  the same delivery path `MutationObserver` (queueMicrotask) relies on. This means async APIs *can* be
  gated by a synchronous page load + `textContent` read, as long as every result funnels into one final
  `.then`.
- **Provide only what you can do correctly, and leave the rest genuinely absent.** `subtle` exposes
  `digest` and nothing else; `sign`/`encrypt`/`deriveKey` stay `undefined` so a page's
  `if (crypto.subtle.encrypt)` feature-check takes its fallback path instead of calling a stub that lies.
  This is the same "construct AND answer honestly" discipline the missing-globals gate holds — a broken
  stub is worse than an honest absence, because the caller cannot route around a method that pretends to
  exist. (SHA-1 is deliberately kept, not dropped: SubtleCrypto still exposes it for verifying legacy
  signatures, even though it is not collision-resistant — "available" is a spec fact, not an endorsement.)

## Forced synchronous reflow — the read path lays out before it answers (tick 213)

The engine lays out in a **batch**: script runs against a layout snapshot taken *before* it started, and
one relayout happens after. That is correct for a script that only measures, and correct for one that only
mutates. It is wrong for the shape every virtualized list is built out of:

```
  measure  ->  mutate  ->  measure       (all inside ONE task / rAF)
```

react-window, react-virtuoso and every data grid size their rows by writing to the DOM and immediately
reading it back. Against a pre-script snapshot the second read returns the geometry the element had
*before* the write — `0` for a node that did not exist yet — so rows collapse, overlap, or render blank.
A real browser answers this by **forcing a synchronous reflow**: a geometry read on a dirtied DOM lays out
first, then returns. It is the *read path's* job; the page never asks for it.

**The relayout machinery already existed** (`relayout_incremental`, `RestyleDamage`). The only missing
piece was wiring it into the read path, and the shape of that wiring is the reusable part:

- **A monotonic `Dom::mutation_seq`, not the dirty bits.** The dirty *bits* answer "must the next batch
  pass do work?" and are **consumed** by that pass — useless for a question asked mid-script that must not
  disturb the batch. A monotonic counter answers by *comparison* instead: the reflow context records the
  seq it laid out against and reflows only when it differs. Repeated reads on an unchanged tree cost one
  integer compare, and the post-script batch relayout still sees exactly the bits it always saw.
- **The hook is a call UPWARD.** Layout lives in `manuk-page` (cascade, box tree, stylesheet set);
  `manuk-js` has no layout dependency and must not grow one. So the host installs a `ReflowFn` +  context
  pointer for the duration of a re-entry, exactly like the view maps.
- **A STACK of hooks, not a slot.** Script rounds nest — a click on a `<label>` dispatches a second click
  at the control it labels, inside the first. With a slot, the inner round's teardown silently disarms the
  outer one and every read after it quietly reverts to the stale snapshot.
- **The reflow builds its OWN maps and re-points the bindings.** It cannot write into the maps the host
  passed in: a script is reading those through a shared reference for the whole round. `ReflowScope`'s
  `Drop` then restores the previously-published pointers — without that, buffers owned by the scope
  outlive it, and the symptom is not a crash but *the next document silently measuring freed memory*.
- **An `IN_REFLOW` re-entrancy guard**, because the reflow performs reads of its own.

**Both `layout_rect` AND `with_style` force it.** `getComputedStyle` is a forced-reflow trigger in real
browsers just as much as `getBoundingClientRect` — the forced reflow re-runs the cascade, so the styles it
publishes are fresh too. Gating only the geometry read would leave the two APIs disagreeing about the same
element one line apart.

Held by `engine/page/tests/g_forced_reflow.rs` (`G_FORCED_REFLOW`). Falsified by removing the
`force_reflow_if_stale()` call: every read reverts to `after:0 row:0 grown:10 offset:0` — pre-mutation
geometry, which is the blank-virtualized-list bug exactly.

⚠ **One `#[test]` fn per JS gate binary** (see `g_canvas.rs`). A test fn that dispatches a click leaves a
live `PageContext` parked on its thread; a second test fn loading a page on another thread faults two
SpiderMonkey runtimes against each other. Sequential `Page::load`s inside ONE fn are fine.

---

## Web Workers — running a script in a scope that must NOT be the page's (tick 280)

`new Worker(url)` used to construct and then fire `error` on the next turn: the shape of a worker
script that 404s. That was honest and it was still a dead end, because a page whose real work happens
off the main thread has no inline fallback. Its `onerror` path surfaces the failure; it does not redo
the job. The observable symptom is not an error message, it is a **spinner that never resolves**.

### The scope is a deny-list over a `with`, not an allow-list

The worker script is evaluated as `Function('__scope', 'with (__scope) { ' + src + ' }')`. Two
consequences worth stating outright:

- **A `"use strict"` at the top of the worker script does not break it.** The directive lands *inside*
  the with-block, where it is an expression statement rather than a prologue directive. Strict worker
  scripts — which is most of them — run unmodified.
- **What the scope does NOT define falls through to the real global on purpose.** `fetch`, `Promise`,
  `TextDecoder`, `crypto`, `WebAssembly`, even a nested `Worker` all resolve without being enumerated.
  The scope is therefore a **deny-list of what a worker must not have** rather than an allow-list of
  what it may, and an allow-list is the thing that goes stale every time the platform grows a name.

The deny-list is the load-bearing half. Each entry (`document`, `window`, `localStorage`, `parent`,
`getComputedStyle`, …) is set to `undefined` **explicitly**, because `typeof document === 'undefined'`
is how essentially every isomorphic module decides which half of itself to run. A worker scope that
leaks the page's globals does not fail loudly — it makes that decision *wrong*, and then lets the
main-thread branch touch a DOM that must not exist there. `G_WEB_WORKER` proves this is the real
failure mode: with the deny-list removed, `sum:true` still passes while `nodoc`/`nowin`/`nols` all
flip together. The compute works and the scope is a lie, which is exactly the half-working state that
is invisible from the API surface.

The scope object is `Object.create(null)`. **This is defensive, and the gate does not assert it** — two
probes were written (`constructor === Object`, `__proto__ === Object.prototype`) and both returned the
same answer under a plain-object scope and a null-prototype one, because the page's own global inherits
from `Object.prototype` too, so the `with` fall-through finds the very same members. The null prototype
stays because it costs nothing and is right; the assertion was deleted, because an assertion that
cannot go red is not evidence, it is decoration that later reads as coverage.

### The clone is taken at POST time

`worker.postMessage(v)` structured-clones `v` *immediately* and schedules delivery of the copy as a
macrotask. Cloning at delivery instead would pass the same round-trip assertions and still be wrong:
the page mutating its payload on the line after the post would change what the worker receives, and the
two sides would share state the spec says they do not. `G_WEB_WORKER` mutates deliberately on the next
line; cloning late flips `echo` and `mutated` together.

Messages posted between `new Worker(...)` and the end of script evaluation are **queued, not dropped** —
posting the job on the very next line is the normal shape, not an author error. `terminate()` is
immediate and final in both directions; letting one more queued message through resurrects the exact
work the page just cancelled.

### Loading, and what is honestly out of reach

`blob:` and `data:` URLs resolve **synchronously**, so the bundler shape
(`new Worker(URL.createObjectURL(new Blob([src])))`) starts in the turn it was constructed. A plain
`new Worker('/w.js')` goes over the network through `fetch`. `importScripts` is synchronous by spec and
there is no synchronous network here, so a pre-scan resolves literal-URL imports before evaluation;
a computed URL throws `NetworkError` rather than no-op'ing and leaving the symbol undefined.

**The divergence, stated rather than discovered: there is no second thread.** A worker that spins does
not keep the UI responsive. What this buys is that the work *completes* and the answer *arrives*, which
is the difference between a page that loads and one that does not — and it is why the constellation row
was **split** into `Web Workers (dedicated)` (gated) and `SharedWorker + worker parallelism` (missing)
rather than flipped to a green that would have overstated it. `SharedWorker` is left as the honest
load-failure stub, but one carrying a real `port` object: a shim that fires `error` and *then* TypeErrors
on `sw.port.postMessage` fails in the wrong place, before the page's own error path can run.

---

## Service Workers — the third side of a capability built over three ticks (tick 281)

Tick 279 built the service worker's **store** (the Cache API). Tick 280 built the **scope** a worker
script runs in. This is registration, the lifecycle, and `fetch` interception — the side that makes
the other two do anything on their own.

What a page loses without `navigator.serviceWorker` is not "offline mode". It is the whole PWA
branch, and on a growing number of sites it is **first render**, because the page awaits
`navigator.serviceWorker.ready` before it paints. Nothing throws; the page never arrives.

### The lifecycle IS the capability

`register()` → evaluate → `install` → **await every promise passed to `waitUntil`** → `activate` →
controlling. That await is not ceremony. `install` extending its own lifetime until the cache is
filled is the entire contract of an offline install step, and an implementation that skips it passes
every API-shaped assertion — registration resolves, both events fire, in the right order — while
serving from a cache it has not finished writing. The failure never appears at registration. It
appears as a **miss on the first offline load**, and it looks like a bug in the site.

`G_SERVICE_WORKER` makes that observable rather than assumed: the worker's install does its cache
write asynchronously inside `waitUntil`, and records *at activate time* whether the write had
finished. Dropping the await flips `waituntil` **alone** — `installed`, `activated`, `order`,
`controller`, `ready` and `intercepted` all still pass. That is the whole reason the claim exists.

### Interception, and the recursion that hangs

The page's `fetch` is wrapped. Every call passes through the active worker's `fetch` handlers first;
if one calls `respondWith`, that is the response and the network is never touched.

**`networkFetch` is captured before the wrapper is installed.** A service worker calling `fetch`
inside its own handler — which the cache-first pattern does on every miss — must reach the network,
not re-enter the wrapper. That is unbounded recursion whose symptom is a hang rather than an error,
and it is the easiest way to get interception wrong.

Two smaller rules that are load-bearing:

- **`respondWith` is recorded synchronously during dispatch.** A handler that calls it after an
  `await` has already lost the race in a real browser, so deferring the check here would make us
  accept code that is broken everywhere else.
- **A handler that responds with `undefined` throws rather than falling back.** Falling back would
  hide the page's bug and make a broken cache look like it worked.

A declined request falls through. Proving that offline cannot mean waiting for a response, so the
gate has the worker record every URL it is asked about and serve the list back on a third URL: the
assertion is that the handler *ran* for the declined URL and *did not respond*, observed from the
only side that can see it.

### Why the worker internals are published on one object

`G.__manukWorkerInternals` exposes the dedicated worker's `sourceOf`, `evaluate` and — the reason it
exists — its **DOM deny-list**. A service worker is a worker scope plus a lifecycle plus
interception; had it grown its own copy, the deny-list would end up enforced in one place and not the
other, and the drift would show up as a service worker that can see `document`. The two scopes share
one array on purpose.

**Not implemented, and absent rather than wrong:** navigation interception, the update/redundant
lifecycle, `clients` beyond a stub, push, background sync, and scope matching past a path prefix.

## Blob object-URLs carry real bytes — `canvas.toBlob` + `blob:` fetch (tick 284)

`URL.createObjectURL(blob)` is how a page moves bytes it generated itself back into the loading
machinery: an image editor's "save", a chart library's PNG download, an upload preview
(`URL.createObjectURL(file)` → `img.src`), and every `canvas.toBlob(b => fd.append('file', b))`. Two
halves have to both work, and before this tick neither did for a real content Blob.

### `canvas.toBlob` decodes the one raster `toDataURL` already produced

The old `el.toBlob` called `cb(null)`. That is not a harmless stub — `null` is exactly what a real
browser returns for a **tainted** cross-origin canvas, so a page testing for that took the
"cannot-export" branch and silently refused to save a canvas it fully owned, with no error thrown.
The bytes already existed: `__cvToDataURL` rasterises what was drawn to a real
`data:image/png;base64,…`. `toBlob` decodes **that one representation** (`atob` → a Blob) rather than
minting a second raster path that could drift from `toDataURL`. It reports the type it actually
encoded — always `image/png` — and **ignores the requested `type` argument** rather than label PNG
bytes `image/jpeg`. It fires the callback on a microtask, never inline, because the spec is async and
a page that reads a variable the callback sets would otherwise find it undefined.

### `blob:` resolves in `fetch` against the one object-URL registry

A `blob:` URL names an in-process Blob, not a network resource, so `globalThis.fetch` short-circuits
it before the host round-trip: it looks the URL up through `__mseLookup` — the **same** registry the
MSE attachment handshake and the Worker `sourceOf` already read — and, when it finds a Blob, resolves
a `__makeResponse` from the Blob's byte-string. The byte-string is passed as both `text` and `raw`;
`raw` is the binary channel (`__bodyBytes` copies each code unit as a byte, no encoder), so a PNG
survives `.arrayBuffer()`/`.bytes()`/`.blob()` unmangled. A `blob:` URL that was revoked, never
registered, or names a non-Blob (a MediaSource) is a `TypeError('Failed to fetch')` — a stale object
URL is a network error in a real browser, not an empty 200.

There is exactly **one** object-URL store. Minting a second in `dom_bindings` (where `URL` lives)
would have been the tidier place, but `mse_js` already owns the registry and is installed
unconditionally; a second store is the drift bug (a URL registered in one, looked up in the other)
this project keeps refusing. `createObjectURL` there already stored arbitrary objects — the tick only
taught the readers to accept a Blob, not just a MediaSource.

GATE: `blob_object_urls_carry_real_bytes_through_fetch` (manuk-page, G_BLOB_URL). PROVEN RED two ways:
the `cb(null)` stub drops `toblob`/`type`/`sig`/`roundtrip`; deleting the `blob:` fetch branch leaves
every upstream claim green and fails only `sig`/`roundtrip` (the fetch hits the network and rejects).
`revoked:true` (the second fetch, after `revokeObjectURL`, rejects while the first succeeded) makes
the two halves exact complements, so no constant satisfies it.

**Not implemented, and absent rather than wrong:** `<img src="blob:…">` / `<a href="blob:…">` visual
rendering (the Rust image-fetch path does not yet consult the JS registry — the next slice); `blob:`
resolution in `XMLHttpRequest` (the modern `fetch` path is wired, legacy XHR is not); and `toBlob`
encoding any format other than PNG.

## `scheduler.postTask` — priority-ordered main-thread work (tick 293)

The scheduler modern frameworks use to keep the UI responsive: `scheduler.postTask(cb, { priority })`
runs work at `user-blocking` > `user-visible` > `background`, so a click handler pre-empts a background
prefetch. React's scheduler, cooperative-yielding loops and `scheduler.yield()` feature-detect it;
absent, `scheduler.postTask(...)` threw on `undefined`.

It is NOT an inert `setTimeout` alias — that is the failure mode the gate is built to catch. It honours
priority ORDER: same-turn posts collect (on one macrotask turn) and the drain runs the highest-priority
bucket first, so three tasks posted `background, user-blocking, user-visible` execute
`user-blocking, user-visible, background`. It also honours the `delay` option, rejects (and never runs)
a task whose `AbortSignal` fires before its turn, and returns a Promise of the callback's return value.
`scheduler.yield()` resolves after a macrotask turn.

### The teeth `G_SCHEDULER_POSTTASK` uses

`priority-order` (the order above — a setTimeout alias that ignores priority runs in post order and
fails), `value` (resolves the callback's return), `abort` (an already-aborted signal rejects the task
and it never runs). Deleting the block was demonstrated to make the first call throw before landing.
[[js-engine]]

## `DOMMatrix` — 2D affine transform math (tick 294)

`canvas.getContext('2d').getTransform()` returns one, charting and graphics libraries build transforms
with it, CSS Typed OM hands it back. It was absent, so `new DOMMatrix(...)` threw `DOMMatrix is not
defined`. This is a real, honest 2D implementation — not an inert stub, so the gate asserts computed
RESULTS (a wrong multiply or inverse is caught, not just the presence of the method).

Constructs from nothing (identity), a 6- or 16-element array, or a `matrix(a,b,c,d,e,f)` string.
Exposes `a`–`f` plus the `m11`/`m12`/`m21`/`m22`/`m41`/`m42` aliases, `is2D`, `isIdentity`. The
transform methods — `multiply`, `translate`, `scale`, `rotate`, `inverse` — are NON-mutating (they
return a new matrix, matching the spec's `*Self`-less forms), `transformPoint({x,y})` applies the affine
map (`x' = a·x + c·y + e`, `y' = b·x + d·y + f`), and `toString` / `toFloat32Array` / `toFloat64Array`
serialise. `DOMMatrixReadOnly` aliases it and `DOMMatrix.fromMatrix` is the copy constructor.

**Honest limit:** 2D only. The 3D components (`m13`–`m44`, `is2D:false`, `rotateAxisAngle`, perspective)
are not modelled — the overwhelmingly common case on the web is the 2D affine matrix. [[js-engine]]

## `DOMPoint` — the geometry point that pairs with DOMMatrix (tick 295)

The point half of the geometry pair: canvas / graphics code maps a coordinate through a transform with
`point.matrixTransform(matrix)`, and `matrix.transformPoint(point)` returns one. It was absent, so
`new DOMPoint(...)` threw. `{x, y, z, w}` with `w` defaulting to `1` (a position, not a direction);
`matrixTransform(m)` applies the 2D affine map, `fromPoint` copies, `toJSON` serialises.
`DOMPointReadOnly` aliases it.

Adding it closed a small honesty gap in `DOMMatrix.transformPoint` (tick 294): it returned a bare object
literal; it now returns a REAL `DOMPoint`, so a caller can chain `.matrixTransform(...)` or read `.w`.
[[js-engine]]

## `DOMQuad` — four points, the enclosing box (tick 296)

The shape `element.getBoxQuads()` and transform code produce when a rectangle has been rotated or skewed
into a general quadrilateral — its corners are no longer axis-aligned. It completes the geometry family
(`DOMMatrix`/`DOMPoint`/`DOMRect` were already here). It was absent, so `DOMQuad.fromRect(...)` threw.

Four `DOMPoint`s (`p1`–`p4`), `DOMQuad.fromRect({x,y,width,height})` (corners clockwise from top-left),
`fromQuad`, `toJSON`, and `getBounds()` — the axis-aligned `DOMRect` bounding box (min/max over the four
points), which is the useful reduction after a transform has skewed the corners. [[js-engine]]

## Error.stackTraceLimit — the property is a shim, the behavior is an honest no (tick 400)

Audit #13 flagged it as a one-line probe; the probe (G_PROBE_CAPABILITIES `stacklimit`) measures
BEHAVIOR — set the limit to 3, recurse 20 deep, count `.stack` frames — because the prelude
already defines the PROPERTY (`typeof` is `'number'`, event_loop.rs shim) and pinning typeof
would be the t195 inert-stub lie. Measured: no truncation — our SpiderMonkey predates the
Firefox-153 implementation of this V8-ism. Pinned `stacklimit:no`; it flips WITH a mozjs bump
that carries the capability, never by retuning the probe (the honest-answer≠fixed-answer rule).
Code that WRITES the property (Sentry, error-reporting SDKs — the common case) works today; only
code that depends on the cap taking effect sees longer stacks than requested.

## document.location is window.location — and the alias must be an accessor (tick 402)

`window.location` had been a full Location shim for hundreds of ticks; `document.location`
— which the spec defines as the SAME object — was never aliased, and `history_bindings::
install` (which carries a native read-only Location) turned out to be dead code: nothing
outside its own tests calls it. The live BOM surface is the WINDOW_PRELUDE shim, period.

The shape constraint: `__applyUrl` REPLACES `g.location` wholesale on every SPA navigation,
so `document.location` must be an accessor (`get → g.location`) — a copied reference goes
stale on the first pushState. Assignment navigates via `__applyUrl` (the legacy redirect
idiom). `document.URL` and `document.documentURI` are read-only spellings of the live href
and were ALSO absent. All three in one prelude block; G_DOCUMENT_LOCATION asserts identity,
the post-pushState swap-tracking, and assignment-navigation. Found as a NAMED console error
by the t401 re-keyed oracle (okta Identity components die reading document.location.search
in their async mount). Post-fix: the rejection is gone, okta missing 128→117; total scored
diffs RISE (523→795) because subtrees that never existed now mount and get scored — the
instrument seeing more is the fix working, not a regression.

## getPropertyValue is total — a string for every input (tick 403)

The computed-style snapshot's accessor did `return this[m[p]||p]` — undefined for anything
outside its hand map/property list — and the no-style fallback was a bare `({})` with no
accessor at all. CSSOM's contract is total: the serialized value for supported properties,
`''` for everything else (unknown, unlisted, custom). Fixed with never-undefined coercion +
a generic kebab→camel fallback (the hand map stays, for the irregular names), and the empty
fallback now carries `getPropertyValue(){return ''}`. Found as the t401 oracle's second named
error (okta: `.getPropertyValue(x).trim()` threw in an async frame). G_GET_PROPERTY_VALUE.

## document.currentScript — a thread-local, set per classic evaluation (tick 404)

Was hardcoded null (the doc comment argued null-beats-undefined — right about the idle case,
silent about the executing case). Now: CURRENT_SCRIPT thread-local (the CURRENT_DOM lifetime
discipline — set immediately before each classic evaluation, cleared after, never held across
a pump), read by doc_get_current_script via return_node_or_null. Modules never set it (spec).
run_one_script carries the NodeId; all three execution loops (blocking, deferred/inserted,
run_scripts) participate. G_CURRENT_SCRIPT asserts per-script identity (each of two scripts
sees ITSELF), attribute readability (okta's .hasAttribute call), and module-null. Third named
error from the t401 instrument converted to a capability.

## structuredClone preserves binary types — the plain-object degrade is silent corruption (tick 421)

`structuredClone` is a host shim (`g.structuredClone`, dom_bindings.rs), not a SpiderMonkey built-in,
and `postMessage` serializes through it (event_loop.rs). It walks the graph with a `seen` map so cycles
are legal (unlike JSON), and clones arrays, `Date`, `Map`, `Set` and plain objects. The gap it closed
at tick 421: **binary data.** A typed array is `typeof === 'object'` and not any of those, so it fell
into the generic object-copy branch and returned `{0:.., 1:.., length:..}` — right keys, wrong type,
zero errors. Every byte-oriented consumer (WASM loader, `crypto.subtle`, a `postMessage`d transferable)
then read garbage.

The fix adds, before the object fallback: `ArrayBuffer` → `slice(0)` (an independent copy); any
`ArrayBuffer.isView` (typed array or DataView) → clone the backing buffer **through the same `walk`**
and re-view it (`new x.constructor(buf, byteOffset, len)`), so two views SHARING one buffer clone to two
views over ONE buffer — the `seen` map keyed on the ArrayBuffer is what preserves that identity, exactly
as the structured-clone spec requires; and `RegExp` → `new RegExp(source, flags)`. Typed arrays take an
element COUNT (`x.length`), a DataView a byte length — distinguished by `BYTES_PER_ELEMENT`.
**Honest limit:** `Blob`/`File`/`SharedArrayBuffer` are still not cloned (SAB is cross-origin-isolated
only; Blob/File match the IndexedDB encoding's known gap), and transfer-list transfer (vs copy) is not
modelled.

## putImageData is a raw pixel blit, ImageData is the buffer libraries build (tick 423)

The canvas 2D backing store is a `tiny_skia::Pixmap` per `<canvas>` node (engine/js/src/canvas.rs,
keyed by NodeId). `getImageData` reads it (demultiplying to straight-alpha RGBA); tick 423 added the
write half. Two facts shape it:

- **`putImageData` is NOT a draw.** The HTML spec says it ignores the current transform, `globalAlpha`,
  `globalCompositeOperation`, shadows and clipping — it *replaces* the pixels in the destination
  rectangle. So `canvas::put_image_data` bypasses `Paint` and assigns premultiplied pixels directly into
  `Pixmap::pixels_mut()` (the exact inverse of `get_image_data`), then marks the node dirty. Routing it
  through `fill_rect` would have applied the transform and alpha and quietly corrupted every filter.
- **The dirty-rectangle overload is resolved on the JS side.** `putImageData(img, dx, dy, dirtyX,
  dirtyY, dirtyW, dirtyH)` clips the source sub-rect (handling negative extents) and passes the narrowed
  window to the FFI, so the native side has one simple `(x, y, w, h, data)` signature. The pixel array
  crosses as a **plain `Array`**, not the `Uint8ClampedArray`, because the shared `arg_f32_array` reader
  uses `GetArrayLength`, which only sees JS arrays.

`ImageData` is the constructor global libraries build pixels into before blitting: `new ImageData(w, h)`
(zeroed) or `new ImageData(Uint8ClampedArray, w[, h])` (adopt, infer height). Its absence threw
`ImageData is not defined` and killed the pipeline on line one; `get`/`createImageData` now return real
instances of it. **Honest limit:** `createImageBitmap` from an `ImageData`/`Blob` source still rejects
(no decode-to-pixels path); this tick is the CPU-side pixel buffer, not a GPU upload.

## TextDecoder honours its label (windows-1252 + utf-16), not UTF-8-for-everything (tick 424)

There are TWO decoding paths and they are not the same. The HTTP layer decodes a response BODY with
`encoding_rs` (the full legacy set: Shift_JIS, GBK, …). The JS `TextDecoder` object (event_loop.rs) is
what a script calls directly on bytes it already holds — and until tick 424 it ignored its `label` and
always decoded UTF-8. The fix normalises the label in the constructor to `__enc` and branches `decode()`:

- **windows-1252** — the `latin1`/`iso-8859-1`/`windows-1252` alias family. A single-byte map: 0x00-0x7F
  and 0xA0-0xFF are the code point equal to the byte; 0x80-0x9F are the CP1252 symbol block (`€ ' " — …
  ™`), which is what makes it windows-1252 rather than raw Latin-1. No streaming state (one byte/char).
- **utf-16le / utf-16be** — two bytes per code unit, emitted as-is (surrogate pairs are already the JS
  string's native form); a trailing odd byte is held under `{stream:true}`.
- **utf-8** (default) — unchanged, including the incomplete-trailing-sequence hold for `{stream:true}`.

**Honest limits:** the full legacy CJK set (Shift_JIS/GBK/EUC-KR/Big5) is NOT in the JS decoder — those
still only decode correctly through the HTTP-layer `encoding_rs` path; an unknown label falls back to
UTF-8 (lenient) instead of the spec's `RangeError`, so nothing that previously limped newly throws;
`fatal`/`ignoreBOM` are stored but BOM stripping is not yet applied.

## test262, and the three defects the FIRST run found before it produced a number (tick 546)

`docs/loop/CONSTELLATION.tsv` carried `?` on test262 from surface audit t83 to tick 545, under a
sentence that reads like a reason not to bother: *"we embed SpiderMonkey, so this should be cheap and
high."* Both halves were wrong in the informative direction. It was not cheap — a runner is real work
— and the number is not automatically high, because **most of what test262 measures about an
embedded engine is what the EMBEDDER did**, and nobody had ever asked.

**The number, and both halves of it.** 94.14% of 87,009 executed subtests (81,908 pass), and 81.41%
of the 100,617 the ratified suite defines, because 13,608 are skipped with named reasons. Quote the
second one next to the first or don't quote either: a runner that skips more and reports a *higher*
pass rate is the exact failure this pair exists to expose.

### The runner: `manuk-wpt test262` (`tests/wpt/src/test262.rs`)

One file is **one or two subtests** — sloppy *and* strict unless `flags` says otherwise — which is why
51,922 files define ~100k subtests and why quoting the file count understates the suite. Frontmatter is
read by hand rather than with a YAML crate, because `info:` is a free-form block scalar full of spec
prose that is regularly not valid YAML in isolation, and a parser that *errors* there silently
mis-classifies the case. Harness order is test262's own (`assert.js`, `sta.js`, includes, body), and a
**missing harness file aborts the case instead of running it bare** — `assert.sameValue` with no
`assert.js` throws `ReferenceError`, which would have been filed as an engine defect.

`--limit` samples with a **stride across the sorted suite**, not off the front: `annexB/**` sorts first
and is the least representative directory there is. Same lesson `scripts/fidelity-sweep.sh` records in
its own header.

### Three defects, in the order the run hit them

**1. `new FinalizationRegistry(fn)` SEGFAULTED the process** — and `typeof FinalizationRegistry` was
`"function"`, so every feature detector on the web said yes. Not `MOZ_CRASH`: a null dereference. The
constructor asks the host for the *incumbent global*, that question routes through `JS::JobQueue`, and
`SpiderMonkeyRuntime` installed no queue. **Scope, precisely:** the page path was already safe
(`event_loop::install` calls `job_queue::install_once` when it builds a document's global), so this was
never a tab crash — it was the bare [`JsRuntime`] seam that `manuk eval` and any embedder uses. The
real defect is that *one of two constructors of the same engine set the host up and the other did
not*, and nothing said so. Both now go through `install_host_hooks`.

**2. The runtime would not say what threw.** `eval`'s error was the literal string
`"uncaught exception while evaluating <file>"` — the same eleven words for a syntax error on line 1,
a `TypeError` from a missing IDL property, and an OOM. This crate's own
`dom_bindings::pending_exception` has reported the real message for ticks (*"every swallowed exception
is a discarded bug report"*); the **runtime's own eval was the one path still swallowing it.** It is
also load-bearing for conformance: a negative test's verdict is *which* error type was thrown, so
~4,000 cases would have been scored on a coin flip. ⚠ **The read must happen inside the script's
realm** — `evaluate_script` has already left it when it returns, and `JS_GetPendingException` outside a
realm does not fail, it **aborts**. The `JSAutoRealm` in that arm is not tidiness.

**3. A batch embedder leaks a global per `eval` — 14.7 GB RSS at 14,000 evaluations.** `eval` creates
a fresh global and realm per call (that isolation is a feature: one test's damage cannot reach the
next), and SpiderMonkey's incremental GC never gets a chance against a loop that only evaluates. The
first full run did not fail, it *swelled* — 100% CPU, RSS climbing, indistinguishable from a hang. So
`JsRuntime` gained `fn gc(&mut self)` (default no-op) and the runner calls it every 250 files: peak
RSS 3.0 GB, whole suite in 140 s. **A metric that cannot survive its own suite is not a metric**, and
the shape of that failure — slow, then heavy, then dead — is worth recognising on sight.

### What the 5,101 failures are, and what they are not

Clustered by area, because 5,101 failures are never 5,101 bugs: `intl402/Temporal` 1,956 (a Stage-3
proposal), `Atomics` + `SharedArrayBuffer` 718 (**the embedder must enable shared memory** — this is
ours, not SpiderMonkey's, and it is what wasm threads need),
`DisposableStack`/`AsyncDisposableStack`/`SuppressedError` 360 (explicit resource management),
`ShadowRealm` 114. So the top ~3,100 are *four* named causes, three of which are "this proposal is not
turned on", and one of which is a real embedder capability with a real dependency behind it.

### The skips are the honest part, and one of them is Bar 0

10,739 **async** (`$DONE` needs a `print` host function and a microtask drain), 1,642 **module-goal**
(the loader is not on the eval seam), 1,225 **host-API** (any reach for `$262` at all — the member-name
list let 33 subtests through to fail with `$262 is not defined`, which is our missing host object
recorded as the engine's defect). And **2 subtests for a measured hang**:
`RGI_Emoji.js` runs `/^\p{RGI_Emoji}+$/v` over the whole Unicode space, at 100% CPU, and did not
finish in four minutes. We cannot say whether it is slow or non-terminating — **and the reason we
cannot is the finding**: there is no `JS_AddInterruptCallback` on this engine, so a synchronous script
cannot be interrupted, deadlined, or asked how far it got. `STATUS.md` has carried *"production
interruptibility (a cancellable long task) is still not built"* under Bar 0 for hundreds of ticks.
This is the first instrument that walked into it and could not walk back out, and with an interrupt
callback `SLOW_CASES` stops being a list and becomes a per-case deadline. [[conformance-and-oracles]]

## An interface object is defined IFF the thing it names exists (tick 608)

`globalThis` carries ~183 **interface objects** — `HTMLMetaElement`, `Navigator`,
`HTMLTableCellElement`, `CanvasRenderingContext2D`. They look like decoration and they are not, for
one mechanical reason:

> **Reading an absent global is a `ReferenceError`, and a `ReferenceError` kills the frame that read
> it.** It does not return `undefined`, it does not degrade, and there is no `?.` a page can write to
> survive it.

That is a different failure class from a missing *method*. `el.foo?.()` survives a missing method;
nothing survives `HTMLMetaElement` not existing, because the throw happens at the *identifier read*,
before any operator the author could have guarded with.

### What it cost, measured

`www.welt.de` (top-1k, in `docs/bench/corpus-v2.tsv`) scored **0.0% coverage — 3,242 of 3,243
elements missing**. Not "rendered badly": rendered *not at all*. The console named it:

```text
ReferenceError: HTMLMetaElement is not defined
ReferenceError: Navigator is not defined
Failed to load website due to adblock: Loader aborted: HTMLMetaElement is not defined
```

The site's loader probes interface objects; the probe threw; **the site concluded it was being
ad-blocked and aborted its own boot.** The engine rendered nothing because the page *decided* to
render nothing. A coverage number says how much is missing and never says why — only the console did.
This is a general hazard of scoring by box-diff: `0.0%` and `we are slow` are indistinguishable in the
metric, and the two want opposite fixes. (t606's pilot had in fact filed this site under *timing*.)

### The rule, and the negative half is the load-bearing half

An interface object is installed **iff the thing it names exists in this engine.** Every name added at
t608 was probed present first (`navigator`, `localStorage`, `performance`, `customElements`,
`crypto.subtle`, `document.implementation`, the 2D context). **`OffscreenCanvas` is deliberately
absent** — `getContext` has no offscreen tier, so `'OffscreenCanvas' in window` must keep answering
`false`. A stub naming a capability we lack defeats feature-detection and is *worse than the gap*
(`DAILY-DRIVER-CERTIFICATION.md` §1). `G_IFACE_SURFACE` asserts that absence, so the list cannot
quietly become a claim instead of a fact.

### Predicates are exact, not generous

`iface(name, test)` answers `instanceof` via `Symbol.hasInstance` rather than a prototype chain — our
reflectors do not have one to hang these off, and the question frameworks ask ("is this an input?") is
answerable directly. An **over-broad predicate is a wrong answer, not a generous one**: `<cite>` is
plain `HTMLElement` and not `HTMLQuoteElement`; `<my-widget>` is `HTMLElement` and not
`HTMLUnknownElement`; `HTMLTableCellElement` is one interface over both `<td>` and `<th>`. The gate
carries nine `NEG_*` claims because without them a predicate that simply returned `true` would satisfy
every positive claim in the file.

### Known residue, named rather than papered over

`CanvasRenderingContext2D.prototype` is **not in a context's prototype chain** — `getContext` builds a
fresh object carrying own methods — so patching `CanvasRenderingContext2D.prototype.fillText` is
accepted and **inert**. The interface object is honest (canvas 2D really does rasterize); the patch
path is not built. That is the [[dom-semantics]] `G_PROTOTYPE` lesson recurring on the canvas surface.
Nine names remain absent, each blocked on a capability rather than on the list:
`IDBFactory`/`IDBDatabase`/`IDBRequest`, `TextTrack`/`TextTrackCue`/`VTTCue`, `DOMStringMap`,
`MessageEvent`, and `OffscreenCanvas` (by design).

### The layering, which is the part to expect next time

Fixing this did **not** make welt.de render. It removed *one* abort and revealed the next:
`TypeError: setting getter-only property "innerText"`, with the same adblock-abort handler catching
it. Boot-path failures on real sites stack, and each fix peels one layer — the same shape the
aljazeera investigation took. **Do not book "site X now works" from "site X's first error is gone".**

## The event-loop drain is bounded by the CLOCK, not only by a task count (t610)

`MAX_TASKS_PER_DRAIN = 20_000` has capped `run_deferred` since Bar 0 was written, and its stated
purpose is a wall-clock one: *"the alternative is a frozen tab."* But **a count is a poor proxy for a
clock**, and the gap is not marginal. Measured per drain, with the clock bound disabled:

```text
  en.wikipedia.org       1 task,      4ms      ← a page that converges
  mangago.me         20000 tasks, 30216ms      ← and it did this FIVE times in one load
  theguardian.com                     did not finish inside 480s
```

Four orders of magnitude. The same declared policy — *"this page is not converging; paint what we
have"* — handed one page 4ms of grace and another half a minute of it, five times over, for reasons
unrelated to what either had actually cost the user. On `mangago.me` the drain was the **single
largest segment of an 85s load**, larger than every network phase put together.

**The effect, two runs per arm on an idle box (`mangago.me`):**

```text
  budget=0 (shipped)    did not finish in 400s   ·   114874ms, visual 0.2%
  budget=5000            35743ms, visual 0.9%    ·    36863ms, visual 0.2%
```

**Latency 3-11x and stable; visual unchanged.** That pairing is the whole claim, and it is what makes
this *not* the North Star's *"fast because we never ran the script"* trap — argued from what was
already true rather than from a fidelity number. **These pages were already being cut**, by the
20,000-task ceiling, just 30 seconds per drain later. The capability outcome is identical by
construction, which is precisely what an unchanged visual score shows independently. All that changed
is **how much of the user's clock a page spends proving it will never converge.**

> ⚠ An earlier draft of this page claimed `theguardian.com` went **22.9% → 52.4% visual** when
> bounded — that bounding script execution *raised* fidelity. **It does not reproduce** and has been
> retracted: guardian does not reliably finish inside 480s in *either* arm, so nothing can be
> differenced across them, and the original reading was taken beside a running release build. A
> single A/B point on a live third-party site is a hypothesis, not a result.

**The two bounds are complementary and both are kept.** A tight self-rescheduling loop of cheap tasks
trips the *count* first; a handful of expensive ones trips the *clock* first. Whichever fires, the
page has already demonstrated it is not converging.

**Checked only on the task boundary.** The budget bounds a runaway *chain* — which is all the count
ceiling was ever for — and never preempts a single running task. JS is not interrupted mid-flight.

### `run_with_fetcher` had no bound at all

`run_deferred` was capped; `run_with_fetcher` — which `run()` delegates to and which `dom_bindings`
drives — had **neither a count ceiling nor a clock**. The exact runaway `MAX_TASKS_PER_DRAIN` exists
to forbid ran forever there. Its `did_io` arm makes it worse rather than better: *"a delivered result
may have scheduled more work"* `continue`s past the task check unconditionally, so a page that
re-fetches on every delivery loops indefinitely. **One rule, two implementations, one of them
enforced** — §VI.3's fourth clause, booked again. Both bounds now apply to both loops.

### What the gate had to prove, and the draft that was green for the wrong reason

Any bound on script execution is one bad constant away from *"fast because we never ran the script"*,
so `G_DRAIN_BUDGET`'s load-bearing claim is the **negative**: a page doing 5,000 real, converging
tasks runs every one of them and lands all 5,000 nodes.

The first draft of the gate asserted *"the runaway load finishes in under 20s"* — and **it passed with
the clock bound deleted.** A cheap self-rescheduling task hits the 20,000-task ceiling quickly, so
the *count* was doing the stopping and the gate never exercised the clock at all. The RED patch is
what caught it; the assertion had looked entirely reasonable.

The rewrite compares **arms instead of constants**: the same runaway page is loaded with the budget
disabled and then with it set, and the gate compares how many spins each got. That is why
`max_drain_ms()` reads its env on every drain rather than memoising into a `OnceLock` — the same
reasoning `g_load_document.rs` sets out. A spin-count comparison is also machine-independent in a way
a millisecond threshold on a loaded build box is not. With the bound removed the two arms read
**80001 and 80001**, and the gate says so.

## A feature-detect that THROWS is worse than one that answers no (tick 615)

`HTMLScriptElement.supports(type)` was `undefined`. A page calls it to decide how to load **its own
code** — an ES-module bundle or a classic fallback, an import map or a hand-rolled resolver. Calling a
static that does not exist is:

```text
  TypeError: HTMLScriptElement.supports is not a function
```

So the page does not take the fallback branch — **it dies at the feature-detect.** For a call whose
entire purpose is to let a page degrade gracefully, that is the worst available outcome, and it is
strictly worse than answering `false`: `false` sends the page down a path it has already written.

**The general shape, which applies to every detect surface:**

> A missing *feature* costs you the feature. A missing *feature-detect* costs you the **fallback** —
> and the fallback is the thing that was going to work.

So a detect API is worth implementing **before** the capability it reports on, and it must be
implemented to return the honest answer rather than the absent one. The same reasoning already applies
to `CSS.supports` (t576/t591, where answering "does it PARSE" promised 31 properties we never render)
and to `MediaSource.isTypeSupported`.

### The answers, and why one of them is deliberately `false`

| type | answer | backing |
|---|---|---|
| `classic` | `true` | classic scripts run |
| `module` | `true` | real resolve hook + cycle-safe graph walk (t512-516) |
| `importmap` | `true` | import maps land on the same path |
| `speculationrules` | **`false`** | not implemented — and the page is *asking to be told no* |

`speculationrules` is the load-bearing one. A page that asks about it does so in order to prefetch by
itself if the answer is no. A flattering `true` means **the page stops and we never start** — a
capability claimed is a capability nobody provides. `[[honest-answer-is-not-a-fixed-answer]]`

### Where it was found, and what it did not fix

Third rung of `www.welt.de`'s anti-adblock chain, after t612's `innerText` setter and t613's XHR
EventTarget. Clearing it did **not** make the site render — the chain continues to
`Error: Failed to execute packing script`, with one visible error left: a `<script type=module>`
request answered with **HTML** (`SyntaxError: expected expression, got '<'`). Three real
TypeError-class gaps found, each justified by its own measured corpus population, and the site still
blank. An arc that peels one layer per fix has to be able to say that, or it becomes a sunk-cost march.

## A module's imports resolve against the MODULE's url, not the document's (tick 617)

`fetch_external_scripts` inlines a `<script src>` into its node and drops `src`. By the time
`prefetch_module_graph` walks the DOM, an **external** module is indistinguishable from an inline one —
and both halves of the module machinery resolved its imports against the **document**:

```rust
// engine/page: the pre-fetch walk
queue.push_back((base_url.clone(), spec));            // base_url = the DOCUMENT url

// engine/js: the root module's SpiderMonkey private
let base = DOC_URL.with(|u| u.borrow().clone());      // …the DOCUMENT url again
```

Both sat under comments that state the correct rule. The page-side one:

> *"the document URL, since an inline module resolves its relative imports against the document"*

— true, and applied to a case it does not cover. And the JS-side one is even more explicit:

> *"A root inline `<script type=module>`'s base IS the document url; a **fetched** module's base is its
> own url — which is exactly why the answer must live on the module, not in one per-document slot."*

**Two comments, in two crates, both correct, both above code doing the opposite.**

### What it cost, measured

`www.welt.de`'s entry module is `/assets/bff-section/scripts/section.module.BPEBKMaY.js` and it imports
`./chunks/react.BPdhuoKc.js`:

```text
  against the SCRIPT    /assets/bff-section/scripts/chunks/react.BPdhuoKc.js   200 ·   8,391 B · JavaScript
  against the DOCUMENT  /chunks/react.BPdhuoKc.js                              404 · 414,112 B · HTML
```

One directory tree too high. And the wrong URL does not fail cleanly — the origin's SPA fallback
answers with **HTML**, which compiles as a module: `SyntaxError: expected expression, got '<'`.

**This is the shape of every Vite/Rollup/esbuild production build**: an entry module under a hashed
asset directory importing its chunks relatively. The class is "sites that ship modern bundled
JavaScript".

```text
  www.welt.de     COVERAGE 0.0% → 94.9%    SHAPE 0.0% (n=1) → 66.9% (n=3063)    verdict BELOW → ok
```

### The mechanism: the DOM cannot answer, so something else must

Once `src` is gone the DOM has no record of where a script came from. `fetch_external_scripts` now
returns `node → origin url` alongside its authorized set; the page carries it as
`Page::module_node_bases` and seeds it into the JS layer beside `MODULE_GRAPH_SOURCES`, on the same
seam and cleared at the same time. `run_module` takes the node and prefers that base over `DOC_URL`.

⚠ **Fixing one half does nothing.** The pre-fetch decides which URLs are *fetched*; the module private
decides which URLs the loader *asks for*. They must agree, and a fix to either alone looks complete
until you run it. Both are independently RED-proven in `G_MODULE_BASE_URL`.

⚠ **A gate that has never been seen to pass is not evidence that the code is broken.** When the gate
still failed after the page-side fix, moving the *document* into the script's directory — where both
resolutions coincide — made it pass, which proved the harness worked and the remaining failure was
real. That diagnostic is what pointed at the second site instead of at the test.

## A dynamic import's module must OUTLIVE the hook that made it (tick 624)

`import()` needs a `HostImportModuleDynamically` hook; without one SpiderMonkey rejects every call with
*"Dynamic module import is disabled or not supported in this context"*. Installing one is the easy
half. The half that cost a parked tick is a **lifetime**.

`run_module` used to clear the compiled-module registry the instant the ROOT was linked and evaluated:

```rust
if have_graph { esm_registry_clear(); }   // "SpiderMonkey's own records keep the graph alive now"
```

Correct **for static imports** — once `ModuleLink` has run, the engine's module records hold the graph
through the still-rooted root. **Wrong for dynamic ones.** `FinishDynamicModuleImport` completes the
caller's promise in a **later microtask**, and the module must still be returned by the resolve hook at
that point. The API header says so, in a sentence that reads as being about registration and is
actually about lifetime:

> *"If successful, **after calling FinishDynamicModuleImport()** the module should be returned by the
> resolve hook when passed |referencingPrivate| and |moduleRequest|."*

So the module was compiled, linked, evaluated, registered and exception-free — and **deleted between
the hook returning and SpiderMonkey asking for it.** The symptom is a promise that rejects with
`undefined`, which looks like nothing in particular.

The clear now runs at the end of the **script pass**, beside where `MODULE_GRAPH_SOURCES` is dropped.
The GC contract is unchanged in substance — *the registry must never outlive the NAVIGATION*, so
nothing pins a dead realm's modules — and it is simply later than the last moment a dynamic import can
need it.

### Why a synchronous hook is honest here

There is no synchronous network on the JS thread, so the module has to be in hand already — and it is:
the page pre-fetches the reachable graph before any script runs. `scan_static_import_specifiers` now
also collects **literal** `import("…")` specifiers, which it deliberately skipped while nothing could
execute one. A **computed** specifier still cannot be seen by a textual scan and **rejects**, which is
the honest answer: the page's `.catch()` is what such a page already relies on, and a promise that
never settles is strictly worse than one that rejects.

### ⚠ A hook installed at one of two sites looks exactly like no hook at all

There are **two** `SetModuleResolveHook` call sites and the page path uses the second. Installing the
dynamic hook at only the first reproduces the pre-hook symptom exactly — *"Dynamic module import is
disabled"* — so the natural reading is "my change did not apply", not "my change applied halfway".
§VI.3's fourth clause from the other direction: not two implementations of one rule disagreeing, but
one implementation registered in one of two places.

### The three symptoms, and what each one means

| break | symptom |
|---|---|
| no hook installed (or only at one site) | `"Dynamic module import is disabled or not supported"` |
| registry cleared too early | `rejected:undefined` |
| scanner does not collect the specifier | the promise **never settles** |

`G_DYNAMIC_IMPORT` names all three in its assertion messages, because the first and third are easy to
mistake for each other and the second looks like nothing.

## Top-level await interleaves, and a cycle links with live bindings (tick 636)

`? ESM top-level await + cyclic module records` sat on the constellation as a named **Interop 2026**
web-compat item. Surface audit #34 had refused to book it `works` on the t608 probe, on the grounds
that *"async/await PARSES"* is not the claim — top-level await is module-only syntax and
multiple-TLA ordering needs a real module graph. That refusal was right, and this is the evidence it
was holding out for: **both halves already worked end-to-end on the real page path.**

**The first draft of the probe nearly published a false absence, and the CONTROL is the only reason
it did not.** Driven with `Page::load` + `take_fetches`, the graph printed `-` — nothing ran, which
reads exactly like *"top-level await is unsupported"*. Running the identical graph with every
`await` deleted **also** printed `-`. Same instrument, no TLA, same nothing: so the instrument was
wrong, not the subject. An external module graph is pre-fetched by **`Page::load_async` and by
nothing else**.

> A negative result feels like it needs no confirmation, and that asymmetry is the defect. The cheap
> version of "name the code path that would deliver this" is **run the control**: the same
> measurement with the feature under test removed. If the control fails too, you were measuring the
> harness.

**Asserting "the module ran" would have been vacuous, and the fix is an ORDER INVERSION.** An engine
that ignored `await` at module scope, or ran modules synchronously in declaration order, satisfies
every "it didn't throw" check. So the two async modules carry **different numbers of awaits** and
stamp a shared counter on completion:

| module | import order | awaits | finishes |
|---|---|---|---|
| `/tla.js` | first | 3 | **second** (`tick:2`) |
| `/tla2.js` | second | 1 | **first** (`two:1`) |

Real async-module semantics interleave them, so the module imported *second* completes *first* —
the reverse of declaration order. The RED probe gives both modules three awaits and the record reads
`tick:1 two:2` while **every other claim in the gate stays green**. That inversion is the entire
discriminating power of the gate.

**The cycle** (`a` imports `b`, `b` imports back) returns `a+A`, which is evidence of **live
bindings**: `b()` reads `marker` from a module that was still evaluating when `b` was linked. A
cycle that was refused throws; one linked by value-snapshot gives `a+undefined`.

**Gate:** `engine/page/tests/g_esm_tla_cycle.rs`, 5 claims, 2 RED mutations tabulated in its header.
