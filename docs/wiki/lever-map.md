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

## Full-tier extension (after RENDER 1–9 + MEDIA M0–M4 — the sufficient daily-driver set)
Master: reference/cap-research/ROADMAP.md. Lowest-numbered unmet wins; falsifiable bar, NOT WPT %.
Three of these (U-1/U-2/U-3) are PULL-FORWARD: S-sized, dependency-free, take them on any blocked-lever tick.
⚑ RE-BASELINE (stale-item correction, tick 147): step 6 grid-template-areas HAS a taffy consumer now, step 8
z-index stacking is PARTLY shipped, and M0 poster paints — PROBE (`--show-failures`) before rebuilding these
three; the synthesis found them further along than the step text above says.

- **U-1 fetch req body+headers** [net/js, S] — bar: POST w/ JSON body + Authorization reaches the socket; response.headers.get('content-type') != null. Closure Fn(&str,&str) at event_loop.rs:1760 drops body+headers (silent-fail class). Depends: none.
- **U-2 streaming download-to-disk** [net, S] — bar: multi-GB HF *.safetensors streams to disk (no OOM), no 30s timeout, cross-host 302 followed. Route attachments through fetch_streaming, not fetch_document (page/lib.rs:3363). Depends: none. (best ROI/tick — the named download requirement.)
- **U-3 SameSite/prefix enforcement** [net, S] — bar: SameSite=Lax cookie NOT on cross-site subresource, IS on top-level OAuth redirect; __Host-/__Secure- enforced at set-time. Wire the dead storage.rs (0 callers) into send_once (lib.rs:952). Depends: none.
- **T0.4 CORS enforcement** [net, M] — bar: cross-origin fetch w/o Access-Control-Allow-Origin rejects (opaque/error); preflight for non-simple; credentials mode honored. 0 hits repo-wide. Depends: none.
- **T1.b/c object-fit + aspect-ratio** [layout, S] — bar: object-fit:cover crops poster/img; aspect-ratio:16/9 card sizes. Depends: 1. (aspect-ratio mapping LANDED t145; object-fit still open.)
- **T1.d–f web-text fidelity** [css/text, M–L] — bar: unicode-range gates Google-Font subsets; @font-face weight/style/size-adjust honored; text-transform:uppercase changes width; overflow-wrap:anywhere forces break. unicode-range = 0 hits. Depends: 1,4.
- **T2.1 boot-global sweep** [js, S] — bar: Vue3 createApp boots (Proxy traps correct); no ReferenceError at module eval on corpus. Verify-only (most already wired). Depends: none.
- **T2.2 crypto.subtle + CSPRNG** [js, M] — bar: crypto.subtle.digest works; getRandomValues cryptographically secure (off Math.random, event_loop.rs:1319). Depends: none.
- **T2.4 ReadableStream + response.body** [js, M] — bar: for-await over resp.body streams; LLM token-stream UI works. body hardcoded null today. Depends: none.
- **T2.5 :host/::part selectors** [css, M] — bar: web-component :host{display:block} applies; ::part() themes. 0 hits — highest-leverage shadow-DOM fix (YouTube/Polymer/Lit visual). Depends: css shadow scoping (done).
- **T2.6 custom-element lifecycle** [dom/js, L] — bar: el instanceof MyElement true; disconnectedCallback fires; slotchange dispatches; HTMLSlotElement.assignedNodes(). Needs per-tag reflector prototypes (architectural). Depends: T2.5.
- **T4.1 POST navigation** [forms, S–M] — bar: <form method=POST> navigates; refused-LOUD never GET-downgrade. Net already POSTs (gui.rs:2282); wire agent/forms.rs + shell/gui.rs. Depends: 2.
- **T4.2 multipart→nav + FormData files** [forms/net, M] — bar: résumé PDF uploads via <input type=file>; fetch(body:formData) sends bytes (urlencodes files today = silent drop, dom_bindings.rs:7765). Encoder+builder exist, unwired. Depends: T4.1.
- **T4.3 constraint validation** [dom/css, M] — bar: required/type=email/pattern block submit; :invalid matches (hard-coded valid, stylo_dom.rs:362). Depends: none.
- **T5.1 shell persistence** [shell, S–M] — bar: bookmarks.json + settings.json + (url,title,visit_count,last_visit) history table survive restart (all evaporate today). History table is the omnibox prerequisite. Depends: none.
- **T6.1 agent actuation** [agent/page, M] — bar: agent click on <div onclick> SPA button runs the JS handler; typing fires input/keydown. Route AgentBrowser.activate (agent/lib.rs:664) through Page::dispatch_click (already works in GUI). HIGHEST-LEVERAGE agentic fix. Depends: none.
- **T6.2 BiDi loopback+auth** [bidi, S] — bar: non-loopback bind refused; handshake token-checked (server.rs:37 raw TcpListener). Depends: none.
- **T4.4 password save/fill UX** [shell/store, L] — bar: save-prompt on login; exact-origin fill picker; keyring-unlock flow. Crypto DONE (store/lib.rs, > Chromium-Linux); iceberg is UX. The named "stay logged in via passwords" requirement. Depends: T4.1, T5.1.

## Bot-wall posture (separate track — NOT engine capability; user scoped OUT)
API-first (Greenhouse boards-api / Lever postings API = public no-auth JSON, zero fingerprint work);
spoof a Chrome UA by default (get past naive UA sniffers only — NOT fingerprint-matching); real-Chrome/CDP
fallback for IG/X/YouTube; accept the gate + Widevine permanent gap. Do NOT fingerprint-match Manuk's own
stack: internal inconsistency (TLS=rustls, h2=hyper, canvas=tiny-skia, UA=Chrome) is worse than an honest
"unknown client", and it's a perishable treadmill against monthly Chrome drift + immune behavioral/reputation layers.

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
