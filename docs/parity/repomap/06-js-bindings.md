# REPOMAP 06 — JavaScript Engine Integration & DOM/Web Bindings

How production browser engines glue a JavaScript VM to the DOM — the wrapper/reflector
model, how the (enormous) binding surface is **generated** rather than hand-written,
how wrapper lifetime is reconciled with GC across two heaps, and how the microtask
queue / event loop / `requestAnimationFrame` are driven — and what a lean from-scratch
Rust browser (**Manuk**, embedding SpiderMonkey via `mozjs`) should fold in.

The thesis, stated up front: every shipping engine's binding layer is **machine-generated
from WebIDL**, because the web platform is ~800–2,200 interfaces and no team hand-writes
that. The second universal lesson is that the hard problem is **not** calling C++ from JS —
it is **lifetime**: a DOM node and its JS wrapper can form a reference cycle that spans
two heaps, and each engine pays a different, large tax to collect it (Gecko's cycle
collector, Blink's unified Oilpan+V8 heap, JSC's opaque roots). The lean outlier —
**Ladybird** — sidesteps both taxes: DOM objects *are* GC-managed JS cells in one heap,
so there is no reflector and no cross-heap cycle. That is the model Manuk should study
hardest, even though Manuk cannot adopt it (it does not own its VM).

Cross-refs: builds on Manuk's `engine/js` (the `mozjs` embedding), `engine/dom` (the
arena DOM), `engine/css` (selector engine used by `querySelector`), and the standing
constraint in `CLAUDE.md` — **never patch SpiderMonkey/mozjs internals; sanctioned
FFI/embedding only.**

---

## 1. Scope & sources

Local clones under `/home/patrickd/manuk/`. Paths surveyed per engine:

**Chromium / Blink + V8** (`chromium/`)
- Wrapper model: `third_party/blink/renderer/platform/bindings/` — `script_wrappable.{h,cc}`, `wrapper_type_info.h`, `dom_data_store.{h,cc}`, `dom_wrapper_world.h`, `active_script_wrappable_base.h`, `v8_per_context_data.cc`.
- IDL codegen: `third_party/blink/renderer/bindings/scripts/` — `generate_bindings.py`, `web_idl/idl_compiler.py`, `bind_gen/` (`interface.py` 7.6k LOC, `blink_v8_bridge.py`, `mako_renderer.py`, `codegen_context.py`).
- Heap: `platform/heap/thread_state.h` (Oilpan `CppHeap` attach to V8 isolate).
- Event loop: `platform/scheduler/public/event_loop.h`, `core/execution_context/agent.{h,cc}`, `core/dom/scripted_animation_controller.cc`, `core/page/page_animator.cc`.
- Global: `bindings/core/v8/local_window_proxy.cc`, `core/frame/{window,navigator}.idl`. **2,228 `.idl` files.**

**Firefox / Gecko + SpiderMonkey** (`firefox/`, `mozjs/`)
- Reflector: `dom/bindings/JSSlots.h` (`DOM_OBJECT_SLOT`), `dom/bindings/DOMJSClass.h`, `dom/bindings/BindingUtils.h`, `dom/base/nsWrapperCache.h`.
- IDL codegen: `dom/bindings/Codegen.py` (**25,286 LOC**), `dom/bindings/Configuration.py`, `dom/bindings/Bindings.conf`. **841 `.webidl` files.**
- Cycle collector: `xpcom/base/nsCycleCollector.{cpp,h}`, `nsCycleCollectionParticipant.h`, `nsCycleCollectorTraceJSHelpers.cpp`, `xpcom/base/CycleCollectedJS{Runtime,Context}.h`.
- mozjs Rust crate (what Manuk builds on): `mozjs/mozjs/src/{rust.rs,conversions.rs,context.rs}`, `mozjs/mozjs/src/gc/{macros.rs,root.rs,custom.rs}`, `mozjs/mozjs-sys/src/jsglue.cpp`.

**WebKit / JavaScriptCore** (`WebKit/`)
- Wrapper: `Source/WebCore/bindings/js/JSDOMWrapper.h`, `JSDOMWrapperCache.h`, `DOMWrapperWorld.h`, `JSNodeCustom.cpp`, `JSDOMGlobalObject.h`, `JSDOMWindowBase.{h,cpp}`, `JSWindowProxy.{h,cpp}`.
- IDL codegen: `Source/WebCore/bindings/scripts/CodeGeneratorJS.pm` (Perl), `IDLParser.pm`. **1,955 `.idl` total; 183 in `dom/`.**
- VM: `Source/JavaScriptCore/runtime/{Structure.h,JSCell.h,JSObject.h,JSGlobalObject.h,MicrotaskQueue.h}`, `jit/JITCode.h`, `heap/`.
- Event loop: `Source/WebCore/dom/{EventLoop.cpp,WindowEventLoop.cpp,Microtasks.cpp,ScriptedAnimationController.cpp}`.

