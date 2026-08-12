//! **G_DOCUMENT_URL_BASE — `document.URL` / `documentURI` and `node.baseURI` exist on EVERY
//! document, not just `window.document`.**
//!
//! ⚠⚠⚠ **`baseURI` DID NOT EXIST ANYWHERE — INCLUDING ON THE MAIN DOCUMENT.** Probed on a page
//! loaded from `https://dp.test/dir/page.html`, before this tick:
//!
//! ```text
//!                                                    BEFORE                          AFTER
//!   document.URL                     CONTROL         "https://dp.test/dir/page.html"  same  ✓
//!   document.baseURI                                  undefined                       ✓ ✗→✓
//!   DOMParser doc .URL                                undefined                       ✓ ✗→✓
//!   DOMParser doc .documentURI                        undefined                       ✓ ✗→✓
//!   DOMParser doc .baseURI                            undefined                       ✓ ✗→✓
//!   DOMParser XML doc .URL                            undefined                       ✓ ✗→✓
//! ```
//!
//! **The CONTROL row is what localises it.** `document.URL` worked, so this is not "URLs are
//! broken": `URL` and `documentURI` were defined as **own properties of `g.document`**, so every
//! document that is not the window's had none of them — and `baseURI` is a **`Node`** property that
//! was never built at all.
//!
//! ⚠⚠ **THE ABSENCE WAS ALREADY WRITTEN DOWN AND ROUTED AROUND.** `reflect_js.rs` carries
//! `new URL(raw, document.baseURI || location.href)` — the `|| location.href` half is exactly this
//! gap, documented as a fallback rather than fixed. A work-around in the tree is a bug report nobody
//! filed.
//!
//! ## Why it is worth a tick
//!
//! `baseURI` is how every framework resolves a relative URL it was handed, and `<base href>` is
//! standard in SPAs served from a sub-path. In WPT it is what `domparsing`'s four
//! `DOMParser-parseFromString-url*` files assert — 45 subtests each — which t1167 unblocked from
//! `harness=TIMEOUT` (the iframe `load` event) only for them to fail here instead.
//!
//! ## Measured — against the t1167 binary, same box, same session
//!
//! ```text
//!   WPT domparsing   149/1293 -> 190/1293   +41   ← CROSSES the 188 ratchet mark
//!   WPT dom         6366/10503 -> 6370/10503  +4
//! ```
//!
//! ⚠⚠⚠ **THAT +41 IS WHY THIS TICK MATTERS BEYOND ITS OWN AREA.** `domparsing` at 149 against a mark
//! of 188 was the SINGLE row holding the whole `docs/loop/WPT-AREAS.tsv` refresh (t1166 measured
//! +7,886 subtests corpus-wide and had to hold the file because of it). At 190 the ratchet's
//! objection is gone.
//!
//! ## Placement, and the one thing that must NOT change
//!
//! `URL`/`documentURI` go on `Document.prototype`, `baseURI` on `Node.prototype` — the same
//! placement `defaultView` already uses. ⚠ The existing own-properties on `g.document` still
//! **shadow** these for the main document, deliberately: they are accessors onto the live
//! `g.location`, which `__applyUrl` replaces wholesale on every SPA `pushState`, and a prototype
//! getter that won instead would go stale on the first navigation. The `pushstate` assertion below
//! pins that.
//!
//! ## How this goes RED
//!
//! Delete the `eval(… DOCUMENT_URL_JS …)` line in `event_loop.rs` → every `✗→✓` row returns to
//! `undefined` while the `document.URL` CONTROL and the `pushstate` row stay green. Verified:
//!
//! ```text
//!   ctlDocURL:true  baseHonoured:FALSE  dpURL:FALSE  dpURI:FALSE  dpTriple:FALSE
//!   xmlURL:FALSE    dpBase:FALSE        elemBase:FALSE           pushstate:true
//! ```
//!
//! ⚠ **`dpTriple` was VACUOUS on the first draft** — written as a bare `d.URL === d.documentURI` it
//! compares two `undefined`s and stays GREEN with the whole prelude severed. It now also requires
//! the value to be non-empty. Found by reading **every row** of the RED-proof rather than just its
//! verdict, which is the only way a vacuous row ever shows itself.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><base href="/base-dir/"></head><body>
<div id="out">-</div>
<script>
var o = [];
try {
  // ── CONTROL: this worked before the tick, and it is what says the defect is about WHICH object
  //    carries the property rather than about URLs.
  o.push('ctlDocURL:' + (document.URL === 'https://dp.test/dir/page.html'));

  // `<base href="/base-dir/">` resolved against the document URL — not assumed absolute.
  o.push('baseHonoured:' + (document.baseURI === 'https://dp.test/base-dir/'));

  var p = new DOMParser();
  var d = p.parseFromString('<html><body>x</body></html>', 'text/html');
  // A DOMParser document's URL is the RESPONSIBLE document's URL (DOM §DOMParser) — not
  // about:blank, not empty.
  o.push('dpURL:' + (d.URL === 'https://dp.test/dir/page.html'));
  o.push('dpURI:' + (d.documentURI === 'https://dp.test/dir/page.html'));
  // ⚠ `&& !!d.URL` is not decoration: written as a bare `===` this row compares two `undefined`s
  //   and stays GREEN with the whole prelude severed — a vacuous assertion, caught by running the
  //   RED-proof and reading every row rather than just the verdict.
  o.push('dpTriple:' + (d.URL === d.documentURI && !!d.URL));

  var x = p.parseFromString('<a/>', 'text/xml');
  o.push('xmlURL:' + (x.URL === 'https://dp.test/dir/page.html'));

  // A node's baseURI comes from ITS OWN document — a DOMParser document with no <base> falls back
  // to the document URL rather than inheriting the window's <base>.
  o.push('dpBase:' + (d.baseURI === 'https://dp.test/dir/page.html'));

  // An ordinary element inherits the property from Node.prototype, not just documents.
  o.push('elemBase:' + (document.getElementById('out').baseURI === 'https://dp.test/base-dir/'));

  // ⚠ The live-location requirement: the main document's own accessors must still win, so a
  //   pushState is visible immediately. A prototype getter that shadowed them would go stale here.
  history.pushState({}, '', '/moved/here.html');
  o.push('pushstate:' + (document.URL === 'https://dp.test/moved/here.html'));
} catch (e) { o.push('THREW:' + e); }
document.getElementById('out').textContent = o.join(' ');
</script></body></html>"##;

