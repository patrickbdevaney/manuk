# MEDIA — the tick plan, and the two walls

**Researched 2026-07-13.** The question was *"can media playback be collapsed from a weeks-long subsystem
into ticks?"* **Yes — for most of the web's `<video>` elements. No — for MSE, and never for DRM.**

## The structural insight that makes this small

> **A video frame is a `DecodedImage`. Playing a video is swapping the `Rc` in a map we already have, and
> calling `request_redraw`.**

`engine/paint` already has `DecodedImage`, `blit_image` into a `tiny_skia::Pixmap`, and
`NodeImages = HashMap<NodeId, Rc<DecodedImage>>`. `engine/page` **already puts a `<video>`'s poster into
that exact map** (tick 28). The shell already drives `request_redraw`.

**Not one line of new paint code is required.** A `<video>` is already an `<img>` that gets a new picture
thirty times a second. *That* is why this collapses into ticks instead of a subsystem — and it is only
true because the poster work landed first.

## The finding that overturned the obvious plan

**There is no pure-Rust H.264 decoder that can decode the H.264 the web actually serves.**

- `openh264` (Cisco, BSD-2) decodes **Constrained Baseline only** — B-frame reordering is unimplemented.
  This is *exactly why Firefox uses OpenH264 for WebRTC and never for `<video>`*.
- `rusty_h264` is genuinely pure Rust and **also Constrained Baseline only**. The Rust-purity is real; the
  capability is not. **A trap.**
- The web's H.264 is overwhelmingly **High profile** (`libx264`'s default: CABAC + B-frames + 8×8).

**But:** YouTube's no-MSE fallback is `avc1.42001E` — **Baseline**, 360p — which is exactly what
`openh264` *can* decode, with **zero system dependencies**. So the ladder is real: `openh264` gets a
working pipeline with `cargo build` and nothing installed; ffmpeg/VA-API gets the rest **behind the same
trait**.

## The tick plan

**Pre-tick (10 min):** workspace MSRV is 1.80; `re_mp4` needs **1.92**, `symphonia` needs 1.85. Bump first
or nothing compiles.

| Tick | Lands | Crates | Size |
|---|---|---|---|
| **1** | **The video's real first frame renders where the poster was.** Proves demux + decode + colour + paint end-to-end, and it is *visible*. | `re_mp4` (demux) · `openh264` (decode) · `yuvutils-rs` (YUV→RGBA) | ½–1 day |
| **2** | **Muted autoplay looping `<video>` actually plays.** | + a decode thread, wall clock | 1–2 days |
| **3** | **Sound, in sync — and `<audio>` for free.** | `symphonia` (audio) · `cpal` (output) | 1–2 days |
| **4** | **Seek, scrub, controls.** | `re_mp4`'s sample table · HTTP `Range` | 1–2 days |
| **5** | **High profile — the rest of the real web.** | `cros-codecs` (VAAPI, Linux, HW, Rust) · `ffmpeg-next` (feature-gated, OFF by default) | 1–2 days |
| **6** | **AV1** — the codec the web is moving to, in memory-safe Rust. | `re_rav1d` (BSD-2, pure Rust) | ~1 day |
| **7** | **WebM containers.** | `matroska-demuxer` | ½–1 day |

**Tick 2 is the highest (web unlocked)/(effort) item in the whole plan.** Hero videos, background loops,
product demos and GIF-replacement clips are a large fraction of *all* `<video>` on the open web — and
**none of them have an audio track, need a clock, or need ABR.** A decode thread and a wall clock unlock
them.

**Define `trait VideoDecoder` in tick 1.** Every later backend drops in behind it. It is the single most
important design decision here and it costs nothing today.

## The two walls, stated once

### MSE — genuinely weeks (2–4), and it is the correct *next* project, not a tick

`MediaSource` + `SourceBuffer` + `appendBuffer` + buffered `TimeRanges` + quota/eviction + a fragmented-MP4
demuxer that works on *incrementally appended* byte ranges. Only after this do hls.js / Shaka / video.js
sites work (Twitch, Vimeo's default player, YouTube above 360p). **It must come after ticks 1–7** — it
needs a working decode pipeline to append into.

> **⚠ DO NOT advertise `MediaSource` / `ManagedMediaSource` before it works.** Their *absence* is what makes
> YouTube serve the progressive 360p fallback. **Advertising MSE we cannot honour turns a working YouTube
> into a black rectangle.** This is the same discipline as `canPlayType() === ""` — extend it to MSE.

### EME / DRM — not weeks. **Never.** (Settled: STATUS.md)

The Widevine CDM is a **proprietary binary requiring a per-browser licensing relationship**. `OpenWV`
reimplements the CDM API but ships **without a device identity** — it needs a private key you cannot
legitimately obtain. It is a key-extraction path, not a licensing path. **Do not go near it.**

- **Unreachable, permanently:** Netflix, Disney+, Max, Hulu, Prime Video, Apple TV+, Peacock, Spotify web.
- **Reachable with no EME at all:** YouTube (360p fallback), Vimeo (non-DRM), X, Reddit, Bluesky, Wikimedia,
  Imgur, news embeds (BBC/Guardian/NYT), and **essentially 100% of ordinary-web `<video>` elements** —
  which are muted autoplay loops with no DRM, no audio, and no ABR. **That last clause is the strategy.**

## Traps (each one cost someone a week)

1. **`symphonia` is not a video demuxer.** Its ISO-MP4 `SampleEntry` has `// Video,` *commented out*; only
   audio entries parse. The 0.6.0 video *types* exist and the demuxers do not populate them. **Use it for
   audio only.** (Pragmatic shortcut: run `re_mp4` for the video track and `symphonia` for the audio track
   over the **same buffer**. Two parsers, one byte range. Zero integration risk; a few ms of redundant CPU.)
2. **`re_video` shells out to an `ffmpeg` *binary*** (`ffmpeg-sidecar`). A licensing dodge, not an
   architecture. Take `re_mp4` and `re_rav1d` **directly**.
3. **`rav1d` upstream has no Rust API** — it is a C-ABI drop-in for libdav1d. **`re_rav1d`** is the fork
   with an actual API.
4. **`mp4parse` (Firefox's) is a box parser, not a sample reader.** The demuxer that uses it is C++ in Gecko.
5. **`servo-media` drags all of GStreamer** — worse packaging than ffmpeg. Reference-only.
6. **`oxideav-vp9`, `rust_h265`** — incremental non-decoders. Zero capability.
7. **VP9 is the codec we leave on the floor.** No usable Rust decoder, stale C bindings, maintenance mode at
   Google. Say `canPlayType('video/webm; codecs="vp9"') === ""` **and mean it** — `cros-codecs` can do it in
   hardware later.
8. **ABR is downstream of MSE, which is downstream of decode.** Parsing an `.m3u8` before MSE exists unlocks
   approximately zero sites. `hls.js`/Shaka are JavaScript and run **on MSE**.
9. **A/V sync is hand-rolled** (~150–250 lines) and no crate does it. Audio device clock is master; wall
   clock when there is no audio track — **which is the majority of web `<video>`.** Take `cpal`, not
   `rodio`: `rodio`'s `Sink` *hides the clock*, and the clock is the thing we need.

## The one-line answer

**Ticks 1+2 — `re_mp4` + `openh264` + `yuvutils-rs`, decoding into the `DecodedImage` slot the poster
already occupies — is ~2–3 days and lands muted looping `<video>` playback, which is most of the `<video>`
elements on the open web.** Everything after tick 7 is a real project; everything before it is a week.