**Ladybird** (`ladybird/`) — the closest lean-from-scratch analogue
- Wrapper: `Libraries/LibWeb/Bindings/PlatformObject.h`, `Intrinsics.h`, `MainThreadVM.cpp`; `Libraries/LibWeb/DOM/{Node,Element}.h`; `Libraries/LibWeb/HTML/Window.h`.
- IDL codegen: `Meta/Generators/generate_libweb_bindings.py`, `Meta/Generators/libweb_bindings/*.py`. **660 `.idl` files.**
- GC (extracted to `LibGC` since brief was written): `Libraries/LibGC/{Cell.h,Heap.{h,cpp},Ptr.h,Root.h,Function.h,HeapGroup.h}`.
- Bytecode VM: `Libraries/LibJS/Bytecode/{Interpreter.cpp,Executable.h,Operand.h}`.
- Event loop: `Libraries/LibWeb/HTML/EventLoop/EventLoop.cpp`, `Bindings/MainThreadVM.cpp`, `HTML/AnimationFrameCallbackDriver.h`.

**Manuk today** (READ): `engine/js/src/{lib.rs,spidermonkey.rs,dom_bindings.rs,bindings_prototype.rs,event_loop.rs,job_queue.rs,history_bindings.rs,history_host.rs}`.

---

## 2. Per-engine approach

### 2.0 The two problems every binding layer solves

1. **Projection** — expose a C++/Rust DOM object to JS as a JS object with the right
   prototype, attributes (getters/setters), and methods, and let a native callback
   recover the backing object from `this`. This is mechanical and is **always generated
   from WebIDL** in shipping engines.
2. **Lifetime** — keep the JS wrapper and the native object alive exactly as long as
   *either* is reachable, and collect them when *neither* is — including when they form
   a cycle (`node.expando = () => node`). When the DOM lives in a refcounted/native heap
   and JS lives in a GC heap, this cycle spans two collectors. This is the hard part,
   and the four engines pick four different points on the design spectrum:

| Engine | DOM heap | JS heap | Cross-heap cycles collected by |
|---|---|---|---|
| **Blink** | Oilpan (GC) | V8 (GC) | **Unified heap** — Oilpan `CppHeap` attached to V8 isolate; one marker |
| **Gecko** | XPCOM refcount | SpiderMonkey (GC) | **Separate cycle collector** (`nsCycleCollector`) reconciling both |
| **JSC** | `RefPtr` refcount | JSC (GC) | **Opaque roots** — GC treats a connected DOM subtree as one reachability unit |
| **Ladybird** | — (none) | LibJS (GC) | **No cross-heap problem** — DOM objects *are* JS cells in one heap |

Manuk sits closest to Gecko-without-the-cycle-collector: a refcount-free arena DOM plus
SpiderMonkey's GC, with reflectors carrying a raw `NodeId` — and, today, **no lifetime
reconciliation at all** (§3).

### 2.1 Blink + V8 — generated bindings, unified-heap wrapper tracing

