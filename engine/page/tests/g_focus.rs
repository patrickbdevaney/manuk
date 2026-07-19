//! **G_FOCUS — focus reaches the CASCADE, and `:focus` / `:focus-within` / `:focus-visible` are
//! three different questions.**
//!
//! This was a **dead-end wire**, the same shape as the parser's quirks verdict at tick 242 and the
//! `RuleIndex` at tick 243. The shell has tracked focus for many ticks and publishes it into the JS
//! world through `publish_view_state` — that is what backs `document.activeElement` — and it never
//! reached the style system. `:focus` answered a hard-coded `false` for the life of every page. The
//! engine had the answer and threw it away, and **no capability probe can see that**: the feature
//! appears present at every layer anyone would inspect.
//!
//! ## What it costs is accessibility, not decoration
//!
//! The focus ring is the only thing telling a keyboard user where they are. And because authors
//! spent twenty years writing `:focus { outline: none }` to remove the ring *mouse* users did not
//! want, on a great many sites the only remaining cue is the author's own `:focus`/`:focus-visible`
//! rule. With the pseudo-class never matching, tabbing through those pages moves an invisible
//! cursor — the page renders exactly what it was told to, and nothing reports a problem.
//!
//! ## The three are not one feature with three names
//!
//! * `:focus` — the exact element. It does **not** match ancestors; treating it as if it did puts a
//!   ring around the whole form every time one field is focused.
//! * `:focus-within` — the element **or any ancestor**. This is the expanding search box and the
//!   open combobox panel: the `<input>` takes focus, the `<div>` is what changes size.
//! * `:focus-visible` — focused **and** the ring is warranted. Clicking a button focuses it, and a
//!   ring there is precisely the noise this pseudo-class exists to remove. Only the caller knows
//!   how focus arrived, so `Page::set_focus` takes `from_keyboard`.
//!
//! ## The RED probes (run, not imagined)
//!
//! * `P::Focus => false` restored → `focus:` fails; the wire is dead again.
//! * `is_focused` walking ancestors like `is_focus_within` → `notancestor:` fails: `:focus` leaks
//!   onto the wrapper and every form gets a ring around all of it.
//! * `is_focus_within` matching the exact node only → `within:` fails; the search box never expands.
//! * `is_focus_visible` ignoring the flag (`self.focused == Some(node)`) → `mousering:` fails,
//!   which is the whole point of the pseudo-class collapsing back into `:focus`.

use manuk_text::FontContext;
use std::collections::HashMap;

/// **The rules live in an EXTERNAL sheet on purpose** — see `g_hover`, where a fixture written with
/// an inline `<style>` was blind to a hover path that silently dropped every `<link>`ed stylesheet.
/// The focus path shares that code, so it must share the coverage.
const CSS: &str = "\
  #inp { box-sizing: border-box; width: 100px; height: 20px; } \
  #inp:focus { width: 300px; } \
  #inp:focus-visible { height: 60px; } \
  #box { width: 50px; } \
  #box:focus-within { width: 400px; } \
  #wrap { width: 600px; } \
  #wrap:focus { width: 700px; }";

const HTML: &str = r##"<!doctype html><html><head>
<link rel="stylesheet" href="/f.css">
<style>body { margin: 0 }</style>
</head><body>
<div id="wrap"><div id="box"><input id="inp" type="text"></div></div>
<input id="other" type="text">
</body></html>"##;

