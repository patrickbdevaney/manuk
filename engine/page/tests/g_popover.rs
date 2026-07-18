//! G_POPOVER — the `popover` attribute API: menus, tooltips, dropdowns and toasts.
//!
//! The other half of the top layer (see `g_dialog`). Every dropdown that has stopped being a
//! hand-rolled `<div class="dropdown">` + an outside-click listener is a `popover`, and the whole
//! surface was absent — `showPopover()` was a TypeError, and with no `[popover]` UA rule the menu's
//! items rendered inline in the middle of the page before anyone opened it.
//!
//! Claims: the property reflects; `showPopover`/`hidePopover`/`togglePopover(force)` work;
//! `beforetoggle` is cancelable and `toggle` reports old/new state; `<button popovertarget>` works
//! with NO script; two `auto` popovers are mutually exclusive but a `manual` one is not; a click
//! outside light-dismisses an `auto` popover and Escape does too.
//!
//! The rendering half is `g_popover_render.rs` (separate binary — two SpiderMonkey contexts tear
//! down messily, see `g_globals`).

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html>
<html><body>
  <div id="menu" popover><p>MENUITEM</p></div>
  <div id="other" popover>other</div>
  <div id="man" popover="manual">manual</div>
  <button id="invoker" popovertarget="menu">Open</button>
  <button id="shower" popovertarget="other" popovertargetaction="show">Show</button>
  <div id="outside">outside</div>
  <div id="out"></div>
  <script>
    var $ = function(id) { return document.getElementById(id); };
    var r = [];
    var m = $('menu');

    // Feature detection is exactly this read.
    r.push('detect:' + ('popover' in HTMLElement.prototype));
    r.push('reflect:' + (m.popover === 'auto' && $('man').popover === 'manual' && $('out').popover === null));

    // show / hide, and the toggle events with their old/new state.
    var seen = [];
    m.addEventListener('beforetoggle', function(e) { seen.push('b:' + e.oldState + '>' + e.newState); });
    m.addEventListener('toggle', function(e) { seen.push('t:' + e.oldState + '>' + e.newState); });
    m.showPopover();
    r.push('shown:' + m.hasAttribute('data-manuk-popover-open'));
    m.hidePopover();
    r.push('hidden:' + (m.hasAttribute('data-manuk-popover-open') === false));
    r.push('events:' + (seen.join(',') === 'b:closed>open,t:closed>open,b:open>closed,t:open>closed'));

    // beforetoggle is cancelable on the way open — a guard vetoes it.
    var veto = function(e) { e.preventDefault(); };
    m.addEventListener('beforetoggle', veto);
    m.showPopover();
    r.push('veto:' + (m.hasAttribute('data-manuk-popover-open') === false));
    m.removeEventListener('beforetoggle', veto);

    // togglePopover(), with and without the force argument.
    r.push('toggleon:' + (m.togglePopover() === true));
    r.push('forceon:' + (m.togglePopover(true) === true));
    r.push('forceoff:' + (m.togglePopover(false) === false));

    // Declarative invocation: NO script — a button with popovertarget.
    $('invoker').click();
    r.push('invoke:' + m.hasAttribute('data-manuk-popover-open'));

    // `auto` popovers are mutually exclusive; a `manual` one is not affected.
    $('man').showPopover();
    $('shower').click();   // popovertargetaction=show on #other
    r.push('exclusive:' + ($('other').hasAttribute('data-manuk-popover-open') === true
                        && m.hasAttribute('data-manuk-popover-open') === false));
    r.push('manualkept:' + $('man').hasAttribute('data-manuk-popover-open'));

    // Light dismiss: a click outside closes the auto popover, and leaves the manual one alone.
    $('outside').click();
    r.push('lightdismiss:' + ($('other').hasAttribute('data-manuk-popover-open') === false
                           && $('man').hasAttribute('data-manuk-popover-open') === true));

    // Escape light-dismisses too.
    m.showPopover();
    var ev = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true });
    ev.key = 'Escape';
    document.dispatchEvent(ev);
    r.push('escape:' + (m.hasAttribute('data-manuk-popover-open') === false));

    $('out').textContent = r.join(' ');
  </script>
</body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn popover_shows_hides_invokes_and_light_dismisses() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://popover.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "detect:true",       // 'popover' in HTMLElement.prototype — the feature-detection read
        "reflect:true",      // el.popover reflects auto/manual/null
        "shown:true",        // showPopover() opens
        "hidden:true",       // hidePopover() closes
        "events:true",       // beforetoggle+toggle, in order, with oldState/newState
        "veto:true",         // beforetoggle is cancelable
        "toggleon:true",     // togglePopover() flips and returns the new state
        "forceon:true",      // togglePopover(true) forces open
        "forceoff:true",     // togglePopover(false) forces closed
        "invoke:true",       // <button popovertarget> — declarative, no script
        "exclusive:true",    // opening an auto popover closes the other auto one
        "manualkept:true",   // ...but not a manual one
        "lightdismiss:true", // an outside click closes auto, leaves manual
        "escape:true",       // Escape light-dismisses auto
    ] {
        assert!(
            got.contains(claim),
            "G_POPOVER: expected {claim} in {got:?}\n  \
             The popover API is how the modern web ships every menu, tooltip and dropdown. A missing \
             `showPopover` is a TypeError that takes the click handler with it."
        );
    }
}
