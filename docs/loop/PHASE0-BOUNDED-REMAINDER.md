# PHASE-0 BOUNDED REMAINDER — the finite finish line

_Derived 2026-07-21 (tick ~328) from a three-way deep-research pass: external SOTA (use counters,
HTTP Archive, Interop, Servo/Ladybird methodology), a full internal audit (git history, gates on
disk, constellation vs reality), and a site-class × capability matrix checked against source.
This SUPERSEDES the stale constellation priority rows and the ready_pct metric (which reads 103%
and gates nothing). The remainder below is finite, sized, and has a named cut line._

**Ground rule (re-confirmed by audit): the constellation runs stale-PESSIMISTIC — re-probe before
building any row marked missing.** Rows corrected during this audit alone: CSP, select actuation,
input Selection, track/captions, popover, hscroll carousels, pointer sequence, :focus cascade,
position:sticky, media playback join (in-crate) — all already BUILT and gated on disk.

## The shape of what remains

After tick 324, the remainder is **not a long tail**. It is:
- **3 genuine subsystems** (multi-tick, decompose before starting),
- **~20 bounded S/M items**,
- **3 verification runs** (measurement, not capability),
- and a **named cut line** (below — exceptions, not hand-waves).

## Tier 1 — JARRING (a site class is broken without it) — do these first

| # | item | classes hit | size |
|---|---|---|---|
| 1 | **Media playback join**: decode thread fed by SourceBuffer → in-page `play()` resolves; `isTypeSupported` steering; cpal audio out; basic ABR. (M1-M6 exist in-crate; this is the join to the page.) | video, social in-feed, news | L (7-12) |
| 2 | **Codec breadth**: H.264 beyond constrained-baseline, AV1 via re_rav1d / rav1d-safe (fully-safe SIMD fork exists, Apr 2026). VP9 stays formally deprioritized — no usable Rust decoder, and MP4/H.264 is 68%+ of crawlable web video. AV1 path also unlocks AVIF. | video, social, news | L (5-10) |
| 3 | **contenteditable + document Selection + basic editing commands** (Gmail compose / Notion class). Selection groundwork is parked on wip/tick328. | messaging, docs, forums | L (7-15) |
| 4 | **IME / composition events** (CJK typing is impossible today) | messaging, docs | M (3-6) |
| 5 | **IndexedDB indexes** (createIndex/IDBKeyRange — Firebase/Cognito SDKs hard-fail without them) | messaging, social, banking | S-M (2-4) |
| 6 | **WebAuthn/passkeys** (passkey-only sites are hard walls; TOTP fallback covers the rest) | banking, dev | M (3-6) |
| 7 | **Password vault + autofill** (crypto core done in store/lib.rs; the iceberg is UX) | all login | M (3-6) |
| 8 | **Cookie attribute enforcement PROBE** (SameSite/Secure/HttpOnly — currently UNKNOWN; wrong = silent login loops) | banking, commerce | S (1-2) |
| 9 | **bidi full line reordering** (RTL locales read broken pages today) | doc web | M (3-4) |

## Tier 2 — MODERATE (degraded but usable without it)

