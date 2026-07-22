//! # The media drive — where a decoded frame finally reaches the screen
//!
//! Tick 262 closed the two ends of the pipeline and said plainly what was still missing: the
//! **shell had no media handling at all**. `Page::pending_media_urls` produced URLs nobody fetched,
//! `VideoPlayer` produced frames nobody asked for, and `Page::set_video_frame` — three lines that
//! blit a frame into the box the poster occupies — had exactly one caller in the whole tree, its
//! own gate. Every piece was built, gated and correct, and a `<video>` on a real page showed a
//! poster forever. This module is the driver that joins them, and it is the last link.
//!
//! ## Why this is a module and not ten lines in the winit loop
//!
//! The joining logic is where the interesting mistakes live — which element got which bytes, what
//! happens on the frame where a decode fails, whether a paused video keeps its picture. Written
//! inline in the event loop none of that is reachable by a test, because a test cannot run winit.
//! So the loop keeps only what genuinely needs a window (the wall-clock delta and the repaint), and
//! everything that can be got wrong lives here where [`MediaSet`] can be driven against a real
//! `Page` and a real fixture. Same reasoning that put the caption bridge behind `caption_map()`.
//!
//! ## A failed decode is remembered, and that is the load-bearing decision
//!
//! `MediaSet` records an entry for a URL whose bytes will not decode, rather than leaving the map
//! empty. Leaving it empty means the next `advance` sees no player, asks for the media again, fails
//! again, and the browser sits in a fetch-decode-fail loop for as long as the page is open — a
//! busy-wait that looks exactly like a slow network from the outside. This is the same storm
//! `image_by_url`'s `Option` (a **known** failure, distinct from "not tried") exists to stop, and it
//! is why the decode result is stored as an `Option<VideoPlayer>` and not merely inserted on success.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use manuk_dom::NodeId;
use manuk_media::{demux, TrackKind, VideoPlayer};

use crate::audio::AudioFeed;

/// Every `<video>` on the current page that has been handed bytes.
///
/// Keyed by `NodeId`, not by URL, and that is not an implementation convenience. A decoded bitmap
/// is immutable and shareable, so `apply_images_by_url` can correctly bind one image to every node
/// naming it. **A playing video carries a position**, so two `<video>` elements pointing at the same
/// file are two independent playbacks that may sit at different times — one paused on frame one
/// while the other runs. Sharing a player between them would make scrubbing one seek the other.
#[derive(Default)]
pub struct MediaSet {
    /// `None` is a decode that was tried and FAILED — see the module note on why that is recorded
    /// rather than forgotten.
    players: HashMap<NodeId, Option<Entry>>,
    /// Live `.muted` IDL overrides (tick 360). `Some` means the page SET the property, and from
    /// that moment the property — not the attribute — is the live state (the attribute is the
    /// default, per the defaultMuted split). Keyed independently of `players` because a player
    /// script routinely sets `.muted` BEFORE the media bytes arrive.
    idl_muted: HashMap<NodeId, bool>,
    /// Live `.volume` IDL values 0..1 (tick 360), same arrival-order independence.
    idl_volume: HashMap<NodeId, f32>,
    /// Live `.playbackRate` values (tick 361), same arrival-order independence.
    idl_rate: HashMap<NodeId, f64>,
}

/// A decoded video and **the frame the page was last actually given**.
///
/// `published` is not the same thing as the player's current position, and conflating them is a
/// real bug the gate caught: comparing the player's frame before and after a tick suppresses the
/// **first** publish, because at that moment the player has not moved (nothing has advanced yet)
/// while the page still holds no picture at all. The question worth asking is never "did the player
/// move" — it is "does the screen differ from what the decoder now says it should be", and only a
/// record of what was *sent* can answer that. It also makes the driver correct across a re-layout
/// or a page that dropped its image map, where the player is mid-stream and the screen is blank.
struct Entry {
    /// `None` is an AUDIO-ONLY stream (tick 363: `<audio src="x.mp3">`) — no frames, no
    /// transport; the feed itself is the playhead (the device consumes it, `position_seconds`
    /// reports it, `exhausted` is `ended`).
    player: Option<VideoPlayer>,
    /// Presentation time of the frame currently handed to the page. `None` = nothing sent yet.
    published: Option<f64>,
    /// The element's decoded audio, shared with the device callback (tick 350). `None` when the
    /// file has no audio track or its codec is not decodable — video plays silently, honestly.
    ///
    /// The **Arc identity is load-bearing**: `AudioOut` captured its clone when the stream
    /// opened, so an MSE re-decode mutates this feed in place (`replace_pcm`) rather than
    /// swapping in a new Arc the device would never see.
    audio: Option<Arc<Mutex<AudioFeed>>>,
}

