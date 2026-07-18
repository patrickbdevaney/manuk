//! **G_MEDIA_SEGMENT_FETCH — a media segment survives the round trip, byte for byte.**
//!
//! Media step **M2**. Tick 223 built the MSE byte pipe: a player can construct a `MediaSource`,
//! attach it to a `<video>` and `appendBuffer()`. What it appends is the output of *this* path —
//! an `XHR`/`fetch` with `responseType = 'arraybuffer'`, usually over a byte `Range`, because that
//! is how every adaptive player pulls segments. A demuxer (M3) is worthless if the bytes reaching it
//! are not the bytes the server sent.
//!
//! **The specific hazard, and why it must be measured rather than reasoned about.** The fetch
//! boundary carries a response body as a **Rust `&str`** (`Page::resolve_fetch(id, status, body:
//! &str, …)`), and `response.arrayBuffer()` reconstructs the bytes from that string. A media segment
//! is not text: an fMP4 or WebM segment contains every byte value, most of them invalid UTF-8. If
//! anything on that path decodes lossily, the bytes come back **replaced** — and the failure is
//! silent and shaped exactly like a codec bug. The segment arrives, `appendBuffer` accepts it, and
//! the demuxer rejects a stream that was valid when it left the server.
//!
//! So this gate does not ask "does `arrayBuffer()` exist". It sends **all 256 byte values**,
//! including a real EBML/WebM header and a lone `0xFF` (never valid UTF-8), and compares them
//! one by one. `0xEF 0xBF 0xBD` — the replacement character — appearing where a high byte was sent
//! is the signature of this bug, so the gate names it explicitly.
//!
//! `Range` is asserted because segmented delivery depends on it: a player asks for
//! `bytes=4-11`, and a server that ignores it (or a client that never sends it) yields whole-file
//! downloads that still *work* while making adaptive streaming impossible.
//!
//! Whatever this measures becomes the constellation's verdict for `fetch uploads + ranges
//! (streaming)`, currently `unknown`.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use manuk_text::FontContext;

/// The 256-byte probe segment: every byte value 0..=255, prefixed with a real EBML magic so the
/// payload is shaped like the WebM initialization segment a player would actually fetch.
fn segment_bytes() -> Vec<u8> {
    let mut v = vec![0x1A, 0x45, 0xDF, 0xA3];
    v.extend((0u16..=255).map(|b| b as u8));
    v
}

const HTML: &str = r##"<!doctype html>
<html><body>
  <div id="out">-</div>
  <script>
    var R = {
      a: [],
      push: function (s) { this.a.push(s); var o = document.getElementById('out');
                           if (o) { o.textContent = this.a.join(' '); } },
      join: function (sep) { return this.a.join(sep); }
    };

    // The whole segment, then a byte range out of the middle — the two requests a player makes.
    fetch('/seg')
      .then(function (r) { return r.arrayBuffer(); })
      .then(function (buf) {
        var u = new Uint8Array(buf);
        R.push('len:' + u.length);
        // The EBML magic a demuxer looks for first.
        R.push('magic:' + (u[0] === 0x1a && u[1] === 0x45 && u[2] === 0xdf && u[3] === 0xa3));
        // Every byte value, compared one by one. `i & 0xff` is what byte i+4 was sent as.
        var bad = -1;
        for (var i = 0; i < 256; i++) {
          if (u[i + 4] !== (i & 0xff)) { bad = i; break; }
        }
        R.push('allbytes:' + (bad < 0 ? 'true' : 'differs@' + bad + '=' + u[bad + 4]));
        // The replacement-character signature of a lossy UTF-8 decode, called out by name.
        var repl = 0;
        for (var j = 0; j < u.length - 2; j++) {
          if (u[j] === 0xef && u[j + 1] === 0xbf && u[j + 2] === 0xbd) { repl++; }
        }
        R.push('replacement:' + repl);
        return fetch('/seg', { headers: { 'Range': 'bytes=4-11' } });
      })
      .then(function (r) {
        R.push('rangestatus:' + r.status);
        return r.arrayBuffer();
      })
      .then(function (buf) {
        var u = new Uint8Array(buf);
        // bytes 4..11 of the segment are values 0,1,2,3,4,5,6,7.
        var ok = u.length === 8;
        for (var i = 0; i < 8 && ok; i++) { if (u[i] !== i) { ok = false; } }
        R.push('range:' + (ok ? 'true' : 'len=' + u.length + ',b0=' + u[0]));
        R.push('done:true');
      })
      .catch(function (e) { R.push('THREW:' + e); });
  </script>
