#!/usr/bin/env bash
# ── LEVER BOARD — the biggest-impact-first work list + progress tally. The observer owns this; the agent
# CONSULTS it to pick the next tick so it attacks the broadest-impact mechanism first, not a narrow win.
#
# Ranks reachable areas by FAILING subtests, split by FLIP-RATE class: HIGH-FLIP areas (DOM/CSSOM/API/
# selector/parsing surfaces) yield many green subtests per fix; LAYOUT areas (flex/grid/sizing/…) are a
# multi-assertion slog (one fix flips ~nothing) — big in raw count but low yield. Exhaust high-flip mass
# FIRST. The encoding tail is out of v1 scope (V1-SCOPE.md) and excluded from the reachable tally.
#   usage: scripts/lever-board.sh
set -uo pipefail
cd "$(dirname "$0")/.."
A=docs/loop/WPT-AREAS.tsv
B=$'\033[1m'; C=$'\033[36m'; G=$'\033[32m'; Y=$'\033[33m'; O=$'\033[0m'
TARGET=83   # daily-driver MVP DIAGNOSTIC: reachable (non-tail) pass-rate goal. Real bar = real-site drivability.

# ── WORKFLOW WARNING (observer, tick 134). This has HUNG the loop 3× and needed a manual kill each time:
R=$'\033[31m'
printf "%s⚠ NEVER poll with a pgrep-of-your-own-pattern wait loop%s — e.g. \`while pgrep -f \"cargo test X\"; do sleep; done\`\n" "$R$B" "$O"
printf "  or \`until ! pgrep -f \"scripts/tick.sh\"; do ...; done\`. The pattern string is in the WAIT LOOP'S OWN cmdline, so\n"
printf "  pgrep always matches itself, never returns empty, and the loop spins FOREVER even after the real job finished.\n"
printf "  Instead: run cargo/tick.sh/tests in the FOREGROUND and let them block. If the harness backgrounds a long command,\n"
printf "  wait on its OUTPUT-FILE CONTENT (e.g. \`grep -q 'VERIFY:' out\`), never on pgrep of a string your wait command contains.\n\n"

