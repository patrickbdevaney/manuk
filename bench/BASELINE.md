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

The network leg (fetch) is measured separately once the pooled streaming client
(P0.4) lands; end-to-end click-to-first-paint is measured once the first-paint
checkpoint (B-latency) exists.

## 3. Per-tab baseline RSS — SLOT RESERVED, NOT YET WIRED 🔧

Requires a running tab/isolate to attribute. Wire after **C1** (tab freeze/discard)
and **G-e** (per-tab accounting) exist; measurement is platform-specific
(Linux `/proc/self/status` VmRSS; macOS/Windows via `getrusage`/PSAPI) and shares the
shared-process self-attribution wrinkle noted in G-e. Not fabricated here.

## 4. Frame time — SLOT RESERVED, NOT YET WIRED 🔧

Requires the GPU present loop (`shell` `gui` feature) and is hard to measure in
headless CI. Wire when the Vello tier (A1) or a headful frame-timing hook lands; the
CPU raster time above is the current stand-in for paint cost.
