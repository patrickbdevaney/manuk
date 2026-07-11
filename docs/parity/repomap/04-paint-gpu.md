# REPOMAP 04 — Paint, Compositing, Rasterization & GPU Rendering

How production browser engines turn a laid-out box tree into pixels on a GPU, and
what a lean from-scratch Rust browser (**Manuk**) should fold in. All paths are
absolute under `/home/patrickd/manuk/`.

---

## 1. Scope & sources

The paint stack has four stages everywhere, though engines draw the lines
differently:

1. **Paint / display-list build** — walk the fragment/layer tree, emit a flat,
   resolution-independent list of draw commands (rects, text runs, images, clips).
2. **Compositing / layerization** — decide what becomes an independently
   composited surface; resolve transforms/clips/scroll (increasingly a *tree*
   resolved on the GPU, not baked per-layer).
3. **Rasterization** — turn draw commands into pixels (CPU Skia, GPU Skia
   Ganesh/Graphite, WebRender's batched GPU, or Vello's compute shaders).
4. **Present / damage** — upload/compose tiles, compute the dirty region, swap.

Sources surveyed:

| Engine | Paths |
|---|---|
| **Blink + cc** | `chromium/third_party/blink/renderer/core/paint`, `.../platform/graphics/paint`, `.../platform/graphics/compositing`, `chromium/cc/{paint,layers,tiles,trees,raster}` |
| **Gecko WebRender** | `firefox/gfx/wr/webrender_api/src`, `firefox/gfx/wr/webrender/src`, `firefox/gfx/layers/wr` |
| **Servo** | `servo/components/layout/display_list`, `servo/components/paint` |
| **WebKit** | `WebKit/Source/WebCore/rendering` (RenderLayerCompositor/Backing), `.../platform/graphics{,/ca,/texmap}` |
| **Ladybird** | `ladybird/Libraries/LibWeb/Painting`, `ladybird/Libraries/LibGfx` |
| **Vello / blitz** | `blitz/packages/blitz-paint/src`, `blitz/Cargo.toml` (Vello via `anyrender_vello` crates; Vello itself not vendored) |
| **Manuk (this repo)** | `engine/paint/src/lib.rs`, `engine/compositor/src/lib.rs`, `shell/src/gui.rs` |

---

## 2. Per-engine approach

### 2.1 Blink + cc (Chromium) — property trees + layer lists + tiled raster

Blink paints the fragment tree into a **display list grouped into paint chunks**,
each chunk tagged with a `{transform, clip, effect}` **property-tree state**. The
whole thing is a `PaintArtifact`. A compositing pass *layerizes* chunks into cc
layers and translates Blink's property trees into cc's property trees. cc then
tiles each picture layer and rasters tiles via a pluggable backend.

**Display list & chunks** (`blink/renderer/platform/graphics/paint/`):
- `PaintArtifact` = display list + chunks: `paint_artifact.h:27`,
  `GetDisplayItemList()` `:40`, `GetPaintChunks()` `:45`.
- `DisplayItem` base `display_item.h:31`; `DrawingDisplayItem` wraps a
  `cc::PaintRecord`; `ForeignLayerDisplayItem` injects a ready-made `cc::Layer`
  (canvas/video) `display_item.h:98-108`.
- `PaintChunk` `paint_chunk.h`: `begin_index/end_index` `:146/:150`, **`properties`**
  (the property-tree state) `:163`, `bounds` `:178`. Built by `PaintChunker`
  (`paint_chunker.h:30`, `UpdateCurrentPaintChunkProperties()` `:53`).
- Recording entry point is `GraphicsContext` (`graphics_context.h:183`), whose
  `Canvas()` `:204` is a `cc::PaintCanvas` recording into a `PaintOpBuffer`.

**Paint property trees** — sparse trees of transform/clip/effect/scroll nodes,
referenced by chunk id, decoupling paint from layer geometry. Each is
`X`+`XOrAlias`+`XAlias` (alias nodes collapse to parent):
`transform_paint_property_node.h:81`, `clip_paint_property_node.h:78`,
`effect_paint_property_node.h:78`, `scroll_paint_property_node.h:47`;
the tuple is `PropertyTreeState` (`property_tree_state.h:72`).

