//! **G_LOAD_GEOMETRY — a box built by a `load` handler must HAVE a box.**
//!
//! ⚠⚠⚠ **`window.addEventListener('load', …)` is where a very large fraction of the web builds its
//! DOM, and everything it built measured ZERO — permanently.** Not on a later read, not after
//! writing the node's own style, not on the next task: the node never acquired geometry at all.
//! Bisected with a six-row battery, two of them controls:
//!
//! ```text
//!   append during parse, read during parse          550   CONTROL
//!   append in the load handler, read there            0   <- the defect
//!   ...re-read after writing an unrelated property    0
//!   ...re-read after writing the node's OWN style     0
//!   ...re-read one task later (setTimeout)            0   <- it never recovers
//!   a node that has existed since parse             550   CONTROL
//! ```
//!
//! **The mechanism is a missing arming, not missing machinery.** A geometry read
//! (`offsetWidth`/`offsetHeight`, `getBoundingClientRect`, used-value `getComputedStyle`) is
//! supposed to force a synchronous reflow: the binding calls up into the host, the host re-cascades
//! and re-lays-out, and the read answers against fresh geometry. `ReflowScope` is that hook, and
//! **seventeen script re-entries install one**. `Page::fire_lifecycle` — the eighteenth, and the
//! one both `load` and `DOMContentLoaded` go through — delegated to `eval_for_test`, which takes
//! neither `fonts` nor a viewport width and therefore *could not* arm it. It is the shape
//! `set_root_box`'s own doc comment warns about: a pass wired into all but one of its call sites is
//! a pass that silently does not run on that one.
//!
//! ⚠⚠ **AND ARMING IT EXPOSED A SECOND DEFECT IN THE PATH IT ARMED.** `forced_reflow` rebuilt its
//! stylesheet list with `MinimalCascade::collect_style_elements` — **inline `<style>` only** — which
//! is the exact hazard `recascade_all_sources`'s doc comment was extracted to name (*"it would
//! quietly drop every external stylesheet"*), sitting unfixed in the reflow path because nothing
//! had been able to reach it. `css/css-grid/abspos/empty-grid-001.html` went **6 → 0** the moment
//! the lifecycle started reflowing, every row reading `width expected 0 but got 784`:
//! `.min-content` comes from `/css/support/width-keyword-classes.css`, and a re-cascade without it
//! gives every grid the full viewport width. Both halves are gated here, because either alone is
//! worse than neither — a reflow that runs with half the cascade replaces an old right answer with
//! a fresh wrong one.
//!
//! **To watch it go RED, two ways:** delete the `ReflowScope::install` from `fire_lifecycle` (the
//! `load` rows read 0 while both controls hold), or point `forced_reflow`'s sheet list back at
//! `collect_style_elements` (the `external` row reads the viewport width instead of the sheet's
//! 120).

use std::io::{Read, Write};
use std::net::TcpListener;

use manuk_text::FontContext;

/// Serves ONE stylesheet; everything else 404s, so a pass cannot come from anywhere but this sheet.
/// Same shape as `g_css_before_lifecycle`'s origin, for the same reason: an external sheet has to
/// arrive over the wire to be an external sheet.
fn css_origin() -> String {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in l.incoming() {
            let Ok(mut s) = stream else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 8192];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let path = req
                    .lines()
                    .next()
                    .and_then(|l| l.split_whitespace().nth(1))
                    .unwrap_or("/")
                    .to_string();
                let (status, ctype, body): (&str, &str, &str) = match path.as_str() {
                    "/site.css" => ("200 OK", "text/css", ".external{width:120px}\n"),
                    _ => ("404 Not Found", "text/plain", "no"),
                };
                let _ = s.write_all(
                    format!(
                        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\n\
                         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                );
            });
        }
    });
    format!("http://{addr}")
}

