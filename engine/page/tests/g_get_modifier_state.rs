//! **G_GET_MODIFIER_STATE — `event.getModifierState(key)` reads the modifier flags.**
//!
//! `getModifierState('Control')` is the spec way to ask whether a modifier was held during a mouse or
//! keyboard event — and it is what keyboard-shortcut libraries (Mousetrap, CodeMirror/ProseMirror
//! keymaps, every "Cmd+K palette") and rich-text editors call, rather than reading `e.ctrlKey` directly,
//! because it also covers `AltGraph`/`CapsLock`/`NumLock`. It was absent, so `e.getModifierState is not a
//! function` threw out of the keydown handler and the shortcut died with it.
//!
//! The claims check the returned booleans, each a way the missing method goes RED:
//!
//!   * A `KeyboardEvent` with `ctrlKey` reports `Control` true, other modifiers false.
//!   * Multiple modifiers each report independently.
//!   * A `MouseEvent` carries the method too.
//!   * An unknown key name returns false (not a throw).
//!   * A plain `Event` does NOT get the method (it is only on modifier-bearing events).

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body><div id="out">-</div><script>
    var r = [];
    var k1 = new KeyboardEvent('keydown', { ctrlKey: true });
    r.push('ctrl:' + k1.getModifierState('Control') + '/' + k1.getModifierState('Shift'));
    var k2 = new KeyboardEvent('keydown', { shiftKey: true, metaKey: true });
    r.push('multi:' + k2.getModifierState('Shift') + '/' + k2.getModifierState('Meta') + '/' + k2.getModifierState('Alt'));
    var m = new MouseEvent('click', { altKey: true });
    r.push('mouse:' + m.getModifierState('Alt'));
    r.push('unknown:' + new KeyboardEvent('keydown', {}).getModifierState('Fn'));
    r.push('plain:' + (typeof new Event('x').getModifierState));
    document.getElementById('out').textContent = r.join(' ');
  </script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn get_modifier_state_reads_modifier_flags() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://get-modifier-state.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "ctrl:true/false",       // Control held, Shift not
        "multi:true/true/false", // Shift + Meta held, Alt not
        "mouse:true",            // a MouseEvent carries getModifierState too
        "unknown:false",         // an unknown modifier name returns false, not a throw
        "plain:undefined", // a plain Event does not get the method (guarded to modifier events)
    ] {
        assert!(
            got.contains(claim),
            "G_GET_MODIFIER_STATE: expected {claim} in {got:?}\n  \
             event.getModifierState(key) must read the modifier flags — its absence throws \
             `not a function` out of a keydown handler and kills the keyboard shortcut."
        );
    }
}
