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

use manuk_dom::NodeId;
use manuk_media::{demux, TrackKind, VideoPlayer};

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
    player: VideoPlayer,
    /// Presentation time of the frame currently handed to the page. `None` = nothing sent yet.
    published: Option<f64>,
}

impl MediaSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Drop everything. Called on navigation: a player is bound to `NodeId`s in a DOM that no longer
    /// exists, and a stale entry would hand the *next* page's node the previous page's video.
    pub fn clear(&mut self) {
        self.players.clear();
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
        let player = Self::decode(bytes);
        let ok = player.is_some();
        if let Some(mut p) = player {
            p.play();
            self.players.insert(
                node,
                Some(Entry {
                    player: p,
                    published: None,
                }),
            );
        } else {
            // Remembered as a known failure. See the module note.
            self.players.insert(node, None);
        }
        ok
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
            Some(Some(e)) => Some((
                e.player.transport().position(),
                e.player.transport().is_playing(),
            )),
            _ => None,
        };
        let Some(mut p) = Self::decode(bytes) else {
            self.players.insert(node, None);
            return false;
        };
        match resume {
            Some((pos, playing)) => {
                p.seek(pos);
                if playing {
                    p.play();
                } else {
                    p.pause();
                }
            }
            None => p.play(), // same autoplay rationale as `load`
        }
        // `published: None` so the current frame is re-pushed even if its presentation time
        // matches — the page's image map may never have seen this player's pixels.
        self.players.insert(
            node,
            Some(Entry {
                player: p,
                published: None,
            }),
        );
        true
    }

    fn decode(bytes: &[u8]) -> Option<VideoPlayer> {
        let movie = demux(bytes).ok()?;
        let track = movie.tracks.iter().find(|t| t.kind == TrackKind::Video)?;
        VideoPlayer::decode(track, bytes).ok()
    }

    /// Advance every player by a wall-clock delta and push the current picture into the page.
    ///
    /// Returns whether any element's picture actually **changed**, because that is what decides
    /// whether to repaint. A video at 30fps has a new frame every 33ms while a compositor runs at
    /// 60 — pushing and repainting unconditionally would burn a full paint on every other frame to
    /// draw a picture identical to the one already on screen.
    ///
    /// The `None` audio clock is the honest state of the tree: `cpal` is unbound, nothing plays
    /// sound, so there is no device clock to be master and the wall clock is correctly the fallback
    /// [`VideoPlayer::tick`] selects. When audio output lands, the clock is threaded in here and
    /// nothing else in this file moves.
    pub fn advance(&mut self, dt: f64, page: &mut manuk_page::Page) -> bool {
        let mut changed = false;
        for (&node, slot) in self.players.iter_mut() {
            let Some(entry) = slot.as_mut() else {
                continue; // a known-failed decode; never retried
            };
            entry.player.tick(dt, None);
            let Some(frame) = entry.player.frame() else {
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
            set.advance(0.0, &mut page),
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
            !set.advance(0.0, &mut page),
            "advancing by zero holds the same frame and must NOT report a repaint — otherwise \
             the compositor burns a full paint per frame drawing an identical picture"
        );

        // ── Playing on shows a DIFFERENT picture. Half the fixture's ~0.1s crosses a frame
        //    boundary, so this must both report a change and paint different pixels.
        assert!(
            set.advance(0.05, &mut page),
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
            !broken.advance(1.0, &mut page),
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
            "open:true",
            "appended:true",
            "buffered:true",
        ] {
            assert!(
                record.contains(claim),
                "MSE dance must reach `{claim}` — got: {record}"
            );
        }

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
            set.advance(0.0, &mut page),
            "the first advance publishes a frame"
        );
        let first = painted(&page);
        assert_ne!(
            blank, first,
            "a decoded MSE frame must change what is painted"
        );

        // ── A re-publish (the stream GREW) must not restart playback.
        assert!(
            set.advance(0.06, &mut page),
            "playing forward crosses a frame boundary"
        );
        let later = painted(&page);
        assert_ne!(first, later, "playing forward paints a different picture");
        assert!(
            set.load_mse(*node, bytes),
            "the re-published stream re-decodes"
        );
        assert!(
            set.advance(0.0, &mut page),
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
