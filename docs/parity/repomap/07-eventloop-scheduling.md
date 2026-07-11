# REPOMAP 07 — Event Loop, Task Scheduling, Rendering Cadence & Threading

How production browser engines structure the **HTML event loop**, schedule the
**rendering pipeline** (style → layout → paint → composite), prioritise **task
queues**, and split work across **processes and threads** — and what a lean
from-scratch Rust browser (**Manuk**) should fold in to fix *latency*. All paths
are absolute under `/home/patrickd/manuk/`.

---

## 1. Scope & sources

Four things are being surveyed, distinct but interlocking:

1. **The HTML event loop** — the tasks-vs-microtasks machine the spec defines: a
   run loop that picks one task, runs it to completion, drains microtasks, then
   (once per frame) runs "update the rendering".
2. **Rendering cadence** — what fires the pipeline: a vsync-aligned refresh
   driver / `BeginMainFrame`, `requestAnimationFrame` servicing, and the
   *document lifecycle* that gates and coalesces style/layout/paint.
3. **Task prioritisation** — priority-ordered queues so input and rendering beat
   background timers (Blink's `TaskPriority`, Gecko's `EventQueuePriority`).
4. **Threading / process model** — the compositor thread, Chromium's
   multiprocess split, Servo's script/layout/constellation actors.

| Engine | Paths |
|---|---|
| **Blink scheduler** | `chromium/third_party/blink/renderer/platform/scheduler/{common,main_thread}` (`main_thread_scheduler_impl.cc`, `task_priority.h`, `use_case.h`, `main_thread_task_queue.*`, `frame_scheduler_impl.*`, `page_scheduler_impl.*`) |
| **Blink lifecycle** | `chromium/third_party/blink/renderer/core/frame/local_frame_view.cc` (`UpdateLifecyclePhases`), `chromium/third_party/blink/renderer/core/dom/document_lifecycle.h` |
| **cc (compositor)** | `chromium/cc/trees/proxy_main.cc` (main↔impl thread proxy) |
| **Gecko** | `firefox/xpcom/threads/{TaskController,EventQueue,nsThread}.*`, `firefox/layout/base/nsRefreshDriver.cpp` |
| **Servo** | `servo/components/constellation/{constellation,pipeline,event_loop}.rs`, `servo/components/script/script_thread.rs`, `servo/components/layout/layout_impl.rs` |
| **WebKit** | `WebKit/Source/WebCore/dom/{EventLoop,WindowEventLoop,Microtasks}.cpp`, `WebKit/Source/WebCore/page/Page.cpp` (`updateRendering`), `.../page/RenderingUpdateScheduler.h` |
| **Ladybird** | `ladybird/Libraries/LibWeb/HTML/EventLoop/{EventLoop,Task,TaskQueue}.cpp` |
| **Manuk (this repo)** | `shell/src/gui.rs` (winit loop), `engine/page/src/lib.rs` (`relayout*`) |

---

## 2. Per-engine approach

### 2.1 Ladybird — the HTML spec event loop, verbatim (read this first)

Ladybird is the clearest reference because it implements the WHATWG algorithm
almost line-for-line, with the spec step numbers in comments. Manuk should model
its own loop on this shape.

**The loop** (`EventLoop.cpp`):
- `EventLoop::process()` `:122` is one turn: if a task queue has a runnable task,
  pick "the first runnable task in taskQueue and remove it" `:142`
  (`take_first_runnable()` `:143`), set it as currently-running, run its steps,
  then **`perform_a_microtask_checkpoint()`** `:157`. One task, then drain all
  microtasks — the fundamental cadence.
- `perform_a_microtask_checkpoint()` `:643` loops `while (!m_microtask_queue.is_empty())`
  dequeuing `:661-663`, guarded by an `m_performing_a_microtask_checkpoint`
  re-entry flag.
- `spin_until(goal)` `:81` is the spec's nested event loop (used by sync APIs):
  it performs a microtask checkpoint `:95` and pumps tasks until the goal holds.
- Idle: if a window loop has no runnable task `:176`, it starts an idle period
  and computes a deadline for `requestIdleCallback` `:181-184`.

**Rendering is itself a task.** `queue_task_to_update_the_rendering()` `:197`
records the render-opportunity time and, "for each navigable that has a rendering
opportunity, queue a global task on the **rendering task source**" `:209` whose
steps call `update_the_rendering()`. So rendering is coalesced into the same
single-threaded loop, not a side channel.

