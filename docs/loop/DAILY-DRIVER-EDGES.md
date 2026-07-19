# DAILY-DRIVER EDGES — the tractable tail to "it does almost everything"

**Purpose.** Get Manuk from the current ~44% Phase-0 readiness to the *good* end of Phase 0 — reasonably
daily-driving **almost every site a human uses** — with a **minimal, bounded** amount of work, and without
sinking ticks into an obscure diminishing-return long tail. Derived from a 4-way capability sweep (tick 193)
against the named target sites, cross-referenced to `CONSTELLATION.tsv` and verified against engine source.

**The thesis (why this is the whole game).** The browser's daily-driver capability surface *is* the
automation-agent surface. What a human can do — see the page (DOM + a11y tree + pixels), click/type/submit,
traverse links, go back/forward, scroll a feed, log in, play a video — is exactly what an agent can do. The
API surface, the harness, MCP, and consumer prompt-to-action are all **downstream** of this. So closing these
edges is not just "render more sites"; it is widening what any agent built on Manuk can accomplish.

**Target corpus (what "almost everything" means here):** YouTube (home / watch / playlist / shorts / recs),
X/Twitter & Instagram (profile + feed scroll, in-feed media), HuggingFace (model/dataset search + cards),
GitHub (repos, blobs, PRs), and the broad doc web (Wikipedia/MDN/docs/news/blogs/forums). **Explicitly out:**
DRM/Widevine (Netflix/Disney+/Spotify) — a licensed proprietary CDM, cannot be built.

---

## 0. First, the map UNDERSTATES the engine — several rows are STALE

The 4 agents independently found capabilities marked `unknown`/`missing` that are **actually built** (verified
against source this session). Correcting these *raises* the readiness number and stops the lever from chasing
things we already have:

| CONSTELLATION row | says | actually | evidence |
|---|---|---|---|
| form method=POST (41) | missing | **works** | `net/src/lib.rs:626 post_document` (+ cross-site CSRF withholding), tick 164 |
| clipboard (58) | unknown | **works** | `navigator.clipboard.writeText/readText`, dom_bindings.rs:8075 |
| `<dialog>` (82) | unknown | **gated** (dialog); popover still missing | `engine/page/tests/g_dialog.rs` exists |
| CORS + credentialed (47) | unknown | **likely works** | `net/cors.rs`, response headers ~tick 170 — *surface-audit to confirm* |
| font fallback CJK/emoji (8) | unknown | **partial** (built, gated on Noto install) | `engine/text FALLBACK_FAMILIES` (Noto CJK/Emoji) |
| bidi (9) | unknown | **partial** (RTL shaping yes; full reordering deferred) | swash RTL; reorder is a "drop-in upgrade" |
| object-fit / aspect-ratio | — | **works** | ticks 181 / 145 — the near-universal thumbnail idiom is covered |
| navigator identity | (no row) | **90% built** | `webdriver:false`, hardwareConcurrency, vendor, screen, matchMedia all present |

**Action:** a `surface-audit` reconciliation pass alone moves readiness up several points. The risk has shifted
from *"API absent"* to *"is it measured, does it stay fast, does it attach to SSR markup."*

---

## 1. THE CRITICAL UNSCOPED EDGES (the tractable tail — do these)

Grouped by kind. Each is either genuinely missing or under-scoped; none is a diminishing-return tail.

