# BUILD AND DEPENDENCIES — what is actually compiled, and what only looks like it is

## `./stylo` IS NOT BUILT. Nothing in this repo compiles it.

This is the single most expensive piece of misinformation the repo has ever contained, so it is written
here in plain terms:

| Question | Answer |
|---|---|
| Is `./stylo` tracked in our git? | **No.** `.gitignore` line 26 ignores `stylo/`. **Zero files tracked.** |
| What is it, then? | A **separate `git clone` of `github.com/servo/stylo`**, with its own `.git` and its own upstream remote. It is a **reference checkout**, local to one machine. |
| Is it a workspace member? | **No.** |
| Is there a `[patch.crates-io]`? | **No.** |
| Is there a path dependency on it? | **No.** |
| So where does Stylo come from? | **crates.io.** `Cargo.lock` pins `stylo 0.19.0` with `source = "registry+…"` and a checksum. Same for `selectors`, `servo_arc`, `stylo_atoms`. |

**Therefore: editing anything under `./stylo` changes nothing.** The edit does not reach the binary, is
not committed, is not shipped, and disappears on a fresh clone of this repo.

**It has already cost a tick.** In tick 42 the `:has()` work began by flipping
`parse_has() -> true` in `stylo/style/servo/selector_parser.rs`, rebuilding, and observing… no change.
That discovery **re-priced the whole decision** — from *"a one-line build-flag delta"* to *"vendor Stylo
and pay a re-apply tax on every dependency bump"* — and is the reason `:has()` was ultimately solved
with a **hand-rolled supplement** over our own selector engine instead.

> **A dirty reference checkout is, by definition, someone believing an edit matters when it cannot.**
> `verify.sh` therefore fails if `./stylo` has local modifications.

## We are NOT forking Stylo. If we ever do, there is exactly one sanctioned way.

The fork surface is deliberately kept as a **short, visible list** in `STATUS.md` — a growing list is
the signal that the rule is being abused. It currently reads **(none yet)**, and that is the truth.

**If a fork ever becomes necessary**, the only sanctioned mechanism is:

```toml
# Cargo.toml — the ONLY way a patched Stylo may enter the build.
[patch.crates-io]
stylo = { path = "vendor/stylo" }     # tracked IN THIS REPO, not a stray clone
```

…with the fork **tracked in our repository** (not gitignored), the delta **named and minimal**, and a
**gate that fails if a dependency bump silently reverts it** — otherwise a `git checkout` of the vendor
directory quietly deletes a capability and nothing says so.

**Anything else is a phantom fork**: an edit that looks load-bearing and is inert.

## The ladder, when a vendored engine cannot do something the web requires

**The capability wins. The question is never *whether*, only *at what cost*:**

| # | Option | Cost |
|---|---|---|
| 1 | Flip a pref | none |
| 2 | A named, minimal flag delta in vendored source | a diff to re-apply on every bump |
| 3 | **A hand-rolled supplement** for the specific gap | real code, real tests — *this is where `:has()` landed* |
| 4 | A hand-rolled module | large |
| — | **Give up the capability** | ❌ **never** |

**Two hard rules survive unchanged from the original constitution:** never copy Blink/Gecko **code**
(algorithm extraction only, cited by reference), and never fork an engine's **algorithms** (no rewriting
the cascade, no reimplementing the JIT).

## Features are not optional when you run a gate

`manuk-page` does **not** enable `spidermonkey` by default. `verify.sh` runs the gates with
`--features stylo,spidermonkey`, because **ADR-011 requires the gates to run the SHIPPING
configuration** — gating on a no-JS build measures a browser no user has.

