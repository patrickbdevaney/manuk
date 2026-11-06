//! W3.17 gate: the OS clipboard is written only inside a HUMAN gesture's dispatch.
//! `setTimeout(writeText('rm -rf ~'), 500)` armed by a human click leaves the clipboard alone, and so
//! does the same click made by the agent (no `HumanActivation`). One test: SpiderMonkey allows one
//! runtime per process.

use manuk_consent::{admit_clipboard, test_mint};
use manuk_page::Page;
use manuk_text::FontContext;

const HTML: &str = r#"<button id=b style="width:200px;height:40px"
    onclick="navigator.clipboard.writeText('sync-ok'); setTimeout(function(){navigator.clipboard.writeText('rm -rf ~')},500)">Copy</button>"#;

#[test]
fn only_the_synchronous_write_inside_a_human_click_gets_through() {
    let fonts = FontContext::new();
    let mut page = Page::load(HTML, "https://clip.example/", &fonts, 1024.0);
    manuk_js::take_clipboard_writes(); // nothing queued before the gesture counts
    let human = test_mint::activation();

    page.dispatch_click_at(20.0, 20.0, &fonts, 1024.0);
    let got = admit_clipboard(manuk_js::take_clipboard_writes(), Some(&human));
    assert_eq!(got, vec!["sync-ok".to_string()], "the human click's own write");

    // The armed timer fires later, with no gesture in scope.
    std::thread::sleep(std::time::Duration::from_millis(700));
    page.run_deferred_scripts(&fonts, 1024.0);
    page.eval_for_test("0");
    let late = admit_clipboard(manuk_js::take_clipboard_writes(), None);
    assert!(late.is_empty(), "a timer write must not reach the OS clipboard: {late:?}");

    // The same click made by the agent: the page DID queue a write, so only the gate stops it.
    // With an activation the same write would pass, which is what lets this assertion go red.
    page.dispatch_click_at(20.0, 20.0, &fonts, 1024.0);
    let raw = manuk_js::take_clipboard_writes();
    assert!(!raw.is_empty(), "the page did queue a write; only the gate stops it");
    assert!(admit_clipboard(raw.clone(), None).is_empty());
    assert_eq!(admit_clipboard(raw, Some(&human)), vec!["sync-ok".to_string()]);
}