**Wrapper model.** Every IDL-exposed DOM class derives from `ScriptWrappable`, which in
the modern tree is `class ScriptWrappable : public v8::Object::Wrappable`
(`platform/bindings/script_wrappable.h:53`). The main-world wrapper is stored **inline**
in the C++ object as a single traced V8 reference — `TraceWrapperV8Reference<v8::Object>
wrapper_;` (`script_wrappable.h:143`); other-world wrappers live in a `DOMDataStore`. The
old "hidden internal fields" mechanism is gone: unwrapping `this` back to the C++ object
uses V8's sandbox-safe pointer tagging, `v8::Object::Unwrap<ScriptWrappable>(...,
CppHeapPointerTagRange(this_tag, max_subclass_tag))` (`script_wrappable.h:149-155`), with
tags `256..2000` **code-generated** per interface (`wrapper_type_info.h:52-124`).

**`WrapperTypeInfo`** is one static const per interface; its pointer identity *is* the
type identity, and it carries the template-install function, the interface name, and the
IDL parent-class pointer for `IsSubclass()` walks (`wrapper_type_info.h:130-223`).
Wrapper identity is maintained by `DOMDataStore` (`dom_data_store.h:66`), a
`GarbageCollected` per-world map with a fast inline-storage path for the main world
(`GetWrapper`/`SetWrapperInInlineStorage`, `:99-127`).

**Codegen (the key innovation, at scale).** A two-stage Python pipeline under
`bindings/scripts/`: stage 1 collects `.idl` into a pickled `web_idl` database and
assigns the CppHeapPointer tags (`web_idl/idl_compiler.py`); stage 2's
`generate_bindings.py:82` dispatches per-construct to `bind_gen/` generators — `interface.py`
(**7,637 LOC**), `namespace.py`, `dictionary.py`, `union.py`, iterators, etc. — rendering
C++ via **Mako** templates (`mako_renderer.py`). A generated `V8Xxx.cc` contains the
`WrapperTypeInfo` definition, `InstallInterfaceTemplate` (a V8 `FunctionTemplate` +
prototype/instance templates), one V8 callback per attribute getter/setter and method,
and `InstallConditionalFeatures` for origin-trial/flagged members. **2,228 `.idl` files**
drive tens of MB of generated code — the mechanism that makes maintaining the whole web
platform tractable.

**GC / lifetime (the second key innovation).** Blink's Oilpan tracing GC owns a `CppHeap`
that is **attached to the V8 isolate** (`platform/heap/thread_state.h:38-66`), unifying
the two heaps. Both directions of the DOM↔JS edge — the inline `wrapper_`
(`ScriptWrappable::Trace` → `visitor->Trace(wrapper_)`, `script_wrappable.cc:65`) and the
JS→C++ CppHeap pointer — are visible to a **single marker**, so cross-heap cycles are
reclaimed in one pass. This *superseded* the legacy `ScriptWrappableMarkingVisitor` /
cycle-collection scheme. Objects with outstanding async work stay alive via
`ActiveScriptWrappableBase::HasPendingActivity()`
(`active_script_wrappable_base.h:20-36`) — e.g. a pending `fetch`/XHR keeps its wrapper.

**Event loop / microtasks / rAF.** `EventLoop` "just manages a microtask queue"
(`platform/scheduler/public/event_loop.h:48`) backed by `v8::MicrotaskQueue`. The `Agent`
owns the loop and drives `PerformMicrotaskCheckpoint()` (`agent.cc:142`); every JS
callback exit runs a microtask scope (`callback_invoke_helper.cc:63`). `requestAnimationFrame`
registers into `FrameRequestCallbackCollection` and fires from the rendering lifecycle —
`PageAnimator::ServiceScriptedAnimations()` → `ScriptedAnimationController::ExecuteFrameCallbacks()`
(`scripted_animation_controller.cc:176`), triggered by the compositor's `BeginMainFrame`.

**Global.** The Window is installed by `LocalWindowProxy` (`local_window_proxy.cc:145-202`),
not a plain wrapper — a persistent global proxy re-associated to `V8Window`'s
`WrapperTypeInfo` across navigation. `console`/`navigator` are ordinary generated
interfaces installed onto the global.

### 2.2 Gecko + SpiderMonkey — reflectors, Codegen.py, and the cycle collector

This is Manuk's own family (same VM, via `mozjs`), so it is the most load-bearing comparison.

**Reflector model.** A reflected DOM object is a JSObject whose `JSClass` is really a
`DOMJSClass` (`dom/bindings/DOMJSClass.h:512-551`) storing the native C++ pointer in
**reserved slot 0** — `#define DOM_OBJECT_SLOT 0` (`dom/bindings/JSSlots.h:17`). Natives
are recovered with `UnwrapDOMObject<T>` → `JS::GetReservedSlot(obj,
DOM_OBJECT_SLOT).toPrivate()` (`BindingUtils.h:106-112`). **This is exactly Manuk's
mechanism** — a raw pointer in a reserved slot — except `DOMJSClass` also carries
`mInterfaceChain` (fast type checks) and `mParticipant` (the **cycle-collection
participant**, `DOMJSClass.h:535`), which Manuk has no equivalent of.

Reflector identity is cached by `nsWrapperCache` (`dom/base/nsWrapperCache.h`):
`GetWrapper()`/`SetWrapper()` (`:103-140`) map a native to a stable JSObject, and
`GetOrCreateDOMReflector` (`BindingUtils.h:1336`) checks the cache before calling the
generated `Wrap`. The GC-liveness hook is `SetPreservingWrapper()`/`TraceWrapper()`
(`:227,:323`) — a preserved wrapper is kept alive by the CC/GC.

**Codegen.** `dom/bindings/Codegen.py` (**25,286 LOC**) turns **841 `.webidl`** files into
per-interface `XxxBinding.cpp/.h`, configured by `Bindings.conf` via `Configuration.py`.
Getters/setters are generated by `CGSpecializedGetter`/`CGGetterCall` (`Codegen.py:10448`)
and `CGSetterCall` (`:10526`); the JS-facing trampolines are policy templates
`GenericGetter<ThisPolicy, ExceptionPolicy>` / `GenericSetter<ThisPolicy>`
(`:3167-3226`, variants for CrossOrigin/LenientThis/Global). A generated accessor unwraps
`this` via the reserved-slot read, calls the C++ native, and converts the result to a
`JS::Value`.

**Cycle collector (the flagship innovation Manuk does *not* get for free).** Gecko unifies
its **refcounted** C++ heap with SpiderMonkey's mark-and-sweep GC so cycles spanning both
(DOM node → JS closure → DOM node) are collectable. `nsCycleCollector`
(`xpcom/base/nsCycleCollector.cpp`) traverses via `TraverseNativeAndJS`, layering
`NoteJSChild` to report JS edges into the CC graph as `JS::GCCellPtr`
(`nsCycleCollectionParticipant.h:316-329`; bridge in
`nsCycleCollectorTraceJSHelpers.cpp:21`). The `CanSkip` family
(`nsCycleCollectionParticipant.h:353-401`) prunes known-alive objects. Native objects
holding JS edges register in the `JSHolderMap` of `CycleCollectedJSRuntime`
(`CycleCollectedJSContext.h`), traced during GC marking. **Servo (and therefore Manuk)
deliberately avoid the cycle collector** — Servo uses tracing-based `JSTraceable`/rooting
instead; Manuk uses neither yet (§3).

