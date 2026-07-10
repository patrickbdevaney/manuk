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
.format(1234567.89)'` evaluates without error). **Decision:** the en-only ICU data
filter (ICU 64+ `--with-data-filter`, keeping `Intl`) targets that ~14.3 MB → an
estimated **~12–13 MB saving** (English ICU data ≈ 1–2 MB), i.e. a **~1/3 binary
reduction** — this clearly justifies the trim (the plan expected only single-digit MB;
the measured target is larger). The *realization* is engineering-gated: a
`MOZJS_FROM_SOURCE=1` per-OS from-source build + baked `MOZJS_ARCHIVE` (the prebuilt is
full-ICU), strictly build/config — never JIT/GC/sandbox. Tracked as C2's build step.

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