**`update_the_rendering()`** `:310` is the ordered pipeline the whole industry
shares (spec step numbers preserved in the source):
1. `frameTimestamp` = last render-opportunity time `:320`.
2. Gather all fully-active docs `:323`; **filter out non-renderable docs** `:329`
   (throttled / no rendering opportunity) — coalescing at the document level.
3. run resize steps `:375`, scroll steps `:380`, **evaluate media queries** `:387`,
   **update animations & send events** `:392`, fullscreen steps `:397`.
4. **run the animation frame callbacks** (`requestAnimationFrame`) `:404`.
5. The **resize-observer loop** `:416`: `while(true)` — recalc styles & update
   layout `:419`, resolve `content-visibility` proximity `:431`, gather resize
   observations at increasing depth `:454`, break when stable `:466`. This is why
   layout can run several times per frame but converges.
6. intersection observations `:509`, then **paint**: step 22 `:509+` — for each
   navigable, `if (!navigable->needs_repaint()) continue;` then
   `navigable->paint_next_frame()`. A dirty bit gates painting per navigable.

Take-away for Manuk: the *order* and the *dirty-gating* (skip non-renderable
docs, skip un-dirtied navigables) are the coalescing mechanism — not threads.

### 2.2 Blink — priority task queues + the document lifecycle state machine

**Priority-ordered task queues.** `task_priority.h:14` defines an 11-level
`enum class TaskPriority`:

```
kControlPriority=0, kHighestPriority=1, kExtremelyHighPriority=2,
kVeryHighPriority=3, kHighPriorityContinuation=4, kHighPriority=5,
kNormalPriorityContinuation=6, kNormalPriority=7(=kDefault),
kLowPriorityContinuation=8, kLowPriority=9, kBestEffortPriority=10
```

Every `MainThreadTaskQueue` maps to one of these; the `base::sequence_manager`
picks the highest-priority non-empty queue each turn. Frames get their own
queues (`frame_scheduler_impl.*`, `frame_task_queue_controller.*`) and pages get
`page_scheduler_impl.*` so a background tab's timers can be throttled/frozen
independently.

**Input-aware prioritisation (an innovation).** The scheduler tracks a
`UseCase` (`use_case.h:18`): `kNone, kEarlyLoading, kLoading, kTouchstart,
kCompositorGesture, kSynchronizedGesture, kMainThreadGesture,
kMainThreadCustomInputHandling`. `main_thread_scheduler_impl.cc`
`ComputeCurrentUseCase()` `:1405` switches the whole queue-priority *policy*
based on what the user is doing: during `kCompositorGesture` `:1475` (a
compositor-driven scroll/fling) it *deprioritises* the main thread so it does not
fight the compositor; on `kTouchstart` and loading it reshuffles so input and
compositing win. Pending input is even inspected (`pending_user_input.*`) to bias
priorities *before* the event is dispatched.

**The document lifecycle** (`document_lifecycle.h:47`) is a strict monotonic
state machine: `kUninitialized → kVisualUpdatePending → kInStyleRecalc →
kStyleClean → kInPerformLayout → kLayoutClean → kInCompositingInputsUpdate →
kCompositingInputsClean → kInPrePaint → kPrePaintClean → kInPaint → kPaintClean`.
Each phase may only run once the previous is *clean*, and `AllowThrottlingScope`
lets off-screen frames skip.

`LocalFrameView::UpdateLifecyclePhases(target_state, reason)` (driven from
`BeginMainFrame`) walks phases up to a **target** and no further —
`UpdateLifecycleToLayoutClean` `:2192`, `...CompositingInputsClean` `:2144`,
`...PrePaintClean` `:2152`, full `kPaintClean` `:2111`. Phase bodies:
`RunStyleAndLayoutLifecyclePhases` `:2789`, `RunCompositingInputsLifecyclePhase`
`:2840`, `RunPrePaintLifecyclePhase` `:2887`, `RunPaintLifecyclePhase` `:2954`.
This is the *gating + coalescing* engine: a `getBoundingClientRect()` call only
forces the pipeline up to `kLayoutClean`; paint is deferred to the frame. All
invalidations between frames just set dirty bits and `ScheduleAnimation()`
`:3898` (a `BeginMainFrame` request tagged with a `cc::BeginMainFrameReason`,
e.g. `kLayoutInvalidation` `:1662`, `kPaintInvalidation` `:2218`) — many
invalidations collapse into one frame.

