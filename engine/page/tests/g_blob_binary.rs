//! **G_BLOB_BINARY — a `Blob` built from BINARY parts must hold the bytes, not `String()` them.**
//!
//! `new Blob([bytes], { type })` is the single most common way binary data enters the platform: a
//! decoded image/audio buffer, a file upload body, `canvas.toBlob`, a `URL.createObjectURL` source, a
//! `fetch` request body. The shim stored every part as a UTF-16 string via `String(p)` — so a
//! `Uint8Array([1,2,3])` part became the literal text `"1,2,3"` (5 chars), and the blob reported
//! `size === 5` and handed back those five ASCII bytes instead of the three it was given. That is the
//! same silent binary-corruption shape as a lossy `structuredClone`: the data looks present and is
//! wrong, and every downstream reader (an upload, an image decoder, a hash) is quietly corrupted.
//! `FileReader.readAsArrayBuffer` was worse — a `new ArrayBuffer(0)` stub that returned nothing.
//!
//! The claims are checked on OBSERVABLE size + bytes, each a way the old `String()`-of-bytes goes RED:
//!
//!   * **`new Blob([Uint8Array])`** has `size` = the byte count and `arrayBuffer()` returns those bytes.
//!   * **`new Blob([ArrayBuffer])`** reads the buffer's bytes (not `"[object ArrayBuffer]"`).
//!   * **A typed-array VIEW** contributes only its own window of the buffer.
//!   * **Mixed string + binary parts** concatenate as bytes in order.
//!   * **`FileReader.readAsArrayBuffer`** returns the real bytes (not an empty buffer).
//!   * **String parts are UNCHANGED** — an ASCII text blob still reads back its text (no regression).

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body><div id="out">-</div><script>
    var r = [];
    var done = function () { document.getElementById('out').textContent = r.join(' '); };
    var pending = 0, finished = false;
    var track = function () { pending++; return function () { pending--; if (finished && pending === 0) done(); }; };
    var bytesOf = function (buf) { var u = new Uint8Array(buf); return Array.prototype.join.call(u, ','); };

    // Uint8Array part: size is the byte count, arrayBuffer() returns the bytes.
    var b1 = new Blob([new Uint8Array([1, 2, 3])]);
    r.push('u8size:' + b1.size);
    (function () { var d = track(); b1.arrayBuffer().then(function (buf) { r.push('u8bytes:' + bytesOf(buf)); d(); }, function () { r.push('u8bytes:ERR'); d(); }); })();

    // ArrayBuffer part: reads the buffer's bytes, not "[object ArrayBuffer]".
    var ab = new Uint8Array([9, 8, 7, 6]).buffer;
    r.push('absize:' + new Blob([ab]).size);

    // A typed-array VIEW over part of a buffer contributes only its window.
    var big = new Uint8Array([10, 20, 30, 40, 50]);
    var view = big.subarray(1, 4); // 20,30,40
    var b3 = new Blob([view]);
    r.push('viewsize:' + b3.size);
    (function () { var d = track(); b3.arrayBuffer().then(function (buf) { r.push('viewbytes:' + bytesOf(buf)); d(); }, function () { r.push('viewbytes:ERR'); d(); }); })();

    // Mixed string + binary parts concatenate as bytes, in order: 'AB' + [1,2] = 65,66,1,2.
    var b4 = new Blob(['AB', new Uint8Array([1, 2])]);
    r.push('mixsize:' + b4.size);
    (function () { var d = track(); b4.arrayBuffer().then(function (buf) { r.push('mixbytes:' + bytesOf(buf)); d(); }, function () { r.push('mixbytes:ERR'); d(); }); })();

    // FileReader.readAsArrayBuffer returns the real bytes (was an empty buffer).
    (function () {
        var d = track(); var fr = new FileReader();
        fr.onload = function () { r.push('frab:' + bytesOf(fr.result)); d(); };
        fr.onerror = function () { r.push('frab:ERR'); d(); };
        fr.readAsArrayBuffer(new Blob([new Uint8Array([4, 5, 6])]));
    })();

    // String parts are UNCHANGED — an ASCII text blob still reads back its text.
    (function () { var d = track(); new Blob(['hello']).text().then(function (t) { r.push('str:' + (t === 'hello' ? 'ok' : t) + '/' + 'hello'.length); d(); }, function () { r.push('str:ERR'); d(); }); })();

    finished = true; if (pending === 0) done();
  </script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn blob_holds_binary_bytes_not_stringified_parts() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://blob-binary.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "u8size:3",      // Uint8Array([1,2,3]) is 3 bytes, not the 5 chars of "1,2,3"
        "u8bytes:1,2,3", // and arrayBuffer() returns those bytes
        "absize:4",      // an ArrayBuffer contributes its byte length, not "[object ArrayBuffer]"
        "viewsize:3",    // a subarray view contributes only its 3-byte window
        "viewbytes:20,30,40",
        "mixsize:4", // 'AB' (2) + [1,2] (2) = 4 bytes
        "mixbytes:65,66,1,2",
        "frab:4,5,6", // readAsArrayBuffer returns real bytes, not an empty buffer
        "str:ok/5",   // a string blob is unchanged — no regression to the text path
    ] {
        assert!(
            got.contains(claim),
            "G_BLOB_BINARY: expected {claim} in {got:?}\n  \
             A Blob built from binary parts (Uint8Array/ArrayBuffer/DataView) must hold the BYTES — \
             `String()`-ing a typed array to \"1,2,3\" is silent data corruption that every binary \
             consumer (upload, image decode, hash) reads as garbage."
        );
    }
}