# ── PHASE MANDATE (observer, tick 138+) — DAILY-DRIVER CAPABILITY via a FIXED SEQUENCE. ENFORCED. Full roadmap +
# good-enough bars + agentic fallbacks: docs/wiki/lever-map.md. Agent picks the LOWEST-NUMBERED unmet target.
printf "%s🎯 PHASE MANDATE — pick the LOWEST-NUMBERED unmet target below. Falsifiable bar, NOT WPT%%.%s\n" "$R$B" "$O"
printf "  %s\U0001F3AF PHASE 0 = THE FULL DAILY-DRIVER CHECKLIST, not the 5 finish-line levers (CORRECTED, tick 222). Those 5 (streaming/a11y-states/WebSocket/scroll-anchor/forced-reflow) were a MILESTONE. Phase 0 is COMPLETE ONLY WHEN THE BROWSER WORKS ON ~ALMOST EVERY WEBSITE across ALL classes (doc/app+hydration/social-feeds/platform-realtime/MEDIA) — tracked by scripts/phase0-progress.sh (now ~44%%: doc 44 / app 64 / platform 30 / MEDIA 5 / cross 48) — until every CRITICAL/reachable edge is works|gated and only the honest OUT-OF-REACH set (canvas office-suites, heavy-WebGL, DRM) is unmet. GRIND THE FULL CHECKLIST (constellation --gaps + DAILY-DRIVER-EDGES.md). TOP REMAINING, BORROW where possible: (1) MEDIA/YouTube [5%% — biggest gap; MSE -> symphonia(demux+AAC)+cpal(audio)+libvpx/rav1d(VP9/AV1) -> <video> playback+sync -> controls; VP9+AAC via isTypeSupported; no ffmpeg; DRM out]. (2) PROBE the 35 UNKNOWNS [cheap, many already work — ? outranks X]. (3) IndexedDB [borrow redb/heed — unblocks AWS/GCP]. (4) completeness identity [software-WebGL+honest-strings, visibilityState, permissions.query, userAgentData]. (5) canvas fillText raster. (6) clipboard.read/file-input/actuation-completers. (7) MathML/bidi/CJK. (8) platform holes (Service Worker, WebGL). DO NOT declare Phase 0 done or trigger Phase 1 until the checklist is substantially met.%s\n" "$C$B" "$O"
printf "  %s▶ CO-#1 PRIORITIES (user directive, tick 223) — work these toward the 'IT DOES ALMOST EVERYTHING' full-checklist Phase-0 bar. They are CO-EQUAL; pick any; RE-READ THIS BOARD EACH TICK (priorities update mid-run). BORROW where possible, GATE each with a falsifiable check, land via tick.sh: %s(A) MEDIA/YouTube%s — M1 MSE skeleton -> M2 arraybuffer/Range -> M3 symphonia demux(fMP4/webm) -> M4 AAC+cpal audio -> M5 VP9/AV1 decode(libvpx/rav1d) -> M6 <video> playback+A/V-sync -> M7 controls+WebVTT; VP9+AAC via isTypeSupported; no ffmpeg; DRM out. %s(B) OAUTH LOGIN%s — O1 redirect flow e2e -> O2 interactive iframe RE-RENDER(row 50; also fixes 3DS) -> O3 popup+postMessage -> O4 3rd-party/cross-site cookies -> O5 FedCM navigator.credentials; ASSURE via a REAL-PROVIDER MATRIX gate (Google GIS/GitHub/MS/Apple/Auth0/Okta round-trips). %s(C) canvas fillText%s — HIGH-LEVERAGE: wire the EXISTING engine/text swash glyph-raster to the 2D ctx; unlocks Google Docs/Sheets VIEWING + chart labels + xterm terminals in ONE fix. %s(D) PROBE the ~35 UNKNOWNS%s — cheap constellation '?' probes (measure-and-pin like hydration); each flips a cell + validates the checklist. THEN the rest of constellation --gaps + DAILY-DRIVER-EDGES.md. Phase 0 = the FULL checklist met (almost every website), never a subset.%s\n" "$G$B" "$R$B" "$O" "$R$B" "$O" "$C$B" "$O" "$Y$B" "$O" "$O"
printf "  %s▶ CO-#2 SEAMS + MEASUREMENT (observer, tick 232 — from ENGINEERING.MD; full synthesis: docs/ENGINEERING-SYNTHESIS.md). Track C (CO-#1 above) stays the MAIN LINE and is unchanged. These INTERLEAVE: take ONE when a CO-#1 lever is blocked, stalled, or crash-gated. RATIONALE: these are SEAMS that are cheap NOW and cost MULTIPLES to retrofit, plus MEASUREMENT that redirects everything else. %sS-1 JsEngine trait%s — extract a trait boundary around mozjs (realms, GC handles, host hooks, module loading, microtask queue); ONE impl (mozjs); do NOT write a second engine. Gate: every JS call site routes through the trait, all JS gates still pass. HIGHEST-REGRET DEFERRAL IN THE REPORT. %sS-2 taint+capability boundary%s — mark content-originated DOM strings TAINTED; add a task-scoped capability-token type. INERT at first: the BOUNDARY is the deliverable, enforcement is Phase 2. Prompt injection is unsolvable at the model layer, so it must be structural. %sS-3 no-pixels agent tab%s — a flag in the paint path: layout + a11y tree, paint ELIDED. Gate: an agent tab yields a correct a11y tree with ZERO display items. Saves the whole raster+GPU+composite budget per background tab. %sS-4 NodeId durability%s — GATE that node identity survives re-render/hydration; publish the contract (kills the #1 source of agent flakiness). %sM-1 memory harness%s — RSS+PSS via /proc/PID/smaps_rollup, Tranco top-100 category-stratified, hold at 10/50/100 tabs, report median+p90 per-tab RSS. THIS CAN FALSIFY OUR OWN POSITIONING (within 20%% of Chrome = drop the memory claim) — that is the point; measure before marketing. %sM-2 tri-differential oracle%s — add Firefox as a 2nd reference; flag ONLY diffs where Manuk disagrees with BOTH (Chromium-only diffing overfits to Chromium bugs). %sM-3 Tranco fidelity score%s — wire structural fidelity to the REAL Phase-0 exit rule: >=0.75 on >=95%% of top-1000 + >=0.70 per top-20 category. The 128-cap checklist is the INPUT; this is the GATE.%s\n" "$Y$B" "$R$B" "$O" "$R$B" "$O" "$C$B" "$O" "$C$B" "$O" "$G$B" "$O" "$G$B" "$O" "$G$B" "$O" "$O"
printf "  %s⛔ PARKED (observer, tick 156): SKIP step-6 grid-template-areas — it burned 2h+ across session windows without landing (too big for one atomic tick; no taffy consumer = a subsystem, not a bounded fix). WIP is in git stash. Do NOT pick it; it needs a dedicated decomposition session. Pick the next BOUNDED lever instead (any of: overflow/position render steps, OR the pull-forward/T-tier levers below: U-3 SameSite, T0.4 CORS, T2 crypto.subtle/ReadableStream/:host, T4 forms POST/validation, T5 shell-persistence, T6.1 agent actuation — all ~1 bounded tick).%s\n" "$R$B" "$O"
printf "  %s🔀 DIVERSIFY (observer, tick 159): the RENDER sequence's REMAINING steps are SUBSYSTEM-SCOPE layout-math (grid[parked], z-index/sticky stacking, CSSOM .sheet) — TWO consecutive stalls (grid 2h, t159 86min WIP-discarded). STOP mining the CSS-layout tail; the easy render levers are DONE (t150-158). PICK FROM THE BOUNDED, HIGH-VALUE OTHER-TIER LEVERS NOW (~20min each, daily-driver-critical, all UNBUILT): U-3 SameSite enforcement (storage.rs dead code); T0.4 CORS; T2 crypto.subtle+CSPRNG / ReadableStream+response.body(null today) / :host+::part selectors; T4 POST-navigation / multipart-FormData / constraint-validation; T5 shell bookmarks+settings+history persistence; T6.1 agent activate->Page::dispatch_click. Take a CSS-layout lever ONLY if it is a SINGLE bounded mechanism (<2 files, NO Taffy distribution/sizing/flex-algorithm math).%s\n" "$C$B" "$O"
# ── AUTO-PIVOT (observer, tick 193): systematized staleness detector replaces the one-off REFRESH note.
# It flags when recent ticks cluster in one engine subsystem with Phase-0 readiness flat, and prints the
# current biggest daily-driver holes (constellation --gaps) so the lever rotates to a NEW domain.
./scripts/lever-pivot.sh || true
printf "  html/dom (~93%%) is a done tail — a html/dom-flip tick does NOT advance the mandate. Correction: Taffy #204\n"
printf "  ('Support CSS Grid') SHIPPED; the intrinsic-sizing blocker is MANUK'S OWN leaf measure — fixable in a tick.\n"
printf "  %sRENDER+INTERACT SEQUENCE (build top-down; each bar = 'done for daily-driver'):%s\n" "$G$B" "$O"
printf "   %s1 Intrinsic sizing + wire calc()%s  bar: calc(100%%-250px) sidebar-split ~1px; border-box p-4 child=136px; css-sizing 12->35%%\n" "$G" "$O"
printf "   %s2 SPA link-intercept (a-click preventDefault cancels shell nav)%s  bar: React Link click => pushState, NO reload; push fires no popstate\n" "$G" "$O"
printf "   %s3 IntersectionObserver driven-on-scroll (verify E2E)%s  bar: headless scroll => 2nd screenful appends+fetches; multi-val rootMargin\n" "$G" "$O"
printf "   %s4 Block/inline edges (margin-collapse, %%-height, inline-space bug)%s  bar: 100vh hero exact; a<b>b</b> gains no space\n" "$G" "$O"
printf "   %s5 Flex common-case%s  bar: 3x flex-1 cards w/ long token don't overflow (~1px); justify-between edges exact; css-flexbox 6->25%%\n" "$G" "$O"
printf "   %s6 Grid common-case (WIRE grid-template-areas: no taffy consumer!)%s  bar: repeat(3,1fr) gap exact; areas holy-grail; css-grid 5->20%%\n" "$G" "$O"
printf "   %s7 Overflow scroll-container%s  bar: overflow-y:scroll reserves gutter; overflow:hidden = BFC contains floats\n" "$G" "$O"
printf "   %s8 Positioned + real z-index stacking + sticky in getBoundingClientRect%s  bar: z-50 modal over z-10 header; sticky rect==pinned top\n" "$G" "$O"
printf "   %s9 Runtime CSSOM .sheet bridge%s  bar: insertRule/adopted-sheets actually cascade (styled-components/Lit restyle)\n" "$G" "$O"
printf "  %sPARALLEL TRACK (layout-independent — pull forward on any crash / crash-gated tick):%s\n" "$C$B" "$O"
printf "   %sP-A Stability%s: catch_unwind at JS-native boundary + contain calc-size SIGSEGV (a crash = lose ALL tabs)\n" "$C" "$O"
printf "   %sP-B Agentic floor%s: occlusion-aware hit_test; enabled/stable Conditions; BiDi quiescence; WIRE script.evaluate (protocol.rs:481)\n" "$C" "$O"
printf "  %s⚡ PULL-FORWARD UNBLOCKS (S-sized, dependency-free, high-gating — take on any tick where a layout lever is blocked):%s\n" "$C$B" "$O"
printf "   %sU-1 fetch req body+headers%s: closure is Fn(&str,&str) (event_loop.rs:1760) — POST body + Authorization DROPPED; response.headers always null. Widen it. (silent-fail class)\n" "$C" "$O"
printf "   %sU-2 streaming download-to-disk%s: downloads buffer in RAM + inherit the 30s doc timeout (page/lib.rs:3363) — multi-GB HF weights OOM/timeout. Route attachments through fetch_streaming. (best ROI/tick)\n" "$C" "$O"
printf "   %sU-3 SameSite/prefix enforcement%s: storage.rs (the ONLY SameSite enforcement) is DEAD CODE, 0 callers — live path is a flat jar. Wire it + __Host-/__Secure- prefixes. (login correctness)\n" "$C" "$O"
printf "  %sTHEN media (BIND GStreamer, never write a codec; lead WebM/VP9/Opus):%s M0 video box+reflector -> M1 src= play ->\n" "$Y$B" "$O"
printf "   M2 MediaSource/SourceBuffer (honest 'buffered' TimeRanges) -> M3 hls.js/dash.js unmodified -> M4 ABR+YouTube(clear only; Widevine=gap).\n"
printf "   %s(media is v1-OPTIONAL for the 'document+download web' milestone — build only if YouTube-class watch is declared; M-track foundation is XL not L)%s\n" "$Y" "$O"
printf "  %sTHEN full-tier sequence (transport/JS-platform/shadow-DOM/forms/shell/agentic — lowest-numbered unmet; falsifiable bar, NOT WPT%%; full map: reference/cap-research/ROADMAP.md):%s\n" "$G$B" "$O"
printf "   %sT0 transport enforcement%s: T0.4 CORS (0 hits — cross-origin fetch must reject; preflight; credentials) [M];  T0.5 CSP (0 hits) [fold into security sweep]\n" "$G" "$O"
printf "   %sT1 render polish%s: object-fit/object-position + aspect-ratio [S]; unicode-range parse+gate [S-M]; @font-face descriptors + size-adjust [M]; text-metric CSS (letter/word-spacing,text-transform,word-break,overflow-wrap) [M-L]\n" "$G" "$O"
printf "   %sT2 JS platform%s: boot-global sweep (verify Proxy traps) [S]; crypto.subtle + CSPRNG off Math.random [M]; ReadableStream + response.body (null today) [M]; :host/::part selectors (0 hits — YouTube/Lit visual fix) [M]\n" "$G" "$O"
printf "   %sT2b shadow-DOM%s: custom-element lifecycle (disconnectedCallback/slotchange/HTMLSlotElement/real instanceof) [L, needs per-tag reflector prototypes]; shadowRoot .host/.mode/closed-null + event retargeting [M]\n" "$G" "$O"
printf "   %sT4 forms%s: POST navigation (refuse-LOUD never GET-downgrade; net already POSTs) [S-M]; multipart->nav + FormData file bodies (urlencodes files today=silent drop) [M]; constraint validation (checkValidity/:invalid hard-coded valid) [M]\n" "$G" "$O"
printf "   %sT5 shell persistence%s: persist bookmarks+settings+a (url,title,visit_count,last_visit) history table (all evaporate on quit) [S-M]; per-origin Preferences store [S]; download progress+persisted list [S-M]\n" "$G" "$O"
printf "   %sT6 agentic actuation%s: T6.1 route agent activate/click_at through Page::dispatch_click (fires NO DOM events today — div-onclick SPA buttons un-actionable; dispatch_click already works in GUI) [M, HIGHEST-LEVERAGE agentic]; typing fires input/keydown [M]; BiDi loopback+auth [S]\n" "$G" "$O"
printf "   %sT4.4 passwords (LAST big table-stakes)%s: save-prompt + fill-picker + keyring-unlock — crypto core DONE in store/lib.rs (> Chromium-Linux bar); the iceberg is UX not crypto [L]\n" "$G" "$O"
printf "  %sPOLISH (after table-stakes)%s: IndexedDB MVP [L-XL, keep ABSENT until done — half-built is worse]; dedicated Web Workers [L]; CSSOM .sheet cascade bridge (step 9) [M-L]; variable/graded fonts+RTL [M]; date/range/color widgets [M]; about: pages + settings UI [M]; profiles/incognito [M]; BiDi script.evaluate+keyboard [L].\n" "$Y" "$O"
printf "  %sSKIP v1 (write the deferral down)%s: HTTP/3/QUIC, zstd, coalescing, OCSP (dead), Privacy Sandbox (dead), Service Worker runtime (XL), WebGL, contenteditable/IME, in-engine PDF render (download != render), Widevine (permanent), address/CC autofill, OS process sandbox (explicit non-goal).\n" "$Y" "$O"
printf "  %s⚑ BOT-WALL (separate track, NOT engine work; user: OUT of scope)%s: API-first for Greenhouse/Lever (public no-auth JSON APIs); spoof a Chrome UA by default (get past UA sniffers only); real-Chrome/CDP fallback for IG/X/YouTube; accept the gate + Widevine gap. Do NOT fingerprint-match Manuk's own stack (perishable treadmill; internal-inconsistency = worse than 'unknown client').\n" "$R$B" "$O"
printf "  %s⚡ INNER-LOOP VELOCITY (agent behavior — fewer edit->compile cycles, no rigor drop):%s cargo CHECK before cargo TEST (compile-error turns skip the 350MB mozjs link); do NOT pre-run verify.sh as a green-check (tick.sh owns the single wall — pre-running pays TWO); iterate on the ONE target gate, full 65-gate wall only at tick.sh landing.\n" "$C$B" "$O"
printf "  %sTHEN%s a reasonable security sweep (T0.5 CSP, codec licensing, capability scoping, POST-never-downgraded-to-GET, BiDi loopback default, OS-sandbox-is-a-v1-non-goal).\n\n" "$Y$B" "$O"