**Bridge → cc** (`platform/graphics/compositing/`): `PaintArtifactCompositor::Update`
groups chunks into `PendingLayer`s and reuses layers across frames
(`paint_artifact_compositor.cc:81` `OldPendingLayerMatcher`). `PropertyTreeManager`
(`property_tree_manager.h:63`) converts Blink property nodes → cc nodes
(`EnsureCompositorTransformNode()` `:112`, etc.).

**cc property trees & layer lists** (`cc/trees/`): `PropertyTree<T>`
(`property_tree.h:67`) with `TransformTree`/`ClipTree`/`EffectTree`/`ScrollTree`
(`:175/:444/:455/:588`); nodes are flat structs indexed by id
(`transform_node.h:26`, etc.). **Layer lists, not layer trees**: cc receives a
flat `layer_list_` plus property trees (`layer_tree_host.h:862`
`IsUsingLayerLists()`, `layer_tree_impl.h:190`).

**cc/paint — serialized ops**: `PaintOp`/`PaintOpType` (`cc/paint/paint_op.h:85`);
`PaintOpBuffer` is a contiguous arena of ops with `Playback(SkCanvas*)`
(`paint_op_buffer.h:192`); cc's `DisplayItemList` pairs ops with per-op visual
rects for culling (`cc/paint/display_item_list.h:32`).

**Tiling & rasterization** (`cc/tiles`, `cc/raster`): `TileManager`
(`tile_manager.h:90`, `PrepareTiles()` `:113`) splits `PictureLayerTiling`
(`picture_layer_tiling.h:73`) grids into tiles with priority queues
(`raster_tile_priority_queue*`), and rasters each via a `RasterBufferProvider`
— GPU (`gpu_raster_buffer_provider.*`), one-copy, zero-copy, or software. Skia
backend is Ganesh **or Graphite** (`gpu_raster_buffer_provider.h:65`
`FlushTileRasterGraphiteCommands()`). `RasterSource` (`cc/raster/raster_source.h:40`)
is the immutable snapshot played back per tile.

**Damage / partial present**: cc `DamageTracker` per render surface
(`cc/trees/damage_tracker.h:35`, `GetDamageRectIfValid()` `:53`) drives partial
swap; Blink-side raster invalidation in
`display_item_raster_invalidator.{h,cc}`.

**Innovations to steal:** (1) **paint property trees** — transform/clip/effect/scroll
as sparse trees referenced by chunks; (2) **layer lists** — flat layers + property
trees instead of a nested layer tree; (3) **tiled raster** with per-tile priority
and pluggable backends.

### 2.2 Gecko WebRender (Rust) — retained scene, spatial tree on GPU, picture-cache tiles

WebRender keeps **no CPU-side layer/texture tree**. It retains three things: a
serialized display list, a built scene (spatial tree + clip tree + interned
primitives), and picture-cache GPU tiles. Everything else — batches, the
render-task graph, transforms — is re-derived per frame and resolved on the GPU.
This is the most important reference because it is Rust and it is the modern model.

**Retained display-list API** (`webrender_api/src/`): `DisplayListBuilder`
(`display_list.rs:892`) serializes typed items into a flat byte buffer,
`BuiltDisplayList` (`display_list.rs:177`), shipped to the backend. Items are one
enum, not virtual objects (`display_item.rs:140` `DisplayItem`); spatial nodes are
a separate stream (`SpatialTreeItem` `:131`).

**Two-phase pipeline** — the key structural idea:
- **Scene building** (slow, own thread, only on display-list change):
  `scene_building.rs:415` `SceneBuilder`, `build()` `:511` → `BuiltScene` `:627`.
  **Interning** diffs retained state so unchanged content is skipped
  (`scene_building.rs:15-40`).
- **Frame building** (fast, per frame, on scroll/animation):
  `frame_builder.rs:125` `FrameBuilder`, `build()` `:647` over `&mut BuiltScene`;
  culling `:265`; output `Frame` `:1378`.
  Backend drives both: `render_backend.rs:758`.

**Spatial tree resolved on GPU** (`spatial_tree.rs:147` `SceneSpatialTree`): scroll
offsets, transforms, sticky and reference frames are tree nodes keyed by
`SpatialNodeIndex` (`:63`). `CoordinateSystem` (`:29`) groups axis-aligned nodes so
the vertex shader can fast-path transforms. A scroll is just a node-offset update —
no re-layerization. Clips are a parallel `ClipTree` (`clip.rs:248`), masks
rasterized as render tasks.

