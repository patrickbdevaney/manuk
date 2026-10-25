//! **G_SOP_FLOOR — a web page cannot read the disk, and cannot read another origin's frame (W3.1).**
//!
//! Before this gate, `manuk_net` served `file:` to any requester and every frame shared one JS realm with
//! no origin check. So any website could write
//!
//! ```html
//! <iframe src="file:///home/u/.ssh/id_ed25519"></iframe>
//! <script>onload = () => send(frames[0].contentDocument.body.textContent)</script>
//! ```
//!
//! and read the key. The same move read a `localhost` admin panel or a logged-in bank through
//! `contentDocument`. For an agent browser that loads untrusted pages on the operator's own machine, that
//! is the whole threat model in two lines.
//!
//! ## The claims
//!
//! ```text
//! A  web page frames a seeded secret file            → NOT rendered     (the loader refuses)
//! A' local page frames its sibling file              → rendered         (vacuity guard: local still works)
//! B  cross-origin frame: contentDocument             → null
//! B  cross-origin frame: contentWindow.document      → throws SecurityError
//! B  cross-origin frame: contentWindow.postMessage   → still a function (embeds keep working)
//! B  data: frame                                     → contentDocument null (opaque origin)
//! B' same-origin frame                               → readable         (vacuity guard)
//! C  each defensive layer alone: net initiator check, navigation policy, resolve_url
//! ```
//!
//! Part C exists because the layers overlap. Part A can stay green when ONE layer is mutated away,
//! since another layer still catches it. Each layer is therefore pinned on its own, so deleting any
//! one of them turns this gate red.
//!
//! Mutation-checked 2026-10-24. Each of these deletions turns this gate RED:
//! - the `contentDocument` cross-origin check;
//! - the net initiator check;
//! - the net navigation-policy check;
//! - the `resolve_url` guard.
//!
//! One layer is NOT independently pinned: the frame loader's own `is_local_url` filter in
//! `fetch_and_load_iframes`. `resolve_url` already turns a web page's `file:` frame source into
//! `about:blank` before the loader sees it, so deleting the filter changes nothing observable here.
//! The filter stays as defence in depth for a source string that reaches the loader without passing
//! through `resolve_url`.

use std::io::{Read, Write};
use std::net::TcpListener;

fn serve_one(body: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut sock) = stream else { continue };
            let mut buf = vec![0u8; 8192];
            let _ = sock.read(&mut buf);
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = sock.write_all(resp.as_bytes());
        }
    });
    addr
}

const PROBE: &str = r#"
  var R = [];
  function p(s) {
    R.push(s);
    var o = document.getElementById('__sop__');
    if (!o) { o = document.createElement('pre'); o.id = '__sop__'; document.body.appendChild(o); }
    o.textContent = R.join('|');
  }
  var f = document.querySelectorAll('iframe');
  function xoRead(fr) { try { fr.contentWindow.document; return 'nothrow'; } catch (e) { return e.name; } }
  p('frames=' + f.length);
  p('xoDocNull=' + (f[0].contentDocument === null));
  p('xoWinDoc=' + xoRead(f[0]));
  p('xoPost=' + (typeof (f[0].contentWindow && f[0].contentWindow.postMessage)));
  p('dataDocNull=' + (f[1].contentDocument === null));
  p('same=' + (f[2].contentDocument ? f[2].contentDocument.body.textContent.trim() : 'NULL'));
"#;