impl MediaSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Drop everything. Called on navigation: a player is bound to `NodeId`s in a DOM that no longer
    /// exists, and a stale entry would hand the *next* page's node the previous page's video.
    pub fn clear(&mut self) {
        self.players.clear();
        self.idl_muted.clear();
        self.idl_volume.clear();
        self.idl_rate.clear();
    }

    /// Has this element already been given its bytes — successfully or not?
    ///
    /// The fetch side asks this so a URL is requested once. Answering only for *successful* decodes
    /// is the fetch-decode-fail loop the module note describes.
    pub fn has(&self, node: NodeId) -> bool {
        self.players.contains_key(&node)
    }

    /// Decode a fetched media resource for one element and start it playing.
    ///
    /// Autoplays, which is correct for where this sits today: nothing yet routes a click to
    /// `play()`, so a player that waited would be a video that can never be started. It is also what
    /// the muted/`autoplay` majority of the open web asks for. The moment controls land (M7) this
    /// becomes conditional on the `autoplay` attribute, and that is a one-line change here.
    ///
    /// Returns whether a picture is now available — the caller repaints on `true`.
    pub fn load(&mut self, node: NodeId, bytes: &[u8]) -> bool {
        if let Some(mut p) = Self::decode(bytes) {
            p.play();
            if let Some(&r) = self.idl_rate.get(&node) {
                p.set_rate(r);
            }
            let audio =
                Self::decode_audio(bytes).map(|pcm| Arc::new(Mutex::new(AudioFeed::new(pcm))));
            self.players.insert(
                node,
                Some(Entry {
                    player: Some(p),
                    published: None,
                    audio,
                }),
            );
            return true;
        }
        // Not an MP4 with video — a raw AUDIO stream? (tick 363: `<audio src="x.mp3">`.) The
        // sniff keeps the probe off every genuinely-broken fetch.
        if manuk_media::sniff_mpeg_audio(bytes) {
            if let Ok(pcm) = manuk_media::decode_audio_stream(bytes) {
                self.players.insert(
                    node,
                    Some(Entry {
                        player: None,
                        published: None,
                        audio: Some(Arc::new(Mutex::new(AudioFeed::new(pcm)))),
                    }),
                );
                return true;
            }
        }
        // Remembered as a known failure. See the module note.
        self.players.insert(node, None);
        false
    }

    /// Decode an **MSE stream** for one element — the join's landing point, and deliberately not
    /// [`MediaSet::load`], because an MSE stream breaks both of `load`'s assumptions:
    ///
    /// - **The stream GROWS.** `appendBuffer` keeps arriving, so this is called again with a
    ///   longer buffer for an element that already has a working player. Rebuilding the player
    ///   from scratch would restart the video on every append — a player-visible bug — so the
    ///   transport position and play/pause state carry over to the new, longer timeline.
    /// - **A failed decode is RETRIED, not remembered.** An init-segment-only buffer (tracks
    ///   defined, zero samples) is the NORMAL first state of every MSE session, not a broken
    ///   file; the "known failure, never retried" discipline that stops the progressive path's
    ///   fetch-decode-fail storm would here permanently kill every stream at its first append.
    ///   No storm is possible in exchange: this path is publish-driven (the page appends, the
    ///   host drains), never poll-driven.
    pub fn load_mse(&mut self, node: NodeId, bytes: &[u8]) -> bool {
        let resume = match self.players.get(&node) {
            Some(Some(e)) => e
                .player
                .as_ref()
                .map(|p| (p.transport().position(), p.transport().is_playing())),
            _ => None,
        };
        // Carried across the re-decode so the device keeps its Arc — see the note on `Entry`.
        let prev_audio = match self.players.get(&node) {
            Some(Some(e)) => e.audio.clone(),
            _ => None,
        };
        let Some(mut p) = Self::decode(bytes) else {
            self.players.insert(node, None);
            return false;
        };
        let playing = match resume {
            Some((pos, playing)) => {
                p.seek(pos);
                if playing {
                    p.play();
                } else {
                    p.pause();
                }
                playing
            }
            None => {
                p.play(); // same autoplay rationale as `load`
                true
            }
        };
        // The audio side of the resume rule: a grown stream REPLACES the PCM in the existing
        // feed (cursor survives); a new stream gets a fresh feed; a grow whose audio no longer
        // decodes keeps what it had rather than going silent mid-session.
        let audio = match (prev_audio, Self::decode_audio(bytes)) {
            (Some(feed), Some(pcm)) => {
                if let Ok(mut f) = feed.lock() {
                    f.replace_pcm(pcm);
                    f.set_playing(playing);
                }
                Some(feed)
            }
            (None, Some(pcm)) => {
                let mut f = AudioFeed::new(pcm);
                f.set_playing(playing);
                Some(Arc::new(Mutex::new(f)))
            }
            (prev, None) => prev,
        };
        // `published: None` so the current frame is re-pushed even if its presentation time
        // matches — the page's image map may never have seen this player's pixels.
        self.players.insert(
            node,
            Some(Entry {
                player: Some(p),
                published: None,
                audio,
            }),
        );
        true
    }

    /// A live media-IDL property write (tick 360): the channel end the mute button and volume
    /// slider land on. Values are stored keyed by node (they may precede the bytes) and applied
    /// to a live feed immediately when one exists. `playbackRate` is accepted and DROPPED here,
    /// deliberately: its transport/mastery interplay is its own tick, and until the host applies
    /// it the JS property stays a stored number exactly as before — no new claim.
    pub fn apply_prop(&mut self, node: NodeId, prop: &str, value: f64) {
        match prop {
            "muted" => {
                let m = value != 0.0;
                self.idl_muted.insert(node, m);
                if let Some(Some(e)) = self.players.get(&node) {
                    if let Some(a) = e.audio.as_ref() {
                        if let Ok(mut f) = a.lock() {
                            f.set_muted(m);
                        }
                    }
                }
            }
            "volume" => {
                let g = value.clamp(0.0, 1.0) as f32;
                self.idl_volume.insert(node, g);
                if let Some(Some(e)) = self.players.get(&node) {
                    if let Some(a) = e.audio.as_ref() {
                        if let Ok(mut f) = a.lock() {
                            f.set_gain(g);
                        }
                    }
                }
            }
            "playbackRate" => {
                self.idl_rate.insert(node, value);
                if let Some(Some(e)) = self.players.get_mut(&node) {
                    if let Some(p) = e.player.as_mut() {
                        p.set_rate(value);
                    }
                }
            }
            _ => {}
        }
    }

    /// The element's audio feed, if its stream decoded one. The GUI hands this to [`AudioOut`]
    /// once; navigation clears the set and the device with it.
    ///
    /// [`AudioOut`]: crate::audio::AudioOut
    pub fn audio_feed(&self, node: NodeId) -> Option<Arc<Mutex<AudioFeed>>> {
        self.players.get(&node)?.as_ref()?.audio.clone()
    }

    /// Some element's audio feed — the one the single output stream binds to.
    ///
    /// One device stream, first feed found: mixing multiple simultaneously-playing videos is a
    /// mixer's job and deliberately out of this tick; the overwhelmingly common page has one
    /// playing video.
    pub fn any_audio_feed(&self) -> Option<Arc<Mutex<AudioFeed>>> {
        self.players
            .values()
            .flatten()
            .find_map(|e| e.audio.clone())
    }

    fn decode(bytes: &[u8]) -> Option<VideoPlayer> {
        let movie = demux(bytes).ok()?;
        let track = movie.tracks.iter().find(|t| t.kind == TrackKind::Video)?;
        VideoPlayer::decode(track, bytes).ok()
    }

    /// The audio half of a decode. `None` — no track, undecodable codec, damaged stream — is a
    /// silently-playing video, never a failed load: audio must not veto a picture that decoded.
    fn decode_audio(bytes: &[u8]) -> Option<manuk_media::Pcm> {
        let movie = demux(bytes).ok()?;
        let track = movie.tracks.iter().find(|t| t.kind == TrackKind::Audio)?;
        manuk_media::decode_track(track, bytes).ok()
    }

    /// Advance every player and push the current picture into the page.
    ///
    /// Returns whether any element's picture actually **changed**, because that is what decides
    /// whether to repaint. A video at 30fps has a new frame every 33ms while a compositor runs at
    /// 60 — pushing and repainting unconditionally would burn a full paint on every other frame to
    /// draw a picture identical to the one already on screen.
    ///
    /// ## `master` is the feed the DEVICE is consuming, and only that feed may own time
    ///
    /// MEDIA.md's A/V-sync rule is audio-is-master: when the output stream is live, the device's
    /// crystal decides what "now" is and the video snaps to it ([`Transport::sync_to_audio`],
    /// tick 250) — advancing the transport by the wall clock beside a running device is two clocks
    /// that visibly part company on any long play. But mastery follows the *device*, not the mere
    /// existence of a feed, and identity is checked by `Arc::ptr_eq` (the same discipline
    /// G_AUDIO_JOIN enforces on the grow cycle) because slaving to a feed the device is NOT
    /// pulling from freezes the picture on its motionless cursor. Two hand-backs to the wall
    /// clock, both load-bearing:
    ///
    /// - **No device** (`master: None` — headless box, no sound hardware, no audio track): the
    ///   wall clock is the honest fallback and behaviour is exactly what it was before the device
    ///   existed.
    /// - **An exhausted or paused master**: an audio track shorter than its video would otherwise
    ///   pin the transport to the end of the sound and freeze the tail of the picture forever.
    ///   When the master stops moving, the wall resumes from wherever the snap left the position.
    ///
    /// [`Transport::sync_to_audio`]: manuk_media::Transport::sync_to_audio
    pub fn advance(
        &mut self,
        dt: f64,
        page: &mut manuk_page::Page,
        master: Option<&Arc<Mutex<AudioFeed>>>,
    ) -> bool {
        let mut changed = false;
        for (&node, slot) in self.players.iter_mut() {
            let Some(entry) = slot.as_mut() else {
                continue; // a known-failed decode; never retried
            };
            // The `muted` attribute reaches the device (tick 352), re-read every frame so
            // setAttribute/removeAttribute takes effect live. `<video autoplay muted>` is THE
            // autoplay pattern of the real web — Chrome only permits autoplay WITH sound when
            // muted — so a device that ignores the attribute blasts audio on pages that were
            // quiet everywhere else. The feed consumes silently rather than pausing (see the
            // `AudioFeed::muted` field note), which is what keeps it a valid sync master.
            // The playback rate (tick 361), re-applied per frame like every live property.
            // `rate_scaled` derives from the requested rate (clamped as the transport clamps)
            // so the chipmunk rule covers AUDIO-ONLY entries too — no transport to consult.
            let rate = self
                .idl_rate
                .get(&node)
                .copied()
                .unwrap_or(1.0)
                .clamp(0.0, 16.0);
            if let Some(p) = entry.player.as_mut() {
                p.set_rate(rate);
            }
            let rate_scaled = rate != 1.0;
            if let Some(a) = entry.audio.as_ref() {
                // The IDL property, once set, IS the live state (tick 360); the attribute is
                // the default it falls back to. Both re-derived per frame so either side's
                // change lands on the next advance. Rate != 1 mutes REGARDLESS (tick 361):
                // there is no time-stretch yet, and pitch-shifted audio is the defect a user
                // hears instantly — silent scaled video is degraded and honest.
                let muted = rate_scaled
                    || match self.idl_muted.get(&node) {
                        Some(&m) => m,
                        None => page
                            .dom()
                            .element(node)
                            .map(|e| e.attr("muted").is_some())
                            .unwrap_or(false),
                    };
                if let Ok(mut f) = a.lock() {
                    f.set_muted(muted);
                    if let Some(&g) = self.idl_volume.get(&node) {
                        f.set_gain(g);
                    }
                }
            }
            // At rate != 1 the device consumes at 1x, so slaving would pin the picture to 1x —
            // the scaled wall governs instead, and the snap-back on returning to rate 1 is
            // CORRECT (the audio position is where the sound is).
            let clock = match (master, entry.audio.as_ref()) {
                _ if rate_scaled => None,
                (Some(m), Some(a)) if Arc::ptr_eq(m, a) => match a.lock() {
                    Ok(f) if f.is_playing() && !f.exhausted() => {
                        let mut c = manuk_media::AudioClock::new(f.sample_rate());
                        c.seek(f.position_seconds());
                        Some(c)
                    }
                    _ => None, // paused/exhausted/poisoned master → the wall clock resumes
                },
                _ => None,
            };
            // Everything below is the VIDEO half; an audio-only entry has no frames and no
            // transport — the device consumes its feed and that IS its playback.
            let Some(player) = entry.player.as_mut() else {
                continue;
            };
            player.tick(dt, clock.as_ref());
            let Some(frame) = player.frame() else {
                continue;
            };
            // Against what was PUBLISHED, never against where the player was a moment ago — see
            // the note on `Entry`. This is what lets the very first frame through on a zero delta.
            if entry.published == Some(frame.presentation_time) {
                continue;
            }
            page.set_video_frame(node, frame.width, frame.height, frame.rgba.clone());
            entry.published = Some(frame.presentation_time);
            changed = true;
        }
        changed
    }

    /// How many elements are being tracked, decoded or not. For diagnostics and the gate.
    pub fn len(&self) -> usize {
        self.players.len()
    }

    pub fn is_empty(&self) -> bool {
        self.players.is_empty()
    }
}