**Picture caching / GPU tiling** (`picture.rs:38-80` is the canonical doc): stable
content becomes retained GPU tiles; each tile tracks a dependency list
(primitives, clips, images, transforms) in a **quadtree**
(`invalidation/quadtree.rs:59`); invalidated leaves union into a per-tile dirty
rect used as the scissor for **partial present**. `Tile`/`TileCacheInstance`
(`tile_cache/mod.rs:225/:719`). Compositor surfaces let video/WebGL update without
invalidating tiles (`picture_composite_mode.rs:44`).

**Batching & GPU renderer**: primitives sort into instanced batches
(`batch.rs:618` `AlphaBatchBuilder`, `add_prim_to_batch` `:781`) keyed on
shader+textures; a render-task graph schedules GPU passes
(`render_task.rs`, `render_task_graph.rs:246`); `Renderer` (`renderer/mod.rs:740`,
`render()` `:1332`) executes them with lazily-compiled shaders (`shade.rs:541`,
GLSL in `webrender/res/`) reading primitive data from GPU buffers
(`renderer/gpu_buffer.rs`) and a shared texture cache (`texture_cache.rs:91`).

**Gecko bridge** (`firefox/gfx/layers/wr/`): `WebRenderLayerManager.h:54` is a
`WindowRenderer`, not a layer tree; `WebRenderCommandBuilder.h:38` translates
`nsDisplayList` items directly into a `wr::DisplayListBuilder` — the old
layerization step is *replaced* by display-list emission.

**Innovations to steal:** retained serialized display list + interning (diff the
scene, not pixels); scene-build/frame-build split on separate threads; spatial tree
resolved on GPU (scroll = node-offset update); picture-cache tiles with quadtree
invalidation as the only retained GPU surface.

### 2.3 Servo (Rust) — clean retained-DL pipeline delegating to WebRender

Servo is the cleanest reference for the *pipeline shape*: layout builds a
WebRender display list, a separate paint module owns the WebRender instance and GPU,
and scroll/hit-test are async and compositor-side. Servo rasterizes almost nothing
itself.

- **Layout emits the DL** (`servo/components/layout/display_list/mod.rs`):
  `DisplayListBuilder<'a>` (`:88`) is a thin wrapper whose key field is
  `webrender_display_list_builder: &'a mut wr::DisplayListBuilder` (`:99`). It maps
  Servo concepts to WebRender `SpatialId`/`ClipChainId`/`ScrollTreeNodeId`
  (`:148/:260/:256`) and pushes stacking contexts with `wr::FilterOp`
  (`:395-462`). The "display list" is literally a WebRender item stream.
- **Paint owns the GPU** (`servo/components/paint/painter.rs`): `Painter` (`:76`)
  holds `RenderApi`, `DocumentId`, `webrender::Renderer` (`:99-105`). Bootstrap is
  `webrender::create_webrender_instance` (`:222`). Per frame it builds a
  `Transaction`, `generate_frame(present=true)` (`:554`), `send_transaction`
  (`:583`). Scroll/zoom processed without relayout (`:300`); hit-test delegated to
  `webrender_api.hit_test` (`:566`).

**Takeaway:** the architecture Manuk wants (layout → retained DL → compositor thread
owns renderer, async scroll/hit-test), but WebRender is heavier than Vello. Servo =
pipeline shape; Vello = rasterizer.

### 2.4 WebKit (C++) — RenderLayer → GraphicsLayer retained tree, swappable GPU backends

WebKit promotes some `RenderLayer`s into composited layers, each backed by a
`RenderLayerBacking` owning one or more `GraphicsLayer`s. `GraphicsLayer` is the
platform-neutral retained node; concrete backends are CoreAnimation (Apple) or
TextureMapper/Coordinated (GTK/WPE, GPU via GL).

- **Compositing decision** (`rendering/RenderLayerCompositor.cpp`): two-phase —
  `computeCompositingRequirements` (`:1272`) with overlap map + backing sharing;
  predicate `requiresCompositingLayer` (`:3289`); `reasonsForCompositing` (`:3426`).
  The per-trigger methods are a spec-in-code catalog of *what forces a layer*:
  animation `:3842`, transform `:3878`, backface `:3909`, video `:3933`, canvas
  `:3951`, filters `:3981`, will-change `:3997`, position/sticky `:4097`, overflow
  scroll `:4157`. `requiresOwnBackingStore` (`:3369`) is the memory optimization.
