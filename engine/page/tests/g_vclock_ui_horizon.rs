//! W6.R1.a.a.a — the post-load virtual-clock horizon must not fire a 10 s UI auto-dismiss timer
//! during a page load, while a testharness page still reaches its 10 s harness timeout.
//!
//! udriver (evals-udriver.vercel.app) mounts an MUI `<Alert>` and dismisses it with
//! `setTimeout(..., 1e4)`. With a 60 s post-load horizon the virtual clock fired that timer before the
//! agent could take its first snapshot, so the banner Chrome shows (sampled ~4 s after navigate) was
//! never in manuk's tree. An ordinary page now gets 6 s (the Chrome oracle's own
//! `--virtual-time-budget`); a page that loaded testharness.js keeps 60 s.

use manuk_text::FontContext;

fn text_of(html: &str) -> String {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(html, "https://vclock.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    page.dom().text_content(out).trim().to_string()
}

#[test]
fn a_ten_second_ui_timer_does_not_fire_during_load_but_a_testharness_timeout_still_does() {
    let got = text_of(
        r#"<!doctype html><html><body><div id="out">shown</div><script>
        setTimeout(function () { document.getElementById('out').textContent = 'early'; }, 3000);
        setTimeout(function () { document.getElementById('out').textContent = 'dismissed'; }, 10000);
        </script></body></html>"#,
    );
    assert_eq!(
        got, "early",
        "a 3 s timer must run during load and a 10 s auto-dismiss must NOT (got {got:?}): the \
         post-load horizon outran a real-time observer, so an agent never sees udriver's MuiAlert"
    );

    // One #[test] per file: two SpiderMonkey runtimes on parallel test threads segfault.
    let got = text_of(
        r#"<!doctype html><html><body><div id="out">-</div><script>
        window.add_completion_callback = function () {};
        setTimeout(function () { document.getElementById('out').textContent = 'harness-timeout'; }, 10000);
        </script></body></html>"#,
    );
    assert_eq!(
        got, "harness-timeout",
        "a testharness page must keep the 60 s horizon or every async WPT file reports TIMEOUT"
    );
}