**Threading.** The renderer's **main thread** runs Blink + the lifecycle; a
separate **compositor thread** (cc) runs via `ProxyMain` (`cc/trees/proxy_main.cc`)
which marshals `BeginMainFrame`/commit between main and impl thread. The impl
thread handles scroll and composited animation *without the main thread*, so a
scroll is smooth even when JS is busy — this is the single biggest snappiness win
in the design.

### 2.3 Gecko — TaskController priority lanes + vsync refresh driver

**Priority lanes.** `EventQueue.h:17` defines `EventQueuePriority`:
`Idle=0, DeferredTimers=1, InputLow=3, Normal=4, MediumHigh=5, InputHigh=6,
Vsync=7, InputHighest=8, RenderBlocking=9, Control=10`. Note input is *split*
into low/high/highest so a live gesture outranks normal work but a stale input
does not starve rendering. `TaskController` (`TaskController.cpp/.h`) is the
modern scheduler behind `nsThread`/`EventQueue`; tasks carry a `TaskManager`
that can dynamically suspend/reprioritise a whole class (e.g. background tabs).

**Refresh driver = the render clock.** `nsRefreshDriver.cpp` is Gecko's
per-document render cadence. `VsyncRefreshDriverTimer` `:439` ticks the driver on
**vsync**, throttling if vsync floods `:436`, and there are distinct timers for
the parent vs `CreateForContentProcess` `:465`. On each tick it runs animation
frame callbacks, flushes style + layout, and drives the paint — the same ordered
"update the rendering" set, clocked by the display. `RenderBlocking` priority
`:26` exists precisely so first-paint-critical work (e.g. render-blocking CSS/JS)
outranks normal tasks.

### 2.4 WebKit — one event loop per agent, `Page::updateRendering` pipeline

`WindowEventLoop` (`WindowEventLoop.cpp`) is the per-origin-cluster event loop;
`EventLoop.cpp` holds the task groups. `EventLoop::run()` `:321` pumps tasks with
an optional deadline; `queueTask` `:198`/`queueMicrotask` `:259` and
`performMicrotaskCheckpoint` `:266` are the spec primitives. `scheduleToRun()`
`:112` arms `m_timer`→`didReachTimeToRun()` `:233`, which afterward
`opportunisticallyRunIdleCallbacks()` `:134` in the slack before the next frame.
A microtask checkpoint is only performed if there *are* microtasks for a fully
active document `:186`.

The render step is `Page::updateRendering()` (`Page.cpp:2231`) — the identical
ordered list: `layoutIfNeeded(UpdateCompositingLayers)` `:2243`, then per-document
`runResizeSteps` `:2291`, `runScrollSteps` `:2295`,
`evaluateMediaQueriesAndReportChanges` `:2299`, `updateAnimationsAndSendEvents`
`:2308`, `serviceRequestAnimationFrameCallbacks` `:2330`, another
`layoutIfNeeded()` `:2337`, then `doAfterUpdateRendering()` `:2413` and
`renderingUpdateCompleted()` `:2570`. `RenderingUpdateScheduler.h` arms this off
the platform display link. `breakToAllowRenderingUpdate()` `:314` can even
interrupt a running JS timer-fire loop so rendering is not starved — the mirror
of Gecko's `MediumHigh`.

### 2.5 Servo — the actor model (constellation + per-pipeline script/layout)

Servo splits the browser into message-passing actors, coordinated by the
**constellation** (`constellation.rs` header `:11-57`): it owns "the set of all
`Pipeline` objects. Each pipeline gives the constellation's view of a `Window`,
with its script thread **and layout**" `:14-16`, and "Pipelines may share script
threads" `:16`. The constellation talks to each script thread purely by
IPC/message-passing `:11`, and "the `Paint` subsystem runs in the same thread as
the `Servo` instance" `:48`.

