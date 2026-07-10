# Measurement baseline (P0.2)

The four CLAUDE.md §8 metrics. Every size/perf-affecting change reports before/after
against these numbers (working-agreement requirement). Re-measure with the commands
shown; update this file when the baseline moves intentionally.

Baseline captured **2026-07-09** on `x86_64-unknown-linux-gnu`.

## 1. Binary size — WIRED ✅

Headless release binary (`manuk`, `--no-default-features`, LTO + `panic=abort` +
`opt-level="s"` + stripped):

| Build | Size |
|---|---|
| `manuk` headless (linux-gnu, stripped) | **5.1 MB** (5,347,184 bytes) |
| `manuk` + `--features spidermonkey` (JS engine linked) | **38.0 MB** (39,797,944 bytes) |

Re-measure: `cargo build --release -p manuk-shell --no-default-features && stat -c%s
target/release/manuk`. CI reports this on the static-binary job. Per-target sizes
(musl/win/mac) land once those binaries build in CI (PLATFORM.md). *(The 4.1 MB → 5.1
MB baseline growth is this session's added engine surface — streaming, tab
model/compositor accounting, incremental relayout, etc.)*

**C2 measurement (2026-07-10) — SpiderMonkey binary-size contribution.** The number the
ICU-trim decision is gated on:

| Component | Size |
|---|---|
| SpiderMonkey adds to the binary (38.0 − 5.1) | **~32.9 MB** |
| — of which **ICU data** (`icu_data.o`, all-locales) | **~14.3 MB** (15,032,552 B) |

`Intl.*` is functional in the built binary (`manuk eval 'new Intl.NumberFormat("en-US")
.format(1234567.89)'` evaluates without error). The *realization* is engineering-gated: a
from-source build (the prebuilt archive is full-ICU), strictly build/config — never
JIT/GC/sandbox.

### C2 REALIZED (2026-07-10) — `spidermonkey-noicu`, measured with a control ✅

The from-source path now **works here** and the ICU trim is **implemented and measured**,
not estimated. `engine/js` gained a `spidermonkey-noicu` feature: the same embedding
surface with mozjs's `intl` cargo feature **off**, so `icu_capi` leaves the dependency
graph and `js/src/configure` runs `--without-intl-api`. This is a *sanctioned build-config
flag on the published `mozjs` crate* — it does not touch SpiderMonkey internals.

