//! **G_COUNTER_SET_AND_PSEUDO_COUNTERS — a counter property on a PSEUDO did nothing, and
//! `counter-set` did not exist.**
//!
//! Two defects in the same walk, both in the construct authors actually write:
//!
//! ```css
//!   li::before { counter-increment: item; content: counter(item) "." }   /* the numbered list */
//! ```
//!
//! `counter_snapshots` read `counter-reset` / `-increment` from the **ELEMENT only**, so a pseudo's
//! own counter actions were ignored entirely and every item rendered the same number. And
//! `counter-set` — CSS Lists 3's third action — was not implemented on either cascade.
//!
//! ⚠ `counter-set` is `engine = "gecko"` in stylo's `longhands.toml`: it does **not exist** in the
//! servo build, so there is no `clone_counter_set()` and no pref that brings one back. It is
//! recovered from `MinimalCascade` the way `appearance` and `field-sizing` already are — **onto the
//! element AND onto its pseudos**, because the pseudo is where it is written.
//!
//! ⭐ That makes a THIRD shape in the "why is this property missing" family, and the three are
//! distinguished by where the gate lives: a `servo_pref` row in `longhands.toml` (t1358, multicol),
//! a `static_prefs::pref!` call inside a value parser (t1369, the `content` alt syntax), and
//! `engine = "gecko"` — which no flag reaches at all.
//!
//! ## ⭐⭐⭐ TWO SPEC CLAIMS WRITTEN FROM MEMORY, BOTH MEASURED WRONG
//!
//! The first draft of this code asserted, in prose, with a worked example:
//!
//! > *"the three run **reset, then set, then increment** … `counter-reset: c; counter-set: c 5;
//! > counter-increment: c` ends at 6"* — and *"`counter-set` **assigns** to a counter that already
//! > exists and does nothing if it does not."*
//!
//! Both are false, and one Chrome measurement each settled it:
//!
//! ```text
//!   counter-reset: a 0; counter-set: a 99; counter-increment: a 1   renders TWO digits (99)
//!                                                                   — not three (100)
//!   counter-set: b 99   with no reset anywhere                      renders TWO digits (99)
//!                                                                   — not one (0)
//! ```
//!
//! So the order is **reset → increment → SET**, and `counter-set` **creates** a counter that does
//! not exist. The reset/set distinction is about SCOPING (reset opens a new nested counter), not
//! about whether the value lands. *A spec sentence recalled is a hypothesis; the fixture is the
//! test.*
//!
//! ## The fixture is built so WIDTH can tell the answers apart
//!
//! A rendered counter is read here as the element's width in a `max-content` box at
//! `16px monospace` (9.639px/char), because Chrome does not expose a pseudo's resolved text to
//! script. **Every row is chosen so a wrong answer changes the DIGIT COUNT**, which width can see:
//!
//! ```text
//!   a 12-item list, `li::before{counter-increment:item; content:counter(item) "."}`
//!     items 1-9    28.92  ("1.x")     items 10-12  38.55  ("10.x")
//!     if the pseudo's increment were ignored, EVERY item is "0.x" and item 10 reads 28.92
//!
//!   reset a 0 · set a 99 · increment a 1     28.91  ("99y")  <- 100 would be 38.55
//!   set b 99, no reset                       28.91  ("99y")  <- 0 would be 19.28
//!   reset c 0 · set c 99                     28.91  ("99y")  <- 0 would be 19.28
//! ```
//!
//! PROVEN RED by three mutations — see the module tail.

use manuk_dom::NodeId;
use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
html,body{margin:0;padding:0}
body{font-family:monospace;font-size:16px;line-height:24px}
ol{list-style:none;padding:0;margin:0;counter-reset:item;width:max-content}
li{width:max-content}
li::before{counter-increment:item; content:counter(item) ".";}
div{width:max-content}
#o1{counter-reset:a 0;counter-set:a 99;counter-increment:a 1} #o1::before{content:counter(a)}
#o2{counter-set:b 99} #o2::before{content:counter(b)}
#o3{counter-reset:c 0;counter-set:c 99} #o3::before{content:counter(c)}
</style></head><body>
<ol>
<li id="n1">x</li><li id="n2">x</li><li id="n3">x</li><li id="n4">x</li>
<li id="n5">x</li><li id="n6">x</li><li id="n7">x</li><li id="n8">x</li>
<li id="n9">x</li><li id="n10">x</li><li id="n11">x</li><li id="n12">x</li>
</ol>
<div id="o1">y</div><div id="o2">y</div><div id="o3">y</div>
</body></html>"##;

