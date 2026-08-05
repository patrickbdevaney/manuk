//! # G_TAB_STOP — a TAB advances the pen to the next stop, and before this it advanced nothing
//!
//! `ab\tcd` inside a `<pre>` rendered as `abcd`. Not "the tab was a bit narrow" — **a tab contributed
//! zero advance**, so every tab-indented preformatted block on the web lost its indentation
//! structure entirely: documentation code samples, config listings, diff views, anything pasted from
//! an editor that indents with tabs.
//!
//! Chrome-measured (monospace 16px, shrink-to-fit `<pre>`, tick 959), and the magnitude is not
//! subtle — the four-tab row is **eight times** too narrow:
//!
//! ```text
//!                                        Chrome     before this gate
//!   "ab\tcd"          (tab-size 8)         96.3       31
//!   "ab\tcd"  tab-size:4                   57.8       31
//!   "ab\tcd"  tab-size:2                   57.8       31
//!   "ab\tcd"  tab-size:0                   38.5       31
//!   "a\tb\tc\td"      (tab-size 8)        240.8       31
//!   "ab cd"   an ordinary SPACE  CONTROL    48.2       39   ← always worked
//! ```
//!
//! ## The rule, and the row that discriminates it
//!
//! **A tab advances the pen to the next multiple of `tab-size × the space advance` that is STRICTLY
//! GREATER than where the pen is.** `tab-size` defaults to 8; `tab-size: 0` advances nothing.
//!
//! The `tab-size:2` row is the whole reason this gate is worth writing. `"ab"` sits exactly ON a
//! two-space stop, and Chrome still moves the tab to the *next* one — so `tab-size:2` and
//! `tab-size:4` measure the **same** 6 characters for two different reasons. An implementation that
//! rounds the pen UP to a multiple (rather than going to the next stop) passes `tab-size:4` and
//! fails only here; one that adds `tab-size` spaces passes both of those and fails the multi-tab
//! row. Any gate for this must carry all three.
//!
//! ## Why the assertions are in units of the run's own advance
//!
//! Chrome's numbers above are px in Chrome's monospace face. Ours is whatever face this box has
//! installed, so a px literal would assert the font rather than the rule. `#c10` measures ten
//! characters of the same run and every other row is stated as a character count — which is exactly
//! the form the rule is written in, and it reproduces Chrome's ratios exactly (96.3 / 9.64 = 9.99
//! characters; 240.8 / 9.64 = 24.98).
//!
//! ## How this goes RED — all three RUN, with the numbers they actually produced
//!
//! - **Delete the `InlineItem::Tab` push from `push_preserved_run`** (the run goes back to being one
//!   `Word`) → `#t8` reads **19.3** where 8 characters is 77.1. That is the original defect exactly:
//!   2 characters, i.e. `ab\tcd` laid out as `abcd`.
//! - **Round the pen UP to a multiple instead of advancing to the NEXT stop** (`ceil` for
//!   `floor(..)+1`) → **only `#t2` fails**, reading 19.3 where 4 characters is 38.5. Every other row
//!   in this file still passes, which is the whole reason that row is here.
//! - **Add one `stop` to the pen instead of advancing to a stop** → `#t8` reads **96.3** where 8
//!   characters is 77.1 (the pen was at 2c and gained a full 8c).
//! - **Resolve `tab-size` at parse time against a fixed font size** → `#big` (the same markup at
//!   32px) stops scaling with its own font.
//!
//! ⚠ This gate passed on its FIRST run, which is worth nothing on its own — a gate is only a ratchet
//! tooth once it has been seen to go red. The three mutations above were each applied, run, and
//! reverted.

use manuk_text::FontContext;

// `[TAB]` is substituted for a real `\t` at load: a literal tab in a Rust source string is invisible
// in a diff and one `cargo fmt` away from being eaten, and this fixture is *about* tabs.
const HTML: &str = r##"<!doctype html><html><head><style>
body{margin:0;font-family:monospace;font-size:16px}
pre,div.pw{font-family:monospace;font-size:16px;margin:0;width:2000px}
div.pw{white-space:pre-wrap}
</style></head><body>
<pre><span id="c10">0123456789</span></pre>
<pre>ab[TAB]<span id="t8">cd</span></pre>
<pre style="tab-size:4">ab[TAB]<span id="t4">cd</span></pre>
<pre style="tab-size:2">ab[TAB]<span id="t2">cd</span></pre>
<pre style="tab-size:0">ab[TAB]<span id="t0">cd</span></pre>
<pre>a[TAB]b[TAB]c[TAB]<span id="tm">d</span></pre>
<pre>[TAB]<span id="ti">x</span></pre>
<pre>ab cd <span id="cs">ef</span></pre>
<div class="pw">ab[TAB]<span id="pw">cd</span></div>
<pre style="font-size:32px"><span id="b10">0123456789</span></pre>
<pre style="font-size:32px">ab[TAB]<span id="big">cd</span></pre>
<pre style="display:inline-block;width:auto" id="sf">ab[TAB]cd</pre>
</body></html>"##;

