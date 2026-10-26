//! **G_REFLOW_STALE_NOT_ADOPTED — a forced layout older than a mutation must not be adopted.**
//!
//! Companion of `g_reflow_adopted.rs` (own file: one page per process). A `load` handler reads
//! geometry (a forced reflow: it needs a prior mutation, else the read is already fresh) and THEN grows the tree. Adopting that layout would fingerprint the grown
//! tree as already styled and the next `apply_stylesheets` would skip the restyle that boxes the new node.
//!
//! **How to break it:** drop the `laid_out_at != dom.mutation_seq()` check in `ReflowScope::into_fresh`
//! -> `apply_stylesheets` returns `RestyleDamage::None` and this fails.
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

const PRE: &str = "document.getElementById('host').className='tall';";
const GROW: &str = "document.body.appendChild(document.createElement('p'));";
const READ: &str = "document.getElementById('host').getBoundingClientRect().height;";

#[test]
fn a_layout_older_than_a_mutation_is_not_adopted() {
    let (mut p, fonts) = page(&format!("{PRE}{READ}{GROW}"));
    let d = p.apply_stylesheets(&HashMap::new(), &fonts, 800.0);
    assert_ne!(
        d,
        RestyleDamage::None,
        "G_REFLOW_ADOPTED: a stale forced layout was adopted over a later mutation"
    );
}