- **Backing** (`RenderLayerBacking.h:74`): one promoted layer can own many
  GraphicsLayers (`m_graphicsLayer`, clipping, foreground/background, scroll
  container) — the source of WebKit's complexity.
- **Abstraction seam** (`platform/graphics/GraphicsLayer.h:110`): retained-tree node
  created via a `GraphicsLayerFactory`, driven by a `GraphicsLayerClient`, painting
  through a `GraphicsContext`. Two backends: `ca/GraphicsLayerCA.h:62` (wraps
  `PlatformCALayer`, tiling in `TileController.cpp`) and
  `texmap/GraphicsLayerTextureMapper.h:36` (portable GL; threaded variant under
  `texmap/coordinated/`).

**Takeaway:** the `requiresCompositingForX` list is the reference for layerization
heuristics Manuk will *eventually* need; the `GraphicsLayer`/`GraphicsContext`
interface is the swappable-backend seam. Heavier than Manuk wants (multi-layer
backings, retained CALayer trees).

### 2.5 Ladybird (C++) — record a flat command list, replay into Skia (now GPU via Ganesh)

Ladybird's model is the pragmatic middle path: paintables **record** a flat command
list, then a **player** replays it into Skia. Migrating web rendering onto Skia gave
them the GPU path essentially for free.

- **Recorder** (`LibWeb/Painting/DisplayListRecorder.h`): flat command vocabulary —
  `fill_rect` (`:39`), `fill_path` (`:49`), gradients (`:69-71`),
  `draw_scaled_decoded_image_frame` (`:75`), `draw_glyph_run` (`:100`),
  `add_clip_rect` (`:102`), rounded corners (`:152`), shadows (`:148-150`), nesting
  (`:134`). Serialized into `Painting/DisplayList.{h,cpp}`.
- **Player = the GPU seam** (`LibWeb/Painting/DisplayListPlayerSkia.cpp`): holds a
  `GrDirectContext` and `SkiaBackendContext` (`.h:61`), includes Ganesh headers
  (`.cpp:28-30`), translates each command into `SkCanvas` calls. CPU vs GPU depends
  purely on whether a live `GrDirectContext` was supplied.
- **LibGfx** (`LibGfx/Painter.h:18`) is now an *abstract* `Painter` whose concrete
  impl is `PainterSkia.cpp` — even the immediate-mode 2D painter is Skia-backed; the
  legacy hand-rolled CPU rasterizer is largely superseded.

**Takeaway:** strong evidence that "record one flat command list, then one pluggable
`DisplayListPlayer`" is the right seam. Swapping Skia for Vello here would mean a
`DisplayListPlayerVello` against the same command vocabulary.

### 2.6 Vello + blitz (Rust) — compute-shader 2D rasterizer (Manuk's intended backend)

Vello is a **compute-based, sort-middle 2D renderer on wgpu**. Unlike Skia/WebRender
(which issue geometry as GPU draw calls per shape or per batch), Vello encodes the
entire scene — every path, clip, gradient, glyph — into flat GPU buffers, then runs
a **pipeline of compute (WGSL) shaders** to rasterize:

- **No per-shape draw calls.** The CPU side only *encodes* into buffers; one
  dispatch chain, not N draws.
- **Sort-middle pipeline:** path flattening → binning/bbox → coarse rasterization
  that bins segments into fixed 16×16 tiles and builds per-tile command lists →
  fine rasterization where one workgroup per tile computes final antialiased
  coverage/blending in a single pass. Cost scales with **pixels + path complexity**,
  largely independent of shape *count*.

Not vendored locally; blitz pulls it as crates and targets all three variants:
`anyrender_vello`, `anyrender_vello_cpu`, `anyrender_vello_hybrid`
(`blitz/Cargo.toml:112-114`), behind an `anyrender::PaintScene` trait.