/// **One test, on purpose** — each JS gate spins its own SpiderMonkey runtime, and two in one
/// binary tear down messily enough to segfault *sometimes*, which is worse than failing.
#[test]
fn a_box_built_by_a_load_handler_has_geometry_and_the_forced_reflow_keeps_the_external_sheets() {
    let fonts = FontContext::new();
    let origin = css_origin();

    // Every width below is 550 or 120 or 90 — chosen so no two rows can be satisfied by the same
    // number, and none of them is the viewport width (which is what the two defects both produce).
    let html = format!(
        r#"<!doctype html><html><head><meta charset="utf-8">
             <link rel="stylesheet" href="{origin}/site.css">
             <style>.inline{{width:90px}}</style>
           </head><body style="margin:0">
             <div id="out">-</div>
             <div id="host"></div>
             <div id="parse-era" style="width:550px;height:10px"></div>
             <script>
               var R = [];
               function add(cls, css) {{
                 var d = document.createElement('div');
                 if (cls) d.className = cls;
                 d.style.cssText = css;
                 document.getElementById('host').appendChild(d);
                 return d;
               }}
               // CONTROL: appended during parse, read during parse.
               R.push('parse=' + add(null, 'width:550px;height:10px').offsetWidth);

               window.addEventListener('load', function () {{
                 // THE DEFECT: appended in the load handler, read in the same handler.
                 var d = add(null, 'width:550px;height:10px');
                 R.push('load=' + d.offsetWidth);
                 // ...and after writing the node's OWN style, which must also be seen.
                 d.style.width = '551px';
                 R.push('own-write=' + d.offsetWidth);
                 // THE SECOND HALF: a class only the EXTERNAL sheet defines.
                 R.push('external=' + add('external', 'height:10px').offsetWidth);
                 // CONTROL for it: the inline `<style>` half, which the broken reflow got right.
                 R.push('inline=' + add('inline', 'height:10px').offsetWidth);
                 setTimeout(function () {{
                   R.push('timer=' + d.offsetWidth);
                   R.push('parse-era=' + document.getElementById('parse-era').offsetWidth);
                   document.getElementById('out').textContent = R.join(' ');
                 }}, 0);
               }});
             </script>
           </body></html>"#
    );

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut page = rt.block_on(manuk_page::Page::load_async(
        &html,
        &format!("{origin}/index.html"),
        &fonts,
        800.0,
    ));
    rt.block_on(page.finish_loading(&fonts, 800.0));

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    let got = got.trim().to_string();
    println!("G_LOAD_GEOMETRY  {got}");

    assert_ne!(
        got, "-",
        "G_LOAD_GEOMETRY: the `load` handler's own report never landed, so nothing below was \
         measured. That is a larger bug than the one this gate is for — see G_LIFECYCLE."
    );

    for (claim, why) in [
        (
            "parse=550",
            "CONTROL — a node appended while the document is still parsing has always measured. If \
             this row moves, the gate is reporting on something other than the defect",
        ),
        (
            "load=550",
            "THE DEFECT — a node appended in a `load` handler read 0. `fire_lifecycle` was the one \
             script re-entry of eighteen that installed no `ReflowScope`, so a geometry read had \
             no way to force the layout its own mutation had invalidated",
        ),
        (
            "own-write=551",
            "a write to the node's OWN style must be seen too — with the hook missing even this \
             read 0, which is what proves nothing was recovering the node later",
        ),
        (
            "external=120",
            "THE SECOND HALF — `forced_reflow` rebuilt the cascade from inline `<style>` only, so \
             a class defined in a `<link>`ed sheet vanished and the box took the viewport width. \
             `sheets_of` is now the single implementation the page and the forced reflow share",
        ),
        (
            "inline=90",
            "CONTROL — the inline half was never lost, so the row above is specifically about \
             EXTERNAL sheets and not about the reflow's cascade in general",
        ),
        (
            "timer=551",
            "one task later the load-era node must still measure: a mutation made in a hook-less \
             round was invisible to every round after it as well, because the next scope recorded \
             the CURRENT dom sequence as one it had already laid out",
        ),
        (
            "parse-era=550",
            "CONTROL — a node that has existed since parse is unaffected by any of it",
        ),
    ] {
        assert!(
            got.split_whitespace().any(|r| r == claim),
            "G_LOAD_GEOMETRY: expected `{claim}` — {why}.\n  got: {got}"
        );
    }
}
