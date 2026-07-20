//! **G_TRACK_SRC — `<track src>` is fetched, parsed by the real parser, and becomes a live track.**
//!
//! The join the caption work has been missing since tick 255. Three ticks built three correct
//! pieces — the WebVTT parser (255), the `TextTrack` API (256), the `cuechange` timeline (257) — and
//! left **no path between them**: the parser had no caller outside its own unit tests, and the only
//! cues a page could ever hold were ones its own JavaScript constructed with `new VTTCue`.
//!
//! That covers hls.js and dash.js, and it covers **nothing else**. A news clip, a course video, a
//! documentation screencast, a `<video>` in a wiki article all ship
//! `<track kind=subtitles src="subs.vtt" default>` and expect the BROWSER to load it. For those,
//! every piece we had built was unreachable, and captions were as absent as before tick 255.
//!
//! ## The limit this tick MEASURED, and did not remove
//!
//! The load is swept from the document, not driven by the page — but a document with **no `<script>`
//! at all never gets a JS context**, which was measured here rather than assumed (a probe page
//! without a script could not evaluate a single expression; adding one line of JS made every piece
//! appear at once). So the honest claim is: `<track src>` loads on any page that runs *some*
//! JavaScript, which is essentially every real video page, and does **not** load on a fully static
//! one. That is a context-creation policy question, not a caption question, and it is left alone
//! deliberately — creating a JS realm for every static document to service a `<track>` would trade a
//! large, universal cost for a narrow case. It is written down rather than papered over.
//!
//! ## How each assertion here can go RED
//!
//! - **The file is FETCHED.** RED, run: before this tick nothing ever requested a `<track>`'s `src`,
//!   so the mock server's request log is empty and every count below is zero.
//! - **The sweep is document-driven, not reflection-driven.** The load must not hang off
//!   `__manukMedia`, which runs when a page's JS *touches* a media element — pages ship `<track>`
//!   and then never mention the video again. RED, run: move the call into `__manukMedia` and this
//!   gate, whose script never touches the video before the fetches are pumped, loads nothing.
//! - **The REAL parser runs, not a second one in JS.** The cue text here contains a `-->`-adjacent
//!   timestamp shape and a multi-line cue, which a naive split would mangle.
//! - **`default` is honoured.** It is the ONLY way a plain `<video>` ever shows a caption: there is
//!   no script to set `mode` and our chrome has no captions button. RED, run: ignore the attribute
//!   and `mode` stays `disabled`, `activeCues` stays empty forever, and the whole feature renders
//!   nothing while every other assertion still passes.
//! - **A non-WebVTT body leaves the track in ERROR, holding nothing.** An `.srt` renamed, or an HTML
//!   error page served with a 200, must not be silently accepted as captions. RED, run: drop the
//!   `!res.ok` branch and the track sits in `readyState` 1 — LOADING, forever — which is what a
//!   page's captions button waits on before it will render itself.
//!
//!   **Two probes of this claim came back GREEN and are recorded rather than hidden.** Making the
//!   parse failure `throw`, and separately deleting the `.catch(fail)`, both left the gate passing:
//!   the two paths are equivalent here because the throw lands in the same rejection handler. So
//!   this gate does NOT measure "reports rather than throws", however reasonable that sentence
//!   sounds — it measures the ERROR state and the absence of cues. A probe that cannot fail measured
//!   nothing, and the honest response is to narrow the claim to what stayed falsifiable.
//! - **The cues reach the tick-257 timeline.** `cuechange` fires from a fetched track exactly as it
//!   does from a scripted one — which is what makes this a connection rather than a parallel path.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use manuk_text::FontContext;

