//! **G_REFLOW_ADOPTED — a layout the scripts forced is kept, and only while it is still true.**
//!
//! A `load` handler that reads geometry makes `forced_reflow` cascade and lay out the whole tree. That
//! result used to die with its `ReflowScope`, and the next phase's `apply_stylesheets` did the same
//! work again over an identical tree (PERF.1.2.a: 5 passes of ~300 ms on puentedemando).
//!
//! Arm 1: mutate then read inside the handler -> the layout is adopted, so the next
//! `apply_stylesheets` has nothing to do (`RestyleDamage::None`) and the page shows the mutation.
//! (Arm 2, the staleness guard, is `g_reflow_stale_not_adopted.rs`: one page per process, because
//! SpiderMonkey's once-only `load` guard is shared by a thread's pages.)
//! Arm 2 (the staleness guard): read THEN mutate -> the forced layout predates the mutation, so adopting
//! it would paint a tree the DOM no longer matches; the next pass must still restyle.
//!
//! **How to break it:** drop the `laid_out_at != dom.mutation_seq()` check in `ReflowScope::into_fresh`
//! -> arm 2 reads `None` (and the mutation is never laid out).
use manuk_css::RestyleDamage;
use manuk_text::FontContext;
use std::collections::HashMap;

fn page(handler: &str) -> (manuk_page::Page, FontContext) {
    let fonts = FontContext::new();
    let html = format!(
        r##"<!doctype html><html><body><style>.tall{{height:50px}}</style><div id="host"></div>
<script>document.addEventListener('DOMContentLoaded', function () {{ {handler} }});</script></body></html>"##
    );
    // `Page::load` fires `load` itself (once-only), running the handler under a `ReflowScope`.
    let p = manuk_page::Page::load(&html, "https://adopt.test/", &fonts, 800.0);
    (p, fonts)
}

const ADD: &str = "document.getElementById('host').className='tall';";
const GROW: &str = "document.body.appendChild(document.createElement('p'));";
const READ: &str = "document.getElementById('host').getBoundingClientRect().height;";

#[test]
fn a_forced_layout_is_adopted_while_it_is_current() {
    let (mut p, fonts) = page(&format!("{ADD}{READ}"));
    let d = p.apply_stylesheets(&HashMap::new(), &fonts, 800.0);
    assert_eq!(
        d,
        RestyleDamage::None,
        "G_REFLOW_ADOPTED: the forced layout was thrown away and redone"
    );
    let host = manuk_css::query_selector_all(p.dom(), p.dom().root(), "#host")[0];
    let h = p
        .root_box
        .node_rects(p.dom())
        .into_iter()
        .find(|(n, _)| *n == host)
        .unwrap()
        .1
        .height;
    assert!(
        h >= 50.0,
        "G_REFLOW_ADOPTED: adopted layout does not contain the mutation (host height {h})"
    );
}
