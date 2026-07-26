//! **G_MEDIA_WEBM — a page appends a real WebM segment and the timeline answers.**
//!
//! Container step **M3b**, at the surface a page actually touches. `engine/media/tests/webm_demux.rs`
//! gates the demuxer against the bytes; this gates the seam between it and JavaScript.
//!
//! **The failure this gate exists for.** `manuk_media::demux` answered `Unsupported(WebM)` for every
//! EBML stream, and `MediaSource.isTypeSupported` refused every WebM type, so `addSourceBuffer`
//! threw `NotSupportedError` and there was no door into the demuxer at all. WebM is what YouTube
//! ships and what most non-MP4 `<video>` on the open web is.
//!
//! **The line this gate is really here to hold.** Demuxing a container and decoding its codec are
//! different claims, and `docs/loop/MEDIA.md` names conflating them as the failure that turns a
//! working YouTube into a black rectangle. So three assertions sit *next to each other* on purpose:
//!
//! * `MediaSource.isTypeSupported('video/webm')` → **true**. The bare container form, which means
//!   what it has always meant for `video/mp4`: we can open this. It is what Chrome answers and it
//!   is the only door to `__demux`.
//! * `MediaSource.isTypeSupported('video/webm; codecs="vp9,opus"')` → **false**. There is no VP9
//!   decoder and no Opus decoder in this tree.
//! * `video.canPlayType('video/webm')` → **''**. Unmoved, and this is the ratchet clause: if it said
//!   otherwise, a `<video>` listing a `.webm` `<source>` before its `.mp4` one would select the
//!   WebM we cannot decode over the MP4 we can. That is a regression traded for a capability, and
//!   trades are refused.
//!
//! **What is asserted about the demux itself** — through the public API only, and against the
//! container's own arithmetic rather than constants someone observed: two tracks with their real
//! codec strings and dimensions, one contiguous `buffered` range covering the whole ~2.74s, and a
//! `MediaSource.duration` that arrived from the file's `Info` element.
//!
//! **Note that this gate needs no `__mseCodecs.push`.** `g_media_buffered` has to register its type
//! by hand to get a SourceBuffer at all, because `addSourceBuffer` refuses a type nothing can
//! decode. Here the bare container form is genuinely supported, so the door opens on its own merits
//! — which is the difference between a reachable capability and machinery no page can call.
//!
//! **RED, run at tick 633:** restoring `if (/^(video|audio)\/webm$/.test(want)) { return true; }` to
//! `return false` fails `open:` and every claim below it — `addSourceBuffer` throws
//! `NotSupportedError` and the page never reaches a byte. Restoring
//! `Container::WebM => Err(Unsupported(WebM))` in `manuk_media::demux` leaves the SourceBuffer open
//! and every timeline claim empty: `ranges:0 vtracks:0`, the inert pipe.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use manuk_text::FontContext;

/// A real encoded WebM: VP9 320×240 video and Opus 48kHz stereo audio — the pair YouTube actually
/// serves. Checked in at `engine/media/tests/data`; see the README there on why the fixtures are
/// real encoder output and not synthesised.
fn segment() -> Vec<u8> {
    let p = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../media/tests/data/bear-vp9-opus.webm"
    );
    std::fs::read(p).unwrap_or_else(|e| panic!("fixture: {e}"))
}

