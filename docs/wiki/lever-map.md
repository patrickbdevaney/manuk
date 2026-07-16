# Manuk Daily-Driver Usability Roadmap (synthesized, tick 138)

Synthesized from the 5-stream research (reference/research-streams-full.md). This is the ENFORCEABLE sequence;
the agent picks the lowest-numbered unmet target as the next tick. Falsifiable good-enough bars, NOT WPT %.
KEY CORRECTION: Taffy #204 ("Support CSS Grid") SHIPPED — the intrinsic-sizing blocker is MANUK'S OWN leaf
measure (fixable in a tick), not a Taffy gap.

## The enforceable sequence (RENDER+INTERACT, build top-down)
1. **Intrinsic sizing + wire calc()** [css-sizing/css-values, M] — bar: calc(100%-250px) sidebar-split ±1px vs Chrome; border-box p-4 border-2 around 100px child = 136px min-content; grid max-content col sizes to longest cell; column-flex inline size = max of items. Fix Manuk's leaf measure (min_content_cache/measure_cache, lib.rs:1959-2078); wire Taffy calc feature, stop collapsing Dim::Calc (taffy_tree.rs:31). Depends: none.
2. **SPA link-intercept** [interaction/history, S] — bar: React <Link> click => preventDefault cancels shell nav, pushState swaps view NO reload; push/replace fire NO popstate; back/fwd fire popstate w/ cloned state. Verify-and-harden the <a> case (dispatch fires before default action; history_host.rs asymmetry already right). Depends: none.
3. **IntersectionObserver driven-on-scroll (E2E)** [interaction, S] — bar: headless scroll => 2nd screenful appends + fetches lazy subresources; multi-val rootMargin. IO/RO are REAL+engine-driven (page/lib.rs:4147); ensure driven per scroll + fetch closes; fix rootMargin parse (dom_bindings.rs:7623). Depends: scroll geom (done).
4. **Block/inline edges** [css-text/css-sizing, M] — bar: parent<->child margin-collapse; 100vh hero exact; height:100% chain reaches flex shell; a<b>b</b> gains NO spurious space (lib.rs:28-31). These are the measure leaves Taffy consumes — fix before flex. Depends: 1.
5. **Flex common-case** [css-flexbox, M] — bar: 3x flex-1 cards w/ long token don't overflow (±1px, min-width:auto=content-min); justify-between edges exact; flex-wrap; gap counted once; abspos children off-line. css-flexbox 6->25%. Taffy impl is fine; fix Manuk<->Taffy boundary + map auto min-size to measured min-content. Depends: 1,4.
6. **Grid common-case** [css-grid, M] — bar: repeat(3,1fr) gap exact; 200px 1fr 1fr @800=200/300/300; auto-placement; grid-template-areas holy-grail. css-grid 5->20%. BLOCKER: grid_template_areas parsed (stylo_map.rs:589) but NO consumer in taffy_tree.rs — resolve areas to line numbers. Defer subgrid/masonry. Depends: 1,5.
7. **Overflow scroll-container** [css-overflow, M] — bar: overflow-y:scroll reserves gutter (text in ~185/200px); overflow:hidden = BFC contains floats; sticky header pins to panel. css-overflow 28->40%. Clipping works (lib.rs:451); add gutter width-consumption + BFC + sticky scrollport. Depends: 1,4.
8. **Positioned + real z-index stacking + sticky in getBoundingClientRect** [css-position/transforms, M] — bar: z-50 modal over z-10 header regardless of DOM order; fixed inside transform parent contained; sticky rect==pinned top. css-position 24->40%. Replace DOM-order paint (lib.rs:22) w/ stacking-context tree + z sort; reflect sticky_shift (lib.rs:308) into rects; position:sticky NOT impl yet (lib.rs:15-31). Depends: 1,4.
9. **Runtime CSSOM .sheet bridge** [cssom, M/L] — bar: insertRule + adopted/constructable sheets cascade (styled-components/Emotion/Lit restyle). Today CSSStyleSheet is a JS stub (event_loop.rs:1345); bridge sheet/cssRules/insertRule into the real Stylo cascade (stylo_engine.rs:208). Depends: 1.

## PARALLEL TRACK (layout-independent — pull forward on any crash / crash-gated tick)
- **P-A Stability** [M] — catch_unwind at the JS-native extern "C" boundary (unbuilt; dom_bindings.rs:508 only guards reflector edges) + contain the calc-size/interpolate-size mozjs SIGSEGV (release-only, tick 126, fresh context). A crash = lose ALL tabs — ranks above forms.
- **P-B Agentic floor** [M] — occlusion-aware hit_test (picks smallest containing box today, should pick topmost-painted over z); A11yNode enabled/disabled + bbox-stable Conditions; BiDi quiescence (network.responseCompleted + no layout churn); WIRE script.evaluate/callFunction (bidi/protocol.rs:481, returns unsupported) => node-id DOM dispatch (FB2).

