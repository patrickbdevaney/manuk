# MEDIA PIPELINE — MSE, the attachment handshake, and the decode registry

> The watch-the-web track (M1…M7). This file records the mechanisms that are easy to get subtly
> wrong and expensive to rediscover. M1 (the MSE byte pipe) is landed; M3–M5 (demux/decode) are not.

## Adaptive streaming never touches `<video src="file.mp4">`

Every site that matters for watching — YouTube, Netflix, Twitch, Vimeo — and every player library
(hls.js, dash.js, shaka, video.js) does the same thing instead: construct a `MediaSource`, hand the
element a `blob:` URL for it, wait for `sourceopen`, `addSourceBuffer(mime)`, then `appendBuffer()`
media segments fetched over XHR in a loop clocked by `updateend`.

The consequence for an engine that lacks MSE is worse than "video does not play". Those players run
`new MediaSource()` inside a capability probe at **module-evaluation** time, so a missing name is a
`ReferenceError` that kills the player script before it renders a single control — and takes the
surrounding bundle's evaluation with it. A player that cannot construct its source object cannot
fall back to progressive download either. It just stops.

## The attachment handshake is the ONLY channel from element to MediaSource

`video.src = URL.createObjectURL(mediaSource)` is the single moment the element learns which
`MediaSource` it is playing. There is nothing else — no attribute, no registry lookup by name.

That makes `URL.createObjectURL` load-bearing rather than a convenience: it registers the object
against the returned `blob:` id, and the `src` **setter** must intercept the assignment, resolve the
id back to the object, and flip the source `closed` → `open`. Miss the interception and the object
URL is stored as an ordinary attribute string, the source stays `closed`, `sourceopen` never fires,
and the player waits on that event **forever** — a hang with nothing wrong in the DOM to see. That
is exactly the failure `g_mse`'s `syncopen:` claim is positioned to catch.

Two details that bite:

- **Revocation must not detach.** Players call `URL.revokeObjectURL(url)` immediately after
  assigning `src`. Once attached, the element holds the reference; revoking only removes the id from
  the registry. Tearing the stream down there kills it at the instant it starts.
- **Look the reflected `src` accessor up lazily, on the prototype chain, at call time.** Reflection
  installs `src` on the prototype and the interception is installed per element; resolving it
  eagerly bakes in an install-order assumption that is not ours to rely on.

`srcObject = mediaSource` is the newer form of the same handshake with no object URL in between, and
needs its own interception.

## The decode registry: a `false` is worth more than an unbacked `true`

`MediaSource.isTypeSupported()` answers from `__mseCodecs`, the registry of what something
downstream can **genuinely** decode. It is empty until a real decoder lands, so every answer is
`false` and every player takes its documented fallback path.

This is the opposite of the instinct to stub `true`. A `true` that is not backed by a decoder steers
the player onto the adaptive path, where it appends segments and then polls `buffered` for a range
that can never grow — hanging forever, with the failure surfacing far from its cause. The honest
`false` fails immediately, legibly, and in a branch the player already handles.

The registry is the hand-off seam for the rest of the track: M3 (demux) and M4/M5 (AAC / VP9 decode)
push the types they land, and `isTypeSupported` starts saying yes for exactly what can be played,
with no change to any of the surrounding machinery. `g_mse` asserts the `false` **first** and then
registers a codec and asserts the answer flips — which is what proves the honesty is a seam and not
a hardcoded constant.

## `buffered` is a media timeline, not a byte count

Bytes appended are held in order (that queue is what a demuxer will read), but nothing is demuxed,
so no media timeline exists and `buffered` is honestly empty. Reporting a fabricated range from
byte counts would be the same unbacked-`true` mistake in a different place.

## The append sequence is a task sequence, and that is load-bearing

`updatestart` fires synchronously inside `appendBuffer`; `update` and `updateend` fire on a **later
task** (`setTimeout`, not a microtask). A player that re-enters `appendBuffer` from its own
`updateend` must find `updating` already `false` and the previous task fully unwound — a microtask
would not guarantee that. Getting this wrong makes the steady-state fill loop throw on its second
segment.

The spec's `InvalidStateError` on a re-entrant `appendBuffer` (and on `endOfStream()` while a buffer
is updating) exists so the player can queue instead of corrupting the stream; players branch on that
exact error name.

## An `updateend` handler that appends is re-entered by its own append

Worth stating separately, because it cost a tick. A listener that appends a segment is re-invoked by
the very append it just made — an unbounded append/timer chain that never lets the event loop drain.
The page does not fail; it **hangs**, at full CPU, indistinguishable from a slow build. `g_mse`
therefore drives its sequence from one listener dispatching on a step counter, never from a listener
that registers another listener.

This is a real player's fill-loop shape, so the same hazard exists in page content. Note the engine
consequence: a runaway timer chain in page script currently spins `Page::load` without a bound.

## Gates: flush the record on every push, not at the end

`g_mse` writes its result to the DOM on **every** push. The sequence is asynchronous and only
completes on `sourceended`, so a single end-of-run write means any earlier break — source never
opened, append threw, listener never fired — reports the identical empty `-`, naming no claim and
pointing at no mechanism. Per-push flushing makes the last recorded claim the failure's location.
This was not hypothetical: the first RED probe reported `got: -` and proved nothing until fixed.

## M2: the segment corruption at the fetch boundary (found t227, FIXED t228)

Tick 223 built the pipe a player appends into. What it appends comes from an `XHR`/`fetch` with
`responseType = 'arraybuffer'`, usually over a byte `Range`. That path was measured with a 260-byte
probe segment — a real EBML magic followed by all 256 byte values — and it was **broken**:

```
sent 260 bytes  →  received 407.   magic:false   allbytes:differs@0=194
```