awk -F'\t' -v B="$B" -v C="$C" -v G="$G" -v Y="$Y" -v O="$O" -v TGT="$TARGET" '
  NR>1 && $1!="TOTAL" && $1!="encoding" {
    p+=$2; t+=$3
    fail=$3-$2
    if (fail>0) { af[$1]=fail; ap[$1]=$4 }
  }
  END {
    pct = t>0 ? 100*p/t : 0
    need = TGT/100*t - p
    printf "%s══ PROGRESS TALLY ══%s\n", B, O
    printf "  reachable (excl encoding tail): %s%d / %d = %.1f%%%s\n", G, p, t, pct, O
    printf "  daily-driver MVP diagnostic:    %d%% reachable  →  %s%d subtests to go%s\n", TGT, Y, (need>0?need:0), O
    printf "\n%s══ LEVER BOARD — PHASE: pick CSS-LAYOUT / media rows first (★), regardless of flip-rate ══%s\n", B, O
    printf "  %-22s %8s %7s  %s\n", "area", "failing", "pass%", "class"
    # sort areas by failing desc (simple insertion into an index array)
    n=0; for (k in af) { key[n++]=k }
    for (i=0;i<n;i++) for (j=i+1;j<n;j++) if (af[key[j]]>af[key[i]]) { tmp=key[i]; key[i]=key[j]; key[j]=tmp }
    for (i=0;i<n;i++) {
      a=key[i]
      cls = (a ~ /flex|grid|sizing|align|overflow|position|display|contain|multicol|break|masking|transform|css-values|css-color|css-backgrounds|css-ui/) ? "★ CSS-LAYOUT — DAILY-DRIVER (build now)" \
          : (a ~ /media|video|audio/) ? "★ MEDIA — DAILY-DRIVER (build now)" \
          : (a ~ /^dom$|html\/dom|selectors|domparsing|cssom|css-fonts/) ? "html/dom (reasonably done — deprioritise)" : "mixed"
      col = (cls ~ /DAILY-DRIVER/) ? G : Y
      printf "  %s%-22s %8d %6.1f%%  %s%s\n", col, a, af[a], ap[a], cls, O
    }
  }' "$A"
