feat(loop tick 234): media M3 — the engine can open a media file, and `buffered` answers

The MSE byte pipe was complete and inert: a page could construct a MediaSource, attach it,
fetch a segment byte-exactly (t227/t228) and appendBuffer it — and sb.buffered.length was 0,
because nothing had ever looked at the bytes. That zero is not cosmetic. `buffered` is the
variable an adaptive player's fetch loop steers by, so a buffered that never advances is a
loop that never advances, and a perfect byte pipe still gets no streaming site past its
first segment.

New crate engine/media (manuk-media), borrowing re_mp4 per the MEDIA.md trap-list. Produces
tracks (kind, RFC 6381 codec string, dimensions, channels/rate, and the avcC/av1C/vpcC/AAC
config record extracted now so M4/M5 are decoder steps and not another parsing step), a
sample table, and merged presentation-time ranges. Wired to JS via __mseDemux — a global
native taking the accumulated stream as a one-char-per-byte string (the convention this
boundary already uses) and answering in JSON. SourceBuffer.__demux populates buffered,
videoTracks/audioTracks and duration.

AAC needed writing, not borrowing: re_mp4 returns None for mp4a. Built mp4a.40.2 from the
esds descriptor.

Corrected a real bug in the borrowed parser: re_mp4 inverts the sync flag of every
FRAGMENTED sample (reader.rs:443 reads bit 16 of a trun sample-flags word as is_sync, but
that bit is sample_is_NON_sync_sample). Found by differential test against three Chromium
fixtures that differ only in their sync flags — the source looks right until you check which
flag bit 16 is. A seek must land on a sync sample, so inverted, every seek into a fragmented
stream lands on a frame that cannot decode standalone: garbage or a silent stall, nothing
thrown. Corrected per sample by origin (stbl count vs trun), not per file.

Honesty held: no decoder, no frame, isTypeSupported still answers false from the empty
__mseCodecs registry — and g_media_buffered ASSERTS that false, so this landing cannot
silently start over-promising. WebM is recognised by sniff and refused by name, not blamed
as corrupt.

Gates: engine/media/tests/demux.rs (real fixtures, both container forms, the differential
sync-flag test) and engine/page/tests/g_media_buffered.rs (a real fMP4 over a real socket,
appended, read back through the public API only). RED, run: dropping the sync correction
reproduces [false,true] vs expected [true,false]; making __demux return immediately
reproduces `ranges:0 start:- end:- vtracks:0` — the inert pipe exactly.

WIKI: docs/wiki/media-pipeline.md