/// Decode the image formats the PAGE cannot (tick 355) — today: AVIF, which rides the same
/// rav1d the `<video>` path uses and therefore must live in the shell lane, not in the page
/// crate every gate binary links. Bytes that fail here simply stay un-rendered, exactly as an
/// undecodable JPEG would; a 10-bit AVIF on the `bitdepth_8` build is an `Err` inside
/// `decode_avif`, never a panic.
pub fn decode_raw_images(
    raws: Vec<(String, Vec<u8>)>,
) -> std::collections::HashMap<String, manuk_paint::DecodedImage> {
    let mut out = std::collections::HashMap::new();
    for (url, bytes) in raws {
        if !manuk_media::sniff_avif(&bytes) {
            continue;
        }
        if let Ok(f) = manuk_media::decode_avif(&bytes) {
            out.insert(
                url,
                manuk_paint::DecodedImage {
                    width: f.width,
                    height: f.height,
                    rgba: f.rgba,
                },
            );
        }
    }
    out
}

/// # G_MEDIA_DRIVE — the frame reaches the screen
///
/// The end of the arc tick 262 left one link short. This drives a **real** fixture through a
/// **real** `Page` and asserts the pixels land in the display list — not that a function was
/// called, and not against a synthetic timeline, because the whole failure being closed is a
/// pipeline that was green at every step and blank at the end.
///
/// ## How each assertion here can go RED
///
/// - **The frame reaches the page.** RED, run: delete the `set_video_frame` call in `advance`.
///   Every player still ticks, every position is still right, `advance` still reports work — and
///   the element paints its poster forever. This is tick 261's caption bug and tick 262's
///   unrequested-movie bug in their third form, and it is invisible to anything that inspects the
///   player rather than the page.
///
/// - **The picture actually CHANGES between two moments.** RED, run: push `frames()[0]` instead of
///   `player.frame()`. Bytes arrive in the display list, the video is visibly "working", and it is
///   a still image.
///
/// - **An unchanged picture does NOT repaint.** RED, run: drop the `presentation_time` guard and
///   always return `true`. Nothing looks wrong on screen; the browser simply burns a full paint per
///   compositor frame to redraw a picture identical to the one already there.
///
/// - **A failed decode is remembered, not retried.** RED, run: `return false` without inserting on
///   the failure path. `has()` stays false forever, the fetch side re-requests every frame, and the
///   browser busy-loops on a broken video in a way that reads as a slow network.
#[cfg(test)]
mod tests {
    use super::*;

    /// Constrained Baseline H.264 + AAC-LC, fragmented — engine/media/tests/data/README.md.
    const AV: &[u8] = include_bytes!("../../engine/media/tests/data/bear-av-baseline_frag.mp4");

    const PAGE_HTML: &str = r#"<!doctype html><html><body>
        <video id="v" width="160" height="120" src="movie.mp4"></video>
      </body></html>"#;

    #[test]
    fn g_media_drive() {
        let fonts = manuk_text::FontContext::new();
        let mut page = manuk_page::Page::load(PAGE_HTML, "https://video.test/", &fonts, 800.0);

        // ── The page asks for the movie (tick 262's link) and names the element it is for.
        let wanted = page.pending_media_urls();
        assert_eq!(
            wanted.len(),
            1,
            "the page must request exactly its one <video src>"
        );
        let (node, url) = wanted[0].clone();
        assert_eq!(url, "https://video.test/movie.mp4");

        let mut set = MediaSet::new();
        assert!(!set.has(node), "nothing has been handed bytes yet");

        // ── BASELINE: what the element looks like with no frame. Without this the "a picture
        //    arrived" claim below is unfalsifiable — every later comparison would be against a
        //    number that has nothing to be different FROM.
        let blank = painted(&page);

        // ── The bytes decode and the element starts playing.
        assert!(set.load(node, AV), "the Baseline fixture must decode");
        assert!(set.has(node), "the element is now tracked");

        // ── First advance: a picture reaches the SCREEN, not merely the player.
        assert!(
            set.advance(0.0, &mut page, None),
            "the first advance publishes a frame"
        );
        let first = painted(&page);
        assert_ne!(
            blank, first,
            "a decoded frame must change what is PAINTED — a driver that ticks the player \
             without calling set_video_frame leaves the element blank forever while every \
             assertion about the player itself stays green"
        );

        // ── An unchanged picture is not republished: no repaint is owed.
        assert!(
            !set.advance(0.0, &mut page, None),
            "advancing by zero holds the same frame and must NOT report a repaint — otherwise \
             the compositor burns a full paint per frame drawing an identical picture"
        );

        // ── Playing on shows a DIFFERENT picture. Half the fixture's ~0.1s crosses a frame
        //    boundary, so this must both report a change and paint different pixels.
        assert!(
            set.advance(0.05, &mut page, None),
            "advancing across a frame boundary must report a new picture"
        );
        let later = painted(&page);
        assert_ne!(
            first, later,
            "playing forward must put a DIFFERENT picture on screen — publishing frames()[0] \
             delivers real bytes and still plays a still image"
        );

        // ── A failed decode is REMEMBERED, so the fetch side never re-requests it.
        let mut broken = MediaSet::new();
        assert!(
            !broken.load(node, b"not a movie"),
            "garbage must not decode"
        );
        assert!(
            broken.has(node),
            "a KNOWN-FAILED decode must be recorded — forgetting it makes the fetch side ask \
             again every frame, a busy-loop that reads from outside as a slow network"
        );
        assert!(
            !broken.advance(1.0, &mut page, None),
            "a failed entry publishes nothing and is never retried"
        );
    }