/// A real WebVTT file, with the two shapes a naive parser gets wrong: a cue whose TEXT contains
/// something timestamp-like, and a cue spanning two lines.
const VTT: &str = "WEBVTT - Example captions\n\
                   \n\
                   intro\n\
                   00:00:00.000 --> 00:00:02.000\n\
                   Hello, and welcome.\n\
                   \n\
                   00:00:02.000 --> 00:00:05.000\n\
                   ALICE: the clock reads 00:00:09.000 here\n\
                   and this line belongs to the same cue\n\
                   \n\
                   00:00:03.500 --> 00:00:06.000 align:start position:10% size:40%\n\
                   BOB: talking over her\n\
                   \n\
                   00:00:06.000 --> 00:00:08.000 line:0 align:middle\n\
                   SIGN: the bottom of the frame is already busy\n\
                   \n\
                   00:00:08.000 --> 00:00:10.000 line:-1 vertical:rl\n\
                   the last line, written vertically\n";

const HTML: &str = r##"<!doctype html>
<html><body>
  <video id="v"><track kind="captions" label="English" srclang="en" src="/subs.vtt" default></video>
  <video id="bad"><track kind="subtitles" src="/notvtt.txt"></video>
  <div id="out">-</div>
  <!-- A page with NO <script> AT ALL never gets a JS context (measured, see the header), so it
       could not load a track no matter how the sweep were written. This one line is what a real
       video page has in abundance; the caption work itself is still driven by nobody. -->
  <script>1;</script>
</body></html>"##;

/// Evaluated AFTER the fetches are pumped, and deliberately NOT part of the page: nothing in the
/// document touches the video before the tracks load, so the load cannot be a side effect of the
/// page reaching for the element.
const REPORT: &str = r##"
  var v = document.getElementById('v');
  var t = v.textTracks[0];
  var texts = [];
  for (var i = 0; i < t.cues.length; i++) { texts.push(t.cues[i].text); }

  // The tick-257 timeline, driven by a track that came off the WIRE.
  var fires = 0, drawn = '';
  t.addEventListener('cuechange', function () {
    fires++;
    var a = this.activeCues, s = [];
    for (var i = 0; i < a.length; i++) { s.push(a[i].text); }
    drawn = s.join('+');
  });
  v.currentTime = 4.0;   // ALICE and BOB overlap here

  var bad = document.getElementById('bad');
  var badTrack = bad.getElementsByTagName('track')[0];

  document.getElementById('out').textContent =
    'tracks=' + v.textTracks.length +
    ' cues=' + t.cues.length +
    ' mode=' + t.mode +
    ' label=' + t.label + ' lang=' + t.language + ' kind=' + t.kind +
    ' id0=' + (t.cues[0].id || '-') +
    ' multiline=' + (texts[1] || '').split('\n').length +
    ' keptstamp=' + ((texts[1] || '').indexOf('00:00:09.000') >= 0) +
    ' fires=' + fires + ' drawn=' + drawn +
    ' align2=' + t.cues[2].align + ' pos2=' + t.cues[2].position + ' size2=' + t.cues[2].size +
    ' line0=' + t.cues[0].line + ' align0=' + t.cues[0].align +
    ' line3=' + t.cues[3].line + ' align3=' + t.cues[3].align +
    ' line4=' + t.cues[4].line + ' vert4=' + t.cues[4].vertical +
    ' badready=' + badTrack.readyState +
    ' badcues=' + (bad.textTracks[0] ? bad.textTracks[0].cues.length : -1);
"##;