printf "\n%s══ DAILY-DRIVER PRIORITY LEVERS — research-informed; weigh ALONGSIDE the raw board (undervalued by raw count) ══%s\n" "$B" "$O"
printf "  (full map + per-site deps + forecast method: %sdocs/wiki/lever-map.md%s)\n" "$C" "$O"
printf "  The board above ranks by WPT MASS and has a KNOWN BLIND SPOT: it hides cheap, boot-critical APIs whose\n"
printf "  absence BLANK-SCREENS whole SPAs. Weigh by daily-driver UNLOCK, not raw subtest count:\n"
printf "  %s⚡ BOOT-CRITICAL — do FIRST despite low WPT mass (cheap, high-gating):%s\n" "$G" "$O"
printf "     • IntersectionObserver/ResizeObserver  — x.com + Instagram feeds are DEAD without IO   (S, 402 WPT)\n"
printf "     • History / pushState                  — every SPA nav degrades to a full page reload  (S, ~1k WPT)\n"
printf "     • Proxy/Reflect + microtask/MessageChannel — Vue3 reactivity silently dead w/o Proxy    (S-M)\n"
printf "     • Fetch + ReadableStream body          — gates Next.js RSC navigation                  (M, ~19k WPT)\n"
printf "  %s⚡ STEP-FUNCTION — largest raw levers, fully daily-driver:%s\n" "$C" "$O"
printf "     • Flex/Grid intrinsic sizing (min/max-content; Taffy #204) — M-L, ~28k WPT, every site\n"
printf "     • Web fonts (@font-face / WOFF2)                           — M, 8.8k WPT, layout-metric fidelity\n"
printf "     • Shadow DOM + Custom Elements                             — L, 20k WPT, YouTube / web-components\n"
printf "  %sMSE/MEDIA is now a PHASE PRIORITY (not deferred)%s — bind GStreamer/FFmpeg, don't hand-write codecs.\n" "$G" "$O"
printf "  %sDEFER (only these)%s: compositor-threaded scrolling, CSS containment/content-visibility (low value, high cost).\n" "$Y" "$O"
printf "  %sWeight by MANUK'S OWN pass-rate (the TALLY above), never Chrome's%s: an area near 100%% HERE is a done tail,\n" "$Y" "$O"
printf "     but an area still far from 100%% here (e.g. dom) has real high-flip gaps — keep grinding those. The research's\n"
printf "     'giant is done' notes are CHROME's numbers; MANUK's measured failing subtests are ground truth and win.\n"
printf "  %sNEXT this phase%s: CSS flex/grid INTRINSIC SIZING (min/max-content, Taffy #204)  ->  more CSS layout  ->  MSE media pipeline.\n" "$G" "$O"
printf "  %sFORECAST before building%s: cluster the failing subtests by ERROR-SIGNATURE — the biggest same-signature\n" "$B" "$O"
printf "     cluster means one fix flips them all (how tick-113 landed +10,249). See docs/wiki/lever-map.md #forecast.\n"
printf "\n%sHOW TO USE (agent):%s Obey the PHASE MANDATE at the top — pick a CSS-LAYOUT (★) or MEDIA tick, NOT html/dom flip.\n" "$B" "$O"
printf "  Run \`manuk-wpt wpt css/css-flexbox --show-failures\` (or css-grid/css-sizing) for the failure histogram; the top\n"
printf "  mechanism there IS the tick. A layout fix that makes real pages render correctly beats a bigger html/dom +N.\n"
printf "  For media: there is little WPT to chase — build the MediaSource/SourceBuffer surface + a GStreamer/FFmpeg bind,\n"
printf "  and gate it with a falsifiable capability test (a sample stream buffers + plays), not a WPT count.\n"