    /// # G_MSE_JOIN — the bytes a player APPENDS reach the screen (tick 349)
    ///
    /// The other half of `g_media_drive`, for the adaptive-streaming class. There the page *names
    /// a URL* and the host fetches it; here the element's src is a `blob:` URL that no fetch can
    /// serve — the only copy of the media is what `appendBuffer` accumulated inside the page, and
    /// this gate proves that copy crosses to the decoder and paints, end to end:
    ///
    ///   `isTypeSupported` says yes honestly → `addSourceBuffer` → `appendBuffer(real fMP4)` →
    ///   `__msePublish` → `Page::take_mse_media` → `MediaSet::load_mse` → pixels change.
    ///
    /// ## How each claim goes RED (each was run, not assumed)
    ///
    /// - **`its:true` / `open:true`** — revert the built-in `canDecode` matcher in `mse_js.rs`:
    ///   `isTypeSupported` answers false and `addSourceBuffer` throws `NotSupportedError`, which
    ///   is today's shipped behaviour and exactly what keeps every adaptive player on its
    ///   fallback. The registry claim and the join land together or not at all.
    /// - **`one stream`** — delete the `__msePublish` call in `SourceBuffer.__demux`: every piece
    ///   still reports green from inside the page (`buffered` grows, `updateend` fires) and
    ///   `take_mse_media` is empty forever — the exact silent dead-player this gate exists for.
    /// - **byte fidelity** — the stream crosses the JS↔Rust boundary as a one-char-per-byte
    ///   string; a lossy decode anywhere replaces high bytes and the demux downstream rejects a
    ///   stream that was valid when appended (G_MEDIA_SEGMENT_FETCH's hazard, at the *other*
    ///   boundary). Compared byte-for-byte against the fixture.
    /// - **the picture changes** — same discipline as `g_media_drive`: only `painted()` says a
    ///   frame reached the *screen*.
    /// - **an init-only prefix is retried, not remembered dead** — `load_mse` with a truncated
    ///   buffer then the full one: the progressive path's "known failure, never retried" rule
    ///   would kill every real MSE session at its first append.
    #[cfg(feature = "_sm")]
    #[test]
    fn g_mse_join() {
        let bytes_js: String = AV
            .iter()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let html = format!(
            r##"<!doctype html><html><body>
  <video id="v" width="160" height="120"></video>
  <div id="out">-</div>
  <script>
    var R = {{ a: [], push: function (s) {{ this.a.push(s);
      var o = document.getElementById('out'); if (o) {{ o.textContent = this.a.join(' '); }} }} }};
    var TYPE = 'video/mp4; codecs="avc1.42E01E, mp4a.40.2"';
    var BYTES = new Uint8Array([{bytes_js}]);
    try {{
      R.push('its:' + MediaSource.isTypeSupported(TYPE));
      R.push('vp9:' + MediaSource.isTypeSupported('video/webm; codecs="vp9"'));
      R.push('mse-av1:' + MediaSource.isTypeSupported('video/mp4; codecs="av01.0.00M.08"'));
      R.push('cpt-av1:' + (document.getElementById('v').canPlayType('video/mp4; codecs="av01.0.00M.08"') === 'probably'));
      R.push('cpt-webm:' + (document.getElementById('v').canPlayType('video/webm; codecs="av01.0.00M.08"') === ''));
      R.push('cpt-mpeg:' + (document.getElementById('v').canPlayType('audio/mpeg') === 'probably'));
      // Live media-IDL channel (tick 360): the writes a mute button / volume slider perform.
      // Two writes to volume so the host-side coalescing (last per prop) is observable.
      var vv = document.getElementById('v');
      vv.muted = true;
      vv.volume = 0.8;
      vv.volume = 0.25;
      R.push('idl-muted:' + (vv.muted === true));
      R.push('idl-vol:' + (vv.volume === 0.25));
      var v = document.getElementById('v');
      var ms = new MediaSource();
      ms.addEventListener('sourceopen', function () {{
        try {{
          var sb = ms.addSourceBuffer(TYPE);
          R.push('open:true');
          sb.addEventListener('updateend', function () {{
            R.push('appended:true buffered:' + (sb.buffered.length > 0));
          }});
          sb.appendBuffer(BYTES.buffer);
        }} catch (e) {{ R.push('THREW-open:' + e.name); }}
      }});
      v.src = URL.createObjectURL(ms);
    }} catch (e) {{ R.push('THREW:' + e); }}
  </script>
</body></html>"##
        );

        let fonts = manuk_text::FontContext::new();
        let mut page = manuk_page::Page::load(&html, "https://stream.test/", &fonts, 800.0);

        // ── The page-side dance settled during load (the event loop drains to quiescence).
        let root = page.dom().root();
        let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
        let record = page.dom().text_content(out);
        for claim in [
            "its:true",
            "vp9:false",
            // The tick-354 registry flip rides here — one JS test per binary (see G_AV1_DRIVE's
            // doc). RED: revert the av01 arm in mse_js canDecode (av1:false) or re-add av01 to
            // canPlayType's refuse-list (cpt-av1:false).
            // `mse-av1`, not `av1` — an `av1:true` claim is VACUOUS: `contains` finds it
            // inside the `cpt-av1:true` record entry, so the MSE arm could be deleted and the
            // gate stayed green (caught live at t354 by a tripwire print; the claim label must
            // never be a substring of another record entry).
            "mse-av1:true",
            "cpt-av1:true",
            "cpt-webm:true",
            // tick 363: raw MPEG audio plays end-to-end, so canPlayType says so. RED: put mp3
            // back in the refuse regex / delete the audio-mpeg arm.
            "cpt-mpeg:true",
            "idl-muted:true",
            "idl-vol:true",
            "open:true",
            "appended:true",
            "buffered:true",
        ] {
            assert!(
                record.contains(claim),
                "MSE dance must reach `{claim}` — got: {record}"
            );
        }

        // ── The live media-IDL channel (tick 360): both writes crossed, volume COALESCED to
        //    the last value, and a drain is a drain.
        let props = page.take_media_props();
        let video_node = manuk_css::query_selector_all(page.dom(), root, "video")[0];
        assert!(
            props
                .iter()
                .any(|(n, p, v)| *n == video_node && p == "muted" && *v == 1.0),
            "v.muted = true must cross the channel — got {props:?}"
        );
        assert!(
            props
                .iter()
                .any(|(n, p, v)| *n == video_node && p == "volume" && *v == 0.25),
            "v.volume must arrive COALESCED to the last write (0.25, not 0.8) — got {props:?}"
        );
        assert_eq!(
            props.len(),
            2,
            "exactly the two coalesced props — a slider dragged across frames must not deliver \
             every intermediate value: {props:?}"
        );
        assert!(
            page.take_media_props().is_empty(),
            "a drain is a DRAIN — the same writes must not be redelivered every frame"
        );

        // ── The stream crossed to the host, named for the right element, byte for byte.
        let streams = page.take_mse_media();
        assert_eq!(
            streams.len(),
            1,
            "exactly one MSE stream must be published — none is the silent dead player \
             (delete __msePublish in mse_js.rs to see this), more is a coalescing bug"
        );
        let video_node = manuk_css::query_selector_all(page.dom(), root, "video")[0];
        let (node, bytes) = &streams[0];
        assert_eq!(
            *node, video_node,
            "the stream must name the attached <video>"
        );
        assert_eq!(
            bytes.as_slice(),
            AV,
            "the appended stream must survive the JS boundary byte-for-byte — a lossy decode \
             replaces high bytes and downstream demux rejects a valid stream"
        );
        assert!(
            page.take_mse_media().is_empty(),
            "a drain is a DRAIN — the same stream must not be redelivered every frame"
        );

        // ── The stream decodes and the picture reaches the SCREEN.
        let blank = painted(&page);
        let mut set = MediaSet::new();
        assert!(set.load_mse(*node, bytes), "the appended fMP4 must decode");
        assert!(
            set.advance(0.0, &mut page, None),
            "the first advance publishes a frame"
        );
        let first = painted(&page);
        assert_ne!(
            blank, first,
            "a decoded MSE frame must change what is painted"
        );

        // ── A re-publish (the stream GREW) must not restart playback.
        assert!(
            set.advance(0.06, &mut page, None),
            "playing forward crosses a frame boundary"
        );
        let later = painted(&page);
        assert_ne!(first, later, "playing forward paints a different picture");
        assert!(
            set.load_mse(*node, bytes),
            "the re-published stream re-decodes"
        );
        assert!(
            set.advance(0.0, &mut page, None),
            "after a reload the current frame is re-published (the image map may be stale)"
        );
        assert_eq!(
            painted(&page),
            later,
            "a reload must RESUME at the previous position — repainting the FIRST frame here \
             means every appendBuffer restarts the video from zero"
        );

        // ── An undecodable prefix (the init segment alone) is retried when the stream grows.
        let probe = manuk_dom::NodeId(999_999);
        assert!(
            !set.load_mse(probe, &AV[..64]),
            "an init-only prefix cannot decode yet"
        );
        assert!(
            set.load_mse(probe, AV),
            "the grown stream MUST be retried — remembering the prefix as dead kills every \
             real MSE session at its first append"
        );
    }