`pipeline.rs::spawn()` `:75` sends `ScriptThreadMessage::SpawnPipeline` `:81` into
an `EventLoop` (`constellation/event_loop.rs` — the constellation's *handle* to a
script thread's OS thread), optionally in a sandboxed content process.

**Layout is now in the script thread.** `script_thread.rs` holds a
`layout_factory` `:400` and creates a `LayoutThread`
(`layout/layout_impl.rs:123`, misleadingly named — it is a struct, not an OS
thread) per document. `ScriptThread::update_the_rendering()` `:1134` is Servo's
render step: it reflows each document (`// Note: this will reflow the doc.` `:1260`),
services rAF, and returns true "if any reflows produced a new display list"
`:1133`. Display lists are shipped to **WebRender** which rasterises on the GPU
on its own thread — off-main-thread paint and scroll.

---

## 3. Manuk today — synchronous pipeline, Tokio only for I/O

**The "event loop" is winit's OS loop, not an HTML event loop.**
`shell/src/gui.rs` `App: ApplicationHandler` handles `window_event` `:1118`
synchronously. Every interaction calls straight into layout + paint on the UI
thread:
- Resize `:1130` → `page.relayout_zoomed(...)` → `rerender()`.
- Scroll (`MouseWheel` `:1148`, arrows `:1043`) → mutate `scroll_y` →
  `rerender()` `:445`, which **re-rasterises the visible viewport on the CPU**
  (`page.paint_scrolled` `:454`) and re-uploads a full-screen texture to wgpu.
- Text edit / checkbox toggle / zoom (`edit_focused_input` `:349`,
  `toggle_checkbox` `:293`, `apply_zoom` `:593`) each call `relayout_zoomed`
  then `rerender()` — synchronous style+layout+paint, inline, per keystroke.
- Navigation `:615` uses `self.rt.block_on(...)` to fetch on the shared Tokio
  runtime, then lays out synchronously.

**The pipeline** (`engine/page/src/lib.rs`): `bytes → DOM → cascade → layout →
paint`, all synchronous. `relayout_zoomed` `:461` re-lays-out the whole document.
There *is* damage classification — `relayout_incremental` `:489` returns
`RestyleDamage::{None,Repaint,Reflow,Rebuild}` and has a paint-only fast path
`apply_paint_only` `:533` — and a real display list with `changed_since` /
`damage_since` (`display_list()` `:770`). But the GUI does **not** use the
incremental path: it calls the full `relayout_zoomed` and repaints the whole
viewport every time.

**What Manuk has right:** one long-lived Tokio runtime for network I/O `:182`
(connection/DNS/TLS reuse), a streaming first-paint checkpoint
(`fetch_streaming_page` `:928`), a `FrameTimer` measuring real GPU-present frames
`:1184`, and damage/dirty infrastructure in the engine core.

**Gaps (the source of the latency complaint):**
1. **No frame cadence.** There is no vsync / rAF / `BeginMainFrame`. Work runs
   *synchronously on the input event* — a burst of scroll or keystroke events
   each triggers a full relayout+repaint with no coalescing. Redraws are pushed
   ad-hoc via `request_redraw()`.
2. **No rendering coalescing.** N input events in one frame = N full pipelines.
   There is no "dirty bit + one render per frame" gate like every engine above.
3. **No task prioritisation.** No task queue at all — no way for input to preempt
   a background layout, no microtask checkpoint, no idle work.
4. **Scroll re-rasterises on the CPU.** Every scroll re-runs `paint_scrolled`
   over the whole viewport and re-uploads a texture. No compositor thread, no
   cached layer/tile the GPU can re-blit at a shifted offset.
5. **Coarse relayout.** Any `>= Repaint` change relayouts the whole tree
   (`relayout_incremental` docs `:483-488`); subtree-partial reuse is a TODO, and
   the GUI bypasses even the existing fast path.
6. **`block_on` on the UI thread.** Navigation fetch blocks the winit thread.

---

## 4. Fold-in recommendations — ranked by latency/snappiness leverage

The user complained about latency. The highest-value work is **coalescing and a
frame cadence**, not multiprocess isolation. Ranked:

**① A frame-scheduled render loop with a single dirty bit (highest leverage,
smallest change).** Replace "relayout+repaint inline on every event" with:
events set dirty flags (`needs_layout`, `needs_paint`, `scroll_dirty`) and call
`request_redraw()`; do the actual pipeline **once**, in `RedrawRequested`,
reading those flags. This is Ladybird's `needs_repaint()` gate (`EventLoop.cpp`
step 22) and Blink's `ScheduleAnimation` model, minus threads. Drive redraws off
winit's `AboutToWait` / a vsync-paced timer so a scroll burst collapses into one
frame. Immediately removes the N-events-N-pipelines blowup.