**It is not truncation, and it is not U+FFFD replacement** (which is what the probe was originally
written to catch, and what the codebase's earlier lossy-storage bugs looked like). It is UTF-8
**inflation**. The response body crosses the boundary as a Rust `&str`
(`Page::resolve_fetch(id, status, body: &str, …)`), so every byte above `0x7F` is carried as a
codepoint and re-encoded as two bytes on the way back out: `0xDF` → `0xC3 0x9F`, and the `194` in
`differs@0` is `0xC2`, that lead byte.

**Why it hid.** Every byte below `0x80` survives perfectly. JSON, HTML, SSE, form bodies — everything
the fetch path has been used for so far — round-trips exactly. Only binary is destroyed, and the
first binary consumer is the media track.

**Why it was a hard blocker for M3.** `appendBuffer` accepts the segment (it accepts any bytes), and
the demuxer then rejects a stream that was valid when it left the server. The symptom appears in the
demuxer, so it reads as a codec bug — but no amount of work on symphonia fixes a corrupted input.
Demux could not be started until it was fixed.

**The fix was a transport representation, not a parser.** Carry the body as a **binary string** — one
code unit per byte, `charCode & 0xFF` — which is the convention this codebase already uses on the
WebSocket path, and move the UTF-8 decode into `.text()`/`.json()`, where it belongs. That is
correct rather than merely expedient: a `Response` has one body, and *the page* decides whether it is
text or bytes; deciding for it at the boundary is what caused this. It touches `Page::resolve_fetch`,
the shell's `pump_fetches` and the prelude's body accessors together, which is why it is its own
tick.

**What landed (t228).** The body now crosses on **two channels**: the host's charset-decoded text
for `.text()`/`.json()`, and the raw bytes as a one-code-unit-per-byte binary string for
`.arrayBuffer()`/`.bytes()`/`.body` and an `arraybuffer` XHR. Neither derives from the other without
loss — re-encoding the text inflates, and decoding the bytes as UTF-8 in JS would throw away the
charset sniffing that makes a legacy-encoded page readable. `Page::resolve_fetch_bytes` is the entry
point a host with real wire bytes should use; the old `resolve_fetch(&str)` still means "this body IS
text" and remains exactly correct for that.

**The streaming path was never affected**, and that is the tell: `deliver_chunk` already used
`js_bytes_literal`, the one-char-per-byte convention. The buffered path being the odd one out is
precisely how this survived — the fix reuses that same helper rather than inventing a second
encoding.

### What DOES work, and is now pinned

Byte-range requests are real: the page's `Range: bytes=4-11` reaches the wire, the server's `206`
surfaces instead of being flattened to `200`, and the requested bytes come back. Segmented delivery —
the other half of adaptive streaming — is not the problem.

---

## M3 — container demux (tick 234): the engine can open a media file

`engine/media` (`manuk-media`) is the demuxer. It is the first step of the media chain that reads
the bytes rather than moving them.

**Why it is demux and not decode.** The MSE pipe was complete and inert: a page could construct a
`MediaSource`, attach it, fetch a segment byte-exactly (t227/t228) and `appendBuffer` it, and
`sb.buffered.length` was `0` because nothing had ever looked at the bytes. That zero is not
cosmetic — **`buffered` is the variable an adaptive player's fetch loop steers by.** It appends,
reads how far the buffer now reaches, and decides what to request next. A `buffered` that never
advances is a loop that never advances, so a perfect byte pipe still gets no site past its first
segment. Demux is what turns the pipe into a loop, and it needs no codec.

**The borrow.** `re_mp4` (Rerun's fork of `mp4`), per the MEDIA.md trap-list: `symphonia` leaves its
ISO-MP4 video `SampleEntry` commented out (audio only), `mp4parse` is a box parser with no sample
reader, and `re_video` shells out to an ffmpeg *binary*. `re_mp4` walks `moof`/`traf`/`trun`, which
is the fragmented form MSE actually streams.

**What the crate produces:** tracks (kind, RFC 6381 `codecs=` string, dimensions, channels/sample
rate, and the `avcC`/`av1C`/`vpcC`/AAC decoder-configuration record extracted for M4/M5), a sample
table (byte range, decode + presentation timestamps, duration, sync flag), and contiguous
presentation-time ranges. `SourceBuffer.__demux` calls it through `__mseDemux`, a global native that
takes the accumulated stream as a one-char-per-byte string and answers in JSON.

### Three things this cost, each worth keeping

**1. A borrowed parser inverted every fragmented sync flag.** `re_mp4`'s `reader.rs:443` computes
`is_sync: (sample_flags >> 16) & 0x1 != 0`, but bit 16 of a `trun` sample-flags word is
`sample_is_NON_sync_sample` — the negation, and the negation was missing. The progressive path is
fine; it reads the `stss` table, which is a positive list.

It was found by **differential test, not by reading the source** — the source looks right until you
check which flag bit 16 is. Chromium ships three fixtures differing only in their sync flags, and
`re_mp4` returned the exact complement for all three. A seek must land on a sync sample, so inverted,
every seek into a fragmented stream lands on a frame that cannot decode standalone: a garbage frame
or a silent stall, nothing thrown. It would have surfaced much later as "our H.264 decoder is
broken", one layer below where the bug is. Corrected per sample by origin (`stbl` count vs `trun`),
not per file, so a file with both is handled rather than assumed away.

**2. `buffered` does not start at zero, and normalising it would be a bug.** The fixture carries a
two-frame composition offset: its samples decode at 0/1001 and present at 2002/4004. That is
ordinary B-frame reorder delay, and `buffered` is a *presentation* timeline. The gate asserted
`start == 0` first, from assumption rather than measurement, and "fixing" it would have meant
discarding a real timestamp — in MSE that offset is how a segment appended at minute three reports
minute three.

**3. The gap tolerance is not a fudge, it is the difference between a loop and a stall.** Those two
frames present one frame apart, leaving a genuine 33ms interior hole. Reported literally that is two
ranges, and a player reading `buffered.length === 2` across 33ms concludes its download failed and
re-fetches what it already has. Merging under a 100ms tolerance is what every shipping
implementation does, for exactly this reason.

### What is deliberately still missing

- **No decoder. No frame is produced.** `isTypeSupported` still answers from the empty `__mseCodecs`
  registry and still says **no**. Knowing where the H.264 is and being able to decode it are
  different claims, and `g_media_buffered` asserts the honest `false` so this landing cannot start
  over-promising. Advertising MSE we cannot honour turns a working YouTube into a black rectangle.
- **WebM/Matroska is not demuxed.** `sniff` recognises EBML and returns a named `Unsupported`, so the
  failure is "this is WebM and we only demux MP4" rather than a parse error blaming the bytes.
- **The demuxer re-parses the accumulated buffer per append**, rather than being incremental. The
  `SourceBuffer` retains every chunk anyway (eviction is its own spec'd algorithm), and an
  incremental parser buys latency there is no decoder downstream to spend yet.

**Gates:** `engine/media/tests/demux.rs` (real fixtures, both container forms, the differential
sync-flag test) and `engine/page/tests/g_media_buffered.rs` (the JS-observable surface: a real fMP4
over a real socket, appended, read back through `sb.buffered` / `sb.videoTracks`).

**Next:** M4 — AAC decode via `symphonia` (audio only, per the trap-list) plus `cpal` for output,
and M5 video decode. Both consume the decoder-configuration record this step already extracts.

## M4 — AAC decode (tick 235): sound-shaped numbers, not yet sound

`engine/media/src/audio.rs`. M3 could find the audio and name it (`mp4a.67`, 44100 Hz, stereo) and
could not produce one sample of it. Naming a codec and decoding it are different claims.

**Borrowed:** `symphonia`, pulled in as `default-features = false, features = ["aac"]` — deliberately
narrow. Symphonia ships a dozen demuxers we must not silently acquire, and its ISO-MP4 demuxer is
**audio-only** (MEDIA.md trap #1), so demux stays re_mp4's job and symphonia's role is confined to
turning AAC packets into PCM. Two parsers with overlapping jobs is how they drift.

**The AudioSpecificConfig had to be rebuilt, not sliced.** An AAC decoder cannot interpret a single
packet without it, and `re_mp4` parses the `esds` descriptor into fields without retaining the
original bytes — so there is nothing to slice. It is re-encoded from the parsed values:
five bits of audio object type, four of sampling-frequency index, four of channel configuration,
three zero flag bits (`AAAAA FFFF CCCC 000`). AAC-LC at 44100 stereo is `0x12 0x10`.

**The assertion that makes it a decode gate.** The decoded PCM frame count must equal the track's
declared duration **in its own timescale** — 121856 units at 44100 is 121856 frames, exactly. Those
numbers come from independent places: the duration from the container headers, the frame count from
summing what the decoder emitted packet by packet. A decoder that dropped packets, doubled a buffer,
mis-read the channel count or returned early lands somewhere else. The gate also asserts a non-zero
peak, because correctly-sized **silence** satisfies every length check.

**Codec-string subtlety worth keeping:** this file is `mp4a.67`, not `mp4a.40.2`. Object type
indication 0x67 is MPEG-2 AAC-LC; `0x40` is the MPEG-4 spelling and takes the `.2` audio-object-type
suffix. Players string-compare these, so reporting one for the other is a rejection.

**Still not playback.** There is no audio device — `cpal` is a separate step, deliberately, because a
device is not headlessly gateable and bundling it would mean the decode could only be proven by
listening. `isTypeSupported` is unchanged and still answers `false`: audio decodes, video does not,
and a stream needs both. Non-AAC audio (MP3, Opus, Vorbis, FLAC, AC-3) is refused up front by name
rather than accepted and failed mid-stream.

**Gate:** `engine/media/tests/audio_decode.rs`. **RED, run:** decoding only the first packet yields
1024 frames against 121856.

**Next:** M5 video decode, then `cpal` output and A/V sync (audio device clock is master; wall clock
when there is no audio track — which is the majority of web `<video>`).

## M5 — H.264 decode: the first real frame

`engine/media/src/video.rs`. Demux names the video track; this turns its samples into pixels.

### The trait exists because the backend is known to be temporary

`trait VideoDecoder` is defined with exactly one implementation, deliberately. `openh264` (Cisco,
BSD-2) decodes **Constrained Baseline only** — B-frame reordering is unimplemented — while the open
web's H.264 is overwhelmingly **High** profile, `libx264`'s default. Firefox makes the same call:
OpenH264 for WebRTC, never for `<video>`.

So this backend cannot play most video on the web, and the value of the step is not that it can. It
is that YouTube's no-MSE fallback is `avc1.42001E` (Baseline, 360p), which decodes with `cargo
build` and **zero system dependencies** — and that the VA-API/ffmpeg backend for High profile drops
in behind the same trait later without a caller changing. Retrofitting that boundary after callers
exist costs multiples of writing it on day one.

### The format mismatch that is the actual work

**MP4 does not store H.264 the way a decoder eats it**, and both halves of the mismatch fail
silently:

1. **AVCC vs Annex-B.** In MP4 each NAL unit carries a big-endian *length prefix*; decoders expect
   `00 00 00 01` *start codes*. Hand a decoder the raw sample and the length parses as a garbage NAL
   header — no frame, no useful error. The prefix width is 1, 2 or 4 bytes and is recorded in `avcC`
   (`avcc[4] & 0b11`, plus one); it is **read, never assumed to be 4**, because assuming 4 against a
   2-byte stream desynchronises on the first NAL.
2. **The SPS/PPS are not in the samples.** They live once, out of band, in the `avcC` record. A
   decoder given only coded frames has never been told the resolution, profile or reference layout,
   so it discards everything until it is. They are converted to Annex-B and prepended to the first
   sample.

### `isTypeSupported` stays honest

`can_decode` parses the profile out of the codec string (`avc1.PPCCLL`, `PP` = `profile_idc`) and
accepts **only 66**. Answering `true` for High would be accepted up front and fail mid-stream, which
is strictly worse for a player than an honest `false` — it has a fallback, and a refusal is how it
gets to use it.

### What makes the gate a decode gate rather than a did-it-run gate

`engine/media/tests/video_decode.rs`, and all three failure modes were **executed, not asserted**:

| RED probe | result |
|---|---|
| `parameter_sets: None` (drop SPS/PPS) | first-frame test FAILED |
| feed raw AVCC, skip the Annex-B rewrite | first-frame test FAILED |
| widen `can_decode` to any `avc1.*` | High-profile refusal test FAILED |

The load-bearing assertion is **non-uniformity**. Dimensions come from two independent sources
(container header vs the decoder's own SPS read), but a correctly-sized *flat green field* passes
every size check ever written — and a flat field is exactly what a mis-fed decoder produces. So the
gate asserts the frame is not a single repeated pixel.

### Isolation, proven in both directions

`video` is opt-in (`default = []`) for the reason `audio` is, plus one more: **openh264 compiles C**,
so a default-on decoder would put a `cc` invocation into every configuration that builds this crate
— including all ~25 gate binaries reached via `manuk-js -> manuk-media`. `cargo tree` finds **0**
openh264 in `manuk-shell` (default *and* `--no-default-features`), in `manuk-js`, and in
`manuk-media` default — and **2** under `--features video`, so the probe can see it when present.
A guard that is never observed to detect anything is not a guard.

### Fixture note, and the pin

The Baseline fixture was minted with the **system ffmpeg binary as a dev tool**. That does not
violate the no-ffmpeg rule, which forbids *linking* ffmpeg into the browser, not using it to author
a test file. Both pre-existing video fixtures are High profile, so tick M5 would otherwise have
failed against its own input — a failure that reads exactly like a wiring bug.

`openh264` is pinned to `=0.9.0`: 0.9.1+ pulls `safe_arch 1.0`/`wide 1.5`, which require rustc 1.89
against this workspace's 1.88, and the resolution error names two SIMD crates without ever
mentioning H.264. Unpin when the toolchain moves.

**Residue:** High profile (VA-API/`cros-codecs`); a decode thread + wall clock for actual playback
(M6); A/V sync against the M4 PCM; AV1 via `re_rav1d`; WebM containers.

## The frame reaches the screen by overwriting the poster's map entry (tick 240)

**There is no video path in the painter, and there must not be one.** The chain that displays a
decoded frame is the chain that already displayed the poster:

```
manuk_media::decode_first_frame(track, &bytes) -> video::Frame { width, height, rgba, .. }
                    │  (host-side; openh264 lives ONLY in manuk-media)
                    ▼
Page::set_video_frame(node, w, h, rgba)   ── overwrites Page::images[node]
                    ▼
CpuPainter::with_layers(.., &self.images, ..) -> manuk_paint::blit_image  ── scales into the content box
```

`Page::images` is `HashMap<NodeId, Rc<DecodedImage>>`, and `DecodedImage { width, height, rgba }` is
`video::Frame` minus its presentation time. A `<video>` is a replaced element
(`manuk_layout::is_replaced_element`), so its box already exists and already gets an image blitted
into it — that is how `<video poster>` renders. **Playing a video is swapping the `Rc` in the map the
poster already occupies**, exactly as MEDIA.md predicted, and it is why the whole media track is
sized in days rather than months.

**The seam takes RGBA, not a media type, and this is a load-bearing decision.** Naming
`manuk_media::video::Frame` in `manuk-page`'s signature would pull `manuk-media`'s decoder features
into `manuk-page`; `openh264` compiles C, and ~25 gate binaries link `manuk-page`. Tick 236 spent
real effort proving that isolation both directions with `cargo tree`. Bytes keep the page
decoder-agnostic — openh264 today, `re_rav1d` or a VA-API backend later, same signature. Same
principle as tick 236's `trait VideoDecoder`: **the boundary is the deliverable, not the backend.**

**A frame must never resize its box.** Unlike `<img>`, a `<video>` is sized by attributes/CSS. Deriving
the box from the current frame reflows the page on frame one and again on every resolution switch — and
switching resolution mid-stream is what adaptive streaming *is*. `set_video_frame` therefore does not
call `apply_natural_size` and does not relayout; `g_video_frame` asserts a 5×-wider frame moves the box
by less than half a pixel.

### The drift this exposed: two decode passes, one of them half-blind

`fetch_images_owned` (async) selected `<img src>` **and** `<video poster>`. `decode_inline_images`
(synchronous, pre-first-layout) selected `<img>` only. So an inline `data:` poster never decoded on
`Page::load`, in any gate, or in the WPT runner, while a network poster did — a divergence between two
functions that exist to do the same job on different transports. The fix is for the inline pass to
choose its source attribute exactly as the async pass does.

**It was found by a gate asserting its own BASELINE.** `g_video_frame` checks the poster paints
*before* handing over a frame, because "the poster's red is gone" is vacuous if nothing was ever
painted. That assertion failed first, which is the argument for asserting the ground you are about to
build on instead of assuming it — the same shape as tick 237's four-quadrant fix, where a weaker claim
had passed on a completely broken draw.

## M6 — the presentation clock (tick 249): a still is not playback

M5 ends with `decode_first_frame` — **one picture**. What separates that from a video is an organ
MEDIA.md's trap list (#9) says no crate provides: something that answers *which frame is now*.
`manuk_media::playback` is that organ, and it is the one step in this track with **no dependency** —
deliberately. A container parser and a codec are large, adversarial and specified, so they are always
borrowed. A presentation clock is small and entirely policy, so hand-rolling ~200 lines is correct
here and would be a mistake one layer down.

### HOLD, never ROUND — the whole correctness of the module

A video **holds** each frame until the next is due, so `frame_at(t)` is the **last** frame with
`presentation_time <= t`, never the *nearest*. The distinction is invisible on every frame boundary
and wrong everywhere between: at 30fps a nearest-frame lookup switches to frame N+1 at 16.7ms, so it
shows a picture the author has not reached yet for **the entire back half of every frame interval**.

This is a trap for the test, not just the implementation. Sampling the timeline at frame timestamps —
the obvious way to write it — passes under *both* implementations. `g_playback_clock` therefore
samples deliberately **between** frames (75% through interval 0), and the RED probe confirmed only
that one assertion flips when `partition_point` is swapped for a nearest-by-distance scan.

### Presentation order is written before it can matter

Frames are sorted by presentation time even though openh264 is Constrained Baseline and emits no
B-frames, so the sort is a **no-op on everything currently decodable**. That is the reason to write
it now: the moment a High-profile backend drops in behind `VideoDecoder`, decode order stops being
presentation order and an index built in decode order plays the picture sequence scrambled — a bug
that would present as "the video is glitchy", far from the line that caused it.

### The threshold that was guessed wrong, and what the real numbers were

The load-bearing assertion is that consecutive frames are **different pictures** — a decoder
re-emitting one frame yields a timeline of the right length that plays a still, and passes every
count, duration, ordering and dimension check ever written.

The first bar was invented — *"more than 1% of bytes must differ"* — and it **failed on real, correct
video**. Measured on the fixture: pair 0→1 differs in **60.4%** of bytes, pair 1→2 in **0.86%**. 33ms
apart in a slow-panning scene is genuinely a tiny delta. The failure mode actually being caught
produces **exactly zero** differing bytes (confirmed by RED probe: `0 of 921600`), so the honest bar
is a floor far above zero and far below real motion — 0.1%, which the tighter pair clears 8×.
**A threshold picked from intuition about what "different" means was wrong by an order of magnitude
in the direction that false-fails good work.** Calibrate against the data, then write the measurement
into the gate so the next reader does not re-guess it.

### The clock does not own the frames

`Transport` (position/playing/ended) is separate from `FrameTimeline` because MEDIA.md's A/V-sync
rule is that the **audio device clock is master** — a dropped video frame is invisible, a stretched
audio sample is not. Advancing by a wall-clock delta is the *fallback* for muted/video-only; the
position must remain settable from outside for audio to drive it. Keeping position out of the frame
store is what leaves that door open.

Residue: this is the clock, not the element. `<video>`'s JS surface still answers the **honest NO**
from the pre-decode era — `play()` rejects and `canPlayType` returns `''` even for `avc1.42001E`,
which we can now decode. That NO has become a lie in the other direction, and correcting it (element
→ timeline → `set_video_frame`) is M6b. Audio and video are also not yet sync'd to one clock.

## M6b — audio is master (tick 250), and two numbers that came out of it

Tick 249's clock advances by a wall-clock delta. MEDIA.md (trap #9) names that the **fallback** —
right for the muted/video-only case that is most of the open web's `<video>`, wrong the moment there
is an audio track, where the **audio device clock is master**. The reason is asymmetric cost: a
dropped video frame is invisible, a stretched audio sample is not. (It is also why `cpal` was taken
over `rodio`: `rodio`'s `Sink` hides the clock, and the clock is the thing needed.)

### The master clock holds an integer, and the drift is 0.53s/hour

An audio device reports **a count of sample frames consumed**. Position is that count divided by the
sample rate — an exact rational, one division on read. The naive alternative keeps an `f64 position`
and adds `frames / sample_rate` per callback; `1024/44100` is not representable in binary floating
point, the error is one-directional, and at ~43 callbacks/second it never cancels.

**Measured, by RED probe rather than by argument:** over ~1 hour of callbacks the accumulating clock
lands on 158,696,652 sample frames where the exact one has 158,720,000 — **23,348 frames lost, 0.53
seconds of lip-sync drift.** Every short-horizon assertion stayed green under the broken clock. This
is the archetype of a bug no quick test catches: it is correct for the first minute and the
complaint it eventually generates ("audio drifts out on long videos") is unreproducible by whoever
receives it.

### Sync SNAPS; it does not blend

`sync_to_audio` assigns the audio position. Averaging the two clocks, or taking whichever is further
along, leaves the video clock authoritative *in part* — and any video contribution to the position
eventually has to be paid back by resampling audio, the one correction a listener can hear. The RED
probe (average instead of snap) broke **two independent assertions**, which is the signal that the
claim is load-bearing rather than decorative.

The correction always lands on the video side, and that is enforced by the **type**: `sync_to_audio`
takes the clock by `&`, so the sync path *cannot* write it. Taking it by `&mut` and nudging
`frames_played` toward the transport would compile and would appear to fix drift.

### 44100 and 30000 are incommensurate — an assertion written wrong first

The obvious claim is "submit one frame interval of audio and frame 1 is on screen." **It is false,
and it failed.** One 30000-timescale frame interval is 1471.47 audio samples; a device delivers whole
samples; 1471 samples lands **0.47 samples short** of frame 1's presentation time, so frame 0 is
correctly still held.

The generalisation is worth more than the fix: **an audio clock can never be assumed to land on a
video frame boundary**, so any sync policy phrased as "when the clocks are equal" fires rarely and by
luck. Hold semantics (M6, above) are exactly what make the incommensurability harmless — the two
halves of this step are load-bearing for each other.

### One stream ends before the other

The fixture's audio (0.0929s) is shorter than its video (0.1001s), deliberately. Audio ending is not
the media ending: the transport keeps playing and the wall-clock fallback carries the remaining
video to its end. A player that treats the master clock stopping as the media stopping truncates
every file whose tracks are not exactly equal in length — which is most of them.

Residue: still no device. `AudioClock` is fed by whoever owns the `cpal` stream, and nothing owns one
yet — decoded PCM correctness is gateable headlessly, audible playback is not. And the `<video>`
element's JS surface still answers the pre-decode era's honest NO (M6b-element, unbuilt).

## M7 — captions (tick 255), and a probe that verified the TEST

`VttTrack::parse` + `active_at(t)`, in `manuk_media::vtt`. **Not feature-gated** — a caption file is
text, so this is the one part of the media stack needing no decoder, which is why it lands while the
`<video>` element wiring does not.

### `active_at` returns a LIST

Cues overlap: two speakers captioned at once, a speaker label held across lines, a translation over
an on-screen sign. So "what is showing at `t`" is a *set*. `Option<&Cue>` compiles, reads as
reasonable, and silently drops the second speaker for the whole overlap — the same shape as tick
254's `selectedOptions`, the plural question answered in the singular, where the wrong answer looks
valid rather than erroring.

### The failure mode of a strict parser is SILENCE, not rejection

Hours are optional (`00:01.500 --> 00:04.000` is the common form). A parser demanding `HH:MM:SS` was
predicted to "reject real files". What it actually does — measured — is return a track with **zero
cues and no error**, because a malformed timestamp skips its own cue rather than failing the file.
The video plays with no captions and nothing is logged. **Leniency is not sloppiness here: strictness
converts a partial success into a silent total failure.**

### The probe that stayed GREEN, and why it is the best result of the tick

Disabling the `NOTE` comment branch **did not fail the gate**. The fixture's NOTE was ordinary prose,
so without the branch it fell through to the generic "neither line is a timing line, skip it" path
and produced the right answer for the wrong reason. Two code paths, one test, and the test could not
distinguish them — the `NOTE` assertion was **vacuous**.

The fix is a fixture whose NOTE body *contains a timestamp line*, the only shape that separates the
paths. It now goes red with 6 cues, printing the translator's private remark over the video.

**This is the argument for process rule 3 in its strongest form.** The RED recipe was written
confidently in the module header and was simply wrong. Running it did not verify the fix — **it
verified the test**, and that direction only ever shows up if you execute the probe instead of
asserting it. A gate can be green, well-commented, and measuring nothing.

Residue: the parser, not the pipeline. Nothing fetches `<track src>`, `textTracks` is still the `[]`
stub, no cue is painted. Cue settings are discarded (no positioning), inline markup (`<v Alice>`,
`<i>`) stays literal, and regions/STYLE/chapters are skipped.

## M7b — the TextTrack API (tick 256), and the track nobody turned on

Tick 255 built the parser. This is the API a page reaches captions through, and it was an inert
object: `addTextTrack()` returned `{cues: [], activeCues: [], mode: 'disabled'}`, accepting every
call, reporting success and holding nothing. A player added 900 cues to it and rendered none.

**Why this surface rather than `<track src>`:** hls.js and dash.js ship their own WebVTT parsers and
call `addTextTrack` + `addCue`, because segmented streams carry captions *inside the media segments*.
On the streaming sites the media track is aimed at, `<track>` is not the path captions take.

### `mode` is how "captions off" is represented

A `TextTrack` defaults to `'disabled'`, and a disabled track has **no `activeCues`**. Every player
sets `mode = 'showing'` as a deliberate separate step for exactly this reason. Serving cues
regardless of mode renders subtitles for a user who turned them off — confirmed by probe
(`mode=disabled active=2`), which is a feature working *too hard* rather than failing, and therefore
the kind of bug that gets reported as "why are there subtitles".

### The same plural lesson, one layer up

`activeCues` is a list for the reason `VttTrack::active_at` is: cues overlap. The singular probe drops
the second speaker for the whole overlap, identically to tick 255. Two implementations of one idea,
in two languages, with the same failure mode available in both — worth noticing that the shape
travels rather than the code.

Residue: **the two halves of captions are built and not connected.** Nothing fetches `<track src>`,
and `TextTrack` does not reach the tick-255 parser — a page must bring its own cues today. No
`cuechange` event, so a listener-based player sees nothing; cue settings are inert; no cue is
painted.

## M7c — the caption timeline fires (tick 257), and the poll nobody was making

Tick 255 built the WebVTT parser; tick 256 built the `TextTrack` that holds cues and answers
`activeCues`. Both were correct, and both were **poll-only**. Nothing polls.

Every caption renderer that exists — the players' own overlays, and the `<track>` UI — is
`track.addEventListener('cuechange', render)`. So a track that computes exactly the right active
cues and never fires is a track whose captions are **computed and never shown**: the same failure
shape as the inert object tick 256 replaced, one layer further along. This is the third time in this
module the bug has been *"the value is right and nobody is told"*, and it is worth naming as a class:
a correct getter is not a capability until something delivers it.

### `currentTime` is not a number, it is the clock

The tick's structural move is that `currentTime` stopped being a plain data property. Storing the
number and telling nobody is precisely what made `cuechange` unreachable — the only thing that knows
a caption boundary was crossed is the write that moved the clock past it. It is now an accessor whose
setter recomputes every track's active set. `mode` became an accessor for the same reason: turning
captions **on** is a state change, and with a long cue already under the playhead the renderer has no
other moment to learn about it.

### The comparison is by IDENTITY, and length is the trap

`cuechange` must fire on *change*, not on every write — a player writes `currentTime` every frame,
and a listener that redraws its caption node each time is a DOM write per frame for a line of text
that did not change. So the sync diffs the new active set against the last one. The tempting diff is
`a.length !== b.length`, and it is wrong in the **most common** case: seeking from one single-cue
line straight to another (a click on the transcript) leaves both sets at length 1, scores as
no-change, and the viewer sits on the previous caption. The diff is element identity, position by
position.

### Three RED probes, three distinct predicted bugs (process rule 3)

The gate was green on the first run, so it was made to fail three ways:

| probe | result | what it is in a real player |
|---|---|---|
| compare by length only | `seek=1:C` (expected `seek=2:A`) | seek lands, the caption never updates |
| fire unconditionally from the setter | `off=3` and `same=7` | fires on a **disabled** track, i.e. renders subtitles the user turned off; and an event storm |
| `mode` back to a data property | `on=0:-` and `offagain=3:A+LIVE` | captions turned on stay blank until the next boundary; captions turned off stay **burned on screen** |

Gate: `engine/page/tests/g_cue_change.rs` (`G_CUE_CHANGE`).

### Residue, named honestly

`<track src>` is still not fetched, so `TextTrack` still does not reach the parser landed at tick 255
— the two halves of captions remain unconnected, and connecting them needs the network path, not
another prelude tick. Cue settings on `VTTCue` are still accepted and inert, so nothing *positions* a
caption, and no cue is painted anywhere: `cuechange` now tells a page's own renderer what to draw,
but the UA draws nothing itself. `enter`/`exit` events on individual cues are absent.

## M7d — the two halves of captions are joined (tick 258b/259), and a limit found by measuring

Ticks 255–257 built three correct pieces and left **no path between them**. The WebVTT parser had no
caller outside its own unit tests. The `TextTrack` API could only ever hold cues the page's own
JavaScript constructed with `new VTTCue`. That covers hls.js and dash.js — and nothing else, while a
news clip, a course video and a documentation screencast all ship
`<track kind=subtitles src="subs.vtt" default>` and expect the *browser* to load it.

`__parseVtt(text)` is the join: a host function in `manuk-js` (which already depends on both
`manuk-media` and the DOM) returning the parsed cues as JSON. It lives there rather than in
`manuk-page` because `manuk-page` does not depend on `manuk-media`, and adding that crate edge to
fetch one file is heavier coupling than the boundary `manuk-js` already owns. The fetch goes through
the page's own `fetch()`, so it inherits base-URL resolution and the host pump rather than growing a
second network path.

### The sweep is document-driven, and that is not a detail

The obvious hook is `__manukMedia`, which installs the media surface when an element is **reflected**
— i.e. when the page's JS touches it. That is exactly backwards for this feature: a page that ships
`<track>` typically never mentions the video again. So the load is swept from the document in
`run_deferred_scripts`, after the scripts run and before the drain, so the fetches it starts are
pumped by the same pass. `__loadTracks` marks each `<track>` `__loading`, so re-sweeping later rounds
picks up tracks added since without re-fetching.

### The limit this tick MEASURED and did not remove

A document with **no `<script>` at all never gets a JS context.** This was measured, not assumed: a
probe page without a script could not evaluate a single expression, and adding one line of JS made
every piece appear at once. So the honest claim is that `<track src>` loads on any page running
*some* JavaScript — essentially every real video page — and does not load on a fully static one.
That is a context-creation policy question, not a caption question, and creating a JS realm for every
static document to service a `<track>` trades a large universal cost for a narrow case. Written down,
not papered over.

### Two RED probes came back GREEN, and the claim was narrowed rather than the probe discarded

Making the parse failure `throw`, and separately deleting the `.catch(fail)`, both left the gate
passing — the paths are equivalent because the throw lands in the same rejection handler. So the gate
does **not** measure "reports rather than throws", however reasonable that sentence sounds. It
measures the ERROR state and the absence of cues, which stayed falsifiable: dropping the `!res.ok`
branch leaves the track in `readyState` 1, LOADING forever, which is what a page's captions button
waits on. This is the second time in this module a probe has stayed green; the rule that catches it is
that a probe which cannot fail measured nothing.

The probes that DID go red: removing the document sweep (nothing is ever fetched), and ignoring
`default` (`mode=disabled`, `fires=0` — every other assertion still passes while the feature renders
nothing).

Gate: `engine/page/tests/g_track_src.rs` (`G_TRACK_SRC`), against a real TCP origin.

### Residue

Nothing PAINTS a cue. `cuechange` hands a page's own renderer what to draw, and every adaptive player
has one — but a plain `<video>` with `<track default>` now holds the right cues, in `showing` mode,
and still shows the viewer nothing, because the UA has no caption overlay of its own. That, plus cue
positioning settings (still parsed and inert), is what stands between this and captions a user can
actually read.

## M7e — cue placement is not decoration (tick 260)

Tick 255 parsed cue timings and text and **discarded the settings** on the timestamp line; ticks 256
and 257 accepted them on `VTTCue` and kept them inert. So every cue in every file arrived at its
renderer bottom-centre, regardless of what the author wrote.

That is not a cosmetic loss. Caption authors use these settings to keep text **off** something:
`line:0` lifts a caption to the top because the bottom of the frame is already occupied — burned-in
subtitles, a scoreboard, a lower-third name card, the speaker's own mouth. `align:start
position:10%` pins a speaker's line to the side of the frame they stand on. Painting everything
bottom-centre lands each cue in the one place the author specifically avoided.

`CueSettings` now carries `vertical` / `line` / `line_is_percent` / `position` / `size` / `align`,
kept in **the spec's own vocabulary rather than resolved to pixels**, because the thing that resolves
them is a renderer that knows the video box — and there are two (the page's own overlay via `VTTCue`,
and eventually ours).

### `auto` is not `0`, and a bare number is not a percentage

Two claims that read as pedantry and are the whole correctness of the module:

- **`None` (auto) must survive as `auto`.** `line:0` is the TOP of the frame; `auto` is the bottom.
  Collapsing auto to 0 moves *every default caption in every file* to the top of the video.
- **A bare `line` number is a LINE COUNT.** Reading `line:0` as "0% down the frame" happens to look
  right, which is what makes the bug survive review — but `line:-1` means the LAST line, i.e. the
  bottom, and as a percentage it is nonsense.

Leniency is preserved: `align:middle` (superseded by `center`) and any unknown setting are skipped
rather than failing the cue, for the same reason a malformed timestamp does not fail the file.

### Probes

RED as predicted: discarding settings (the pre-tick behaviour) → `align/position/size` gone; reading
a bare `line` as a percentage → `line:0` becomes `0%`; emitting `0` for auto → every default cue
moves to the top.

**One probe was a silent no-op and one assertion was vacuous — both caught, both recorded.** The
first `line`-as-percentage probe set the flag at the top of a branch that assigns `false` two lines
later, so it changed nothing and the gate stayed green. The assertion it was aimed at read
`got.contains("line3=0")`, which **also matches `line3=0%`** — the very bug it was meant to catch. It
now asserts `"line3=0 align3=center"`, spanning the field boundary. Substring assertions on a
flat report string are a standing hazard in these gates: assert the delimiter too.

Gate: `engine/page/tests/g_track_src.rs` (`G_TRACK_SRC`), extended with four real-world cue shapes.

### Residue

Unchanged and now the only thing left in the caption arc: **nothing paints a cue.** The placement
data is correct, complete and available to a page's own renderer; a plain `<video>` with
`<track default>` still shows the viewer nothing, because the UA has no caption overlay.

## M7f — the UA paints the caption (tick 261)

Six ticks of caption work (255-260) all ended the same way: *correct data, handed to a renderer that
does not exist.* Parse, hold, time, fire `cuechange`, fetch `<track src>`, preserve placement — each
one hands cues to **a page's own overlay**, and a plain `<video>` with `<track default>` has no page
overlay, because a document with no player library never draws a caption itself. **The browser is
supposed to.** Until this tick a cue that was parsed correctly, timed correctly and placed correctly
reached the viewer as nothing at all, and every one of those six gates was green while it happened.

### The three-crate join

The painter is in Rust and **never sees the DOM** — `LayoutBox.node` is its only link, and no builder
signature takes a `&Dom`. The caption state is in **JavaScript** — `el.__textTracks`,
`el.__currentTime`, `track.mode` — because that is where the `TextTrack` API is. So the wire runs:

```
JS  el.__publishCues()          — the showing tracks' active cues
 ↓  __setActiveCues(nodeId, json)
Rust ACTIVE_CUES  (dom_bindings) — a thread-local, keyed by node
 ↓  manuk_js::active_cues()
     Page::caption_map()        — → manuk_paint::CaptionMap
 ↓
     caption_items(video_rect, cues) → Rect + Text display items
```

The last hop is the sanctioned channel the painter already uses for `images` and `z_index`: **a
NodeId-keyed side map**, resolved by the page layer, which is the layer that can see both sides.

`CueSettings`'s doc comment (tick 260) deferred pixel resolution to "a renderer that knows the video
box". `caption_items` **is** that renderer, and it is where `auto` finally has to mean something.

### Three things that are not details

**`ACTIVE_CUES` is STATE, not a queue.** Every other host bridge here (`PENDING_HISTORY`,
`PENDING_OPENS`) is drained by the host. A caption is on screen until it isn't; a paint that
*consumed* the cue set would show each caption for exactly one frame and then blank the picture.
The page overwrites a node's entry; the host reads without taking.

**An empty array must be sent.** It is how a cue leaves the screen. A bridge that only ever added
cues would burn the last caption of every video permanently into the frame.

**`hidden` is not `disabled`, and this is where it finally bites.** `activeCues` answers for a hidden
track — its cues are live, its `cuechange` still fires, so a page's own renderer keeps working — but
`hidden` means *exactly* "do not display this", and it is the mode a player sets **when it draws
captions itself**. So `__publishCues` filters on `mode === 'showing'`, and the `mode` setter publishes
**unconditionally**: flipping `showing`→`hidden` leaves `activeCues` identical, so the cue-diff
correctly sees no change and fires nothing — and the overlay would keep painting a caption the user
just turned off.

`VTTCue.line` also carries three things in one property — `'auto'`, a bare number (a **line count**,
possibly negative), or a `'%'`-suffixed string. `Number('10%')` is `NaN`; `parseFloat` on a line count
loses the distinction. Neither alone will do, and `line:0` reads correctly under both, which is what
lets the bug survive.

### RED probes — four, and one of them was VACUOUS

| probe | result |
|---|---|
| drop the caption emit from `push_group` | RED — "the active auto-line cue was not painted at all" |
| collapse `auto` to line 0 | RED — auto (21.6) not below `line:0` (21.6) |
| publish `hidden` tracks too | RED — "a `hidden` track still painted" |
| paint cues *before* the video | **GREEN — the assertion could not fail** |

The paint-order check was written against `DisplayItem::Image`, and **a `<video>` with no poster
decodes no bitmap**, so there was no Image item, the `if let Some(img_idx)` skipped the assertion
entirely, and it passed under a probe that painted every caption behind the frame. This is the
[[scripted-edit-silent-noop]] / vacuous-assertion class again, in its third distinct disguise across
three ticks: tick 260 caught a substring assertion that matched the bug it was hunting, and here a
conditional assertion whose condition was never true. **The fix is the same each time: make the thing
you are asserting about exist unconditionally.** The video now carries `background:#123456`, the item
is found by that exact colour (the captions emit `Rect`s too, so "the first Rect" would be wrong
again), and the reversal probe goes RED at "the caption paints at 1, BEHIND the video at 4".

Gate: `engine/page/tests/g_caption_paint.rs` (`G_CAPTION_PAINT`). `g_track_src`, `g_cue_change`,
`g_text_tracks` and `manuk-media`'s `vtt_captions` all still green.

### Residue

The caption arc is closed end-to-end: a `<track default>` on a plain `<video>` is fetched, parsed,
timed, placed and **drawn**. What remains is fidelity, not absence:

- **`vertical: rl/lr` is painted horizontally.** Recorded on `CaptionCue` rather than dropped, so the
  gap is visible; Japanese vertical subtitles are legible but wrongly laid out.
- **Text width is estimated, not shaped** (`chars × font_size × 0.5`) — the display list carries plain
  strings by design and shaping happens at raster time, so `align: end` and `size:` clipping are
  approximate. This is the same stub-metrics problem as [[ch-ex-unit-fontmetrics-lever]].
- **No cue box overlap avoidance** — two simultaneous cues with explicit equal `line` values collide.
- **`DisplayList::damage_since` over-damages `Text`** (a hardcoded 4096px-wide box), so a caption
  change repaints the row rather than the cue.

## Tick 262 — the browser finally asks for the movie, and something owns the clock

Six ticks of captions ended on a real join (261). This one closes the two that were still missing at
the **opposite** ends of the same pipeline, and both are the same shape as the caption bug: work that
was built, gated and correct, with nobody on either side of it.

### The front of the chain: `<video src>` was never requested

`Page::pending_image_urls` reads `<img src>` and — for a `<video>` — its **`poster`**. That is all.
So a plain `<video src="movie.mp4">` on a real page was not undecodable, not unsupported, not stalled:
it was **never fetched**. Every media test in the tree feeds bytes from `include_bytes!`, so demux,
AAC, H.264, the frame timeline and A/V sync were all green against fixtures the loader could never
have produced. `Page::pending_media_urls` is the missing producer.

**It returns `(NodeId, url)` pairs and the pair is the whole design.** Images are legitimately
answered by URL alone — `apply_images_by_url` re-walks the DOM and binds one decoded bitmap to every
node naming it, because a bitmap is immutable and shareable. **A playing video is not.** It carries a
position, so two `<video>` elements on one URL are two independent playbacks that may sit at
different times, which is exactly why `set_video_frame` was keyed by `NodeId` in the first place. The
host therefore has to be told which element it is fetching *for*, at request time, not at delivery.

**Source selection follows the spec's shape, and the asymmetry in `media_type_rejected` is the
point.** Sites list `<source type="video/webm">` before the MP4 and expect an MP4-only UA to walk
past the first one. Taking the first `<source>` unconditionally fetches a file that cannot decode
while a playable one sits two lines below — and the failure then surfaces as *a broken decoder*,
which is the misattribution that costs a tick. But the question the table answers is deliberately
**not** "do we support this type"; it is "are we **certain** we do not". Only a certain no is acted
on; an unknown or absent MIME is **attempted**, because the container sniffer and the decoder
downstream are the honest authorities. A wrong `no` is invisible (the video simply never loads); a
wrong `yes` costs one fetch that fails loudly. The table is kept as string policy in `manuk-page`
because naming `manuk-media`'s types would drag `openh264`'s C toolchain into all ~25 gate binaries
that link this crate — the isolation tick 236 established.

### The back of the chain: three clocks and no player

`FrameTimeline` indexes frames by presentation time (249), `Transport` holds a position, `AudioClock`
is master — and **nothing owned all three at once**. Every gate drove the parts by hand, so the tree
could demonstrate each step of playback and could not play. `VideoPlayer` is the owner a host holds.

Two decisions carry it. **The player picks its clock, not the caller.** MEDIA.md's rule is
audio-is-master, but most `<video>` on the open web is muted or has no audio track, and for those a
master clock that never ticks freezes the picture on frame one. So `tick(dt, Option<&AudioClock>)`
routes to `sync_to_audio` when there is a device and `advance` when there is not — one call that
cannot be got wrong by forgetting which case you are in, which is what happens when the choice is
left at the call site of two similarly-named methods. **And `frame()` answers while paused**, because
a paused video shows a picture; gating it on `is_playing` blanks the element on `pause()` and shows
nothing at all before the first `play()`.

The transport is armed from `FrameTimeline::duration()`, not the container's declared duration: a
partially-buffered stream has fewer frames than the header promises, and a transport that believes
the header runs the position off the end of what can be shown while `ended` never latches.

### RED probes: five, all fired

| probe | result |
|---|---|
| drop the `video`/`audio` arm from `pending_media_urls` | RED — "a `<video src>` must be requested" |
| take the first `<source>` unconditionally | RED — WebM fetched, MP4 below it never seen |
| invert `media_type_rejected` into an allow-list | RED — the unknown MIME silently stops loading |
| `frame()` returns `frames()[0]`, ignoring the transport | RED — "must show a DIFFERENT picture" |
| `tick` treats `None` as "no clock, do not move" | RED — the muted case freezes on frame one |
| `tick` calls `advance` *then* `sync_to_audio` | RED — position 0.1001, not the device's 0.1 |

The last one is the one worth keeping. Advance-then-sync ends up at audio's position on almost any
input, so the obvious assertion passes; it only fails against a `dt` **deliberately an order of
magnitude larger** than the audio advance. Asserting that the wall clock is *discarded* is a
different claim from asserting the result looks right, and only the first one catches a video clock
that stays partly authoritative — the state audio-is-master exists to forbid.

Gates: `engine/page/tests/g_media_urls.rs` (`G_MEDIA_URLS`), `engine/media/tests/g_video_player.rs`
(`G_VIDEO_PLAYER`). All of `manuk-media` (24 tests) and `g_video_frame`/`g_caption_paint` still green.

### Residue — stated plainly, because this is the caption trap again

**This does not yet play a video on screen.** Both ends now exist and meet in the middle, but the
**shell has no media handling at all** — no fetch of the URLs this produces, no per-frame tick, no
call into `set_video_frame` outside its own gate. That is the next tick and it is a shell tick, not
an engine one. Saying so here rather than claiming M6 is done is the direct lesson of the caption arc,
where six green gates shipped and the viewer saw nothing.

Also open: `MediaSource.isTypeSupported` still answers **false for everything** (`__mseCodecs` is
empty) even though MP4 + Constrained-Baseline H.264 + AAC now genuinely decode. That is honest *only*
until the shell can play them — populating the registry before a real `<video>` plays would be the
strictly worse lie the file's own comment warns about. Populate it in the same tick that lands
playback, never before. And `<audio src>` URLs are produced but there is no audio output path
(`cpal` is unbound), so audio elements resolve to a request nothing consumes yet.

## Tick 263 — the last link: a `<video>` on a real page now shows moving pictures

Tick 262 ended by stating its own residue plainly: *"this does not yet play a video on screen — the
shell has zero media handling."* This closes it. `shell/src/media.rs` is the driver, and with it the
chain runs end to end for the first time: `pending_media_urls` → `fetch_media_bytes` → `demux` →
`VideoPlayer` → `set_video_frame` → the painter's existing `blit_image`. Nothing new was invented in
the middle; the whole tick is the *joining*, which is what the previous eight media ticks kept
leaving out.

### The decoder lives in the shell, and only in the shell

`manuk-media` with `features = ["video"]` is a dependency of `manuk-shell` alone. That is the
deliberate consequence of tick 236's isolation and of `set_video_frame` taking raw RGBA rather than a
`manuk_media::video::Frame`: naming the media type in `manuk-page` would compile openh264's C into
all ~25 gate binaries that link that crate. The shell is one binary and the one place a real decode
has to happen. **Measured cost: 13.6s, once, then cached** — no warm-wall tax, which is the only
thing that would have made this trade refusable.

### Why a module and not ten lines in the event loop

Every interesting mistake here is in the joining — which element got which bytes, what happens when a
decode fails, whether the first frame is allowed through. Inline in the winit loop none of it is
reachable by a test, because a test cannot run winit. So the loop keeps only what needs a window (the
wall-clock delta, the repaint) and `MediaSet` holds everything that can be got wrong.

### Three decisions that are not details

**1. `Entry::published` is the frame the PAGE was given, not the player's position — and the gate
caught the bug that proves it matters.** The first implementation compared the player's frame before
and after the tick. That suppresses the **very first publish**: at that moment the player has not
moved (nothing has advanced yet) while the page holds no picture at all, so the driver reported "no
change" and the element stayed blank forever. The question is never *did the player move*; it is
*does the screen differ from what the decoder now says it should be*, and only a record of what was
**sent** answers that. It is also what keeps the driver correct across a re-layout that dropped the
image map, where the player is mid-stream and the screen is blank.

**2. A failed decode is REMEMBERED.** `players` stores `Option<Entry>`, and a `None` means *tried and
failed*, distinct from *never tried*. Leaving the map empty on failure means the fetch side sees no
entry, re-requests the file, fails again, and the browser busy-loops for as long as the page is open
— which from outside is indistinguishable from a slow network. Exactly the storm `image_by_url`'s
own `Option` exists to stop.

**3. `advance_media()` runs BEFORE `needs_paint` is read.** A playing video is what decides whether
the frame owes a paint, so publishing after that check would show every frame one redraw late and
drop the last frame of every video entirely.

Navigation clears the set at both `nav_gen += 1` sites: the players are keyed by `NodeId` into the
outgoing DOM, and kept across a navigation they would hand the next page's nodes the previous page's
video.

### RED probes: three, all fired

| probe | result |
|---|---|
| drop the `set_video_frame` call, keep everything else | RED — "a decoded frame must change what is PAINTED" |
| remove the `published` guard, always report a repaint | RED — a zero delta reported a repaint it did not owe |
| forget failed decodes instead of recording them | RED — `has()` stays false, the fetch side re-asks forever |

**The gate asserts through the PAINT PATH, not the driver's own state.** `painted()` renders the page
and compares canvas bytes, and every claim is *the picture changed* against a `blank` baseline taken
before any frame exists — never *a picture exists*, which is unfalsifiable. Reading the frame back off
the player would assert that the code believes it published something. Six caption ticks were green
on exactly that belief while the viewer saw nothing.

### Residue

- **Whole-file buffering.** `fetch_media_bytes` downloads the entire resource before one frame
  decodes — correct for the short files this can play, an OOM on a feature-length one. Real delivery
  is `Range` requests against a progressive buffer, which is the machinery MSE's `SourceBuffer`
  exists to feed. The demuxer already reports `DemuxError::Incomplete`, so the seam is open.
- **Autoplay, unconditionally.** Nothing routes a click to `play()` yet, so a player that waited
  would be a video that can never start. This becomes conditional on the `autoplay` attribute the
  moment controls (M7) land — one line in `MediaSet::load`.
- **No audio.** `cpal` is unbound, so `advance` passes `None` as the clock and the wall-clock
  fallback is correct rather than a shortcut. `<audio src>` URLs are produced and fetched but nothing
  consumes them.
- **`MediaSource.isTypeSupported` still answers false for everything.** `__mseCodecs` is empty. With
  playback now genuinely working for MP4 + Constrained-Baseline H.264, populating it is finally
  honest — and it is the next media tick rather than this one, because a registry claim should land
  with the gate that proves the claim.

## Tick 264 — the honest "no" that became a lie

`canPlayType` returned `''` for every type and `play()` returned a **rejected** promise. Both were
scrupulously correct when they were written — the prelude's own comment says so: *"`''` is the
spec's 'no'. `'probably'`/`'maybe'` are the only other answers, and both would be lies."*

Tick 263 made them lies in the other direction. A plain `<video src="movie.mp4">` carrying
Constrained-Baseline H.264 now fetches, decodes and plays on screen — and the browser was still
telling every page that asked that it could not. **A site that politely feature-detects was hiding a
player that would have worked** and showing its "your browser cannot play this" fallback instead.
That is the failure mode the original stub existed to prevent, arrived at from the opposite side.

**An honest answer is not a fixed answer.** This is the general lesson and it is worth naming: a
capability stub that hard-codes "no" is honest exactly as long as the capability is absent, and it
is the *only place in the tree that knows the answer changed*. Nothing fails when it goes stale —
no test breaks, no gate reddens, the browser simply under-reports itself forever. The three ticks
before this one all closed a variant of the same shape (built, correct, joined to nothing); this is
its fourth: **built, correct, joined — and still announcing the old state of the world.**

### The three answers, and why the distinction is real

| type | answer | why |
|---|---|---|
| `video/mp4; codecs="avc1.42E01E, mp4a.40.2"` | `probably` | codecs NAMED and we have them |
| `video/mp4` | `maybe` | container we read, codec unstated — cannot be promised |
| `video/mp4; codecs="avc1.640028"` | `''` | High profile; openh264 is Constrained Baseline only |
| `video/webm; codecs="vp9"` | `''` | no demuxer, no decoder |

`'probably'` for a bare `video/mp4` would be the same lie in reverse: that container carries HEVC
and High-profile H.264 too, and neither decodes here. The profile lives in the two hex digits after
`avc1.` — `42` plays, `4d` (Main) and `64` (High) do not, and that is most of the real web.

`play()` now resolves and flips `paused`. Which surfaced a second bug: `paused` was defined with
`ro()` — **getter-only** — so `el.paused = false` is a silent no-op in sloppy mode. Every player
would have painted a play button over a running video. It is now backed by a real flag.

### RED probes: three, all fired

| probe | result |
|---|---|
| answer `probably` for a bare container | RED — `bare:false` |
| drop the `avc1.42` profile check | RED — `nohigh:false` (High profile claimed as playable) |
| restore `ro('paused', true)` | RED — `playing:false` |

Probe 3 is worth keeping. Under the broken getter-only property **`repaused:true` stayed GREEN**,
because a getter that always returns `true` satisfies "pause() left it paused" perfectly. Only the
`playing` assertion catches it. An assertion that a value is what it already was is not an
assertion — the fifth appearance of the vacuous-assertion class in five ticks.

### Residue — one incoherence, deliberately not fixed here

`el.error` is still eagerly `MediaError(4)` (`MEDIA_ERR_SRC_NOT_SUPPORTED`). For a Baseline MP4 that
now plays, that is inconsistent with `canPlayType` saying `probably`, and a player that checks
`error` before anything else will still give up. **It was left alone on purpose, because fixing it
naively trades one honesty for another:** setting `error` to the spec-initial `null` would mean a
bare `<video src="x.webm">` with no `type` attribute reports no error at all and simply hangs, where
today it reports 4 immediately and the site shows its fallback. That is a regression on the
honest-failure axis, and the ratchet does not trade. The real fix is a **shell→JS bridge that reports
the actual decode outcome** — the shell already knows (`MediaSet` records a known-failed decode);
nothing tells the page. That is the next tick, and it makes `error` truthful in both directions
instead of picking which half to be wrong about.

`MediaSource.isTypeSupported` also still answers `false` for everything, and that one is **still
correct**: MSE's `appendBuffer` accepts segments that nothing drives into the decoder, so claiming
support would make every adaptive player append forever against a stall. The distinction is exact —
`canPlayType` answers for `<video src>`, which works; `isTypeSupported` answers for MSE, which does
not yet. Two questions, two different truths, and conflating them is how a player ends up wedged.

## Tick 265 — the outcome bridge: `video.error` stops guessing

Tick 264 closed with a stated incoherence: `canPlayType` answered `'probably'` for Baseline MP4 while
`el.error` was still eagerly `MediaError(4)` on every media element. A player that checks `error`
first — and most do, because it is the cheapest test — gave up on video that was about to play.

**The reason it was left for its own tick is the interesting part: neither fixed value is honest.**

| default | what it gets right | what it breaks |
|---|---|---|
| eager `MediaError(4)` | undecodable media shows a fallback | abandons video that works |
| spec-initial `null` | playable video proceeds | undecodable media shows a **dead player**, forever |

Picking either is choosing which half to be wrong about, and swapping one for the other would have
been a capability bought with an honesty regression — the trade the ratchet refuses. **The only
honest answer is the real one, and the host is the only layer that has it.** The shell fetches the
bytes and knows whether they decoded; `MediaSet` was already recording exactly that. Nothing told
the page. `Page::set_media_outcome(node, ok)` → `el.__setOutcome(ok)` is that report arriving.

So the default *is* now spec-initial `null` — correct in the spec's own terms, because no load has
been **attempted** — and it stops being a guess the moment the host reports. On success: `error`
null, `readyState` HAVE_ENOUGH_DATA, `networkState` IDLE. On failure: `MediaError(4)`, HAVE_NOTHING,
NETWORK_NO_SOURCE. Both fire their events (`loadedmetadata`/`loadeddata`/`canplay`, or `error`),
because a state change no event announces is a state change no player notices — they bind
`onerror`/`oncanplay` rather than polling.

**A failed fetch reports too, and that is not a detail.** The obvious `continue` on a 404 leaves the
element at `error === null` forever, which reads to every player as *still loading* — so a missing
video file hangs the fallback it is supposed to trigger. The fetch failure now travels as empty
bytes, which fail to decode and arrive as a real error.

### RED probes: three, all fired

| probe | result |
|---|---|
| make `set_media_outcome` a no-op | RED — `null` where `4` was owed (the two halves never joined) |
| success path does not clear `err` | RED — a recovered element stays permanently errored |
| restore the eager `MediaError(4)` | RED — `4` before any load was attempted |

Probe 1 is the one the gate exists for. The JS half is gated in the conformance suite and the Rust
half in `g_media_outcome_bridge`; **without a gate that crosses the boundary, both stay green while
no real page ever hears a word** — which is precisely the built-and-never-joined family that ticks
261-264 each closed one variant of.

### Note: the bridge assertion does NOT live in the shell gate

It was written there first and moved. `shell/src/media.rs`'s test is a unit test in a bin crate,
co-running with 58 others in one process, and mozjs's teardown crashes when a leaked runtime is
co-run — the reason `js_conformance_suite` is `#[ignore]`d and launched in isolation. A JS-evaluating
assertion there would have been a SIGSEGV at process exit, taking the whole shell suite (and the
wall) with it. The shell gate stays pure Rust; the JS crossing is gated in
`engine/page/tests/g_media_urls.rs`, which is its own test binary.

### Residue

`isTypeSupported` still answers `false` for everything, still correctly — it answers for MSE, where
`appendBuffer` accepts segments nothing drives into a decoder. Whole-file buffering, unconditional
autoplay and the absent audio device are unchanged from tick 263. `readyState` jumps straight to
HAVE_ENOUGH_DATA rather than climbing through HAVE_METADATA as bytes arrive, which is honest for a
whole-file fetch and becomes wrong the moment ranged fetching lands.

## Tick 349 — the MSE playback JOIN: appended bytes reach the decoder

The adaptive-streaming class had every piece and no path between them: `appendBuffer` accumulated
real bytes (`__bin`), `__mseDemux` populated `buffered` from them, the shell's `MediaSet` could
decode+drive Baseline H.264 — and an MSE-attached `<video>` stayed a dead player forever, because
its `src` is a `blob:` URL that `pending_media_urls`→`fetch_media_bytes` can never serve. The only
copy of the media lives *inside the page*.

### The channel

`SourceBuffer.__demux` (each settled append that demuxed a **video** track) →
`__msePublish(nodeId, __bin)` [one-char-per-byte, same convention as `__mseDemux`] →
`dom_bindings::PENDING_MSE_STREAMS` (thread-local queue, clipboard/postMessage shape) →
`manuk_js::take_mse_streams()` → `Page::take_mse_media()` (coalesces to the NEWEST stream per
node — ten appends between host visits = one decode, not ten) → `gui::advance_media` drains →
`MediaSet::load_mse`.

The FULL stream is published each time, never a delta: an fMP4 decoder needs the init segment plus
every fragment as one contiguous buffer (`FrameTimeline::decode` takes the whole thing).

### `load_mse` is deliberately not `load`

Two of `load`'s assumptions invert for MSE. (1) The stream GROWS — a re-publish must not restart
playback, so transport position + play/pause state carry across the re-decode (seek into the new,
longer timeline). (2) A failed decode is RETRIED, not remembered — an init-segment-only buffer is
the NORMAL first state of every MSE session; the progressive path's "known failure, never
re-requested" rule would kill every real session at its first append. No fetch-storm is possible
in exchange: this path is publish-driven, never poll-driven.

### The registry now tells the truth (and only the truth)

`canDecode` (feeding both `isTypeSupported` and `addSourceBuffer`) gained a built-in matcher for
exactly what the tree genuinely plays end-to-end: MP4 container, `avc1.42xxxx` (Baseline only —
the profile byte, same refusal as `video::can_decode`) and `mp4a.40.*` (AAC). WebM/VP9/AV1 stay
`false` — no demuxer, no decoder, and a YES without one steers a player onto a path that hangs.
The `__mseCodecs` push-registry is untouched (gates use it).

### G_MSE_JOIN (shell suite — which IS in the verify wall, unlike the older media gates)

Drives the full dance in a real scripted page against the real `bear-av-baseline_frag.mp4`:
`isTypeSupported:true` → `addSourceBuffer` → `appendBuffer` → publish → byte-for-byte fidelity
across the JS boundary → decode → `painted()` changes → re-publish resumes (frame equals the
pre-reload frame, not frame 0) → init-only prefix fails then retries. RED-proven both directions:
deleting the `__msePublish` call fails at "exactly one stream" (the silent dead player); reverting
the `canDecode` matcher fails at `its:true` with `THREW-open:NotSupportedError` (today's shipped
behaviour). Restored byte-for-byte after each probe.

**The old note "a JS-evaluating assertion in the shell unit binary would SIGSEGV at exit" no
longer holds** — measured twice this tick: full suite, exit 0, no signal. The clean-exit work
(`G_CLEAN_EXIT`, process-wide `JS_ShutDown` discipline) landed since that note was written.

### Residue, honestly

ABR quality switching (no `SourceBuffer.remove`-driven eviction pressure, no bandwidth estimate),
High/Main-profile H.264 (the codec ladder's M5+ rung: cros-codecs/VAAPI or the ffmpeg-next
feature-gate), audio OUTPUT (decoded PCM exists, `cpal` unbound — gate on PCM, never on a sound
card), and background-tab drains (the harvest rides `advance_media`, which only the foreground
redraw path calls).

## Tick 350 — audio OUTPUT: the device end, and why the gate never listens

The last dead organ in the A/V-file pipeline. AAC decoded to sample-exact PCM since M4 (tick 235)
and the tick-349 join put appended streams through the decoder — but nothing on the box ever
consumed the PCM, so every video played mute. `cpal` (BORROWED, 0.17) is now the device;
`shell/src/audio.rs` is the split that keeps it gateable.

### The pump/device split IS the gate design

`AudioFeed` (the pump) is pure arithmetic: decoded interleaved PCM + a cursor, filled into
whatever chunk sizes the caller asks for. `AudioOut` (the device) is a `cpal` output stream whose
real-time callback locks the shared `Arc<Mutex<AudioFeed>>` and calls `fill`. Everything that can
be got wrong — a sample dropped at a chunk boundary, a cursor that creeps while paused, a restart
on an MSE re-decode — lives in the pump, where `G_AUDIO_PUMP` drives it against the real fixture's
decode with NO device. The observer's standing rule (tick 264) is load-bearing here: **gate on
decoded-PCM correctness, never audible playback** — a gate that needs a working sound card
false-REDs on every headless box. `AudioOut::open` returning `None` (no hardware) is the normal
headless case, probed once per page, and is a silently-playing video rather than an error.

### Three contracts the gates pin

- **Silence is written, not assumed.** Every non-delivering path (paused / exhausted / poisoned
  lock) must `fill(0.0)` the WHOLE buffer — the device plays whatever is in it, and an untouched
  buffer replays the previous callback as a stutter-loop. The gate pre-fouls buffers with NaN and
  asserts they come back zeroed.
- **The cursor lands EXACTLY.** First run of the gate caught its own hole: advancing by
  `out.len()` instead of the copied count corrupts nothing mid-stream (full chunks are equal) and
  only overshoots at the tail — invisible to the byte-exact concatenation check, caught only by
  `cursor == samples.len()` after the drain. A green that could not go red measured nothing.
- **The Arc survives the MSE grow.** The device captured its feed clone at open time. `load_mse`
  therefore mutates the carried feed in place (`replace_pcm`: new PCM, cursor kept, clamped if
  shorter) — a fresh Arc would leave the stream pulling from an orphan and the audio dies on the
  first append. Only `Arc::ptr_eq` can see this; `G_AUDIO_JOIN` asserts it.

### Feature-lane discipline (the wall cares)

`dep:cpal` rides the shell's `gui` feature — the `--no-default-features` headless check never
compiles ALSA bindings. `manuk-media` gains `audio` in the SHELL's dep only (joining `video`
there): the ~25 manuk-page gate binaries build `manuk-media` through `manuk-js` with
`default-features = false`, so symphonia stays out of their link exactly as the M4 isolation
established.

### Residue, honestly

~~A/V sync is still wall-clock~~ — closed by tick 351 (below). No volume/`muted` plumbing into
the feed; one output stream binds the FIRST audio-carrying element (mixing is a mixer's job);
non-AAC audio still refused by name.

## Tick 351 — A/V master-slave sync: the device clock owns time

The wire the tick-350 residue named: `MediaSet::advance` passed `None` for the audio clock, so on
a box with sound the picture ran on the wall clock while the device ran on its own crystal — two
clocks that visibly part company on any long play (the lip-sync class; invisible in every short
test, guaranteed at scale). Every piece already existed: `Transport::sync_to_audio` SNAPS (t250),
`VideoPlayer::tick(dt, Option<&AudioClock>)` routes to it, `AudioFeed::position_seconds()` is the
sample-exact device cursor (t350). Tick 351 is the join plus its two honesty rules.

### Mastery follows the DEVICE, not the existence of a feed

`gui::advance_media` hands `MediaSet::advance` the feed the output stream is actually consuming
(`AudioOut::feed()`, captured at open). Inside `advance`, an entry is slaved **only** when its
own feed is that one by `Arc::ptr_eq` — the same identity discipline `G_AUDIO_JOIN` pins on the
grow cycle, and for the same reason read in the other direction: a feed the device is NOT pulling
from has a motionless cursor, and a motionless master would freeze the picture on frame N
forever. The slave clock is built per frame: `AudioClock::new(rate)` + `seek(position_seconds())`
— exact, since the cursor is frame-integral.

### Two hand-backs to the wall, both load-bearing

- **No device** (`master: None`): headless box, no sound hardware, no audio track — the wall
  clock is the honest fallback and behaviour is byte-for-byte what it was before the device
  existed. This is why the gate runs headless without a special case.
- **Exhausted or paused master**: an audio track shorter than its video would otherwise pin the
  transport to the end of the sound and freeze the picture's tail. When the master stops moving,
  `tick` gets `None` and the wall resumes from wherever the snap left the position.

### G_AV_MASTER (shell suite = IN the verify wall) — each claim RED-proven by edit

1. **audio is master, the wall's lie ignored** — with the device-bound feed consumed to T, an
   absurd wall delta lands the transport exactly on T. RED: pass `None` inside `advance` (the
   pre-351 wire) — every OTHER media gate stays green, which is the point.
2. **identity, not availability** — an imposter feed (same PCM, wrong Arc) does not govern. RED:
   drop the `Arc::ptr_eq` guard.
3. **exhaustion hands back** — truncate the master's PCM at its cursor; the tail still advances
   by dt. RED: drop the `exhausted()` check — the position pins to the audio's end.

## Tick 352 — `muted` reaches the device: silent consumption, never pause

`<video autoplay muted>` is THE autoplay pattern of the real web (Chrome only permits autoplay
WITH sound when muted), so from the moment the device landed, ignoring the attribute meant quiet
pages blast audio here. `MediaSet::advance` re-reads the element's `muted` content attribute
every frame (so `setAttribute`/`removeAttribute` takes effect live) into `AudioFeed::muted`.

The design point: a muted `fill` **consumes at full rate and delivers zeros** — the cursor
advances exactly as if audible. Mute-as-pause fails twice: the feed stops being a valid A/V
master (t351 hands a non-moving master back to the wall), and unmute resumes STALE audio
desynced by the whole muted interval. With silent consumption the clock never stops and unmute
lands on the exact sample the silence reached. G_MUTED_OUT RED-proofs: delete the DOM sync
(both videos play loud), mute-as-pause (the clock freezes), keep the copy under mute (the
pre-fouled buffer catches the leak).

Residue, honestly: `volume` and the live `.muted` IDL property have NO content attribute — they
need a JS live-property channel to the host (follow-on, not smuggled in half-built).

## Tick 353 — AV1 decode: re_rav1d behind the M5 trait

The codec the web is moving to, decoded in memory-safe Rust: `re_rav1d` 0.1.3 through its own
safe `dav1d` module (a fork of dav1d-rs; upstream `rav1d` is a C-ABI drop-in with no Rust API —
MEDIA.md trap #3). `default-features = false` keeps nasm out of the build; `bitdepth_8` only
(what the web serves). The `av1` feature implies `video` and stays out of every js/page gate
lane — the honest-registry rule is enforced at compile time: `can_decode_video` answers yes to
`av01.*` ONLY under the feature, so a lane without the decoder keeps saying no.

Structure: `Av1Decoder` behind `VideoDecoder`; `decoder_for` ladder in playback.rs picks by
codec string (fall-through keeps H264's refusal wording for unknown codecs); the trait gained a
defaulted `finish()` because dav1d is a QUEUE — pictures can arrive after their sample, and the
timestamp rides THROUGH the decoder (microseconds in `send_data`, back out on the picture) so
pts-by-call-order conflation is impossible. Conversion is BT.601 limited I420/I422/I444/I400 →
RGBA.

**The archaeology, one variable at a time** (the first version failed 'no sample produced a
frame' and the obvious fix — rerun's `max_frame_delay(1)` — was NOT the cause): under DEFAULT
settings dav1d delays the picture past the last `decode_sample` drain, and the original
`finish()` called `flush()` first — **dav1d's flush is a seek-reset that DISCARDS pending
pictures**, so the tail (here: the whole 1-frame stream) silently vanished. Reproduced exactly:
default+flush FAILS, default+drain-only PASSES, delay=1 makes pictures synchronous either way.
Shipped: delay=1 (browser latency + the MSE grow cycle wants same-call pictures) AND drain-only
finish — doubly safe, each on its own merits.

RED ledger: U/V swap → yellow decodes as cyan `[47,255,255]`, quadrant assert fails; the
flush-discard pair above; av1C config-send is NOT RED-provable with this fixture (its keyframe
carries its own sequence header) — noted in the module, kept per AV1-ISOBMFF §2.3.

Fixture: `four-colors-av1.mp4` from Chromium test data (BSD-3, provenance in tests/data/README
per the t235 steer: test DATA with attribution, CODE never).

## Tick 354 — AV1 ships: the shell lane and the three registries flip together

The organ (t353) reaches users only when the shell's `manuk-media` features include `av1` — and
the moment it does, three honesty registries that said "AV1: certain no" become lies in the
other direction. All flipped in this tick, per the t349 organ+registry rule: `mse_js`
`canDecode` (isTypeSupported), `canPlayType`'s refuse-list, and `media_type_rejected`'s
certain-no list (`<source type>` selection). WebM/VP9 stay refused — genuinely absent.

Gates: G_AV1_DRIVE (four-colors AV1 through MediaSet CHANGES what is painted; an av01
`<source>` is requested not skipped) + the JS registry claims folded into `g_mse_join`'s page —
ONE JS test per shell binary (two mozjs contexts in one test process abort on thread-local
teardown; the t262 rule re-confirmed live when a second `_sm` test was added and the suite
aborted).

**Two traps caught live, both worth keeping:**
1. **The vacuous substring claim.** The gate asserted `record.contains("av1:true")` — which the
   `cpt-av1:true` entry satisfies as a SUBSTRING, so deleting the MSE arm left the gate green.
   Caught by a tripwire (force-fail with an impossible claim to print the record). Rule: a
   record claim label must never be a substring of another record entry.
2. **Stale-binary false-green during RED runs** was the first suspicion and was WRONG — the
   binary rebuilt fine; the vacuous claim was the whole story. Probe the assertion before
   blaming the toolchain.

## Tick 355 — AVIF stills: the hero-image hole, closed in the shell lane

An AVIF is an AV1 keyframe in a HEIF box; both halves borrowed (`avif-parse` 1.4 walks the
container, the t353 `re_rav1d` instance decodes). The design point is WHERE it lives: the
obvious home — next to `image::load_from_memory` in manuk-page — is exactly forbidden, because
every gate binary links manuk-page and the decoder-isolation rule keeps rav1d out of them. So
the seam is `fetch_image_urls_with_raw` (bytes the page cannot decode come back RAW instead of
dropped) and the SHELL decodes (`decode_raw_images`: sniff ftyp-avif/avis → `decode_avif` →
`DecodedImage`), merging into the same map `apply_images_by_url` already takes — off the UI
thread, in the fetch task.

G_AVIF_PAINT (shell suite = IN the wall): the solid-red Blink 8bpc fixture decodes RED pixels
(a U/V swap paints blue, a range error washes grey) AND changes what a real page paints; the
10bpc twin is refused gracefully by the `bitdepth_8` build (empty map — never a panic, never a
wrong picture); non-AVIF bytes are sniff-skipped. RED-proven: insert-drop (the silent vanish)
and sniff-false, both watched fail.

Honest residue: alpha (a separate aux AV1 image) renders opaque; 10/12-bit refused;
`data:`-URL AVIF in headless page loads stays undecoded (the isolation rule cuts both ways).