**Event loop / Promise jobs.** `CycleCollectedJSContext` *is* a `JS::JobQueue`
(`CycleCollectedJSContext.h:348`); `JS::SetJobQueue(mJSContext, this)`
(`CycleCollectedJSContext.cpp:142`) routes SpiderMonkey promise jobs into Gecko's
`runJobs()`/`PerformMicroTaskCheckPoint`. **This is the exact hook Manuk's `job_queue.rs`
uses** (`SetJobQueue` + a `RustJobQueue` trap vtable) — the sanctioned embedding path,
confirmed here as the same one Gecko itself uses.

**The mozjs Rust crate (Manuk's actual substrate).** Two crates: `mozjs-sys` (raw FFI +
C++ glue) and `mozjs` (safe-ish wrappers). Reserved-slot access is out-of-line C++ glue
(`mozjs-sys/src/jsglue.cpp:1131` `JS_GetReservedSlot`; proxy slots `:794-802`) surfaced to
Rust via generated `*_wrappers.in.rs`. Conversions are the traits `ToJSValConvertible` /
`FromJSValConvertible` (`mozjs/src/conversions.rs:135-182`) — the Rust analog of Codegen's
C++ conversion emission, and **the traits a Manuk binding generator would target**.
Rooting is `rooted!`/`rooted_vec!` (`mozjs/src/gc/macros.rs`) over `RootedGuard`, with
`CustomTrace` (`gc/custom.rs:12`) for tracing custom GC types. **Crucially: `mozjs`
provides only the raw JSAPI + conversion traits. The Codegen-equivalent lives in the
embedder** — i.e. Manuk must build it (or hand-write), exactly as Servo's `script` crate does.

### 2.3 JavaScriptCore + WebCore — RefPtr wrappers, opaque roots, Structure/IC + JIT tiers

**Wrapper model.** `JSDOMWrapper : JSDOMObject : JSC::JSDestructibleObject`
(`bindings/js/JSDOMWrapper.h:67-83`) holds the DOM object by a **strong** `Ref<Impl>
m_wrapped` (`:98`) — the JS cell owns a RefPtr on the WebCore object, so the wrapper
keeps the DOM object alive (not vice-versa). Identity is cached by
`cacheWrapper`/`getCachedWrapper` (`JSDOMWrapperCache.h:164-171`) into a per-world
`HashMap<void*, JSC::Weak<JSObject>>` (`DOMWrapperWorld.h:52`) — **weak** values, so the
map preserves identity only while a wrapper is otherwise reachable.

**Codegen.** `bindings/scripts/CodeGeneratorJS.pm` (Perl) with `GenerateHeader`/
`GenerateImplementation` (`:3234,:4676`) emits `JSXxx.{h,cpp}` per interface (**1,955
`.idl`**); attribute getters/setters and operations at `:5848,:6098,:6462`.

**GC / lifetime — opaque roots (no cycle collector).** JSC is mark-sweep + generational
(`Source/JavaScriptCore/heap/`) and breaks DOM↔JS cycles with an **opaque-roots**
reachability scheme instead of a separate collector. `JSNodeOwner::isReachableFromOpaqueRoots`
(`JSNodeCustom.cpp:70`) keeps a node's wrapper alive if the node `isConnected()` or its
opaque root (the tree root) is marked; `JSNode::visitAdditionalChildrenInGCThread`
(`:88`) registers the node's root as an opaque root during marking. Effect: the single GC
treats an entire connected DOM subtree as one reachability unit — simpler than Gecko's CC,
without Blink's full heap unification.

**Key innovations to flag.** (1) The **Structure** system (hidden classes) —
`class Structure : JSCell` (`runtime/Structure.h:194`) with `StructureID`, transition
tables, and `PropertyTable` offsets driving **inline caches**, plus transition
watchpoints the DFG uses to assume structure stability. (2) The **multi-tier JIT** —
`enum class JITType { InterpreterThunk (LLInt), BaselineJIT, DFGJIT, FTLJIT }`
(`jit/JITCode.h:64`): LLInt → Baseline → DFG → FTL escalation. Both are performance
mechanisms Manuk gets *for free from SpiderMonkey* (Warp/Ion) and should never
reimplement.

**Event loop.** `JSGlobalObject` holds a `MicrotaskQueue` (`JSGlobalObject.h:238`) driven
by WebCore's `EventLoop::performMicrotaskCheckpoint(vm)` (`dom/EventLoop.cpp:266`);
`requestAnimationFrame` via `ScriptedAnimationController`. Global is `JSDOMWindowBase` +
a `JSWindowProxy` shell that survives navigation.

### 2.4 Ladybird — DOM objects ARE JS cells (the lean model)

