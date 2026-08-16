//! **G_GRID_LINE_NAMES — `<line-names>` are part of `grid-template-columns`'s resolved value, and
//! every one of them was dropped.**
//!
//! `getComputedStyle(el).gridTemplateColumns` on `[a] repeat(4, [b] 200px [c]) [d]` is
//! `"[a b] 200px [c b] 200px [c b] 200px [c b] 200px [c d]"` in Chrome, and was
//! `"200px 200px 200px 200px"` here. **Every track SIZE was already right** — the names survive
//! parsing and survive the cascade, and were discarded at the `stylo_map` boundary, because
//! `template_to_tracks` never read `TrackList::line_names`.
//!
//! What that costs: a grid library reading its own template back gets a list it cannot match to the
//! named lines it wrote, and `grid-column: b / c` cannot be reconciled with the resolved value.
//!
//! ⚠⚠⚠ **THE MERGE AT A REPEAT BOUNDARY IS THE HARD PART AND IS WHY THE `rep:` ROW LEADS.** A
//! repetition's *closing* names and the next repetition's *opening* names are ONE grid line, so
//! `repeat(4, [b] 200px [c])` yields `[c b]` in the middle and `[c d]` at the end — not `[c] [b]`
//! and not four separate `[b]`/`[c]` pairs. Getting the count right but the merge wrong produces a
//! name attached to the wrong line, which is a **wrong answer of the right type** and strictly worse
//! than the missing names it replaces.
//!
//! **To watch it go RED:**
//!
//! 1. return `Vec::new()` from `template_line_names` → every row loses its brackets and reads exactly
//!    as it did before this tick;
//! 2. push instead of merging at the repeat boundary → `rep:` reads `[c] [b]` where it wants `[c b]`,
//!    and the trailing group lands on the wrong line;
//! 3. drop the `sizes.len()` trailing group in the serializer's weave → the closing `[d]` / `[last]`
//!    disappears while everything before it still passes.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
 .g { display: grid; width: 800px; }
 #rep  { grid-template-columns: [a] repeat(4, [b] 200px [c]) [d]; }
 #plain{ grid-template-columns: repeat(4, 200px); }
 #ends { grid-template-columns: [first] 90px [last]; }
 #fr   { grid-template-columns: [a] 1fr [b] 1fr [c]; }
 #mid  { grid-template-columns: 60px [mid] 100px; }
</style></head><body style="margin:0">
<div class="g" id="rep"><div>1</div></div>
<div class="g" id="plain"><div>1</div></div>
<div class="g" id="ends"><div>1</div></div>
<div class="g" id="fr"><div>1</div></div>
<div class="g" id="mid"><div>1</div></div>
<div id="out">-</div>
<script>
  var R = [];
  ['rep', 'plain', 'ends', 'fr', 'mid'].forEach(function (k) {
    R.push(k + ':<' + getComputedStyle(document.getElementById(k)).gridTemplateColumns + '>');
  });
  document.getElementById('out').textContent = R.join('  ');
</script></body></html>"##;

// ⚠ ONE `#[test]` fn per JS gate binary — two live `PageContext`s on two threads fault SpiderMonkey.
#[test]
fn grid_template_resolved_value_carries_its_line_names() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://gridnames.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        (
            "rep:<[a b] 200px [c b] 200px [c b] 200px [c b] 200px [c d]>",
            "THE GATE, and this exact string is what WPT lists as acceptable. A repetition's closing \
             names and the next one's opening names are ONE line, so the middles are `[c b]` and the \
             end is `[c d]`. Pushing instead of merging gives `[c] [b]` — the right count of names \
             attached to the wrong lines, which is worse than having none",
        ),
        (
            "plain:<200px 200px 200px 200px>",
            "THE CONTROL. An unnamed template must gain NOTHING — no empty `[]` groups, no stray \
             spaces. It is also the row that proves the sizes were never the problem: named and \
             unnamed `repeat(4, 200px)` lay out identically",
        ),
        (
            "ends:<[first] 90px [last]>",
            "the simple case, and the row that catches a weave which drops its trailing group: \
             `[last]` sits after the final track and has no size to hang off",
        ),
        (
            "fr:<[a] 400px [b] 400px [c]>",
            "names ride on the USED value, not the specified one — `1fr` resolves to 400px on an \
             800px grid and the names must survive that substitution rather than only appearing on \
             the computed-value fallback path",
        ),
        (
            "mid:<60px [mid] 100px>",
            "a name on an interior line only: the first and last groups are empty and must be \
             OMITTED, not emitted as `[]`",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_GRID_LINE_NAMES: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
