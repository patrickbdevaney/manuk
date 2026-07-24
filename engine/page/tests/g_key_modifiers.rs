//! **G_KEY_MODIFIERS — a dispatched `KeyboardEvent` carries `ctrlKey`/`shiftKey`/`altKey`/`metaKey`,
//! and a shortcut chord does NOT type a stray character into a contenteditable.**
//!
//! Before this, `Page::dispatch_key` built a KeyboardEvent with `key`/`keyCode`/`which` but NO modifier
//! flags — so `e.ctrlKey`/`e.metaKey`/`e.shiftKey`/`e.altKey` all read `undefined` (falsy). Every page
//! keyboard-shortcut handler was dead: the Cmd/Ctrl+K command palette (Slack, Notion, Linear, GitHub),
//! a composer that inserts a newline only on `Shift+Enter`. And a modifier chord aimed at an editable
//! (`Ctrl+B`, `Cmd+K`) inserted its letter as text, because the default editing action ignored modifiers.
//!
//! `Page::dispatch_key_mods(node, ty, key, KeyModifiers{..}, ..)` now threads the four flags into the
//! event AND suppresses the printable-key default action when `ctrl`/`meta` is held (browsers treat those
//! as shortcuts, not text). `Shift`/`Alt` alone still produce text.
//!
//! ## Each claim, and how it goes RED
//!
//!   * `seen=ctrl:true|shift:true|alt:false|meta:false` — a handler reading the four flags off the event
//!     sees exactly what was dispatched. RED: drop the modifier props from the event literal → all four
//!     read `undefined` and the join shows `ctrl:undefined|...`.
//!   * `metaK=true` — a Cmd+K handler (`if (e.metaKey && e.key==='k') e.preventDefault()`) fires and
//!     `preventDefault` propagates back as "do not perform default" (dispatch_key_mods returns false).
//!   * `chordText=Hi` — typing `Ctrl+b` into a `<div contenteditable>` inserts NOTHING (the editable keeps
//!     its "Hi"): a chord is a shortcut, not text. RED: without the chord guard, `b` is inserted → "Hib".
//!   * `plainText=Hix` — a PLAIN `x` (no modifiers) still inserts, proving the guard is chord-specific and
//!     didn't break ordinary typing.

use manuk_page::{KeyModifiers, Page};
use manuk_text::FontContext;

const W: f32 = 800.0;

fn node(p: &Page, sel: &str) -> manuk_dom::NodeId {
    let root = p.dom().root();
    manuk_css::query_selector_all(p.dom(), root, sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"))
}

#[test]
fn keyboard_event_carries_modifier_flags_and_a_chord_does_not_type() {
    let fonts = FontContext::new();
    let mut p = Page::load(
        r#"<!doctype html><body>
<div id="ed" contenteditable="true">Hi</div>
<div id="log"></div>
<script>
  var out = [];
  var ed = document.getElementById('ed');
  // A handler that records the four modifier flags off the KeyboardEvent it receives.
  document.addEventListener('keydown', function (e) {
    if (e.key === 'K' || e.key === 'k') {
      out.push('seen=ctrl:' + e.ctrlKey + '|shift:' + e.shiftKey + '|alt:' + e.altKey + '|meta:' + e.metaKey);
      // A Cmd/Ctrl+K command palette: claim the key so the browser default does not run.
      if (e.metaKey) { out.push('metaK=true'); e.preventDefault(); }
    }
  });
  window.__flush = function () { document.getElementById('log').textContent = out.join(' '); };
</script></body>"#,
        "https://keymods.test/",
        &fonts,
        W,
    );
    let ed = node(&p, "#ed");

    // 1. Ctrl+Shift+K — the handler must see ctrl:true shift:true alt:false meta:false.
    p.dispatch_key_mods(
        ed,
        "keydown",
        "K",
        KeyModifiers {
            ctrl: true,
            shift: true,
            alt: false,
            meta: false,
        },
        &fonts,
        W,
    );

    // 2. Cmd+K — the meta-K handler preventDefaults, so dispatch reports "do not perform default".
    let meta_k_default = p.dispatch_key_mods(
        ed,
        "keydown",
        "k",
        KeyModifiers {
            ctrl: false,
            shift: false,
            alt: false,
            meta: true,
        },
        &fonts,
        W,
    );
    assert!(
        !meta_k_default,
        "G_KEY_MODIFIERS: a Cmd+K handler that calls preventDefault() must make dispatch_key_mods return \
         false (the browser default must not run) — got true (the modifier flag never reached the handler)"
    );

    // 3. Ctrl+b typed into the editable: a chord is a SHORTCUT, not text — nothing is inserted.
    p.dispatch_key_mods(
        ed,
        "keydown",
        "b",
        KeyModifiers {
            ctrl: true,
            shift: false,
            alt: false,
            meta: false,
        },
        &fonts,
        W,
    );
    let after_chord = p.dom().text_content(ed);

    // 4. A plain `x` (no modifiers) DOES insert — the guard is chord-specific, ordinary typing unbroken.
    p.dispatch_key(ed, "keydown", "x", &fonts, W);
    let after_plain = p.dom().text_content(ed);

    p.dispatch_key(node(&p, "#ed"), "keyup", "x", &fonts, W); // settle
                                                              // Flush the recorded flags into #log.
    p.eval_for_test("window.__flush && window.__flush()");
    let out = p.dom().text_content(node(&p, "#log"));
    println!("KEY-MODIFIERS RESULT: {out}");
    println!("  after_chord={after_chord:?} after_plain={after_plain:?}");

    for claim in [
        "seen=ctrl:true|shift:true|alt:false|meta:false",
        "metaK=true",
    ] {
        assert!(
            out.contains(claim),
            "G_KEY_MODIFIERS: expected `{claim}` in {out:?}\n  \
             the dispatched KeyboardEvent must expose ctrlKey/shiftKey/altKey/metaKey to page handlers."
        );
    }
    assert_eq!(
        after_chord, "Hi",
        "G_KEY_MODIFIERS: Ctrl+b typed into a contenteditable must insert NOTHING (a chord is a shortcut, \
         not text) — the editable should still read \"Hi\""
    );
    assert_eq!(
        after_plain, "Hix",
        "G_KEY_MODIFIERS: a plain `x` (no modifiers) must still insert — the chord guard must not break \
         ordinary typing"
    );
}
