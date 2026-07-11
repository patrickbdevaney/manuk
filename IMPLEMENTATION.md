# IMPLEMENTATION.md — Manuk ordered execution plan

Synthesis of the committed **Directions of improvement** dispositions in `CLAUDE.md`
(RESEARCH.md Axes A–G, pass-1 + pass-2). **Planning only** — nothing here is
implemented yet; each item awaits explicit authorization before execution. This
plan does not re-open settled dispositions; it sequences and operationalizes them.

## How to read this

Each item: **Change** (files/crates + the existing seam it hooks) · **Depends on**
(other item IDs; D-axis compat is sequenced before E-axis UI that assumes it, per
CLAUDE.md priority) · **Platform** (see legend) · **Acceptance** (a WPT
subdir+pass-count target, a size/memory delta, or a demonstrated behavior).

Items disposed *deferred / insufficient-evidence / monitor-upstream* **begin with the
follow-up research or validation gate**, not a build step (flagged ⏳). A deferral is
never silently promoted to a build task.

### Platform legend (addendum — cross-platform is a first-class dimension)

- **[XP]** identical on Linux/macOS/Windows *by construction* — the crate abstracts
  the OS (hyper, rustls, wgpu, winit, taffy, tiny-skia, html5ever, fontdb/fontdue,
  tokio). No per-OS code.
- **[PS]** platform-specific path exists but is abstracted by a reused crate that has
  a per-OS backend which **must be verified on each OS** (e.g. `keyring`,
  `accesskit`, SpiderMonkey/Stylo build toolchain).
- **[L!]** **Linux-only, flagged explicitly** per CLAUDE.md Portability — never
  silent. (Currently nothing in this plan is [L!]; `io_uring`/`tokio-uring` stays out
  of the GUI codebase by directive. Any future Linux-only fast path must be added
  here as [L!] and feature-gated.)

> **Honesty note (per user):** we are *not* running macOS/Windows test hardware this
> phase. Cross-platform is pursued by (a) engineering-by-construction ([XP] crates),
> and (b) **CI** on all three OSes (GitHub Actions `ubuntu`/`macos`/`windows`
> runners). macOS/Windows acceptance = *CI-verified*, not locally verified. This is
> stated so no [XP] claim is mistaken for a local test result.

### WPT pin (fetched fresh 2026-07-09, not carried from the research passes)

All WPT acceptance checks below are pinned to **web-platform-tests/wpt commit
`7f6164e469f932da11e4bc8b3f047d0d89b0baaf`** (master HEAD, dated **2026-07-09**).
The upstream WPT runner (item **P0.3**) checks out exactly this SHA. Per item, the
pinned subdir(s) are named. **Re-pin caveat:** WPT moves daily; if execution begins
materially later than this date, P0.3's first step re-fetches a fresh SHA and this
file is updated in the same change — a stale pin makes the acceptance check
meaningless, so the pin is treated as live, not frozen.

### Standing acceptance check (applies across the whole plan, not per item)

**S-XP — three-platform build + working-binary gate.** Every phase boundary must
re-satisfy:
1. `cargo build --workspace` **and** `cargo build --workspace --no-default-features`
   succeed on **Linux, macOS, Windows** CI runners.
2. `cargo test --workspace` passes on all three.
3. The release profile's platform static-linking targets each **produce a binary
   that runs** (not merely compiles): Linux `x86_64-unknown-linux-musl` (fully
   static), Windows `x86_64-pc-windows-msvc` (static-CRT), macOS
   `x86_64/aarch64-apple-darwin` (standard framework linking). "Runs" = `manuk
   render https://example.com -o out.png` produces a valid PNG in CI.
4. Feature builds `--features spidermonkey` and `--features stylo` build on all three
   (see **P0.5** for the per-platform SpiderMonkey/Stylo reality check).

This gate is **sequenced early (P0.1)** and re-run at each phase boundary, so no item
is built/verified against Linux alone and found broken elsewhere later.

---

## Phase 0 — Foundations (must precede feature work)

These unblock everything and establish the cross-platform + measurement discipline
the later acceptance checks depend on.

#### P0.1 — Cross-platform CI + static-linking working-binary gate  ⏳ (verify-first)
- **Change:** add `.github/workflows/ci.yml` (or equivalent) running `cargo build
  --workspace` + `cargo test --workspace` on `ubuntu-latest`, `macos-latest`,
  `windows-latest`; add release jobs cross-building the three static targets and
  running `manuk render` on the produced binary. No engine code — CI + verifying the
  existing `.cargo/config.toml` (musl/static-CRT flags) actually work.
- **Depends on:** nothing. **First item in the plan.**
- **Platform:** [PS] — this item *is* the per-OS verification. Current state
  (honest): Linux-gnu native builds+tests pass; Linux-musl, Windows-msvc-static-CRT,
  and **all** macOS are **unverified**. Expect first-run failures (musl needs the
  target + possibly `mozjs` from-source; macOS needs its own `mozjs` prebuilt).
