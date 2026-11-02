//! **G_SW_SCOPE — SEC.DID.12.c (SECURITY.md P17): a page can register a service worker only from its own
//! origin, with a scope inside the script's directory; WebRTC is absent.**
//!
//! A registered worker intercepts every `fetch` the page makes, so a foreign-origin script (or a scope
//! outside the script's directory) would hand an attacker the page's network. Controls (same-origin script,
//! default and in-directory scope, blob: script) stop a constant-reject passing; refusals stop a no-check
//! passing. WebRTC (`RTCPeerConnection`, which would open raw UDP past the egress guard) must stay absent.
//!
//! Mutation record (chrome_ver=145.0.7632.116): deleting the origin/scope check in `register` in
//! `engine/js/src/event_loop.rs` turns `xorigin` and `scope` RED; making it always reject turns `same`,
//! `inscope` and `blob` RED.
use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html>
<html><body><div id="out">-</div>
<script>
  var a = [];
  function P(s) { a.push(s); document.getElementById('out').textContent = a.join(' '); }
  function tryReg(tag, url, opts) {
    try {
      return navigator.serviceWorker.register(url, opts).then(
        function () { P(tag + ':ok'); }, function (e) { P(tag + ':rej'); });
    } catch (e) { P(tag + ':throw'); return Promise.resolve(); }
  }
  P('rtc:' + (typeof RTCPeerConnection === 'undefined' && typeof webkitRTCPeerConnection === 'undefined' ? 'absent' : 'present'));
  P('ipc:' + ((typeof chrome !== 'undefined' && chrome && chrome.runtime) || (typeof browser !== 'undefined' && browser && browser.runtime) || (typeof ipc !== 'undefined') || (typeof webkit !== 'undefined' && webkit && webkit.messageHandlers) || typeof external !== 'undefined' && external && external.invoke ? 'present' : 'absent'));
  var blob = URL.createObjectURL(new Blob(["self.addEventListener('install',function(){});"]));
  tryReg('xorigin', 'https://evil.test/sw.js')
    .then(function () { return tryReg('scope', 'https://pwa.test/app/sw.js', {scope: '/other/'}); })
    .then(function () { return tryReg('xscope', blob, {scope: 'https://evil.test/'}); })
    .then(function () { return tryReg('blob', blob); });
</script></body></html>"##;

#[test]
fn service_worker_registration_is_same_origin_and_webrtc_absent() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://pwa.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    for (claim, why) in [
        ("rtc:absent", "RTCPeerConnection would open raw UDP outside the egress guard"),
        ("ipc:absent", "no extension/IPC bridge (chrome.runtime, browser.runtime, window.ipc, webkit.messageHandlers) may be reachable from page script"),
        ("xorigin:rej", "a cross-origin worker script must be refused"),
        ("scope:rej", "a scope outside the script's directory must be refused"),
        ("xscope:rej", "a cross-origin scope must be refused"),
        ("blob:ok", "a page-minted blob: worker must still register (control)"),
    ] {
        assert!(got.contains(claim), "G_SW_SCOPE: expected `{claim}`\n  got: {got}\n  {why}.");
    }
}