## THEN media (BIND GStreamer, never write a codec; lead royalty-free WebM/VP9/Opus)
- **M0** [M] video/audio real replaced-element layout box + real HTMLMediaElement reflector (promote __manukMedia shim, dom_bindings.rs:1085; keep MediaError{4}+poster as permanent floor). bar: <video poster> sizes correct, poster paints, .play() rejects NotSupportedError w/o crash.
- **M1** [L] progressive src= playback — new engine/media crate, gstreamer-rs appsrc->decodebin->appsink, bytes from engine/net, frames->wgpu texture keyed by PTS as compositor video layer (Servo VideoSink). Lifecycle NOT tied to JS-reflector GC. bar: self-hosted VP9/Opus webm plays w/ synced audio, scrubs, fires events.
- **M2** [L] MSE MediaSource/SourceBuffer JS surface — per-SourceBuffer appsrc->parsebin. bar: create->createObjectURL->addSourceBuffer->append fMP4/webm init+media plays; isTypeSupported correct; updating/updateend serialized; **buffered TimeRanges HONEST through a seek** (highest-leverage bug); remove() eviction.
- **M3** [M] hls.js/dash.js UNMODIFIED bring-up (conformance, no new feature) — fix isTypeSupported codec strings, buffered precision, updateend timing, changeType, timestampOffset/appendWindow, multi-SourceBuffer, QuotaExceededError.
- **M4** [XL] ABR + YouTube-class (last, riskiest) — real ABR, fMP4/H.264/AAC lane (licensing first), dav1d for AV1, Clear Key EME only (Widevine = permanent gap). bar: 1080p VP9 DASH plays minutes w/ adaptation+seek+stable memory; YouTube reaches playing for CLEAR content or degrades to poster+MediaError.

## Forms (server-rendered actor tier)
12. **Form POST navigation + constraint validation (+Selection, contenteditable)** [forms, M] — bar: <form method=POST> real navigation (net POSTs via send_raw), POST **refused loudly never downgraded to GET**; required/type=email/:invalid/checkValidity/reportValidity block submit; Selection stub->real. SPA half done (submit+preventDefault+FormData). Ranked below read/scroll tasks. Depends: 2.

## Agentic fallbacks (the a11y tree decouples driving from rendering — v1 ships while CSS imperfect)
- FB1 layout imperfect -> target by role+name not pixel (default path, no work).
- FB2 box-less element -> DOM click/.focus()+value by node id via JS engine (needs script.evaluate, P-B) — most important gap.
- FB3 ambiguous -> disambiguate never guess (Grounded::Ambiguous on thin margin).
- FB4 occluded/z-stack -> semantic node-id dispatch + verify post-condition (retired by 8+P-B).
- FB5 page re-rendering -> wait on observable post-condition (BiDi network.responseCompleted), re-resolve.
- FB6 a11y insufficient (canvas/webgl) -> set-of-marks screenshot as SECONDARY channel (vision = permanent insurance, disambiguator over known nodes).
- FB7 visual state unreadable -> read state from DOM/ARIA (aria-disabled/selected/:checked correct even when paint isn't).
- FB8 native action too thin -> refuse explicitly (capabilities.rs) or route around (build GET URL, agent/forms.rs).
- FB9 scroll/viewport wrong -> scroll by semantic anchor (Action::ScrollTo{name}) not pixel.
Metric: track "% corpus actions completed WITHOUT falling back to vision" as the real agentic-progress signal.

## Deferred
Compositor-threaded scrolling (main-thread OK for v1; don't relayout per scroll tick); Widevine/DRM (permanent gap, Clear Key only); H.264/HEVC/AAC as LEAD lane (patent/GPL — lead WebM/VP9/Opus/AV1); MSE-in-Worker, EME beyond Clear Key; writing-mode/subgrid/masonry/named-lines; 3D transforms/preserve-3d (AABB approx OK); fragmentation (multicol/print); true per-threshold IO crossing/live regions/aria-owns; incremental paint during parse; rav1d (ship dav1d now).

## Terminal gate — reasonable security sweep
Codec licensing audit (GStreamer LGPL core vs GPL/patent gst-plugins-bad/-ugly/FFmpeg), capability-scoping/anti-prompt-injection in capabilities.rs, POST-never-downgraded-to-GET. After render+media+agentic good-enough.
