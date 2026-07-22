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
