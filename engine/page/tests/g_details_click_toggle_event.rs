//! **G_DETAILS_CLICK_TOGGLE_EVENT — the summary CLICK is the THIRD entrance to `open`, and it is
//! the one the choke point never saw.**
//!
//! t1400 measured `<details>`' `toggle` against headless Chrome and found it wrong in three ways at
//! once: it must be QUEUED, it must be a real `ToggleEvent` carrying `oldState`/`newState`, and
//! `<details>` fires **no `beforetoggle` at all** (only `[popover]` and `<dialog>` do). It fixed
//! that at the one place both *script* spellings funnel through — the `open` **attribute**, hooked
//! in `dom_bindings::queue_open_toggle`, which covers `el.open = true` and `setAttribute('open','')`
//! together.
//!
//! ⭐⭐⭐ **But the way a human opens a disclosure is neither of those spellings.** A summary click
//! runs the UA's activation behaviour in `Page::dispatch_click`, which flips the attribute on the
//! **Rust** `Dom` directly — it never enters a JS binding, so it never reaches the choke point. It
//! carried its own hand-written pair of dispatches instead, and that second implementation was the
//! pre-t1400 one, frozen in place:
//!
//! ```text
//!   chrome 145.0.7632.116, click <summary id=sb> in a name="faq" group     we, before
//!   ────────────────────────────────────────────────────────────────────   ─────────────────────
//!   beforetoggle on #b / on the auto-closed peer #a       NEITHER          BOTH — spurious
//!   toggle on #b                     ToggleEvent closed>open, trusted      plain Event, no states
//!   toggle on the peer #a            ToggleEvent open>closed,   trusted    plain Event, no states
//!   delivery                         QUEUED (sync read after click = [])   SYNCHRONOUS
//!   order                            the CLICKED panel, then the peer      the peer, then #b
//! ```
//!
//! Every one is silent. A handler reading `e.newState` — the idiomatic way to branch on which way a
//! panel went — read `undefined` on the click path and the correct string on the script path, in
//! the same page, for the same element. That is the shape of the divergence this gate exists to
//! forbid: **when a rule has N entrances, a test of one entrance is evidence about that one only.**
//!
//! `<details>` is the web's script-free "show more": GitHub's folded diffs and review threads, MDN's
//! collapsible sections, and the `<details name>` accordion every docs FAQ is built from.
//!
//! ⭐ **ARM 8 is the one that could not be written from the outside.** The queued-toggle choke point
//! resolves its element through `__nodes`, so "does an untouched `<details>` get its event?" is a real
//! question — and `toggle` does not bubble, so there appears to be no way to listen for one without
//! first resolving the element and thereby answering the question. There is: a **non-bubbling event
//! still runs the CAPTURE phase down to its target**, so `document.addEventListener('toggle', fn,
//! true)` hears a panel nobody holds. Chrome agrees (`CAP:q:closed>open CAP:p:open>closed`). That arm
//! is why the reflector-priming line originally written into `dispatch_click` was DELETED instead of
//! kept: it is inert, and this arm checks the property it was defending.
//!
//! ⚠ THE QUEUED-vs-SYNCHRONOUS row is NOT asserted here and that is deliberate: `dispatch_click`
//! returns only after the drain, so there is no instant a Rust gate can read between the two.
//! `g_toggle_event_details_dialog` owns that arm on the script path. What this gate owns is the
//! click path's event SHAPE, its ABSENT `beforetoggle`, its ORDER, and its reach.
//!
//! ⚠ **Named non-claim, measured:** Chrome queues a `toggle` `closed>open` for a `<details open>` the
//! moment it is INSERTED — from the parser at load, and from an `innerHTML` write — and that pending
//! event COALESCES with a later transition (an `innerHTML`-inserted `<details open>` closed by its
//! group reports `closed>closed`, not `open>closed`). We fire no insertion toggle. Out of scope for
//! t1403; an arm asserting the `innerHTML` case was written, measured against Chrome, found to be
//! asserting OUR value rather than Chrome's, and removed rather than shipped.
//!
//! ⚠ ONE `#[test]` in this binary: two SpiderMonkey contexts in one manuk-page binary tear each
//! other down and the second test reads the first one's empty output.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<details id="a" name="faq" open><summary id="sa">A</summary><p>BODY-A</p></details>
<details id="b" name="faq"><summary id="sb">B</summary><p>BODY-B</p></details>
<details id="c"><summary id="sc">C</summary><p>BODY-C</p></details>
<details id="p" name="grp" open><summary id="sp">P</summary><p>BODY-P</p></details>
<details id="q" name="grp"><summary id="sq">Q</summary><p>BODY-Q</p></details>
<div id="out">-</div>
<script>window.__log = [];
  // ARM 8's listener, and the ONLY route to #p and #q: `toggle` does not bubble, but a non-bubbling
  // event still runs the CAPTURE phase down to its target, so a document-level capture listener hears
  // it — Chrome-measured. Neither #p nor #q is ever resolved from script, which is the point.
  document.addEventListener('toggle', function (e) {
    var id = e.target && e.target.id;
    if (id === 'p' || id === 'q') { window.__log.push('CAP:' + id + ':' + e.oldState + '>' + e.newState); }
  }, true);
  ['a','b','c'].forEach(function (id) {
    var el = document.getElementById(id);
    el.addEventListener('beforetoggle', function () { window.__log.push('BT:' + id); });
    el.addEventListener('toggle', function (e) {
      window.__log.push('T:' + id + ':' + e.oldState + '>' + e.newState
                        + ':' + (e.constructor && e.constructor.name) + ':' + e.isTrusted);
    });
  });
