//! **G_CLIPBOARD_IMAGE — `navigator.clipboard.read()` returns an IMAGE the user copied elsewhere.**
//!
//! The paste half of the Clipboard API carried `text/plain` only (tick 287); a screenshot copied in
//! another application came back as an empty text item, so the modern "paste an image" workflow —
//!
//! ```js
//! const items = await navigator.clipboard.read();
//! for (const it of items)
//!   if (it.types.includes('image/png')) drop(await it.getType('image/png'));  // an AI-chat / issue tracker
//! ```
//!
//! — never saw the picture. This wires the binary READ direction: the host seeds the real OS-clipboard
//! image via `manuk_js::set_host_clipboard_image(mime, bytes)`, the bridge hands it to JS as
//! `"<mime>;base64,<data>"`, and `read()` returns a `ClipboardItem` whose `getType(mime)` resolves a
//! real image `Blob` carrying the exact bytes.
//!
//! Teeth a stub cannot pass:
//!   * `has-image` — `read()` returns a `ClipboardItem` keyed by `image/png`. RED: drop the
//!     `__clipboardReadImage` handling in `read()` and only a text item comes back.
//!   * `blob-type`/`blob-size` — `getType('image/png')` resolves a Blob of the RIGHT type and byte
//!     length (not a text/plain wrapper of the base64 string).
//!   * `bytes` — the Blob's first bytes are the PNG signature the host seeded (0x89 'P' 'N' 'G'),
//!     proving the ACTUAL bytes round-tripped base64 → atob → Uint8Array → Blob, not a placeholder.
//!   * `no-text` — an image-only clipboard yields NO text/plain item, and `getType('text/plain')`
//!     rejects — a ClipboardItem is keyed only by the types it actually holds.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
var r = [];
function push(s) { r.push(s); }
function finish() { document.getElementById('out').textContent = r.join(' '); }

try {
  navigator.clipboard.read().then(function (items) {
    push('count:' + (Array.isArray(items) ? items.length : 'NA'));
    var imgItem = null, hasText = false;
    for (var i = 0; i < items.length; i++) {
      if (items[i].types.indexOf('image/png') >= 0) imgItem = items[i];
      if (items[i].types.indexOf('text/plain') >= 0) hasText = true;
    }
    push('has-image:' + (imgItem !== null));
    push('no-text:' + (hasText === false));
    if (!imgItem) { finish(); return; }
    return imgItem.getType('image/png').then(function (blob) {
      push('blob-type:' + (blob.type === 'image/png'));
      push('blob-size:' + (blob.size === 10));
      return blob.arrayBuffer().then(function (buf) {
        var b = new Uint8Array(buf);
        // PNG signature: 0x89 'P' 'N' 'G'
        push('bytes:' + (b[0] === 0x89 && b[1] === 0x50 && b[2] === 0x4E && b[3] === 0x47));
        // an image-only clipboard has no text/plain type on the image item
        return imgItem.getType('text/plain').then(
          function () { push('rejects-absent:false'); finish(); },
          function () { push('rejects-absent:true'); finish(); }
        );
      });
    });
  }, function (e) { push('THREW-CHAIN:' + e); finish(); });
} catch (e) { push('THREW:' + e); finish(); }
</script></body></html>"##;

#[test]
fn navigator_clipboard_read_returns_a_copied_image() {
    // The host seeds the OS clipboard with an IMAGE the user copied in another app: a 10-byte buffer
    // whose first 8 bytes are the real PNG signature (so `bytes` proves the exact bytes survived).
    let png = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x01];
    manuk_js::set_host_clipboard_image("image/png".to_string(), png);

    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://clip-img.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("CLIPBOARD-IMAGE RESULT: {got}");

    for claim in [
        "count:1",             // image-only clipboard → exactly one item
        "has-image:true",      // …keyed by image/png — the whole point
        "no-text:true",        // …and NOT a spurious text/plain item
        "blob-type:true",      // getType('image/png') is an image Blob
        "blob-size:true",      // …of the right byte length (10), not the base64 text
        "bytes:true",          // …carrying the actual PNG signature bytes the host seeded
        "rejects-absent:true", // getType('text/plain') rejects on an image-only item
    ] {
        assert!(
            got.contains(claim),
            "G_CLIPBOARD_IMAGE: expected `{claim}`\n  got: {got}\n\n  \
             navigator.clipboard.read() must return the real OS-clipboard IMAGE (host-seeded) as a \
             ClipboardItem whose getType(mime) resolves a Blob carrying the exact bytes — the \
             paste-a-screenshot path, not text/plain only."
        );
    }
}
