//! **G_STRUCTURED_CLONE_BINARY — `structuredClone` must preserve BINARY types, not degrade them to
//! plain objects.**
//!
//! `structuredClone` is the deep-copy primitive the modern web is built on: it is what `postMessage`
//! serializes with (Manuk routes worker/window messaging through it), what a state library deep-copies
//! with, and the same structured-clone algorithm IndexedDB stores by. The shim already handled arrays,
//! `Date`, `Map`, `Set` and cyclic plain objects — but a **typed array fell into the plain-object
//! branch** and came back as `{0:.., 1:.., ...}`: the bytes appeared present, and every byte-oriented
//! consumer (a WASM loader reading the buffer, a `crypto.subtle` call, a canvas `ImageData`, a
//! `postMessage`d transferable) then read garbage — silently. That is the exact silent-corruption shape
//! this project refuses: a wrong answer that looks like a working one.
//!
//! The claims are checked on OBSERVABLE type + value survival, each a way the old plain-object copy
//! goes RED:
//!
//!   * **`Uint8Array`** clones to a real `Uint8Array` with the same bytes (not `{0:..}`).
//!   * **`ArrayBuffer`** clones to a real, INDEPENDENT `ArrayBuffer` (mutating the copy leaves the
//!     original untouched — a deep copy, not an aliased view).
//!   * **A non-byte typed array (`Float64Array`)** keeps its element type and values.
//!   * **Two views SHARING one buffer** clone to two views over ONE cloned buffer — a write through one
//!     view is visible through the other (buffer identity preserved, per the spec).
//!   * **`DataView`** clones to a real `DataView` reading the same value.
//!   * **`RegExp`** clones to a real `RegExp` whose `.test()` still works (a plain-object copy has no
//!     `.test`).

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body><div id="out">-</div><script>
    var r = [];
    // Uint8Array: real type + bytes, not a plain object.
    var u = structuredClone(new Uint8Array([10, 20, 30]));
    r.push('u8:' + (u instanceof Uint8Array) + '/' + u[0] + ',' + u[1] + ',' + u[2] + '/' + u.length);

    // ArrayBuffer: a real, INDEPENDENT buffer (mutating the clone must not touch the original).
    var srcBuf = new Uint8Array([1, 2, 3]).buffer;
    var cloneBuf = structuredClone(srcBuf);
    new Uint8Array(cloneBuf)[0] = 99;
    r.push('ab:' + (cloneBuf instanceof ArrayBuffer) + '/' + new Uint8Array(srcBuf)[0] + '/' + new Uint8Array(cloneBuf)[0]);

    // Float64Array: element type + values survive.
    var f = structuredClone(new Float64Array([1.5, -2.25]));
    r.push('f64:' + (f instanceof Float64Array) + '/' + f[0] + ',' + f[1]);

    // Two views over ONE buffer → two views over ONE cloned buffer (a write through one shows in the other).
    var shared = new ArrayBuffer(4);
    var a = new Uint8Array(shared), b = new Uint8Array(shared);
    var c = structuredClone({ a: a, b: b });
    c.a[0] = 77;
    r.push('shared:' + (c.a.buffer === c.b.buffer) + '/' + c.b[0]);

    // DataView: real type, same value.
    var dvBuf = new ArrayBuffer(4);
    new DataView(dvBuf).setInt32(0, 12345);
    var dv = structuredClone(new DataView(dvBuf));
    r.push('dv:' + (dv instanceof DataView) + '/' + dv.getInt32(0));

    // RegExp: source + flags survive; .test() still works.
    var re = structuredClone(/ab+c/gi);
    r.push('re:' + (re instanceof RegExp) + '/' + re.test('xxABBBCyy') + '/' + re.flags);

    document.getElementById('out').textContent = r.join(' ');
  </script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn structured_clone_preserves_binary_and_regexp_types() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://structuredclone-binary.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "u8:true/10,20,30/3", // Uint8Array clones to a real Uint8Array with its bytes
        "ab:true/1/99", // ArrayBuffer clone is INDEPENDENT: original still 1, clone mutated to 99
        "f64:true/1.5,-2.25", // Float64Array keeps element type and values
        "shared:true/77", // two views share ONE cloned buffer — a write through one shows in the other
        "dv:true/12345",  // DataView clones to a real DataView reading the same value
        "re:true/true/gi", // RegExp clones to a real RegExp whose .test() works, flags preserved
    ] {
        assert!(
            got.contains(claim),
            "G_STRUCTURED_CLONE_BINARY: expected {claim} in {got:?}\n  \
             structuredClone must preserve binary types (ArrayBuffer, typed arrays, DataView) and \
             RegExp — degrading a Uint8Array to a plain `{{0:..}}` object is silent data corruption \
             that every byte-oriented consumer (WASM, crypto, postMessage) then reads as garbage."
        );
    }
}