    /// # G_AUDIO_JOIN — a loaded stream exposes its audio to the device, and a re-decode resumes
    ///
    /// The [`crate::audio`] gate proves the pump is sample-exact; this proves [`MediaSet`]
    /// actually *builds* one from a real load and keeps the device's handle valid across the MSE
    /// grow cycle. The claims and their RED edits:
    ///
    /// - **an A/V load exposes a feed** — drop the `decode_audio` call in `load`: every video
    ///   plays silently forever while the video-side assertions stay green (the exact dead-organ
    ///   failure the MSE join gate exists for, one organ over).
    /// - **the Arc survives a re-decode** — build a fresh `AudioFeed` in `load_mse` instead of
    ///   `replace_pcm` on the carried one: the device stream keeps pulling from the orphaned old
    ///   feed, and every append silences the audio from that moment on. `Arc::ptr_eq` is the only
    ///   observer that can see this.
    /// - **the cursor survives a re-decode** — reset it: every appendBuffer restarts the sound.
    #[test]
    fn g_audio_join() {
        let node = manuk_dom::NodeId(1);
        let mut set = MediaSet::new();
        assert!(set.load(node, AV), "the A/V fixture must decode");
        let feed = set
            .audio_feed(node)
            .expect("an A/V file must expose its audio feed — silent video is the dead organ");
        {
            let mut f = feed.lock().unwrap();
            assert!(f.is_playing(), "autoplay parity with the video side");
            assert!(f.sample_rate() > 0 && f.channels() > 0);
            // Consume some audio so the resume below has a position to lose.
            let mut buf = vec![0f32; 1024];
            assert_eq!(f.fill(&mut buf), 1024, "the feed delivers real samples");
        }
        let pos = feed.lock().unwrap().position_seconds();
        assert!(pos > 0.0);

        // ── The MSE grow cycle: same Arc, position kept.
        assert!(set.load_mse(node, AV), "the re-published stream re-decodes");
        let feed2 = set
            .audio_feed(node)
            .expect("the grown stream still has audio");
        assert!(
            Arc::ptr_eq(&feed, &feed2),
            "a re-decode must mutate the feed the device already holds — a fresh Arc leaves the \
             output stream pulling from an orphan and the audio dies on the first append"
        );
        assert!(
            (feed2.lock().unwrap().position_seconds() - pos).abs() < 1e-9,
            "audio must RESUME across a re-decode, not restart"
        );

        // ── A stream with no decodable audio yields no feed, and does not veto the picture.
        let mut silent = MediaSet::new();
        assert!(!silent.load(node, b"not a movie"), "garbage still fails");
        assert!(silent.audio_feed(node).is_none());
    }

    /// # G_AV_MASTER — the device clock owns time, and only the device's own feed may be master
    ///
    /// The engine gate (`av_sync`, tick 250) proves `Transport::sync_to_audio` snaps; G_AUDIO_PUMP
    /// (tick 350) proves the feed's cursor is sample-exact. This proves the JOIN: `MediaSet::advance`
    /// actually routes the device-bound feed's position into the transport — the wire that was
    /// `None` for a tick, leaving the picture on the wall clock while the device ran on its own
    /// crystal (the lip-sync class, invisible in any short test, guaranteed on a long play).
    ///
    /// ## How each claim goes RED (each was run, not assumed)
    ///
    /// - **audio is master** — pass `None` for the clock inside `advance` (the pre-tick-351 wire):
    ///   the transport follows the wall delta and the snap assertion fails. This is the silent
    ///   two-clocks state: every other media gate stays green.
    /// - **the wall's lie is ignored while mastered** — same edit; a huge `dt` beside an unmoved
    ///   device would run the picture ahead of the sound.
    /// - **identity, not availability** — drop the `Arc::ptr_eq` guard and slave to any feed
    ///   handed in: the imposter (same PCM, wrong Arc — a feed the device is NOT consuming)
    ///   governs the transport, and on a real page a second video's motionless feed freezes the
    ///   first one's picture.
    /// - **an exhausted master hands back to the wall** — drop the `exhausted()` check: audio
    ///   shorter than its video pins the position to the end of the sound and the tail of the
    ///   picture freezes forever.
    #[test]
    fn g_av_master() {
        let fonts = manuk_text::FontContext::new();
        let mut page = manuk_page::Page::load(PAGE_HTML, "https://video.test/", &fonts, 800.0);
        let (node, _) = page.pending_media_urls()[0].clone();

        let mut set = MediaSet::new();
        assert!(set.load(node, AV), "the A/V fixture must decode");
        let feed = set.audio_feed(node).expect("the fixture carries audio");

        // Timings scaled to what the fixture actually holds, so no assertion rides on a guess.
        let video_dur = set.players[&node]
            .as_ref()
            .unwrap()
            .player
            .as_ref()
            .unwrap()
            .transport()
            .duration();
        let movie = manuk_media::demux(AV).unwrap();
        let pcm = manuk_media::decode_track(movie.audio().unwrap(), AV).unwrap();
        let audio_dur = (pcm.samples.len() / pcm.channels as usize) as f64 / pcm.sample_rate as f64;
        let dur = video_dur.min(audio_dur);
        assert!(
            dur > 0.01,
            "the fixture must hold a real timeline, got {dur}s"
        );

        // ── The device consumed real time; the transport SNAPS to it and the wall delta — tiny
        //    or absurd — is ignored while the master governs.
        feed.lock().unwrap().seek_seconds(0.4 * dur);
        let ta = feed.lock().unwrap().position_seconds();
        assert!(ta > 0.0);
        let position = |set: &MediaSet| {
            set.players[&node]
                .as_ref()
                .unwrap()
                .player
                .as_ref()
                .unwrap()
                .transport()
                .position()
        };
        set.advance(0.001, &mut page, Some(&feed));
        assert!(
            (position(&set) - ta).abs() < 1e-9,
            "audio is master: the transport must land exactly on the device position {ta}, got {}",
            position(&set)
        );
        set.advance(500.0, &mut page, Some(&feed));
        assert!(
            (position(&set) - ta).abs() < 1e-9,
            "a wall-clock lie beside an unmoved device must be IGNORED — the picture ran ahead \
             of the sound by {}s",
            position(&set) - ta
        );

        // ── A feed the device is NOT consuming may not govern, however plausible its numbers.
        let imposter = Arc::new(Mutex::new(crate::audio::AudioFeed::new(pcm.clone())));
        imposter.lock().unwrap().seek_seconds(0.8 * dur);
        let dt = 0.05 * dur;
        set.advance(dt, &mut page, Some(&imposter));
        assert!(
            (position(&set) - (ta + dt)).abs() < 1e-6,
            "a non-device feed must NOT be master — the wall clock governs (want {}, got {})",
            ta + dt,
            position(&set)
        );

        // ── The master ran dry (audio shorter than video): the wall clock RESUMES, the tail of
        //    the picture keeps moving.
        {
            let mut f = feed.lock().unwrap();
            let cursor_now = (f.position_seconds() * f.sample_rate() as f64).round() as usize
                * f.channels() as usize;
            let mut short = pcm.clone();
            short.samples.truncate(cursor_now);
            f.replace_pcm(short);
            assert!(f.exhausted(), "the truncated master must read as exhausted");
        }
        let before = position(&set);
        set.advance(dt, &mut page, Some(&feed));
        assert!(
            (position(&set) - (before + dt)).abs() < 1e-6,
            "an exhausted master hands time back to the wall — a frozen tail means the video \
             pinned itself to the end of a shorter audio track (want {}, got {})",
            before + dt,
            position(&set)
        );
    }