</body></html>"##;

#[test]
fn a_media_segment_survives_the_fetch_boundary_byte_for_byte() {
    let tmp = std::env::temp_dir().join(format!("manuk-seg-{}", std::process::id()));
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
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                sink.lock().unwrap().push(req.clone());
                let seg = segment_bytes();

                // Honour a single `bytes=a-b` range, as a media origin does.
                let range = req
                    .lines()
                    .find(|l| l.to_ascii_lowercase().starts_with("range:"))
                    .and_then(|l| l.split('=').nth(1).map(|s| s.trim().to_string()));
                let (status, body) = match range {
                    Some(r) => {
                        let mut p = r.split('-');
                        let a: usize = p.next().unwrap_or("0").trim().parse().unwrap_or(0);
                        let b: usize = p.next().unwrap_or("0").trim().parse().unwrap_or(0);
                        let end = (b + 1).min(seg.len());
                        (206u16, seg[a.min(seg.len())..end].to_vec())
                    }
                    None => (200u16, seg),
                };
                let head =
                    format!(
                    "HTTP/1.1 {status} {}\r\nContent-Type: video/webm\r\nAccept-Ranges: bytes\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n",
                    if status == 206 { "Partial Content" } else { "OK" },
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
                // The BYTES entry point, deliberately: handing a media segment across as text is
                // the exact bug this gate was written to catch.
                Ok(r) => page.resolve_fetch_bytes(id, r.status, &r.body, &r.headers, &fonts, 800.0),
                Err(_) => page.resolve_fetch_bytes(id, 0, b"", &[], &fonts, 800.0),
            }
        }
    }

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("SEGMENT PROBE: {got}");
    let sent = log.lock().unwrap().join("\n").to_ascii_lowercase();

    assert!(
        sent.contains("range: bytes=4-11"),
        "the page's Range header must reach the wire — segmented delivery is the whole of adaptive \
         streaming, and a client that never sends Range downloads whole files instead\n{sent}"
    );

    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_MEDIA_SEGMENT_FETCH: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}

/// **Fixed in tick 228; this gate is the ratchet on it.**
///
/// The corruption was real and is measured above: 260 bytes sent, **407 received** — not truncation
/// and not U+FFFD replacement, but UTF-8 **inflation**, because the buffered fetch path carried the
/// body only as text and `arrayBuffer()` re-encoded it (`0xDF` → `0xC3 0x9F`; the `194` was `0xC2`,
/// that lead byte). Every byte below `0x80` survived, which is why JSON/HTML/SSE never noticed and
/// only binary was destroyed.
///
/// The fix carries the body on **two channels**: the host's charset-decoded `text` for
/// `.text()`/`.json()`, and the raw bytes as a one-code-unit-per-byte binary string for
/// `.arrayBuffer()`/`.bytes()`/`.body` and an `arraybuffer` XHR. Neither derives from the other
/// without loss, which is the whole reason there are two.
///
/// **RED, run:** settling the request through the text-only `Page::resolve_fetch` instead of
/// `resolve_fetch_bytes` reproduces `len:407 magic:false allbytes:differs@0=194` exactly.
///
const CLAIMS: &[(&str, &str)] = &[
    (
        "done:true",
        "the whole two-request sequence must complete; anything else means a fetch never resolved",
    ),
    (
        "rangestatus:206",
        "a byte-range request must surface its 206, not be flattened to 200",
    ),
    (
        "range:true",
        "the ranged response must be exactly the requested 8 bytes, with the right values",
    ),
    (
        "len:260",
        "the segment must arrive at its real length; 407 was every byte above 0x7F inflated into two",
    ),
    (
        "magic:true",
        "the EBML magic must arrive intact — it is the first thing a demuxer reads, and the first thing a re-encode destroys",
    ),
    (
        "allbytes:true",
        "every one of the 256 byte values must survive the fetch boundary; a media segment is not text, and altered bytes read as a codec bug that is not a codec bug",
    ),
    (
        "replacement:0",
        "no U+FFFD sequences — those would mean a lossy decode replaced the bytes a re-encode did not inflate",
    ),
];
