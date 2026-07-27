//! **G_IMAGES_SURVIVE_BUDGET — the images this page already downloaded must reach it even when the
//! load budget dies waiting on a different one.**
//!
//! This is tick 654's defect **one phase further down**, and it is the phase where the loss is what
//! the user actually looks at. `finish_loading` enforces the load budget as a hard deadline dropped
//! wherever it runs out, on the stated ground that *"each phase fetches everything it needs and only
//! then applies it to the DOM, so a dropped future loses that phase's ENHANCEMENT and never a
//! half-mutated document."* For a fan-out that sentence quietly says the opposite of what it means:
//!
//! ```text
//!   join_all(every distinct image url)  ->  decode  ->  fold into the cache  ->  apply
//! ```
//!
//! `join_all` is **one** future. It yields its vector only when the *last* fetch settles, so a single
//! image behind a stalled CDN takes down every image that had already arrived, decoded or not. The
//! phase does not lose "the enhancement it had not got yet" — it loses everything it *paid for*. On a
//! news front page, an image-heavy shop or `keirin.jp`, that is the whole page: every `<img>` collapses
//! to 0x0, so the elements are present in the DOM, counted by coverage, and occupy no space. That is
//! exactly the signature the t654 sweep recorded — `keirin.jp` coverage 98.0% -> 74.4% once the
//! stylesheets landed and the picture-shaped holes stopped being full-width UA blocks.
//!
//! **How this gate makes the deadline fire on purpose.** One local socket serves two BMPs of distinct,
//! unmistakable widths *instantly* and then **stalls forever** on a third — the shape of one slow
//! image host among several, which is the ordinary condition of the open web, not an exotic one. With
//! `MANUK_LOAD_BUDGET_MS` small, the phase is guaranteed to be cancelled inside that third fetch,
//! which is precisely where the old code lost the other two.
//!
//! Hermetic: one loopback socket, no live origin, and the stall is ours. It cannot false-RED on the
//! network, and it cannot pass by failing to reproduce — both preconditions (the fast images *were*
//! served; the load *did* return on the budget) are asserted before the claim.

use manuk_text::FontContext;
use std::io::{Read, Write};
use std::net::TcpListener;

/// Two widths that no default, no UA rule and no coincidence produces, so "the image was applied" and
/// "the image was lost and the element collapsed" can never be read as the same number.
const FAST_A_W: u32 = 41;
const FAST_A_H: u32 = 23;
const FAST_B_W: u32 = 67;
const FAST_B_H: u32 = 29;

const HTML: &str = r##"<!doctype html><html><body>
<img id="a" src="fast-a.bmp">
<img id="b" src="fast-b.bmp">
<img id="slow" src="slow.bmp">
</body></html>"##;

/// A 24-bit BMP of exactly `w` x `h`, built by hand — no image-encoder dev-dependency, and no CRC to
/// get wrong. `image` sniffs the `BM` magic, so the extension and the Content-Type are courtesy only.
fn bmp(w: u32, h: u32) -> Vec<u8> {
    let row = ((w * 3 + 3) / 4) * 4; // rows are padded to a 4-byte boundary
    let pixels = (row * h) as usize;
    let mut v = Vec::with_capacity(54 + pixels);
    v.extend_from_slice(b"BM");
    v.extend_from_slice(&((54 + pixels) as u32).to_le_bytes()); // file size
    v.extend_from_slice(&0u32.to_le_bytes()); // reserved
    v.extend_from_slice(&54u32.to_le_bytes()); // pixel-data offset
    v.extend_from_slice(&40u32.to_le_bytes()); // BITMAPINFOHEADER size
    v.extend_from_slice(&(w as i32).to_le_bytes());
    v.extend_from_slice(&(h as i32).to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes()); // planes
    v.extend_from_slice(&24u16.to_le_bytes()); // bits per pixel
    v.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB, no compression
    v.extend_from_slice(&(pixels as u32).to_le_bytes());
    v.extend_from_slice(&2835i32.to_le_bytes()); // 72 dpi
    v.extend_from_slice(&2835i32.to_le_bytes());
    v.extend_from_slice(&0u32.to_le_bytes()); // palette: all colours
    v.extend_from_slice(&0u32.to_le_bytes()); // all colours important
    v.resize(54 + pixels, 0x7f); // mid-grey body; the pixels are never inspected
    v
}

/// The **decoded pixels bound to `#id`** — the map the painter draws an `<img>` from — as
/// `(width, height)`, or `None` if this element has no image on the page.
///
/// ⚠ This gate originally asserted on the element's LAID-OUT width, and a control run killed that
/// reading before it could become a false claim: an `<img>` measures 784px (the full content width)
/// whether its bytes arrived or not, because natural sizing does not reach layout. That is a real and
/// larger fidelity defect, and it is **not** this one — it is recorded as its own thread rather than
/// smuggled in here. What this gate is about is whether the bytes survive the deadline, so it asks
/// the map that holds the bytes.
fn image_on(page: &manuk_page::Page, id: &str) -> Option<(u32, u32)> {
    let root = page.dom().root();
    let node = *manuk_css::query_selector_all(page.dom(), root, &format!("#{id}")).first()?;
    page.decoded_images()
        .get(&node)
        .map(|i| (i.width, i.height))
}

