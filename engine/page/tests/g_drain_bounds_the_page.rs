//! **G_DRAIN_BOUNDS_THE_PAGE — a non-converging page costs the hang guard ONCE per navigation, not
//! once per dynamic-script round.**
//!
//! The drain's two bounds are honestly named: `MAX_TASKS_PER_DRAIN` and its clock twin bound **a
//! drain**. The promise written beside them is about a **page**:
//!
//! > *"Without a ceiling, 'drain to quiescence' means 'never return', and the tab is gone with no
//! > recourse — which is precisely the failure Bar 0 exists to forbid."*
//!
//! A navigation runs the loop once at load and again per dynamic-script round, so a page that **both
//! spins and injects scripts** pays the bound once per round. Measured at the page level on
//! `www.agoda.com`, three consecutive runs agreeing within a second (tick 666):
//!
//! ```text
//!   load_async 3717ms   finish_loading 39894ms   TOTAL 43611ms      <- against a 12s budget
//!   17 drains, each to its own ~2.3s ceiling.   17 x 2.3 ~= 39.
//! ```
//!
//! **And `finish_loading`'s own `tokio::time::timeout` cannot enforce its budget here**, because a
//! timeout fires only at an await point and these drains are synchronous JavaScript. The bound has to
//! be a decision made *between* rounds.
//!
//! ⚠ **THE SHAPE OF THIS FIXTURE IS THE WHOLE GATE, and it is the reason tick 661 retracted tick
//! 660.** t660 claimed this defect from a fixture that only *spun*. Such a page injects no
//! `<script src>`, so `fetch_and_run_dynamic_scripts` breaks on its first round at
//! `pending.is_empty()` — the round loop is never entered, the count is 1 either way, and t661's gate
//! passed with the fix **disabled**. The claim was retracted and the change reverted. t661 then named
//! the experiment that would decide: *a page that both spins AND injects*. That is what this serves,
//! and it is why the assertion below can go red at all.
//!
//! Hermetic: one loopback socket serving scripts that chain, no live origin.

use manuk_text::FontContext;
use std::io::{Read, Write};
use std::net::TcpListener;

/// Each served script does two things: starts a self-rescheduling timer (so its drain never
/// converges) and appends the NEXT `<script src>` (so the round loop has work and runs again).
/// Without the second half this fixture is tick 660's, and the gate cannot fail.
fn script_for(n: u32) -> String {
    format!(
        "(function () {{\n\
         \x20 var i = 0;\n\
         \x20 (function spin() {{ i++; setTimeout(spin, 0); }})();\n\
         \x20 if ({n} < 4) {{\n\
         \x20   var s = document.createElement('script');\n\
         \x20   s.src = 'chain{next}.js';\n\
         \x20   document.body.appendChild(s);\n\
         \x20 }}\n\
         }})();\n",
        n = n,
        next = n + 1
    )
}

const HTML: &str = r##"<!doctype html><html><body><div id="d">x</div>
<script>
  var s = document.createElement('script');
  s.src = 'chain1.js';
  document.body.appendChild(s);
</script>
</body></html>"##;

/// A page that converges immediately — the control, so "we bounded it once" can never be confused
/// with "we stopped running scripts".
const CALM: &str = r##"<!doctype html><html><body><div id="d">x</div>
<script>document.getElementById('d').textContent = 'done';</script>
</body></html>"##;