**Wrapper model — there is none.** `class PlatformObject : public JS::Object`
(`Bindings/PlatformObject.h:36`), and `JS::Object` is a `GC::Cell`. A DOM node is
**directly** a garbage-collected JS object — one allocation, one identity, no
reflector/wrapper pair. `DOM::Node : EventTarget` → `PlatformObject` → `JS::Object` →
`GC::Cell` (`DOM/Node.h:111`; `WEB_PLATFORM_OBJECT` macro `PlatformObject.h:22`).
Prototypes/constructors are lazily created and cached per realm via
`ensure_web_prototype<T>()`/`ensure_web_constructor<T>()` (`Bindings/Intrinsics.h:66-76`) —
so, unlike Manuk today, methods hang off **one shared prototype per interface**, not off
every instance.

**Codegen.** A Python generator (`Meta/Generators/generate_libweb_bindings.py`, modules in
`libweb_bindings/`) emits a `XPrototype` + `XConstructor` and argument marshalling per
interface at build time (**660 `.idl`**).

**GC — one unified mark-sweep heap.** `LibGC/Heap.h:85` `collect_garbage`, phases
`gather_conservative_roots` / `mark_live_cells` / `sweep_dead_cells` (`:173-177`);
**conservative** stack/register scan via `setjmp` (`Heap.cpp:36,100-104`). On-heap edges
use `GC::Ref`/`GC::Ptr` (`Ptr.h:19,82`) followed by `Cell::visit_edges` (`Cell.h:222`);
stack roots use `GC::Root` (`Root.h:46`); captured-callback cells are kept alive by
`GC::Function` (`Function.h:16`, the old `HeapFunction`) — used for tasks, promise jobs,
and rAF callbacks. **Because DOM and JS objects share one GC and are marked together, a
JS→DOM→JS cycle is freed by ordinary mark-sweep — no cycle collector, no cross-heap
boundary.** (A `HeapGroup`/`CrossHeapMember` bridges realms *within* one jointly-marked
group, so even that does not reintroduce the cross-heap-cycle problem.)

**Bytecode VM — register-based interpreter, no JIT.** `Bytecode/Interpreter.cpp`
(`run_executable:274`) over a flat register/local array; `Executable.h:296` holds the
bytecode stream and inline property caches. AST → bytecode → C++ interpreter loop only —
**no native codegen**. The deliberately lean single-tier model (contrast V8/JSC).

**Event loop.** `HTML/EventLoop/EventLoop.cpp` — `perform_a_microtask_checkpoint()`
(`:157`), `update_the_rendering()` (`:310`). Promise jobs bridge into the HTML loop:
`host_enqueue_promise_job` → `HTML::queue_a_microtask` (`Bindings/MainThreadVM.cpp:282-301`),
so JS reactions run as HTML microtasks. `requestAnimationFrame` via
`AnimationFrameCallbackDriver`, invoked from `update_the_rendering`. Window is itself a
`PlatformObject` serving as the JS global (`HTML/Window.h:59-65`).

**Why Ladybird matters to Manuk:** it is the only surveyed engine that is single-process,
single-heap, own-VM, with DOM native to that heap. It proves the reflector indirection and
the cross-heap cycle tax are *choices forced by embedding a third-party VM*, not
essential. Manuk **has chosen to embed** (SpiderMonkey), so it inherits the tax — but
Ladybird shows exactly which complexity is avoidable if that ever changes, and its
per-realm shared-prototype + `GC::Function` callback model is directly imitable today.

---

## 3. Manuk today — honest assessment

Manuk embeds SpiderMonkey via `mozjs 0.18` (the Servo path, deliberately **not** V8;
`engine/js/src/lib.rs:6`). The engine is initialized once per process behind a `OnceLock`
and leaked (`spidermonkey.rs:28-44`); one `Runtime` per process, stored `thread_local` as
`ManuallyDrop` and never dropped, because SpiderMonkey does not support multi-`Runtime`
create/destroy per process (`lib.rs:135-158`). Each document gets a fresh global (the
navigation model). This is all correct and matches the sanctioned embedding pattern.

**What works and is genuinely good:**
- **Reserved-slot reflectors** exactly as Gecko/Servo do: a shared `NODE_CLASS` with two
  reserved slots `(NodeId as Int32, *mut Dom as PrivateValue)` (`dom_bindings.rs:188-204`).
  `node_and_dom` gracefully returns `None` (→ `null`) rather than segfaulting on a
  non-reflector `this` (`:220-233`) — the "thin safe helper layer" that Step-0 flagged.
- A **real hand-written DOM subset**: traversal getters, mutation (`appendChild`,
  `insertBefore`, `removeChild`, `remove`, `cloneNode`), `createElement`/`createTextNode`,
  attribute reflection, `querySelector[All]`/`getElementById`/`getElementsBy*` delegating
  to the real `manuk_css` selector engine, `textContent`/`innerHTML` (round-tripping
  through the real HTML parser/serializer), form-control IDL (`value`/`checked`),
  `getBoundingClientRect` + `getComputedStyle` from a pre-script layout/style snapshot.