- **Scene building** (`blitz/packages/blitz-paint/src/`): `paint_scene(scene: &mut
  impl PaintScene, ...)` (`lib.rs:42`) walks the DOM; `render.rs` uses **kurbo**
  geometry (`Affine, Rect, Stroke`) and **peniko** paint (`Fill, Gradient`) —
  Vello's shared Linebender types — and calls e.g. `scene.fill(Fill::NonZero,
  Affine::IDENTITY, bg_color, None, &rect)` (`render.rs:161`).
- **Full command surface** blitz exercises: `fill`, `stroke`, `push_layer`/
  `pop_layer`, `push_clip_layer`, `draw_box_shadow`, `draw_glyphs`, `draw_image`,
  `append`, `reset` — essentially all of Vello's API.
- **Layers = clip + blend + alpha** (`blitz-paint/src/layers.rs`):
  `maybe_push_layer` (`:45`) implements CSS stacking-context isolation; fast path
  `push_clip_layer` when `opacity==1.0 && filter.is_none()` (`:68`), else full
  `push_layer(Mix::Normal, opacity, ...)` (`:70`). Note a **real Vello clip-depth
  cap** (`:60-67`).

**Takeaway (Manuk's backend):** adopt the `blitz-paint` shape directly — a scene
generator that walks the paintable tree and emits kurbo+peniko `fill`/`stroke`/
`draw_glyphs`/`push_layer` into a `vello::Scene`, then one `Renderer::render`
dispatch per frame. GPU rasterization with zero per-shape draw calls and no need to
implement WebKit-style layerization or a WebRender-style batching renderer. The
`anyrender::PaintScene` trait is worth copying as the seam so a `vello_cpu` fallback
and `vello_hybrid` (broader GPU compat) drop in unchanged.

### Cross-engine synthesis

| Engine | Intermediate repr | Rasterizer | GPU model | Steal |
|---|---|---|---|---|
| **Blink+cc** | PaintArtifact: chunks + property trees; `PaintOpBuffer` | Skia (Ganesh/Graphite) | tiled raster, layer lists + property trees | property trees; layer lists; tiled raster |
| **WebRender** | retained serialized DL + built scene | own GPU (batched) | retained scene, spatial tree on GPU, picture-cache tiles | retained scene + interning; scene/frame split; GPU spatial tree; quadtree tile invalidation |
| **Servo** | WebRender DL | WebRender | delegated | pipeline shape: layout→DL, compositor thread owns GPU, async scroll |
| **WebKit** | GraphicsLayer retained tree | GraphicsContext → CA/TextureMapper | retained tiled layer tree | compositing-reason catalog; pluggable GraphicsLayer seam |
| **Ladybird** | own flat command list | Skia (Ganesh) | draw-call batching | record-once, one pluggable player seam |
| **Vello/blitz** | `vello::Scene` (kurbo+peniko) | **Vello compute** | **sort-middle, no per-shape draws** | the whole backend + `anyrender::PaintScene` trait |

---

## 3. Manuk today

Manuk's paint is an honest **CPU rasterizer with a `wgpu` fullscreen-quad present**,
behind a `Painter` trait explicitly designed to accept a Vello GPU tier later.

**Display list** (`engine/paint/src/lib.rs:20-205`): `DisplayList { items:
Vec<DisplayItem> }` where `DisplayItem` is `Rect | Text | Image`
(`lib.rs:87-102`). Built from the `LayoutBox` tree by `build_layered`
(`lib.rs:127`), which groups items per box and stably sorts by **effective
z-index** — a flat approximation of CSS stacking contexts (a positioned element's
z applies to its whole subtree). Overflow clipping is a per-box `clip_map` of
ancestor rects intersected at paint time (`layered_groups` `:142`). This is a
single flat list — **no paint chunks, no property trees, no layer list.**

**Rasterization** (`engine/paint/src/lib.rs:317-676`): the `Painter` trait
(`:317`) has one impl, `CpuPainter` (`:328`), backed by `tiny-skia` for fills
(`fill_rect` `:558`) and image blits (`blit_image` `:573`, bilinear), plus
`swash`/`fontdue`-rasterized glyph coverage blitting (`blit_coverage` `:624`,
`blit_color_glyph` `:512`). `render_scrolled` (`:381`) paints only the visible
viewport by shifting content up by `scroll_y` and clipping. Deterministic and
**headless** — no GPU/display needed — which is why the render-to-PNG test path
works (`save_png` `:242`). Confirmed dep: `tiny-skia` (`engine/paint/Cargo.toml:21`);
**no `vello` dependency exists yet** (it is aspirational in the module docs).

**Damage primitives exist but are not wired to the GPU** (`engine/paint/src/lib.rs:28-74`):
`DisplayList::changed_since` (equality check to skip idle re-upload) and
`damage_since` (a coarse union of bounding rects of items that differ *by index* —
a safe over-approximation; text contributes a generous box since width isn't
stored). The compositor crate adds `Damage` (dirty-rect accumulation with a `full`
short-circuit, `engine/compositor/src/lib.rs:287-346`) and `Viewport::scroll_by`
which marks damage `full` on any scroll (`:374`). **These are policy/state only.**

**Compositor crate is policy, not a real compositor** (`engine/compositor/src/lib.rs`):
`TabManager` assigns per-tab render **tiers** (`FocusedGpu` / `BackgroundCpu` /
`Hibernated`, `:144-283`) for the isolate-per-tab memory model; `FrameTimer`
(`:58-139`) reports avg/p95/FPS/jank; `mem::process_rss_bytes` reads Linux
`/proc/self/status` (`:31`). There are **no compositor layers, no tiling, no
property/spatial tree** here — the name is aspirational.

**GPU present path** (`shell/src/gui.rs`): `winit` window + `wgpu` surface presenting
the CPU canvas as a GPU-sampled fullscreen triangle (WGSL `:33`, pipeline `:1325+`).
Each frame: `upload()` (`:1443`) **creates a brand-new `Rgba8Unorm` texture and
`write_texture`s the *entire* canvas** (`:1449-1473`), then `draw()` (`:1491`) does
one `draw(0..3)` and `frame.present()`. This is a working `wgpu` path into which a
`VelloGpuPainter` slots — but it is unverified headlessly (needs a display), and it
is **full-frame every time**: no damage-driven partial upload, no persistent texture
reuse, no partial present.

**Honest gaps:**
1. **No real compositor layers** — one flat display list, one full-viewport CPU
   raster; z-index and overflow-clip are flat approximations, not stacking
   contexts / clip trees.
2. **No property/spatial tree** — transforms, clips and scroll are baked at paint
   time (`render_scrolled` subtracts `scroll_y`), so every scroll is a full repaint
   (`Viewport::scroll_by` marks `Damage` full).
3. **Damage not wired to present** — `changed_since`/`damage_since`/`Damage` exist
   and are unit-tested but the GPU `upload` ignores them and re-uploads the whole
   canvas each frame.
4. **GPU path unverified headlessly** — the `wgpu` present needs a display; only the
   CPU raster is CI-verifiable.
5. **No Vello backend** — the `Painter` trait has exactly one (CPU) impl; `vello` is
   not even a dependency.

---

## 4. Fold-in recommendations (ranked by leverage for a lean/fast browser)

The big architectural question: **retained WebRender-style GPU scene, Vello compute
rasterizer, or keep CPU tiny-skia + selective GPU?**

**Recommendation: Vello compute rasterizer behind the existing `Painter`/scene seam,
plus a small amount of WebRender's *architecture* (retained scene + spatial tree),
but NOT WebRender's renderer itself.** Vello gives GPU-quality rasterization for a
fraction of WebRender's code (WebRender is ~a hundred files of batching, segments,
render-task graph, texture cache, GLSL). For a *lean* browser, Vello's "encode a
`Scene`, one dispatch" model is the best speed/leanness tradeoff: you get
zero-per-shape-draw-call GPU raster without writing a batching renderer. blitz already
proves the exact wiring against a DOM.

Ranked:

1. **Adopt a `Scene`-style command list as the paint IR, mirroring Ladybird's
   recorder + one pluggable player** (highest leverage, low risk). Generalize
   `DisplayList` slightly and define the seam as an `anyrender::PaintScene`-shaped
   trait: `fill`/`stroke`/`draw_glyphs`/`draw_image`/`push_clip_layer`/`push_layer`/
   `pop_layer`. This future-proofs the split: `CpuPainter` (tiny-skia) stays the
   headless/background tier; a `VelloPainter` becomes the focused tier — both consume
   one IR. This is a refactor of `engine/paint/src/lib.rs`, no new subsystems.
   *Files:* `engine/paint/src/lib.rs:317` (`Painter` trait), model on
   `blitz/packages/blitz-paint/src/{lib,render,layers}.rs`.

2. **Add the `VelloPainter` GPU tier** (high leverage). Depend on `vello` (+
   `vello_cpu` as the fallback that keeps CI headless-verifiable), build a
   `vello::Scene` from the same IR using kurbo+peniko, and render it into the shell's
   existing `wgpu` device/queue instead of the CPU-canvas-to-texture upload. Reuse
   the surface/config already in `shell/src/gui.rs:1325+`. Mind Vello's clip-depth
   cap (`blitz-paint/src/layers.rs:60`).

3. **Wire the existing damage primitives into present** (high leverage, already
   half-built). Use `DisplayList::damage_since`/`Damage::bounding` to (a) skip
   upload entirely when `!changed_since`, and (b) partial-upload only the dirty
   sub-rect via `write_texture` with an offset origin into a **persistent** texture
   instead of allocating a new full texture every frame (`shell/src/gui.rs:1443`).
   Even on the CPU tier this cuts idle-frame cost to near zero.

4. **Introduce a minimal property/spatial tree so scroll stops being a full repaint**
   (medium leverage, medium effort). Copy WebRender's *idea*, not its code: a small
   arena of transform/clip/scroll nodes (Blink `cc/trees/*_node.h` structs are the
   simplest reference; `firefox/gfx/wr/webrender/src/spatial_tree.rs:147` the modern
   one), with the display list referencing node ids. Then a scroll is a node-offset
   update and only newly-exposed content is dirty — the single biggest interactive
   speed win after GPU raster.

5. **Layerization heuristics — defer** (low near-term leverage). Only when you need
   composited animations/video does a real compositor pay off. When you do, WebKit's
   `requiresCompositingForX` catalog
   (`WebKit/Source/WebCore/rendering/RenderLayerCompositor.cpp:3842-4157`) is the
   spec-in-code; keep it a *layer list + property trees* (Blink model) not a nested
   layer tree.

**BLOAT to avoid:**
- **Do NOT vendor WebRender.** Its batching/segmentation/render-task-graph/texture-cache
  machinery (`firefox/gfx/wr/webrender/src/{batch,render_task,texture_cache}.rs`, GLSL
  in `res/`) is enormous and exists to be a *general* renderer; Vello replaces all of
  it with compute shaders you don't maintain.
- **Do NOT build Skia/Ganesh or a C++ Skia binding.** tiny-skia already covers the CPU
  tier; Vello covers GPU. Skia is a huge native dependency antithetical to a lean Rust
  browser.
- **Do NOT replicate WebKit's multi-GraphicsLayer-per-backing structure** — it is the
  main source of WebKit compositor complexity and unnecessary until animation demands it.
- **Do NOT tile prematurely.** cc/WebRender tiling pays off at large scroll surfaces;
  a viewport-sized Vello scene + dirty-rect present is far simpler and enough for a
  lean browser's first GPU milestone.

---

## 5. Open questions for frontier research

1. **Vello maturity & headless CI.** Vello is alpha; `vello_cpu`/`vello_hybrid` are
   younger still. Can the CPU/hybrid variant serve as the *deterministic headless*
   reference the current tiny-skia path provides (for render-to-PNG tests), or do we
   keep tiny-skia purely for CI and Vello purely for on-screen? What is the pixel-diff
   tolerance between tiny-skia and vello_cpu?
2. **One IR for two rasterizers.** Can a single `PaintScene`-shaped command list drive
   both tiny-skia (imperative, immediate) and Vello (encode-then-dispatch) without
   leaking Vello's clip-depth cap and layer semantics into the CPU path? Where does the
   abstraction cost show up?
3. **Damage granularity vs. compute raster.** Vello re-rasterizes a whole `Scene`
   region per dispatch; fine-grained damage rects matter less than for a
   texture-upload model. Is per-scene-region dirty tracking (WebRender picture-cache
   tiles, `firefox/gfx/wr/webrender/src/picture.rs:38-80`) worth it for Manuk, or does
   "re-encode the viewport scene, present" win on simplicity?
4. **Scroll without full re-encode.** With a spatial tree, can Manuk translate a
   retained Vello `Scene` by a scroll offset (transform node) and re-present without
   re-walking layout — approaching WebRender's async-scroll behavior on a Vello backend?
5. **Per-tab tiers vs GPU memory.** `TabManager` promises `FocusedGpu` vs
   `BackgroundCpu` (`engine/compositor/src/lib.rs:144`). What is the real GPU-memory
   and context cost of keeping the focused tab on Vello while N background tabs hold
   tiny-skia canvases, and where is the crossover that should trigger hibernation?
6. **Text on GPU.** Glyph atlas caching for Vello (`draw_glyphs` + a peniko/parley
   glyph cache) vs the current `swash` coverage blit — what is the quality/perf
   tradeoff, and does subpixel/hinting parity survive the move to GPU coverage?