- **Acceptance:** S-XP items 1–3 green on all three OS runners. Any target that
  cannot produce a working binary becomes its **own tracked sub-item here**, fixed
  before Phase 1 (so later work isn't Linux-only-verified then broken).

#### P0.5 — SpiderMonkey + Stylo per-platform build-path reality check  ⏳ (verify-first)
- **Change:** confirm, on each OS CI runner, that `--features spidermonkey` (mozjs)
  and `--features stylo` build **for real**, not assumed. Document per-OS friction:
  `mozjs` links **prebuilt archives** from `servo/mozjs` releases for common targets
  (linux-gnu/macOS/Windows x86_64 + aarch64) — verify each resolves; **musl has no
  prebuilt → from-source**, which needs the full Moz toolchain (clang, python,
  autoconf) and is the ~30-min build; Windows from-source needs LLVM/`LIBCLANG_PATH`
  + MozillaBuild (prebuilt avoids this). Stylo is pure-Rust (cross-platform by
  construction) but unverified on macOS/Windows.
- **Depends on:** P0.1.
- **Platform:** [PS] — the whole point. Flag musl-SpiderMonkey (from-source, heavy)
  and Windows-from-source-toolchain as named friction items if CI hits them.
- **Acceptance:** a per-OS matrix table (in this file, updated) stating for each of
  {linux-gnu, linux-musl, macOS, windows-msvc} whether mozjs = prebuilt|from-source|
  fails, and stylo = ok|fails. No feature work that assumes JS/Stylo proceeds until
  the intended default (feature-off) is green everywhere and the feature path is
  documented per OS.

#### P0.2 — Measurement harness (§8)
- **Change:** a `bench/` crate/CI: binary size (per target, incl. trimmed builds),
  per-tab baseline RSS, click-to-navigate latency, frame time. Reuses `cargo bloat`
  / `cargo tree --duplicates` / `cargo-udeps` (C3 standing tooling). Emits
  before/after numbers per the working agreement.
- **Depends on:** P0.1.
- **Platform:** [XP] (size/latency measurable per-OS in CI; RSS attribution detail is
  cross-platform but see G-e for the shared-process wrinkle).
- **Acceptance:** CI publishes the four metrics on every PR; a baseline snapshot
  committed. Prereq for C1, C2, §3, G-e (all need measured deltas).

#### P0.3 — Upstream WPT runner (§5)
- **Change:** extend `tests/wpt` (`manuk-wpt`): `find_wpt_checkout()` → clone/checkout
  WPT at the **pinned SHA `7f6164e…` (2026-07-09)**; a wptreport-style runner that
  executes reftests/wdspec against the engine and folds results into the existing
  `Report`. Grows the built-in reftests meanwhile.
- **Depends on:** P0.1.
- **Platform:** [XP] (harness is pure Rust; runs per-OS in CI).
- **Acceptance:** `cargo run -p manuk-wpt -- --wpt $WPT_DIR` runs ≥1 real WPT subdir
  at the pinned SHA and reports pass/fail/skip counts. **This item makes every
  WPT-target acceptance below executable** — those items report their subdir's
  current-vs-target pass count against `7f6164e…`.

#### P0.4 — Net redesign: pooled streaming client (B1 + B2 + B3 foundation)
- **Change:** `engine/net` — replace the per-request `conn::http1::handshake` with
  **`hyper_util::client::legacy::Client` + `hyper-rustls::HttpsConnector`**
  (`HttpsConnectorBuilder`, ALPN `h2,http/1.1`). Expose the response body as a chunk
  stream via `http_body_util::BodyExt::frame().await` (not `collect()`). Add
  `async-compression` (feature-gated gzip/deflate/br) decoding the chunk stream;
  send `Accept-Encoding: gzip, deflate, br`. Seam: the existing `manuk_net::fetch` /
  `request` API keeps its shape; add a streaming variant returning a chunk stream.
- **Depends on:** P0.1. **Foundation for** B-latency, D4, F1-headers.
- **Platform:** [XP] — hyper/rustls/hyper-rustls/hyper-util/async-compression are all
  pure-Rust, no OpenSSL, identical on all three OSes.
- **Acceptance:** (a) `manuk render https://example.com` still 200 on all three CI
  OSes; (b) a **gzip/br-encoded** real page decodes correctly (behavior); (c)
  connection reuse demonstrated — two sequential same-origin fetches perform **one**
  TLS handshake (observe via a counter/log); (d) h2 negotiated when the origin
  offers it (ALPN result logged). No WPT target.
- **B3-h3 (deferred, not built now):** HTTP/3 via `quinn` + `h3`/`h3-quinn` is
  **deferred** behind Alt-Svc/HTTPS-RR discovery + a UDP path + h2 fallback
  (marginal browsing win over pooled h2; `h3-quinn` pre-1.0). Revisit after D4 makes
  subresource multiplexing matter. When built: [XP] (`quinn` pure-Rust UDP/QUIC,
  cross-platform).

#### P0.6 — Correctness infrastructure (§5)
- **Change:** beyond the WPT runner (P0.3): (a) secondary **real-site screenshot
  diffing** vs a real Chrome render (visual-fidelity check, *not* the primary
  signal); (b) **property/fuzz testing** of the parsers (`manuk-html` via html5ever
  is fuzz-hardened upstream; fuzz our CSS value/selector parser and layout) — **never
  JIT/GC fuzzing** (that stays vendored; crossing the SpiderMonkey boundary is
  forbidden). Reuse `cargo-fuzz`/`arbitrary`.
- **Depends on:** P0.3.
- **Platform:** [XP].
- **Acceptance:** a fuzz job runs in CI without crossing the JS boundary; a
  screenshot-diff harness compares one real page vs Chrome and reports pixel delta.
  No WPT target (this *is* correctness infra).

---

## Phase 1 — Compatibility breadth (Axis D) + incremental-layout seeding (A2)

The long pole. Sequenced **before** Axis-E UI polish that assumes real page
rendering. D1/D2 are independent and can proceed in parallel; D3 gates on the event
loop; D4 gates on P0.4.

#### D4 — Loading: external resources, decoding, charset
- **Change:** `engine/net` + `engine/page`. **Charset:** reuse `encoding_rs` (decode)
  + `chardetng` (fallback detect); hand-roll the WHATWG sniff orchestration
  (BOM → `Content-Type` → 1024-byte `<meta>` prescan → detector → default). **Decode:**
  reuse `async-compression` (from P0.4, `Content-Encoding` before charset).
  **Resource scheduling:** hand-roll a WHATWG-ordered fetch scheduler in
  `engine/page` for `<link rel=stylesheet>` / `<script src>` / `<img>` (render-
  blocking CSS; `defer`/`async` scripts). **`data:` URLs** now (RFC 2397, reuse
  base64). **`blob:` deferred** → D3 (needs a Blob store in the JS/DOM layer).
- **Depends on:** P0.4 (streaming + pooled client), P0.3 (WPT acceptance).
- **Platform:** [XP] — encoding_rs/chardetng/async-compression pure-Rust.
- **Acceptance:** WPT `encoding/` and `fetch/` @ **`7f6164e…`** — record current
  pass count, target = encoding_rs/spec coverage for `encoding/`; behavior: a
  Shift-JIS/`<meta charset>` page renders correct text; an external stylesheet
  applies before first paint.

#### D1 — Layout: floats + margin-collapse, positioning, tables
- **Change:** `engine/layout` — hand-roll (from-scratch mandate; taffy stays flex/
  grid-only). Sequence by entanglement: **(1) floats + margin-collapse** as one
  BFC-aware pass (`FloatContext`/`PlacementAmongFloats`-style, threaded
  `Option<&mut FloatContext>`); **(2) positioning** via a containing-block chain
  (abs/rel/fixed, then sticky); **(3) tables** fixed→auto, **documenting the chosen
  interpretation** where CSS2 §17 is ambiguous (working-agreement requirement).
  Reference: Servo `layout_2020` float/table code.
- **Depends on:** P0.3. Independent of D2/D3.
- **Platform:** [XP] — pure layout math.
- **Acceptance (per sub-feature, all @ `7f6164e…`):** floats → `css/CSS2/floats/` +
  `css/CSS2/floats-clear/`; margin-collapse → `css/CSS2/margin-collapse/`;
  positioning → `css/CSS2/abspos/` + `css/CSS2/positioning/` + `css/CSS2/zindex/` +
  `css/css-position/` (sticky); tables → `css/css-tables/` + `css/CSS2/tables/`.
  Each: current pass count 0 → target = pass the subdir's applicable reftests
  (exact counts produced by P0.3 at the pin).

#### D2 — Stylo real-cascade activation
- **Change:** `engine/css` — replace the `StyloEngine` delegate (behind the existing
  `StyleEngine` trait + `stylo` feature) with a real Stylo cascade. Implement
  Stylo's DOM "wall" (`TDocument`/`TElement`/`TNode`/`TRestyleDamage` +
  `selectors::Element`) over a `(&Dom, NodeId)` handle; provide a `Device`
  (viewport/media); build a `Stylist`; map `ComputedValues` → `ComputedStyle`.
  **Impedance (resolved):** attach an **`AtomicRefCell<ElementData>` per element
  node** (field on the arena element `Node` or a `NodeId`-indexed side-table); mirror
  `blitz-dom`'s pattern (`atomic_refcell` + Stylo). Reference: Blitz `blitz-dom` +
  `stylo_taffy`.
- **Depends on:** P0.3, P0.5 (Stylo builds per-OS). Independent of D1/D3.
- **Platform:** [PS] — Stylo is pure-Rust (cross-platform by construction) but its
  build is heavy and unverified on macOS/Windows → gated on P0.5.
- **Acceptance:** WPT @ **`7f6164e…`**: `css/selectors/`, `css/css-cascade/`,
  `css/mediaqueries/`, `css/css-variables/`, `css/css-values/` (calc). Current 0 →
  target = Stylo's Firefox-grade coverage (counts from P0.3). Behavior: a page using
  `@media`, custom properties, and `calc()` renders correctly under `--features
  stylo`.

#### D3 — Web API binding surface (event loop + jQuery-core DOM subset)
- **⏳ Step 0 (validation gate, not a build step):** prototype **ONE** interface
  (`Element.textContent`) two ways — (B) hand-written mozjs binding storing `NodeId`
  in a reflector reserved slot vs (A) Servo `script_bindings` codegen — and measure
  glue cost + arena-DOM fit. Commit the DOM-object-model decision from the prototype.
  The pass-2 lean is (B); **validate, do not assume.**
- **Change (after Step 0):** new `engine/js::bindings` real surface. **Event loop:**
  reuse mozjs `RustJobQueue` (promise microtasks) + hand-roll a tokio-backed host
  task queue (timers/events; `setTimeout` is host-provided) → HTML macrotask→drain-
  microtasks loop. **Bindings:** hand-write the **jQuery-core subset** (jQuery ≈74%
  of pages): `querySelector`/`getElementById`, `createElement`/`appendChild`/
  `textContent`/`innerHTML`, `addEventListener`, `XHR`/`fetch`, `classList`/`style`
  — `NodeId` in a reserved slot, `JS::Heap` side-table for the few JS-refs (event
  listeners/expandos). Prioritize further APIs by chromestatus UseCounter + HTTP
  Archive BigQuery. Unblocks `blob:` (D4), DOM mutation (A2 activation),
  `script.evaluate` (E4 BiDi).
- **Depends on:** P0.5 (SpiderMonkey per-OS), D3-Step-0. Gates: A2 activation, D4
  `blob:`, E4 script/input, E6.
