# PLATFORM.md — cross-platform build/verification status

Living record for the standing cross-platform gate (IMPLEMENTATION.md **S-XP** /
**P0.1** / **P0.5**). Updated as targets are verified. **This session runs on
Ubuntu**; macOS/Windows are engineered-for-portability and exercised in GitHub CI,
**not** on local hardware — anything not built here is flagged, never assumed.

Legend: ✅ verified here (Linux) · 🟡 CI-configured, unverified locally · 🔧
engineered/documented-portable, local build blocked by a sandbox toolchain gap.

## Default workspace build + tests (feature-off; the shipping path)

| Target | build --workspace | build --no-default-features | test --workspace | Notes |
|---|---|---|---|---|
| `x86_64-unknown-linux-gnu` | ✅ | ✅ | ✅ 40 passed | primary dev target |
| `x86_64-unknown-linux-musl` (static) | 🔧 | 🔧 | — | correct target+config; headless (no wgpu). Local build blocked only by missing `musl-gcc` (no sudo); CI installs `musl-tools`. |
| `x86_64-pc-windows-msvc` (static-CRT) | 🟡 | 🟡 | 🟡 | `+crt-static` in `.cargo/config.toml`; all deps portable (no OpenSSL — rustls; no POSIX-only APIs). Runs on GitHub `windows-latest`. |
| `aarch64-apple-darwin` (framework) | 🟡 | 🟡 | 🟡 | standard framework linking. Runs on GitHub `macos-latest`. |

All engine crates are portable by construction: `hyper`, `rustls` (ring, no
OpenSSL), `wgpu` (Vulkan/Metal/DX12), `winit`, `taffy`, `tiny-skia`, `html5ever`,
`fontdb`/`fontdue`, `tokio`. No Linux-desktop-only API is used. **No `io_uring`/
`tokio-uring`** anywhere (would be `[L!]` Linux-only, feature-gated, and is kept out
of the GUI codebase per CLAUDE.md Portability).

## Heavy feature builds (P0.5)

`--features spidermonkey` (mozjs) and `--features stylo` are **off by default**, so
the default/headless build and the musl static binary above do **not** depend on
them.

| Target | mozjs (`spidermonkey`) | stylo | Verified |
|---|---|---|---|
| linux-gnu x86_64/aarch64 | **prebuilt archive** (servo/mozjs releases) | pure-Rust | ✅ Linux x86_64 (mozjs 9.5s, stylo 22s) |
| macOS x86_64/aarch64 | **prebuilt archive** available | pure-Rust | 🟡 CI (Linux-only heavy job in CI today; see below) |
| windows-msvc x86_64/aarch64 | **prebuilt archive** available | pure-Rust | 🟡 documented; from-source would need LLVM/`LIBCLANG_PATH`+MozillaBuild |
| **linux-musl** | **NO prebuilt → from-source** (clang+python+autoconf, ~30 min) | pure-Rust | 🔧 friction: `--features spidermonkey` on musl is from-source only. Niche — default musl binary needs no mozjs. |

**Friction items flagged (not silently assumed):**
1. **musl + `--features spidermonkey`** has no prebuilt → from-source Moz toolchain.
   Tracked; not on the default shipping path. If ever needed, it becomes its own
   plan item (bake a musl mozjs archive in CI).
2. **Windows/macOS feature builds** rely on prebuilt archives resolving in CI; the
   CI `features-linux` job verifies Linux only today. Extending the feature-build job
   to macOS/Windows is a cheap follow-up once the prebuilt-resolution is confirmed
   green on those runners (kept off the hot path to keep CI fast).

## What "verified" means here

- ✅ = built and/or tested on this Ubuntu box this session.
- 🟡 = the CI workflow (`.github/workflows/ci.yml`) runs it on a GitHub hosted
  runner; **await the first green run** before treating macOS/Windows as verified.
- 🔧 = the choice is documented-portable and would build with the standard toolchain;
  the only blocker is a missing tool in this sandbox (e.g. `musl-gcc`), which CI
  provides.