#[test]
fn a_page_that_spins_and_injects_costs_the_ceiling_once_not_once_per_round() {
    let tmp = std::env::temp_dir().join(format!("manuk-drainpage-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).ok();
    unsafe { std::env::set_var("MANUK_STATE", &tmp) };
    // Small, so a spinning drain reaches its bound in well under a second and four rounds of them
    // still finish inside a test rather than inside a coffee break.
    unsafe { std::env::set_var("MANUK_MAX_DRAIN_MS", "300") };

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
                let idx: u32 = path
                    .trim_start_matches("/chain")
                    .trim_end_matches(".js")
                    .parse()
                    .unwrap_or(0);
                if idx == 0 {
                    return;
                }
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let body = script_for(idx);
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/javascript\r\nContent-Length: {}\r\n\
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
    let fonts = FontContext::new();
    let base = format!("http://{addr}/index.html");

    // ── THE CONTROL FIRST. A converging page must give up ZERO times — otherwise "one" below could
    //    be a browser that has simply stopped running scripts, which is the opposite of a fix.
    let calm_hits = rt.block_on(async {
        manuk_js::event_loop::clear_convergence_state();
        let mut p = manuk_page::Page::load_async(CALM, &base, &fonts, 800.0).await;
        p.finish_loading(&fonts, 800.0).await;
        manuk_js::event_loop::drain_ceiling_hits()
    });
    println!("DRAIN PROBE: a converging page gave up {calm_hits} time(s)");
    assert_eq!(
        calm_hits, 0,
        "a page that converges immediately tripped the hang guard {calm_hits} time(s) — the bound is \
         firing on healthy pages, which costs capability rather than saving it."
    );

    // ── THE SUBJECT.
    let hits = rt.block_on(async {
        manuk_js::event_loop::clear_convergence_state();
        let mut p = manuk_page::Page::load_async(HTML, &base, &fonts, 800.0).await;
        p.finish_loading(&fonts, 800.0).await;
        manuk_js::event_loop::drain_ceiling_hits()
    });
    let fetched = served.load(std::sync::atomic::Ordering::SeqCst);
    println!(
        "DRAIN PROBE: spins+injects gave up {hits} time(s); {fetched} chained script(s) served"
    );

    // ── PRECONDITIONS, both of them, because this fixture has two halves and either one missing
    //    makes the claim vacuous in a different way.
    assert!(
        fetched >= 1,
        "the gate's own fixture failed: no chained script was ever served, so the page never INJECTED \
         and the round loop was never entered — which is exactly the hole that made tick 660's \
         version unable to fail."
    );
    assert!(
        hits >= 1,
        "the gate's own fixture failed: the page never tripped the hang guard at all, so it never \
         SPUN and this run did not reproduce the condition being bounded."
    );

    // ── THE CLAIM, part 1 — THE MECHANISM: the chain stops. Each served script injects the next, so
    //    an unbounded round loop serves the whole chain. Measured with the bound disabled: **4 scripts
    //    served, 9 give-ups.** With it: 1 and 3. This is the assertion that names the fix, and it is
    //    the one that cannot be satisfied by anything else.
    assert!(
        fetched <= 1,
        "G_DRAIN_BOUNDS_THE_PAGE: {fetched} chained scripts were served, so the round loop kept \
         starting rounds for a page that had already reported it was not converging. Each round costs \
         a full drain ceiling. Measured on a real site (t666): 17 give-ups and `finish_loading` at \
         39.9s against its own 12s budget — which the outer `tokio::time::timeout` CANNOT enforce, \
         because a timeout fires at an await point and these drains are synchronous JavaScript. The \
         bound has to be a decision made BETWEEN rounds."
    );

    // ── THE CLAIM, part 2 — AND THE COST DOES NOT SCALE WITH THE PAGE. Three is the navigation's
    //    FIXED set of drain sites (the document's own scripts, the deferred pass, one dynamic round);
    //    what the defect did was let a page ADD to that set without limit. A number that stays put
    //    while the chain grows is the property worth gating, which a bare `== 1` would not be:
    //    those first three drains are legitimate first executions, and refusing them would be
    //    "bounded" achieved by not running the page.
    //
    //    ⚠ RESIDUAL, named rather than hidden: those three are not deduplicated with each other. A
    //    page that spins pays ~3 ceilings, not 1. Bounding THAT means an early-out inside `run_deferred`
    //    once the page is already flagged, which risks starving a page that spins once and would have
    //    converged later — a capability trade this gate deliberately does not make.
    assert!(
        hits <= 3,
        "the hang guard gave up {hits} times for one navigation ({fetched} chained scripts). The \
         navigation has three fixed drain sites; anything above that means the page is still able to \
         add drains to its own load, which is the defect."
    );
}
