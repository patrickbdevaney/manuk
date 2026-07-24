//! **G_DOCUMENT_HAS_FOCUS — `document.hasFocus()` answers, and agrees with page visibility.**
//!
//! `document.hasFocus()` — "is the user looking at this document right now?" — is called by idle-detection,
//! analytics heartbeats, "pause the video/carousel when the tab is backgrounded" logic and presence
//! indicators. It was **absent**, and an absent method is not a graceful "unknown": `document.hasFocus()`
//! is a synchronous `TypeError` that takes the whole handler down (the same failure class as a missing
//! `document.hidden`).
//!
//! We do not model system window focus separately from tab foregrounding, so `hasFocus()` is tied to the
//! signal the shell already owns — the tab-in-front state behind `visibilityState`/`document.hidden`. That
//! keeps the two from ever contradicting each other. Two things to prove:
//! 1. it exists and returns a boolean `true` for the foreground (visible) document;
//! 2. it TRACKS visibility — when the shell backgrounds the tab (`visibilityState` → `hidden`),
//!    `hasFocus()` becomes `false`, and it never disagrees with `document.hidden`.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<div id="out">-</div>
<script>
  var R = [];
  R.push('type:' + (typeof document.hasFocus));
  R.push('visible:' + document.hasFocus());
  R.push('agreesVisible:' + (document.hasFocus() === !document.hidden));
  // Drive the shell's visibility flip the same way the host does.
  var g = (typeof globalThis !== 'undefined') ? globalThis : window;
  g.__setVisibility('hidden');
  R.push('hiddenFocus:' + document.hasFocus());
  R.push('agreesHidden:' + (document.hasFocus() === !document.hidden));
  g.__setVisibility('visible');
  R.push('restored:' + document.hasFocus());
  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn a_has_focus_answers_and_tracks_visibility() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://focus.test/", &fonts, 400.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("type:function", "hasFocus must be a real method — absent it was a TypeError that killed the handler"),
        ("visible:true", "the foreground (visible) document has focus"),
        (
            "agreesVisible:true",
            "hasFocus() must equal `!document.hidden` — it is tied to the same tab-in-front fact, so the \
             two can never contradict",
        ),
        (
            "hiddenFocus:false",
            "when the shell backgrounds the tab (visibilityState → hidden), the document no longer has \
             focus — this is the whole point for idle/pause logic",
        ),
        ("agreesHidden:true", "…and it still agrees with `document.hidden` in the hidden state"),
        ("restored:true", "raising the tab again restores focus — the signal tracks, it is not one-way"),
    ] {
        assert!(
            got.contains(claim),
            "G_DOCUMENT_HAS_FOCUS: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