#[test]
fn a_track_element_fetches_parses_and_becomes_a_live_caption_track() {
    let tmp = std::env::temp_dir().join(format!("manuk-track-{}", std::process::id()));
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

                // `/notvtt.txt` serves an HTML error page with a 200 — the real-world shape of a
                // caption URL that has rotted, and the reason "not WebVTT" must not be fatal.
                let (ctype, body) = if req.contains("/notvtt.txt") {
                    (
                        "text/html",
                        "<!doctype html><h1>404 Not Found</h1>".to_string(),
                    )
                } else {
                    ("text/vtt", VTT.to_string())
                };
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\n\
                     Connection: close\r\n\r\n",
                    body.len()
                );
                let _ = sock.write_all(head.as_bytes());
                let _ = sock.write_all(body.as_bytes());
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

    // ── 1. The file was actually REQUESTED. Before this tick, nothing ever asked. ────────────
    let reqs = log.lock().unwrap().clone();
    assert!(
        reqs.iter().any(|r| r.contains("/subs.vtt")),
        "the <track>'s src was never fetched — the parser still has no page-side caller. \
         requests seen: {reqs:?}"
    );

    page.eval_for_test(REPORT);
    let root = page.dom().root();
    let out_node = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out_node);
    assert!(
        !got.is_empty() && got != "-",
        "the report never ran — the caption path threw. A track that fails to load must fail the \
         TRACK, not the page. got: {got:?}"
    );

    // ── 2. Parsed by the REAL parser, and the track carries the element's attributes. ────────
    assert!(
        got.contains("tracks=1") && got.contains("cues=5"),
        "all five cues arrive as a single track; got: {got}"
    );
    assert!(
        got.contains("label=English") && got.contains("lang=en") && got.contains("kind=captions"),
        "kind/label/srclang come off the element — a player enumerates textTracks to find the \
         user's language. got: {got}"
    );
    assert!(
        got.contains("id0=intro"),
        "the cue identifier line is kept; got: {got}"
    );
    assert!(
        got.contains("multiline=2") && got.contains("keptstamp=true"),
        "a two-line cue stays ONE cue, and timestamp-shaped text INSIDE a cue is text, not a \
         timestamp — the two shapes a naive JS re-implementation mangles. got: {got}"
    );

    // ── 3. `default` is the only thing that ever turns a plain <video>'s captions on. ────────
    assert!(
        got.contains("mode=showing"),
        "the `default` attribute must turn the track on: there is no script here to set mode and \
         no captions button in our chrome, so ignoring it renders NOTHING while every other \
         assertion still passes. got: {got}"
    );

    // ── 4. The fetched track drives the tick-257 timeline, identically to a scripted one. ────
    assert!(
        got.contains("fires=1") && got.contains("drawn=ALICE:"),
        "cuechange fires from a track that came off the WIRE, and both overlapping speakers are \
         active at t=4.0 — this is what makes it a CONNECTION rather than a parallel path. \
         got: {got}"
    );
    assert!(
        got.contains("BOB: talking over her"),
        "the second speaker is present in the active set; got: {got}"
    );

    // ── 5. CUE PLACEMENT — the settings are not decoration. ─────────────────────────────────
    assert!(
        got.contains("align2=start") && got.contains("pos2=10") && got.contains("size2=40"),
        "align/position/size come off the timestamp line — a speaker's line pinned to the side of \
         the frame they stand on. Dropped, every cue lands bottom-centre. got: {got}"
    );
    assert!(
        got.contains("line0=auto") && got.contains("align0=center"),
        "a cue with NO settings keeps the spec defaults, and `auto` must stay the string `auto`: \
         `line:0` is the TOP of the frame and `auto` is the bottom, so collapsing auto to 0 puts \
         every default caption at the top of the video. got: {got}"
    );
    assert!(
        got.contains("line3=0 align3=center"),
        "`line:0` is a LINE COUNT, not a percentage, and it lifts the caption to the top because \
         the bottom of the frame is already busy. `align:middle` is a superseded value that must \
         be skipped without costing the cue its text — leniency, not rejection. got: {got}"
    );
    assert!(
        got.contains("line4=-1 vert4=rl"),
        "`line:-1` counts UP from the bottom — read as a percentage it is nonsense, and read as \
         `auto` it silently moves. `vertical:rl` is how Japanese captions are written. got: {got}"
    );

    // ── 6. A body that is not WebVTT fails the TRACK, loudly and locally. ────────────────────
    assert!(
        got.contains("badready=3") && got.contains("badcues=0"),
        "an HTML error page served with a 200 must leave the track in ERROR(3) holding no cues — \
         not throw, and not silently accept garbage as captions. got: {got}"
    );
}
