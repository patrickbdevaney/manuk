//! **G_OKLCH_COLOR_MIX — Tailwind v4's default palette is `oklch()`, and its opacity modifiers
//! compile to `color-mix()`.**
//!
//! Surface audit #31 filed this `unknown` rather than `missing` on purpose. A grep of `engine/`
//! returns zero occurrences of `oklch` or `color-mix` — and that is **not a measurement**, because
//! Stylo is a *dependency*: it may parse and resolve both without this repository ever naming them.
//! **A grep cannot answer a capability question when the capability lives below you.** So this is a
//! probe first and a gate second.
//!
//! Why it is the largest unresolved question on the CSS list rather than a frontier curiosity:
//! Tailwind v4 does not offer `oklch` as an option, it **emits it by default** — every
//! `text-slate-700`, every `bg-blue-500` is an `oklch()` literal — and every opacity utility
//! (`bg-blue-500/50`) compiles to `color-mix(in oklab, … 50%, transparent)`. A site built on it
//! either renders in colour or renders in whatever our fallback is, and the population is large and
//! growing.
//!
//! The claims are deliberately *behavioural*: each declaration must produce the **specific** sRGB
//! triple the colour actually denotes, not merely "something non-default". Asserting only
//! "not black" would pass on an engine that silently substituted one wrong colour for another.
//!
//! ## THE ANSWER — measured, and it is good news
//!
//! **All five resolve, and four of them to the exact integer.** The capability arrived free, through
//! Stylo, and had simply never been asked for. Row 217 of the constellation moves `unknown` → `gated`.
//!
//! ```text
//! oklch(0.7 0.15 250)                       (75, 163, 247)
//! color-mix(in oklab, red 50%, blue)        (140, 83, 162)
//! color-mix(in srgb, black 50%, white)      (128, 128, 128)
//! lab(50% 40 30)                            (187, 88, 70)
//! color(display-p3 1 0 0)                   (255, 0, 0)
//! ```
//!
//! ## ⚠ THE EXPECTED VALUES BELOW WERE WRONG ON THE FIRST WRITING, AND THAT IS THE LESSON
//!
//! This gate was first written asserting `oklch(0.7 0.15 250) == (57, 137, 217)` and
//! `color-mix(in oklab, …) == (186, 0, 152)` — **numbers recalled rather than derived** — and it
//! failed against an engine that was exactly right. Re-deriving them from the CSS Color 4 matrices
//! (OKLab→LMS→linear sRGB with the published coefficients; Lab via D50→Bradford→D65→sRGB) reproduces
//! the engine's output **to the integer** on all four.
//!
//! So the numbers here are *derived*, and the derivation is written down so the next reader can
//! re-run it instead of trusting it:
//!
//! ```text
//! l_ = L + 0.3963377774a + 0.2158037573b   R = +4.0767416621l − 3.3077115913m + 0.2309699292s
//! m_ = L − 0.1055613458a − 0.0638541728b   G = −1.2684380046l + 2.6097574011m − 0.3413193965s
//! s_ = L − 0.0894841775a − 1.2914855480b   B = −0.0041960863l − 0.7034186147m + 1.7076147010s
//! (l,m,s) = (l_,m_,s_)³, then the sRGB transfer function
//! ```
//!
//! **A gate whose expected value came from memory tests the memory, not the engine** — and it fails
//! in the direction that costs most, because a red gate on correct code invites someone to "fix" the
//! code. Sibling of the project's standing lesson that a probe which *passes* must be asked what it
//! held fixed; this is the same question asked of a probe that *fails*.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><style>
  /* Tailwind v4 emits exactly this shape. oklch(0.7 0.15 250) is a mid blue. */
  #a { color: oklch(0.7 0.15 250); }
  /* …and this one for `/50` opacity modifiers. Half-way between red and blue in oklab. */
  #b { color: color-mix(in oklab, rgb(255,0,0) 50%, rgb(0,0,255)); }
  /* A simple sRGB mix: the answer is exactly halfway, and easy to state. */
  #c { color: color-mix(in srgb, rgb(0,0,0) 50%, rgb(255,255,255)); }
  /* CSS Color 4's `lab()`/`color()` — the same family, different entry points. */
  #d { color: lab(50% 40 30); }
  #e { color: color(display-p3 1 0 0); }
  /* The CONTROL: a plain colour on the same page. If this one fails, the page never cascaded and
     every other assertion here is meaningless. */
  #ctl { color: rgb(1, 2, 3); }
