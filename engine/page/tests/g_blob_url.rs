//! **G_BLOB_URL — a Blob object-URL carries real bytes: `canvas.toBlob` → `createObjectURL` → `fetch`.**
//!
//! `URL.createObjectURL(blob)` is how the web moves bytes it generated itself: an image editor's
//! "save", a chart library's PNG download, an upload preview (`URL.createObjectURL(file)` → `img.src`),
//! and every `canvas.toBlob(b => fd.append('file', b))` upload. Two halves have to both work or the
//! whole idiom is decoration:
//!
//!   1. `canvas.toBlob(cb)` must hand back a **real** Blob of what was drawn. The old stub called
//!      `cb(null)` — which is precisely what a real browser returns for a *tainted* cross-origin
//!      canvas, so a page testing for that took the "cannot export" branch and silently refused to
//!      save a canvas it fully owned. No error, no symptom, just a download button that does nothing.
//!   2. `fetch(URL.createObjectURL(blob))` must read those bytes back. A `blob:` URL is not a network
//!      resource; it names an in-process Blob. Without resolution the fetch goes to the network, which
//!      has no such host, and rejects.
//!
//! Why an integration gate and not a unit test: `typeof canvas.toBlob === 'function'` was true for the
//! entire life of the `cb(null)` stub, and `typeof URL.createObjectURL === 'function'` was true when it
//! only knew how to mint MediaSource attachment URLs. The capability is only real if a byte drawn on a
//! canvas comes back out the other end of a `fetch`, unchanged.
//!
//! **The claim that carries it** is `sig` + `roundtrip`: the eight-byte PNG signature survives
//! `toBlob` → `createObjectURL` → `fetch` → `arrayBuffer`, and the recovered length equals the Blob's
//! own `.size`. A PNG magic number cannot be faked by a stub that returns an empty 200, and it cannot
//! be produced at all if `toBlob` handed back `null`.
//!
//! **RED, run:** restore `el.toBlob = function(cb){ cb(null); }` → `toblob:null`, and every downstream
//! claim (which needs the blob) never records. Delete the `blob:` branch in `globalThis.fetch` → the
//! fetch hits the network and rejects → `sig`/`roundtrip` never succeed and `revoked` reads the wrong
//! way (the *first* fetch already failed). Make the branch return a constant empty 200 → `sig` fails
//! (no PNG bytes) and `roundtrip` fails (length 0 ≠ size). No constant satisfies all of them, because
//! `revoked:true` demands that the SECOND fetch (after `revokeObjectURL`) reject while the first
//! succeeded — the two are exact complements.
//!
//! One object-URL registry, not two: this reads back through the SAME `__mseLookup` store the MSE
//! attachment and the Worker `sourceOf` already consult. A second registry is the drift bug this
//! project keeps refusing.

use manuk_text::FontContext;

