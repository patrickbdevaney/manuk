//! **G_TOGGLE_EVENT_SOURCE — a popover ToggleEvent names the invoker that caused it.**
//!
//! `ToggleEvent.source` (Baseline 2024) is the element that triggered a popover toggle — the
//! `<button popovertarget>` for a declarative open, the `{source}` option for `showPopover()`, and
//! `null` for a bare imperative call. A menu/tooltip framework reads it to position and focus the
//! popover relative to the button that opened it; without it, a page that opens one menu from three
//! different buttons cannot tell which one the user clicked, and anchors the menu to the wrong place.
//!
//! Asserted: the imperative `showPopover({source})` carries the invoker to the `toggle` event; a
//! bare `hidePopover()` carries `null`; and the declarative `<button popovertarget>` click carries
//! the button itself.
//!
//! **RED, run:** drop `ev.source = source || null` from `__popToggleEvent` (source is always
//! undefined → `imp:true`/`decl:true` flip to false), or stop threading `{source: t}` from
//! `__popClick` (only `decl:true` flips).

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html>
<html><body>
  <button id="btn" popovertarget="pop">open</button>
  <div id="pop" popover>menu</div>
  <div id="out">-</div>
  <script>
    var R = {
      a: [],
      push: function (s) { this.a.push(s); var o = document.getElementById('out');
                           if (o) { o.textContent = this.a.join(' '); } },
    };
    try {
      var btn = document.getElementById('btn');
      var pop = document.getElementById('pop');
      var lastSource = 'unset';
      pop.addEventListener('toggle', function (ev) { lastSource = ev.source; });

      // Imperative, with an explicit source.
      pop.showPopover({ source: btn });
      R.push('imp:' + (lastSource === btn));
      R.push('open1:' + pop.hasAttribute('data-manuk-popover-open'));

      // Bare imperative close: no invoker.
      pop.hidePopover();
      R.push('bare:' + (lastSource === null));

      // Declarative: the popovertarget button click carries itself as the source.
      lastSource = 'unset';
      btn.click();
      R.push('decl:' + (lastSource === btn));
      R.push('open2:' + pop.hasAttribute('data-manuk-popover-open'));

      R.push('done:true');
    } catch (e) { R.push('threw:' + (e && e.name ? e.name : e)); }
  </script>
</body></html>"##;

#[test]
fn a_popover_toggle_event_names_its_invoker() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ui.test/", &fonts, 800.0);

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("TOGGLE EVENT SOURCE PROBE: {got}");

    for (claim, why) in CLAIMS {
        assert!(
            got.contains(claim),
            "G_TOGGLE_EVENT_SOURCE: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}

const CLAIMS: &[(&str, &str)] = &[
    (
        "imp:true",
        "showPopover({source: btn}) must carry the invoker to the toggle event — this is the whole \
         point of the property: to tell a framework which control opened the popover",
    ),
    (
        "open1:true",
        "the imperative showPopover actually opened the popover (or the toggle event was vacuous)",
    ),
    (
        "bare:true",
        "a bare hidePopover() has no invoker, so ToggleEvent.source is null — not a stale reference \
         to the last one",
    ),
    (
        "decl:true",
        "a <button popovertarget> click carries the BUTTON as the source — the declarative path is \
         how the API is used without any script, and the invoker must survive the click routing",
    ),
    (
        "open2:true",
        "the declarative click toggled the (closed) popover back open",
    ),
    (
        "done:true",
        "the whole sequence ran; a throw drops this token",
    ),
];