</style></head><body>
<p id="a">a</p><p id="b">b</p><p id="c">c</p><p id="d">d</p><p id="e">e</p><p id="ctl">ctl</p>
</body></html>"##;

fn rgb(page: &manuk_page::Page, sel: &str) -> (u8, u8, u8) {
    let root = page.dom().root();
    let n = manuk_css::query_selector_all(page.dom(), root, sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    let c = page
        .styles_of(n)
        .unwrap_or_else(|| panic!("no style for {sel}"))
        .color;
    (c.r, c.g, c.b)
}

/// Colour maths crosses two float conversions; a couple of levels either way is not a defect.
fn near(got: (u8, u8, u8), want: (u8, u8, u8), tol: i32) -> bool {
    (got.0 as i32 - want.0 as i32).abs() <= tol
        && (got.1 as i32 - want.1 as i32).abs() <= tol
        && (got.2 as i32 - want.2 as i32).abs() <= tol
}

#[test]
fn tailwind_v4_colour_syntax_resolves() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://oklch.test/", &fonts, 800.0);

    let ctl = rgb(&page, "#ctl");
    assert_eq!(
        ctl,
        (1, 2, 3),
        "CONTROL: a plain `rgb()` on the same page must cascade. If this fails the page never \
         styled at all and every other assertion below is measuring nothing."
    );

    let a = rgb(&page, "#a");
    let b = rgb(&page, "#b");
    let c = rgb(&page, "#c");
    let d = rgb(&page, "#d");
    let e = rgb(&page, "#e");
    println!(
        "OKLCH PROBE: oklch={a:?} color-mix-oklab={b:?} color-mix-srgb={c:?} lab={d:?} p3={e:?}"
    );

    // `color-mix(in srgb, black 50%, white)` is exactly mid-grey. The least ambiguous claim here,
    // and the one whose expected value needs no colour-science argument.
    assert!(
        near(c, (128, 128, 128), 2),
        "`color-mix(in srgb, black 50%, white)` must be mid-grey, got {c:?}. This is the simplest \
         possible mix and needs no colour-space argument — if it is wrong, `color-mix()` is not \
         being resolved at all and every Tailwind v4 opacity utility on the page is a wrong colour."
    );
    // oklch(0.7 0.15 250) — a mid blue. Value cross-checked against the CSS Color 4 conversion.
    assert!(
        near(a, (75, 163, 247), 2),
        "`oklch(0.7 0.15 250)` must resolve to its sRGB triple, got {a:?}. Tailwind v4 emits oklch \
         BY DEFAULT for every palette colour, so this is not a frontier feature — it is the colour \
         of the text on a large and growing population of sites."
    );
    assert!(
        near(b, (140, 83, 162), 2),
        "`color-mix(in oklab, red 50%, blue)` must resolve, got {b:?} — this is the exact shape a \
         Tailwind `/50` opacity modifier compiles to."
    );
    assert!(
        near(d, (187, 88, 70), 2),
        "`lab(50% 40 30)` must resolve, got {d:?} — derived via D50 → Bradford → D65 → sRGB"
    );
    assert!(
        near(e, (255, 0, 0), 2),
        "`color(display-p3 1 0 0)` must resolve; P3 red is outside sRGB so it clips to the sRGB \
         primary, got {e:?}"
    );
}
