//! **G_CAP_TOUCH_PROBE — the FUNCTION leg's producer, driven through a real page load.**
//!
//! `DAILY-DRIVER-CERTIFICATION.md` §4's FUNCTION leg requires the capabilities a site *actually
//! touches* to exercise green for that site. Ticks 583–585 built the certificate's shape; until this
//! gate the shape had **no producer** — `CapOutcome` was decided by whatever a caller passed in,
//! which is the same "a number nobody measured" defect the whole redesign exists to remove.
//!
//! It runs `corpus::TOUCH_PROBE_JS` on real pages **in this engine** and asserts the record. Three
//! claims, each mapping to a property the probe has to have:
//!
//! 1. **A page that touches nothing records nothing.** Every capability stays `Untouched`, so a
//!    static document legitimately passes. If this failed, every site would carry claims it never
//!    made and the certificate would stop being finite.
//! 2. **A touch that works is recorded `works`** — the probe must not report a working capability as
//!    broken merely because it wrapped it.
//! 3. **An observer that never fires is `noop`, not `works`.** This is the claim
//!    `typeof X === 'function'` cannot make, and this engine ships inert stubs that would pass such
//!    a check. An observer that never fires and one that does not exist are the same thing to the
//!    user staring at an empty feed.
//!
//! The probe is injected as an ordinary page script ahead of the document's own — the shape
//! `chrome.rs` already uses for instrumented copies — so it observes the *page*, not the engine's
//! internals. It lives in this crate rather than `engine/page/tests` because `manuk-page` cannot
//! depend on `manuk-wpt`: the instrument depends on the engine, never the reverse.

use manuk_text::FontContext;
use manuk_wpt::corpus::{parse_touch_record, CapOutcome, TOUCH_PROBE_JS};

fn record(body: &str) -> String {
    let html = format!(
        "<!doctype html><html><body><script>{TOUCH_PROBE_JS}</script><script>{body}</script>\
         <script>__manukCapsFlush();</script></body></html>"
    );
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(&html, "https://cap.test/", &fonts, 800.0);
    let root = page.dom().root();
    // ⚠ The node must EXIST. Returning `String::new()` when the probe never ran would make claim 1
    // ("a quiet page records nothing") pass for the worst possible reason — the probe not running at
    // all — which is `CERT_MIN_SHAPE_SAMPLE`'s vacuous-pass lesson in a test helper. It bit exactly
    // that way while this gate was being written: the localStorage claim failed with `got: ""`, and
    // the quiet-page claim was passing on the same empty string.
    let n = manuk_css::query_selector_all(page.dom(), root, "#__manuk_caps")
        .first()
        .copied()
        .expect(
            "the probe did not run: `#__manuk_caps` is absent, so an empty record cannot be \
             distinguished from a probe that never executed",
        );
    page.dom().text_content(n)
}

#[test]
fn the_probe_records_what_the_page_touches_and_nothing_else() {
    // ── 1. A page that touches nothing.
    let none = record("var x = 1 + 1;");
    let f = parse_touch_record("quiet", &none).expect("record must parse");
    assert!(
        f.caps.iter().all(|(_, o)| *o == CapOutcome::Untouched),
        "a page that reaches for nothing must record nothing — otherwise every site carries claims \
         it never made and the certificate stops being finite. got: {none:?}"
    );
    assert!(
        f.functions(),
        "…and it FUNCTIONS: a static document really does work without IndexedDB"
    );

    // ── 2. A touch that works.
    let works = record("try { localStorage.setItem('k','v'); } catch (e) {}");
    assert!(
        works.contains("local-storage=works"),
        "a capability the page used successfully must record `works` — the probe must not report a \
         working capability as broken merely because it wrapped it. got: {works:?}"
    );

    // ── 3. THE CLAIM A PRESENCE CHECK CANNOT MAKE.
    let obs = record("try { var o = new IntersectionObserver(function(){}); } catch (e) {}");
    assert!(
        obs.contains("intersection-observer=noop"),
        "an IntersectionObserver that is CONSTRUCTED but never FIRES must record `noop`, not \
         `works`. `typeof IntersectionObserver === 'function'` cannot tell those apart, and this \
         engine ships inert stubs that would pass such a check. got: {obs:?}"
    );
    let parsed = parse_touch_record("obs", &obs).expect("record must parse");
    assert!(
        !parsed.functions(),
        "…and a `noop` must FAIL the site's FUNCTION leg, or recording it changes nothing"
    );
}