- **Platform:** [PS] — mozjs build per-OS (P0.5); binding logic itself [XP].
- **Acceptance:** Step 0 → a decision record + a working single-interface prototype.
  Full: WPT `dom/` + `html/dom/` @ **`7f6164e…`** (current 0 → target = the jQuery-
  core-covered subset's tests); behavior: a real page's inline `<script>` doing
  `document.querySelector(...).textContent = ...` mutates the rendered page.

#### A2 — Incremental relayout/repaint data structures (seed now, activate on D3)
- **Change:** seed the three structures behind the compositor's existing `Damage`:
  **(1)** `RuleFeatureSet` in `manuk-css` (class/id/attr → descendant/sibling
  `InvalidationSet`s, whole-subtree fallback); **(2)** matched-properties cache +
  sibling style-sharing in the cascade (sharing-breakers: id/inline/container-units/
  handlers); **(3)** a `LayoutDamage` enum + cached fragments on `LayoutBox` (box-
  tree structural rebuild → box rebuild → fragment-layout-only → repaint). Reuse
  taffy `mark_dirty`+cache for flex/grid subtrees. Carry dirty/damage bits in the
  fragment tree **now**; activation is a fill-in once mutation exists.
- **Depends on:** structures seedable after D1/D2 land the fragment/style shapes;
  **activation blocked on D3** (dynamic mutation / scroll-hover restyle / resize).
- **Platform:** [XP].
- **Acceptance:** correctness reuses the **same** layout reftests (D1/D2 subdirs @
  `7f6164e…`) run incrementally vs full — identical output. Perf: a class-change on
  a large page relayouts only the invalidated subtree (measured via P0.2: relayout
  time ≪ full).

---

## Phase 2 — Performance & memory (Axes A/B/C)

#### B-latency — First-paint checkpoint + speculative preconnect
- **Change:** `engine/page` + `shell`. **First-paint checkpoint (hand-roll):** feed
  net chunks (P0.4) to `parser.process()`; once `<head>` + its render-blocking CSS
  are processed, do the first layout+paint of the DOM-so-far at `<body>` start
  (FOUC guard), then incremental relayout/repaint per chunk (A2). **Preconnect
  (hand-roll, deferred until a shell hover signal):** on link hover/pointerdown warm
  a pooled socket (~10s idle cap, bounded concurrency), **user-initiated + same-
  origin-biased**; `dns-prefetch` for maybe-origins. **Privacy constraint:** no
  unsolicited cross-origin preconnect.
- **Depends on:** P0.4, A2 (incremental repaint), D4 (preload scanner overlap).
- **Platform:** [XP].
- **Acceptance:** time-to-first-paint on a slow-streaming page < time-to-full-load
  (measured, P0.2); preconnect warms a socket on hover (behavior). No WPT target.

#### A3 — Shaped-run cache (now) + decoded-image cache (after D4)
- **Change:** **(1)** shaped-run cache in `manuk-text` extending the `FontKey` cache
  — key `(FontKey, quantized size, text, script/dir/features)`, word-level + run-
  level fallback, `lru` + byte budget. **Buildable now. (2)** decoded-image cache
  keyed `(resource-id/URL, decode size, format)` → RGBA, `lru` + byte budget —
  **after D4** (image loading). Both **per-tab accounted**, dropped on C1 discard.
- **Depends on:** shaped-run: none (now). decoded-image: D4. Both cross-ref C1.
- **Platform:** [XP] (`lru` pure-Rust).
- **Acceptance:** repeated layout of the same text hits the cache (counter) with a
  measured relayout speedup (P0.2); image cache respects its byte budget (evicts
  under pressure). No WPT target.

#### A1 — Vello GPU/CPU paint tier  ⏳ (monitor-upstream, no build step)
- **⏳ This is a deferral, not a build task.** Disposition = **defer**; `Painter`
  trait already isolates the swap. Follow-up = **monitor** `linebender/vello`: adopt
  `vello_cpu` behind `Painter` the moment its **API stabilizes (≥0.1)** (features are
  already production-ready; API instability is the sole blocker as of 2026-Q1);
  `vello_hybrid` (GPU) after API stability + glyph fixes. Keep `tiny-skia` meanwhile.
- **Depends on:** upstream release (external). No in-repo dependency.
- **Platform:** [XP] — wgpu (Vulkan/Metal/DX12) + Vello are cross-platform;
  verify on all three when adopted.
- **Acceptance (when triggered):** a `VelloCpuPainter`/`VelloGpuPainter` behind the
  existing `Painter` trait passes the same render behavior as `tiny-skia` (visual
  reftest / screenshot diff), on all three OSes; frame-time delta measured (P0.2).
  No WPT target (rendering quality → visual reftests).

#### C1 — Realize hibernation (freeze / discard)
- **Change:** `engine/compositor` — hand-roll the two actions on the existing tiers:
  **freeze** (background-CPU tier: throttle timers/JS task queue ~1/min, keep DOM+
  layout, shrinking GC; audio/WS/RTC exempt) and **discard** (hibernated tier: drop
  fragment tree + rasterized tiles + the tab's SpiderMonkey realm, retain URL/DOM-
  source, relayout/re-fetch on wake — the real RAM reclaim). Reuse SpiderMonkey
  compartment/realm-per-tab over the shared process-global runtime. (`CLAUDE.md` §1
  "share startup snapshots" reframed already — SpiderMonkey has no V8 snapshot.)
- **Depends on:** P0.2 (measure the delta), D3 (timers/JS to throttle exist).
- **Platform:** [PS] — tile eviction-to-disk uses a temp path (cross-platform via
  `std`); SpiderMonkey realm ops [XP] within mozjs.
- **Acceptance:** a backgrounded tab's resident memory drops from active to a small
  residual on **discard** (measured, P0.2 — target the Chrome-shaped ~80–300MB→
  ~5–10MB order); frozen tab's timers fire ≤1/min (behavior).

#### C2 — SpiderMonkey build-size reduction  ⏳ (measure-first, engineering-gated)
- **⏳ Step 0 (engineering/measurement gate, not a silent trim):** research is
  sufficient; the residual is a **one-time build + measurement**. Do:
  `MOZJS_FROM_SOURCE=1` + patch `mozjs_sys`'s configure invocation (keep `Intl`,
  ship **en-only ICU data** via ICU 64+'s data-filter; `--enable-optimize
  --disable-debug`; **never** `--without-intl-api`); bake a reusable archive via
  `MOZJS_CREATE_ARCHIVE`, link thereafter via `MOZJS_ARCHIVE`. **Measure the binary-
  size delta (P0.2)** before adopting.
- **Change (after Step 0, if delta justifies):** commit the trimmed-archive build
  config; per-OS archive baking (see friction below). Strictly build/config — never
  JIT/GC/sandbox.
- **Depends on:** P0.2, P0.5.
- **Platform:** [PS] — **real friction:** the from-source trim must be produced per
  target. macOS/Windows/linux-gnu have `servo/mozjs` prebuilts *of the default
  config* but our **trimmed** archive must be self-baked per-OS; **musl** is already
  from-source. Windows self-bake needs the LLVM/MozillaBuild toolchain. Track each
  OS's trimmed-archive bake as a sub-item.
- **Acceptance:** measured binary-size reduction (target: the ICU-data delta,
  ~single-digit MB) with `Intl.*` still functional (behavior: `Intl.NumberFormat`
  works); no correctness regression in the JS test suite. No WPT target.

#### §7-proc — Isolate/compartment-per-tab process model + site isolation
- **Change:** `engine/compositor` + `shell` — move toward the CLAUDE.md target:
  isolate/compartment-per-tab (SpiderMonkey compartments — already the model) **multi-
  plexed over a small number of OS processes** (not one-process-per-tab), with **site
  isolation** for untrusted content. Builds on C1's realm-per-tab and the shared
  process-global runtime. Keep untrusted JS inside the vendored SpiderMonkey sandbox;
  TLS stays `rustls`. (This is a large architecture item — stage after C1.)
- **Depends on:** C1 (realm-per-tab), D3 (untrusted JS to isolate).
- **Platform:** [PS] — OS process spawning + IPC differ per OS; abstract behind a
  small process/IPC shim (or reuse a cross-platform IPC crate); verify per-OS.
- **Acceptance:** N tabs run across M≪N OS processes (behavior); a crashing tab does
  not take down others (behavior); per-tab baseline RSS measured (P0.2). No WPT
  target.

#### C3 — Dependency-graph hygiene (standing tooling)
- **Change:** adopt `cargo tree --duplicates` + `cargo bloat` + `cargo-udeps` in CI
  (part of P0.2). Optional low-priority: drop winit's `sctk-adwaita` default feature
  if we forgo Wayland CSD (removes the transitive `tiny-skia 0.11.4`) — a measured UX
  tradeoff, **Linux-only concern** (Wayland decorations).
- **Depends on:** P0.2.
- **Platform:** [XP] tooling; the `sctk-adwaita` trim is a Linux-only UX tradeoff
  (note, not [L!] code — it's a dependency feature).
- **Acceptance:** CI reports the dep graph; no *new* duplicate major versions merge
  without a note. No structural change required (gating already correct).

---

## Phase 3 — Headful user-facing features (Axis E) — after Axis D

Sequenced after D so UI is built against real page rendering, not a stub.

#### E1 — Chrome UI essentials
- **Change:** `shell` — hand-roll, reusing existing engine primitives. Tabs/windows/
  history (per-tab nav stack)/bookmarks/settings/URL-suggestions = shell state
  (winit + a small local store). **Find-in-page** reuses the fragment tree
  (`TextFragment` runs + `visible_text` → match → overlay-highlight rects, overlay
  avoids relayout). **Full-page zoom** = relayout at a zoom factor (reuse
  `layout_document`, crisp). **Pinch-zoom** = compositor surface transform. No new
  engine surface.
- **Depends on:** D1/D2 (real pages to find/zoom over).
- **Platform:** [XP] — winit windowing/input identical across OSes.
- **Acceptance:** find-in-page highlights all matches over the fragment tree
  (behavior); Ctrl+/- reflows crisply; back/forward traverses history. No WPT target.

#### E5 — Native content-blocking (not an extension runtime)
- **Change:** `engine/net` — reuse Brave's **`adblock`** crate (EasyList/EasyPrivacy/
  uBO-syntax network blocking) at the request layer; cosmetic-filter hooks into
  DOM/paint. **Extensions-as-a-runtime stays OUT OF SCOPE** (scope trap + attack
  surface). This is a §2/§7 privacy feature, not "extensions."
- **Depends on:** P0.4 (net request layer).
- **Platform:** [XP] — `adblock` pure-Rust.
- **Acceptance:** a known tracker URL from a standard filter list is blocked
  (behavior); page load issues fewer third-party requests with a list loaded. No WPT
  target.

#### E2 — Local encrypted password store + autofill (audited-crate-only)
- **Change:** new `shell`/store module. **Audited crates only, zero hand-rolled
  crypto:** `keyring` (v4 — OS secret store), a RustCrypto AEAD (`chacha20poly1305`
  or `aes-gcm`, both NCC-audited) for at-rest, `argon2` (Argon2id) for the primary-
  password-derived key when the OS store is unavailable (**never** Chromium's weak
  hardcoded fallback), `psl` for eTLD+1. Candidate on-disk format: KDBX4. **Origin
  matching:** `signon_realm` = scheme+host+port exact-match default; PSL/eTLD+1 for
  HTML forms surfaced as a *related-domain* suggestion; **no scheme-downgrade**;
  cross-origin iframe requires the field's own origin; affiliation deferred.
- **Depends on:** D3 (form fields / origin from the DOM). Hard rule: **if any
  disposition would require a hand-rolled crypto primitive, stop and flag** — it does
  not here.
- **Platform:** [PS] — `keyring` has DPAPI (Windows) / Keychain (macOS) / Secret
  Service (Linux) backends; **each backend must be CI-verified**. The no-keyring
  Argon2id fallback is the [XP] path.
- **Acceptance:** store+retrieve a credential on each OS (CI, per-backend); fill only
  on exact origin; **no** cross-scheme/cross-origin-iframe fill (security behavior
  tests). No WPT target.

#### E7 — Profiles / containers + SOCKS proxy
- **⏳ Storage-layer prerequisite:** profiles/containers are a **storage-partition**
  design (profile ⊃ container cookie-jar ⊃ per-site jar / Total Cookie Protection),
  **blocked on a storage layer we do not have** (no cookies/cache/history/
  localStorage). So this item **begins by building the storage layer** (cookie jar +
  keyed stores) — a real prerequisite, not silent deferral.
- **Change:** (1) storage layer in `engine/net`/`engine/page` (cookie jar, cache,
  history, localStorage) partitioned by a profile→container→site key. (2) **VPN =
  reuse `tokio-socks`** (route through a user-provided SOCKS5/HTTP proxy; WireGuard
  via external `wireproxy`). **Bundling a WireGuard/OpenVPN client is out of scope.**
- **Depends on:** the storage layer (built here) precedes container partitioning.
- **Platform:** [XP] — `tokio-socks` + storage are pure-Rust.
- **Acceptance:** two containers keep separate cookie jars for the same site
  (behavior); traffic routes through a configured SOCKS proxy. No WPT target.

#### E3 — Translate-page
- **Change:** `shell` panel. **Lead:** reuse **E6's `InferenceBackend`** (BYO-
  endpoint, zero model runtime). **Local private tier:** reuse Bergamot/Marian models
  on-demand (~20 MiB/pair downloaded, not bundled) via `bergamot-translator` FFI or
  ONNX (`ort`) — local-runtime pick deferred behind E6. DOM extraction/reinjection
  reuses the block/inline distinction (layout already separates them).
- **Depends on:** E6 (`InferenceBackend` panel wiring).
- **Platform:** [XP] for BYO-endpoint; [PS] for the local ONNX/FFI runtime (verify
  per-OS when built).
- **Acceptance:** a foreign-language page's visible text is translated in place,
  structure preserved (behavior). No WPT target.

---

## Phase 4 — Agent capability, security retrofit, blue-ocean (Axis §4 / E6 / E4 / G)

#### §4a — Agent observation upgrade + G-c accessibility tree (one investment, two payoffs)
- **Change:** build **one** accessibility/semantic tree (DOM + ARIA/implicit roles +
  layout geometry): **reuse `accesskit`** for the screen-reader platform bridge;
  hand-roll the DOM→tree mapping (WAI-ARIA + HTML-AAM). Feeds **(1)** human screen-
  reader a11y and **(2)** the agent's structured observation channel (`manuk-agent`
  `Observation` — role/label/bbox, better than raw text+screenshot; reduces reliance
  on the injection-prone screenshot). Adds **viewport-clipped text** + **element
  bounding boxes** so the agent can click by coordinate, not only link index.
- **Depends on:** D3 (DOM roles), D1 (layout geometry). Sequenced with D3.
- **Platform:** [PS] — `accesskit` = UIA (Windows) / NSAccessibility (macOS) / AT-SPI
  (Linux); **each backend CI-verified**. The agent-observation consumer is [XP].
- **Acceptance:** WPT `wai-aria/` @ **`7f6164e…`** (role computation subset; current
  0 → target = the implemented roles); a screen reader announces a rendered page
  (per-OS, CI where possible); the agent clicks a button by role+bbox.

#### §4b — Agent actions + reliability
- **Change:** `manuk-agent` — typing, form submit, back/forward, wait-for-load,
  scroll-to-element, tab management (reuse the `AgentBrowser` + `run_task` seam);
  retry/rate-limit handling; screenshot/response caching; token budgeting;
  deterministic replay (→ G-d).
- **Depends on:** §4a (element refs for typing/click), D3 (form submit).
- **Platform:** [XP].
- **Acceptance:** the agent fills+submits a form and reads the result page
  (behavior). No WPT target.

#### E6 — In-browser AI panel + prompt-injection security retrofit (hardens the agent too)
- **Change:** `shell` side-panel reusing `InferenceBackend` (same core as the agent —
  the thesis). **Security (layered, all reuse of established patterns, no novel
  crypto/ML):** **(1)** CaMeL/dual-LLM structural separation — retrofit `run_task` so
  the `Observation` (page text + screenshot) is **untrusted, provenance-tagged data
  kept out of the instruction/planner channel** (fixes the Comet-class flaw we
  currently share); **(2)** per-site permissions; **(3)** OWASP Action-Guard human-
  in-the-loop by irreversibility heuristics (financial/admin/externally-visible/
  data-deletion/cross-origin credential); **(4)** an injection classifier via the
  existing backend. **Applies to both the panel and the existing agent path.**
- **Depends on:** D3 (page DOM for the panel), §4b (gated actions to guard).
- **Platform:** [XP].
- **Acceptance:** a page with a hidden injected instruction (white-on-white / in the
  screenshot) does **not** cause a cross-origin/sensitive action (red-team behavior
  test); sensitive actions require explicit confirmation. No WPT target.

#### E4 — Developer tools / WebDriver BiDi remote end
- **Change:** a BiDi **remote end** (JSON-RPC over WebSocket) reusing
  `tokio-tungstenite` (already in the stack) + a JSON dispatcher over the spec CDDL.
  **Minimal subset now:** `session` + `browsingContext` (navigate/captureScreenshot/
  getTree/create/close + load events) + `network` events + `log` — wraps existing
  `AgentBrowser`/`engine/net` ops → **makes the agent Puppeteer-drivable.** Deferred
  sub-commands: `script.evaluate`/`callFunction` (after D3), `input.performActions`
  (after §4a hit-testing/element refs). Bespoke inspector UI only for humans.
- **Depends on:** the minimal subset needs only P0.4 + `AgentBrowser`; script/input
  gated on D3/§4a.
- **Platform:** [XP] — `tokio-tungstenite` pure-Rust.
- **Acceptance:** WPT `webdriver/tests/bidi/` @ **`7f6164e…`** (session/browsing
  Context/network/log subset; current 0 → target = the implemented modules);
  behavior: **Puppeteer 23+ connects and navigates** a page.

#### G-a — Unified human+agent live session
- **Change:** low-marginal-cost feature of the shared `engine/page` core — let the
  human hand the *same live session* (cookies/scroll/DOM) to the agent and take it
  back (both front-ends on one `Page`). Not novel (market caught up); the
  architectural cleanliness is the differentiator.
- **Depends on:** E7 storage layer (session state), §4b (agent actions), E6 (consent
  to hand off).
- **Platform:** [XP].
- **Acceptance:** human browses to a logged-in page, hands off, agent continues in
  the same session, human resumes (behavior). No WPT target.

#### G-b — Local-first semantic history
- **Change:** reuse `fastembed` (small quantized on-device model, offline) +
  `sqlite-vec` (embedded HNSW) or **`LEANN`-style recompute** (store pruned graph,
  recompute embeddings at query — ~97% storage savings, natural since we have the
  encoder + re-parse every page). Index the clean text `engine/page` already
  produces; volatility-scored re-embed (per-URL change rate) + content-addressing;
  **encrypt the index at rest reusing the E2 AEAD**. Queried by the E6 panel.
- **Depends on:** E2 (AEAD), E6 (query surface), P0.2 (§3 storage budget).
- **Platform:** [PS] — `fastembed` uses ONNX Runtime (per-OS binaries; verify);
  index/store [XP].
- **Acceptance:** semantic query over previously-read pages returns the right page;
  index storage stays within the §3 budget (measured, LEANN recompute path). No WPT
  target.

#### G-d — Deterministic replay / provenance
- **Change:** extend `manuk-agent`'s `AgentOutcome` transcript into an **append-only
  event log** recording observations + actions + **model/tool responses** (models
  aren't reproducible → record their outputs). Uniquely exact because the agent is
  display-free with a deterministic CPU raster (stable screenshot hashes where GPU
  agents diverge). Strict-mode divergence check = reproducibility proof; doubles as a
  regression harness (§5).
- **Depends on:** §4b (actions to log).
- **Platform:** [XP] — CPU raster determinism is cross-platform (same `tiny-skia`
  output).
- **Acceptance:** a recorded agent run replays byte-identically (strict mode green)
  on the same platform; screenshot hashes stable. No WPT target.

#### G-e — Instant per-tab resource honesty
- **Change:** `shell` task-manager UI + `engine/compositor` per-tab accounting.
  Reuse SpiderMonkey **per-compartment memory reporters** for per-tab JS heap; hand-
  roll compositor per-tab layout/paint/tile accounting. The **shared-process self-
  attribution** (per-tab CPU needs self-timing; ~30–45% JS heap is "heap-
  unclassified") is the wrinkle, shared with C1/§8.
- **Depends on:** P0.2, C1 (tier model), D3 (per-compartment JS to attribute).
- **Platform:** [PS] — per-tab CPU timing differs per OS (getrusage/thread times);
  abstract behind a small shim; memory reporters [XP] via mozjs.
- **Acceptance:** the task manager shows plausible per-tab CPU/RAM that moves with
  load (behavior), attribution error bounded (documented, given heap-unclassified).
  No WPT target.

#### §4c — Backend breadth
- **Change:** add local backends (llama.cpp/Ollama) as `InferenceBackend` impls
  without touching `run_task` (the trait already isolates this).
- **Depends on:** none beyond the existing trait.
- **Platform:** [PS] — local inference runtimes are per-OS (verify); the trait seam
  is [XP].
- **Acceptance:** the agent completes a task via a local backend with no `run_task`
  change. No WPT target.

---

## Phase 5 — Honest presentation (Axis F)

Net-header parts can land early with P0.4; the JS-env part gates on D3.

#### F1 — Honest presentation (never fingerprint mimicry)
- **Change:** `engine/net` — a **truthful** `User-Agent` identifying Manuk + real
  engine/version; the universal `Mozilla/5.0 (…) … Manuk/<ver>` general-token form is
  **flagged for human sign-off** (not competitor impersonation, but a judgment call).
  A complete, consistently-ordered default header set (Accept / Accept-Language /
  Accept-Encoding). Honest rustls TLS (already correct). Spec-complete JS/DOM env
  (D3) so probes pass because we *actually implement* the APIs. **OUT OF SCOPE:**
  JA3/JA4 or Akamai-h2 fingerprint mimicry, Chrome/Safari UA spoofing, header-order
  copying, JS-quirk faking. **Never reshape our fingerprint to match a mainstream
  browser.**
- **Depends on:** P0.4 (headers); the JS-env clause on D3. **Human sign-off** on the
  Mozilla-token UA before shipping.
- **Platform:** [XP] — headers/TLS identical across OSes.
- **Acceptance:** sends a truthful, correctly-ordered request set (behavior/inspect);
  **no** fingerprint-mimicry code exists (review gate). No WPT target.

#### F2 — Graceful-degradation interstitial (UX honesty, not evasion)
- **Change:** `shell` — detect a hard-wall **honestly** via the documented
  `cf-mitigated: challenge` header (and repeated 403/429 + challenge interstitial);
  show a calm "this site blocks non-mainstream browsers; Manuk won't impersonate
  another browser" page with honest options (retry / copy URL / open in another
  browser). **Never solve or bypass the challenge.**
- **Depends on:** P0.4 (response headers), E1 (shell chrome for the interstitial).
- **Platform:** [XP].
- **Acceptance:** a challenge response triggers the interstitial, not a bypass
  attempt (behavior); no challenge-solving code exists (review gate). No WPT target.

---

## Dependency summary (critical path)

```
P0.1 cross-platform gate ─┬─ P0.5 SM/Stylo per-OS ─┬─ D2 (Stylo)
                          ├─ P0.2 measurement ──────┼─ C1, C2, G-e, §3
                          ├─ P0.3 WPT runner ───────┼─ every WPT acceptance
                          └─ P0.4 net redesign ─────┴─ D4 ─ B-latency
                                                       D1 ─┐
                                       D3 (⏳prototype) ───┼─ A2 activate, D4 blob,
                                                           │   §4a, E2, E4 script,E6
                                       §4a a11y (accesskit)┘
D (compat) ───────────────► E (headful UI) ───► §4/E6/E4/G (agent+blue-ocean) ─► F
```

- **Axis D before Axis E** (UI built on real rendering) — per CLAUDE.md priority.
- **P0.1/P0.5 cross-platform checks are first**, re-run at every phase boundary (S-XP),
  so nothing is Linux-only-verified then found broken on macOS/Windows.
- Deferrals (⏳): **A1** (monitor upstream Vello API stability), **D3 Step 0**
  (validation prototype before the subset), **C2 Step 0** (measure the trim before
  adopting), **E7** (build the storage layer prerequisite first) — none are silently
  promoted to build tasks.


---

## Phase 6 — Addendum (from the 2026-07-10 RESEARCH pass: Axis H + Axis D amendment)

Derived **only** from that pass's dispositions. Ordered for execution. Nothing here
re-opens or re-executes prior scope. WPT commit pinned at
**`7cbfe0be66b3b2c228cf5da70ed3cbeb30b214b3` (2026-07-10)** for every WPT acceptance below.

Sequencing rationale: **N1 → N2** because a shared session-history model must exist before
the History API can bind to one (and it retires two duplicated stacks). **N3** is
independent (parser-only) and can run at any point. **N4** depends on N3 (needs a shadow
node kind). **N5–N6** are agent-surface items with no engine dependency. **N7–N8** are
research-first plan entries, not build tasks.

#### N1 — Unified session-history model (prerequisite for N2)
- **Change:** hoist one `SessionHistory` type into `engine/page` (or a small shared crate)
  with the semantics both existing stacks already implement independently: push truncates
  forward entries; re-pushing the current URL is a no-op; traversal past either end is an
  error, not a silent stall. Retire the duplicates in `manuk-agent::AgentBrowser`
  (`history`/`hist_pos`) and `shell::chrome::History`, which are the *same* model written
  twice. Add the spec's `index`/`length` accessors and a `traverse(delta)` entry point so
  `browsingContext.traverseHistory` and `history.go(n)` share one implementation.
- **Seam:** `engine/page` (owned by both front-ends); consumed by `manuk-agent`,
  `shell::chrome`, and `manuk-bidi`'s `browsingContext.traverseHistory`.
- **Depends on:** nothing new. Supersedes nothing; it is a refactor with a behavior
  contract already pinned by existing tests in both crates.
- **Platform:** [XP] — pure logic. *Verified on Linux.*
- **Acceptance:** the existing `manuk-agent` history tests and `shell::chrome` history
  tests both pass **unchanged** against the hoisted type (proving the two stacks really
  were one model); `traverse(-1)`/`traverse(+1)` drive `browsingContext.traverseHistory`
  with the BiDi tests still green. Behavior, no WPT target.

#### N2 — History API bindings (`pushState`/`replaceState`/`popstate`/`hashchange`)
- **Change:** `engine/js::dom_bindings` — bind `history.pushState(data, unused, url)`,
  `history.replaceState(...)`, `history.length`, `history.state`, `history.go/back/forward`
  over N1's model. Enforce the spec's rules exactly: both **update the document URL without
  fetching or loading**; **neither fires `popstate`** (only traversal does); the target URL
  must be **same-origin** or the call throws `SecurityError`. Fire `popstate` on traversal
  and `hashchange` on fragment-only changes, through the existing `event_loop` task queue.
- **Seam:** the D3 binding-helper layer (`reserved-slot` reflectors + `JS_DefineFunction`),
  feature-gated behind `spidermonkey`. **The model (N1) is not feature-gated**; only the
  bindings are — the shell and agent use the same history without any JS.
- **Depends on:** N1; D3's binding layer (landed). Uses only sanctioned JSAPI — **does not
  touch JIT/GC/sandbox**.
- **Platform:** [XP] logic; the JS binding is *engineered-for-portability,
  unverified-elsewhere* (SpiderMonkey builds only verified on Linux here).
- **Acceptance:** **WPT `html/browsers/history/the-history-interface/` @ `7cbfe0be`**
  — current **0**, target: the `pushState`/`replaceState` URL-update, no-fetch,
  no-popstate, and same-origin-rejection subset. Plus a behavior check: a page calling
  `history.pushState({}, '', '/a')` changes `location.pathname` **without a network
  request** (assert the fetch count is unchanged) and `back()` then fires exactly one
  `popstate`.

#### N3 — Declarative Shadow DOM: own `TreeSink` over the arena DOM
- **Change:** `engine/html` — replace the `RcDom` → arena copy with a `TreeSink`
  implemented **directly on `manuk_dom::Dom`**, and override the two hooks html5ever
  already calls: `allow_declarative_shadow_roots()` and **`attach_declarative_shadow()`
  (which defaults to `false`, which is why `<template shadowrootmode>` is silently dropped
  today)**. Add a shadow-root node kind + `<slot>` assignment to `manuk-dom`, and a
  **flat-tree** traversal (host → shadow root → slotted light-DOM children) that layout and
  the a11y tree walk instead of the raw node tree. Reuses html5ever's tokenizer + tree
  builder unchanged — the DSD parsing rules are already implemented upstream.
- **Seam:** `engine/html` (`parse`/`parse_bytes`/`StreamParser`), `engine/dom` (node kind +
  flat tree), consumed unchanged by layout / a11y / find-in-page.
- **Depends on:** nothing. Independent of D3 — **DSD requires no JavaScript**.
- **Platform:** [XP] — pure-Rust parser + arena. *Verified on Linux.*
- **Acceptance:** **WPT `shadow-dom/` @ `7cbfe0be`** — current **0**, target: the
  declarative-shadow-root + `<slot>` assignment subset. Plus behavior: parsing
  `<div><template shadowrootmode="open"><slot></slot></template><p>light</p></div>`
  yields a shadow root whose slot is filled by the `<p>` in the **flat tree**, while the
  `<p>` remains a light-DOM child of the host in the node tree. Secondary, non-negotiable
  check: the `RcDom` removal must not regress any existing `manuk-html` test.

#### N4 — Shadow-DOM style scoping (`::slotted`, flat-tree selector matching)
- **Change:** `engine/css` — make real the four `selectors::Element` methods currently
  stubbed in `stylo_dom.rs` (`parent_node_is_shadow_root`, `containing_shadow_host`,
  `is_html_slot_element`, `assigned_slot`), and match selectors over N3's flat tree so
  styles do not leak across the shadow boundary. Add `::slotted()`. Follow Servo, which has
  shadow DOM + `<slot>` layout + `::slotted` already.
- **Seam:** `engine/css::stylo_dom` (the `selectors::Element` wall) and `MinimalCascade`'s
  matcher, so both style paths agree.
- **Depends on:** **N3** (a shadow node kind must exist first). Does not depend on D3.
- **Platform:** [XP]. *Verified on Linux.*
- **Acceptance:** a rule inside a shadow root does **not** match a light-DOM element of the
  same name, and a light-DOM rule does not match inside the shadow root (encapsulation, both
  directions); `::slotted(p)` matches a slotted `<p>`. WPT `shadow-dom/` @ `7cbfe0be`
  pass count **increases** over N3's baseline — record both numbers.

#### N5 — `Capabilities`: per-invocation permission scoping (H4)
- **Change:** `manuk-agent` — a `Capabilities` value (allowed action kinds + an origin
  allowlist + the existing `allow_sensitive_actions` bit), passed into `run_task` and
  consulted by the **existing `assess_action` guard**, which already sits at CaMeL's
  enforcement point (a check before each tool call). The E6 panel constructs a narrower
  `Capabilities` than the headless binary. **No second action schema**, and **no bearer
  token**: macaroon-style attenuation buys nothing in-process and is deferred to N8.
- **Seam:** `AgentConfig` / `assess_action` / `run_task` — all landed; this is an added
  field and a filter, not a new subsystem.
- **Depends on:** nothing new (§4b's action set + E6's guard, both landed).
- **Platform:** [XP]. *Verified on Linux.*
- **Acceptance:** behavior — with a `Capabilities` permitting only `{navigate, scroll,
  finish}`, a model emitting `{"action":"submit"}` is **refused with a reason** and the run
  continues; with an origin allowlist of `example.com`, a `navigate` to `evil.org` is
  refused **even though `classify_target` alone would call it Safe** (proving the allowlist
  is a distinct, composing constraint, not a duplicate of the heuristic). No WPT target.

#### N6 — `ObservationPolicy`: budget-keyed serialization depth (H2)
- **Change:** `manuk-agent` — one `ObservationPolicy { include_screenshot, include_axtree,
  include_links, include_text, max_axtree_lines, text_budget, token_budget }` over the
  **one** existing engine data structure (`Observation` + `A11yNode`). Serialization depth
  is a harness parameter, **keyed on a token budget, not on a model-capability enum** — the
  evidence is that more context degrades *every* model, and that nothing shows richer
  observations help larger ones. Lean defaults; viewport clipping already in place.
- **Seam:** `Observation::to_prompt` (landed) becomes `to_prompt(&ObservationPolicy)`;
  `AgentConfig` holds the policy. The E6 panel and the headless binary pass different
  policies; there is **one** implementation.
- **Depends on:** nothing new (§4a/§4b landed).
- **Platform:** [XP]. *Verified on Linux.*
- **Acceptance:** behavior — a lean policy and a rich policy over the *same* `Observation`
  produce prompts differing only in included sections, the lean one is strictly shorter,
  and **both keep the E6 untrusted-content fence intact** (a policy must never be able to
  drop the fence). Assert the rendered prompt respects `token_budget` (approximate
  chars/token). No WPT target.

#### N7 — ✅ CLOSED (2026-07-10): research pass done; the blocker is lifted
- **Outcome:** the three questions are answered (full disposition in RESEARCH.MD § PASS-2).
  (a) **A newer `mozjs` does not fix it.** `Runtime::create` calls `InitSelfHostedCode`
  unconditionally, and SpiderMonkey needs `js::UseInternalJobQueues` *before* that; mozjs
  offers no hook in between, so the call always arrives too late. Reproduced by probe
  (SIGSEGV inside the call) and confirmed against `mozjs` `main`. An API-shape problem, not
  a version bug. (b) **A custom `JobQueue` is implementable from safe Rust today** —
  `mozjs::glue::{JobQueueTraps, CreateJobQueue}` + `SetJobQueue` resolve and type-check
  (compile-verified). This is `mozjs_sys` embedding glue and the public `JS::JobQueue`
  interface — the same hook browsers use — so it is **inside CLAUDE.md's boundary**: no
  JIT/GC/sandbox is touched and SpiderMonkey needs no patch. (c) All module hooks are
  exposed (`SetModuleResolveHook`, `SetModuleDynamicImportHook`, `FinishDynamicModuleImport`
  in raw jsapi; `CompileModule`/`ModuleLink`/`ModuleEvaluate` in the safe wrappers).
- **No implementation was performed under this entry**, as it specified. The work it
  unblocks is **N9**.

#### N9 — Custom `JobQueue` → module scripts → dynamic `import()` (unblocked by N7)
- **Change, in three steps, in this order:**
  1. **Custom `JobQueue`.** Implement `JobQueueTraps` whose `enqueuePromiseJob` pushes onto
     the microtask queue `engine/js::event_loop` already owns and whose `runJobs` is our
     existing drain; install it with `CreateJobQueue` + `SetJobQueue`. This **also retires
     the standing D3 gap** that native `Promise.prototype.then` reactions bypass our event
     loop — a strictly larger win than dynamic `import()` alone, and the reason it is first.
  2. **`<script type="module">`.** `CompileModule` → `ModuleLink` → `ModuleEvaluate`, with a
     `SetModuleResolveHook` that resolves specifiers against the page URL through the
     existing fetch layer. `ModuleEvaluate`'s top-level-await `MutableHandleValue` out-param
     is a Promise, so it depends on step 1.
  3. **Dynamic `import()`.** `SetModuleDynamicImportHook` + `FinishDynamicModuleImport`.
     Depends on step 1 (it resolves a Promise) and step 2 (it needs the module loader).
- **Seam:** `engine/js::event_loop` (queues), `engine/js::modules` (new), `engine/net` (fetch
  for the resolve hook). Feature-gated behind `_sm`, like every other SpiderMonkey binding.
- **Depends on:** N7 (done). **Boundary:** sanctioned embedding API only — `JS::JobQueue`,
  `SetJobQueue`, the module hooks. **If any step would require patching SpiderMonkey's
  JIT/GC/sandbox, stop and return to the human.**
- **Platform:** [XP] logic; the SpiderMonkey bindings are
  *engineered-for-portability-unverified-elsewhere* (SpiderMonkey builds verified on Linux
  only in this environment).
- **Acceptance:**
  1. `Promise.resolve(1).then(...)` runs its reaction when our event loop drains — through
     the **native** promise machinery, not the `queueMicrotask` shim. This is the assertion
     the current D3 note says we cannot make.
  2. `<script type="module">import {x} from "./m.js"; globalThis.ok = x;</script>` evaluates
     with the resolve hook serving `./m.js`, and `ok` is set.
  3. `import("./m.js").then(m => globalThis.ok = m.x)` resolves after a loop drain.
  4. WPT `html/semantics/scripting-1/the-script-element/module/` @ `7cbfe0be` recorded as a
     target. **State the measured pass count honestly** — it is `0` until the P0.3 upstream
     runner is integrated, and no number may be claimed before then.

#### N8 — ⏳ Imperative Custom Elements + attenuated capabilities — RESEARCH/DEFERRED
- **Follow-up research required:** (a) `customElements.define()` / `Element.attachShadow()`
  need D3's binding layer **plus** the custom-element reaction/upgrade queue — scope that
  queue against our `event_loop` before committing; (b) **H2's deferred sub-question**:
  whether observation depth should be auto-tuned per model — needs an eval harness, since
  the literature only evaluates fixed configurations; (c) **H4's deferred sub-question**:
  macaroon-style attenuation, justified **only** once agent authority crosses a process
  boundary (out-of-process agent, or agent→sub-agent delegation).
- **Depends on:** D3 (for (a)); N5/N6 landing first (for (b)/(c)).
- **Platform:** n/a (research).
- **Acceptance of the research pass:** a disposition per sub-item. **No code.**

**Explicitly out of scope for this addendum** (recorded so it is not silently promoted):
XPath locators; scoped custom-element registries; `input.setFiles` / file upload;
Navigation API (`navigation.navigate`); Set-of-Marks screenshot annotation.

## Open human decisions (surface before executing the relevant item)

1. **F1 UA string** — ✅ **DECIDED (2026-07): `Mozilla/5.0 (…) … Manuk/<ver>`** (the
   universal compat token; honest, not competitor impersonation). Apply when F1 lands.
2. **D3 Step-0 outcome** — ✅ **DECIDED (2026-07): hand-write the prioritized subset
   on a thin safe binding-helper layer** (prototype validated the arena-DOM
   reserved-slot approach; raw jsapi per-interface is segfault-prone). Servo codegen
   is the fallback if the subset balloons. See CLAUDE.md D3 "Step-0 outcome".
3. **C2** — still open: ship the trimmed SpiderMonkey archive only if the measured
   size delta justifies the per-OS bake friction (bring the number first).

---

## Known-limitations ledger (honest closure state, 2026-07-10)

The rendering/interactivity foundation was driven to Chrome parity measurement-first
via the layout-parity harness (`manuk-wpt parity`, 70/70 probes across 29 pages). This
ledger records what is **closed**, what is **inherent** (cannot be closed without
changing the premise), and what is **bounded-but-deferred** — so nothing reads as
"done" that isn't. No item here is silently claimed as passing.

### Closed and verified vs Chrome (±3px box geometry)
Block flow; box model (`box-sizing`, borders, margins incl. negative + auto-centering);
flexbox (direction/wrap/gap/grow/shrink/basis/justify/align-items/align-self/nesting);
CSS grid (`grid-template-*` px/fr/%/auto + `repeat()`, gap); sizing (`min/max-*`,
percentage width/height, recursive `calc()` with `+ - * / ()`); positioning (relative +
absolute incl. left+right / top+bottom stretch); inline-block; inline padding/border;
`white-space:nowrap`; `transform` (translate/scale/rotate/skew/matrix, origin-composed);
`vertical-align` (top/bottom exact); **tables** (fixed layout, colspan/rowspan occupancy,
border-collapse, default cell padding). Interactivity (links, form-control focus/caret,
text entry, checkbox/radio incl. name-group exclusivity, form submit, omnibox
search-vs-URL) verified through the real GUI binary via synthesized X11 input.

### Inherent — cannot be "closed", characterized precisely instead
- **Exact text width/height vs Chrome.** Manuk rasterizes with the *system* font stack;
  Chrome ships its own default faces. Identical text therefore measures a few px
  differently — this is a font-availability fact, **not a layout defect**. The harness
  isolates it by preferring explicitly-sized probes and measuring layout *consequences*
  (a following block's position) rather than raw glyph runs. Closing it would mean
  bundling and force-matching Chrome's exact metrics, which is neither desirable nor the
  goal. `vertical-align:baseline`/`middle` inherit this same font-baseline variance and
  are documented as such (top/bottom, which don't, are exact-tested).

### Bounded but unbounded-in-aggregate — narrowed, explicitly not complete
- **DOM/BOM/CSSOM surface.** This is the entire web platform (thousands of APIs) — it is
  *definitionally* not closeable by enumeration; a "complete" surface is Servo/Chromium
  scope. Implemented the high-frequency core: `getElementById`, `querySelector[All]`,
  `getElementsByTagName`/`ClassName`, `createElement`/`appendChild`/`remove`,
  get/set/remove/hasAttribute, `textContent`/`innerHTML`/`tagName`/`id`/`className`,
  `getBoundingClientRect`, `addEventListener`/`dispatchEvent`, `setTimeout`/event loop.
  **Not yet present** (representative, not exhaustive): live `HTMLCollection`s (current
  collections are static Arrays), a full CSSOM `element.style` object, `classList`
  `DOMTokenList`, `Event` constructor (dispatch takes a type string), `dataset`,
  DOM traversal props (`parentNode`/`children`/`nextSibling`), `getComputedStyle`.

### Bounded-and-deferred smaller items (recorded, not closed)
`history.state` serializes via JSON rather than StructuredClone; cookies are not carried
in the agent hand-off; no session response-body cache; per-tab JS heap size always
reports `None`; SOCKS4 / HTTP `CONNECT` proxying absent (SOCKS5/HTTP proxy present);
`aria-labelledby` and `translate` attributes unmodeled; hit-testing ignores occlusion
(z-order overlap); table auto column sizing does not redistribute a colspan cell's
intrinsic width, and border-collapse does not run per-edge conflict resolution.

---

## Phase 7 — RESEARCH v2 Directive (2026-07-11): responsiveness, fidelity, parity, agent spine

Derived from `RESEARCH_V2_DIR.MD` (8 axes). Staged single-agent per its process rule.
**Priority: Axis R first** (the felt problem), then P/U, then DT/AG, then MEM/SRC.

### Axis R — Responsiveness & navigation (HIGHEST). Diagnosis (confirmed 2026-07-11)
`shell/gui.rs::goto_no_history` runs the whole pipeline via `rt.block_on(fetch_html →
Page::load_async → fetch_and_apply_stylesheets)` **on the UI thread** — so during any of the
six nav triggers (typed+Enter, search-result click, in-page link, Back, Forward, Refresh)
winit processes no events and the OS shows "unresponsive". Back/forward has **no bfcache**
(re-runs the full pipeline). This is one architectural gap surfaced through six entry points.

- **R2 — bfcache (do first; most contained, biggest back/forward lever).** Keep the
  constructed `Page` (DOM + layout + images) in a bounded LRU keyed by URL. On Back/Forward,
  if the target is cached and **eligible**, swap it in instantly (no pipeline). Eligibility
  (from web.dev/bfcache + Blink `Document::CanBeCached`): skip if the main resource carried
  `Cache-Control: no-store`, or a load is in flight. Bound to ~4–6 pages; LRU-evict; this is
  the retained *tier* the compositor's tiering already models (MEM2). Seam: a
  `BfCache { LruCache<String, CachedPage> }` in the shell; cache the outgoing page on nav,
  restore on Back/Forward before falling through to a fresh load. **Verifiable** (unit-test
  the cache/eligibility; the swap is deterministic).
- **R1/R3 — off-thread navigation + input responsiveness.** Move `fetch→DOM→cascade→layout`
  off the UI thread onto a **dedicated page-worker thread** (JS-safe: it owns the single
  SpiderMonkey `Runtime`, honoring one-Runtime-per-process; only `Send` results cross the
  boundary — the arena DOM is `Send`). The UI thread: keeps rendering the prior page + a
  progress affordance, stays live to chrome input, can **cancel** a stale load (generation
  token), and swaps the finished `Page` in on a winit `UserEvent`/proxy wakeup. Servo's
  Constellation/Script-thread split is the reference (SRC1). Default (no-JS) build is fully
  `Send` → clean; spidermonkey build routes JS through the same worker. *Live-window
  verification* for the actual responsiveness; the threading/handoff logic compiles + is
  testable.
- **R4 — speculative preconnect.** On link hover + omnibox typing, preconnect (DNS+TLS warm)
  to the likely host/search endpoint. Privacy-safe: same-origin/opt-in only. Composes with
  the preload scanner + HTTP cache. Lower priority.

### Axis P — fidelity. **P0 finding (2026-07-11): NOT a fetch regression.**
A Wikipedia article render (`en.wikipedia.org/wiki/Browser_engine`) applies **2 external
stylesheets + 6 images** correctly; text/fonts/logo/tables/timeline render. The residual is
**P1 layout fidelity** — the Vector-2022 skin's sidebar flows linearly instead of as a
positioned column (likely `position:sticky`/`fixed` falling back to static + grid-area gaps).
Already improved vs the directive's screenshots by this session's Stylo-default + Taffy work.
P0 closes with no fetch work; P1 continues:
- **P1** — `position:sticky`/`fixed` layout (currently sticky→static), `grid-template-areas`,
  complex floats/table corners, overflow-scroll containers. Per-feature WPT targets.
- **P2** — UAX#14 line-breaking (`unicode-linebreak` crate) — parity-sensitive (shifts wrap).
- **P3** — inline SVG/MathML (preserve namespaces → new inline-SVG layout/paint path).
- **P4** — Custom Elements + Shadow DOM + `pushState`/`replaceState` + dynamic `import()`
  (framework interactivity). Confirm coverage.
- **P5** — replace hot-path `eval`-string DOM bindings with native JSAPI reflectors.

### Axis U — feature parity (leanly, over existing engine primitives)
U1 nav chrome + bfcache (R2); U2 bookmarks/downloads/history store (one schema, shares
`store`); U3 keyboard shortcuts — **needs a selection model over the fragment/text tree
(largest new surface)** + copy/paste + zoom; U4 tabs/windows/tab-groups + duplicate=clone
session; U5 autofill origin-matching (anti-phishing correctness); U6 **persistent encrypted
cookie jar** (what "stay logged in" needs — composes with `store`); U7 **Google OAuth popup
flow** (`window.open`+`postMessage`+`window.opener` — real multi-window JS plumbing; forcing
function for cross-context correctness); U8 zoom + translate (reuse local model); U9
downloads (stream-to-FS) + `<input type=file>` uploads.

### Axis DT — DevTools over **WebDriver-BiDi** (`bidi` exists; one surface, two consumers).
Minimal panels: DOM inspector (arena), console (SpiderMonkey), network (`engine/net`),
box-model (fragment tree). Reuses existing introspection.

### Axis AG — agent spine. AG1 BiDi as the external API (+ the in-process typed
`BrowserAction` already shipped); AG2 task-intent AXTree pruning over the `a11y` tree +
`diff`; AG3 expose both semantic (a11y) and visual (screenshot indexed-overlay) targeting;
AG4 audit the provenance Action-Guard fence; AG5 **measure** the in-process advantage vs a
CDP-over-socket baseline (the claim needs a number).

### Axis MEM. MEM1 SoA/DOD DOM — **defer, measure current reflow cache first** (large refactor,
likely premature). MEM2 realize hibernation (freeze-JS/evict-tiles) reconciled with R2's
bfcache tier (one memory model). MEM3 `cargo bloat` binary breakdown + ICU delta
(`spidermonkey-noicu`) + dedupe duplicate crate resolutions (build-config only).

### Axis SRC. Targeted **reference reads** (approach/edge-cases, never code): Blink
`Document::CanBeCached` (R2 eligibility), Servo constellation/script-thread boundary (R1),
float/table structure (P1), `window.opener` plumbing (U7). Do **not** take Chromium's process
model / quirk-compat / IPC — Manuk's in-process model is a deliberate divergence.

### Sequencing: R2 (bfcache) → R1/R3 (off-thread) → R4 → P1/P2 → U6/U7 → DT/AG → MEM3.
Each item: primary-source-backed disposition, seam-scoped, new WPT target where relevant,
parity gate stays green.