- A **reflector identity cache** so `a.firstChild === b` holds (`new_reflector:333-366`).
- A **real Event model** — capture/target/bubble, `target`/`currentTarget`,
  `preventDefault`/`stopPropagation` (`LISTENER_PRELUDE`, `dom_bindings.rs:1336-1385`).
- A **spec-shaped event loop**: microtask-before-macrotask, `setTimeout`/`queueMicrotask`,
  and `fetch`/XHR whose I/O the Rust loop performs (`event_loop.rs`).
- A **native Promise job queue** via the *correct* embedder hook — `JS::JobQueue` +
  `SetJobQueue` with a `RustJobQueue` trap vtable (`job_queue.rs`), the same hook Gecko
  uses (`CycleCollectedJSContext.cpp:142`), after correctly diagnosing that mozjs's
  `UseInternalJobQueues` cannot be sequenced before `InitSelfHostedCode`.
- **ES modules**: `CompileModule`/`ModuleLink`/`ModuleEvaluate` for self-contained modules
  (`dom_bindings.rs:1274-1289`).
- **Tier-0 BOM globals** (`window`/`self`/`console`/`navigator`) installed as a prelude so
  a bundle that eagerly caches them doesn't `ReferenceError` on line 1
  (`WINDOW_PRELUDE:1392`), with an **honest** UA (Axis F, no competitor mimicry).
- The **History API** (`pushState`/`replaceState`/`popstate`/`hashchange`) over a host
  `SessionHistory`, correctly firing no network and no `popstate` on push
  (`history_bindings.rs`).

**The gaps — ranked by how much they will hurt:**