fn rect_of(page: &manuk_page::Page, sel: &str) -> manuk_layout::Rect {
    let dom = page.dom();
    let n = manuk_css::query_selector_all(dom, dom.root(), sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    *page
        .root_box
        .node_rects(dom)
        .get(&n)
        .unwrap_or_else(|| panic!("no box for {sel}"))
}

#[test]
fn g_tab_stop() {
    let fonts = FontContext::new();
    let html = HTML.replace("[TAB]", "\t");
    let page = manuk_page::Page::load(&html, "https://tab.test/", &fonts, 1200.0);

    // The run's own character advance, measured from ten characters of it. Everything below is
    // stated in these units — see the module doc.
    let c = rect_of(&page, "#c10").width / 10.0;
    assert!(
        c > 1.0,
        "G_TAB_STOP: the control run measured {c} px/char — the fixture did not lay out at all, so \
         nothing below means anything"
    );

    let at = |sel: &str, chars: f32, why: &str| {
        let got = rect_of(&page, sel).x;
        let want = chars * c;
        assert!(
            (got - want).abs() < 1.01,
            "G_TAB_STOP: `{sel}` expected x {want:.1} ({chars} characters at {c:.2}px), got \
             {got:.1}.\n  {why}"
        );
    };

    // ── THE DEFECT: a tab advanced nothing, so `cd` sat at 2 characters instead of 8.
    at(
        "#t8",
        8.0,
        "a tab advances the pen to the next multiple of tab-size (8) x the space advance. \
         Reading 2c means the tab contributed ZERO advance — `ab\\tcd` rendered as `abcd`.",
    );

    // ── `tab-size` is honoured, and the two rows that pin the RULE rather than a number.
    at(
        "#t4",
        4.0,
        "tab-size:4 — `ab` is at column 2, the next 4-stop is 4.",
    );
    at(
        "#t2",
        4.0,
        "tab-size:2 — THE DISCRIMINATOR. `ab` sits EXACTLY on a 2-stop and the tab must still move, \
         to the NEXT one (4). Reading 2c means the pen was rounded up to a multiple instead of \
         advanced to the next stop, which passes the tab-size:4 row above.",
    );
    at(
        "#t0",
        2.0,
        "tab-size:0 — Chrome-measured: the tab advances nothing at all, and `ab\\tcd` really is 4 \
         characters wide here.",
    );

    // ── THE MULTI-TAB ROW: the one that separates "advance to a stop" from "insert N spaces".
    at(
        "#tm",
        24.0,
        "`a\\tb\\tc\\t` — 1->8, 9->16, 17->24. Inserting 8 spaces per tab instead would give 27c \
         and would still pass every single-tab row above.",
    );

    // ── A LEADING tab: the indentation case, which is most of the tabs on the web.
    at(
        "#ti",
        8.0,
        "a tab at column 0 advances to the first stop — this is what indents every line of a \
         tab-indented code sample.",
    );

    // ── `pre-wrap` (the `<textarea>` default) preserves tabs too, not only `pre`.
    at(
        "#pw",
        8.0,
        "white-space:pre-wrap preserves tabs — it is the UA default for <textarea> and for every \
         'preformatted but still wrapping' block.",
    );

    // ── THE STOP IS A FUNCTION OF THE RUN'S OWN FONT, resolved at layout, not at parse.
    let cb = rect_of(&page, "#b10").width / 10.0;
    assert!(
        (cb - 2.0 * c).abs() < 1.5,
        "G_TAB_STOP: the 32px control is {cb:.2}px/char against the 16px run's {c:.2} — the fixture \
         is not testing two sizes and the row below proves nothing"
    );
    let big = rect_of(&page, "#big").x;
    assert!(
        (big - 8.0 * cb).abs() < 1.01,
        "G_TAB_STOP: `#big` expected x {:.1} (8 characters at the 32px advance {cb:.2}), got \
         {big:.1}.\n  A `tab-size` <number> is a count of SPACE ADVANCES in the run's own font, so \
         the stop must double when the font does. Resolving it to px at parse time freezes it at \
         whatever size the parser happened to see.",
        8.0 * cb
    );

    // ── THE CONTROL THAT MUST NOT MOVE: an ordinary space always worked, and this fix must not
    //    have touched it. `ab cd ` is 6 characters.
    at(
        "#cs",
        6.0,
        "CONTROL — an ordinary space in a <pre> was never broken. If this moved, the tab work \
         changed preserved-whitespace measurement generally, which is a regression, not a fix.",
    );

    // ── AND THE WIDTH REACHES SHRINK-TO-FIT, which is where a text error becomes a LAYOUT error:
    //    a container that hugs its content one tab too tightly re-wraps everything below it.
    let sf = rect_of(&page, "#sf").width;
    assert!(
        (sf - 10.0 * c).abs() < 1.51,
        "G_TAB_STOP: the inline-block <pre> hugged {sf:.1}px where 10 characters is {:.1} — the \
         tab advances the pen but does not reach the intrinsic width, so every shrink-to-fit box \
         around tabbed text comes out short.",
        10.0 * c
    );
}
