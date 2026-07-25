//! **G_CASCADE_ORIGIN — an author rule beats a UA rule, whatever its specificity.**
//!
//! The cascade sorts by **origin first and specificity second**: a normal author declaration
//! outranks *every* normal user-agent declaration, no matter how specific the UA selector is or how
//! weak the author's. That ordering is what makes `* { margin: 0 }` work, and `* { margin: 0 }` —
//! together with `*,::before,::after { margin:0; padding:0 }`, which is the first rule in Tailwind's
//! preflight, in Normalize, in every hand-rolled reset since 2004 — is on a large fraction of the
//! open web.
//!
//! **It did not work here.** `stylo_engine.rs` handed our `UA_CSS` sheet to the Stylist with
//! `Origin::Author`, on a comment reading *"the UA sheet is matched first (lowest priority); author
//! rules override it"*. Document order is only the **last** tie-break in the cascade, below origin
//! and below specificity — so `body { margin: 8px }` (0,0,1) beat an author `* { margin: 0 }` (0,0,0)
//! and the reset was silently ignored. Measured at tick 556 against live Chromium on the same page:
//! Chromium put `body` at `[0 0 1200×92]`, we put it at `[8 8 1184×91]`.
//!
//! The 8px is the *smallest* instance. Every UA rule in the sheet has a type or descendant selector
//! and therefore outranked a reset: `ul, ol { padding-left: 40px }` survived `* { padding: 0 }`,
//! `blockquote { margin: 1em 40px }` survived it, and any author rule written with a
//! deliberately-weak selector — which is what a reset is — lost. A whole page's horizontal rhythm is
//! decided by rules the author explicitly overrode.
//!
//! **The fix is the origin, not a re-ordering hack**: the sheet is parsed as `Origin::UserAgent`, so
//! Stylo's own cascade puts it below the author level where it belongs. That also gets the
//! `!important` ordering right for free (a UA `!important` outranks an author `!important`), which
//! no amount of sheet re-ordering can express.
//!
//! Four claims, and the last three are guards — a fix that made author rules always win by
//! *dropping* the UA sheet's influence would pass claim 1 and be much worse than the bug:
//!
//! 1. **the reset wins** — `* { margin: 0 }` zeroes the UA `body` margin, and `* { padding: 0 }`
//!    zeroes the UA `ul` padding-left;
//! 2. **the UA rule still applies when nobody overrides it** — an untouched `body`/`ul` on the same
//!    page keeps 8px / 40px;
//! 3. **specificity still decides WITHIN the author origin** — `#id` beats `*`, so the fix has not
//!    replaced one hard-coded winner with another;
//! 4. **author `!important` still beats author normal** — the ordinary important path is untouched.

use manuk_text::FontContext;

/// Two documents, because claim 1 and claim 2 are contradictory *on the same element*: the point of
/// claim 2 is that the SAME UA rule that just lost to a reset still wins when there is no reset.
/// Different elements on one page would work for `ul` but not for `body`, of which there is one.
const RESET: &str = r##"<!doctype html><html><head><style>
  * { margin: 0; padding: 0; }
  #loud { color: rgb(0, 128, 0); }
  * { color: rgb(255, 0, 0); }
  .imp { margin-left: 7px !important; }
  #beats { margin-left: 3px; }
</style></head><body>
<ul id="ul"><li>a</li></ul>
<blockquote id="bq">q</blockquote>
<p id="loud">loud</p>
<div id="beats" class="imp">x</div>
</body></html>"##;

/// The same markup with NO author rules at all — the UA sheet must still place everything.
const BARE: &str = r##"<!doctype html><html><body>
<ul id="ul"><li>a</li></ul>
<blockquote id="bq">q</blockquote>
</body></html>"##;

fn style_of<'p>(page: &'p manuk_page::Page, sel: &str) -> &'p manuk_css::ComputedStyle {
    let root = page.dom().root();
    let n = manuk_css::query_selector_all(page.dom(), root, sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"));
    page.styles_of(n)
        .unwrap_or_else(|| panic!("no style for {sel}"))
}