1. **Bindings are hand-written and do not scale.** `dom_bindings.rs` is ~1,650 lines for a
   jQuery-core subset of **one** conflated `Node` interface. The real platform is 800+
   interfaces (Gecko's number). Every method is a hand-rolled `unsafe extern "C"` with
   manual `vp` arithmetic. There is **no WebIDL codegen** — the thing every shipping engine
   considers non-negotiable. The module's own doc admits the Step-0 decision was to
   hand-write "before choosing hand-write vs Servo codegen" (`bindings_prototype.rs:2`);
   that choice is now due.

2. **Large parts of the "bindings" are implemented by building JS source strings and
   `eval`-ing them.** The identity cache is a JS `__nodes` map populated by
   `eval_in_current_global(cx, "…__nodes[{id}]=__pending_node…")` (`new_reflector:352-364`);
   the listener registry, event dispatch, `getComputedStyle`, and `getBoundingClientRect`
   all format a JS string and evaluate it (`dom_bindings.rs:180,542,913-919,978`); the
   promise job queue enqueues by `eval`-ing `"__promiseJobs.push(__pendingJob)"`
   (`job_queue.rs:123-127`); the whole event loop is driven by `evaluate_script` of small
   snippets (`event_loop.rs:86-95`). This is a genuine architectural smell: it is slow (a
   parse+compile per operation), it is **fragile to any page that shadows/deletes the
   `__`-prefixed globals or `Array.prototype.push`**, and it is a latent injection surface
   (values are hand-escaped by `js_string_literal`). Shipping engines never do this — they
   call the C++/JSAPI directly (`JS_SetElement`, `JS::Call`, native `JSObject`s). Replacing
   string-eval with direct JSAPI is the single highest-leverage correctness/perf cleanup.

3. **No GC/lifetime story for reflectors.** The `*mut Dom` in a slot is dereferenced
   `unsafe`ly and is sound only "for the lifetime the reflectors are reachable… the
   embedder must not free the Dom meanwhile" (`dom_bindings.rs:28-30`). There is **no
   tracing** of `NodeId`→wrapper edges, no `nsWrapperCache`-style preservation, no cycle
   collection. Today this is masked because a document's global is short-lived and the
   `Dom` outlives it, but the moment JS can hold a detached subtree alive across GC (e.g.
   `var saved = el.cloneNode(true)` on a node later removed from the arena), Manuk has a
   **use-after-free or a leak** with no mechanism to prevent it. This is the Gecko cycle
   collector's entire reason for existing, and Manuk has nothing analogous.

4. **Methods are defined per-instance, not per-prototype.** `define_members` is called on
   every reflector (`new_reflector:350`), so N nodes × M methods = N·M `JS_DefineFunction`
   calls and N·M function objects. The code comments admit "production would hang them off
   per-interface prototype objects" (`dom_bindings.rs:33-34`). Ladybird's
   `ensure_web_prototype` (`Intrinsics.h:66`) is the fix and is directly imitable.

5. **Missing Web API surface.** No structured clone (History state is JSON round-tripped,
   `history_bindings.rs:24`), no real `Event` constructor (`dispatchEvent` takes a type
   string), no `MutationObserver`, no `URL`/`URLSearchParams`, no `TextEncoder`/`Decoder`,
   no timers with delay ordering (`setTimeout` ignores its delay, `event_loop.rs:37`), no
   `WeakRef`/`FinalizationRegistry`, no `localStorage`, no `CustomEvent`/`AbortController`,
   no external `<script src>` loading. `fetch` returns a **host thenable, not a native
   `Promise`** (`event_loop.rs:56-62`) — now fixable since the native job queue works.

---

## 4. Fold-in recommendations (ranked by leverage)

### R1 — Replace string-`eval` bindings with direct JSAPI calls. **(highest leverage, do first)**
Everything currently done by formatting-and-evaluating a JS snippet should call the
JSAPI/`mozjs` wrappers directly, the way `dom_bindings.rs` already does for `JS_SetElement1`
in `node_array`. Concretely: build the identity cache as a native structure (a
`HashMap<NodeId, Heap<*mut JSObject>>` traced by the runtime, or a JS `Map` manipulated via
`JS::MapSet` rather than `eval`), invoke event listeners with `JS::Call` instead of an
`__dispatchEvent` string, and build `DOMRect`/computed-style results with `JS_NewObject` +
`JS_DefineProperty` instead of evaluating an object literal. This removes the per-operation
parse/compile cost, closes the page-shadowing and injection fragility, and is a
prerequisite for R3 (you cannot trace edges that live inside `eval`-string state). No new
architecture — just moving existing behavior onto the sanctioned FFI surface Manuk already
uses elsewhere.

### R2 — Build a WebIDL-style binding generator (a small one), don't keep hand-rolling. **(high)**
The verdict from every shipping engine is unambiguous: hand-written bindings do not reach
platform coverage. **But do not port Gecko's 25k-line `Codegen.py` or Blink's Mako
pipeline** — that is BLOAT for a lean browser. Build a **minimal** generator that:
- parses a *curated subset* of `.webidl` (the ~30–50 interfaces real pages touch — see R4),
  reusing an existing WebIDL parser crate (e.g. `weedle`) rather than writing one;
- emits Rust that targets Manuk's existing idioms: reserved-slot reflectors, the
  `ToJSValConvertible`/`FromJSValConvertible` traits `mozjs` already provides
  (`conversions.rs:135-182`), and **per-interface prototype objects** (R5);
- generates one native trampoline per attribute/method from an interface description,
  eliminating the hand-rolled `unsafe extern "C"` boilerplate that dominates
  `dom_bindings.rs`.

The leverage: 50 interfaces × ~15 members hand-written is ~10k lines of unsafe FFI to
maintain and fuzz; generated, it is a few hundred lines of generator + declarative IDL.
This is the same trade every engine made. Keep the generator dumb and the runtime helpers
(rooting, string conversion, error mapping) hand-written and shared — Servo's split.

### R3 — Give reflectors a real lifetime story. **(high; correctness, not features)**
Pick one of two models, both sanctioned:
- **(preferred, lean) Wrapper-cache + tracing, à la Servo.** Store wrappers in a native
  identity map; register a **trace hook** so that live wrappers keep their backing arena
  nodes pinned and vice-versa, using `mozjs`'s `CustomTrace`/rooting
  (`gc/custom.rs:12`). Because Manuk's DOM is an **arena with `NodeId` indices, not
  refcounted pointers**, the classic C++↔JS *cycle* largely dissolves: the arena owns node
  storage, so a JS wrapper holding a `NodeId` cannot create a native refcount cycle — it
  can only keep a *slot* logically alive. The remaining job is (a) don't reuse a `NodeId`
  slot while a wrapper still references it, and (b) trace wrapper→node reachability for
  detached subtrees. This is dramatically simpler than Gecko's cycle collector and is the
  right lean bet.
- **(reject) A full cycle collector.** Do **not** build an `nsCycleCollector` analog — it
  is the single largest piece of Gecko's DOM plumbing and Manuk's arena model does not need
  it. Flag as explicit BLOAT-to-avoid.

Whichever, the invariant to establish and test: *a JS value that outlives a node's removal
from the arena must not dereference freed storage.* Today that invariant is undocumented
and unenforced (`dom_bindings.rs:28-30`).

### R4 — The minimal high-value Web API surface (for parity + agent-native use). **(medium)**
Rank by what real pages and an in-process agent actually need:
1. **DOM mutation + traversal** — largely present; finish `Event`/`CustomEvent`
   constructors and real `dispatchEvent(Event)`.
2. **Timers done correctly** — `setTimeout`/`setInterval` honoring delay ordering and
   `clearTimeout` (today delay is ignored, `event_loop.rs:37`).
3. **`fetch` returning a native `Promise`** (now unblocked by `job_queue.rs`), plus
   `Response`/`Headers` shape; `AbortController`.
4. **`URL`/`URLSearchParams`, `TextEncoder`/`TextDecoder`, structured clone** — small, high
   utility, and structured clone fixes the History state gap (`history_bindings.rs:24`).
5. **`MutationObserver`** — *disproportionately valuable for the agent*: it is the
   native "tell me when the DOM changed" primitive the agent's observation loop wants,
   and it reuses the microtask checkpoint Manuk already has.
6. `localStorage`/`sessionStorage` (host-backed), `requestAnimationFrame` (host-driven off
   the render loop, as Blink/Ladybird do).

**Agent-native angle (the differentiator):** because Manuk's controller is in-process (see
repomap 05), the agent does **not** need these APIs *serialized* — it can read the arena
DOM directly. So prioritize the APIs *pages* need to run, and expose agent observation via
the native arena + `MutationObserver`, not via a bindings-heavy CDP-style surface.

