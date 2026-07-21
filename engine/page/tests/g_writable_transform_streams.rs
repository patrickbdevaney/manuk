//! **G_WRITABLE_TRANSFORM_STREAMS — real `WritableStream` + `TransformStream` + piping.**
//!
//! `WritableStream` and `TransformStream` were INERT NAMES: `typeof` said `'function'` but
//! `new WritableStream(...).getWriter` / `new TransformStream(...).readable` were `undefined`, so any
//! real use threw. That is the "typeof lies" failure class. Streaming pipelines — `body.pipeThrough(
//! transform).pipeTo(sink)` — need both to actually MOVE and reshape data. `ReadableStream` was already
//! real; this makes the write half and the middle real, and wires `pipeTo`/`pipeThrough`.
//!
//! The teeth prove DATA FLOW, which an inert stub cannot:
//!   * `writable` — chunks written to a WritableStream reach the underlying sink, in order.
//!   * `transform` — a TransformStream reshapes each chunk (here, doubling numbers) onto its readable.
//!   * `pipe` — `readable.pipeThrough(transform).pipeTo(writable)` delivers the transformed chunks.
//!
//! Proven RED: gate out the WritableStream block and `has-writer` is false while the pipe throws.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
var r = [];
function push(s) { r.push(s); }
function finish() { document.getElementById('out').textContent = r.join(' '); }

try {
  // WritableStream delivers to the sink.
  var got = [];
  var ws = new WritableStream({ write: function (c) { got.push(c); } });
  push('has-writer:' + (typeof ws.getWriter === 'function'));
  var w = ws.getWriter();

  // TransformStream that doubles each number.
  var ts = new TransformStream({ transform: function (c, ctrl) { ctrl.enqueue(c * 2); } });
  push('has-readable:' + (typeof ts.readable === 'object' && ts.readable !== null &&
                          typeof ts.writable === 'object'));

  Promise.resolve(w.write(1)).then(function () { return w.write(2); }).then(function () { return w.close(); })
    .then(function () {
      push('writable:' + (got.join(',') === '1,2'));

      // source -> transform (double) -> collect via pipeTo into a WritableStream.
      var collected = [];
      var src = new ReadableStream({ start: function (c) { c.enqueue(5); c.enqueue(10); c.close(); } });
      var sink = new WritableStream({ write: function (c) { collected.push(c); } });
      var t2 = new TransformStream({ transform: function (c, ctrl) { ctrl.enqueue(c * 2); } });

      return src.pipeThrough(t2).pipeTo(sink).then(function () {
        push('transform:' + (collected.join(',') === '10,20'));
        push('pipe:' + (collected.length === 2));
        finish();
      });
    }).then(null, function (e) { push('CHAIN-THREW:' + e); finish(); });
} catch (e) {
  push('THREW:' + e);
  finish();
}
</script></body></html>"##;

#[test]
fn writable_and_transform_streams_move_and_reshape_data() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://streams.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("STREAMS RESULT: {got}");

    for claim in [
        "has-writer:true",
        "has-readable:true",
        "writable:true",  // chunks reach the sink in order
        "transform:true", // pipeThrough doubles each chunk
        "pipe:true",      // the full pipe delivered both
    ] {
        assert!(
            got.contains(claim),
            "G_WRITABLE_TRANSFORM_STREAMS: expected `{claim}`\n  got: {got}\n\n  \
             `WritableStream`/`TransformStream` must MOVE data (write reaches the sink) and RESHAPE it \
             (transform enqueues onto readable), and `pipeThrough`/`pipeTo` must compose them. An \
             inert stub whose getWriter/readable is undefined fails."
        );
    }
}
