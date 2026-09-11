//! **G_AN_INLINE_OF_ONLY_BREAKS — `<span><br><br></span>` is a spacer, and it reported a box several
//! line boxes tall and as wide as whatever happened to precede it.**
//!
//! An inline whose entire content is `<br>` elements is the oldest vertical-spacer idiom on the web
//! — `serennu.com` uses `<span class="tiny"><br /><br /></span>` twice in its nav, and it is all over
//! the hand-written long tail. It owns no fragment of its own (the break belongs to the `<br>`), so
//! it fell into the *"an inline that carries no fragment of its own still has an inline box"* branch,
//! which emits **two** reporters — one at the head of the element's items and one at the tail.
//!
//! That rule is right for what it was written for (`<span><i></i></span>`, an inline whose content is
//! an atomic descendant) and wrong here in three ways at once: **the head lands at the END of the
//! PREVIOUS content and the tail at the START of the NEXT line**, so the union comes out several line
//! boxes tall, offset upward, and as wide as the text before it.
//!
//! Chrome-measured on THIS EXACT FIXTURE (`google-chrome-stable --headless=new --dump-dom`,
//! `16px/20px monospace`, `div {width:600px}`). Transcribed, never derived:
//!
//! ```text
//!                                        Chrome            before
//!   XX<span><br></span>YY             [19,  0,0,19]    [ 0,  0,19,39]
//!   XX<span><br><br></span>YY         [ 0, 60,0,19]    [ 0, 40,19,59]
//!   <span><br></span>YY               [ 0,100,0,19]    [ 0,100, 0,39]
//!   XX<span><br></span>               [19,140,0,19]    [ 0,140,19,20]
//! ```
//!
//! **Chrome's rect is ONE line box tall and ZERO wide in every one of them, and it sits where the
//! LAST `<br>` sits.** A single reporter inserted immediately before that break reproduces all four.
//!
//! ## ⚠⚠⚠ THE FLOW IS ALREADY RIGHT AND MUST NOT MOVE — that is half this gate
//!
//! All eight containing-block heights below are Chrome-identical *before and after*. This rule
//! changes what the element REPORTS, never what it contributes to the flow: the reporter is
//! zero-width and `holds_line: false`, exactly like the two it replaces. A fix that moved a `<div>`
//! height would be trading a reported rect for real layout, and the `DIV` rows are what refuse it.
//!
//! ## ⚠⚠ TWO ROWS ARE STILL WRONG, AND THEY ARE HERE ON PURPOSE
//!
//! `s2` and `s6` keep a width of 19 where Chrome says 0 and 10 — in both, 19 is the width of the
//! `XX` that precedes the span, and in both the element's content spans **more than one line**. That
//! is a SECOND defect, in the multi-line inline union rather than in this branch, and it is stated
//! rather than guessed at: *a plausible rule that fits every fixture you happened to write is the
//! most expensive kind of wrong* (t1418). A gate that names what it cannot catch beats one that
//! pretends (t1417) — so both rows are asserted against what we produce TODAY, with Chrome's number
//! in the message, and a fix will fail this test and be told exactly what to write instead.
//!
//! Mutations that must turn this red:
//!   1. restore the two head/tail reporters        -> s1, s3, s4 regress to the `before` column
//!   2. insert before the FIRST break, not the last -> s2 lands on the wrong line (y 40, not 60)
//!   3. insert AFTER the last break                 -> the reporter falls to the next line
//!   4. let the reporter hold a line open           -> a DIV height moves
//!   5. fire the branch for a non-break inline      -> s7 (`<span><i></i></span>`) regresses

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  body { margin:0; font: 16px/20px monospace }
  div  { width: 600px }
  i    { display:inline-block; width:12px; height:8px; background:#000 }
</style></head><body>
<div id="d1">XX<span id="s1"><br></span>YY</div>
<div id="d2">XX<span id="s2"><br><br></span>YY</div>
<div id="d3"><span id="s3"><br></span>YY</div>
<div id="d4">XX<span id="s4"><br></span></div>
<div id="d5">XX<span id="s5">Q<br></span>YY</div>
<div id="d6">XX<span id="s6"><br>Q</span>YY</div>
<div id="d7">XX<span id="s7"><i id="i7"></i></span>YY</div>
<div id="d8">XX<span id="s8"></span>YY</div>
<div id="d9">XX<span id="s9"><br><i id="i9"></i></span>YY</div>
<div id="d10">XX<span id="s10"><i id="i10"></i><br></span>YY</div>
</body></html>"##;

/// `(selector, x, y, w, h)` — every number from `google-chrome-stable --headless=new --dump-dom` on
/// this exact fixture.
const CHROME: &[(&str, i32, i32, i32, i32)] = &[
    // ── The rule this gate is for: an inline whose only content is `<br>`.
    ("#s1", 19, 0, 0, 19),
    ("#s3", 0, 100, 0, 19),
    ("#s4", 19, 140, 0, 19),
    // ── THE CONTROL ARM. These three must NOT change: the branch must keep firing for an inline
    //    whose content is an atomic descendant (`s7`), must not fire for an inline that owns its own
    //    fragment (`s5`), and must leave a genuinely empty inline alone (`s8`). Without them, "insert
    //    one reporter at the last break" could be written as "always insert one reporter".
    ("#s5", 19, 160, 10, 19),
    ("#s7", 19, 240, 12, 19),
    ("#i7", 19, 247, 12, 8),
    ("#i9", 0, 307, 12, 8),
    ("#i10", 19, 327, 12, 8),
    ("#s8", 19, 260, 0, 19),
];

/// ⚠ **KNOWN WRONG, and asserted against OURSELVES so the gate cannot pretend.** Both rows keep the
/// width of the `XX` before them, and in both the element's content spans more than one line — a
/// second defect in the multi-line inline union, not in the branch this gate is about.
/// `(selector, ours-today, chrome)`.
const KNOWN_WRONG: &[(&str, [i32; 4], [i32; 4])] = &[
    ("#s2", [0, 60, 19, 19], [0, 60, 0, 19]),
    ("#s6", [0, 220, 19, 19], [0, 220, 10, 19]),
    // ── ⚠⚠ AND THESE TWO ARE THE DISCRIMINATING ROWS, added because the mutation pass proved the
    //    gate could not tell an inline of only breaks from one that also DRAWS. `s9` and `s10` mix a
    //    `<br>` with an atomic `<i>`: the single-reporter branch must NOT fire for them, and with
    //    the guard removed it does and these numbers move. They are wrong today for the same
    //    multi-line reason as `s2`/`s6`, so they are pinned, not claimed.
    ("#s9", [0, 280, 19, 39], [0, 300, 12, 19]),
    ("#s10", [0, 320, 31, 39], [19, 320, 12, 19]),
];

/// **THE FLOW MUST NOT MOVE.** Chrome's heights for all eight containing blocks; identical before
/// and after the change.
const BLOCK_HEIGHTS: &[(&str, i32)] = &[
    ("#d1", 40),
    ("#d2", 60),
    ("#d3", 40),
    ("#d4", 20),
    ("#d5", 40),
    ("#d6", 40),
    ("#d7", 20),
    ("#d8", 20),
    ("#d9", 40),
    ("#d10", 40),
];

#[test]
fn an_inline_whose_only_content_is_br_reports_one_line_box_at_the_last_break() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://inline.test/", &fonts, 800.0);
    let dom = page.dom();
    let rects = page.root_box.node_rects(dom);

    let find = |sel: &str| -> [i32; 4] {
        let n = manuk_css::query_selector_all(dom, dom.root(), sel);
        assert!(
            !n.is_empty(),
            "G_AN_INLINE_OF_ONLY_BREAKS: `{sel}` did not match"
        );
        let r = rects
            .get(&n[0])
            .unwrap_or_else(|| panic!("G_AN_INLINE_OF_ONLY_BREAKS: `{sel}` has NO rect at all"));
        [
            r.x.round() as i32,
            r.y.round() as i32,
            r.width.round() as i32,
            r.height.round() as i32,
        ]
    };

    // ── VACUITY. `#s1` is the ordinary spelling and the whole point; if it does not hold, every
    //    control row below is passing on an engine that never implemented the rule.
    assert_eq!(
        find("#s1"),
        [19, 0, 0, 19],
        "VACUOUS: `XX<span><br></span>YY` must report ONE line box at the break — Chrome says \
         [19 0 0x19]. Without this row the control arm proves nothing."
    );

    for &(sel, x, y, w, h) in CHROME {
        assert_eq!(
            find(sel),
            [x, y, w, h],
            "G_AN_INLINE_OF_ONLY_BREAKS: {sel} is {:?}, Chrome says [{x} {y} {w}x{h}]\n\n  \
             An inline whose only content is `<br>` reports ONE line box, ZERO wide, where the LAST \
             break sits. Two reporters straddling the breaks put the head at the end of the \
             PREVIOUS content and the tail at the start of the NEXT line, so the union came out \
             several lines tall and as wide as the text before it. The `#s5`/`#s7`/`#s8` rows are \
             the control: an inline that owns a fragment, one whose content is an atomic \
             descendant, and an empty one must all be untouched.",
            find(sel)
        );
    }

    for &(sel, ours, chrome) in KNOWN_WRONG {
        assert_eq!(
            find(sel),
            ours,
            "G_AN_INLINE_OF_ONLY_BREAKS: {sel} moved.\n\n  \
             This row is KNOWN WRONG and pinned on purpose: we produce {ours:?} and Chrome says \
             {chrome:?}. Both remaining rows keep the width of the `XX` before them and in both the \
             element's content spans MORE THAN ONE LINE — a second defect, in the multi-line inline \
             union rather than in the single-reporter branch this gate is about.\n\n  \
             If you have just fixed it: this failure is the good outcome. Move {sel} into CHROME \
             with {chrome:?} and delete it from KNOWN_WRONG."
        );
    }

    for &(sel, h) in BLOCK_HEIGHTS {
        assert_eq!(
            find(sel)[3],
            h,
            "G_AN_INLINE_OF_ONLY_BREAKS: {sel} height is {}, Chrome says {h}\n\n  \
             ⚠ THE FLOW MUST NOT MOVE. This rule changes what an inline REPORTS, never what it \
             contributes: the reporter is zero-width and holds no line open, exactly like the two \
             it replaces. A containing block whose height moved means the reporter grew a \
             `holds_line`, which trades a reported rect for real layout.",
            find(sel)[3]
        );
    }
}