fn by_id(page: &manuk_page::Page, id: &str) -> NodeId {
    let dom = page.dom();
    dom.descendants(dom.root())
        .find(|&n| {
            dom.element(n)
                .and_then(|e| e.attr("id"))
                .is_some_and(|v| v == id)
        })
        .unwrap_or_else(|| panic!("VACUOUS: no element with id={id:?}"))
}

fn width(page: &manuk_page::Page, id: &str) -> f32 {
    page.root_box
        .node_rects(page.dom())
        .get(&by_id(page, id))
        .unwrap_or_else(|| panic!("VACUOUS: no box for id={id:?}"))
        .width
}

#[test]
fn a_pseudo_s_counter_actions_run_and_counter_set_exists() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ctr.test/", &fonts, 1200.0);
    let near = |g: f32, w: f32| (g - w).abs() < 1.5;

    // ── VACUITY. The pseudo must be materialising SOMETHING, or every width below is just "x".
    assert!(
        width(&page, "n1") > 20.0,
        "VACUOUS: item 1 measures {} — narrower than one digit plus a period plus the text, so the \
         ::before is not being materialised and no row below is a test of counters",
        width(&page, "n1")
    );

    // ── ARM 1 · THE PSEUDO'S OWN `counter-increment` RUNS. Items 1-9 are one digit, 10-12 are two.
    //    If the pseudo's increment were ignored every item would read "0.x" and item 10 would match
    //    item 9 — which is exactly what the DIGIT BOUNDARY is here to catch.
    for n in [1usize, 2, 9] {
        let w = width(&page, &format!("n{n}"));
        assert!(
            near(w, 28.92),
            "G_COUNTER_SET_AND_PSEUDO_COUNTERS item {n}: one digit plus '.x' is 28.92 wide in \
             Chrome, got {w}"
        );
    }
    for n in [10usize, 11, 12] {
        let w = width(&page, &format!("n{n}"));
        assert!(
            near(w, 38.55),
            "G_COUNTER_SET_AND_PSEUDO_COUNTERS item {n}: TWO digits plus '.x' is 38.55 wide in \
             Chrome, got {w}. Reading 28.92 means the counter never passed 9 — a pseudo's own \
             `counter-increment` is being ignored and every item renders the same number."
        );
    }

    // ── ARM 2 · `counter-set` EXISTS, RUNS LAST, AND CREATES.
    let rows: &[(&str, f32, &str)] = &[
        ("o1", 28.91, "reset→increment→SET: 99, not 100. Reading 38.55 is the reset→set→increment order this code first assumed"),
        ("o2", 28.91, "`counter-set` on a counter that does not exist CREATES it: 99, not 0. Reading 19.28 is the `assigns only if present` reading"),
        ("o3", 28.91, "reset then set, no increment: 99. Reading 19.28 means the set was dropped entirely"),
    ];
    for (id, want, why) in rows {
        let got = width(&page, id);
        assert!(
            near(got, *want),
            "G_COUNTER_SET_AND_PSEUDO_COUNTERS #{id}: Chrome renders this {want} wide, got {got}.\n  \
             {why}"
        );
    }
}

// ── HOW THIS GOES RED ──────────────────────────────────────────────────────────────────────────
//
// N1  drop the pseudo counter-action loop from `counter_snapshots` (the pre-tick behaviour)
//       -> every list item reads 28.92, so items 10-12 fail. The `o*` rows stay green, because
//          their counter actions are on the ELEMENT.
// N2  run `counter-set` BEFORE `counter-increment` (the order this code first assumed)
//       -> `o1` reads 38.53 — 100 instead of 99. The other two rows stay green, which is why the
//          order needs its own row rather than being folded into "does set work".
//       ⚠ The mutation must MOVE the set loop, not add a second one before the increment: a
//          duplicate leaves the later loop winning and the gate stays GREEN. Recorded because the
//          first attempt at this mutation did exactly that and read as a vacuous row.
// N3  guard `counter-set` with `if let Some(slot) = live.get_mut(nm)` ("assign only if present")
//       -> `o2` reads 19.28 — the counter is never created. `o1` and `o3` stay green because a
//          reset precedes the set in both, which is exactly why the un-reset row exists.
