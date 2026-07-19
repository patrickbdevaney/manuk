//! **G_MEDIA_BUFFERED — a player appends a real segment and `buffered` answers.**
//!
//! Media step **M3**, at the surface a page actually touches.
//!
//! **The failure this gate exists for.** Every piece of the MSE byte pipe worked and the whole
//! thing was still inert. A page could construct a `MediaSource`, attach it to a `<video>`, fetch a
//! segment byte-exactly (t227/t228) and `appendBuffer` it — and `sb.buffered.length` was `0`,
//! because nothing had ever looked at the bytes. `SourceBuffer.__chunks` described itself as "a
//! faithful record of what the page handed us and nothing more".
//!
//! That zero is not cosmetic. **`buffered` is the variable an adaptive player's fetch loop steers
//! by**: it appends a segment, reads how far its buffer now reaches, and decides what to request
//! next. A `buffered` that never advances is a loop that never advances — the player either
//! re-fetches the same segment forever or stalls. So the byte pipe could be perfect and no
//! streaming site would progress past its first segment.
//!
//! **What is asserted.** A *real* fragmented MP4 — the form MSE streams — served over a real
//! socket, fetched by page JS as an `ArrayBuffer`, appended, and then read back through the public
//! API only: `sb.buffered.length`, `.start(0)`, `.end(0)`, `sb.videoTracks`. The timeline values
//! are checked against the container's own arithmetic (2002 and 5005 ticks at 30000 Hz), so this
//! measures a demux of those bytes and not a plausible-looking constant.
//!
//! **What is deliberately NOT asserted: that anything can be decoded or played.** No frame is
//! produced here and none is claimed. `MediaSource.isTypeSupported` still answers `false` from the
//! empty `__mseCodecs` registry, which is the discipline `docs/loop/MEDIA.md` insists on —
//! advertising MSE support we cannot honour turns a working YouTube into a black rectangle. This
//! gate asserts that too, so the demuxer landing cannot silently start over-promising.
//!
//! **RED, run:** making `SourceBuffer.prototype.__demux` return immediately reproduces the original
//! failure exactly — `ranges:0 start:- end:-`, the inert pipe.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use manuk_text::FontContext;

/// A real encoded fragmented MP4: H.264 640×360, two frames at a 30000 timescale. Checked in at
/// `engine/media/tests/data` — see the README there on why the fixtures are real files and not
/// synthesised ones.
fn segment() -> Vec<u8> {
    let p = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../media/tests/data/bear-640x360-v-2frames_frag.mp4"
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

    // The honesty check, asserted BEFORE anything else: a demuxer that can find the H.264 in a
    // file is not a decoder, and isTypeSupported must keep saying so.
    R.push('claims:' + MediaSource.isTypeSupported('video/mp4; codecs="avc1.64001E"'));

    var ms = new MediaSource();
    var v = document.getElementById('v');
    v.src = URL.createObjectURL(ms);

    ms.addEventListener('sourceopen', function () {
     try {
      R.push('sourceopen:true');
      // **The decoder-registration seam, used exactly as g_mse uses it.** `addSourceBuffer` refuses
      // a type nothing can decode — correctly — so a demux gate has to register the type to get a
      // buffer at all. The honesty assertion above was taken BEFORE this line: what is being
      // proven is that the *engine* does not volunteer support, not that this page cannot opt in.
      __mseCodecs.push('video/mp4; codecs="avc1.64001E"');
      var sb = ms.addSourceBuffer('video/mp4; codecs="avc1.64001E"');
      // Before any bytes: honestly empty, which is the state this gate is ratcheting away from.
      R.push('empty:' + (sb.buffered.length === 0));

      fetch('/seg')
        .then(function (r) { return r.arrayBuffer(); })
        .then(function (buf) {
          R.push('bytes:' + buf.byteLength);
          sb.addEventListener('updateend', function () {
            var b = sb.buffered;
            R.push('ranges:' + b.length);
            if (b.length > 0) {
              // Reported to 4dp so the assertion is on the container's arithmetic, not on float
              // formatting. 2002/30000 = 0.0667, 5005/30000 = 0.1668.
              R.push('start:' + b.start(0).toFixed(4));
              R.push('end:' + b.end(0).toFixed(4));
            } else {
              R.push('start:- end:-');
            }
            R.push('vtracks:' + sb.videoTracks.length);
            R.push('codec:' + (sb.videoTracks[0] ? sb.videoTracks[0].codec : '-'));
            R.push('dims:' + (sb.videoTracks[0]
                     ? sb.videoTracks[0].width + 'x' + sb.videoTracks[0].height : '-'));
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
fn a_real_segment_is_demuxed_and_buffered_reports_it() {
    let tmp = std::env::temp_dir().join("manuk-g-media-buffered");
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
                    "HTTP/1.1 200 OK\r\nContent-Type: video/mp4\r\nAccept-Ranges: bytes\r\n\
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
    println!("MEDIA BUFFERED PROBE: {got}");

    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_MEDIA_BUFFERED: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}

const CLAIMS: &[(&str, &str)] = &[
    (
        "claims:false",
        "isTypeSupported must still say no. A demuxer finds the H.264 in the file; it does not \
         decode it. Saying yes here is what makes YouTube serve a stream we cannot play instead of \
         the progressive fallback we can — the one failure MEDIA.md calls out by name",
    ),
    (
        "sourceopen:true",
        "the MediaSource must actually attach to the element; without the handshake nothing below \
         this line ever runs and every other claim would be vacuously absent rather than false",
    ),
    (
        "empty:true",
        "before any append, buffered must be empty — a gate whose 'after' state is also its \
         'before' state proves nothing",
    ),
    (
        "bytes:18659",
        "the whole fixture must arrive byte-exact; a short read would demux to a different \
         timeline and the range assertions below would be measuring the truncation",
    ),
    (
        "ranges:1",
        "THE GATE. Two abutting frames are one contiguous range. 0 is the inert pipe this tick \
         exists to fix; 2 would mean the gap tolerance is not merging the stream's own 33ms \
         interior seam and a player would read its buffer as unplayable swiss cheese",
    ),
    (
        "start:0.0667",
        "2002 ticks at 30000Hz. NOT zero: this fixture carries a two-frame composition offset, so \
         its media genuinely begins two frames in, and a demuxer that normalised that away would \
         be discarding a real timestamp — in MSE the offset is how a segment appended at minute \
         three reports minute three",
    ),
    (
        "end:0.1668",
        "5005 ticks at 30000Hz — the last frame's presentation time plus its duration",
    ),
    (
        "vtracks:1",
        "the container's track list must reach the page; a player reads it to decide whether it \
         still needs an audio SourceBuffer",
    ),
    (
        "codec:avc1.64001E",
        "the exact RFC 6381 string, read out of the avcC. A track count that is right with a codec \
         that is wrong looks identical until a real player string-compares it",
    ),
    (
        "dims:640x360",
        "the dimensions the video element must size itself to, taken from the container rather \
         than assumed",
    ),
    (
        "done:true",
        "the whole append sequence must complete; anything else means updateend never fired and \
         the assertions above read a half-finished probe",
    ),
];
