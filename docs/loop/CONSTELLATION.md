# THE CONSTELLATION — what "a browser you can actually use" means, mechanically

**WPT is the scoreboard. This is the goal.**

A 90% WPT score with no video, no OAuth popup and no real iframe is not a daily driver — it is a very
well-tested rendering library. And a browser that renders 99% of the web but leaks memory across 100 tabs
is not one either. So the near horizon needs a definition that is **qualitative in what it demands and
mechanical in how it is checked**, or it will drift the way the pattern ledger drifted six times.

This file is that definition. It is not prose: it is the human-readable face of
[`CONSTELLATION.tsv`](CONSTELLATION.tsv), which `scripts/constellation.sh` scores every tick and
`scripts/ratchet.sh` refuses to let regress.

---

## The four classes of the web, and what each one actually demands

The web is not one thing, and "supports the web" is not one capability. It is four overlapping
constellations, and a browser can be excellent at one and useless at the next.

### 1. DOC WEB — where text and content live
*Wikipedia, arXiv, MDN, news, blogs, government forms, wikis, documentation.*

The oldest web and still the largest by page count. It demands **breadth, not depth**: a thousand small
things, each of which must simply work. Its failure mode is not a crash — it is a page that renders
*almost* right, and is therefore quietly wrong.

The hard parts are not the ones people expect:

* **Text is the whole thing.** Shaping, bidi (Arabic/Hebrew), CJK line-breaking, ligatures, font fallback
  across scripts, emoji. A browser that cannot fall back from a Latin font to a CJK one renders a Japanese
  article as a wall of tofu boxes, and no amount of correct CSS saves it.
* **Legacy character encodings.** A page served as Shift_JIS, Big5, GBK or EUC-KR is *mojibake* without
  its decoder — which is most of the pre-2010 Japanese, Chinese and Korean web, and a great deal of what
  is still linked from it. **This is not a niche: it is ~767,000 WPT subtests and a large fraction of the
  non-English internet.**
* **Tables, floats, multicol, print styles.** The CSS nobody writes any more and every academic page uses.
* **Find-in-page, selection, copy.** Reading is not passive.

### 2. APP WEB — the SPA, rendered by a framework
*React, Vue, Svelte, Angular, Solid; dashboards, editors, internal tools.*

Demands **depth in the DOM and the event loop**, and it is unforgiving: a framework does not degrade, it
throws. One missing primitive and the page is blank — not degraded, *blank*. Every gap here is a cliff.

* **The DOM must be a real DOM**: prototypes you can patch, live collections, `MutationObserver` that
  actually observes, attribute reflection. (Every one of these was a stub or absent, and each cost a tick.)
* **The event loop must be a real event loop**: microtasks, `requestAnimationFrame`, `{once, passive,
  signal}`, `AbortController`.
* **ES modules, dynamic `import()`, top-level `await`.**
* **Web Components**: custom elements, shadow DOM, slots, `adoptedStyleSheets`, constructable stylesheets.
  This is how design systems ship, and how Lit works.
* **Hydration** — server-rendered markup that the client re-attaches to. It is the dominant delivery
  pattern and it fails *silently* when the DOM disagrees with the server's.
* **Routing without navigation**: `pushState`, `popstate`, the History API.

### 3. PLATFORM WEB — accounts, feeds, media, the sites people actually live in
*Twitter/X, Reddit, YouTube, Gmail, Figma, Notion, GitHub, Slack, banking, shopping.*

Demands **the systems no rendering engine has**, and this is where a from-scratch browser dies. These
sites are not documents and they are not apps — they are *clients* for a backend, and they assume the
browser is a platform.

* **Identity.** Cookies with `SameSite`/`Secure`/`HttpOnly`, CORS with credentials, and — the one that
  blocks *everything* — **OAuth**: a redirect flow, or a `window.open` popup that talks back via
  `postMessage`. **You cannot log in to most of the modern web without it.** No login, no platform web.
* **Real iframes.** Not a rendered bitmap — a **nested browsing context** with its own document, its own
  global, its own script, and `postMessage` across the boundary. Embeds, payment frames, OAuth frames,
  ads, YouTube players. *We currently render iframes as a picture of a page, which is why 596 WPT subtests
  in one file fail at `doc.defaultView`.*