    /// # G_MUTED_OUT — the `muted` attribute reaches the device, as silent consumption
    ///
    /// `<video autoplay muted>` is THE autoplay pattern of the real web (Chrome only permits
    /// autoplay WITH sound when muted) — so from the moment the output device landed (t350),
    /// ignoring the attribute means pages that are quiet in every other browser blast audio
    /// here. And mute must be silent CONSUMPTION, not pause: the cursor advances at full rate
    /// under zeros, so the feed remains a valid A/V master and unmute is seamless and in sync.
    ///
    /// ## How each claim goes RED (each was run, not assumed)
    ///
    /// - **the DOM attribute reaches the feed** — delete the sync block in `advance`: both
    ///   videos play loud, and nothing else in the tree notices.
    /// - **muted still consumes** — make the muted branch return without advancing the cursor
    ///   (mute-as-pause): the clock freezes, the mastery rule hands the picture to the wall,
    ///   and unmute resumes STALE audio desynced by the whole muted interval.
    /// - **muted delivers zeros** — keep the copy in the muted branch: the sound leaks and only
    ///   the pre-fouled buffer can see it.
    #[test]
    fn g_muted_out() {
        const TWO: &str = r#"<!doctype html><html><body>
            <video id="q" muted src="quiet.mp4"></video>
            <video id="l" src="loud.mp4"></video>
          </body></html>"#;
        let fonts = manuk_text::FontContext::new();
        let mut page = manuk_page::Page::load(TWO, "https://video.test/", &fonts, 800.0);
        let wanted = page.pending_media_urls();
        let node_of = |suffix: &str| {
            wanted
                .iter()
                .find(|(_, u)| u.ends_with(suffix))
                .map(|(n, _)| *n)
                .expect("both videos request their media")
        };
        let (quiet, loud) = (node_of("quiet.mp4"), node_of("loud.mp4"));

        let mut set = MediaSet::new();
        assert!(set.load(quiet, AV) && set.load(loud, AV));
        let qf = set.audio_feed(quiet).unwrap();
        let lf = set.audio_feed(loud).unwrap();

        // ── The attribute reaches the feed on the next advance — and ONLY where it is present.
        set.advance(0.0, &mut page, None);
        assert!(
            qf.lock().unwrap().is_muted(),
            "a <video muted>'s feed must mute — otherwise every autoplay-muted page on the \
             web plays sound the moment a device exists"
        );
        assert!(
            !lf.lock().unwrap().is_muted(),
            "an unmuted video's feed must NOT mute"
        );

        // ── Muted = silent CONSUMPTION: zeros delivered, position advancing at full rate.
        let movie = manuk_media::demux(AV).unwrap();
        let want = manuk_media::decode_track(movie.audio().unwrap(), AV)
            .unwrap()
            .samples;
        {
            let mut f = qf.lock().unwrap();
            let before = f.position_seconds();
            let mut buf = [f32::NAN; 1024];
            let n = f.fill(&mut buf);
            assert_eq!(n, 1024, "a muted fill still CONSUMES samples");
            assert!(
                buf.iter().all(|&s| s == 0.0),
                "a muted fill must deliver pure silence — anything else is the leak the \
                 pre-fouled buffer exists to catch"
            );
            assert!(
                f.position_seconds() > before,
                "the muted clock must keep RUNNING — mute-as-pause freezes the A/V master and \
                 desyncs the eventual unmute by the whole muted interval"
            );

            // ── Unmute mid-stream: the very next samples are the ones AT the position the
            //    silence reached — sync held, nothing stale.
            f.set_muted(false);
            let mut aud = vec![0f32; 512];
            assert_eq!(f.fill(&mut aud), 512);
            assert_eq!(
                &aud[..],
                &want[1024..1536],
                "unmute must resume at the position the silent consumption reached — getting \
                 want[0..] here means the muted interval never consumed and the audio is late \
                 by exactly that interval"
            );
        }
    }

    /// # G_AV1_DRIVE — AV1 plays end-to-end, and every registry tells the same truth (tick 354)
    ///
    /// t353 built the decoder; this proves the SHELL ships it and that the three honesty
    /// registries flipped in the same tick: a claim of "plays" and the ability to play must land
    /// together (the t349 rule) — a registry ahead of the organ steers players into a hang, a
    /// registry behind it hides a capability that works. The JS-side registry claims (mse
    /// isTypeSupported + canPlayType) ride in `g_mse_join`'s page: ONE JS test per binary — two
    /// mozjs contexts in one test process abort on thread-local teardown (t262 rule,
    /// re-confirmed live this tick).
    ///
    /// ## How each claim goes RED
    ///
    /// - **the picture** — drop `av1` from the shell's manuk-media features: `decoder_for` falls
    ///   through to H264's refusal, `load` returns false, and nothing paints.
    /// - **the `<source type>` fetch** — put `av01` back in `media_type_rejected`'s certain-no
    ///   list: the page skips the source and the request never happens.
    #[test]
    fn g_av1_drive() {
        const AV1: &[u8] = include_bytes!("../../engine/media/tests/data/four-colors-av1.mp4");
        let fonts = manuk_text::FontContext::new();
        let mut page = manuk_page::Page::load(PAGE_HTML, "https://video.test/", &fonts, 800.0);
        let (node, _) = page.pending_media_urls()[0].clone();

        // ── The picture: the AV1 stream decodes in the shell lane and reaches the screen.
        let blank = painted(&page);
        let mut set = MediaSet::new();
        assert!(
            set.load(node, AV1),
            "the AV1 fixture must decode — a false here is the shell lane missing the `av1` \
             feature while the registries claim it plays"
        );
        assert!(
            set.advance(0.0, &mut page, None),
            "the first advance publishes a frame"
        );
        assert_ne!(
            blank,
            painted(&page),
            "a decoded AV1 frame must change what is PAINTED"
        );

        // ── The `<source type>` gate: an av01 source is attempted, not certainly-refused.
        const SRC: &str = r#"<!doctype html><html><body>
            <video id="v" width="160" height="120">
              <source src="clip.mp4" type='video/mp4; codecs="av01.0.00M.08"'>
            </video>
          </body></html>"#;
        let page2 = manuk_page::Page::load(SRC, "https://video.test/", &fonts, 800.0);
        assert!(
            page2
                .pending_media_urls()
                .iter()
                .any(|(_, u)| u.ends_with("clip.mp4")),
            "a <source type=av01> must be REQUESTED — av01 on the certain-no list skips a \
             stream that genuinely plays"
        );
    }