#[test]
fn document_url_and_base_uri_exist_on_every_document() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://dp.test/dir/page.html", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("G_DOCUMENT_URL_BASE RESULT: {got}");

    assert!(
        got.contains("ctlDocURL:true"),
        "G_DOCUMENT_URL_BASE: the CONTROL failed — `document.URL` on the MAIN document is wrong, so \
         nothing below is a test of prototype placement.\n  got: {got}"
    );

    for (claim, why) in [
        (
            "baseHonoured:true",
            "`document.baseURI` must resolve `<base href=\"/base-dir/\">` against the document URL. \
             It was UNDEFINED — on the main document — which is why `reflect_js.rs` carries \
             `document.baseURI || location.href` as a work-around.",
        ),
        (
            "dpURL:true",
            "A DOMParser-created document's URL is the RESPONSIBLE document's URL (DOM §DOMParser). \
             `URL` was an own property of `g.document`, so every other document had none.",
        ),
        ("dpURI:true", "`documentURI` is the same value as `URL`."),
        (
            "dpTriple:true",
            "…and they must agree with each other AND be non-empty. ⚠ Written as a bare `===` this \
             row compared two `undefined`s and stayed green with the fix removed — a vacuous \
             assertion, found by reading every row of the RED-proof instead of just its verdict.",
        ),
        (
            "xmlURL:true",
            "An XML document from the same parser gets the same URL — the rule is about the parser's \
             responsible document, not the MIME type.",
        ),
        (
            "dpBase:true",
            "A DOMParser document with no `<base>` falls back to its own document URL. It must NOT \
             inherit the WINDOW's `<base href>`, which is what reading the base off `g.document` \
             instead of the node's own document would do.",
        ),
        (
            "elemBase:true",
            "`baseURI` is a `Node` property, so an ordinary element has it too — it is not a \
             Document-only spelling.",
        ),
        (
            "pushstate:true",
            "⚠ The main document's OWN `URL`/`documentURI` accessors must keep shadowing the \
             prototype ones: they read the live `g.location`, which `__applyUrl` replaces wholesale \
             on every SPA navigation. A prototype getter that won here would go stale on the first \
             pushState.",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_DOCUMENT_URL_BASE: expected `{claim}`.\n  {why}\n  got: {got}"
        );
    }
}