/// One test in the binary — a second `Page::load` stands up a second SpiderMonkey context in the
/// same process and does not survive teardown (see `g_hover`, `g_mse`, `g_quirks_mode`).
#[test]
fn focus_reaches_the_cascade_and_the_three_pseudo_classes_stay_distinct() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://focus.test/", &fonts, 800.0);
    let external = HashMap::from([("https://focus.test/f.css".to_string(), CSS.to_string())]);
    page.apply_stylesheets(&external, &fonts, 800.0);

    let node_of = |page: &manuk_page::Page, sel: &str| {
        let root = page.dom().root();
        manuk_css::query_selector_all(page.dom(), root, sel)[0]
    };
    let rect_of = |page: &manuk_page::Page, sel: &str| {
        page.node_rects()
            .get(&node_of(page, sel))
            .copied()
            .unwrap_or_else(|| panic!("{sel} has no box"))
    };

    // ── Baseline. Nothing is focused; the fixture's own numbers must hold or every claim below is
    //    vacuous.
    assert!(
        (rect_of(&page, "#inp").width - 100.0).abs() < 0.5
            && (rect_of(&page, "#box").width - 50.0).abs() < 0.5,
        "G_FOCUS: unfocused baseline must be #inp 100px / #box 50px — got {} / {}. The fixture is \
         not measuring what it claims.",
        rect_of(&page, "#inp").width,
        rect_of(&page, "#box").width
    );

    // ── KEYBOARD focus on the input. All three should apply.
    let inp = node_of(&page, "#inp");
    let changed = page.set_focus(Some(inp), true, &fonts, 800.0);
    assert!(
        changed,
        "G_FOCUS: focusing an unfocused element is a change"
    );

    assert!(
        (rect_of(&page, "#inp").width - 300.0).abs() < 0.5,
        "G_FOCUS: `#inp:focus {{ width: 300px }}` must apply — got {}. 100 means focus still does \
         not reach the cascade: the shell knows, `document.activeElement` knows, and the style \
         system was never told. That is the dead-end wire this gate exists for.",
        rect_of(&page, "#inp").width
    );
    assert!(
        (rect_of(&page, "#inp").height - 60.0).abs() < 0.5,
        "G_FOCUS: `:focus-visible` must apply to KEYBOARD focus — got height {}. This is the cue a \
         keyboard user navigates by, and on sites that stripped the UA ring with \
         `:focus {{ outline: none }}` it is the ONLY cue left.",
        rect_of(&page, "#inp").height
    );
    assert!(
        (rect_of(&page, "#box").width - 400.0).abs() < 0.5,
        "G_FOCUS: `#box:focus-within` must match while a DESCENDANT holds focus — got {}. This is \
         the expanding search box and the open combobox panel: the <input> takes focus, the <div> \
         is what changes size.",
        rect_of(&page, "#box").width
    );

    // ── `:focus` MUST NOT match ancestors. `#wrap:focus` is on the grandparent.
    // #wrap carries an explicit base width, because a block's AUTO width is 800 — larger than the
    // 700 the focus rule would set. Asserting `< 700` against an auto width cannot tell "the rule
    // did not match" from "the rule matched and I measured the wrong thing", which is a claim that
    // passes for the wrong reason. The base width makes the two answers different numbers.
    let wrap = rect_of(&page, "#wrap").width;
    assert!(
        (wrap - 600.0).abs() < 0.5,
        "G_FOCUS: `:focus` must NOT match an ancestor of the focused element — #wrap resolved to \
         {wrap}, meaning `#wrap:focus` matched. That is what `:focus-within` is for, and \
         conflating them puts a focus ring around the whole form every time one field is focused."
    );

    // ── MOUSE focus. `:focus` still applies; `:focus-visible` must NOT.
    page.set_focus(Some(inp), false, &fonts, 800.0);
    assert!(
        (rect_of(&page, "#inp").width - 300.0).abs() < 0.5,
        "G_FOCUS: `:focus` applies however focus arrived — got {}.",
        rect_of(&page, "#inp").width
    );
    assert!(
        (rect_of(&page, "#inp").height - 20.0).abs() < 0.5,
        "G_FOCUS: `:focus-visible` must NOT match a MOUSE-focused control — got height {}. A ring \
         on every button a mouse user clicks is exactly the noise that made authors write \
         `:focus {{ outline: none }}` in the first place, taking keyboard users' only cue with it. \
         If this collapses into `:focus`, the pseudo-class has no reason to exist.",
        rect_of(&page, "#inp").height
    );

    // ── Moving focus away must UNDO all three. A state only ever added leaves every element that
    //    was ever focused stuck in its focused style.
    page.set_focus(Some(node_of(&page, "#other")), true, &fonts, 800.0);
    assert!(
        (rect_of(&page, "#inp").width - 100.0).abs() < 0.5
            && (rect_of(&page, "#box").width - 50.0).abs() < 0.5,
        "G_FOCUS: moving focus to another control must undo BOTH `:focus` on the old target and \
         `:focus-within` on its ancestors — got #inp {} / #box {}. The ancestor half is the one \
         that needs both chains marked dirty: the <div> is not on the new focus path at all.",
        rect_of(&page, "#inp").width,
        rect_of(&page, "#box").width
    );

    // ── Blur entirely.
    assert!(
        page.set_focus(None, false, &fonts, 800.0),
        "G_FOCUS: blurring is a change"
    );
    assert!(
        page.dom().focused().is_none() && !page.dom().is_focus_within(node_of(&page, "#wrap")),
        "G_FOCUS: with nothing focused, neither :focus nor :focus-within may match anything"
    );
    assert!(
        !page.set_focus(None, false, &fonts, 800.0),
        "G_FOCUS: re-publishing the SAME focus state must report no change — focus is republished \
         on ordinary event turns and recascading the document for each is a per-event cost for no \
         visual change"
    );
}
