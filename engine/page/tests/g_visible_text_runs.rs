//! **G_VISIBLE_TEXT_RUNS — a line-BREAK OPPORTUNITY is not a space.**
//!
//! `Page::visible_text()` is the page as *read* rather than as marked up: it walks the laid-out
//! boxes and concatenates their inline runs. It is not a convenience — it is
//! `Observation.text`, the string `manuk-agent` hands a model, and the body
//! `store::history_index` embeds for full-text history search.
//!
//! It joined **every** fragment with a space:
//!
//! ```rust
//! words.join(" ")
//! ```
//!
//! But the line breaker emits a fragment per *break opportunity*, not per line — and CSS puts a
//! break opportunity after a hyphen, after `//`, and after `?` in a URL. So a word the layout merely
//! *could* have broken came back broken, on the same line, with a space wedged into it:
//!
//! ```text
//! rendered:  This site blocks non-mainstream browsers
//! observed:  This site blocks non- mainstream browsers
//! rendered:  https://walled.example/?a=1&b=2
//! observed:  https:// walled.example/? a=1&b=2
//! ```
//!
//! **Nothing about the rendering was wrong** — the pixels are right, the DOM `textContent` is right.
//! Only the string the agent reads and the index searches was wrong, which is why no visual gate
//! could see it and why it survived to be found by a `contains()` assertion on an unrelated test.
//!
//! The consequences are exactly where they hurt most for an agent-native browser: a model asked to
//! find "non-mainstream" on the page finds nothing; a user searching their history for a URL finds
//! nothing; and every hyphenated compound, every URL and every long token on the open web is
//! affected, silently, in favour of a *plausible-looking* string.
//!
//! **The fix uses the geometry that was already there.** Two runs on the same baseline whose boxes
//! touch (`next.x <= prev.x + prev.width`) are one word: concatenate. A real gap on the same line, or
//! a different baseline, separates words: insert one space. A trailing space that belongs to the run
//! is already inside `text` and already inside `width`, so it survives either way.
//!
//! Claims:
//! - a hyphenated compound, a URL and a slashed path come back **whole**;
//! - words genuinely separated by a space still are (the guard against "concatenate everything",
//!   which would pass claim 1 and glue the page into one token);
//! - a hard `<br>` still separates;
//! - text from two different blocks still separates.

use manuk_text::FontContext;

// 320px viewport, deliberately narrow: real wrapping happens, so the same string exercises both the
// "broke here" and "merely could have broken here" paths.
const HTML: &str = r##"<!doctype html><html><body style="margin:0;font:16px sans-serif">
<p id="hyph">This site blocks non-mainstream browsers today</p>
<p id="url">https://walled.example/?a=1&amp;b=2 did not load</p>
<p id="gap">alpha beta gamma</p>
<p id="br">before<br>after</p>
<div id="b1">block one</div><div id="b2">block two</div>
</body></html>"##;

#[test]
fn a_break_opportunity_is_not_a_space() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://vt.test/", &fonts, 320.0);
    let got = page.visible_text();
    println!("VISIBLE TEXT: {got:?}");

    for (want, why) in [
        (
            "non-mainstream",
            "a hyphen is a break OPPORTUNITY, not a space. The line breaker emits `non-` and \
             `mainstream` as separate runs whether or not the line actually broke there, and \
             joining every run with ' ' wedged a space into the middle of the word an agent was \
             asked to find",
        ),
        (
            "https://walled.example/?a=1&b=2",
            "a URL breaks after `//` and after `?` too — so the string a user searches their \
             history for came back as `https:// walled.example/? a=1&b=2` and matched nothing",
        ),
        (
            "alpha beta gamma",
            "THE GUARD: words genuinely separated by a space must stay separated. Fixing claim 1 \
             by concatenating everything would pass it and glue the whole page into one token — a \
             worse failure, and one that also looks plausible",
        ),
        (
            "before after",
            "a hard `<br>` is a real separation: the runs sit on different baselines, so one space",
        ),
        (
            "block one block two",
            "two sibling blocks are separate lines, so their text stays separated",
        ),
    ] {
        assert!(
            got.contains(want),
            "G_VISIBLE_TEXT_RUNS: expected {want:?} in visible_text()\n  got: {got:?}\n\n  {why}."
        );
    }

    // The negative form of claim 1, stated separately: it is the exact string the bug produced, and
    // asserting its ABSENCE is what makes this gate go red for the original defect rather than for
    // some other change to spacing.
    assert!(
        !got.contains("non- mainstream"),
        "G_VISIBLE_TEXT_RUNS: `non- mainstream` — a space inserted at a break opportunity the \
         layout did not take. got: {got:?}"
    );
    assert!(
        !got.contains("https:// "),
        "G_VISIBLE_TEXT_RUNS: `https:// ` — same defect, inside a URL. got: {got:?}"
    );
}
