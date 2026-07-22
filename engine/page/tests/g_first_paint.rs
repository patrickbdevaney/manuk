//! **G_FIRST_PAINT — the document must reach the screen without waiting for its images.**
//!
//! The load path used to fetch and decode **every image on the page** before the shell was handed
//! anything at all, so the window stayed blank until the last tracking pixel on a news front page had
//! either arrived or timed out. Measured on nytimes.com: the document was parsed, cascaded and laid out
//! — *everything needed to paint* — in **1.7s**, and the user saw it at **14s**. Twelve of those
//! seconds were images nobody was looking at yet, because there was nothing on the screen to look at.
//!
//! No browser a person would use does that. Chromium puts the article up and lets the assets land
//! afterwards, reflowing as they arrive — which is exactly what an `<img>` without intrinsic dimensions
//! does anyway.
//!
//! This gate asserts the promise directly: **a page whose images are black holes still paints promptly.**
//! Not "eventually finishes" — *promptly*, on a budget that a stalled image cannot touch. If it ever
//! regresses, the browser goes back to feeling broken while every other gate stays green, which is the
//! precise failure mode that made this gate necessary and not merely nice.

use std::io::Read;
use std::net::TcpListener;
use std::time::{Duration, Instant};

use manuk_text::FontContext;

/// Accepts the connection, reads the request, and never replies — a dead CDN, which is the common case
/// on a page with a hundred third-party images.
fn blackhole() -> String {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in l.incoming() {
            let Ok(mut s) = stream else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 1024];
                let _ = s.read(&mut buf);
                std::thread::sleep(Duration::from_secs(3600));
            });
        }
    });
    format!("http://{addr}")
}

/// Serve `html` over real HTTP on localhost, so `prefetch_document` — **the path the shell actually
/// takes** — can be measured. Anything less is testing a different function than the one the user waits
/// for, which is how "the browser feels slow" survives a green benchmark.
fn serve(html: String) -> String {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in l.incoming() {
            let Ok(mut s) = stream else { continue };
            let body = html.clone();
            std::thread::spawn(move || {
                use std::io::Write;
                let mut buf = [0u8; 2048];
                let _ = s.read(&mut buf);
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = s.write_all(resp.as_bytes());
            });
        }
    });
    format!("http://{addr}/")
}

#[test]
fn first_paint_does_not_wait_for_images() {
    // **This gate's FIRST version was vacuous**, and the falsifier would have missed it because the
    // mutation I wrote did not compile (a compile error and a failing assertion are the same exit code
    // — the falsifier now refuses that). The vacuity: it called `Page::load`, which never fetched an
    // image in its life. It would have passed before the fix, after the fix, and with the fix reverted.
    //
    // The images were on the paint path in exactly ONE place: `prefetch_document`, which is what the
    // shell calls. So that is what this measures. A gate must exercise the path the user waits on.
    std::env::set_var("MANUK_NET_TIMEOUT_MS", "20000");

    let hole = blackhole();
    let imgs: String = (0..20)
        .map(|i| format!(r#"<img src="{hole}/dead{i}.png" width="100" height="80">"#))
        .collect();
    let url = serve(format!(
        r#"<!doctype html><html><body>
             <h1 id="headline">The article is what the user came for</h1>
             <p id="body">This text must be on screen while the images are still in flight.</p>
             {imgs}
           </body></html>"#
    ));

    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let started = Instant::now();
    let loaded = rt
        .block_on(manuk_page::prefetch_document(&url))
        .expect("prefetch");
    let page = match loaded {
        manuk_page::Loaded::Prefetched(pre) => {
            manuk_page::Page::from_prefetched(*pre, &fonts, 800.0)
        }
        manuk_page::Loaded::Document {
            html, final_url, ..
        } => manuk_page::Page::load(&html, &final_url, &fonts, 800.0),
        _ => panic!("expected a document"),
    };
    let elapsed = started.elapsed();

    // (1) It painted, and it painted PROMPTLY. Twenty black holes at a 20s deadline cannot touch this
    //     number, because nothing on the paint path asks them anything. If they could, this is 20s+.
    assert!(
        elapsed < Duration::from_secs(5),
        "G_FIRST_PAINT: the document took {elapsed:?} to become paintable with 20 dead images on it.\n  \
         First paint is waiting for images again. On nytimes.com that cost 12 seconds of blank window \
         while the document itself had been laid out for 1.7s. A browser that does this feels broken \
         even when every other gate is green — which is exactly why this gate exists."
    );

    // (2) And it is a real page, not an empty one that "finished" by giving up.
    let root = page.dom().root();
    let h = manuk_css::query_selector_all(page.dom(), root, "#headline");
    assert!(
        !h.is_empty(),
        "the headline must be in the painted document"
    );

    // (3) The images are still WANTED — deferred, not dropped. The shell fetches them on a background
    //     task and applies them when they land. "Fast" must not be achieved by quietly never loading
    //     the images at all, which is a different bug wearing this gate's success as a disguise.
    assert_eq!(
        page.pending_image_urls().len(),
        20,
        "G_FIRST_PAINT: the page reports {} pending images, not 20. If this is 0 the images will never \
         be shown, and the speed is a lie.",
        page.pending_image_urls().len()
    );
}

/// **Inline `<svg>` paints its vectors** (tick 394 — the paint half of the SVG-internals spec).
///
/// The icon/logo idiom is an inline `<svg>` subtree, no network involved. Before this, the
/// element laid out (a correctly-sized box after t389/391) but painted NOTHING — an icon-heavy
/// app shell rendered as blank squares. The subtree is serialized back to markup and rasterized
/// through the SAME usvg/resvg path that decodes `<img src="*.svg">` (xmlns injected when the
/// HTML parser dropped it). The assert is on PIXELS, not on the decode returning Some: a decoded
/// image that never reaches the display list is the failure this gate exists to catch.
#[test]
fn an_inline_svg_paints_its_vectors() {
    let fonts = FontContext::new();
    let html = r##"<!doctype html><body style="margin:0">
        <svg viewBox="0 0 10 10" style="width:100px;height:100px;display:block">
          <rect x="0" y="0" width="10" height="10" fill="#ff0000"/>
        </svg></body>"##;
    let page = manuk_page::Page::load(html, "https://svg.test/", &fonts, 200.0);
    let canvas = page.paint(&fonts, 200, 200);
    let px = canvas.rgba_bytes();
    // Sample the center of the 100×100 svg box at (50, 50).
    let (w, x, y) = (200usize, 50usize, 50usize);
    let i = (y * w + x) * 4;
    let (r, g, b) = (px[i], px[i + 1], px[i + 2]);
    assert!(
        r > 200 && g < 60 && b < 60,
        "G_FIRST_PAINT/svg: the inline svg's solid-red rect must paint red at its center, \
         got rgb({r},{g},{b}) — the vector was laid out but never rasterized"
    );
}
