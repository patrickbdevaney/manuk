//! **G_TRANSITION_SAMPLE_COST — an element whose style did not change was paying the full
//! transition sample on EVERY cascade, forever.**
//!
//! `transition-property: all` expands to every animatable longhand, and
//! [`manuk_css::transition::sample`] discovers that nothing is moving by CONSTRUCTING both animated
//! values for each of them and comparing: ~400 `AnimationValue::from_computed_values` calls, each
//! allocating, to return the empty vector. A page that declares `transition: all` on a component —
//! which is a Tailwind/shadcn idiom, not an exotic one — pays that for every such element on every
//! style recalc, whether or not anything about it changed.
//!
//! ⚠⚠⚠ **AND THE MEMO ADDED FOR EXACTLY THIS COST STRUCTURALLY MISSES THIS CASE.** `MEMO` keys the
//! before-change style by POINTER, and `cascade_via_stylo_sized` re-publishes a FRESH `Arc` as the
//! next pass's `before` for every element whose sample came back EMPTY. So the elements that answer
//! *"nothing is transitioning"* over and over — the overwhelming majority — are precisely the ones
//! whose memo key is invalidated on every single pass. The memo covered the running transitions and
//! missed the idle ones, which is backwards from where the mass is.
//!
//! The fix is one comparison: if all twenty style structs are equal then every longhand's computed
//! value is equal, so every one of `sample`'s comparisons is `a == b` and the empty vector is
//! provable without constructing anything.
//!
//! ⭐ **THIS GATE COUNTS, IT DOES NOT TIME.** The cost is invisible in any value the engine
//! publishes, so a wall-clock assertion is the obvious gate and it is the wrong one: the synthetic
//! probe used to find this moved its own UNTOUCHED control arm by 54% between two runs of one
//! binary. `manuk_css::transition::samples_taken()` is exact, machine-independent, and goes red on
//! a number rather than on a mood.
//!
//! MEASURED on the real instrument, not this fixture:
//! `css/css-backgrounds/animations/border-image-width-interpolation.html` reports as many subtests
//! as it can finish before the 5s script watchdog cuts it, so its total was NOT FIXED — 558, 539,
//! 510, 516 across four runs of one release binary, with the failing count invariant at 0. After the
//! fix, three of four runs reach the full **558** and the file's wall time falls from a pinned
//! ~5,050ms to 3,777–4,734ms. ⚠ It is not eliminated — run 6 still truncated at 516. The remaining
//! cost is that every `getComputedStyle` re-cascades the WHOLE document, which is a different and
//! much larger fix.

use manuk_text::FontContext;

/// 60 elements carrying `transition-property: all` with a negative delay — so
/// `transition::may_be_running` says yes for every one of them — and a script that forces repeated
/// style recalcs WITHOUT changing any of them. Nothing here can possibly be transitioning; the
/// question is only what we pay to find that out.
fn page(n: usize, rounds: usize, mutate: bool) -> String {
    let mut s = String::from(
        "<!doctype html><html><head><style>\n\
         .t{width:80px;height:80px;background:#333;\n\
            transition-property:all;transition-duration:1s;transition-delay:-0.5s;}\n\
         </style></head><body>\n",
    );
    for i in 0..n {
        s.push_str(&format!("<div class=t id=t{i}></div>\n"));
    }
    let body = if mutate {
        // The control: one element genuinely changes each round, so a sample MUST be taken.
        "document.getElementById('t0').style.width=(100+r)+'px';"
    } else {
        ""
    };
    s.push_str(&format!(
        "<div id=out>x</div><script>\n\
         var acc=0;\n\
         for(var r=0;r<{rounds};r++){{\n\
           {body}\n\
           acc+=getComputedStyle(document.getElementById('t{last}')).width.length;\n\
         }}\n\
         document.getElementById('out').textContent='acc='+acc;\n\
         </script></body></html>",
        last = n - 1
    ));
    s
}

fn ran(page: &manuk_page::Page) -> String {
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, "#out");
    assert!(!hits.is_empty(), "fixture is missing #out");
    page.dom().text_content(hits[0])
}

#[test]
fn an_unchanged_element_does_not_pay_the_transition_sample() {
    let fonts = FontContext::new();
    const N: usize = 60;
    const ROUNDS: usize = 6;

    // ── t1: nothing changes. Every recalc must conclude "no transition" for all 60 elements
    // WITHOUT entering the animated-value loop even once.
    let before = manuk_css::transition::samples_taken();
    let p = manuk_page::Page::load(&page(N, ROUNDS, false), "http://x/", &fonts, 800.0);
    let quiet = manuk_css::transition::samples_taken() - before;
    let out = ran(&p);
    assert!(
        out.starts_with("acc="),
        "the fixture's script must actually run, or this gate measures nothing — got {out:?}"
    );

    // ── n1 (CONTROL): the identical page, except one element's width really does change every
    // round. A sample is now REQUIRED, and a fix that simply stopped sampling would fail here.
    let before = manuk_css::transition::samples_taken();
    let p2 = manuk_page::Page::load(&page(N, ROUNDS, true), "http://x/", &fonts, 800.0);
    let busy = manuk_css::transition::samples_taken() - before;
    let out2 = ran(&p2);
    assert!(
        out2.starts_with("acc="),
        "control fixture's script must run — got {out2:?}"
    );

    // ⚠ **ZERO, not "fewer" — and the exactness is the point.** The first cascade of the document
    // has no previous style to compare against, so it takes no sample either; every pass after it
    // finds all twenty style structs equal and returns without entering the loop. Pre-fix this
    // reads exactly {N} — one full ~400-construction sample per element — which is what makes the
    // threshold a real edge and not a fitted constant.
    assert_eq!(
        quiet, 0,
        "an unchanged document with {N} `transition: all` elements took {quiet} transition samples. \
         It must take NONE: nothing changed, so nothing can be transitioning, and every one of those \
         samples is ~400 allocating AnimationValue constructions spent to re-derive that. Pre-fix \
         this is exactly {N}."
    );

    // ⭐ THE NEGATIVE HALF, and it is the one that stops this from being satisfied by doing nothing:
    // a document where something really IS transitioning must still take samples.
    assert_eq!(
        busy, ROUNDS as u64,
        "the control page — the same {N} elements, one of them genuinely changing on each of the \
         {ROUNDS} rounds — took {busy} samples. It must take exactly {ROUNDS}: one per real change, \
         and not one per element. Zero here would mean the early-out is swallowing real transitions \
         and `transition: width .3s` is broken on the open web; {N}-ish would mean the memo stopped \
         covering the running case."
    );
}