**② Coalesce input → style → layout → paint into ordered phases with early-out
per phase.** Adopt Blink's lifecycle target idea: a scroll needs *no* relayout —
only a paint at a new offset; a text edit needs layout+paint; a color change
needs paint only. Wire the GUI to the **already-existing** `relayout_incremental`
+ `RestyleDamage` + `apply_paint_only` path instead of `relayout_zoomed`, so a
keystroke that only repaints doesn't relayout. This is pure win using code that
already exists in `engine/page/src/lib.rs`.

**③ Cache the painted surface and composite scroll on the GPU (kills scroll
latency).** Today scroll re-rasterises the CPU canvas. Instead: rasterise the
page (or tall tiles) **once** into a GPU texture; on scroll, just change the
sampled `uv`/offset in the existing wgpu present shader (`gui.rs` WGSL `:36`).
This is the WebRender/cc "off-main-thread scroll" insight in a single-threaded
budget — no compositor thread required, just a retained texture. Re-raster only
the newly-exposed band. Biggest perceived-smoothness gain for scrolling.

**④ A real (single-threaded) HTML event loop with microtask checkpoints.**
Needed the moment JS/`requestAnimationFrame`/timers/promises matter. Model it on
Ladybird `EventLoop::process()`: one task → drain microtasks → (once per frame)
"update the rendering" in the spec's order (resize → scroll → media queries →
animations → rAF → layout → paint). Manuk already collects the ingredients
(`subresources()`, streaming, damage) — it needs the *loop* that orders them.

**⑤ A minimal priority scheme (do NOT build Blink's 11 levels).** Two or three
lanes suffice for a lean browser: **input/scroll > rendering/rAF > background
(prefetch, image decode, network completion)**. Enough to keep a keystroke ahead
of a background image decode. Gecko's split (`InputHigh` vs `Normal` vs `Idle`)
is the mental model; three variants is plenty.

**⑥ Move the paint/raster off the UI thread — only after ①–③.** A single
**render/raster worker thread** (page→display-list on the UI thread, raster on the
worker, present on the UI thread) is the one threading split worth the
complexity: it matches Servo's WebRender-on-its-own-thread and Blink's compositor
thread benefit (input stays responsive while a big page rasterises) **without**
the cost of full multiprocess isolation.

**BLOAT to avoid for a lean browser:**
- **Full multiprocess site isolation** (Chromium's browser/renderer/GPU/utility
  process zoo) — enormous IPC/serialization surface; the security model it buys
  is not Manuk's priority and it *hurts* latency (IPC hops) rather than helping.
- **Per-frame/per-page scheduler objects** (`frame_scheduler_impl`,
  `page_scheduler_impl`, budget pools, `find_in_page_budget_pool_controller`) —
  these exist to arbitrate hundreds of frames/tabs; a lean browser needs one
  loop with a dirty bit.
- **An 11-priority `sequence_manager`** — over-engineered for one document.
- **A separate compositor *process*** — a worker *thread* captures the win.

---

## 5. Open questions for frontier research

1. **Async-runtime as the event loop?** Manuk already runs Tokio. Could the HTML
   event loop, rAF cadence, and priority lanes be expressed as Tokio tasks +
   a `tokio::select!` priority poll, rather than a bespoke loop — or does the
   spec's "one task then microtask checkpoint" determinism fight Tokio's
   cooperative scheduler? (Servo runs its own thread loops *alongside* async I/O;
   worth studying which invariants break.)
2. **Vsync without a compositor.** winit exposes redraw requests but not a clean
   vsync signal on all platforms; wgpu present-mode (Fifo) *is* vsync-bound. Can
   present-pacing alone drive a good-enough frame clock, or is an explicit
   display-link needed for jank-free rAF?
3. **How far does single-threaded coalescing scale?** At what page complexity
   does CPU raster on the UI thread miss frame budget even with damage-only
   repaint, forcing the ④/⑥ worker thread? Need the `FrameTimer` p95 numbers
   across a corpus to set the threshold.
4. **Retained-tile invalidation for a compute rasteriser (Vello).** If Manuk
   moves to Vello (GPU compute) per CLAUDE.md, the cc/WebRender tile model may not
   map cleanly — does damage-region → re-encode-scene fit Vello's paradigm, and
   where is the coalescing boundary?
5. **Input prediction / preemption.** Blink inspects *pending* input to bias
   priority before dispatch. Is any of that latency-hiding worthwhile
   single-threaded, or does it only pay off once there is a thread to preempt?
