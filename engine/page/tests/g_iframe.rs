//! **G_IFRAME — an `<iframe>` must have a box, and it must show its document.**
//!
//! 23% of the oracle corpus has one, and here **usage and damage are the same number**: we rendered a
//! zero-width box with nothing in it. `<iframe>` is the gateway to embeds, maps, video players, payment
//! frames and comment widgets — most of what makes a page feel like the modern web.
//!
//! Two bugs, and the first one is the embarrassing one:
//!
//!   1. **`iframe` was not in the replaced-element list at all**, in either cascade path. So it laid out
//!      at **zero width** — the box was gone before we ever got as far as failing to fetch its document.
//!      An unsized `<iframe>` is **300×150** by spec, which is not trivia: an iframe has no intrinsic
//!      size to fall back on, so with no default it collapses to nothing.
//!   2. Nothing ever fetched or rendered the child document.
//!
//! What this gate asserts, and — as importantly — what it asserts is **still true afterwards**:
//!
//!   - The box exists, at the author's size or the spec's default.
//!   - The child document renders into it (its pixels are not the background).
//!   - **First paint does not wait for it.** An iframe is the single most likely thing on a page to be
//!     slow; a page whose embed is a black hole must still paint its own article. That is G_FIRST_PAINT's
//!     rule, and an embed is exactly the thing that would break it.
//!   - **The child cannot reach the parent's DOM.** This comes free from the architecture — a
//!     `PageContext` is per-`Page` — but "it happens to be true" and "it is guaranteed" are different
//!     claims, and only one of them survives a refactor.

use manuk_text::FontContext;

