//! **G_CSS_SUPPORTS_HOOK — `CSS.supports()` answered `false` for EVERYTHING on the shell's path and
//! the agent's, including `display: flex`.**
//!
//! The hook that lets `CSS.supports()` ask the real CSS engine was installed in **`Page::load` only**
//! — the synchronous path. `load_async` (the agent and every fidelity measurement) and
//! `from_prefetched` (**the shell**, i.e. the shipping browser) never installed it, so the JS API
//! answered `false` to every question:
//!
//! ```text
//!   CHROME  width:5px=true   display:flex=true   color:red=true   width:5lh=true
//!   MANUK   width:5px=FALSE  display:flex=FALSE  color:red=FALSE  width:5lh=FALSE
//! ```
//!
//! **`display: flex` is the tell.** The Rust-level `supports_condition` has asserted that exact
//! string true since it was written and its unit test passes — the engine knew the answer and no page
//! could reach it. Three consecutive ticks (721, 722, 723) recorded this as *"a false negative on
//! `lh`/`rlh`/`rch`"* while chasing units; it was never about units.
//!
//! **A false NEGATIVE on feature detection is not a missing feature — it is every
//! progressive-enhancement guard on the web taking its fallback.** `CSS.supports()` is how a site
//! decides whether to ship the grid layout or the float one, the scroll-snap carousel or the manual
//! one, `display:flex` or a table. Answering `false` to all of it does not degrade gracefully; it
//! silently selects the 2015 codepath on a browser that can run the 2026 one.
//!
//! ⚠ The fix is a *move*, not an addition: `install_supports_hook()` now runs in `Page::from_dom`,
//! the one function every construction path goes through. **Three callers is what produced one.**
//!
//! ⚠ Named residual, pinned by assertion (4): `CSS.supports('width','5cqw')` is `true` in Chrome and
//! `false` here — container-query length units are a genuine, separate gap, and the honest `false`
//! is not this hook's fault.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
 <div id="pos">-</div><div id="neg">-</div><div id="compound">-</div><div id="cq">-</div>
 <script>
   window.addEventListener('load', function () {
     // Both call shapes: one-argument condition and the two-argument (property, value) form.
     document.getElementById('pos').textContent = [
       CSS.supports('display: flex'),
       CSS.supports('display', 'flex'),
       CSS.supports('color: red'),
       CSS.supports('width', '5px'),
       CSS.supports('width', '5lh'),
       CSS.supports('width', '5rch')
     ].join(',');
     // A hook that answers `true` unconditionally passes every assertion above. These are what stop
     // that: an unknown property, an unknown value, and an unknown unit must all be false.
     document.getElementById('neg').textContent = [
       CSS.supports('manuk-not-a-prop', '1px'),
       CSS.supports('display', 'manuk-not-a-value'),
       CSS.supports('width:5manukunit')
     ].join(',');
     // `not` and `and` prove the CONDITION grammar is reaching the engine, not just a declaration.
     document.getElementById('compound').textContent = [
       CSS.supports('not (display: flex)'),
       CSS.supports('(display: flex) and (color: red)')
     ].join(',');
     document.getElementById('cq').textContent = String(CSS.supports('width', '5cqw'));
   });
 </script>
</body></html>"#;

fn text(page: &manuk_page::Page, sel: &str) -> String {
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, sel);
    assert!(!hits.is_empty(), "{sel} must exist");
    page.dom().text_content(hits[0])
}

/// **One test, on purpose** — see `g_defer`.
#[test]
fn css_supports_answers_from_the_engine_on_every_construction_path() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    // ⚠ `load_async`, NOT `Page::load`. `Page::load` is the one path that already installed the hook,
    // so a gate written against it would have passed throughout the entire bug.
    let page = rt.block_on(manuk_page::Page::load_async(
        HTML,
        "https://supports.test/",
        &fonts,
        800.0,
    ));

    let pos = text(&page, "#pos");
    let neg = text(&page, "#neg");
    let compound = text(&page, "#compound");
    let cq = text(&page, "#cq");
    println!("CSS-SUPPORTS pos=[{pos}] neg=[{neg}] compound=[{compound}] cqw={cq}");

    // (1) **The engine's own answers reach the page.** RED: remove `install_supports_hook()` from
    // `Page::from_dom` → `false,false,false,false,false,false`, which is what the shell shipped.
    assert_eq!(
        pos, "true,true,true,true,true,true",
        "CSS.supports must answer TRUE for supported declarations in BOTH call shapes — got \
         {pos:?}. All-false means the hook is not installed on this construction path; \
         `display: flex` is the tell, because the engine's own unit test asserts it true."
    );

    // (2) **…and it is not a stub returning `true`.** RED: make the hook `|_| true` → all true here.
    assert_eq!(
        neg, "false,false,false",
        "an unknown property, an unknown value and an unknown unit must all be FALSE — got \
         {neg:?}. A hook that answers `true` unconditionally satisfies assertion (1) and is a worse \
         bug than the one being fixed: it makes a page ship a codepath this engine cannot run."
    );

    // (3) **The condition GRAMMAR reaches the engine**, not merely a bare declaration — `not` must
    // invert and `and` must conjoin. RED: wrap every input in parentheses unconditionally → `not (…)`
    // becomes `(not (…))` and stops parsing.
    assert_eq!(
        compound, "false,true",
        "`not (display: flex)` must be false and `(display: flex) and (color: red)` true — got \
         {compound:?}"
    );

    // (4) **THE NAMED RESIDUAL, PINNED.** Container-query length units are a real, separate gap:
    // Chrome answers `true` here and this engine answers `false`, honestly. If this ever reads
    // `true`, `cqw` landed — delete this assertion and say so in the journal.
    assert_eq!(
        cq, "false",
        "container-query units are not supported in this build, so CSS.supports must say so \
         (Chrome: true). If this now reads `true`, cqw landed; update the gate."
    );
}
