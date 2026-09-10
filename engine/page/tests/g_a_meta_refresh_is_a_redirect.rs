//! **G_A_META_REFRESH_IS_A_REDIRECT — the page that says "you are being redirected" and never was.**
//!
//! This engine handled exactly one `http-equiv` — `Content-Security-Policy` — and no other. A
//! declarative refresh is the oldest redirect on the web and it is still how corporate portals,
//! payment gateways, domain moves and half of enterprise software land the user somewhere else. On
//! this engine those pages rendered their stub — usually an empty `<body>` — and stopped.
//!
//! **Measured on the representative 200-site CrUX trend corpus (t1485): three of them are a
//! `<meta refresh>` stub AS THEIR HOMEPAGE** — `secure.paymentech.com` (222 bytes),
//! `www.datacareservices.com` (104 bytes), `linxonline.co.pierce.wa.us` (768). All three were filed
//! by the fidelity instrument as `probe-blocked`, whose doc comment asserts *"a page-supplied CSP, in
//! every case observed so far"* — and **zero of the eight sites carrying that tag have a CSP at
//! all.** The actual mechanism is that Chrome followed the refresh and the injected probe went with
//! the document it left.
//!
//! ## The grammar, arbitrated against headless Chrome — one variant per page, ten pages
//!
//! ```text
//!   http-equiv   content                     Chrome         why it is here
//!   refresh      0;url=dest.html             navigates      the ordinary spelling
//!   refresh      0; URL=dest.html            navigates      `url=` is case-insensitive
//!   Refresh      0;dest.html                 navigates      `url=` is OPTIONAL
//!   REFRESH      0;url='dest.html'           navigates      quotes are stripped
//!   refresh      "  0 ;  url  =  x  "        navigates      whitespace everywhere
//!   refresh      dest.html                   DOES NOT       the TIME is required
//!   refresh      0                           reloads self   no url = this document
//!   refresh      1;url=dest.html             navigates      a delay is still a refresh
//!   not-refresh  0;url=dest.html             DOES NOT       only `refresh` counts
//! ```
//!
//! ⚠ **THE SIXTH ROW IS THE ONE A HAND-WRITTEN PARSER GETS WRONG.** `content="dest.html"` is the
//! obvious spelling and Chrome refuses it: the value must begin with a time. A parser that split on
//! `;` and took the last field would navigate here — and would drag a page off itself for any other
//! use of the `refresh` name.
//!
//! ⚠ **AND THE HOST PERFORMS IT, NOT THE PAGE.** `Page::meta_refresh` reports; it does not navigate.
//! Same shape as `take_scroll_requests` and `take_form_submits`: a `Page` does not own the tab it is
//! displayed in, and a navigation that bypassed the host would leave the omnibox, the back stack and
//! the agent's own idea of *where am I* describing a document that is gone.
//!
//! ⚠ **RESIDUE:** the shell follows only a **sub-second** refresh. `content="5;url=…"` is a page
//! asking to be read first, and honouring it instantly would yank the document out from under the
//! user — strictly worse than not following it. Every redirect stub in the corpus is `0` or `1`.
//!
//! Mutations that must turn this red:
//!   1. accept a `content` with no leading time   -> the `bare` row navigates
//!   2. drop the `url=` prefix strip              -> `withUrl` reports "url=dest.html"
//!   3. drop the quote strip                      -> `quoted` reports "'dest.html'"
//!   4. match `http-equiv` case-sensitively       -> `upper` reports none
//!   5. return the raw url instead of resolving   -> every row loses its origin

use manuk_text::FontContext;

fn page_for(meta: &str) -> String {
    format!("<!doctype html><html><head><meta charset=utf-8>{meta}</head><body>stub</body></html>")
}

fn refresh_of(html: &str, fonts: &FontContext, rt: &tokio::runtime::Runtime) -> String {
    let page = rt.block_on(async {
        manuk_page::Page::load_async(html, "https://stub.test/a/b.html", fonts, 800.0).await
    });
    match page.meta_refresh() {
        Some((secs, url)) => format!("{secs}|{url}"),
        None => "none".to_string(),
    }
}

#[test]
fn a_declarative_refresh_is_reported_to_the_host_exactly_as_chrome_reads_it() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Runtime::new().unwrap();

    let rows: Vec<(&str, &str, &str)> = vec![
        // (name, the <meta>, expected "secs|absolute-url" or "none")
        (
            "plain",
            r#"<meta http-equiv="refresh" content="0;url=dest.html">"#,
            "0|https://stub.test/a/dest.html",
        ),
        (
            "upperUrl",
            r#"<meta http-equiv="refresh" content="0; URL=dest.html">"#,
            "0|https://stub.test/a/dest.html",
        ),
        (
            "noUrlEq",
            r#"<meta http-equiv="Refresh" content="0;dest.html">"#,
            "0|https://stub.test/a/dest.html",
        ),
        (
            "quoted",
            r#"<meta http-equiv="REFRESH" content="0;url='dest.html'">"#,
            "0|https://stub.test/a/dest.html",
        ),
        (
            "spaces",
            r#"<meta http-equiv="refresh" content="   0 ;   url  =  dest.html  ">"#,
            "0|https://stub.test/a/dest.html",
        ),
        // ⚠ THE ROW A HAND-WRITTEN PARSER GETS WRONG.
        (
            "bare",
            r#"<meta http-equiv="refresh" content="dest.html">"#,
            "none",
        ),
        // No url = reload THIS document, reported as the document's own URL.
        (
            "selfReload",
            r#"<meta http-equiv="refresh" content="0">"#,
            "0|https://stub.test/a/b.html",
        ),
        (
            "delayed",
            r#"<meta http-equiv="refresh" content="1;url=dest.html">"#,
            "1|https://stub.test/a/dest.html",
        ),
        (
            "absolute",
            r#"<meta http-equiv="refresh" content="0;url=https://other.test/x">"#,
            "0|https://other.test/x",
        ),
        (
            "notRefresh",
            r#"<meta http-equiv="not-refresh" content="0;url=dest.html">"#,
            "none",
        ),
        // Two metas: the FIRST wins — the later ones describe a page that will not exist.
        (
            "firstWins",
            r#"<meta http-equiv="refresh" content="0;url=one.html"><meta http-equiv="refresh" content="0;url=two.html">"#,
            "0|https://stub.test/a/one.html",
        ),
    ];

    let mut got = Vec::new();
    for (name, meta, _) in &rows {
        got.push(format!(
            "{name}={}",
            refresh_of(&page_for(meta), &fonts, &rt)
        ));
    }
    let line = got.join(" ");
    println!("META REFRESH: {line}");

    // ── VACUITY. The ORDINARY spelling must be found. Every "does not navigate" row below is
    //    satisfied by a parser that returns `None` for everything, and this is what forbids it.
    assert!(
        line.contains("plain=0|https://stub.test/a/dest.html"),
        "VACUOUS: the ordinary `0;url=…` spelling was not recognised, so every refusal row below is \
         passing on a parser that never finds anything — {line}"
    );

    let want: Vec<String> = rows.iter().map(|(n, _, w)| format!("{n}={w}")).collect();
    assert_eq!(
        line,
        want.join(" "),
        "\n  the declarative-refresh grammar must read exactly as Chrome reads it\n  want: {}\n  \
         got:  {line}",
        want.join(" ")
    );
}
