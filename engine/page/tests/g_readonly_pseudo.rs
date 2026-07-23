//! **G_READONLY_PSEUDO — `:read-only`/`:read-write` match in the querySelector engine, agreeing with the cascade.**
//!
//! The mutability pseudo-classes. An `<input>`/`<textarea>` WITHOUT a `readonly` attribute is
//! `:read-write`; a readonly control — and every non-editable element (a `<p>`, a `<div>`) — is
//! `:read-only` (CSS Selectors L4 / HTML). The live Stylo cascade already resolved both
//! (`stylo_dom.rs`: `P::ReadOnly`/`P::ReadWrite`), so `input:read-only { … }` rendered — but the
//! querySelector engine dropped them:
//!   * `:read-only` fell through to `_ => return None`, so the WHOLE selector was discarded and
//!     `querySelector(':read-only')` matched nothing, and
//!   * `:read-write` was modelled as `Pseudo::NeverStatic` — it never matched at all.
//!
//! So a form library doing `querySelectorAll('input:read-write')` (enumerate the editable fields) or
//! `':read-only'` (find the locked ones) got the wrong answer while the CSS styled them correctly — the
//! two-engines-disagree class t453 closed for `:disabled`.
//!
//! The RED probe: revert `parse_pseudo` (drop `read-only`, put `read-write` back under `NeverStatic`)
//! and `rw`/`ro`/`plain` collapse together while the cascade `styled` claim still passes — proving the
//! gate targets the querySelector half specifically.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  input           { box-sizing: border-box; width: 100px; }
  input:read-only { width: 300px; }
</style></head><body style="margin:0">
<input id="edit">
<input id="ro" readonly>
<textarea id="ta"></textarea>
<p id="para">text</p>
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
fn readonly_pseudo_matches_in_query_selector() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://readonly-pseudo.test/", &fonts, 800.0);

    // ── querySelector engine (pseudo_matches). `:read-write` = editable inputs/textareas only.
    let rw = ids_matching(&page, ":read-write");
    assert!(
        rw.contains(&"edit".to_string()) && rw.contains(&"ta".to_string()),
        "G_READONLY_PSEUDO: `:read-write` must match a plain <input> and <textarea> — got {rw:?}. \
         The querySelector engine still models `:read-write` as never-matching."
    );
    assert!(
        !rw.contains(&"ro".to_string()) && !rw.contains(&"para".to_string()),
        "G_READONLY_PSEUDO: `:read-write` must NOT match a readonly input or a non-editable <p> — got {rw:?}."
    );

    // `:read-only` = a readonly control OR any non-editable element.
    let ro = ids_matching(&page, ":read-only");
    assert!(
        ro.contains(&"ro".to_string()) && ro.contains(&"para".to_string()),
        "G_READONLY_PSEUDO: `:read-only` must match a `readonly` input and a non-editable <p> — got {ro:?}. \
         The querySelector engine dropped the whole selector (`:read-only` was an unknown pseudo)."
    );
    assert!(
        !ro.contains(&"edit".to_string()) && !ro.contains(&"ta".to_string()),
        "G_READONLY_PSEUDO: `:read-only` must NOT match an editable input/textarea — got {ro:?} (over-match)."
    );

    // A tag-qualified query is the load-bearing form-library idiom.
    let editable_inputs = ids_matching(&page, "input:read-write");
    assert!(
        editable_inputs == vec!["edit".to_string()],
        "G_READONLY_PSEUDO: `input:read-write` must be exactly the one editable <input> — got {editable_inputs:?}."
    );

    // ── Live Stylo cascade (stylo_dom.rs): `input:read-only { width:300px }` — already worked; this pins
    //    that the two engines now AGREE (the readonly input is both queried and styled; the plain one neither).
    assert!(
        (width_of(&page, "ro") - 300.0).abs() < 0.5,
        "G_READONLY_PSEUDO: `input:read-only` must style the readonly input — #ro width is {}, not 300.",
        width_of(&page, "ro")
    );
    assert!(
        (width_of(&page, "edit") - 100.0).abs() < 0.5,
        "G_READONLY_PSEUDO: `input:read-only` must NOT style an editable input — #edit width {} (over-match).",
        width_of(&page, "edit")
    );
}