### 1a. Feed & app rendering/interactivity — the make-or-break for X / IG / HF / GitHub
| edge | what breaks without it | scope | notes |
|---|---|---|---|
| **Scroll anchoring** (`overflow-anchor`) | X/IG/LinkedIn feeds JUMP on every load-more and every image reflow | **missing** | `page/src/lib.rs:1473` — "a re-layout silently un-scrolls every container." **The #1 feed-correctness fix.** |
| **Synchronous forced reflow** (`getBoundingClientRect`/`ResizeObserver` after intra-tick mutation) | react-window/virtuoso measure→mutate→measure in one frame → blank/overlapping rows | **under-scoped** | Engine uses *batch* relayout (script runs vs a pre-script snapshot). Needs on-demand reflow when JS reads geometry after a dirtying write. This is the mechanism behind "infinite scroll ❓". |
| **`URL.createObjectURL` / `blob:`** | MSE video setup, blob image previews, blob workers all TypeError | **missing** (verified) | Small, named. Prereq for in-feed video. |
| **SSR hydration** (attach to server markup) | HuggingFace (SvelteKit SSR) is a dead screenshot; GitHub React islands inert | **unknown — PROBE FIRST** | Frameworks *mount* (proven, tick 26); attaching to server-rendered DOM is never measured. May already work like React did — cheap to check, huge if it fails silently. |
| **Large-DOM / code-blob paint perf** | GitHub 1k–5k-line blobs & long PR threads stutter | **unknown** | No node budget / incremental paint. v1 "feels good" is untested on exactly these page shapes. |
| **`<details>`/`<summary>` disclosure** | GitHub READMEs / MDN / docs collapsibles render always-open, no toggle | **missing** | Bounded tick: open attr + click-to-toggle + default marker. |
| **MathML rendering** | Wikipedia math, arXiv, MDN render raw/broken | **missing** | Foreign-content not modelled (`html/sink.rs`). The visible hole in our doc-web strength. |
| **bidi reordering** (Arabic/Hebrew) | RTL doc web in wrong visual order | **partial → finish** | RTL shaping exists; full line reordering is the deferred "drop-in." |

### 1b. Browser-completeness identity — present as a REAL browser (completeness, not evasion)
| edge | what breaks without it | scope | notes |
|---|---|---|---|
| **WebGL context + honest renderer/vendor strings** | Cloudflare/LinkedIn flag a `null`-WebGL browser as headless | **missing** | Biggest residual "looks-headless" tell. Fix = a **real** (software/llvmpipe) GL backend reporting its **true** strings — not a spoof. Also unlocks WebGL apps (row 60). |
| **`document.visibilityState` + Page Visibility** | anti-bot heuristics flag "tab never visible"; autoplay/animation loops gate on it | **missing** (verified) | Trivial: return `'visible'`, fire `visibilitychange`. |
| **`navigator.permissions.query()`** | detectors cross-check it vs `Notification.permission`; the *inconsistency* is itself a bot tell; unguarded calls throw | **missing** | Small shim returning `{state}` consistent with the Notification stub. |
| **`navigator.userAgentData` (Client Hints)** + canonical UA/`platform` | modern UA-sniff reads us as stale/fake; UA contains `"Manuk"`, `platform` is non-canonical `"linux x86_64"` → LinkedIn degraded path | **partial** | navigator is 90% built. Add `deviceMemory`+`userAgentData`, canonicalize `platform` → `"Linux x86_64"`, decide UA policy (a recognized UA string). |

### 1c. Agent-actuation — the automation moat (how an agent acts & confirms)
| edge | what breaks without it | scope | notes |
|---|---|---|---|
| **A11y node STATES** (checked/expanded/selected/disabled/value/focused) | agent CANNOT confirm the result of its own action — is the box checked? is the menu open? | ✅ **BUILT** (re-probed t251) | `A11yNode.state: A11yState` exists, with **tri-state `Checked`** (False/True/Mixed). Gated by `g_a11y_state.rs`. ⚠ **This row read "missing (verified)" and nominated itself the "highest-leverage agentic fix" — it was a PHANTOM.** |
| **Richer interactive a11y roles** (menu/tab/dialog/switch/slider/progressbar) | agent can't ground the web-app widgets it most needs to drive | **partial** | `Role` enum stops at ~26 roles. |
| **hover / dblclick / contextmenu dispatch** | hover-reveal menus, right-click, double-click-select undrivable | ✅ **BUILT** (t245 hover, t251 dblclick+contextmenu) | `dispatch_hover_at` (t245), `dispatch_dblclick` + `dispatch_contextmenu` (t251, `g_mouse_actuation.rs`). dblclick fires the real **click/click/dblclick sequence** with `detail` as the click count. |
| **file-input actuation** (set `FileList` on `input[type=file]`) | every upload flow undrivable | ✅ **BUILT** (t247) | `Page::set_input_files` + a real `FileList`; multipart encode gated. **Row was stale: `DataTransfer` is no longer inert (t248).** |
| **native `<select>` choose + `selectedIndex`/`change`** | dropdowns undrivable | **partial** | Attrs reflect; synthetic option-choice + change firing looks unbuilt. |
| **drag-and-drop** (`dragstart`/`drop`/`dataTransfer`) | Kanban/reorder/drag-upload undrivable | **partial** (t248) | `Page::dispatch_drop` fires `dragenter`/`dragover`/`drop` with a real `DataTransfer` carrying files (`g_drop_upload.rs`) — the **upload** half. Drag-to-REORDER is still closed: no `dragstart` from a draggable source, no `effectAllowed`. |
| **contenteditable + Selection** | rich editors (Notion/Gmail-compose) shaky | **partial** | Range works, Selection is a stub. |
| **IndexedDB** | X/LinkedIn session cache; some auth SDKs (Firebase) hard-fail | **missing** | More a hard capability wall than a detection tell. |

