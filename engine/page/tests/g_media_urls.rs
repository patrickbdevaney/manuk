//! # G_MEDIA_URLS — the browser finally asks for the movie
//!
//! The missing FIRST link of the media chain. `set_video_frame` paints a decoded frame, the
//! timeline indexes frames by presentation time, the demuxer reads fragmented MP4 — and none of it
//! could ever run on a real page, because `pending_image_urls` reads `<video>`'s **`poster`** and
//! nothing else. A `<video src="movie.mp4">` was not undecodable; it was **never requested**.
//!
//! ## How each assertion here can go RED
//!
//! - **`<video src>` is requested at all.** RED, run: delete the `video`/`audio` arm. This is the
//!   whole defect being closed and nothing else in the tree notices — every media unit test feeds
//!   its bytes from `include_bytes!`, so the entire pipeline stays green while no page ever loads a
//!   video.
//!
//! - **The pair carries the NodeId.** Asserted by the type, and by binding the returned id back
//!   through `set_video_frame` below. A playing video has a position, so two `<video>` elements on
//!   one URL are two independent playbacks — answering by URL alone (as images correctly do) is a
//!   bitmap-shaped answer to a stream-shaped question.
//!
//! - **Source selection SKIPS what cannot decode.** RED, run: take the first `<source>` child
//!   unconditionally. The WebM listed first is fetched, the MP4 two lines below it is never seen,
//!   and the failure surfaces as a broken decoder rather than a broken chooser — the exact
//!   misattribution that costs a tick.
//!
//! - **An unknown `type` is ATTEMPTED, not rejected.** RED, run: invert `media_type_rejected` into
//!   an allow-list. Every stream with an unusual or absent MIME string silently stops loading while
//!   the common cases stay green, which is how a UA breaks media it could have played.
//!
//! - **`src` on the element beats `<source>`.** RED, run: consult the children first. The spec
//!   never looks at `<source>` when the attribute is present.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html>
<html><body>
  <video id="direct" src="movie.mp4" poster="still.png"></video>
  <video id="picky">
    <source src="movie.webm" type="video/webm">
    <source src="movie.mp4" type="video/mp4; codecs=&quot;avc1.42E01E&quot;">
  </video>
  <video id="odd"><source src="stream.bin" type="video/x-something-new"></video>
  <video id="bare"></video>
  <audio id="sound" src="track.m4a"></audio>
  <img src="photo.png">
</body></html>"#;

fn page(fonts: &FontContext) -> manuk_page::Page {
    manuk_page::Page::load(HTML, "https://example.com/watch/", fonts, 800.0)
}

#[test]
fn g_media_urls() {
    let fonts = FontContext::new();
    let p = page(&fonts);
    let wanted = p.pending_media_urls();

    let urls: Vec<&str> = wanted.iter().map(|(_, u)| u.as_str()).collect();

    // ── The direct `src` is requested, resolved against the document base — and the POSTER is not
    // what came back, which is the entire defect.
    assert!(
        urls.contains(&"https://example.com/watch/movie.mp4"),
        "a <video src> must be requested; got {urls:?}"
    );
    assert!(
        !urls.iter().any(|u| u.contains("still.png")),
        "the poster is an image and travels the image path; media wants the MOVIE: {urls:?}"
    );
    assert!(
        !urls.iter().any(|u| u.contains("photo.png")),
        "an <img> is not media: {urls:?}"
    );

    // ── Audio elements are media too.
    assert!(
        urls.contains(&"https://example.com/watch/track.m4a"),
        "an <audio src> must be requested; got {urls:?}"
    );

    // ── Source selection skipped the WebM it cannot demux and reached the MP4 it can.
    let root = p.dom().root();
    let picky = manuk_css::query_selector_all(p.dom(), root, "#picky")[0];
    let chosen = wanted
        .iter()
        .find(|(n, _)| *n == picky)
        .map(|(_, u)| u.as_str());
    assert_eq!(
        chosen,
        Some("https://example.com/watch/movie.mp4"),
        "resource selection must SKIP the undecodable WebM and take the MP4 below it"
    );

    // ── An unrecognised type is attempted: the sniffer downstream is the honest authority.
    assert!(
        urls.contains(&"https://example.com/watch/stream.bin"),
        "an unknown MIME must be ATTEMPTED, never pre-rejected by a string table: {urls:?}"
    );

    // ── A <video> with no src and no usable <source> asks for nothing.
    assert!(
        !urls.iter().any(|u| u.ends_with("/watch/")),
        "an empty <video> must not request its own document: {urls:?}"
    );

    // ── The NodeId in the pair is the key `set_video_frame` wants: the round trip closes here.
    let mut p2 = page(&fonts);
    let (node, _) = p2
        .pending_media_urls()
        .into_iter()
        .next()
        .expect("at least one media request");
    p2.set_video_frame(node, 2, 2, vec![7u8; 2 * 2 * 4]);
}

/// **The outcome bridge, through the RUST entry point the shell actually calls.**
///
/// `video.error` used to be an eager `MediaError(4)` on every media element. That was the honest
/// signal while nothing could decode, and it became a lie contradicting `canPlayType` the moment
/// tick 263 made Baseline MP4 play — a player that checks `error` first (most do) gave up on a
/// video that was about to work. **Neither fixed value is honest:** an eager error abandons video
/// that works, and a permanent `null` hangs on video that does not, showing a dead player where a
/// fallback belonged. Only the real outcome is, and the host is the only layer that has it.
///
/// ## How each assertion here can go RED
///
/// - **Spec-initial `null`.** RED, run: restore the eager `MediaError(4)`. Every player gives up
///   before the shell has even fetched the file.
/// - **A failed decode REACHES the element.** RED, run: make `set_media_outcome` a no-op, or have
///   it eval against `node` instead of `node.0`. The JS half stays green in the conformance suite
///   and no real page ever hears a word — the two halves gated separately and never joined.
/// - **A success CLEARS it.** RED, run: report only failures. Every element that recovered stays
///   permanently errored, which is the half a "report the error" bridge silently drops.
#[test]
fn g_media_outcome_bridge() {
    let fonts = FontContext::new();
    let mut p = manuk_page::Page::load(
        r#"<!doctype html><html><body>
             <video id="v" src="movie.mp4"></video><div id="out">-</div>
             <script>window.__ready = 1;</script>
           </body></html>"#,
        "https://video.test/",
        &fonts,
        800.0,
    );
    let (node, _) = p
        .pending_media_urls()
        .into_iter()
        .next()
        .expect("the <video src> is requested");

    let read = |p: &mut manuk_page::Page| -> String {
        p.eval_for_test(
            "(function(){var v=document.getElementById('v');               document.getElementById('out').textContent =                  v.error === null ? 'null' : String(v.error.code);})()",
        );
        let root = p.dom().root();
        let out = manuk_css::query_selector_all(p.dom(), root, "#out")[0];
        p.dom().text_content(out)
    };

    assert_eq!(
        read(&mut p),
        "null",
        "spec-initial: no load has been ATTEMPTED, so there is nothing to report"
    );
    p.set_media_outcome(node, false);
    assert_eq!(
        read(&mut p),
        "4",
        "a host-reported FAILED decode must reach the element as MEDIA_ERR_SRC_NOT_SUPPORTED —          this is what lets a site show its fallback instead of a dead player"
    );
    p.set_media_outcome(node, true);
    assert_eq!(
        read(&mut p),
        "null",
        "...and a successful outcome must CLEAR it; a bridge that only reports failure leaves          every recovered element permanently errored"
    );
}
