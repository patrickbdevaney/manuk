//! **G_CLIPBOARD_READ — the PASTE half of the async Clipboard API (`navigator.clipboard.read` /
//! `readText`).**
//!
//! The COPY half (`writeText`) already worked; PASTE did not. `readText()` returned only the text
//! THIS page had itself written, so a "paste from clipboard" button, a rich-text editor, or an
//! image-paste drop zone reading `navigator.clipboard.readText()` came back EMPTY whenever the user
//! had copied something in another application — which is the entire point of paste. This wires the
//! READ direction of the host clipboard bridge: the host seeds the real OS-clipboard text via
//! `manuk_js::set_host_clipboard(...)`, and `readText()`/`read()` pull it back.
//!
//! The gate has teeth a stub cannot pass:
//!   * `external` — `readText()` must resolve to text the page NEVER wrote (the host-seeded value).
//!     The old self-echo implementation returns `''` here and fails.
//!   * `item-getType` — `read()` returns a real `ClipboardItem`; `getType('text/plain')` resolves a
//!     Blob whose text is the clipboard contents.
//!   * `absent-rejects` — `getType('image/png')` REJECTS (the type is not present). A shim that
//!     resolves every requested type fails this — a `ClipboardItem` is keyed by the types it holds.
//!   * `roundtrip` — `writeText(x)` then `readText()` returns `x` (same-page copy→paste), proving the
//!     write and read halves share one clipboard cell.
//!
//! Proven RED: revert `readText` to `Promise.resolve(g.__clipboardText || '')` and `external` reads
//! empty; make `getType` resolve unconditionally and `absent-rejects` fails; drop the `__clipboardRead`
//! bridge registration and `external` reads empty again.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
var r = [];
function push(s) { r.push(s); }
function finish() { document.getElementById('out').textContent = r.join(' '); }

try {
  var c = navigator.clipboard;
  push('present:' + (typeof c === 'object' && c !== null &&
                     typeof c.readText === 'function' && typeof c.read === 'function'));

  // readText() must return what the USER copied elsewhere (host-seeded), not what this page wrote.
  c.readText().then(function (t) {
    push('external:' + (t === 'copied-in-another-app'));

    return c.read();
  }).then(function (items) {
    var item = items && items[0];
    push('read-array:' + (Array.isArray(items) && items.length === 1));
    push('types-text:' + (item && item.types && item.types.indexOf('text/plain') >= 0));

    // getType for a present type resolves a Blob carrying the clipboard text.
    return item.getType('text/plain').then(function (blob) {
      return blob.text().then(function (txt) {
        push('item-getType:' + (txt === 'copied-in-another-app'));
        // getType for an ABSENT type must reject — a ClipboardItem is keyed by the types it holds.
        return item.getType('image/png').then(
          function () { push('absent-rejects:false'); },
          function () { push('absent-rejects:true'); }
        );
      });
    });
  }).then(function () {
    // Same-page copy -> paste round-trip: writeText then readText returns the written text.
    return navigator.clipboard.writeText('page-wrote-this').then(function () {
      return navigator.clipboard.readText();
    });
  }).then(function (t) {
    push('roundtrip:' + (t === 'page-wrote-this'));
    finish();
  }, function (e) { push('THREW-CHAIN:' + e); finish(); });
} catch (e) {
  push('THREW:' + e);
  finish();
}
</script></body></html>"##;

#[test]
fn navigator_clipboard_read_is_a_real_paste_surface() {
    // The host seeds the OS clipboard with text this page never wrote — an external copy.
    manuk_js::set_host_clipboard("copied-in-another-app".to_string());

    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://clip.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("CLIPBOARD-READ RESULT: {got}");

    for claim in [
        "present:true",        // readText + read both exist
        "external:true", // readText sees what was copied in ANOTHER app — the paste that matters
        "read-array:true", // read() -> [ClipboardItem]
        "types-text:true", // the item is keyed by text/plain
        "item-getType:true", // getType('text/plain') -> Blob whose text is the clipboard contents
        "absent-rejects:true", // getType('image/png') rejects — the anti-stub tooth
        "roundtrip:true", // writeText then readText round-trips one clipboard cell
    ] {
        assert!(
            got.contains(claim),
            "G_CLIPBOARD_READ: expected `{claim}`\n  got: {got}\n\n  \
             `navigator.clipboard.readText()`/`read()` must return the REAL OS-clipboard contents \
             (host-seeded, incl. text copied in another app), not just this page's own last write. \
             A stub that echoes only self-written text fails `external`; one that resolves every \
             getType fails `absent-rejects`."
        );
    }
}
