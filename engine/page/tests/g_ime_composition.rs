//! **G_IME_COMPOSITION — CJK/accented text arrives through a composition, not a keystroke.**
//!
//! An input method editor (Pinyin, Kana, hanja, a dead-key accent, mobile autocorrect) does not
//! deliver a `keydown` for the character it commits. The user types phonetic/romanised input into an
//! IME buffer, and when they commit, the browser fires an ordered burst — `compositionstart`,
//! `compositionupdate`, `beforeinput`, `input`, `compositionend` — carrying the composed text. A
//! browser that only ever synthesised `keydown`/`input` for ASCII left every CJK user, and everyone
//! typing an accented letter, unable to enter text into a rich editor. `Page::dispatch_composition`
//! is the headless entry point for that burst; this gate proves the burst is well-formed across the
//! JS boundary.
//!
//! ## How each assertion here can go RED
//!
//! - **The ordered sequence.** RED, run: drop the `compositionend` line from the dispatch script — a
//!   rich editor (which suppresses its per-keystroke autocomplete while `isComposing` is true and
//!   acts on `compositionend`) would believe a composition is open *forever*. The `ORDER=` assertion
//!   fails.
//! - **`isComposing` on the input events.** RED, run: pass `isComposing:false` on the `input` — an
//!   editor's `if (e.isComposing) return;` guard, the idiom for "ignore half-composed text", stops
//!   firing and the editor acts on phonetic garbage. The `ic=true` assertion fails.
//! - **`inputType: insertCompositionText`.** RED, run: omit it — an undo stack / a `beforeinput`
//!   validator can no longer tell a composition commit from a paste. The `it=insertCompositionText`
//!   assertion fails.
//! - **The value is committed through the setter, before `input`.** RED, run: move the
//!   `target.value = …` line after the `input` dispatch — a controlled component reading
//!   `e.target.value` in its `input` handler sees the *stale* value and reverts the commit. The
//!   `tv=你好` assertion fails.
//! - **`beforeinput` is the veto point.** RED, run: ignore the `beforeinput` return value and always
//!   commit — an editor that `preventDefault()`-s the insert (read-only-while-composing, a maxlength
//!   guard) has its veto ignored. The veto field's `VVAL=` (empty) assertion fails.

use manuk_page::Page;
use manuk_text::FontContext;

const W: f32 = 800.0;

fn node(p: &Page, sel: &str) -> manuk_dom::NodeId {
    let root = p.dom().root();
    manuk_css::query_selector_all(p.dom(), root, sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"))
}

fn log(p: &Page) -> String {
    let n = node(p, "#log");
    p.dom().text_content(n)
}

/// **One test, deliberately** — a `PageContext` is per-process here, so a second `Page::load` in the
/// same binary races the first one's runtime and SIGSEGVs (a harness Bar-0 signature, see
/// `g_mouse_actuation.rs`). The normal field and the veto field therefore live in ONE page.
#[test]
fn a_composition_commits_cjk_text_as_an_ordered_burst_with_a_veto_point() {
    let fonts = FontContext::new();
    let mut p = Page::load(
        r#"<!doctype html><body>
<input id="f" value="">
<input id="v" value="">
<div id="log"></div>
<script>
  var types = [], rich = [], vtypes = [];
  function rec(el, arr, richArr) {
    ['compositionstart','compositionupdate','beforeinput','input','compositionend'].forEach(function (t) {
      el.addEventListener(t, function (e) {
        arr.push(t);
        if (richArr) {
          // Read `e.target.value` LIVE so `input`'s snapshot proves the commit landed before it fired.
          richArr.push(t + ':d=' + e.data + ':it=' + (e.inputType || '-') +
                       ':ic=' + e.isComposing + ':tv=' + e.target.value);
        }
      });
    });
  }
  var f = document.getElementById('f'); rec(f, types, rich);
  var v = document.getElementById('v'); rec(v, vtypes, null);
  // A rich editor vetoing the insertion (read-only-while-composing / maxlength guard).
  v.addEventListener('beforeinput', function (e) { e.preventDefault(); });

  window.__report = function () {
    document.getElementById('log').textContent =
      'ORDER=' + types.join(',') +
      ' || ' + rich.join(' || ') +
      ' || VORDER=' + vtypes.join(',') + ' VVAL=' + v.value;
  };
</script></body>"#,
        "https://ime.test/",
        &fonts,
        W,
    );

    let f = node(&p, "#f");
    let v = node(&p, "#v");

    // Commit "你好" into the normal field, and a would-be "안녕" into the veto field.
    let f_proceed = p.dispatch_composition(f, "你好", &fonts, W);
    let v_proceed = p.dispatch_composition(v, "안녕", &fonts, W);

    p.eval_for_test("window.__report();");
    let out = log(&p);

    // ── The ordered burst, in full ───────────────────────────────────────────────────────────
    assert!(
        out.contains("ORDER=compositionstart,compositionupdate,beforeinput,input,compositionend"),
        "an IME commit fires the whole ordered sequence; got: {out}"
    );

    // ── compositionupdate carries the composing text ─────────────────────────────────────────
    assert!(
        out.contains("compositionupdate:d=你好"),
        "compositionupdate carries the composed data; got: {out}"
    );

    // ── beforeinput: the cancelable commit, tagged and composing ─────────────────────────────
    assert!(
        out.contains("beforeinput:d=你好:it=insertCompositionText:ic=true"),
        "beforeinput carries inputType=insertCompositionText, the composed data, isComposing=true; \
         got: {out}"
    );

    // ── input: not cancelable, still composing, and the value is ALREADY committed ───────────
    assert!(
        out.contains("input:d=你好:it=insertCompositionText:ic=true:tv=你好"),
        "the `input` handler, reading e.target.value, sees the committed text — the commit went \
         through the value setter BEFORE input fired (the controlled-component contract); got: {out}"
    );

    // ── compositionend: the composition has ENDED, so isComposing is false ────────────────────
    assert!(
        out.contains("compositionend:d=你好:it=-:ic=false"),
        "compositionend carries the final data and isComposing=false (the composition is over); \
         got: {out}"
    );

    assert!(
        f_proceed,
        "nothing vetoed the normal field's beforeinput, so the commit proceeds"
    );
    assert_eq!(
        p.dom().element(f).and_then(|e| e.attr("value")),
        Some("你好"),
        "the committed text is written into the field's value"
    );

    // ── The veto: beforeinput.preventDefault() means "do not insert" ─────────────────────────
    assert!(
        out.contains("VORDER=compositionstart,compositionupdate,beforeinput,input,compositionend"),
        "the whole sequence still fires even when the insert is vetoed — the composition still \
         starts and ends; got: {out}"
    );
    assert!(
        out.contains("VVAL= ") || out.trim_end().ends_with("VVAL="),
        "the vetoed field's value stays EMPTY — preventDefault() on beforeinput blocks the insert; \
         got: {out}"
    );
    assert!(
        !v_proceed,
        "dispatch_composition returns false when a handler vetoed the beforeinput"
    );
    assert_eq!(
        p.dom().element(v).and_then(|e| e.attr("value")),
        Some(""),
        "the vetoed field's DOM value is unchanged"
    );
}
