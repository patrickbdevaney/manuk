//! **G_AX_GENERATED_NAME (end-to-end) — a `::before` must reach the ACCESSIBLE NAME.**
//!
//! Generated content is **not in the DOM by construction** — script must never see it — and the
//! accessibility tree is built from the DOM. So there was **no path** by which a `::before` could
//! reach `accessible_name`, and every pseudo was silently missing from it (t1097). accname §4.3
//! step 2F requires the opposite: the pseudo text is part of name-from-content.
//!
//! ```text
//!   button::before{content:"★ "}                 Chrome "★ Save"      ours (pre-t1098) "Save"
//!   a::after{content:" (opens in a new tab)"}    Chrome "Docs (…)"    ours (pre-t1098) "Docs"
//! ```
//!
//! ⚠⚠⚠ **THIS FILE EXISTS BECAUSE THE UNIT TEST IN `manuk-a11y` CANNOT SEE THE PRODUCER.** That one
//! injects its own `GeneratedText` map, so it proves the *consumer* — the ordering, the negative
//! arm — and stays GREEN when the producer is unwired. Verified by mutation: replacing
//! `manuk_layout::generated_text(...)` with `&Default::default()` in `Page::a11y_tree` compiles
//! cleanly and leaves every `manuk-a11y` test passing while the whole feature is dead. **The two
//! halves need two tests, and this is the half that rides the real cascade + layout.**
//!
//! ⚠ The counter row is not decoration: it is the only assertion that the AX tree and the PAINTER
//! resolve counters through the *same* walk. If they ever diverge, a screen reader announces a
//! different section number from the one on the screen — a defect no pixel test can see.
//!
//! To watch it go RED: unwire the producer as above, or drop the `ContentPart::Counter` arm.

use manuk_page::Page;
use manuk_text::FontContext;

/// Walk the tree for the node whose name we want, by role + a substring we know is in it.
fn names(tree: &manuk_a11y::A11yNode, out: &mut Vec<String>) {
    if !tree.name.is_empty() {
        out.push(tree.name.clone());
    }
    for c in &tree.children {
        names(c, out);
    }
}

#[test]
fn generated_content_reaches_the_accessible_name_through_the_real_page() {
    let html = r#"<!doctype html><html><body>
<button id="b">Save</button>
<a id="l" href="/docs">Docs</a>
<h2 id="h">Alpha</h2>
<style>
  #b::before { content: "\2605  "; }  /* TWO spaces: a hex escape eats one as its delimiter */
  #l::after  { content: " (opens in a new tab)"; }
  body       { counter-reset: sec; }
  #h         { counter-increment: sec; }
  #h::before { content: "S" counter(sec) ". "; }
  #b::after  { content: "hidden"; display: none; }
</style>
</body></html>"#;

    let fonts = FontContext::new();
    let page = Page::load(html, "file:///t.html", &fonts, 800.0);
    let mut got = Vec::new();
    names(&page.a11y_tree(), &mut got);
    let all = got.join(" | ");

    assert!(
        all.contains("\u{2605} Save"),
        "accname §4.3 step 2F: a `::before` is part of the name, so the button is \"★ Save\". \
         Bare \"Save\" means the pseudo never reached the tree — which it could not before t1098, \
         because generated content is not in the DOM and the tree is built from the DOM.\n  got: {all}"
    );
    assert!(
        all.contains("Docs (opens in a new tab)"),
        "…and `::after` FOLLOWS the content. This is the case that matters most: the pseudo \
         carries text a sighted user gets from context and a screen-reader user gets only here.\n  \
         got: {all}"
    );
    assert!(
        all.contains("S1. Alpha"),
        "…and the counter is resolved by the SAME document-order walk the painter uses \
         (`manuk_layout::counter_snapshots`). A second copy of that walk would let the announced \
         section number drift from the printed one, which no pixel test can see.\n  got: {all}"
    );
    assert!(
        !all.contains("hidden"),
        "…and a `display:none` pseudo is NOT announced, on the same rule the painter drops it by \
         (t1093). Announcing content that is not rendered would make the tree more wrong than the \
         empty map this replaced.\n  got: {all}"
    );
}
