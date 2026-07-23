//! **G_TEXTDECODER_ENCODINGS — `TextDecoder` must honour its label, not decode everything as UTF-8.**
//!
//! `new TextDecoder(label)` took its `label` and threw it away: every decoder decoded UTF-8. So the
//! legacy and UTF-16 content still all over the web came back as mojibake — Windows-authored CSV/HTML in
//! windows-1252, an `.decode()` of a non-UTF-8 `fetch` response, a binary protocol that frames its text
//! as UTF-16. `new TextDecoder('latin1').decode([0xE9])` returned garbage instead of `é`, and nothing
//! threw: the page just showed `Ã©`-shaped noise wherever a byte over 0x7F appeared.
//!
//! The claims check the decoded STRING for each label, and that the pre-existing UTF-8 (incl. streaming)
//! path is unchanged — each a way the old label-ignoring decoder goes RED:
//!
//!   * **`latin1` / `iso-8859-1`** map each byte to its Latin-1 character.
//!   * **`windows-1252`** decodes the 0x80-0x9F punctuation block (€, curly quotes) that raw Latin-1 lacks.
//!   * **`utf-16le` / `utf-16be`** read two bytes per code unit, honouring endianness.
//!   * **`.encoding`** reports the canonical name (the utf-16/ucs-2 aliases collapse to `utf-16le`).
//!   * **UTF-8 (default) still decodes multi-byte and still holds a split sequence under `{stream:true}`.**

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body><div id="out">-</div><script>
    var r = [];
    // latin1 / iso-8859-1: byte → Latin-1 char.
    r.push('latin1:' + new TextDecoder('latin1').decode(new Uint8Array([0xe9, 0xe8, 0xff])));
    r.push('iso:' + new TextDecoder('iso-8859-1').decode(new Uint8Array([0xe0])));
    // windows-1252: the 0x80-0x9F block — 0x80 is €, 0x92 is a curly apostrophe. Raw latin1 has neither.
    r.push('cp1252:' + new TextDecoder('windows-1252').decode(new Uint8Array([0x80, 0x92, 0x99])));
    // utf-16le / utf-16be: two bytes per unit; 0x20AC = €.
    r.push('u16le:' + new TextDecoder('utf-16le').decode(new Uint8Array([0x41, 0x00, 0xac, 0x20])));
    r.push('u16be:' + new TextDecoder('utf-16be').decode(new Uint8Array([0x00, 0x42, 0x20, 0xac])));
    // Canonical .encoding name (aliases collapse).
    r.push('enc:' + new TextDecoder('LATIN1').encoding + '/' + new TextDecoder('utf-16').encoding);
    // UTF-8 default is unchanged: € is three bytes.
    r.push('utf8:' + new TextDecoder().decode(new Uint8Array([0xE2, 0x82, 0xAC])));
    // UTF-8 streaming still holds a split multi-byte sequence.
    var d = new TextDecoder();
    var first = d.decode(new Uint8Array([0xC3]), { stream: true });
    var second = d.decode(new Uint8Array([0xA9]));
    r.push('stream:' + (first === '' ? 'held' : 'leaked') + '/' + second);
    document.getElementById('out').textContent = r.join(' ');
  </script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn textdecoder_honours_its_encoding_label() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://textdecoder-encodings.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "latin1:éèÿ",                // Latin-1 bytes decode to their characters
        "iso:à",                     // iso-8859-1 alias
        "cp1252:€’™", // windows-1252's 0x80-0x9F block (raw latin1 would give C1 controls)
        "u16le:A€",   // utf-16le honours byte order
        "u16be:B€",   // utf-16be honours byte order
        "enc:windows-1252/utf-16le", // canonical encoding names
        "utf8:€",     // UTF-8 default unchanged
        "stream:held/é", // {stream:true} still holds a split sequence, no regression
    ] {
        assert!(
            got.contains(claim),
            "G_TEXTDECODER_ENCODINGS: expected {claim} in {got:?}\n  \
             TextDecoder must honour its label — decoding a latin1 or utf-16 byte stream as UTF-8 is \
             silent mojibake on the legacy/Windows content still all over the web."
        );
    }
}
