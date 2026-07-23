//! **G_CONTENTEDITABLE_PSEUDO — `:read-write`/`:read-only` honour `contenteditable`, in BOTH engines.**
//!
//! Follow-on to t454 (which matched `:read-only`/`:read-write` for inputs/textareas and left the
//! `contenteditable`-makes-`:read-write` edge unmodelled) and t456 (which defined `isContentEditable`).
//! Per CSS Selectors L4 / HTML, a `contenteditable` host — and any element inside it — is `:read-write`,
//! not `:read-only`. Both selector engines checked input/textarea only, so a rich-editor host
//! (`<div contenteditable>`) was styled by `:read-only` rules and missed by `:read-write` ones, disagreeing
//! with `el.isContentEditable`.
//!
//! A shared `is_contenteditable(dom, node)` (walk self→ancestors for the `contenteditable` attribute,
//! nearest explicit state wins — mirroring the t456 JS shim) now backs `:read-write`/`:read-only` in the
//! querySelector engine (`pseudo_matches`) AND the live Stylo cascade (`stylo_dom.rs`), so styling and
//! querying agree with editability.
//!
//! Teeth (RED-proven by reverting both arms to input/textarea-only):
//!   * `rw` — `:read-write` matches the contenteditable host AND a plain child inside it (inheritance),
//!     not a `contenteditable=false` island or a plain outside `<div>`.
//!   * `ro` — `:read-only` is the exact complement: the false-island and the outside div, not the host.
//!   * `styled` — the cascade `div:read-write { width:250px }` styles the contenteditable host.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  div { box-sizing: border-box; width: 100px; }
  div:read-write { width: 250px; }
</style></head><body style="margin:0">
<div id="host" contenteditable><span id="child">a</span><div id="island" contenteditable="false"><span id="locked">b</span></div></div>
<div id="outside">c</div>
</body></html>"##;

fn ids_matching(page: &manuk_page::Page, sel: &str) -> Vec<String> {
    let root = page.dom().root();
    manuk_css::query_selector_all(page.dom(), root, sel)
        .into_iter()
        .filter_map(|n| {
            page.dom()
                .element(n)
                .and_then(|e| e.attr("id"))
                .map(String::from)
        })
        .collect()
}

fn width_of(page: &manuk_page::Page, id: &str) -> f32 {
    let root = page.dom().root();
    let n = manuk_css::query_selector_all(page.dom(), root, &format!("#{id}"))[0];
    page.node_rects().get(&n).map(|r| r.width).unwrap_or(0.0)
}

#[test]
fn contenteditable_drives_read_write_read_only() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ce-pseudo.test/", &fonts, 800.0);

    // ── querySelector engine (pseudo_matches).
    let rw = ids_matching(&page, ":read-write");
    assert!(
        rw.contains(&"host".to_string()) && rw.contains(&"child".to_string()),
        "G_CONTENTEDITABLE_PSEUDO: `:read-write` must match a `contenteditable` host and a plain child \
         inside it (inheritance) — got {rw:?}. The querySelector engine ignores contenteditable."
    );
    assert!(
        !rw.contains(&"island".to_string())
            && !rw.contains(&"locked".to_string())
            && !rw.contains(&"outside".to_string()),
        "G_CONTENTEDITABLE_PSEUDO: `:read-write` must NOT match a `contenteditable=false` island, its \
         child, or a plain outside <div> — got {rw:?} (over-match)."
    );

    // `:read-only` is the exact complement.
    let ro = ids_matching(&page, ":read-only");
    assert!(
        ro.contains(&"island".to_string())
            && ro.contains(&"locked".to_string())
            && ro.contains(&"outside".to_string()),
        "G_CONTENTEDITABLE_PSEUDO: `:read-only` must match the false-island, its child, and the outside \
         <div> — got {ro:?}."
    );
    assert!(
        !ro.contains(&"host".to_string()) && !ro.contains(&"child".to_string()),
        "G_CONTENTEDITABLE_PSEUDO: `:read-only` must NOT match the editable host or its child — got {ro:?}."
    );

    // ── Live Stylo cascade (stylo_dom.rs): `div:read-write { width:250px }` styles the editable host,
    //    NOT the outside plain div (still 100) — the two engines now agree on editability.
    assert!(
        (width_of(&page, "host") - 250.0).abs() < 0.5,
        "G_CONTENTEDITABLE_PSEUDO: `div:read-write` must style the contenteditable host — #host width is \
         {}, not 250. The cascade matcher ignores contenteditable.",
        width_of(&page, "host")
    );
    assert!(
        (width_of(&page, "outside") - 100.0).abs() < 0.5,
        "G_CONTENTEDITABLE_PSEUDO: `div:read-write` must NOT style a plain outside <div> — #outside width \
         {} (over-match).",
        width_of(&page, "outside")
    );
}
