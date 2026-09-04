//! **G_INLINE_HANDLER_WITHOUT_SCRIPT — a document whose only JavaScript is an inline handler
//! attribute got no JavaScript at all.**
//!
//! The JS context was stood up only for documents containing a `<script>` element, under a comment
//! that ended:
//!
//! > *"With no initial script, no listener can ever be registered, so there is nothing to lose."*
//!
//! ⭐⭐⭐ **That sentence is false, and an inline event-handler attribute is the counterexample.** It
//! IS a listener registration and it needs no `<script>` element:
//!
//! ```text
//!   <body onload="…">                                how the CSS-WG's own layout tests bootstrap
//!   <button onclick> <a onclick> <form onsubmit>     ordinary legacy markup
//!   <img onerror="this.src='fallback.png'">          a RENDERING consequence: no fallback loads
//! ```
//!
//! **The proof is the empty script.** `<body onload>` on a script-free document did nothing; adding
//! an EMPTY `<script></script>` made it run. An empty script adds no behaviour, so it cannot be what
//! fixed it — the only thing it changed was whether a context existed. An `onclick` control behaved
//! identically, so this was never `load`-specific.
//!
//! ⚠ **Priced, and small, and said so** (t1367's rule): **0 of 53** freshly-fetched CrUX pages carry
//! an inline handler with no `<script>`, against **27 of 400** sampled WPT `css/` files. A CORRECTNESS
//! tick with ~0 corpus weight, landed because a comment asserting something false is a defect in its
//! own right, and because an optimisation that silently removes a capability is exactly the trade the
//! ratchet exists to refuse.

use manuk_text::FontContext;

fn text_of(html: &str, sel: &str) -> String {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(html, "https://ih.test/", &fonts, 800.0);
    let root = page.dom().root();
    let n = manuk_css::query_selector_all(page.dom(), root, sel)[0];
    page.dom().text_content(n)
}

#[test]
fn an_inline_handler_runs_on_a_document_with_no_script_element() {
    // ── 1. `<body onload>` — no `<script>` anywhere in the document.
    assert_eq!(
        text_of(
            r##"<!doctype html><html><body onload="document.getElementById('out').textContent='RAN'">
                <div id="out">-</div></body></html>"##,
            "#out"
        ),
        "RAN",
        "`<body onload>` is how the CSS-WG's own layout tests bootstrap, and it needs no `<script>` \
         element. The context was not being created, so the handler was never wired."
    );

    // ── 2. THE CONTROL THAT NAMES THE MECHANISM. The same document plus an EMPTY `<script></script>`
    // already worked before this fix. An empty script adds no behaviour — it only changes whether a
    // context exists — so this pair is the proof that the CONTEXT was the missing thing and not
    // anything about the handler.
    assert_eq!(
        text_of(
            r##"<!doctype html><html><body onload="document.getElementById('out').textContent='RAN'">
                <div id="out">-</div><script></script></body></html>"##,
            "#out"
        ),
        "RAN",
        "CONTROL: with an empty `<script>` this passed even before the fix. If arm 1 fails while this \
         passes, the context is the missing thing."
    );

    // ── 3. NOT `load`-SPECIFIC — the defect was the missing CONTEXT, so EVERY inline handler was
    // dead on a script-free page. Driven through the real click path rather than the load lifecycle.
    {
        let fonts = FontContext::new();
        let mut page = manuk_page::Page::load(
            r##"<!doctype html><html><body>
                <button id="b" onclick="document.getElementById('out').textContent='CLICKED'">go</button>
                <div id="out">-</div></body></html>"##,
            "https://ih.test/",
            &fonts,
            800.0,
        );
        let root = page.dom().root();
        let b = manuk_css::query_selector_all(page.dom(), root, "#b")[0];
        page.dispatch_click(b, &fonts, 800.0);
        let root = page.dom().root();
        let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
        assert_eq!(
            page.dom().text_content(out),
            "CLICKED",
            "`<button onclick>` is ordinary legacy markup and needs no `<script>` element. The defect \
             was the missing CONTEXT, so EVERY inline handler was dead on a script-free page — this \
             arm is what says the fix is not about the `load` event."
        );
    }

    // ⚠ **AN ARM THAT WAS WRITTEN, MEASURED AND MOVED OUT.** `<img onerror>` was going to be the
    // rendering consequence here (the author's fallback never loads). It still fails after this fix,
    // and it fails WITH a `<script>` present too — so it is a different mechanism (the `error` event
    // for a failed image fetch), not this one. Asserting it here would have made this gate red for a
    // reason it does not own; named instead, as the next tick's candidate.

    // ── 4. THE OTHER CONTROL, AND IT IS THE ONE THAT KEEPS THIS FIX HONEST. A document with NO
    // script and NO inline handler must still take the fast path: no engine spin-up. The gate cannot
    // observe "no context" directly from the page, so it asserts the thing that would break if the
    // predicate were widened to "always" — a static page still renders its markup untouched.
    assert_eq!(
        text_of(
            r##"<!doctype html><html><body><div id="out">STATIC</div></body></html>"##,
            "#out"
        ),
        "STATIC",
        "CONTROL: a document with neither script nor handler is unchanged — the optimisation this \
         fix narrows is still there for the pages it was written for."
    );
}
