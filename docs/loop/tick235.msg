feat(loop tick 235): media M4 — AAC decodes, and the length proves it

M3 could find the audio and name it (mp4a.67, 44100Hz, stereo) and could not produce a
single sample of it. Naming a codec and decoding it are different claims.

engine/media/src/audio.rs decodes AAC to interleaved f32 PCM via symphonia, pulled in
narrowly (default-features = false, features = ["aac"]) so its AUDIO-ONLY ISO-MP4 demuxer is
not silently acquired alongside re_mp4's — two parsers with overlapping jobs is how they
drift.

ONE CORRECTION TO THE ABOVE, caught on re-landing and worth more than the feature itself:
engine/js said `manuk-media = { workspace = true, default-features = false }` and cargo
IGNORED it — a member's `default-features` does nothing for a `workspace = true` dependency
unless the workspace declaration says it too. Cargo warns, and warns it may become a hard
error. The guard I wrote to keep the decoder out of every gate binary was never the thing
holding; `manuk-media`'s `default = []` was doing all the work by itself. Nothing leaked,
because an empty default has nothing to leak — but the line READ as protection, so the day
something joined `default`, it would have walked into all ~25 gate binaries and the wall
would have regressed with a guard sitting right there looking correct. Moved to the
workspace declaration, where cargo enforces it.

Proven both directions rather than asserted: `cargo tree` finds 0 symphonia in
manuk-shell (default AND --no-default-features) and in manuk-js, and 9 under
`-p manuk-media --features audio` — the probe can see it when it is there.

The AAC AudioSpecificConfig had to be REBUILT, not sliced: a decoder cannot interpret a
single packet without it, and re_mp4 parses the esds descriptor into fields without
retaining the original bytes, so there is nothing to slice. Re-encoded from the parsed
values (AAAAA FFFF CCCC 000; AAC-LC/44100/stereo = 0x12 0x10).

THE ASSERTION that makes this a decode gate rather than a did-it-run gate: decoded PCM frame
count must equal the track's declared duration in its own timescale — 121856 units at 44100
is 121856 frames, exactly. Those numbers come from independent sources (container headers vs
summing decoder output packet by packet), so a decoder that dropped packets, doubled a
buffer, truncated, or mis-read the channel count lands somewhere else. Plus a non-zero peak
assertion, because correctly-sized SILENCE passes every length check.

Codec-string subtlety kept: the fixture is mp4a.67 (MPEG-2 AAC-LC object type indication),
not mp4a.40.2 (the MPEG-4 spelling, which takes the audio-object-type suffix). Players
string-compare these, so reporting one for the other is a rejection.

Honesty held: isTypeSupported is UNCHANGED and still answers false — audio decodes, video
does not, and a stream needs both. There is no audio device; cpal is a separate step because
a device is not headlessly gateable and bundling it would mean the decode could only be
proven by listening to it. Non-AAC audio (MP3/Opus/Vorbis/FLAC/AC-3) is refused up front by
name rather than accepted and failed mid-stream.

Gate: engine/media/tests/audio_decode.rs. RED, run: decoding only the first packet yields
1024 frames against 121856 — off by the whole track.

WIKI: docs/wiki/media-pipeline.md
