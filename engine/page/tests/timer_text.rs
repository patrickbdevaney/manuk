//! W4.A1.a gate: text a timer callback writes with `textContent` must be in the page's visible text.
//! Replacing a text node keeps the DOM node COUNT the same while the new node has no computed style,
//! so a count-based "did the tree grow?" check skipped the restyle and the text laid out as nothing.

use manuk_text::FontContext;

const TICKER: &str = r##"<!doctype html><html><body><p id="o">init</p>
<script>
  var n = 0;
  function render() { document.getElementById('o').textContent = 'tick' + (n++); }
  setInterval(render, 40);
</script></body></html>"##;

#[tokio::test(flavor = "current_thread")]
async fn timer_written_text_survives_settle() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(TICKER, "https://timer.test/", &fonts, 800.0);
    page.settle_async(&fonts, 800.0, std::time::Duration::from_secs(2))
        .await;
    let text = page.visible_text();
    assert!(
        text.contains("tick"),
        "text written by a timer's textContent vanished from visible_text: {text:?}"
    );
}
