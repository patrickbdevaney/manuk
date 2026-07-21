//! **G_BLOB_STREAM — `Blob.prototype.stream()` returns a real byte ReadableStream.**
//!
//! `blob.stream()` used to return `null` — inert. A real one lets `blob.stream().pipeThrough(new
//! TextDecoderStream())` read a File/Blob incrementally and compose with the stream pipeline made real
//! in ticks 298/299. The teeth prove the actual BYTES come through and that the stream composes.
//!
//! Proven RED: restore `stream = function(){ return null; }` and `is-stream` is false while the read
//! throws.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
var r = [];
function push(s) { r.push(s); }
function finish() { document.getElementById('out').textContent = r.join(' '); }

try {
  var blob = new Blob(['hello']);
  var stream = blob.stream();
  push('is-stream:' + (stream !== null && typeof stream.getReader === 'function'));

  // read the bytes: 'hello' -> [104,101,108,108,111].
  var reader = stream.getReader();
  var bytes = [];
  function pump() {
    return reader.read().then(function (step) {
      if (step.done) { return; }
      for (var i = 0; i < step.value.length; i++) { bytes.push(step.value[i]); }
      return pump();
    });
  }
  pump().then(function () {
    push('bytes:' + (bytes.join(',') === '104,101,108,108,111'));

    // compose with a TextDecoderStream: blob.stream().pipeThrough(...) -> 'hello'.
    var out = '';
    var dec = new Blob(['world']).stream().pipeThrough(new TextDecoderStream());
    var dr = dec.getReader();
    function dpump() {
      return dr.read().then(function (step) {
        if (step.done) { return; }
        out += step.value;
        return dpump();
      });
    }
    return dpump().then(function () {
      push('pipe-decode:' + (out === 'world'));
      finish();
    });
  }).then(null, function (e) { push('CHAIN-THREW:' + e); finish(); });
} catch (e) {
  push('THREW:' + e);
  finish();
}
</script></body></html>"##;

#[test]
fn blob_stream_yields_real_bytes_and_composes() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://blobstream.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("BLOB-STREAM RESULT: {got}");

    for claim in [
        "is-stream:true",   // returns a real ReadableStream, not null
        "bytes:true",       // the blob's actual bytes come through
        "pipe-decode:true", // blob.stream().pipeThrough(TextDecoderStream) decodes to the text
    ] {
        assert!(
            got.contains(claim),
            "G_BLOB_STREAM: expected `{claim}`\n  got: {got}\n\n  \
             `blob.stream()` must return a real ReadableStream whose bytes are the blob's contents and \
             which composes with `pipeThrough(new TextDecoderStream())`."
        );
    }
}
