//! **G_CLIPBOARD_GESTURE — SEC.DID.6 (SECURITY.md P6/P6b): the async Clipboard API needs a user gesture.**
//!
//! A page script with no gesture must neither READ the OS clipboard (P6) nor OVERWRITE it (P6b),
//! even when the host clipboard is seeded. Agent sessions have no human gesture, so this is also the
//! "empty in agent mode" guarantee.
//!
//!   * `read-denied` / `read-denied-item` — `readText()` / `read()` with no gesture reject
//!     (`NotAllowedError`) and never expose the seeded secret.
//!   * `write-denied` — `writeText()` with no gesture rejects and queues NOTHING for the OS bridge
//!     (`take_clipboard_writes()` is empty).
//!   * `read-ok` / `write-ok` — inside a real click the same calls work (the gate is not a blanket ban).
//!
//! RED: drop the `__clipDenied()` check in `readText`/`read` and `read-denied*` leak the secret;
//! drop it in `writeText` and `write-denied` finds a queued write.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<button id="go">go</button><div id="out">-</div>
<script>
var r = [];
function done() { document.getElementById('out').textContent = r.join(' '); }
var c = navigator.clipboard;
// No gesture (load time): all must reject.
c.readText().then(function (t) { r.push('read-denied:false(' + t + ')'); },
                  function (e) { r.push('read-denied:' + (e.name === 'NotAllowedError')); })
.then(function () { return c.read(); })
.then(function (i) { r.push('read-denied-item:false'); },
      function (e) { r.push('read-denied-item:' + (e.name === 'NotAllowedError')); })
.then(function () { return c.writeText('EVIL-OVERWRITE'); })
.then(function () { r.push('write-denied:false'); },
      function (e) { r.push('write-denied:' + (e.name === 'NotAllowedError')); })
.then(function () { r.push('stage1'); done(); });
document.getElementById('go').addEventListener('click', function () {
  c.readText().then(function (t) { r.push('read-ok:' + (t === 'SECRET-IN-OS-CLIPBOARD')); });
  c.writeText('user-copy').then(function () { r.push('write-ok:true'); });
});
</script></body></html>"##;

#[test]
fn clipboard_read_and_write_require_a_user_gesture() {
    let _ = manuk_js::take_clipboard_writes();
    manuk_js::set_host_clipboard("SECRET-IN-OS-CLIPBOARD".to_string());
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://clipgate.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    for claim in [
        "read-denied:true",
        "read-denied-item:true",
        "write-denied:true",
        "stage1",
    ] {
        assert!(
            got.contains(claim),
            "G_CLIPBOARD_GESTURE: expected `{claim}`\n  got: {got}"
        );
    }
    assert!(
        !got.contains("SECRET"),
        "secret leaked without a gesture: {got}"
    );
    assert!(
        manuk_js::take_clipboard_writes().is_empty(),
        "G_CLIPBOARD_GESTURE: a gesture-less writeText reached the OS clipboard bridge"
    );
    let go = manuk_css::query_selector_all(page.dom(), root, "#go")[0];
    page.dispatch_click(go, &fonts, 800.0);
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let _ = out;
    assert_eq!(
        manuk_js::take_clipboard_writes(),
        vec!["user-copy".to_string()],
        "write inside a click must reach the bridge"
    );
}
