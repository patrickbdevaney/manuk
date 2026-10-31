//! **G_LEAN_AGENT_PROFILE (W4.E2) — the lean profile reads the same page for a fraction of the
//! bytes, and the pixels come back when a screenshot asks.**
//!
//! Two identical servers (the HTTP cache is keyed by URL, so a second pass over ONE server would be
//! served from cache and prove nothing). Each serves a page of six noisy PNGs, honouring `Range`.
//! Full profile first, lean second; asserts: the accessibility tree (roles, names, click points)
//! is identical, the lean pass pulled under a third of the bytes, the tracker script was refused
//! before any request, and the screenshot taken after `load_visuals` is byte-identical.
//!
//! Own binary: the profile switch is process-global.
//!
//! Mutations that must turn this red:
//!   1. `images_deferred()` always false          → bytes assertion
//!   2. `apply_lean_image_dims` does nothing      → a11y click points drift (images are 0x0)
//!   3. `load_visuals` a no-op                    → screenshot differs (no pixels)
//!   4. `is_tracker` always false                 → blocked == 0

use std::io::{Read, Write};
use std::sync::atomic::Ordering::Relaxed;

fn noisy_png(w: u32, h: u32, seed: u32) -> Vec<u8> {
    let mut x = seed.wrapping_mul(2654435761).max(1);
    let mut raw = Vec::new();
    for _ in 0..h {
        raw.push(0u8);
        for _ in 0..w * 3 {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            raw.push(x as u8);
        }
    }
    let mut out = Vec::new();
    {
        let mut enc = png_encoder(&mut out, w, h);
        enc.write_all(&raw).ok();
    }
    out
}

// Minimal stored-deflate PNG writer: no extra dependency, and large (incompressible) by construction.
fn png_encoder(out: &mut Vec<u8>, w: u32, h: u32) -> impl Write + '_ {
    struct P<'a> {
        out: &'a mut Vec<u8>,
        raw: Vec<u8>,
        w: u32,
        h: u32,
    }
    impl Write for P<'_> {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.raw.extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    impl Drop for P<'_> {
        fn drop(&mut self) {
            fn crc(d: &[u8]) -> u32 {
                let mut c = 0xffff_ffffu32;
                for &b in d {
                    c ^= b as u32;
                    for _ in 0..8 {
                        c = if c & 1 != 0 {
                            0xedb8_8320 ^ (c >> 1)
                        } else {
                            c >> 1
                        };
                    }
                }
                !c
            }
            fn adler(d: &[u8]) -> u32 {
                let (mut a, mut b) = (1u32, 0u32);
                for &x in d {
                    a = (a + x as u32) % 65521;
                    b = (b + a) % 65521;
                }
                (b << 16) | a
            }
            let mut z = vec![0x78, 0x01];
            let n = self.raw.len();
            let mut i = 0;
            while i < n {
                let len = (n - i).min(65535);
                z.push(if i + len == n { 1 } else { 0 });
                z.extend_from_slice(&(len as u16).to_le_bytes());
                z.extend_from_slice(&(!(len as u16)).to_le_bytes());
                z.extend_from_slice(&self.raw[i..i + len]);
                i += len;
            }
            z.extend_from_slice(&adler(&self.raw).to_be_bytes());
            let chunk = |out: &mut Vec<u8>, t: &[u8], d: &[u8]| {
                out.extend_from_slice(&(d.len() as u32).to_be_bytes());
                let mut td = t.to_vec();
                td.extend_from_slice(d);
                out.extend_from_slice(&td);
                out.extend_from_slice(&crc(&td).to_be_bytes());
            };
            self.out.extend_from_slice(b"\x89PNG\r\n\x1a\n");
            let mut ihdr = Vec::new();
            ihdr.extend_from_slice(&self.w.to_be_bytes());
            ihdr.extend_from_slice(&self.h.to_be_bytes());
            ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
            chunk(self.out, b"IHDR", &ihdr);
            chunk(self.out, b"IDAT", &z);
            chunk(self.out, b"IEND", &[]);
        }
    }
    P {
        out,
        raw: Vec::new(),
        w,
        h,
    }
}

