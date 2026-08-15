//! **G_DOCUMENT_WRITE — `document.write` exists, and its markup lands where the PARSER would have
//! put it.**
//!
//! It was **absent**, not partial: `grep -rn document.write engine/` returned zero. A 200-site CrUX
//! sweep, histogrammed by assertion message, puts `TypeError: document.write is not a function` at
//! **7 distinct sites (3.5%)** — `alastonsuomi.com`, `videa.hu`, `oilprice.com`, `nautica.com`,
//! `cyoinatu-onna.com`, `razaoautomovel.com`, `ru.restaurantguru.com`. It is the top *engine-gap*
//! throw in that histogram; the only two entries above it are one vendor script's own internal state
//! and a Cloudflare bot wall, neither of which is a gap of ours.
//!
//! `document.write` reads like 1997 trivia until you notice what still emits it: **ad and analytics
//! tags**, which inject their payload as `document.write('<script src=...>')` because that is the
//! only way to get a synchronous dependency into a parsing document. The throw takes the rest of the
//! inline script with it, and usually the boot sequence too.
//!
//! **Teeth, and the middle one is the real claim.** Existence alone would be satisfied by a no-op —
//! the shape this project files under *"the page is told YES and renders blank"* — so the gate
//! asserts the markup is (1) in the document, (2) **positioned as the parser would have positioned
//! it: the running `<script>`'s NEXT SIBLING**, not appended to `<body>`, and (3) live in the DOM as
//! real elements, queryable by id and by tag. A `write()` outside script execution has no insertion
//! point and must fall back to `<body>` rather than vanish.
//!
//! Proven RED: without the binding, every claim reads `err:...` because the first `write()` throws
//! and the inline script never reaches its own assertions. With a no-op binding, `w1-parent` and
//! `w1-next-sibling` fail while `w1-exists` passes — which is the reason position is asserted at all.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<div id="host">
  <span id="before">B</span>
  <script id="s1">
    try {
      // The ad-tag idiom, minus the network: write markup from a script that is mid-document.
      document.write('<i id="w1">written</i>');
      window.r = [];
      var w1 = document.getElementById('w1');
      window.r.push('w1-exists:' + !!w1);
      window.r.push('w1-text:' + (w1 && w1.textContent));
      window.r.push('w1-tag:' + (w1 && w1.tagName));
      // THE POSITION CLAIM. The parser would have made the written nodes the script's next
      // siblings, so `w1` must be the script's sibling inside #host — NOT appended to <body>.
      window.r.push('w1-parent:' + (w1 && w1.parentNode && w1.parentNode.id));
      var s1 = document.getElementById('s1');
      window.r.push('w1-next-sibling:' + (s1 && s1.nextElementSibling === w1));
      // Multiple arguments concatenate into ONE parse, and writeln appends a newline.
      document.write('<b id="w2">', 'two', '</b>');
      var w2 = document.getElementById('w2');
      window.r.push('w2-concat:' + (w2 && w2.textContent));
      window.r.push('writeln-is-fn:' + (typeof document.writeln === 'function'));
    } catch (e) {
      window.r = ['err:' + e];
    }
  </script>
</div>
<script id="s2">
  try {
    document.getElementById('out').textContent = window.r.join(' ');
  } catch (e) {
    document.getElementById('out').textContent = 'err:' + e;
  }
</script>
</body></html>"##;

#[test]
fn document_write_inserts_markup_where_the_parser_would_have() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://dw.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("DOCUMENT-WRITE RESULT: {got}");

    for claim in [
        "w1-exists:true",       // the method exists AND its markup reached the document
        "w1-text:written",      // as real content, not an escaped string
        "w1-tag:I",             // parsed as an ELEMENT, not inserted as text
        "w1-parent:host",       // — and placed where the PARSER would have: inside #host,
        "w1-next-sibling:true", // — immediately after the running <script>, not appended to <body>
        "w2-concat:two",        // document.write(a, b, c) is ONE string, not three calls
        "writeln-is-fn:true",   // the sibling nobody feature-detects separately
    ] {
        assert!(
            got.contains(claim),
            "G_DOCUMENT_WRITE: expected `{claim}`\n  got: {got}\n\n  \
             `document.write` must exist and must insert its markup as the running <script>'s next \
             siblings — that is where the parser's input stream would have put it. An `err:` here \
             means the method is missing outright (7 of 200 CrUX sites die on exactly that \
             TypeError); a passing `w1-exists` beside a failing `w1-parent` means the markup is \
             being appended somewhere convenient instead of where the page expects it."
        );
    }
}