#[test]
fn images_already_downloaded_are_not_discarded_when_the_budget_dies_on_a_slow_one() {
    let tmp = std::env::temp_dir().join(format!("manuk-imgbudget-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).ok();
    unsafe { std::env::set_var("MANUK_STATE", &tmp) };
    // Small enough that the stalled image MUST outlast it, large enough that two BMPs served from a
    // loopback socket in microseconds are comfortably inside it. The `served` precondition below is
    // what rules out the degenerate reading where nothing arrived either.
    unsafe { std::env::set_var("MANUK_LOAD_BUDGET_MS", "1500") };

    let served = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let counter = served.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut sock) = stream else { continue };
            let counter = counter.clone();
            std::thread::spawn(move || {
                let mut buf = vec![0u8; 8192];
                let n = sock.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let path = req
                    .lines()
                    .next()
                    .and_then(|l| l.split_whitespace().nth(1))
                    .unwrap_or("/")
                    .to_string();
                let body = match path.as_str() {
                    "/fast-a.bmp" => bmp(FAST_A_W, FAST_A_H),
                    "/fast-b.bmp" => bmp(FAST_B_W, FAST_B_H),
                    // **THE STALL.** One slow image host among several — accepted, never answered —
                    // so the fan-out is still awaiting it when the load budget expires. This is the
                    // ordinary condition of a page that mixes a CDN with a third-party asset host.
                    _ => {
                        std::thread::sleep(std::time::Duration::from_secs(30));
                        return;
                    }
                };
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: image/bmp\r\nContent-Length: {}\r\n\
                     Connection: close\r\n\r\n",
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
    let base = format!("http://{addr}/index.html");
    let fonts = FontContext::new();
    let started = std::time::Instant::now();
    let page = rt.block_on(async {
        let mut p = manuk_page::Page::load_async(HTML, &base, &fonts, 800.0).await;
        p.finish_loading(&fonts, 800.0).await;
        p
    });
    let elapsed = started.elapsed();
    println!(
        "IMAGE-BUDGET PROBE: served={} elapsed={:?} a={:?} b={:?} slow={:?}",
        served.load(std::sync::atomic::Ordering::SeqCst),
        elapsed,
        image_on(&page, "a"),
        image_on(&page, "b"),
        image_on(&page, "slow"),
    );

    // ── PRECONDITIONS, asserted rather than assumed — a green that means "nothing was ever served"
    //    or "the deadline never fired" is a green about the empty case.
    assert!(
        served.load(std::sync::atomic::Ordering::SeqCst) >= 2,
        "the gate's own fixture failed: the two fast images were not both served, so nothing was in \
         hand to be discarded and this test would be asserting the empty case."
    );
    assert!(
        elapsed < std::time::Duration::from_secs(20),
        "the load did not return on the budget at all ({elapsed:?}) — the stalled image was awaited \
         to completion, so this run never exercised a mid-phase cancellation."
    );

    // ── THE CLAIM. Both images that arrived are on the page, each identified by its own unmistakable
    //    dimensions; the stalled one is honestly absent rather than faked.
    assert_eq!(
        (
            image_on(&page, "a"),
            image_on(&page, "b"),
            image_on(&page, "slow")
        ),
        (
            Some((FAST_A_W, FAST_A_H)),
            Some((FAST_B_W, FAST_B_H)),
            None
        ),
        "G_IMAGES_SURVIVE_BUDGET: the two images that had ALREADY been downloaded never reached the \
         page. The image phase fans out with `join_all`, which is ONE future yielding ONE vector when \
         the LAST fetch settles — so a single stalled host discards every image that arrived before \
         it. Drive the fan-out against a deadline and keep what completed: the budget is allowed to \
         drop the images that did not arrive, and nothing else."
    );

    // ── AND THE SECOND HALF OF THE SAME DEFECT, which the probe found rather than predicted: the
    //    cancelled fan-out lost the "we already asked" record along with the bytes, so `load_async`'s
    //    budgeted pass and `finish_loading`'s ran the SAME two fetches — **4 responses for 2 images**,
    //    a G_DEDUP-class duplicate on the wire caused by the cancellation, not by the dedup logic.
    //    Banking what arrived also banks the bookkeeping, so the second pass has only `slow` left.
    assert_eq!(
        served.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "each fast image went to the WIRE more than once: the interrupted fan-out threw away the \
         record of what it had already fetched, so the next budgeted pass asked again."
    );
}
