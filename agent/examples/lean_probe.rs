//! W4.E2 probe: load one URL and print wall ms, wire body bytes, tracker requests refused, and
//! FNV digests of the observation (a11y lines + links) and of the screenshot PNG.
//! The profile comes from `MANUK_AGENT_PROFILE` (unset = full, `lean`). Usage: `lean_probe <url>`.
fn fnv(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf29ce484222325, |h, x| {
        (h ^ *x as u64).wrapping_mul(0x100000001b3)
    })
}
fn main() {
    let url = std::env::args().nth(1).expect("usage: lean_probe <url>");
    let rt = manuk_net::runtime();
    rt.block_on(async {
        let mut b = manuk_agent::AgentBrowser::new(1280, 800);
        manuk_net::reset_fetch_stats();
        let t = std::time::Instant::now();
        let r = b.navigate(&url).await; eprintln!("navigated");
        let wall = t.elapsed().as_millis();
        let wire = manuk_net::lean::WIRE_BYTES.load(std::sync::atomic::Ordering::Relaxed);
        let blocked = manuk_net::lean::BLOCKED.load(std::sync::atomic::Ordering::Relaxed);
        let (obs, a11y, links) = match b.observe() {
            Ok(o) => (
                true,
                fnv(o.semantics.join("\n").as_bytes()),
                fnv(o.links.iter().map(|l| l.href.as_str()).collect::<Vec<_>>().join("\n").as_bytes()),
            ),
            Err(_) => (false, 0, 0),
        };
        if let (Ok(d), Ok(o)) = (std::env::var("LEAN_DUMP"), b.observe()) {
            let _ = std::fs::write(d, o.semantics.join("\n"));
        }
        let nsem = b.observe().map(|o| o.semantics.len()).unwrap_or(0);
        eprintln!("observed"); let ts = std::time::Instant::now();
        b.load_visuals().await;
        let png = b.screenshot_png().unwrap_or_default();
        if let Ok(d) = std::env::var("LEAN_PNG") {
            let _ = std::fs::write(d, &png);
        }
        let shot = fnv(&png);
        let shot_ms = ts.elapsed().as_millis();
        let wire_after = manuk_net::lean::WIRE_BYTES.load(std::sync::atomic::Ordering::Relaxed);
        println!(
            "ok={} observed={obs} wall_ms={wall} wire_bytes={wire} blocked={blocked} sem_lines={nsem} a11y={a11y:016x} links={links:016x} shot={shot:016x} shot_ms={shot_ms} wire_after_shot={wire_after}",
            r.is_ok()
        );
    });
}