* **The infinite feed.** `IntersectionObserver` + `scrollTop` + virtualisation + lazy images + a
  `WebSocket`/SSE for live updates. Every one of these must be right *simultaneously* or the feed either
  freezes, duplicates, or loads the entire timeline at once.
* **Media.** See below — it is its own constellation.
* **Service Worker + Cache API.** Offline, PWAs, and the reason a returning visit is instant.
* **Uploads**: `<input type=file>`, drag-and-drop, `FileReader`, `Blob`, progress.
* **Canvas/WebGL**: charts, maps, Figma, games.

### 4. MEDIA — the short pole
*YouTube, Netflix, Twitch, Spotify, and every autoplaying video in every feed.*

The single biggest hole in every independent engine, and the one that cannot be faked. It has a **short
pole** (play a file) and a **long subsystem** (stream it adaptively, decrypt it, sync it, hardware-decode
it).

* **Demux**: MP4/ISOBMFF, WebM, MPEG-TS.
* **Decode**: H.264, VP8/VP9, AV1 (video); AAC, MP3, Opus, Vorbis (audio).
* **Output**: an audio device, A/V sync, frame pacing.
* **MSE (Media Source Extensions)** — how YouTube, Twitch and every adaptive player *actually* deliver
  video. Without it, `<video src>` plays a file and **nothing on the real web plays at all.**
* **EME/Widevine** — DRM. Netflix, Prime, Spotify. A settled, stated deferral: it requires a proprietary
  CDM, and it is the one capability that cannot be built, only licensed.
* **WebVTT** subtitles.

> **Borrow, do not build.** The methodology's standing rule for this class: media is decades of
> other people's work. `symphonia` (Rust, audio), `dav1d`/`rav1e` (AV1), `openh264`, `cpal` (audio out),
> and `ffmpeg` behind a feature flag. A tick spent writing an H.264 decoder is a tick not spent on the
> browser. **The engine's job is the plumbing — demux, buffer, sync, present — not the codec.**

---

## Cross-cutting, and non-negotiable

These are not a class; they are the conditions under which the other four are worth anything.

| | what it means | how it is held |
|---|---|---|
| **Bar 0** | No hang, no crash, no unrecoverable panic. Ever. | `G_CONTAIN`, `G_HANG`, `G_CLEAN_EXIT`, and the ratchet's `crashes = 0` |
| **Lean** | The same URL never goes to the wire twice. No duplicate work in the call graph. | `G_DEDUP`, and the ratchet's `dupes = 0` |
| **Fast** | Parse/cascade/layout/paint budgets that a regression **fails the tick**. | The perf floors (F1/F2), binding |
| **Memory** | 100 tabs without collapse. Process-per-tab; hibernate the cold ones. | The standing 100-tab RSS benchmark |
| **Graceful** | The narrow tail degrades — it does not take the page with it. | *A stub is worse than an absence.* A feature we do not have must say so (`getContext('webgl') → null`), never lie. |

---

## How this is enforced, so it cannot drift

The pattern ledger drifted six times because it was **prose that nobody executed**. This does not:

1. **[`CONSTELLATION.tsv`](CONSTELLATION.tsv)** — one row per capability: `class`, `capability`,
   `what breaks without it`, `status`, `gate`, `receipt`.
2. **`status` may only be `gated`** if a named `G_*` gate asserts it. `works` requires a probe receipt in
   the journal. **`unknown` is a bug**, not a state — it means nobody has measured, and *an absent
   measurement is not a negative measurement.*
3. **`scripts/constellation.sh`** scores coverage per class every tick and prints the largest missing
   capability *per class* — so the loop cannot spend twenty ticks perfecting the doc web while the
   platform web has no login.
4. **`scripts/ratchet.sh`** refuses any tick that lowers a class's gated count.
5. **`scripts/orient.sh`** prints it beside the WPT map, so **both horizons are visible in the same
   breath** — because they are nearly orthogonal (measured, tick 70) and optimising one blind to the other
   is how ten ticks went into the wrong room.

**The tick is chosen by whichever is further behind**: the WPT mechanism with the most failing subtests,
or the constellation class with the largest hole. Not by whichever is more interesting.
