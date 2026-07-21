//! **G_TEXT_CODEC_STREAMS — `TextDecoderStream` / `TextEncoderStream`.**
//!
//! The streaming text codecs that ride a fetch pipeline:
//! `res.body.pipeThrough(new TextDecoderStream())` turns byte chunks into decoded-string chunks WITHOUT
//! buffering the whole body — and correctly across a multi-byte character split over a chunk boundary
//! (the `{stream:true}` contract). They were absent. Built on the real TransformStream (tick 298).
//!
//! The teeth prove real streaming decode/encode:
//!   * `decode-split` — a UTF-8 `é` (0xC3 0xA9) split across two chunks decodes to one `café`, not two
//!     mojibake halves — the streaming boundary is honoured.
//!   * `encode` — a string chunk encodes to the right UTF-8 bytes.
//!
//! Proven RED: delete the `TextDecoderStream` block and `present` reads `undefined` while the pipe
//! throws.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
var r = [];
function push(s) { r.push(s); }
function finish() { document.getElementById('out').textContent = r.join(' '); }

try {
  push('present:' + (typeof TextDecoderStream === 'function' && typeof TextEncoderStream === 'function'));

  // stream "café" as bytes split mid-character: [c,a,f,0xC3] then [0xA9].
  var src = new ReadableStream({
    start: function (c) {
      c.enqueue(new Uint8Array([0x63, 0x61, 0x66, 0xC3]));
      c.enqueue(new Uint8Array([0xA9]));
      c.close();
    }
  });
  var out = '';
  var decoded = src.pipeThrough(new TextDecoderStream());
  var reader = decoded.getReader();
  function pump() {
    return reader.read().then(function (step) {
      if (step.done) { return; }
      out += step.value;
      return pump();
    });
  }
  pump().then(function () {
    push('decode-split:' + (out === 'café'));

    // encode a string chunk to UTF-8 bytes.
    var esrc = new ReadableStream({ start: function (c) { c.enqueue('AB'); c.close(); } });
    var encoded = esrc.pipeThrough(new TextEncoderStream());
    var er = encoded.getReader();
    var bytes = [];
    function epump() {
      return er.read().then(function (step) {
        if (step.done) { return; }
        for (var i = 0; i < step.value.length; i++) { bytes.push(step.value[i]); }
        return epump();
      });
    }
    return epump().then(function () {
      push('encode:' + (bytes.join(',') === '65,66'));
      finish();
    });
  }).then(null, function (e) { push('CHAIN-THREW:' + e); finish(); });
} catch (e) {
  push('THREW:' + e);
  finish();
}
</script></body></html>"##;

#[test]
fn text_codec_streams_decode_and_encode_across_chunks() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://codec.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("CODEC-STREAMS RESULT: {got}");

    for claim in [
        "present:true",
        "decode-split:true", // multi-byte char split across chunks decodes correctly
        "encode:true",       // string chunk -> UTF-8 bytes
    ] {
        assert!(
            got.contains(claim),
            "G_TEXT_CODEC_STREAMS: expected `{claim}`\n  got: {got}\n\n  \
             `TextDecoderStream`/`TextEncoderStream` must stream through a pipe, decoding a multi-byte \
             character split across chunk boundaries into one correct string and encoding back to \
             UTF-8 bytes."
        );
    }
}
