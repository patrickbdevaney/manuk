//! **G_A_SCRIPT_NAVIGATION_IS_A_NAVIGATION — the four spellings of the web's most common redirect,
//! none of which did anything.**
//!
//! `location.href = "/x"` is how a legacy portal, a login gate, a locale splash and half the domain
//! moves on the web send a user somewhere else. In this engine the BOM shim built `window.location`
//! as a **plain object**, so:
//!
//! ```text
//!   location.href = "/dest"        wrote a property. Nothing happened.
//!   location.assign("/dest")       reached `__applyUrl`, which rewrites the URL for a SINGLE-PAGE
//!                                  APP and never touches the network. Nothing happened.
//!   location.replace("/dest")      the same. Nothing happened.
//!   window.location = "/dest"      REPLACED THE LOCATION OBJECT WITH A STRING — measured,
//!                                  `typeof location` became `"string"`, and every later
//!                                  `location.pathname` on the page read `undefined`.
//! ```
//!
//! Four spellings, one missing capability, and **not one of them threw**: the page sat on the
//! document it was trying to leave, looking exactly like a page that had decided to stay.
//! `house.udn.com` in the 200-site CrUX trend corpus is 195 bytes of precisely this and nothing
//! else — the user typed the address and got a blank page.
//!
//! ## ⚠⚠⚠ THE CONTROL ARM IS THE POINT OF THIS GATE
//!
//! `history.pushState` / `replaceState` change the URL and go **nowhere**, and they share
//! `__applyUrl` with the redirect path. A fix that made the shared function report a navigation
//! would turn every SPA route change into a full network re-fetch — a far worse regression than the
//! bug, and completely invisible in a test that only checks that redirects now work. The
//! `pushState` and `replaceState` rows below are what forbid it.
//!
//! ⚠ **A FRAGMENT IS NOT A NAVIGATION EITHER.** `location.href = "#top"` is a same-document
//! navigation in every browser; reporting it would send the host back to the network for the
//! document it is already displaying, and on a page that does it in a scroll handler, forever.
//!
//! ⚠ **AND THE HOST PERFORMS IT, NOT THE PAGE** — the same contract as `Page::meta_refresh`.
//!
//! Mutations that must turn this red:
//!   1. make `href` a data property again          -> `hrefSet` reports none
//!   2. leave `location` a data property on the global -> `bareSet` reports none
//!   3. point `assign`/`replace` back at `__applyUrl`  -> `assign`/`replace` report none
//!   4. report the navigation from `__applyUrl`     -> `pushState`/`replaceState` start navigating
//!   5. drop the fragment refusal                   -> `hashOnly` navigates
//!   6. report the raw string instead of resolving  -> every row loses its origin
//!
//! ⚠ **AND ONE MUTATION CAME BACK GREEN, WHICH IS WHY THIS GATE IS WORTH ITS WALL TIME.** The
//! refusal read `next == self.final_url || strip_fragment(&next) == strip_fragment(&..)`, and
//! deleting the FIRST clause changed nothing: equal strings have equal fragment-stripped prefixes,
//! so the second comparison had always answered every case the first one did. An inert guard,
//! deleted — the fourth instance of t1403's rule in this arc, and the second in three ticks.

use manuk_text::FontContext;

fn nav_of(script: &str, fonts: &FontContext, rt: &tokio::runtime::Runtime) -> String {
    let html = format!(
        "<!doctype html><html><head><meta charset=utf-8></head><body>stub\
         <script>{script}</script></body></html>"
    );
    let page = rt.block_on(async {
        manuk_page::Page::load_async(&html, "https://stub.test/a/b.html", fonts, 800.0).await
    });
    page.take_script_navigation()
        .unwrap_or_else(|| "none".to_string())
}

#[test]
fn a_script_navigation_is_reported_to_the_host_and_a_spa_route_change_is_not() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Runtime::new().unwrap();

    let rows: Vec<(&str, &str, &str)> = vec![
        // ── THE FOUR SPELLINGS.
        (
            "hrefSet",
            r#"location.href = "/dest";"#,
            "https://stub.test/dest",
        ),
        (
            "bareSet",
            r#"window.location = "dest.html";"#,
            "https://stub.test/a/dest.html",
        ),
        (
            "assign",
            r#"location.assign("?q=1");"#,
            "https://stub.test/a/b.html?q=1",
        ),
        (
            "replace",
            r#"location.replace("https://other.test/x");"#,
            "https://other.test/x",
        ),
        (
            "docLoc",
            r#"document.location = "/d";"#,
            "https://stub.test/d",
        ),
        // ── THE CONTROL ARM. `__applyUrl` is shared; a navigation must NOT be.
        (
            "pushState",
            r#"history.pushState({}, "", "/spa/route");"#,
            "none",
        ),
        (
            "replaceState",
            r#"history.replaceState({}, "", "/spa/other");"#,
            "none",
        ),
        // ── A fragment is a SAME-DOCUMENT navigation.
        ("hashOnly", r##"location.href = "#top";"##, "none"),
        // ── The document it is already in.
        ("selfTarget", r#"location.href = "/a/b.html";"#, "none"),
        // ── A script that does not navigate.
        ("noNav", r#"var x = 1;"#, "none"),
        // ── The LAST one wins: a script that assigns twice is going to the second place.
        (
            "lastWins",
            r#"location.href = "/one"; location.href = "/two";"#,
            "https://stub.test/two",
        ),
    ];

    let mut got = Vec::new();
    for (name, script, _) in &rows {
        got.push(format!("{name}={}", nav_of(script, &fonts, &rt)));
    }
    let line = got.join(" ");
    println!("SCRIPT NAV: {line}");

    // ── VACUITY. Every "none" row is satisfied by an engine that reports NOTHING, which is exactly
    //    the engine this gate was written against. The ordinary spelling must be found first.
    assert!(
        line.contains("hrefSet=https://stub.test/dest"),
        "VACUOUS: `location.href = \"/dest\"` reported no navigation, so every `none` row below is \
         passing on the very bug this gate exists to catch — {line}"
    );

    let want: Vec<String> = rows.iter().map(|(n, _, w)| format!("{n}={w}")).collect();
    assert_eq!(
        line,
        want.join(" "),
        "\n  a script navigation must be reported to the host, and an SPA route change must not\n  \
         want: {}\n  got:  {line}",
        want.join(" ")
    );
}
