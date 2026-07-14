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