</script>
</body></html>"##;

fn click(page: &mut manuk_page::Page, fonts: &FontContext, sel: &str) {
    let root = page.dom().root();
    let n = manuk_css::query_selector_all(page.dom(), root, sel)[0];
    page.dispatch_click(n, fonts, 800.0);
}

/// Drain the log into `#out` and read it back, then clear it for the next arm.
fn take_log(page: &mut manuk_page::Page) -> String {
    page.eval_for_test(
        "document.getElementById('out').textContent = window.__log.join(' '); \
         window.__log.length = 0;",
    );
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    page.dom().text_content(out)
}

/// One test in the binary — see the module note.
#[test]
fn a_summary_click_queues_the_same_toggle_event_the_script_path_does() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://details-click.test/", &fonts, 800.0);

    // ── ARM 1-4: click #sb. #b opens (closed→open) and, by `name="faq"` exclusivity, #a
    // auto-closes (open→closed). Chrome fires the CLICKED panel's toggle first, the peer's second,
    // and NO `beforetoggle` on either.
    click(&mut page, &fonts, "#sb");
    let open_log = take_log(&mut page);
    println!("DETAILS-CLICK open log: {open_log}");

    // ARM 1 — the clicked panel: a real ToggleEvent with both states and the trusted flag.
    assert!(
        open_log.contains("T:b:closed>open:ToggleEvent:true"),
        "G_DETAILS_CLICK_TOGGLE_EVENT: clicking #sb must fire on #b the SAME event the script path \
         fires — a trusted `ToggleEvent` with oldState `closed` and newState `open` (got \
         {open_log:?}). A handler branching on `e.newState` reads `undefined` from a plain `Event`."
    );
    // ARM 2 — the accordion peer gets its own, with the states of ITS transition.
    assert!(
        open_log.contains("T:a:open>closed:ToggleEvent:true"),
        "G_DETAILS_CLICK_TOGGLE_EVENT: the `<details name>` auto-close of #a must fire a trusted \
         `ToggleEvent` oldState `open` → newState `closed` (got {open_log:?}). A collapsed panel \
         that is never told it collapsed cannot tear its lazy-loaded contents down."
    );
    // ARM 3 — `<details>` has NO `beforetoggle`. Chrome-measured on this exact fixture: neither the
    // clicked panel nor the peer gets one. A spurious cancel-shaped event on an element whose spec
    // has no cancel point tells a component listening for the popover idiom that it has a veto.
    assert!(
        !open_log.contains("BT:"),
        "G_DETAILS_CLICK_TOGGLE_EVENT: `<details>` fires NO `beforetoggle` on ANY path — headless \
         Chrome fires none for the summary click either (got {open_log:?}). Only `[popover]` and \
         `<dialog>` emit one."
    );
    // ARM 4 — ORDER: the panel the user clicked, then the peer the accordion closed.
    let (t_b, t_a) = (open_log.find("T:b:"), open_log.find("T:a:"));
    assert!(
        t_b.is_some() && t_a.is_some() && t_b < t_a,
        "G_DETAILS_CLICK_TOGGLE_EVENT: the CLICKED panel's `toggle` must precede the auto-closed \
         peer's (got {open_log:?}) — the two state changes are queued in the order they happened."
    );

    // ── ARM 5-6: click #sb again. #b closes; nothing else changes, because closing a panel opens
    // no sibling. The states must track REALITY, not the first transition.
    click(&mut page, &fonts, "#sb");
    let close_log = take_log(&mut page);
    println!("DETAILS-CLICK close log: {close_log}");
    assert!(
        close_log.contains("T:b:open>closed:ToggleEvent:true"),
        "G_DETAILS_CLICK_TOGGLE_EVENT: the second click on #sb must report `open` → `closed` \
         (got {close_log:?}); the states are read from the transition, not asserted from the event."
    );
    assert!(
        !close_log.contains("T:a:") && !close_log.contains("BT:"),
        "G_DETAILS_CLICK_TOGGLE_EVENT: CLOSING a panel disturbs no sibling — exclusivity is a rule \
         about the panel that OPENS (got {close_log:?})."
    );

    // ── ARM 7: scoping. #c has no `name`, so it belongs to no group: opening it must leave the
    // named group alone. The control that proves the peer arms above are the group rule and not
    // "every details in the document gets an event".
    click(&mut page, &fonts, "#sc");
    let solo_log = take_log(&mut page);
    println!("DETAILS-CLICK solo log: {solo_log}");
    assert!(
        solo_log.contains("T:c:closed>open:ToggleEvent:true")
            && !solo_log.contains("T:a:")
            && !solo_log.contains("T:b:")
            && !solo_log.contains("BT:"),
        "G_DETAILS_CLICK_TOGGLE_EVENT: an UNNAMED `<details>` is not a group — opening #c must fire \
         its own toggle and nobody else's (got {solo_log:?})."
    );

    // ── ARM 8: THE ELEMENT THE PAGE HAS NEVER TOUCHED. #p and #q are resolved by NOBODY — not by
    // `getElementById`, not by an inline `ontoggle`. The only listener that can hear them is the
    // document-level CAPTURE one registered above, and `toggle` reaches it because a non-bubbling
    // event still runs the capture phase down to its target (Chrome: `CAP:q:closed>open
    // CAP:p:open>closed`). That makes this the arm that pins the reflector guarantee: the choke
    // point resolves its element through `__nodes`, and an untouched `<details>` is not in it.
    click(&mut page, &fonts, "#sq");
    let capture_log = take_log(&mut page);
    println!("DETAILS-CLICK capture log: {capture_log}");
    assert!(
        capture_log.contains("CAP:q:closed>open") && capture_log.contains("CAP:p:open>closed"),
        "G_DETAILS_CLICK_TOGGLE_EVENT: a `<details>` the page never resolves from script still owes \
         its `toggle` to a document-level CAPTURE listener (got {capture_log:?}) — `toggle` does not \
         bubble, but a non-bubbling event still runs the capture phase down to its target. The \
         queued-toggle choke point resolves its element through `__nodes`, so an untouched element \
         must be materialised before it is queued."
    );
}
