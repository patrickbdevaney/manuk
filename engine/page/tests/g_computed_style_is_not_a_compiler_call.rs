//! **G_COMPUTED_STYLE_IS_NOT_A_COMPILER_CALL — reading computed style must not compile a program.**
//!
//! `getComputedStyle(el)` built its result by **formatting an ~11 KB JavaScript SOURCE STRING and
//! handing it to SpiderMonkey's parser and bytecode compiler — on every single call.** Roughly 8 KB
//! of that was *constant*: the `getPropertyValue` body, the `item`/`getPropertyPriority` bodies, a
//! 50-entry kebab→camel lookup-table literal, and the `__n` names array. All of it re-tokenized,
//! re-parsed and re-compiled per read, for an object whose only per-call content is ~70 short strings.
//!
//! **This is a hang mechanism, not a slow path.** `getComputedStyle` is what `jQuery.css()` calls,
//! which is what every `.width()`, `.height()`, `.offset()` and `.is(':visible')` calls, which is what
//! a jQuery-era layout routine calls once per element per pass. Measured on `ticket.jfa.jp` (a CrUX
//! corpus site sitting in the `timeout-150s` bucket): its `footerPosition()` runs on a 3-second
//! `setTimeout` that reschedules itself, and each round was **cut down by the script-preemption
//! watchdog** mid-`.css()` — so the page never quiesced, never finished, and the site scored
//! **unscorable** rather than badly. 147 s on our own clock, SOLO, with no Chromium in the picture.
//!
//! ```text
//!   Xe → Ge → css → get → css → ce.fn[o] → footerPosition   <- jQuery, in a self-rescheduling timer
//!   @dom_event.js:1:11077   "Script terminated by timeout"  <- column 11077 IS the generated source
//! ```
//!
//! The fix is not a cache and not a budget: the constant machinery moves onto a **prototype installed
//! once per global**, so a call emits only its own data. Nothing about the *values* changes — every
//! other `g_computed_*` gate is the conformance half of this one, and this gate is only about cost.
//!
//! **How to break it:** put the `getPropertyValue`/`item` bodies and the kebab map back into the
//! per-call literal (i.e. revert `CS_PROTO_JS` and inline it into `computed_style_js`). The loop below
//! stops completing inside the budget.

use manuk_text::FontContext;

/// 4,000 reads — a jQuery layout pass over a few hundred elements touching a handful of properties
/// each, which is an ordinary page, not a stress test. Every read goes through a *different* element
/// so no per-element memo can answer it, and the properties read are the ones `.css()` reads.
const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<div id="host"></div>
<script>
  var host = document.getElementById('host');
  for (var i = 0; i < 200; i++) {
    var d = document.createElement('div');
    d.style.width = (100 + i) + 'px';
    host.appendChild(d);
  }
  var kids = host.children, t0 = Date.now(), n = 0;
  for (var pass = 0; pass < 20; pass++) {
    for (var j = 0; j < 200; j++) {
      var cs = getComputedStyle(kids[j]);
      n += cs.width.length + cs.display.length + cs.position.length;
    }
  }
  document.getElementById('out').textContent =
    'done reads=' + (200 * 20) + ' n=' + n + ' ms=' + (Date.now() - t0);
</script></body></html>"##;

#[test]
fn four_thousand_computed_style_reads_complete_inside_the_drain_budget() {
    let fonts = FontContext::new();
    let started = std::time::Instant::now();
    let page = manuk_page::Page::load(HTML, "https://cs-cost.test/", &fonts, 800.0);
    let wall = started.elapsed();
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    // 1. THE CAPABILITY CLAIM: the loop RAN TO COMPLETION. A preempted script leaves `-`, because
    //    the assignment is the last statement — which is exactly what `footerPosition` did on
    //    ticket.jfa.jp, forever, every three seconds.
    assert!(
        got.starts_with("done reads=4000"),
        "G_COMPUTED_STYLE_IS_NOT_A_COMPILER_CALL: 4,000 computed-style reads did not finish.\n  \
         #out = {got:?}\n\n  A `-` here means the watchdog cut the script mid-loop. That is the \
         ticket.jfa.jp mechanism verbatim: jQuery's .css() is getComputedStyle, and a page that \
         cannot finish one layout pass inside a drain never quiesces and is scored UNSCORABLE."
    );

    // 2. THE COST CLAIM, in the units the failure is felt in. The whole page — parse, cascade,
    //    layout, and 4,000 reads — inside a budget that a *compile-per-read* implementation cannot
    //    meet. Deliberately loose (the RED arm overruns it by multiples, not by a margin), because a
    //    tight timing assertion on shared CI is a flake generator, not a gate.
    assert!(
        wall.as_secs_f32() < 8.0,
        "G_COMPUTED_STYLE_IS_NOT_A_COMPILER_CALL: page took {:.2}s for 4,000 reads.\n  \
         #out = {got:?}\n\n  getComputedStyle must not hand the JS engine a fresh ~11 KB program \
         per call; the constant half belongs on a prototype installed once per global.",
        wall.as_secs_f32()
    );
}
