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
