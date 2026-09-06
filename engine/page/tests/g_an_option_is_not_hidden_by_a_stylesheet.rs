//! **G_AN_OPTION_IS_NOT_HIDDEN_BY_A_STYLESHEET — the Stylo UA sheet said `option { display: none }`,
//! so a `<select>` exposed NO options at all: every dropdown on the web, missing from an agent's
//! perception.**
//!
//! ⭐⭐⭐ **THE COMMENT DIRECTLY ABOVE THE OFFENDING LINE ALREADY RECORDED THE FIX.** The
//! `<source>`/`<picture>` note in the same sheet, one paragraph earlier, says it exactly: *"Hiding
//! them here produced the right box and the wrong answer, and `getComputedStyle(source).display` is
//! exactly what a responsive-image shim reads."* An `<option>` is the same shape — it is not hidden
//! by a stylesheet; a `<select>` simply **draws its own text instead of its children**
//! (`control_text`), which is a structural fact about the widget. The same bug survived one line
//! below its own lesson.
//!
//! ⚠⚠ **AND THE TWO CASCADES DISAGREED, WHICH IS THE FAILURE `apply_ua_defaults`' LOCKSTEP NOTE
//! EXISTS TO WARN ABOUT** — *"The two cascades disagreeing about which elements render at all is how
//! a `<source>` ends up with 19px of height in one configuration and none in the other."*
//! `MinimalCascade` never listed `option`; only this sheet did. So the accessibility tree contained
//! every dropdown's options under one build and none under the other, and which one you got depended
//! on a cargo feature.
//!
//! Chrome-measured (`google-chrome --headless --dump-dom`):
//!
//! ```text
//!                                             Chrome    before    after
//!   getComputedStyle(option).display           block      none     block
//!   the <select>'s height                       19px      19px      19px  ✓
//!   gap between the paragraphs around it        54px      54px      54px  ✓
//!   options in the accessibility tree              3         0         3
//! ```
//!
//! ⭐⭐ **THE TWO GEOMETRY ROWS ARE THE WHOLE RISK, AND THEY DO NOT MOVE.** The rule was added for a
//! real reason the sheet still records — *"left as plain `inline`, the inline collector recurses into
//! a `<select>`'s `<option>`s and paints every one of them into the surrounding line — rust-lang.org's
//! language picker rendered as a row of twelve language names"*. That is why this gate measures the
//! select's height and the flow around it rather than only the computed value: if the options ever
//! start generating boxes, the gap moves from 54 to something much larger and these rows say so
//! before a corpus sweep would.
//!
//! Mutations that must turn this red:
//!   1. `option, optgroup { display: none }`   → the tree has 0 options; `display` reads `none`
//!   2. delete the rule entirely               → `display` reads `inline`, not Chrome's `block`
//!   3. `display: inline` instead of `block`   → same as 2, and the a11y rows still pass
//!
//! ⚠ This gate needs `--features stylo`: the rule it is about exists only in that sheet, and under
//! `MinimalCascade` every row here already passed. That asymmetry IS the defect.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8><style>
body { margin: 0; font: 16px/1.4 monospace }
</style></head><body>
<p id="before">BEFORE</p>
<select id="s"><option>English</option><option selected>Français</option><option>Deutsch</option></select>
<p id="after">AFTER</p>
<div id="out">-</div>
<script>
var o = document.querySelector('option'), s = document.getElementById('s');
var a = document.getElementById('after').getBoundingClientRect();
var b = document.getElementById('before').getBoundingClientRect();
document.getElementById('out').textContent =
  'display=' + getComputedStyle(o).display +
  ' selh=' + Math.round(s.getBoundingClientRect().height) +
  ' gap=' + Math.round(a.top - b.bottom);
</script></body></html>"##;

fn options(n: &manuk_a11y::A11yNode, out: &mut Vec<String>) {
    if n.role == manuk_a11y::Role::Option {
        out.push(n.name.clone());
    }
    for c in &n.children {
        options(c, out);
    }
}

#[test]
fn an_option_is_structurally_unrendered_not_display_none() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://option.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("OPTION: {got}");

    // ── VACUITY. The script must have run, or `display=` below is a placeholder and every
    //    assertion is vacuous.
    assert!(
        got.starts_with("display="),
        "VACUOUS: the probe script did not run — got {got:?}"
    );

    // Chrome headless: the computed value AND the geometry that the old rule was protecting.
    assert_eq!(
        got, "display=block selh=19 gap=54",
        "\n  an <option>'s computed display is Chrome's `block`, and NOTHING about the flow moves\n\
           got: {got}"
    );

    // ── AND THE POINT OF THE WHOLE CHANGE: an agent can see the dropdown's contents.
    let mut names = Vec::new();
    options(&page.a11y_tree(), &mut names);
    assert_eq!(
        names,
        vec!["English", "Français", "Deutsch"],
        "the accessibility tree must expose every option of a collapsed <select> — Chrome does, \
         and an agent choosing from a dropdown has nothing else to read"
    );
}