// A 4×4 canvas with four distinct quadrants — asymmetric on purpose so the bytes are non-trivial and a
// blank/constant raster could not accidentally match. The PNG signature check does not depend on the
// colours, but drawing real pixels keeps the fixture honest about exercising the raster path.
const HTML: &str = r#"<!doctype html><html><body>
<canvas id="c" width="4" height="4"></canvas>
<div id="out">-</div>
<script>
  var R = [];
  var flush = function () { document.getElementById('out').textContent = R.join(' '); };

  var c = document.getElementById('c');
  var x = c.getContext('2d');
  x.fillStyle = '#ff0000'; x.fillRect(0, 0, 2, 2);
  x.fillStyle = '#00ff00'; x.fillRect(2, 0, 2, 2);
  x.fillStyle = '#0000ff'; x.fillRect(0, 2, 2, 2);
  x.fillStyle = '#ffff00'; x.fillRect(2, 2, 2, 2);

  // toBlob is asynchronous by spec: this callback must land on a LATER turn. If it fired inline, the
  // gate could not tell a real async Blob from a synchronous stub.
  var firedInline = false, afterCall = false;
  c.toBlob(function (blob) {
    R.push('async:' + (afterCall === true));           // proof toBlob did not run synchronously
    R.push('toblob:' + (blob == null ? 'null' : (blob instanceof Blob ? 'blob' : typeof blob)));
    if (blob == null) { flush(); return; }
    R.push('type:' + blob.type);
    R.push('sizepos:' + (blob.size > 0));

    var u = URL.createObjectURL(blob);
    R.push('objurl:' + (typeof u === 'string' && u.indexOf('blob:') === 0));
    flush();

    fetch(u).then(function (r) {
      R.push('status:' + r.status);
      return r.arrayBuffer();
    }).then(function (buf) {
      var v = new Uint8Array(buf);
      // The PNG magic number: 89 50 4E 47 0D 0A 1A 0A. Its presence proves the exact bytes toBlob
      // wrote survived createObjectURL -> fetch -> arrayBuffer with no encoder in the path.
      R.push('sig:' + (v.length >= 8 && v[0] === 137 && v[1] === 80 && v[2] === 78 && v[3] === 71 &&
                       v[4] === 13 && v[5] === 10 && v[6] === 26 && v[7] === 10));
      R.push('roundtrip:' + (v.length === blob.size));
      flush();
      // Revoking must make the URL a network error, not merely un-list it. A second fetch now fails.
      URL.revokeObjectURL(u);
      return fetch(u);
    }).then(function () {
      R.push('revoked:false'); flush();                // a resolved fetch of a revoked URL is the bug
    }, function () {
      R.push('revoked:true'); flush();                 // rejected — the object URL is gone
    });
  }, 'image/png');
  afterCall = true;
  R.push('inline:' + firedInline);                     // false: the callback has not run yet
  flush();
</script></body></html>"#;

#[test]
fn blob_object_urls_carry_real_bytes_through_fetch() {
    let fonts = FontContext::new();
    // A fresh origin so nothing about the object-URL store can leak between runs.
    let page = manuk_page::Page::load(HTML, "https://blob-gate.test/", &fonts, 400.0);

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "inline:false",
            "toBlob's callback must not run synchronously — a page that reads a variable the callback \
             sets would find it undefined if it fired inline",
        ),
        (
            "async:true",
            "and when it does run, it is after the constructing turn finished, which is what 'async by \
             spec' means observably",
        ),
        (
            "toblob:blob",
            "toBlob hands back a real Blob, not the `null` a tainted canvas returns. The old stub \
             returned null and every canvas-export button silently did nothing",
        ),
        (
            "type:image/png",
            "the Blob is labelled with the format we ACTUALLY encoded, never the requested `type` we \
             did not honour — a PNG-bytes Blob labelled image/jpeg is the lie this refuses",
        ),
        (
            "sizepos:true",
            "the exported Blob has bytes; a zero-length Blob is a stub wearing the right shape",
        ),
        (
            "objurl:true",
            "createObjectURL returns a real `blob:` URL for a Blob, not only for a MediaSource",
        ),
        (
            "sig:true",
            "THE CLAIM: the eight-byte PNG signature survives toBlob -> createObjectURL -> fetch -> \
             arrayBuffer. A network fetch of blob: would have rejected; an empty 200 would have no \
             magic number",
        ),
        (
            "roundtrip:true",
            "and the recovered byte length equals the Blob's own .size — the bytes came back whole, \
             not truncated or re-encoded through a text codec",
        ),
        (
            "revoked:true",
            "revokeObjectURL really unregisters: the second fetch of the same URL rejects. Firing \
             nothing while leaving the URL live would let freed bytes stay readable",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_BLOB_URL: expected {claim} in {got:?}\n  {why}\n  \
             A Blob object-URL must carry real bytes end-to-end: canvas.toBlob produces a genuine PNG \
             Blob, createObjectURL mints a blob: URL for it, and fetch reads those exact bytes back. \
             The stub returned `cb(null)` and blob: fetches went to the network and failed — both are \
             invisible from the page's side until an export silently produces nothing."
        );
    }
}