| # | item | size |
|---|---|---|
| 10 | Live CSS transitions/animations (real timeline; end-state-only today) | M-L (4-8) |
| 11 | WebGL software backend + honest strings (vector maps; raster fallback exists — Google Maps degrades, doesn't die) | L (7-15) |
| 12 | Form widget completeness: date/color/time pickers, native select popup paint, accent-color | M (3-5) |
| 13 | Container queries (Stylo-side — live cascade is cascade_via_stylo; never grow MinimalCascade) | M (3-6) |
| 14 | Visual-effects bundle: filter, backdrop-filter, mix-blend-mode, clip-path, background-attachment | M (4-6) |
| 15 | Fullscreen API | S (1-2) |
| 16 | MP3 decode (symphonia feature flag) | S (1) |
| 17 | Clipboard image paste (binary ClipboardItem) | S (1-2) |
| 18 | Drag editor-half (dragstart/setData/effectAllowed) | S-M (2-3) |
| 19 | MathML (Wikipedia/arXiv/MDN formulas) | M (3-6) |
| 20 | multicol | M (3-4) |
| 21 | Print output (@media print applies; render it to a printable surface) | S-M (2-3) |
| 22 | SVG namespace/foreignObject completeness | S-M (2-4) |
| 23 | ch/ex real font metrics (sub-pixel drift compounds; Parley now borrowable) | S-M (2-3) |
| 24 | PDF hand-off polish (open-with-OS-viewer; in-browser viewer below cut — though pdf.js may run on the landed canvas surface as a free post-Phase-0 experiment) | S (1-2) |

## Tier 3 — EXIT VERIFICATION (measurement ticks; the ORACLE GATE)

| # | item | size |
|---|---|---|
| 25a | Run **test262** (SpiderMonkey embedded; number unknown at near-zero cost; Ladybird publishes 97.8%) | S |
| 25b | Run the **100-tab RSS benchmark** (defined, never run; the memory thesis rests on zero data) | S |
| 25c | Large-DOM interactivity probe (8.8k-node paint measured linear; interaction unmeasured) | S |
| 25d | **Fidelity instrument rebuild + full-corpus sweep** per FIDELITY-SCORING-REDESIGN.md (selector-path keying, parent-relative shape, root-cause clustering, jarring invariants) — THE exit gate | M (3-5) + sweep |

## THE CUT LINE — named exceptions (out of Phase 0, with reasons)

1. **EME/Widevine** (Netflix/Spotify DRM) — licensed proprietary CDM; cannot be built, only licensed.
2. **WebRTC** (Meet/Zoom voice/video) — a second media-stack-sized subsystem; async messaging works without it.
3. **WebGPU + heavy-WebGL creative apps** (Figma/Canva/TradingView-tier) — GPU subsystem, not ticks.
4. **Canvas-office EDITING** (Docs/Sheets write-path) — viewing is reachable now; editing = IME + rich clipboard + collab cursors, an office-suite of quirks.
5. **Web Audio API** (AudioContext) — games/DAWs; element audio covers Phase-0 listening.
6. **Push + Web Notifications** — progressive enhancement.
7. **True worker parallelism / SharedWorker / OffscreenCanvas** — perf phase (workers run, same-thread).
8. **HTTP/3/QUIC + WebTransport** — V1-SCOPE.md skip; feature-detects cleanly.
9. **In-browser PDF viewer** — OS hand-off ships Phase 0.
10. **Devtools** — Phase 1+.
11. **Process-per-tab** — decided architecture, Phase-1 security work.
12. **Modern-CSS niche tail** — subgrid, @scope, anchor positioning, scroll-driven animations, custom highlights, text-wrap:balance, JPEG XL, WebCodecs: absent-by-construction, feature-detect cleanly, cosmetic.
13. **Vertical writing modes** — niche; doc web unaffected.
14. **Opus decode** — steer to AAC via isTypeSupported (plan of record).
15. **VP9** — no usable Rust decoder; AV1+H.264 covers the practical web (external data confirms).
16. **Bot-detection evasion** — constitutionally out (completeness ≠ evasion; completeness mostly landed).

## The budget

- **Jarring-only core** (Tier 1 + Tier 3): **≈ 45-65 ticks** — "no site class is broken."
- **Full bounded remainder**: **≈ 90-135 ticks**; with the historical 1.5-2× L-subsystem overrun,
  **realistic bound 100-150 ticks ≈ one week of loop time** at demonstrated cadence (20-30/day).
- Phase-0 EXIT = the certificate in FIDELITY-SCORING-REDESIGN.md §5 (Bar 0 + jarring invariants
  ≥95% + shape ≥0.75 on ≥95% + interactivity ≥95% + named exceptions only), measured by the
  rebuilt instrument on the stratified corpus — **not** by ready_pct, not by WPT count.

## Marquee strategy (from the Ladybird lesson)

Perception moves on **one threshold number + one marquee app**, not breadth claims. Ours:
- Threshold: the Phase-0 exit certificate headline (shape fidelity on the top-1000).
- Marquee: **YouTube plays** (Tier-1 items 1+2) — the single most legible "it's a real browser" proof.