const HTML: &str = r##"<!doctype html>
<html><body>
  <video id="v"></video>
  <div id="out">-</div>
  <script>
    var R = {
      a: [],
      push: function (s) { this.a.push(s); var o = document.getElementById('out');
                           if (o) { o.textContent = this.a.join(' '); } },
    };

    // ── The three claims, taken BEFORE anything else and deliberately adjacent. Opening a
    //    container is not decoding a codec, and the gap between these lines is the whole discipline.
    R.push('bare:' + MediaSource.isTypeSupported('video/webm'));
    R.push('vp9:' + MediaSource.isTypeSupported('video/webm; codecs="vp9"'));
    R.push('vp9opus:' + MediaSource.isTypeSupported('video/webm; codecs="vp9,opus"'));
    R.push('canplay:[' + document.getElementById('v').canPlayType('video/webm') + ']');

    // ── The AV1 line (tick 634). AV1 is the OTHER codec WebM carries, it has decoded here since
    //    t354, and both answers said no — a false absence, not a conservative one. These sit next
    //    to the vp9 lines above on purpose: the SAME container, one codec yes and one no, is what
    //    proves the answer is about the decoder rather than about the file extension.
    R.push('av1:' + MediaSource.isTypeSupported('video/webm; codecs="av01.0.01M.08"'));
    R.push('av1opus:' + MediaSource.isTypeSupported('video/webm; codecs="av01.0.01M.08,opus"'));
    R.push('bareav01:' + MediaSource.isTypeSupported('video/webm; codecs="av01"'));
    R.push('cpav1:[' + document.getElementById('v').canPlayType('video/webm; codecs="av01.0.01M.08"') + ']');
    R.push('cpvp9:[' + document.getElementById('v').canPlayType('video/webm; codecs="vp9"') + ']');

    var ms = new MediaSource();
    var v = document.getElementById('v');
    v.src = URL.createObjectURL(ms);

    ms.addEventListener('sourceopen', function () {
     try {
      R.push('sourceopen:true');
      // No `__mseCodecs.push` — the bare container form is supported on its own merits.
      var sb = ms.addSourceBuffer('video/webm');
      R.push('open:true');
      R.push('empty:' + (sb.buffered.length === 0));

      fetch('/seg')
        .then(function (r) { return r.arrayBuffer(); })
        .then(function (buf) {
          R.push('bytes:' + buf.byteLength);
          sb.addEventListener('updateend', function () {
            var b = sb.buffered;
            R.push('ranges:' + b.length);
            if (b.length > 0) {
              R.push('start:' + b.start(0).toFixed(2));
              R.push('end:' + b.end(0).toFixed(2));
            } else {
              R.push('start:- end:-');
            }
            R.push('vtracks:' + sb.videoTracks.length);
            R.push('atracks:' + sb.audioTracks.length);
            R.push('vcodec:' + (sb.videoTracks[0] ? sb.videoTracks[0].codec : '-'));
            R.push('acodec:' + (sb.audioTracks[0] ? sb.audioTracks[0].codec : '-'));
            R.push('dims:' + (sb.videoTracks[0]
                     ? sb.videoTracks[0].width + 'x' + sb.videoTracks[0].height : '-'));
            R.push('rate:' + (sb.audioTracks[0] ? sb.audioTracks[0].sampleRate : '-'));
            R.push('duration:' + (ms.duration > 0 ? ms.duration.toFixed(2) : '-'));
            R.push('done:true');
          });
          sb.appendBuffer(buf);
        })
        .catch(function (e) { R.push('threw:' + e); });
     } catch (e) { R.push('threw:' + (e && e.name ? e.name : e)); }
    });
  </script>
</body></html>"##;

#[test]
fn a_real_webm_segment_is_demuxed_and_the_timeline_answers() {
    let tmp = std::env::temp_dir().join("manuk-g-media-webm");
    std::fs::create_dir_all(&tmp).ok();
    unsafe { std::env::set_var("MANUK_STATE", &tmp) };

    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let sink = log.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut sock) = stream else { continue };
            let sink = sink.clone();
            std::thread::spawn(move || {
                let mut buf = vec![0u8; 8192];
                let n = sock.read(&mut buf).unwrap_or(0);
                sink.lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&buf[..n]).to_string());
                let body = segment();
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: video/webm\r\nAccept-Ranges: bytes\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = sock.write_all(head.as_bytes());
                let _ = sock.write_all(&body);
                let _ = sock.flush();
            });
        }
    });

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let base = format!("http://{addr}/watch");
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, &base, &fonts, 800.0);

    let base_url = url::Url::parse(&base).unwrap();
    for _ in 0..8 {
        let reqs = page.take_fetches();
        if reqs.is_empty() {
            break;
        }
        for (id, raw_url, method, headers, body) in reqs {
            let abs = base_url.join(&raw_url).expect("resolvable URL");
            let hdrs: Vec<(&str, &str)> = headers
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            match rt.block_on(manuk_net::request_from(
                &method,
                abs.as_str(),
                &hdrs,
                body.into(),
                Some(&base),
            )) {
                Ok(r) => page.resolve_fetch_bytes(id, r.status, &r.body, &r.headers, &fonts, 800.0),
                Err(_) => page.resolve_fetch_bytes(id, 0, b"", &[], &fonts, 800.0),
            }
        }
    }

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("MEDIA WEBM PROBE: {got}");

    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_MEDIA_WEBM: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}