#[test]
fn iframes_have_a_box_show_their_document_and_do_not_block_paint() {
    let fonts = FontContext::new();

    // ── (1) The box. This is what was actually broken: `iframe` was in no replaced-element list, so it
    //        laid out at zero width and the embed was invisible before any content question arose.
    let html = r#"<!doctype html><html><body style="margin:0">
        <iframe id="sized" src="https://embed.test/a" width="400" height="200"></iframe>
        <iframe id="unsized" src="https://embed.test/b"></iframe>
      </body></html>"#;
    let mut page = manuk_page::Page::load(html, "https://parent.test/", &fonts, 900.0);

    let root = page.dom().root();
    let sized = manuk_css::query_selector_all(page.dom(), root, "#sized")[0];
    let no_size = manuk_css::query_selector_all(page.dom(), root, "#unsized")[0];
    let rects = page.node_rects();
    let rect_of = |n| *rects.get(&n).expect("the iframe must have a layout box");

    let s = rect_of(sized);
    assert!(
        s.width == 400.0 && s.height == 200.0,
        "G_IFRAME: a sized iframe laid out at {}x{}, expected 400x200",
        s.width,
        s.height
    );

    let u = rect_of(no_size);
    assert!(
        u.width == 300.0 && u.height == 150.0,
        "G_IFRAME: an UNSIZED iframe laid out at {}x{}, expected the spec default 300x150.\n  \
         An iframe has no intrinsic size to fall back on, so with no default it collapses to nothing \
         and the embed is invisible before any question of its content arises. `iframe` was in no \
         replaced-element list at all, which is why it was ZERO WIDTH on 23% of the web.",
        u.width,
        u.height
    );

    // ── (2) First paint does not wait for the embed. The documents have NOT been fetched at this point
    //        — that happens after paint, on a background task — and the page must be perfectly paintable
    //        anyway. An iframe is the most likely thing on any page to be slow.
    assert_eq!(
        page.pending_iframes().len(),
        2,
        "G_IFRAME: the page must REPORT the iframes it still wants, having painted without them. \
         If this is 0, they will never load — 'fast' achieved by never loading the embed is the same \
         class of lie as 'fast' achieved by never loading the images."
    );

    // ── (3) The child document renders into the box.
    let child = r#"<!doctype html><html><body style="margin:0;background:#ff0000">
        <p id="inner">the embed</p></body></html>"#;
    page.render_iframe(sized, child, "https://embed.test/a", &fonts, 0);

    let canvas = page.paint(&fonts, 900, 400);
    let px = canvas.rgba_bytes();
    // Sample the middle of the sized iframe's box. It was filled red by the child document; if the
    // child did not render, this is the parent's white background.
    let (x, y) = (200usize, 100usize);
    let i = (y * 900 + x) * 4;
    let (r, g, b) = (px[i], px[i + 1], px[i + 2]);
    assert!(
        r > 200 && g < 80 && b < 80,
        "G_IFRAME: the iframe's box painted rgb({r},{g},{b}), but the child document's background is \
         RED. The nested document did not render into the frame.\n  \
         A 300x150 hole where an embed should be is what 23% of the web looked like."
    );

    // ── (3b) **`<body>`'s background propagates to the CANVAS**, and this is bigger than iframes.
    //
    //         CSS says the root element's background paints the whole canvas, and if the root has none,
    //         `<body>`'s is propagated up to it. We hard-coded WHITE. So every dark-themed page whose
    //         content is shorter than the viewport painted its content on a correct dark box floating in
    //         a **white void** — and it was found here, because an iframe's child document is exactly
    //         "a page shorter than its viewport". It was never an iframe bug.
    let dark = manuk_page::Page::load(
        r#"<html><body style="margin:0;background:#101014"><p>short</p></body></html>"#,
        "https://dark.test/",
        &fonts,
        400.0,
    );
    let dc = dark.paint(&fonts, 400, 600);
    let dpx = dc.rgba_bytes();
    let di = (550usize * 400 + 200) * 4; // well BELOW the content
    assert!(
        dpx[di] < 40 && dpx[di + 1] < 40 && dpx[di + 2] < 40,
        "G_IFRAME/canvas: a dark page painted rgb({},{},{}) below its content, expected its own dark \
         background. `<body>`'s background must propagate to the canvas — otherwise every dark site on \
         the web is a dark box floating in a white void.",
        dpx[di], dpx[di + 1], dpx[di + 2]
    );

    // ── (4) Isolation. The child is a whole `Page` with its OWN JS context, so its script has no path
    //        to the parent's DOM — it cannot reach it because it does not have it. That is a guarantee
    //        from the architecture, not a policy anyone has to remember; this pins it so a refactor
    //        cannot quietly turn it into a coincidence.
    let hostile = r#"<html><body><script>
        try { parent.document.body.setAttribute('data-pwned', 'yes'); } catch (e) {}
        try { top.document.title = 'pwned'; } catch (e) {}
      </script></body></html>"#;
    page.render_iframe(no_size, hostile, "https://evil.test/", &fonts, 0);
    let body = manuk_css::query_selector_all(page.dom(), page.dom().root(), "body")[0];
    assert!(
        page.dom().element(body).and_then(|e| e.attr("data-pwned")).is_none(),
        "G_IFRAME: a child frame's script reached into the PARENT's DOM. Frames are the boundary \
         between a page and a third party, and a payment iframe that the host page can rewrite is not \
         a payment iframe."
    );

    // ── (5) **`contentDocument` / `contentWindow` — the nested browsing context is READABLE.**
    //
    //        This is the tick's whole point and the single largest gated capability the project has
    //        found: a script in the parent reaches into a frame's document and reads it back. It is
    //        WPT's entire `encoding` suite (767,003 subtests) — `iframeRef(f).querySelectorAll(...)` —
    //        and it is the platform web itself: embeds, OAuth frames, payment fields, comment widgets.
    //
    //        The child document was always built and then thrown away; this pins that it survives, that
    //        a reflector resolves against ITS OWN arena (not the parent's — the bug that made this
    //        impossible), and that node identity across the document boundary holds.
    let mut fr = manuk_page::Page::load(
        r#"<!doctype html><html><body>
             <iframe src="https://embed.test/child" id="f"></iframe>
             <span id="p">parent-span</span>
             <script>window.__x = 1;</script>
           </body></html>"#,
        "https://parent.test/",
        &fonts,
        900.0,
    );
    let froot = fr.dom().root();
    let fnode = manuk_css::query_selector_all(fr.dom(), froot, "#f")[0];
    fr.render_iframe(
        fnode,
        r#"<!doctype html><html><head><title>frame-title</title></head>
             <body><span data-cp="A7">child-a</span> <span data-cp="A8">child-b</span></body></html>"#,
        "https://embed.test/child",
        &fonts,
        0,
    );
    fr.eval_for_test(
        r#"var r = [];
           function iframeRef(f){ return f.contentWindow ? f.contentWindow.document : f.contentDocument; }
           var f = document.getElementById('f');
           r.push('cd=' + (typeof f.contentDocument));
           r.push('divcd=' + (typeof document.getElementById('p').contentDocument));
           var d = iframeRef(f);
           var ns = d.querySelectorAll('span');
           r.push('childSpans=' + ns.length);
           r.push('cp0=' + (ns[0] && ns[0].getAttribute('data-cp')));
           r.push('text0=' + (ns[0] && ns[0].textContent));
           r.push('title=' + d.title);
           r.push('parentSpans=' + document.querySelectorAll('span').length);
           r.push('identity=' + (f.contentDocument.querySelector('span') === d.querySelector('span')));
           var s = document.createElement('script'); s.id = '__cd__'; s.type = 'application/json';
           s.textContent = r.join('|'); document.documentElement.appendChild(s);"#,
    );
    let dom = fr.dom();
    let out = manuk_css::query_selector_all(dom, dom.root(), "#__cd__");
    let report = out
        .first()
        .map(|&n| dom.text_content(n))
        .unwrap_or_default();
    // A frame's document must exist, a non-frame's must not, the child's OWN nodes must come back (2
    // spans, not the parent's 1), the child's data must decode, and `===` must hold across the boundary.
    for needle in [
        "cd=object",
        "divcd=undefined",
        "childSpans=2",
        "cp0=A7",
        "text0=child-a",
        "title=frame-title",
        "parentSpans=1",
        "identity=true",
    ] {
        assert!(
            report.contains(needle),
            "G_IFRAME/contentDocument: expected `{needle}` — a script must read a frame's own document \
             through `contentWindow.document`/`contentDocument`, and get the CHILD's nodes, not the \
             parent's. This gates WPT's entire `encoding` suite (767k subtests) and the platform web \
             (embeds, OAuth, payments). Full report: {report}"
        );
    }
}