    /// # G_AVIF_PAINT — an AVIF hero image decodes in the shell lane and reaches the page
    ///
    /// The blank-hero-image class: modern CDNs serve AVIF FIRST, so a browser without the
    /// decoder shows a hole where the page's largest picture belongs. The color fixture is
    /// solid red, so the assert is on the PIXELS — a channel swap or a range error paints the
    /// wrong color, not a slightly different one.
    ///
    /// ## How each claim goes RED
    ///
    /// - **the decode** — drop the `insert` in `decode_raw_images`: every AVIF fetch quietly
    ///   vanishes, exactly the silent-drop this gate exists to catch.
    /// - **the sniff** — make `sniff_avif` answer false: same silent vanish, one layer down.
    /// - **the graceful 10-bit no** — asserted by running it: the `bitdepth_8` build must
    ///   answer a 10-bit stream with an empty map, never a panic and never a wrong picture.
    #[test]
    fn g_avif_paint() {
        const RED8: &[u8] =
            include_bytes!("../../engine/media/tests/data/red-full-range-420-8bpc.avif");
        const RED10: &[u8] =
            include_bytes!("../../engine/media/tests/data/red-full-range-420-10bpc.avif");

        // ── The 8-bit still decodes to RED pixels.
        let decoded =
            decode_raw_images(vec![("https://video.test/hero.avif".into(), RED8.to_vec())]);
        let img = decoded
            .get("https://video.test/hero.avif")
            .expect("an 8bpc AVIF must decode in the shell lane");
        assert!(img.width > 0 && img.height > 0);
        let center = ((img.height / 2) * img.width + img.width / 2) as usize * 4;
        let px = &img.rgba[center..center + 4];
        assert!(
            px[0] > 200 && px[1] < 80 && px[2] < 80,
            "the solid-red fixture must decode RED — got {px:?} (a U/V swap paints blue, a \
             range error washes it grey)"
        );

        // ── The 10-bit still is a graceful no on the bitdepth_8 build: empty, not a panic.
        assert!(
            decode_raw_images(vec![("x".into(), RED10.to_vec())]).is_empty(),
            "a 10-bit AVIF must be REFUSED by the 8-bit build — a wrong picture or a panic \
             are both worse than a hole"
        );

        // ── Non-AVIF raw bytes are skipped by the sniff, not fed to the container parser.
        assert!(decode_raw_images(vec![("y".into(), b"GIF89a not avif".to_vec())]).is_empty());

        // ── The join: the decoded map reaches a real page and CHANGES what is painted.
        const HTML: &str = r#"<!doctype html><html><body>
            <img src="hero.avif" width="64" height="64">
          </body></html>"#;
        let fonts = manuk_text::FontContext::new();
        let mut page = manuk_page::Page::load(HTML, "https://video.test/", &fonts, 800.0);
        let blank = painted(&page);
        let filled = page.apply_images_by_url(decoded, &fonts, 800.0);
        assert!(filled > 0, "the <img> must bind the decoded AVIF by URL");
        assert_ne!(
            blank,
            painted(&page),
            "a decoded AVIF must change what is PAINTED — the hero-image hole, closed"
        );
    }

    /// # G_IDL_FEED — live `.muted`/`.volume` writes land on the device feed (tick 360)
    ///
    /// The other end of the `__mediaProp` channel: `MediaSet::apply_prop` must make the write
    /// REAL at the device boundary, with the spec's precedence — the IDL property, once set, is
    /// the live state and the `muted` attribute is only the default it falls back to.
    ///
    /// ## How each claim goes RED
    ///
    /// - **IDL beats the attribute** — drop the `idl_muted` lookup in `advance` (always read the
    ///   attribute): unmuting via the player's button becomes impossible on any `<video muted>`.
    /// - **volume scales the samples** — drop the gain multiply in `AudioFeed::fill`: the slider
    ///   moves, the loudness does not.
    /// - **gain never leaks through mute** — asserted by running it: a muted fill is zeros at
    ///   ANY gain.
    #[test]
    fn g_idl_feed() {
        const TWO: &str = r#"<!doctype html><html><body>
            <video id="q" muted src="quiet.mp4"></video>
            <video id="l" src="loud.mp4"></video>
          </body></html>"#;
        let fonts = manuk_text::FontContext::new();
        let mut page = manuk_page::Page::load(TWO, "https://video.test/", &fonts, 800.0);
        let wanted = page.pending_media_urls();
        let node_of = |suffix: &str| {
            wanted
                .iter()
                .find(|(_, u)| u.ends_with(suffix))
                .map(|(n, _)| *n)
                .unwrap()
        };
        let (attr_muted, plain) = (node_of("quiet.mp4"), node_of("loud.mp4"));

        let mut set = MediaSet::new();
        // ── A prop that arrives BEFORE the bytes (players set .muted at construction).
        set.apply_prop(plain, "muted", 1.0);
        assert!(set.load(attr_muted, AV) && set.load(plain, AV));
        set.advance(0.0, &mut page, None);

        let qf = set.audio_feed(attr_muted).unwrap();
        let lf = set.audio_feed(plain).unwrap();
        assert!(
            lf.lock().unwrap().is_muted(),
            "an IDL mute set BEFORE the media loaded must still land on the feed"
        );

        // ── IDL beats the attribute in the unmute direction: .muted = false on <video muted>.
        set.apply_prop(attr_muted, "muted", 0.0);
        set.advance(0.0, &mut page, None);
        assert!(
            !qf.lock().unwrap().is_muted(),
            "the IDL property, once set, is the LIVE state — the muted attribute is only the \
             default. A player's unmute button is dead on every <video muted> otherwise."
        );

        // ── Volume scales the delivered samples exactly; the position accounting is unchanged.
        let movie = manuk_media::demux(AV).unwrap();
        let want = manuk_media::decode_track(movie.audio().unwrap(), AV)
            .unwrap()
            .samples;
        set.apply_prop(attr_muted, "volume", 0.25);
        set.advance(0.0, &mut page, None);
        {
            let mut f = qf.lock().unwrap();
            let mut buf = vec![f32::NAN; 512];
            assert_eq!(f.fill(&mut buf), 512);
            for (i, (&got, &raw)) in buf.iter().zip(want.iter()).enumerate() {
                assert!(
                    (got - raw * 0.25).abs() < 1e-6,
                    "volume 0.25 must scale sample {i} exactly: got {got}, want {}",
                    raw * 0.25
                );
            }
        }

        // ── Gain never leaks through the mute-silence contract.
        set.apply_prop(attr_muted, "muted", 1.0);
        set.advance(0.0, &mut page, None);
        {
            let mut f = qf.lock().unwrap();
            let mut buf = [f32::NAN; 128];
            assert!(f.fill(&mut buf) > 0, "muted still consumes (t352)");
            assert!(
                buf.iter().all(|&s| s == 0.0),
                "a muted fill is ZEROS at any gain — a gain-scaled leak is still a leak"
            );
        }
    }