### R5 — Per-interface prototype objects. **(medium, cheap)**
Adopt Ladybird's `ensure_web_prototype`/`ensure_web_constructor` pattern
(`Intrinsics.h:66-76`): one prototype per interface per global, methods defined once on the
prototype, reflectors created with that prototype. Removes the N·M instance-method
explosion (`dom_bindings.rs:350`) and gives correct `instanceof`/`constructor` semantics
that pages test for. Falls out naturally from R2's generator.

### Is SpiderMonkey/mozjs the right long-term bet vs V8?
**Yes — keep SpiderMonkey.** Reasons specific to the lean-Rust mandate:
- **Rust-native embedding.** `mozjs` gives Rust the JSAPI, rooting macros, and conversion
  traits directly (`rust.rs`, `gc/macros.rs`, `conversions.rs`); V8's embedding is C++
  and the Rust `rusty_v8` binding is lower-level and Chromium-versioned. The whole binding
  layer, GC integration, and job queue Manuk has built assume the `mozjs` surface.
- **The sanctioned-embedding constraint is already satisfied.** Manuk's job queue uses the
  *same* `JS::JobQueue`/`SetJobQueue` hook Gecko uses; reserved slots are the *same*
  mechanism. Switching to V8 would restart all of this against `ScriptWrappable` +
  `CppHeap` + Oilpan-style tracing (§2.1), which is *more* machinery, not less.
- **You get Warp/Ion, the Structure-equivalent, and years of adversarial fuzzing for
  free** — the exact things `CLAUDE.md` says never to touch. V8 offers the same class of
  wins but with a heavier, C++-first integration.
- **The one honest caveat:** SpiderMonkey's *single-Runtime-per-process* teardown
  constraint (`lib.rs:135`, and the `--ignored` test isolation throughout) is a real
  ergonomic tax on multi-tab/isolate architectures. It is livable (one process per tab, or
  one long-lived runtime with per-tab realms) but should be a conscious architecture
  decision, not a surprise.

Only revisit the VM choice if Manuk ever decides to **own** its JS engine (the Ladybird
path) to erase the reflector + lifetime tax entirely — a very large undertaking that trades
"never fuzz a JIT" for "control the whole heap." Not recommended near-term.

### BLOAT to explicitly avoid
- A full cycle collector (`nsCycleCollector` analog) — R3 shows the arena model doesn't need it.
- A 25k-line codegen — R2's curated generator is the lean equivalent.
- Reimplementing hidden classes / inline caches / a JIT — SpiderMonkey provides these; touching them violates the modification boundary.
- A serialized automation/DOM-snapshot surface for the agent — repomap 05's thesis: the in-process agent reads the arena directly.
- Isolated worlds / multiple `DOMWrapperWorld`s (Blink) — an extension-isolation feature Manuk has no use case for yet.

---

## 5. Open questions for frontier research

1. **Arena `NodeId` reuse vs wrapper liveness.** If the arena recycles a freed `NodeId`
   slot while a JS wrapper still holds that id, the wrapper silently rebinds to an
   unrelated node. Does Manuk need generational `NodeId`s (index + generation counter), or
   a "pin slot while wrapped" flag driven by the trace hook (R3)? This is the crux of the
   lifetime model and has no answer in the code today.
2. **How minimal can the binding generator be** before it's cheaper to hand-write? Is there
   a crossover interface count (~20? ~50?) below which a declarative macro (not a codegen
   binary) suffices? Prototype R2 against the real top-50 interfaces and measure.
3. **Can the microtask/event loop stop round-tripping through `eval` entirely** (R1) while
   preserving the exact spec ordering the current `event_loop.rs` tests pin? What is the
   native-`JS::Call` rewrite's effect on the `pTmU` ordering guarantees?
4. **Agent-native Web APIs.** Is `MutationObserver` the right agent observation primitive,
   or should the agent observe the arena *below* the binding layer (no JS involvement) and
   treat `MutationObserver` purely as a page-compat API? The in-process architecture makes
   this a real design fork.
5. **`fetch` as native `Promise` + streaming.** Now that the job queue works, what is the
   right boundary between the Rust async I/O loop and JS-visible `Promise`/`ReadableStream`
   — how much of Streams is worth implementing vs. a buffered `Response.text()` shim?
6. **Structured clone without a serializer zoo.** Can Manuk reuse SpiderMonkey's own
   `JS_StructuredClone` (a sanctioned JSAPI) rather than hand-rolling, fixing History state
   and `postMessage` in one move?