/// `[margin-top, margin-left, padding-left]` in px, as the LIVE cascade computed it.
fn boxes(page: &manuk_page::Page, sel: &str) -> [f32; 3] {
    let s = style_of(page, sel);
    [
        s.margin.top.resolve(0.0, 0.0),
        s.margin.left.resolve(0.0, 0.0),
        s.padding.left.resolve(0.0, 0.0),
    ]
}

fn assert_px(got: f32, want: f32, what: &str, why: &str) {
    assert!(
        (got - want).abs() < 0.51,
        "G_CASCADE_ORIGIN: {what} — expected {want}px, got {got}px.\n  {why}"
    );
}

#[test]
fn an_author_rule_outranks_a_user_agent_rule_whatever_its_specificity() {
    let fonts = FontContext::new();
    let reset = manuk_page::Page::load(RESET, "https://origin.test/", &fonts, 800.0);
    let bare = manuk_page::Page::load(BARE, "https://origin.test/", &fonts, 800.0);

    // ── 1. THE RESET WINS. This is the defect, and it is the commonest author rule on the web.
    assert_px(
        boxes(&reset, "body")[0],
        0.0,
        "`* { margin: 0 }` vs the UA `body { margin: 8px }` (margin-top)",
        "a normal AUTHOR declaration outranks every normal USER-AGENT declaration — origin is \
         sorted before specificity, so `*` (0,0,0) must still beat `body` (0,0,1). Handing the UA \
         sheet to the Stylist as `Origin::Author` demotes that to a document-order tie-break, which \
         the more specific UA selector wins",
    );
    assert_px(
        boxes(&reset, "#ul")[2],
        0.0,
        "`* { padding: 0 }` vs the UA `ul, ol { padding-left: 40px }`",
        "the 8px body margin is the SMALLEST instance of this bug — every UA rule in the sheet has \
         a type selector, so every one of them outranked the reset that was written to remove it",
    );
    assert_px(
        boxes(&reset, "#bq")[1],
        0.0,
        "`* { margin: 0 }` vs the UA `blockquote, figure { margin: 1em 40px }` (margin-left)",
        "a 40px indent the author explicitly removed",
    );

    // ── 2. THE GUARD THAT MATTERS MOST. Winning claim 1 by weakening the UA sheet would be a far
    //    worse browser than the bug: every unstyled document would lose its metrics at once.
    assert_px(
        boxes(&bare, "body")[0],
        8.0,
        "the UA `body { margin: 8px }` on a page with NO author rules",
        "the UA sheet must still apply when nothing overrides it — a fix that made author rules \
         win by removing the UA sheet's influence would pass claim 1 and break every unstyled page",
    );
    assert_px(
        boxes(&bare, "#ul")[2],
        40.0,
        "the UA `ul, ol { padding-left: 40px }` on a page with NO author rules",
        "same guard, on the property that decides where every bullet on the web sits",
    );
    assert_px(
        boxes(&bare, "#bq")[1],
        40.0,
        "the UA `blockquote { margin: 1em 40px }` on a page with NO author rules",
        "same guard, horizontally",
    );

    // ── 3. SPECIFICITY STILL DECIDES WITHIN THE AUTHOR ORIGIN. `#loud` is declared BEFORE the
    //    universal rule, so if specificity stopped being consulted the later `*` would win on order.
    let c = style_of(&reset, "#loud").color;
    assert!(
        (c.r, c.g, c.b) == (0, 128, 0),
        "G_CASCADE_ORIGIN: `#loud {{ color: green }}` must beat a LATER `* {{ color: red }}` — \
         specificity is still the tie-break inside one origin. got rgb({}, {}, {})",
        c.r,
        c.g,
        c.b
    );

    // ── 4. AUTHOR `!important` still beats author normal, and still beats a more specific normal
    //    rule. Origin sorting and importance sorting are the same comparison; a change to one is
    //    exactly the kind of change that silently breaks the other.
    assert_px(
        boxes(&reset, "#beats")[1],
        7.0,
        "`.imp { margin-left: 7px !important }` vs `#beats { margin-left: 3px }`",
        "an important author declaration outranks a normal one of HIGHER specificity — the \
         importance half of the same sort the origin fix touches",
    );
}
