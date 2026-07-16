//! **G_CHARDATA — `element.click()` and the CharacterData interface, both of which simply did not exist.**
//!
//! Found by WPT (tick 44):
//!
//!   * **`element.click()` was missing.** It is how the web *activates* things — menus, modals,
//!     carousels, "click the hidden file input", every framework's programmatic activation, and every
//!     test that drives a UI. Its absence is a `TypeError` on the call, so whatever was running dies
//!     with it, and an `async_test` waiting for the resulting event **never completes**.
//!
//!   * **CharacterData was `data` and nothing else** — no `length`, `substringData`, `appendData`,
//!     `insertData`, `deleteData` or `replaceData`. WPT scored `CharacterData-replaceData` **0/34**,
//!     which is what "the method does not exist" looks like from the outside.
//!
//! **The offsets are UTF-16 code units, and that is the whole difficulty.** `"😀".length === 2` in
//! JavaScript, and offset 1 lands *inside* the surrogate pair. Rust strings are UTF-8, so a naive
//! implementation counting `char`s corrupts every emoji, every CJK surrogate and every combining
//! sequence on the web — silently, and only for the users who write in those scripts.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
  <button id="btn">go</button>
  <div id="out">-</div>
  <script>
    var R = [];

    // ── element.click() — the activation path.
    var clicks = 0, bubbled = 0;
    var btn = document.getElementById('btn');
    btn.addEventListener('click', function (e) { clicks += 1; });
    document.addEventListener('click', function (e) { bubbled += 1; });
    R.push('clickIsFn:' + (typeof btn.click === 'function'));
    btn.click();
    R.push('clicked:' + clicks);
    R.push('bubbled:' + bubbled);          // a click must BUBBLE — delegation depends on it

    // ── CharacterData, in UTF-16 code units.
    var t = document.createTextNode('hello');
    R.push('len:' + t.length);
    R.push('sub:' + t.substringData(1, 3));       // "ell"
    t.appendData(' world');
    R.push('append:' + t.data);
    t.insertData(5, ',');
    R.push('insert:' + t.data);                   // "hello, world"
    t.deleteData(5, 1);
    R.push('delete:' + t.data);                   // "hello world"
    t.replaceData(0, 5, 'HELLO');
    R.push('replace:' + t.data);                  // "HELLO world"

    // **The surrogate pair.** An emoji is TWO UTF-16 code units. If these offsets are counted in
    // Rust `char`s, this silently returns the wrong text — and only for the scripts that use it.
    var e = document.createTextNode('a😀b');
    R.push('emojiLen:' + e.length);               // 4, not 3
    R.push('emojiSub:' + (e.substringData(1, 2) === '😀'));

    // ── And it must THROW a real DOMException on a bad offset — that is what real code catches.
    var threw = 'no';
    try { t.substringData(999, 1); }
    catch (err) { threw = (err instanceof DOMException) ? err.name : ('wrong:' + err); }
    R.push('throws:' + threw);

    // ── The offsets/counts are WebIDL `unsigned long` = ToUint32 (modular, NOT clamped to 0). `-1`
    // becomes 4294967295, so an out-of-range negative is an IndexSizeError, and a large negative that
    // wraps back in bounds acts on the wrapped offset. The old code clamped negatives to 0, turning every
    // one of these into a silent in-bounds no-op.
    var u = document.createTextNode('test');
    var negThrew = 'no';
    try { u.deleteData(-1, 10); }               // -1 -> 4294967295 > length -> throws
    catch (err) { negThrew = (err instanceof DOMException) ? err.name : ('wrong:' + err); }
    R.push('negOffThrows:' + negThrew);
    u.insertData(-0x100000000 + 2, 'X');        // wraps to offset 2 -> "teXst" (NOT "Xtest")
    R.push('wrapIns:' + u.data);
    var v = document.createTextNode('test');
    R.push('bigOff:' + v.substringData(0x100000000 + 1, 1));   // wraps to offset 1 -> "e"
    R.push('negCount:' + v.substringData(0, -1));              // count 4294967295 -> clamps -> "test"

    // ── Required arguments are a TypeError BEFORE any DOM step — not a silent default.
    var argThrew = 'no';
    try { v.substringData(); } catch (err) { argThrew = err.constructor.name; }
    R.push('subNoArgs:' + argThrew);           // TypeError
    var appThrew = 'no';
    try { v.appendData(); } catch (err) { appThrew = err.constructor.name; }
    R.push('appNoArgs:' + appThrew);           // TypeError

    // ── `data` is [LegacyNullToEmptyString]: `node.data = null` is "", not the literal "null".
    var w = document.createTextNode('test');
    w.data = null;
    R.push('dataNull:[' + w.data + ']');       // []
    w.data = undefined;                        // ...but undefined stringifies normally
    R.push('dataUndef:' + w.data);             // undefined

    document.getElementById('out').textContent = R.join(' ');
  </script></body></html>"#;

#[test]
fn click_activates_and_character_data_is_a_real_interface_counted_in_utf16() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://cd.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        // click
        "clickIsFn:true",
        "clicked:1",
        "bubbled:1", // delegation — a click that does not bubble breaks every delegated handler
        // CharacterData
        "len:5",
        "sub:ell",
        "append:hello world",
        "insert:hello, world",
        "delete:hello world",
        "replace:HELLO world",
        // UTF-16, not chars
        "emojiLen:4",
        "emojiSub:true",
        // and it throws the RIGHT exception
        "throws:IndexSizeError",
        // ToUint32 (unsigned long), not clamp-to-0
        "negOffThrows:IndexSizeError",
        "wrapIns:teXst",
        "bigOff:e",
        "negCount:test",
        // required arguments are a TypeError
        "subNoArgs:TypeError",
        "appNoArgs:TypeError",
        // [LegacyNullToEmptyString]
        "dataNull:[]",
        "dataUndef:undefined",
    ] {
        assert!(
            got.contains(claim),
            "G_CHARDATA: expected `{claim}`\n  got: {got}\n\n  \
             `element.click()` is how the web activates things — its absence is a TypeError that takes \
             down whatever was running. CharacterData's methods are how every text-editing surface \
             mutates text. And `emojiLen:3` instead of `emojiLen:4` means the offsets are being counted \
             in Rust chars rather than UTF-16 code units, which silently corrupts every emoji and every \
             surrogate pair on the web."
        );
    }
}
