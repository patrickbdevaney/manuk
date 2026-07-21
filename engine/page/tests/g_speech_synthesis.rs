//! **G_SPEECH_SYNTHESIS — the Web Speech API exists and reports "cannot speak" honestly.**
//!
//! Screen readers, accessibility "read aloud" buttons, language-learning apps and reader-mode UIs
//! construct `new SpeechSynthesisUtterance(text)` and call `speechSynthesis.speak/getVoices/cancel`,
//! often UNGUARDED — so absent, `SpeechSynthesisUtterance is not defined` (or `undefined.getVoices()`)
//! throws out of the a11y handler.
//!
//! We ship no TTS engine, so — like geolocation — the honest posture is the API present with a truthful
//! "cannot speak" result, never a pretense that it spoke. The gate asserts that contract:
//!
//!   1. `speechSynthesis` + `SpeechSynthesisUtterance` exist; `speak`/`getVoices`/`cancel` callable;
//!      constructing an utterance and calling the API does not throw.
//!   2. `getVoices()` returns an empty array (true — no voices installed).
//!   3. `speak(u)` fires the utterance's `error` event (`error: 'synthesis-unavailable'`) and NOT
//!      `end` — a fired `end` would claim it spoke when the user heard nothing.
//!
//! RED: removing the shim drops `defined` and `erroredhonest` — `SpeechSynthesisUtterance` is not
//! defined and the construction throws, the exact dead-a11y-handler failure.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html>
<html><body>
  <div id="out">-</div>
  <script>
    var R = {
      a: [],
      push: function (s) { this.a.push(s); var o = document.getElementById('out');
                           if (o) { o.textContent = this.a.join(' '); } }
    };
    try {
      R.push('defined:' + (typeof SpeechSynthesisUtterance === 'function' &&
                           window.speechSynthesis && typeof speechSynthesis.speak === 'function' &&
                           typeof speechSynthesis.getVoices === 'function' &&
                           typeof speechSynthesis.cancel === 'function'));

      R.push('novoices:' + (Array.isArray(speechSynthesis.getVoices()) &&
                            speechSynthesis.getVoices().length === 0));

      var u = new SpeechSynthesisUtterance('hello');
      var ended = false, erred = '';
      u.onend = function () { ended = true; };
      u.onerror = function (e) { erred = (e && e.error) || 'yes'; };
      speechSynthesis.speak(u);
      // Record after a microtask so the async error has settled.
      Promise.resolve().then(function () {
        Promise.resolve().then(function () {
          R.push('erroredhonest:' + (erred === 'synthesis-unavailable' && ended === false));
        });
      });

      R.push('ready:true');
    } catch (e) {
      R.push('THREW:' + e);
    }
  </script>
</body></html>"##;

#[test]
fn speech_synthesis_present_and_honestly_unavailable() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://tts.test/", &fonts, 800.0);
    let root = page.dom().root();

    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("defined:true", "`speechSynthesis` + `SpeechSynthesisUtterance` must exist with speak/getVoices/cancel callable — a11y/reader code uses them unguarded, so absence throws `SpeechSynthesisUtterance is not defined`"),
        ("novoices:true", "`getVoices()` must return an empty array — true, since no TTS voices are installed"),
        ("erroredhonest:true", "`speak()` must fire the utterance's `error` ('synthesis-unavailable') and NOT `end` — firing `end` would claim it spoke when the user heard nothing"),
        ("ready:true", "the a11y setup must complete without throwing"),
    ] {
        assert!(
            got.contains(claim),
            "G_SPEECH_SYNTHESIS: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