#[test]
fn a_web_page_cannot_read_the_disk_or_another_origins_frame() {
    let fonts = manuk_text::FontContext::new();
    let rt = tokio::runtime::Runtime::new().unwrap();

    // ── A: a real web page (served over http from localhost) frames a seeded secret file. ──────────
    let dir = std::env::temp_dir().join(format!("manuk-sop-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let secret = dir.join("secret.html");
    std::fs::write(&secret, "<p>SECRET-FILE-CANARY</p>").unwrap();
    let secret_url = format!("file://{}", secret.display());
    let addr = serve_one(format!(
        "<!doctype html><html><body><iframe src=\"{secret_url}\"></iframe></body></html>"
    ));
    let web_url = format!("http://{addr}/");
    let (html, final_url) = rt.block_on(manuk_page::fetch_html(&web_url)).unwrap();
    let page = rt.block_on(manuk_page::Page::load_async(&html, &final_url, &fonts, 800.0));
    let framed = page.rendered_iframe_urls().join(",");
    println!("SOP A: web page rendered frames = {framed:?}");
    assert!(
        !framed.contains("secret.html"),
        "**THE POINT**: a web page framed a local file and it LOADED. Any site could read \
         ~/.ssh or ~/.claude through `contentDocument`. got {framed:?}"
    );

    // ── A': the vacuity guard. A LOCAL page still frames its sibling file. ─────────────────────────
    let sibling = dir.join("sibling.html");
    std::fs::write(&sibling, "<p>SIBLING</p>").unwrap();
    let parent = dir.join("parent.html");
    std::fs::write(&parent, "<!doctype html><iframe src=\"sibling.html\"></iframe>").unwrap();
    let parent_url = format!("file://{}", parent.display());
    let (html, final_url) = rt.block_on(manuk_page::fetch_html(&parent_url)).unwrap();
    let page = rt.block_on(manuk_page::Page::load_async(&html, &final_url, &fonts, 800.0));
    let framed = page.rendered_iframe_urls().join(",");
    println!("SOP A': local page rendered frames = {framed:?}");
    assert!(
        framed.contains("sibling.html"),
        "a LOCAL page must still frame its own sibling file. Without this claim, a fix that broke every \
         file: frame would pass claim A while breaking the whole local-fixture corpus. got {framed:?}"
    );

    // ── B: contentDocument across origins (frames installed directly, at the origins under test). ──
    // ⚠ The empty <script> is load-bearing: a JS context is only built for a document that HAS a
    // script element, so without it `eval_for_test` silently runs nothing and every claim reads empty.
    let parent_html = "<!doctype html><html><body>\
        <iframe id=a></iframe><iframe id=b></iframe><iframe id=c></iframe>\
        <script></script></body></html>";
    let mut page = manuk_page::Page::load(parent_html, "https://parent.test/", &fonts, 900.0);
    let root = page.dom().root();
    let fr = manuk_css::query_selector_all(page.dom(), root, "iframe");
    page.render_iframe(fr[0], "<p>OTHER-ORIGIN-CANARY</p>", "https://bank.test/statement", &fonts, 0);
    page.render_iframe(fr[1], "<p>DATA-CANARY</p>", "data:text/html,x", &fonts, 0);
    page.render_iframe(fr[2], "<p>SAME-CANARY</p>", "https://parent.test/child", &fonts, 0);
    page.eval_for_test(PROBE);
    let dom = page.dom();
    let got = manuk_css::query_selector_all(dom, dom.root(), "#__sop__")
        .first()
        .map(|&n| dom.text_content(n))
        .unwrap_or_default();
    println!("SOP B: {got}");
    for (claim, why) in [
        ("xoDocNull=true", "a cross-origin frame's contentDocument must be null (HTML); it was the whole other document"),
        ("xoWinDoc=SecurityError", "contentWindow.document of a cross-origin frame must throw SecurityError"),
        ("xoPost=function", "a cross-origin frame must keep a contentWindow with postMessage — OAuth, payment and video embeds call it, and a null window breaks them"),
        ("dataDocNull=true", "a data: frame is an opaque origin, so cross-origin to its embedder"),
        ("same=SAME-CANARY", "VACUITY GUARD: a same-origin frame must still be readable, or every embed and the WPT suite break"),
    ] {
        assert!(got.contains(claim), "G_SOP_FLOOR: expected `{claim}`\n  got: {got}\n  {why}.");
    }

    // ── C: each layer, alone. ───────────────────────────────────────────────────────────────────────
    // C1 — the net layer refuses `file:` to a web INITIATOR, even while the navigation policy is local.
    manuk_net::set_top_level_document(&parent_url);
    let r = rt.block_on(manuk_net::fetch_from(&secret_url, Some("https://evil.test/")));
    assert!(r.is_err(), "C1: manuk_net served file: to a web initiator: {:?}", r.map(|x| x.status));
    // C2 — with no initiator, the top-level navigation decides: a web document gets nothing.
    // ⚠ A FRESH file, never requested before: manuk_net does not retry a URL that already failed in this
    // navigation, so asking for `secret_url` again would be refused by that negative cache whatever the
    // policy said. That made this claim green with the policy check deleted, which is how it was caught.
    let c2 = dir.join("c2.html");
    std::fs::write(&c2, "<p>C2</p>").unwrap();
    manuk_net::set_top_level_document("https://evil.test/");
    let r = rt.block_on(manuk_net::fetch(&format!("file://{}", c2.display())));
    assert!(r.is_err(), "C2: manuk_net served file: while a web document was the top level");
    // The vacuity read uses a DIFFERENT file: manuk_net does not retry a URL that already failed in
    // this navigation, so re-reading `secret_url` would only measure that negative cache.
    manuk_net::set_top_level_document(&parent_url);
    let r = rt.block_on(manuk_net::fetch(&format!("file://{}", sibling.display())));
    assert!(r.is_ok(), "C2 vacuity: a local top-level document must still read local files: {:?}", r.err());
    // C3 — URL resolution: a web base never resolves into the disk; a file: base still does.
    assert_eq!(manuk_page::resolve_url("https://evil.test/", &secret_url), "about:blank", "C3");
    assert_eq!(manuk_page::resolve_url(&parent_url, "sibling.html"), format!("file://{}", sibling.display()), "C3 vacuity");

    let _ = std::fs::remove_dir_all(&dir);
}
