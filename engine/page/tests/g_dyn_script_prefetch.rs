//! **G_DYN_SCRIPT_PREFETCH — a script inserted by a dynamic script is fetched as soon as it is
//! inserted, not after the rest of its round has arrived (PERF.1.2).**
//!
//! `fetch_and_run_dynamic_scripts` ran in rounds: fetch every pending `<script src>` (`join_all`), run
//! them in document order, then look for the scripts THOSE inserted. So a chain hop waited for the
//! slowest unrelated fetch of the round before its child was even requested. puentedemando.com
//! (PERF.1.2 trace): GTM inserts cookieyes + clarity + gtag; cookieyes inserts banner.js, which was
//! requested 0.9 s after cookieyes had arrived, because clarity.ms took 0.7 s to connect.
//!
//! Now each script runs as soon as it and the ones before it have arrived (same order as before), and
//! whatever it inserted is fetched right away; it still runs in the next round.
//!
//! The fixture: the page inserts `s1.js` (served at once) and `s2.js` (served after 800 ms). `s1.js`
//! inserts `s3.js`. Asserted:
//!   * `s3.js` is requested BEFORE `s2.js` has been served (the overlap),
//!   * the scripts still execute in the old order s1, s2, s3 (nothing reordered, nothing skipped),
//!   * `s3.js` is requested exactly once (the early fetch and the round's fetch are one request).
//!
//! The same test then runs the page again with `MANUK_NO_DYN_PREFETCH=1` (the old round-at-a-time
//! behaviour) and requires the opposite ordering — s3 requested only after s2 was served — so the
//! gate is shown to tell the two apart on every run, not just to pass.

use manuk_text::FontContext;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const HTML: &str = r##"<!doctype html><html><body><div id="d">x</div>
<script>
  window.__order = [];
  ['s1.js', 's2.js'].forEach(function (u) {
    var s = document.createElement('script');
    s.src = u;
    document.body.appendChild(s);
  });
</script>
</body></html>"##;

const S1: &str = "window.__order.push('s1'); var s = document.createElement('script'); s.src = 's3.js'; document.body.appendChild(s);";
const S2: &str = "window.__order.push('s2');";
const S3: &str = "window.__order.push('s3'); document.getElementById('d').textContent = window.__order.join(',');";

#[derive(Default, Debug)]
struct Log {
    s2_served: Option<Instant>,
    s3_requested: Vec<Instant>,
}

/// Serve s1/s2/s3; s2 answers after 800 ms. Returns the base URL and the request log.
fn serve() -> (String, Arc<Mutex<Log>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let log = Arc::new(Mutex::new(Log::default()));
    let l = log.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut sock) = stream else { continue };
            let l = l.clone();
            std::thread::spawn(move || {
                let mut buf = vec![0u8; 8192];
                let n = sock.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
                let body = match path.as_str() {
                    "/s1.js" => S1,
                    "/s2.js" => {
                        std::thread::sleep(Duration::from_millis(800));
                        S2
                    }
                    "/s3.js" => {
                        l.lock().unwrap().s3_requested.push(Instant::now());
                        S3
                    }
                    _ => "",
                };
                let _ = sock.write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/javascript\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    )
                    .as_bytes(),
                );
                let _ = sock.flush();
                if path == "/s2.js" {
                    l.lock().unwrap().s2_served = Some(Instant::now());
                }
            });
        }
    });
    (format!("http://{addr}/index.html"), log)
}

fn run(rt: &tokio::runtime::Runtime, fonts: &FontContext) -> (String, Log) {
    let (base, log) = serve();
    let text = rt.block_on(async {
        let mut p = manuk_page::Page::load_async(HTML, &base, fonts, 800.0).await;
        p.finish_loading(fonts, 800.0).await;
        let root = p.dom().root();
        let hit = manuk_css::query_selector_all(p.dom(), root, "#d");
        p.dom().text_content(hit[0])
    });
    let log = std::mem::take(&mut *log.lock().unwrap());
    (text, log)
}

#[test]
fn an_inserted_script_is_fetched_before_the_slow_rest_of_its_round_arrives() {
    let tmp = std::env::temp_dir().join(format!("manuk-dynprefetch-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).ok();
    unsafe { std::env::set_var("MANUK_STATE", &tmp) };
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let fonts = FontContext::new();

    // ── Arm 1: the change.
    let (text, log) = run(&rt, &fonts);
    println!("PREFETCH ON: d={text:?} log={log:?}");
    assert_eq!(
        text, "s1,s2,s3",
        "the three scripts must all run, in the order the round-at-a-time loop ran them"
    );
    assert_eq!(
        log.s3_requested.len(),
        1,
        "s3.js must be requested exactly once: the early fetch and the round's fetch are one request"
    );
    let (s2, s3) = (log.s2_served.unwrap(), log.s3_requested[0]);
    assert!(
        s3 < s2,
        "s3.js (inserted by s1.js) was requested {:?} AFTER s2.js was served: it waited for the slow \
         rest of its round instead of being fetched when s1.js inserted it",
        s3.duration_since(s2)
    );

    // ── Arm 2: the old behaviour must fail the ordering assertion, or the gate proves nothing.
    unsafe { std::env::set_var("MANUK_NO_DYN_PREFETCH", "1") };
    let (text, log) = run(&rt, &fonts);
    unsafe { std::env::remove_var("MANUK_NO_DYN_PREFETCH") };
    println!("PREFETCH OFF: d={text:?} log={log:?}");
    assert_eq!(text, "s1,s2,s3", "control arm: same scripts, same order");
    let (s2, s3) = (log.s2_served.unwrap(), log.s3_requested[0]);
    assert!(
        s3 > s2,
        "with MANUK_NO_DYN_PREFETCH=1 s3.js must be requested only after s2.js was served (the old \
         round-at-a-time behaviour); it was not, so this fixture cannot tell the two apart"
    );
}