const HTML: &str = "<html><head><title>g</title></head><body><h1>Gallery</h1>\
<img src='i0.png' alt='p0'><img src='i1.png' alt='p1'><img src='i2.png' alt='p2'>\
<img src='i3.png' alt='p3'><img src='i4.png' alt='p4'><img src='i5.png' alt='p5'>\
<script src='https://www.googletagmanager.com/gtm.js'></script><button>Buy</button></body></html>";

fn serve() -> (String, std::thread::JoinHandle<()>) {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let h = std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { break };
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
                let (body, ct): (Vec<u8>, &str) = if path == "/" {
                    (HTML.as_bytes().to_vec(), "text/html")
                } else if let Some(i) = path.strip_prefix("/i").and_then(|p| p.strip_suffix(".png"))
                {
                    (
                        noisy_png(
                            90 + i.parse::<u32>().unwrap_or(0),
                            90,
                            7 + i.parse::<u32>().unwrap_or(0),
                        ),
                        "image/png",
                    )
                } else {
                    let _ = s.write_all(
                        b"HTTP/1.1 404 NF\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    );
                    return;
                };
                let lower = req.to_ascii_lowercase();
                let (status, part, extra) = match lower.find("range: bytes=0-") {
                    Some(p) => {
                        let end: usize = lower[p + 15..]
                            .split(|c: char| !c.is_ascii_digit())
                            .next()
                            .and_then(|d| d.parse().ok())
                            .unwrap_or(body.len());
                        let end = end.min(body.len() - 1);
                        (
                            "206 Partial Content",
                            body[..=end].to_vec(),
                            format!("Content-Range: bytes 0-{end}/{}\r\n", body.len()),
                        )
                    }
                    None => ("200 OK", body.clone(), String::new()),
                };
                let _ = s.write_all(
                    format!("HTTP/1.1 {status}\r\nContent-Type: {ct}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n", part.len()).as_bytes(),
                );
                let _ = s.write_all(&part);
            });
        }
    });
    (base, h)
}

async fn run(url: &str) -> (Vec<String>, u64, usize, Vec<u8>) {
    let mut b = manuk_agent::AgentBrowser::new(1024, 768);
    manuk_net::reset_fetch_stats();
    b.navigate(url).await.expect("navigate");
    let wire = manuk_net::lean::WIRE_BYTES.load(Relaxed);
    let blocked = manuk_net::lean::BLOCKED.load(Relaxed);
    let sem = b.observe().expect("observe").semantics;
    b.load_visuals().await;
    (sem, wire, blocked, b.screenshot_png().expect("png"))
}

#[test]
fn lean_reads_the_same_page_for_a_fraction_of_the_bytes() {
    let (a, _ha) = serve();
    let (b, _hb) = serve();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        manuk_net::lean::set(false);
        let (sem_full, wire_full, blocked_full, png_full) = run(&format!("{a}/")).await;
        manuk_net::lean::set(true);
        let (sem_lean, wire_lean, blocked_lean, png_lean) = run(&format!("{b}/")).await;
        println!("full wire={wire_full} lean wire={wire_lean} blocked lean={blocked_lean}");
        assert!(
            sem_full
                .iter()
                .any(|l| l.contains("pic") || l.contains("p0")),
            "VACUOUS: no images in the tree: {sem_full:?}"
        );
        assert!(
            wire_full > 100_000,
            "VACUOUS: full profile fetched only {wire_full} bytes"
        );
        assert_eq!(blocked_full, 0);
        assert!(blocked_lean >= 1, "tracker script was not refused");
        assert_eq!(
            sem_full, sem_lean,
            "the a11y tree (names and click points) must not change"
        );
        assert!(
            wire_lean * 3 < wire_full,
            "lean {wire_lean} is not under a third of full {wire_full}"
        );
        assert_eq!(
            png_full, png_lean,
            "the screenshot after load_visuals must equal the full profile's"
        );
    });
}