    /// # G_RATE — playbackRate scales time, and the audio mutes instead of chipmunking (t361)
    ///
    /// ## How each claim goes RED
    ///
    /// - **time scales** — drop the `dt * self.rate` scaling in `Transport::advance`: 2x plays
    ///   at 1x and every speed control on the web is decorative.
    /// - **the chipmunk rule** — drop `rate_scaled` from the mute derivation in `advance`: at 2x
    ///   the device keeps playing 1x-pitched audio against a 2x picture.
    /// - **mastery refusal** — drop the `rate_scaled => None` arm: the device (still consuming
    ///   at 1x) governs the transport and the picture is pinned to 1x while claiming 2x.
    #[test]
    fn g_rate() {
        let fonts = manuk_text::FontContext::new();
        let mut page = manuk_page::Page::load(PAGE_HTML, "https://video.test/", &fonts, 800.0);
        let (node, _) = page.pending_media_urls()[0].clone();

        let mut set = MediaSet::new();
        assert!(set.load(node, AV));
        let feed = set.audio_feed(node).unwrap();
        let dur = set.players[&node]
            .as_ref()
            .unwrap()
            .player
            .as_ref()
            .unwrap()
            .transport()
            .duration();
        let position = |set: &MediaSet| {
            set.players[&node]
                .as_ref()
                .unwrap()
                .player
                .as_ref()
                .unwrap()
                .transport()
                .position()
        };

        // ── Rate 2: the position advances at 2*dt on the wall path, and the feed mutes even
        //    though nothing ever set .muted (the chipmunk rule).
        set.apply_prop(node, "playbackRate", 2.0);
        let dt = 0.2 * dur;
        set.advance(dt, &mut page, None);
        assert!(
            (position(&set) - 2.0 * dt).abs() < 1e-9,
            "rate 2 must advance the position by 2*dt — got {} want {}",
            position(&set),
            2.0 * dt
        );
        assert!(
            feed.lock().unwrap().is_muted(),
            "rate != 1 must MUTE — there is no time-stretch, and pitch-shifted audio is the \
             defect a user hears instantly"
        );

        // ── At rate != 1 the device may NOT govern: consume the feed and pass it as master;
        //    the scaled wall must win.
        feed.lock().unwrap().seek_seconds(0.1 * dur);
        let before = position(&set);
        set.advance(dt, &mut page, Some(&feed));
        assert!(
            (position(&set) - (before + 2.0 * dt).min(dur)).abs() < 1e-9,
            "a 1x-consuming device must not govern a 2x transport — got {} want {}",
            position(&set),
            (before + 2.0 * dt).min(dur)
        );

        // ── Rate back to 1: mastery restores (snap to the device position — the audio is where
        //    the sound is) and the mute lifts.
        set.apply_prop(node, "playbackRate", 1.0);
        let device_pos = feed.lock().unwrap().position_seconds();
        set.advance(0.001, &mut page, Some(&feed));
        assert!(
            (position(&set) - device_pos).abs() < 1e-9,
            "rate 1 restores audio mastery — the snap to the device position is CORRECT"
        );
        assert!(
            !feed.lock().unwrap().is_muted(),
            "rate 1 lifts the chipmunk mute (nothing else asked for silence)"
        );
    }

    /// # G_MP3_DRIVE — an `<audio src="x.mp3">` becomes a playing audio-only entry (t363)
    ///
    /// The t362 organ's join. The page already REQUESTS the stream (`pending_media_urls` walks
    /// audio elements); this proves the bytes become a device-consumable feed with the live
    /// property machinery attached — and that no frame is ever published for it (an `<audio>`
    /// must not paint).
    ///
    /// ## How each claim goes RED
    ///
    /// - **the fallback** — delete the sniff/stream branch in `load`: the MP3 fails MP4 demux,
    ///   is recorded dead, and every podcast page is silent with a green suite.
    /// - **sample fidelity** — the feed must deliver `decode_audio_stream`'s exact samples; any
    ///   re-decode or format detour shows here.
    /// - **the mute plumbing reaches audio-only entries** — asserted by running it.
    #[test]
    fn g_mp3_drive() {
        const MP3: &[u8] =
            include_bytes!("../../engine/media/tests/data/bear-audio-10s-CBR-no-TOC.mp3");
        const HTML: &str = r#"<!doctype html><html><body>
            <audio id="a" src="pod.mp3"></audio>
          </body></html>"#;
        let fonts = manuk_text::FontContext::new();
        let mut page = manuk_page::Page::load(HTML, "https://pod.test/", &fonts, 800.0);
        let wanted = page.pending_media_urls();
        assert_eq!(wanted.len(), 1, "the <audio src> must be requested");
        let (node, url) = wanted[0].clone();
        assert_eq!(url, "https://pod.test/pod.mp3");

        let mut set = MediaSet::new();
        assert!(
            set.load(node, MP3),
            "an MPEG stream must load as an AUDIO-ONLY entry — failing MP4 demux and giving up \
             is the silent-podcast state this gate exists to catch"
        );
        let feed = set.audio_feed(node).expect("the entry exposes its feed");

        // ── The feed delivers the decoder's exact samples.
        let want = manuk_media::decode_audio_stream(MP3).unwrap().samples;
        {
            let mut f = feed.lock().unwrap();
            assert!(f.is_playing(), "autoplay parity with the video side");
            let mut buf = vec![0f32; 2048];
            assert_eq!(f.fill(&mut buf), 2048);
            assert_eq!(
                &buf[..],
                &want[..2048],
                "the device boundary must see the stream decoder's samples exactly"
            );
        }

        // ── No frame is ever published: an <audio> element must not paint.
        let before = painted(&page);
        assert!(
            !set.advance(0.5, &mut page, None),
            "an audio-only entry publishes no picture"
        );
        assert_eq!(painted(&page), before);

        // ── The live-property machinery reaches audio-only entries.
        set.apply_prop(node, "muted", 1.0);
        set.advance(0.0, &mut page, None);
        assert!(
            feed.lock().unwrap().is_muted(),
            "IDL mute must land on an audio-only feed"
        );
        set.apply_prop(node, "muted", 0.0);
        set.apply_prop(node, "playbackRate", 1.5);
        set.advance(0.0, &mut page, None);
        assert!(
            feed.lock().unwrap().is_muted(),
            "the chipmunk rule covers audio-only entries: rate != 1 with no time-stretch mutes"
        );
    }

    /// **What the viewer would actually see** — the page is painted and the rendered canvas is
    /// returned, rather than any state the driver holds.
    ///
    /// This is the deliberate choice. Reading the frame back out of the driver, or off the player,
    /// asserts that the code believes it published a picture; only painting asserts that a picture
    /// is *on screen*. Six caption ticks were green against the former while the viewer saw nothing.
    /// Every comparison in the gate is between two of these, so a `blank` baseline is taken first
    /// and the claims are all "the picture CHANGED" — falsifiable, unlike "a picture exists".
    fn painted(page: &manuk_page::Page) -> Vec<u8> {
        let fonts = manuk_text::FontContext::new();
        page.paint(&fonts, 400, 200).rgba_bytes().to_vec()
    }
}