**Running `cargo test -p manuk-page --test g_globals` without the features runs an engine with no JS
engine at all.** Every script "fails", every gate goes red, and the red is meaningless. This has already
caused working code to be deleted on a false signal (PROCESS #32). **Run a gate the way `verify.sh` runs
it, or you are not running the gate.**

## MSRV

The workspace MSRV is **1.80**. Media work needs **1.92** (`re_mp4`) and **1.85** (`symphonia`) — the
bump is a prerequisite, not a detail (`docs/loop/MEDIA.md`).

---
# Backfill — mechanisms recovered from ticks 1–42 (pre-wiki)

## mozjs links a PREBUILT ARCHIVE by default — which is why the ICU trim looks impossible and isn't

`servo/mozjs` links a **prebuilt archive** (Servo's fixed config, **with** Intl+ICU), which is why it
"builds in seconds" **and why custom configure flags appear to have no effect** — they only apply under
`MOZJS_FROM_SOURCE=1` (a ~30-minute build). Amortize with `MOZJS_CREATE_ARCHIVE` / `MOZJS_ARCHIVE`.

**Two recorded beliefs that turned out WRONG:**

1. *"No ICU knob exists; it needs a source patch."* **False** — mozjs publishes an **`intl` cargo feature**
   (`mozjs/intl` → `mozjs_sys/intl` → `dep:icu_capi`). Turning it off makes `js/src/configure` run
   `--without-intl-api`.
2. *"It needs `autoconf2.13`."* **False** — mozjs ships a **pre-generated `js/src/configure`.** No sudo
   needed; clang/lld/libclang from conda-forge and cbindgen via `cargo install` suffice.

⚠ **The non-obvious blocker: bindgen's libclang does NOT inherit the clang driver's gcc-toolchain probe**,
so `CLANGFLAGS` must point at libstdc++ (`-I/usr/include/c++/13`) or the build dies with
**`'utility' file not found`.**

## ICU is 47% of the binary — and only a CONTROL BUILD reveals it

Measured on a real linked, **stripped release** binary: JS-less baseline **5.1 MB** → **38.0 MB** with
SpiderMonkey. Disabling `intl` → **17.6 MB**, a **20.4 MB (53.7%)** saving.

**But because the no-ICU build is from-source while the default is prebuilt, a CONTROL BUILD (from-source
*with* intl) is needed to isolate the variables:** prebuilt-vs-source is only **0.5 MB**, so **ICU's true
cost is 19.8 MB (46.9%).**

*An earlier estimate of ~14.3 MB was an undercount — it measured only `icu_data.o` and missed ICU **code**
plus ICU4X.*

**Verification, not assumption:** `libjs_static.a` has **11,492 ICU symbols with `intl` and 0 without**, and
the ICU-free engine **still runs real JS** (JIT loop, regexp, JSON) while `typeof Intl === "undefined"`.

**Tradeoff, which is why it stays opt-in:** `--without-intl-api` **deletes the whole `Intl` web API** — a
real spec regression. **The surgical variant is to keep `Intl` and ship en-only ICU data** via
`--with-data-filter` (Mozilla's own preferred cleanup; **Firefox ships 459 locales**).

## `servo/mozjs` ships prebuilt archives for every platform EXCEPT musl

macOS (x86_64 + aarch64), windows-msvc (x86_64 + aarch64), linux-gnu (x86_64 + aarch64) — **no musl
prebuilt**, so a musl build of SpiderMonkey is **from-source only.** Stylo, by contrast, is **pure Rust and
portable by construction.** *A static musl binary is cheap ONLY if it does not need the JS engine.*

## The Stylo feature adds ~70–90 crates, 44 of them `icu_*` — and needs Python 3 + Mako at BUILD time

The weight is concentrated and specific: **44 `icu_*` crates** pulled transitively by `icu_segmenter` (which
stylo depends on with `features = ["auto", "compiled_data"]`), where **`compiled_data` bakes locale data as
`const` blobs** — per Mozilla's own ICU4X docs this can add **tens of MB** and consume **~1 GB of build-time
RAM** (dead-code elimination reclaims unused locales).

⚠ **A hard toolchain requirement: `stylo/style/build.rs` invokes `properties/build.py` and `panic!`s if
`python3` is absent.**

**The good news:** **no nightly and no C compiler** on the `servo` feature path (bindgen/C is `gecko`-only);
**stable Rust suffices.**

## `cargo tree` OVERTURNED the received wisdom about duplicate dependencies

The recorded belief that the build carried *"two `tiny-skia` / two `cssparser` versions"* was **wrong on both
counts.**

- The default build has **exactly ONE `cssparser` (0.34)**; the second (0.37) appears **only under
  `--features stylo`** (Stylo brings its own).
- The duplicate **`tiny-skia 0.11.4` is a TRANSITIVE of `winit → sctk-adwaita`** (Wayland client-side
  decorations, GUI+Linux only) — **not a competing copy of the paint crate's 0.12.**
- **The default engine+agent graph pulls ZERO `wgpu`/`winit`/`sctk`.**

> **The feature gating was already correct, and the "fix" would have been for a problem that did not
> exist.** *Adopt `cargo tree --duplicates` / `cargo bloat` / `cargo-udeps` as **standing measurement**, not
> one-off audits.*

## Vello LOOKS ready and isn't — API stability, not features, is the blocker

Sparse-strips is **0.0.7 with no API-stability guarantee**; `vello_cpu` is feature-complete enough to be
"broadly usable" (blur, drop-shadow, clipping, gradients) **but its API is explicitly not stable**;
`vello_hybrid` is "roughly beta" with the same caveat.

**The corroborating datapoint: Servo adopted Vello ONLY for the bounded `<canvas>` surface and kept
WebRender for page compositing.**

> **Keep `tiny-skia` behind a `Painter` trait — the seam makes waiting cheap** — and adopt `vello_cpu` the
> moment its API stabilizes.

## No pure-Rust decoder exists for WOFF2 or for the H.264 the web actually serves

**`woff2` 0.2/0.3 do not build**, so WOFF2 web fonts — **i.e. most real web fonts** — are **silently
dropped** by any loader that only accepts raw sfnt magic. **Neither swash nor skrifa decompresses WOFF2.**

**Video is worse: `openh264` and `rusty_h264` are Constrained-Baseline ONLY** and cannot decode the H.264
the web actually serves (High profile: CABAC + B-frames + 8×8, libx264's default). **This is exactly why
Firefox uses OpenH264 for WebRTC and NEVER for `<video>`.** *`rusty_h264`'s Rust-purity is real and the
capability is not: it is a TRAP.*

**The escape:** YouTube's no-MSE fallback is **`avc1.42001E` — Baseline, 360p** — which openh264 **can**
decode with **zero system dependencies**. So the cheap decoder is the right **first** rung, **behind a trait
defined in tick 1** that makes High-profile via `cros-codecs`/ffmpeg **a swap rather than a rewrite.**

## The media crate ecosystem is full of names that promise a capability they do not have

- **`symphonia` is NOT a video demuxer** — its ISO-MP4 `SampleEntry` has **`// Video,` commented out**. Audio
  only.
- **`re_video` shells out to an `ffmpeg` BINARY** (`ffmpeg-sidecar`) — *a licensing dodge, not an
  architecture.*
- **`rav1d` upstream has NO Rust API** (it is a C-ABI drop-in for libdav1d). **`re_rav1d`** is the fork with
  one.
- **`mp4parse` (Firefox's) is a box parser, not a sample reader** — the demuxer that uses it is **C++ inside
  Gecko.**
- **`servo-media` drags all of GStreamer.**
- **A/V sync is hand-rolled (~150–250 lines) because no crate does it.** Take **`cpal`, not `rodio`** —
  ***`rodio`'s `Sink` HIDES the clock, and the clock is the thing you need.***

## ⚠ Do NOT advertise `MediaSource` before it works

**YouTube serves progressive 360p precisely BECAUSE it cannot find MSE.** Advertising
`MediaSource`/`ManagedMediaSource` you cannot honour **turns a working YouTube into a black rectangle.**

*Same discipline as `canPlayType() === ""` and a rejected `play()` promise: **honest "cannot" is a supported
code path in every player library; a lying "can" is not.***

**EME/Widevine is permanently unreachable** (the CDM is a proprietary binary needing a per-browser licensing
relationship; **`OpenWV` ships without a device identity and is a key-extraction path, not a licensing
one**) — which permanently excludes Netflix, Disney+, Max, Hulu, Prime Video, Apple TV+, Peacock and Spotify
web, **and excludes essentially nothing else**, because ordinary-web `<video>` is **muted autoplay loops
with no DRM, no audio and no ABR.**

## The render pipeline compiles to wasm32 — but `usize` in a packed handle does not

**The whole render pipeline minus JS builds for `wasm32-unknown-unknown`:** `dom`, `css`+**Stylo**,
`layout`(taffy), `paint`(tiny-skia), `html`, `text`. **Stylo — the obvious risk — compiles cleanly.**
SpiderMonkey (C++) is deliberately absent, which is *why* the in-browser demo is JS-free. Proven repeatably
by `scripts/wasm-check.sh`.

**The one real portability bug, and it is a general lesson:** `NodeId` packed `generation << 32 | index`
into a **`usize`**. On `wasm32` (and any 32-bit target) `usize` is **32 bits**, so `<< 32` **overflows and
the crate does not compile** — *"this arithmetic operation will overflow"*, invisible on a 64-bit dev
machine, fatal on the target. **Any type that packs bit-fields into a pointer-width integer is a latent
32-bit bug.** Back it with an explicit `u64` (identical to `usize` on 64-bit, correct on 32-bit), never
`usize`. This also hardens the ARM/cross-platform target — the same fix serves wasm and ARM at once.

## A committed `.cargo/config.toml` must not require a tool the clone does not bring

`rustc-wrapper = "sccache"` in a **committed** `.cargo/config.toml` makes the repository **unbuildable by
anyone without sccache installed.** Cargo does **not** degrade gracefully — it dies:

```
error: could not execute process `sccache .../rustc -vV` (never executed)
Caused by: No such file or directory (os error 2)
```

**That one line failed every CI job on every OS** (badge lane, macOS, Windows, and all three static
targets) — and **never once locally**, because sccache was installed on the dev machine. *That asymmetry is
the whole shape of the bug.*

**Make build accelerators opt-in via the environment**, enabled only when the tool is actually on `PATH`
(`scripts/mem-guard.sh` does this). Same caching, no hard dependency.

> **The general rule, and this project had already written it down: a committed artifact must be usable by
> anyone who clones this repo.**

## Cargo features are a UNION — one unpinned declaration re-enables what four others disabled

`hyper-rustls` and `rustls` were both declared `default-features = false, features = ["ring", …]` to keep
the crypto backend **pure Rust**. But `engine/net` also declared **`tokio-rustls = "0.26"` with default
features**, and tokio-rustls's defaults enable **`rustls/aws-lc-rs`** — a C/assembly backend requiring
**NASM + CMake**, which fails the **Windows** link with `link.exe: exit code 1104`.

**Cargo unions features across every dependent.** Disabling a feature in four places does nothing if the
fifth declaration enables it. Pinning `tokio-rustls` to `ring` removed `aws-lc-sys` from the tree entirely.

> **Check with `cargo tree -f "{p} :: {f}"`, not by reading Cargo.toml** — the manifest says what you
> *asked for*; the tree says what you *got*.

## `Instant::now()` panics on wasm — a debug line can take down the engine

`std::time::Instant::now()` is **unsupported on `wasm32-unknown-unknown`** (`std::sys::pal::wasm::
unsupported::time`). A **debug-only** `tracing::debug!` timing the Stylo rule-index build **panicked the
entire cascade** in the browser — and surfaced as `RuntimeError: unreachable` from inside the wasm module,
*a diagnosis pointing nowhere near a debug line.*

> **A measurement must never be able to break the thing it measures.** Gate clock reads behind
> `#[cfg(not(target_arch = "wasm32"))]`.

## wasm has no filesystem, so `load_system_fonts()` finds nothing

The page laid out **correctly** (2,526px of content) and rendered **blank**. *A font problem never looks
like a font problem.* Fonts must be **compiled into the binary** (`include_bytes!`) — and use the
**Liberation** faces specifically, because those are what Chrome's `Arial`/`Times New Roman` requests
resolve to on Linux, so text measures like the native engine rather than like a lookalike.

## The release cadence — and the way it silently did nothing

The trigger is **not a calendar**. A version number should mean *something changed for the user*, so a
release fires when the **capability ledger** (`docs/loop/WEB-PATTERNS.md`) moves **and** the tick declares
itself `pattern-class`, `capability`, or `bar-0`. Infrastructure, instrument and doc ticks are skipped —
that is the point, not a gap. The tag is derived, never typed: `v0.<TICK>.0`, read from `STATUS.md`,
because the tick *is* the unit of progress here.

**It was broken for several ticks, and reported success the whole time.** The gate grepped the *commit
message* for `TICK SHAPE:`; the pre-commit hook enforces that trailer in the *journal*. Two sources of
truth for one claim — so the gate read the one nobody has to write, found nothing, and skipped the build
while printing a green check. Tick 62 (a genuine Bar 0 capability win) shipped no binary.

> **A check whose source of truth differs from the enforcement mechanism's is a check that silently never
> fires.** And the green tick is what makes it dangerous: it certifies that the thing is working.

The gate now reads the journal, with the same `awk "/^## Tick N/,0"` construct `tick.sh` uses — `,0` (to
EOF) and *not* `,/^## Tick [0-9]/`, because **awk tests a range's END pattern against the START line as
well**, so the obvious version collapses the range to a single line and finds nothing, for every tick.
The extraction is proven against the real journal before it is trusted.