### 1d. Media — the borrow-plan (≈8–9 bounded ticks to a WATCHABLE YouTube)
The de-risking facts: the **frame sink is already built** (`DecodedImage` → wgpu present); the media JS surface
is a clean stub at one wiring point; the net stack does Range trivially. The adaptive players (hls.js/dash.js/
YouTube's own) are **ordinary JS** — they need only (a) binary transport, (b) MSE, (c) `<video>` events.

**Steering insight:** YouTube picks formats from `MediaSource.isTypeSupported()`. Report `true` for VP9 + AAC
and `false` for Opus → YouTube serves **VP9 + AAC**, which **removes Opus (C libopus) and H.264 (patent) from
the critical path**. Borrow **à-la-carte, pure-Rust-first**; reject full ffmpeg for v1 (LGPL packaging + huge C
attack surface vs Manuk's memory-safety value + WALL-punishing build). Each decoder behind a feature flag in
the per-tab sandbox.

| step | capability | borrow | tick |
|---|---|---|---|
| 1 | **Binary transport** — `arraybuffer` XHR + `Response.arrayBuffer()` carrying real bytes; Range header | *(wiring; hyper already does Range)* | 1 |
| 2 | **MSE state machine** — MediaSource/SourceBuffer/`isTypeSupported`(steering)/`createObjectURL`/appendBuffer/`updateend`/`sourceopen`/`.buffered`/`endOfStream` | *(wiring)* | ~2 (subsystem, fresh context) |
| 3 | **Demux** fMP4/WebM | **`symphonia`** (MPL-2.0 — same license as Manuk) | 1 |
| 4 | **AAC decode + audio out** | `symphonia` (AAC, pure-Rust) + **`cpal`** (audio device = master clock) | 1 |
| 5 | **VP9 decode** → RGBA `DecodedImage` | **`libvpx`** (mature/fast) or **`rav1d`** for AV1 (memory-safe Rust) | 1 |
| 6 | **Playback engine + A/V sync** — currentTime/seek/timeupdate/playing/waiting/ended, play/pause | *(wiring on cpal clock)* | ~2 (subsystem) |
| 7 | **Controls + buffering + fullscreen + WebVTT** | *(shell chrome)* | 1 |

Home/playlist/shorts/recs are ordinary DOM+JS that already render — once the watch page plays, they come along
(Shorts = same MSE path, portrait layout). **Twitch (live), +~3–4 ticks later:** MPEG-TS demux (`mpeg2ts`, or
ffmpeg escape-hatch) + H.264 (`openh264` Cisco binary) + low-latency tuning.

---

## 2. THE DIMINISHING-RETURN TAIL — consciously SKIP (do NOT spend ticks here)

- **DRM / EME / Widevine** — *permanently out* (licensed CDM; cannot be built).
- **Opus decode** — steer YouTube to AAC via `isTypeSupported`; no mature pure-Rust decoder, not worth C libopus.
- **H.264** — out for YouTube (VP9/AV1 served); Twitch-only, and only via Cisco `openh264` prebuilt.
- **Full ffmpeg** — rejected for v1 (LGPL + C surface + WALL build cost); reserve only as Twitch-TS escape hatch.
- **Service Worker / Cache API / PWA / Web Push** — progressive enhancement; sites load fine on first visit.
- **WebSocket / SSE for feed render** — X's timeline is GraphQL *polling*; WS only affects DMs/live badges.
- **Variable-font axes, Popover API, multicol, `@media print`, code-fold, `-webkit-line-clamp`, backdrop-filter,
  PiP, WebCodecs, `navigator.plugins` real enumeration, XPath targeting, IME/composition** — cosmetic, niche,
  or covered by a cheaper path (dialog covers modals; agents inject final text sidestepping IME; empty plugin
  stub suffices).

---

## 3. Completeness vs the "no bot-detection-fighting" scope — reconciled

These are compatible; the line is **whether the underlying thing is real**:
- **In scope (completeness):** expose surfaces a genuine headful browser *has*, with **honest** values —
  `webdriver:false` because we truly aren't WebDriver-driven; a **real** software-GL context with its **true**
  renderer string; `visibilityState:'visible'` because the tab is; real `screen`/`devicePixelRatio`. When a
  site challenges Manuk here, it is *mis-classifying a real browser* — fixing that makes the browser more real.
- **Out of scope (evasion), unchanged:** lying — claiming a GPU we lack, rotating/randomizing fingerprints,
  spoofing a *specific* competitor build, defeating a challenge by pretending to be something we're not.
- **The one honest tension:** a truthfully *software-rendered* WebGL string is itself sometimes used as a
  headless heuristic. We ship the real string; if a site still blocks a truthfully-software browser, that is
  the site over-fitting, and chasing it further is the evasion arms race the constitution declines.

This refines [[scope-botdetection-shell]]: **completeness IS in scope; evasion is NOT.**

---

## 4. Site-class readiness map (the gating edge per target)

| site class | already strong | the gating edge(s) to reach good-enough |
|---|---|---|
| **Doc web** (Wikipedia/MDN/news/blogs/forums) | ~90%+ — DOM/CSS/tables/fonts | MathML, `<details>`, bidi reorder |
| **GitHub** | DOM, routing, markdown, syntax spans | large-blob paint perf, `<details>`, hydration-probe |
| **HuggingFace** | SPA routing, fetch, cards | **hydration** (probe first), virtualized-list forced-reflow |
| **X / Instagram** | fetch/GraphQL, IO, scroll, object-fit tiles | **scroll anchoring**, forced-reflow, `createObjectURL`, in-feed video (media), completeness identity |
| **LinkedIn** | most of the app surface | completeness identity (userAgentData/UA/WebGL/visibilityState), IndexedDB |
| **YouTube** | home/playlist/shorts DOM renders | the media borrow-plan (steps 1–7) |
| **agent driving any of the above** | click/type/key/focus/scroll-into-view, a11y roles+names | **a11y STATES**, richer roles, hover/dblclick, file-input, `<select>`, drag-drop |

## 5. Suggested sequencing (probe-cheap first, then bounded builds, then media)
1. **Reconcile the stale rows** (surface-audit) — free readiness + honest map.
2. **Probe the cheap unknowns** (`? outranks ✗`): hydration, CORS-confirm, drag-drop, AVIF — each either flips
   green or reveals a real hole to prioritize.
3. **Bounded high-value builds:** scroll-anchoring, forced-reflow, `createObjectURL`, a11y-states, `<details>`,
   visibilityState + permissions.query + userAgentData, hover/dblclick dispatch, file-input actuation.
4. **The two subsystems** (fresh context, not bounded ticks): the completeness WebGL backend; the MSE→playback
   media chain (steps 1–7).
5. **MathML, richer a11y roles, `<select>`/drag-drop actuation, IndexedDB** as they surface as gating.

The lever-pivot (`scripts/lever-pivot.sh`) + this checklist keep the loop aimed at *constellation-moving*
capability instead of an incremental render tail.

---

# PART II — THE GENERALIZED INTERNET (representative classes + honest reachability)

Extends the target set from a few sites to a representative split of what a present-day user actually uses,
from a second sweep (cloud-console/dev-infra/AI-inference + productivity/AI-chat/commerce), verified in source.

## The dominant finding: STREAMING is the #1 unlock
Every modern **AI-chat UI (claude.ai / ChatGPT / Gemini / Grok)** streams its answer via
`fetch().body.getReader()`; **cloud-console live logs** and **AI-inference playgrounds (Groq / Cerebras)**
stream via SSE / chunked fetch. Today **all of it is inert** — `ReadableStream` is a name-only stub, SSE is a
non-connecting stub, and `__deliverXhr` hands the whole body over at once (no `readyState 3`). So an AI-chat
**answer literally never renders.** The plumbing is *half-built*: `manuk_net::fetch_streaming(url, on_chunk)`
already exists — the missing piece is **bridging chunks into a JS `ReadableStream` / progressive XHR / a real
EventSource**. This one fresh-context subsystem flips the entire AI-chat class (the highest-value new target)
from broken to daily-drivable, and unblocks console live-logs + inference token output at the same time.

## Reachability by class — the honest map
| tier | classes / sites | state |
|---|---|---|
| **Reachable now (rendering-bound)** | doc/reference web (Wikipedia/MDN/news/blogs/forums), marketing & realty pages (Wix/Squarespace/Zillow chrome), **GitHub** repos/blobs, e-commerce browse/cart/gallery, **Google Drive** file list, simpler SaaS dashboards | strongest; residual risk is hydration-probe + large-DOM perf |
| **One subsystem away (highest ROI)** | **AI-chat** (claude.ai/ChatGPT/Gemini/Grok) + **inference playgrounds** (Groq/Cerebras) + **console live logs** → all gated on **streaming**; **social feeds** (X/IG/LinkedIn) → gated on **scroll-anchoring + forced-reflow** | bounded/subsystem work already scoped |
| **Login-gated** | **AWS & GCP consoles** — blocked at the front door by **IndexedDB**-backed auth SDKs (Cognito/Amplify, Google Identity); **banks** — need completeness identity + strong cookies (passkey-only would block; TOTP survives); OAuth/OIDC consoles pending the **OAuth redirect probe** | **DigitalOcean / GroqCloud / Cerebras plausibly reachable** if OAuth probes pass |
| **Out-of-reach subsystems (honest)** | **Google Docs / Sheets** (custom **canvas-text** engine — `fillText` is a no-op → blank doc/grid; also IME + rich clipboard); **Canva** (WebGL editor); **vector Google Maps** (WebGL tiles); **DRM media** (Netflix/Spotify) | consciously deferred; each is a Figma-class hole, not a bounded tick |

## New critical edges this sweep added (folded into CONSTELLATION.tsv)
- **fetch streaming response body** (ReadableStream / progressive XHR / real SSE) — *the* AI-chat/console unlock.
- **canvas `fillText` rasterization** — chart labels, xterm.js terminals, canvas apps (bounded: raster exists in `engine/text`).
- **`clipboard.read()` + paste-image** — AI-chat screenshot paste; pairs with file-input actuation + `createObjectURL` as the upload path (also the agent-upload path).
- **cross-origin iframe re-render on mutation** — 3-D Secure checkout challenge, interactive OAuth frames (row 50: readable but a frozen snapshot).
- **password vault (save + autofill)** — the one real gap in the persistence layer (below).
- Reclassified: **WebSocket** (row 53) & **SSE** (row 54) from social-feed-TAIL to **CRITICAL** for cloud/AI; **WebAuthn** (row 96) unknown→missing (TOTP fallback survives).

## PERSISTENCE / UX — the state layer (audited: strong, one gap)
`shell/src/session.rs` persists to disk and restores on launch — **bookmarks · history** (credential-URL
redacted) **· settings · downloads · session/tab restore** (incl. named collections) **· cookies** (RFC 6265,
flushed on exit) **· localStorage** (per-origin, beside the jar). Session retention across restarts — what
makes it a *real* daily driver — is built. **The one gap: a password vault (save + autofill);** today only
credential-URL redaction exists. Bounded shell work, in v1 scope.

## AGENTIC FOUNDATION — solid core, small named gaps
Present: CSS + role/name targeting, `dispatch_click / type / input / key / blur / submit`, `scrollIntoView`.
So an agent can find, click, type into controlled React/Vue fields, submit, key, and scroll. The gaps are
**named and bounded**, not architectural: (1) **a11y node STATES** (checked/expanded/selected/value — an agent
acts blind to its own result); (2) **extra dispatchers** (hover/dblclick/contextmenu/file-input/drag);
(3) the upload path (file-input + `createObjectURL` + `clipboard.read`).

## Prioritized unlock sequence (max coverage per tick)
1. **fetch streaming subsystem** → AI-chat + inference + console logs (highest value; half-built).
2. **a11y node STATES** → the agentic moat (agent can confirm its own actions).
3. **scroll-anchoring + forced-reflow** → all feeds + console data grids.
4. **canvas `fillText`** → chart labels + terminals (bounded).
5. **clipboard.read + file-input + `createObjectURL`** → AI-chat upload + agent upload.
6. **password vault** → daily-driver UX completeness.
7. **IndexedDB** → unblocks AWS/GCP login.
8. **completeness identity** (WebGL string / visibilityState / permissions.query / userAgentData) → banks + enterprise consoles behind Cloudflare.
9. **media borrow-plan** → YouTube.

**Consciously OUT (don't spend ticks):** Google Docs/Sheets canvas-text, Canva/vector-Maps WebGL, DRM,
PaymentRequest/Apple-Pay (card-form fallback reachable), autofill-engine polish, Web Workers for this class.

**Confidence read:** the path from here to "daily-drives almost everything a person uses" is a *small set of
named, mostly-bounded subsystems* (streaming, a11y-states, scroll-anchoring, canvas-text, the media chain) plus
a short honest out-of-reach list (office suites, heavy-WebGL creative apps, DRM). That is a sound foundation to
build the downstream agentic layer on — because the automation surface IS this daily-driver surface.

---

# PART III — comprehensiveness & the honest parity verdict

## Business / banking / finance — no new primitive, same edges (the key structural finding)
Adding enterprise SaaS (Salesforce/Workday/ServiceNow/Jira/Slack-web), banking apps, and stock/finance/trading
platforms introduces **zero new critical unscoped edges** — each recombines primitives already on the list:
- real-time price feeds + rapid grid updates → **WebSocket + streaming + forced-reflow + large-DOM perf** (finish-line/scoped)
- transaction tables / resource grids → **virtualized data grid = forced-reflow + scroll-anchoring** (finish-line)
- enterprise/bank auth → **form-POST (works) + OAuth-probe + iframes (gated) + completeness-identity**; **WebAuthn/passkeys** missing but **TOTP/password fallback** survives
- dashboards/charts → **SVG (works-ish) + canvas-fillText (scoped)**; **advanced WebGL charts (TradingView-tier) out-of-reach**
- reports → **print/@media-print (unknown, minor gap)**; collaborative rich-text (Notion/Confluence) → **Selection (partial gap)**

Reachability: business SaaS + web-banking + finance-*data* are **reachable** once the finish-line lands (WebSocket
+ completeness + grids), with three honest exceptions — **advanced WebGL charts**, **canvas-native office suites
(Office 365 web, like Google Docs)**, and **banks that whitelist Chromium-only** (their policy, not our capability).

## The honest verdict on "runs almost every website with Chromium parity"
The question bundles two different claims; they have different answers.
- **"Runs almost every *type* of website, usably and faithfully"** — **YES by end of Phase 0.** The classes are
  covered, the primitives generalize (business/banking/finance prove it by re-using them), and rendering is
  Chromium-*faithful* (fidelity oracle, ~0.75+ structural). This is the claim to hold.
- **"Full Chromium *parity* — pixel-identical + every API — on almost every website"** — **NO, and that is not the
  Phase-0 bar.** There is an honest tail Phase 0 consciously does not close: (a) **out-of-reach subsystems** —
  canvas-native office/creative (Google Docs/Sheets, Office-365-web, Canva, Figma), heavy/advanced WebGL (vector
  maps, TradingView-tier charts), DRM; (b) a **rendering tail** — complex flex/grid distribution, subgrid,
  container-queries (unmeasured), MathML, bidi-reorder, `ch`/`ex` font metrics → sub-pixel-perfect on hard layouts;
  (c) **quirks-mode + the niche-API long tail**; (d) **whitelist-only sites** that block non-Chromium by policy.

**So: by end of Phase 0 the browser faithfully renders and is usable on the large majority of mainstream sites
across every major class — "runs almost every website" in the practical sense — but it is NOT literal
pixel-and-API parity with Chromium on 100% of the web.** The gap is a conscious, named tail (diminishing-return
CSS/quirks + genuinely-out-of-reach canvas/WebGL/DRM subsystems), not a hidden hole.

## Is the checklist comprehensive? (the honest hedge)
High, *measured* confidence it captures the generalized primitives — the recurrence of the same edges across
unrelated classes is the evidence. But it is **not a proof of exhaustiveness**: ~35 capabilities remain `unknown`
(unprobed), and some are Interop-2026 rows never measured. The **surface-audit** (reconcile the map against the
world) and the **fidelity oracle** (visual diff vs Chromium) are the standing checks that catch a class we
haven't stress-tested — so the honest posture is "comprehensive for what we've measured, with an explicit
instrument for what we haven't," not "certified complete."