Headless release, stripped (this file's metric — `--no-default-features`):

| Build | Size |
|---|---|
| `--features spidermonkey` (intl/ICU) | **38.0 MB** (39,851,304 B) |
| `--features spidermonkey-noicu` | **17.6 MB** (18,455,040 B) |
| **Saved** | **20.4 MB (53.7%)** |

**Control (why the number is trustworthy).** The noicu build is *from-source* while the
intl build links the *prebuilt archive*, so their diff conflates two variables. Building
from-source **with** `intl` isolates them (gui-default profile, stripped):

| Build | Size |
|---|---|
| A: prebuilt + intl | 44,941,320 B |
| B: from-source + intl *(control)* | 44,403,776 B |
| C: from-source, noicu | 23,594,304 B |

`A − B` = **0.5 MB** → prebuilt vs from-source is near-equivalent. `B − C` = **19.8 MB
(46.9%)** → that is ICU/`intl`'s **true** cost. The earlier ~14.3 MB estimate counted only
`icu_data.o` and undercounted ICU *code* + ICU4X.

**Verified behavior (not assumed).** The ICU-free engine still executes real JS —
`eval 'let s=0;for(let i=0;i<200000;i++)s+=i%7;s'` → `599994` (JIT loop), regexp → `99`,
`JSON` round-trip → `3`. And `Intl` is genuinely gone: `typeof Intl === "undefined"` → `1`,
`new Intl.NumberFormat("en-US")` **throws**; the intl build reports `typeof Intl ===
"object"` → `1`. `libjs_static.a` carries **11,492 ICU symbols** with intl, **0** without.

**Tradeoff — why this is opt-in, not the default.** `--without-intl-api` removes the whole
`Intl` object, a real web API. That is a spec-compliance regression, so `spidermonkey`
(full, prebuilt) stays the default and `spidermonkey-noicu` is an explicit opt-in for
size-constrained targets. The more surgical variant — keep `Intl`, ship en-only ICU data —
is now *unblocked* (it needs the same from-source path) and is the tracked follow-up.

**Reproducing (no sudo required).** The from-source build needs clang + llvm-objdump +
libclang + cbindgen. On this box they came from conda-forge into an isolated env, plus
`cargo install cbindgen`:

```
conda create -n mozbuild -c conda-forge -y clang=18 clangxx=18 llvm-tools=18 lld=18 libclang=18
conda activate mozbuild && cargo install cbindgen
export CC=clang CXX=clang++ LIBCLANG_PATH="$CONDA_PREFIX/lib"
# bindgen's libclang doesn't inherit the driver's gcc-toolchain probe → point it at libstdc++:
export CLANGFLAGS="-I/usr/include/c++/13 -I/usr/include/x86_64-linux-gnu/c++/13 -I/usr/include/c++/13/backward"
cargo build --release -p manuk-shell --no-default-features --features spidermonkey-noicu
```

`autoconf2.13` is **not** needed (mozjs ships a pre-generated `js/src/configure`).
Platform: built + verified on Linux x86_64; the feature is [XP] but per-OS from-source
builds are unverified on macOS/Windows.

## 2. Click-to-navigate latency — WIRED ✅ (render proxy)

Deterministic CPU-pipeline proxy (`cargo bench -p manuk-page --bench pipeline`),
fixed in-memory page, no network variance:

| Benchmark | Median |
|---|---|
| `load_800` (parse + style + layout) | **51.3 µs** |
| `load+paint_800x600` (full CPU pipeline) | **758 µs** |

**First-paint checkpoint (B-latency), added 2026-07-10** — a large streamed article
(head + above-the-fold, then a 400-paragraph tail):

| Benchmark | Median |
|---|---|
| `streaming_first_paint` (head+fold prefix only) | **13.6 µs** |
| `streaming_full_load` (whole ~400-paragraph document) | **1.55 ms** |

The first paint (above-the-fold) is laid out in **~113× less time** than the full
document — that gap is the click-to-first-paint win the streaming parser
([`manuk_html::StreamParser`] → `Page::load_streaming`) buys: the user sees content at
the head-complete checkpoint, before the tail streams in. (End-to-end over a *real*
slow socket additionally needs a chunked-fetch API in `manuk-net` — `fetch()` buffers
the body today; that's the next enabler.)

The network leg (fetch) is measured separately once the pooled streaming client
(P0.4) lands.

## 3. Per-tab baseline RSS — WIRED (Linux) ✅ / per-tab attribution 🔧

`manuk_compositor::mem::process_rss_bytes()` reads whole-process RSS from
`/proc/self/status` `VmRSS` on Linux (macOS `getrusage`/Windows PSAPI need a platform
crate — return `None`, engineered-unverified). The shell `render` prints it.

| State | Process RSS |
|---|---|
| headless render of example.com (`--no-default-features`, 800px) | **~60.6 MB** |

Most of that baseline is `fontdb`'s loaded system-font set + the process floor; a
freshly-rendered page's fragment tree adds far less. **Per-tab attribution** is the
[`TabManager::total_mem`] heap *estimate* ([`Page::estimated_bytes`]) — this RSS figure
is the ground-truth reality check (tabs share one process, so true per-tab RSS is not
directly separable — the shared-process self-attribution wrinkle from G-e). C1 proves
the *retained-heap* drops on discard; whether RSS follows depends on the allocator
returning freed pages to the OS.

## 4. Frame time — WIRED (CPU raster + GPU present) ✅

`manuk_compositor::FrameTimer` is a rolling per-frame instrument (last / average /
p95 / FPS / jank vs a `FRAME_BUDGET_60FPS` ~16.67 ms budget), wired at both tiers.

| Frame | Time | How |
|---|---|---|
| CPU raster `paint_frame_800x600` (SAMPLE, re-paint) | **~675 µs** | `cargo bench -p manuk-page --bench pipeline -- paint_frame` |
| CPU raster, live example.com (800×143, +first paint) | ~3.2 ms | `manuk render …` prints `frame:` |
| **GPU present** (winit/wgpu, 240 back-to-back frames) | **avg 0.64 ms, p95 1.07 ms, 0/240 jank (~1562 fps)** | `manuk browse <url> --frames 240` (`gui` feature) |

The GPU-present figure is a **real on-screen measurement on X11 hardware** (the shell
`gui` redraw loop times each `gpu.draw()`); the present is uncapped (no vsync throttle
in this mode), so it measures raw per-frame present cost, not the display refresh rate.
All tiers are far under the 16.67 ms 60-fps budget for representative pages. The Vello
GPU-compute tier (A1) is monitor-upstream and slots behind the same `Painter` seam /
`FrameTimer`.
