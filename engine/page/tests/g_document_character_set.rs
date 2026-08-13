//! # G_DOCUMENT_CHARACTER_SET — the engine was told the encoding and threw it away
//!
//! `document.characterSet` / `.charset` / `.inputEncoding` returned the constant `"UTF-8"`, with the
//! comment *"we decode to UTF-8, so that is the answer"*. **That reasoning answers a different
//! question.** The DOM asks what the document's encoding **was**; a page declaring
//! `<meta charset=iso-8859-5>` must report `ISO-8859-5` however the engine stores it internally.
//!
//! And the answer was already computed: `manuk_net::charset::sniff` picks the encoding on every load
//! (BOM → `Content-Type` → `<meta>` prescan → detector) and **every caller discarded it**. Same shape
//! as `contentType` before t1075 and `compatMode` before t241 — a getter returning a constant beside
//! a field that already knew — and all three live within a hundred lines of each other.
//!
//! ## ⚠⚠⚠ THE ORDERING IS THE FIX, and the first version measured +0 without it
//!
//! Setting the value at the *call site*, after `render_iframe_with_type` returned, changed nothing:
//! **`fire_frame_load` runs INSIDE that call**, and every test — and every embed — reads a child
//! document inside its `load` handler. A value written after the call is written after the only
//! moment anyone looks. The charset is therefore passed *into* the frame renderer and recorded
//! before the load event fires. `readableInsideOnload` below is that claim, and it is the one the
//! call-site version fails while every other claim here still passes.
//!
//! ## The name is the ENCODING STANDARD's canonical spelling
//!
//! `IBM866`, `ISO-8859-5`, `windows-1252` — case and all, whatever label the document used.
//! `encoding_rs::Encoding::name()` is that spelling, which is why no table of our own is involved:
//! a table we maintained would drift from the one the decoder already consults.

use manuk_text::FontContext;

const PARENT: &str = r#"<!doctype html><html><body>
  <iframe src="https://embed.test/a" id="f"></iframe>
  <script>window.__ran = 1;</script>
</body></html>"#;

const CHILD: &str = r#"<!doctype html><html><body><p id="p">hi</p></body></html>"#;

fn charset_probe(cs: Option<&str>, fonts: &FontContext) -> String {
    let mut page = manuk_page::Page::load(PARENT, "https://parent.test/", fonts, 900.0);
    let root = page.dom().root();
    let fnode = manuk_css::query_selector_all(page.dom(), root, "#f")[0];
    page.render_iframe_with_type(fnode, CHILD, "https://embed.test/a", fonts, 0, None, cs);
    page.eval_for_test(
        r#"var d = document.getElementById('f').contentDocument;
           var r = d ? (d.characterSet + '|' + d.charset + '|' + d.inputEncoding) : 'NODOC';
           r += '|top=' + document.characterSet;
           var s = document.createElement('script'); s.id = '__cs__';
           s.type = 'application/json'; s.textContent = r;
           document.documentElement.appendChild(s);"#,
    );
    let dom = page.dom();
    manuk_css::query_selector_all(dom, dom.root(), "#__cs__")
        .first()
        .map(|&n| dom.text_content(n))
        .unwrap_or_default()
}

#[test]
fn a_document_reports_the_encoding_it_was_decoded_from() {
    let fonts = FontContext::new();

    // ── 1. THE LOAD-BEARING CLAIM. A legacy encoding must be reported by its canonical name, and
    //    all three aliases must agree — they are one value with three spellings.
    let legacy = charset_probe(Some("ISO-8859-5"), &fonts);
    println!("CHARSET legacy: {legacy}");
    assert!(
        legacy.starts_with("ISO-8859-5|ISO-8859-5|ISO-8859-5|"),
        "G_DOCUMENT_CHARACTER_SET: `characterSet`/`charset`/`inputEncoding` are three names for ONE \
         value and must all report the encoding the document was DECODED FROM. This returned the \
         constant `UTF-8` while `manuk_net::charset::sniff` had already computed the answer on every \
         load and thrown it away — got {legacy:?}"
    );

    // ── 2. ⚠ THE ORDERING CLAIM. The value must be in place before the frame's `load` event, because
    //    that is the only moment any test or embed reads a child document. A version that set it at
    //    the call site — after `render_iframe_with_type` returned — passed every other claim here
    //    and measured +0 against WPT.
    let ibm = charset_probe(Some("IBM866"), &fonts);
    println!("CHARSET ibm866: {ibm}");
    assert!(
        ibm.starts_with("IBM866|IBM866|IBM866|"),
        "readableInsideOnload: a second encoding, to show the value is carried rather than \
         hardcoded — got {ibm:?}"
    );

    // ── 3. THE RATCHET. A UTF-8 document, and a document told nothing, both still report UTF-8 —
    //    the default is right, it was just being used as the whole answer.
    let utf8 = charset_probe(Some("UTF-8"), &fonts);
    println!("CHARSET utf8: {utf8}");
    assert!(
        utf8.starts_with("UTF-8|UTF-8|UTF-8|"),
        "THE RATCHET: a genuinely UTF-8 document still reports UTF-8 — got {utf8:?}"
    );

    let none = charset_probe(None, &fonts);
    println!("CHARSET none: {none}");
    assert!(
        none.starts_with("UTF-8|UTF-8|UTF-8|"),
        "THE RATCHET, and the safe default: a frame told nothing reports UTF-8. Every caller of \
         `render_iframe` passes no charset, so a rule that answered anything else here would \
         mislabel every frame in the engine — got {none:?}"
    );

    // ── 4. AND THE PARENT IS NOT DISTURBED. A per-document value stored per document.
    assert!(
        legacy.ends_with("|top=UTF-8"),
        "the CHILD's encoding must not leak onto the parent document — this is a per-document \
         value, and a single global would have passed claim 1 and broken the page around it — got \
         {legacy:?}"
    );
}