const CLAIMS: &[(&str, &str)] = &[
    (
        "bare:true",
        "the bare container form is the only door into the demuxer — `addSourceBuffer` consults \
         `isTypeSupported`, so a `false` here makes the whole WebM path unreachable from a page \
         however good the demuxer is",
    ),
    (
        "vp9:false",
        "there is no VP9 decoder in this tree. Saying yes to a codecs= string is what makes an \
         adaptive player choose a stream we cannot play instead of one we can — the black-rectangle \
         failure MEDIA.md names",
    ),
    (
        "vp9opus:false",
        "and no Opus decoder either. This is the claim that must not move when the demuxer lands",
    ),
    (
        "canplay:[]",
        "`canPlayType` on the BARE form must stay the empty string — the spec's 'no'. Bare webm on \
         the open web is overwhelmingly VP9+Opus, so if this moved, a <video> listing an \
         unqualified .webm <source> before its .mp4 one would select a file we cannot decode over \
         the MP4 we can: a REGRESSION traded for a capability, which the ratchet refuses",
    ),
    // ── The tick-634 AV1 line. Every one of these is in the SAME container as the vp9 claims
    //    above, which is what makes them evidence about the decoder rather than about the file.
    (
        "av1:true",
        "AV1-in-WebM decodes end-to-end here (G_MEDIA_WEBM_AV1 pushes real EBML samples through \
         re_rav1d to real 480x360 pictures) and both answers used to say no. A capability that \
         works and reports absent is a false absence — the class this loop keeps catching — and \
         `addSourceBuffer` consulting this is what makes it reachable from an adaptive player",
    ),
    (
        "av1opus:false",
        "a MIXED list is refused: we would render the video and silently drop the Opus audio, \
         which is not playing the file. This is the assertion that stops `av1:true` from being \
         read as `webm:true`",
    ),
    (
        "bareav01:false",
        "the bare `av01` string (what a WebM AV1 track reports when its CodecPrivate is not a \
         readable av1C) is refused, because `manuk_media::av1::can_decode` refuses it too. Both \
         sides must compute the SAME QUANTITY or the claim describes something other than the \
         capability behind it",
    ),
    (
        "cpav1:[probably]",
        "canPlayType is what decides which <source> a <video> selects, so it — not \
         isTypeSupported — is the answer that gates playback for an ordinary media element. \
         'probably' is the spec's word for `the codecs were NAMED and we have them`",
    ),
    (
        "cpvp9:[]",
        "and canPlayType still says no to VP9 in the same container, one line apart. NOTE what \
         actually holds this, because the RED probe corrected me: the codec refuse-list in \
         `canPlayType` catches `vp9` BEFORE the webm arm is reached, so this claim is guarded by \
         that list and not by the shared predicate — mutating the predicate to accept everything \
         leaves this line green. It is defence in depth, and the honest reading of it is `the \
         refuse-list still runs first`, not `the predicate refuses VP9`",
    ),
    (
        "sourceopen:true",
        "the MediaSource must attach to the element; without the handshake nothing below runs and \
         every claim after it would be vacuously absent rather than false",
    ),
    (
        "open:true",
        "`addSourceBuffer('video/webm')` must succeed with NO `__mseCodecs.push` escape hatch — \
         that is the difference between a reachable capability and machinery no page can call",
    ),
    (
        "empty:true",
        "before any bytes, `buffered` is honestly empty — the state this gate ratchets away from",
    ),
    (
        "bytes:101414",
        "the whole fixture reached page JS as an ArrayBuffer. A short read would make every \
         timeline claim below a measurement of a truncated file",
    ),
    (
        "ranges:1",
        "ONE contiguous span. Matroska stores almost no frame durations, so without the derivation \
         every sample span is empty and this is 0 — and a player reading 0 re-fetches media it \
         already holds, forever",
    ),
    (
        "start:0.00",
        "the stream starts at zero, from the container's own first block timestamp",
    ),
    (
        "end:2.74",
        "and runs to ~2.74s. This is the union of the video and audio timelines, so it also proves \
         the two tracks were placed on ONE timeline rather than two",
    ),
    (
        "vtracks:1",
        "the video track was found by kind, not by position",
    ),
    ("atracks:1", "and so was the audio track"),
    (
        "vcodec:vp9",
        "the SHORT form, because that is all the container carries. A fabricated `vp09.00.10.08` is \
         a string a player string-compares against isTypeSupported and branches on",
    ),
    ("acodec:opus", "read from the track's CodecID, not guessed from the container"),
    (
        "dims:320x240",
        "real dimensions out of the track's Video element — an aspect-ratio box a page sizes from",
    ),
    (
        "rate:48000",
        "the Opus sampling frequency, which is a float in Matroska and an integer here",
    ),
    (
        "duration:2.74",
        "`MediaSource.duration` came from the Segment's Info element. It is NaN until something \
         knows better, and a demuxed WebM now knows better",
    ),
    (
        "done:true",
        "the updateend handler ran to completion — without this the absence of a failing claim \
         would only mean the callback never fired",
    ),
];
