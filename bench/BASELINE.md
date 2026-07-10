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
| `manuk` headless (linux-gnu, stripped) | **4.1 MB** (4,278,888 bytes) |

Re-measure: `cargo build --release -p manuk-shell --no-default-features && stat -c%s
target/release/manuk`. CI reports this on the static-binary job. Per-target sizes
(musl/win/mac) land once those binaries build in CI (PLATFORM.md).

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

## 4. Frame time — SLOT RESERVED, NOT YET WIRED 🔧

Requires the GPU present loop (`shell` `gui` feature) and is hard to measure in
headless CI. Wire when the Vello tier (A1) or a headful frame-timing hook lands; the
CPU raster time above is the current stand-in for paint cost.
