//! **G_CLIPBOARD_IMAGE_WRITE — `navigator.clipboard.write()` copies an IMAGE to the OS clipboard.**
//!
//! The write half carried `text/plain` only (tick 287): a page that copies a generated image —
//!
//! ```js
//! canvas.toBlob(blob =>
//!   navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })]));  // "copy chart" / "copy image"
//! ```
//!
//! resolved successfully while silently DROPPING the image, so nothing reached the OS clipboard. This
//! wires the binary WRITE direction, symmetric to the read landed in tick 461: `write()` reads the
//! image Blob's bytes, base64s them (a JS string is UTF-16; raw bytes are not valid text) and hands
//! them to the host via `__clipboardWriteImage`, which the host drains through
//! `manuk_js::take_pending_clipboard_image_writes()` to put on the real OS clipboard.
//!
//! Teeth a stub cannot pass:
//!   * `queued` — after `write([ClipboardItem({image/png})])`, exactly one image part is queued for the
//!     host. RED: revert `write()` to drop non-text parts and the queue is empty.
//!   * `mime` — the queued part's MIME is `image/png`, not a stringified blob.
//!   * `bytes` — the queued bytes are the EXACT bytes the page put in the Blob (the PNG signature),
//!     proving the round-trip Blob → btoa → base64 → host → b64_decode preserved them.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
var bytes = new Uint8Array([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x2A]);
var blob = new Blob([bytes], { type: 'image/png' });
var item = new ClipboardItem({ 'image/png': blob });
navigator.clipboard.write([item]).then(
  function () { document.getElementById('out').textContent = 'wrote'; },
  function (e) { document.getElementById('out').textContent = 'THREW:' + e; }
);
</script></body></html>"##;

#[test]
fn navigator_clipboard_write_copies_an_image_to_the_host() {
    // Drain any prior state so this test reads only its own write.
    let _ = manuk_js::take_pending_clipboard_image_writes();

    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://clip-imgw.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let marker = page.dom().text_content(out);
    println!("CLIPBOARD-IMAGE-WRITE MARKER: {marker}");

    // The page's write() promise must have resolved (not thrown) during the load microtask flush.
    assert!(
        marker.contains("wrote"),
        "the clipboard.write() promise did not resolve — got marker {marker:?}"
    );

    let writes = manuk_js::take_pending_clipboard_image_writes();
    println!("CLIPBOARD-IMAGE-WRITE QUEUE: {writes:?}");

    assert_eq!(
        writes.len(),
        1,
        "G_CLIPBOARD_IMAGE_WRITE: exactly one image part must be queued for the host — got {}. \
         clipboard.write() of a ClipboardItem({{'image/png': blob}}) must copy the image, not drop it.",
        writes.len()
    );
    let (mime, got_bytes) = &writes[0];
    assert_eq!(
        mime, "image/png",
        "G_CLIPBOARD_IMAGE_WRITE: wrong MIME: {mime:?}"
    );
    assert_eq!(
        got_bytes,
        &vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x2A],
        "G_CLIPBOARD_IMAGE_WRITE: the queued bytes must be the EXACT bytes the page put in the Blob \
         (Blob → btoa → base64 → host → b64_decode round-trip); got {got_bytes:?}"
    );
}
