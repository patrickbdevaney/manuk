//! **G_FORM — the browser must be writable.**
//!
//! Forms are **50% of the oracle corpus** (`docs/loop/CAPABILITIES.md`, measured over 237 real sites),
//! and they are the difference between a *reader* and a *browser*: without them you cannot search, log
//! in, or buy anything.
//!
//! The load-bearing assertion here is (3), and it is not about forms at all — it is about **not doing
//! the thing the author cancelled**. A form on a React/Vue/Svelte page is not submitted by the browser:
//! the page listens for `submit`, calls `preventDefault()`, and does its own `fetch`. With no `submit`
//! event ever dispatched, that handler never ran — so we performed the **full GET navigation the author
//! had explicitly cancelled**, throwing away the page and everything the user had typed. From the user's
//! side the site "reloads itself" whenever anyone presses a button, and nothing in any log says why.
//!
//! The rest is the serialization details that servers actually branch on, and that nobody would guess:
//!
//!   - A checked checkbox with **no `value`** submits the string **`"on"`** — not `""`. "The box was
//!     ticked" arriving as an empty string reads at the far end as "ticked, and the user typed nothing".
//!     Those are different claims.
//!   - An **unchecked** box is not a successful control at all: it contributes *nothing*, which is again
//!     different from contributing an empty string.
//!   - `application/x-www-form-urlencoded` encodes a space as **`+`**, not `%20`. `encodeURIComponent`
//!     alone gets this wrong — quietly, and only for values containing spaces, which is the worst
//!     possible distribution for a bug.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
    <form id="f" action="/search" method="get">
      <input name="q" value="rust">
      <input type="checkbox" name="safe" checked>
      <input type="checkbox" name="off">
      <input type="submit" value="Go">
    </form>
    <div id="out">-</div>
    <div id="fired">no</div>
    <script>
      var r = [];
      var f = document.getElementById('f');
      var fd = new FormData(f);
      r.push('q:' + (fd.get('q') === 'rust'));
      // A CHECKED box with no `value` is "on"; an UNCHECKED box is absent entirely.
      r.push('checked_on:' + (fd.get('safe') === 'on'));
      r.push('unchecked_absent:' + (fd.get('off') === null));
      // form-urlencoded: a space is `+`.
      r.push('urlencode_plus:' + (new URLSearchParams({a: 'x y'}).toString() === 'a=x+y'));
      r.push('formdata_serializes:' + (fd.toString().indexOf('q=rust') >= 0));
      // The interception EVERY modern form performs.
      f.addEventListener('submit', function (e) {
        e.preventDefault();
        document.getElementById('fired').textContent = 'yes';
      });
      r.push('submit_is_fn:' + (typeof f.submit === 'function'));
      r.push('requestSubmit_is_fn:' + (typeof f.requestSubmit === 'function'));
      document.getElementById('out').textContent = r.join(' ');
    </script>
  </body></html>"#;

fn text(page: &manuk_page::Page, sel: &str) -> String {
    let root = page.dom().root();
    let hits = manuk_css::query_selector_all(page.dom(), root, sel);
    assert!(!hits.is_empty(), "{sel} must exist");
    page.dom().text_content(hits[0])
}

/// One test, on purpose — two SpiderMonkey contexts in one binary tear down messily and segfault
/// nondeterministically, and a flaky gate gets ignored. (See `g_defer`.)
#[test]
fn forms_serialize_correctly_and_submit_is_cancellable() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://form.test/", &fonts, 800.0);

    // (1)+(2) Serialization — the details a server branches on.
    let got = text(&page, "#out");
    for claim in [
        "q:true",                // a plain named control
        "checked_on:true",       // a checked box with no `value` is "on", not ""
        "unchecked_absent:true", // an unchecked box is not a successful control at all
        "urlencode_plus:true",   // form-urlencoded: a space is `+`, not %20
        "formdata_serializes:true",
        "submit_is_fn:true",
        "requestSubmit_is_fn:true",
    ] {
        assert!(
            got.contains(claim),
            "G_FORM: expected {claim} in {got:?}\n  \
             Forms are 50% of the corpus. These are the details servers actually branch on."
        );
    }

    // (3) **THE one.** A `submit` event fires, and `preventDefault()` is honoured — so the browser does
    //     NOT navigate. Without this, every AJAX form on the web performs the full page navigation its
    //     author explicitly cancelled, and the user loses what they typed while nothing says why.
    let root = page.dom().root();
    let form = manuk_css::query_selector_all(page.dom(), root, "#f")[0];
    let proceed = page.dispatch_submit(form, &fonts, 800.0);

    assert_eq!(
        text(&page, "#fired"),
        "yes",
        "G_FORM: the page's `submit` listener never ran. A form on any modern framework is submitted by \
         the PAGE, not the browser — with no event, its handler is dead code."
    );
    assert!(
        !proceed,
        "G_FORM: the page called preventDefault() and the browser is going to navigate ANYWAY.\n  \
         This throws away the page and everything the user typed, and does it for the majority of forms \
         on the web — every one that submits over fetch. From the user's side the site 'reloads itself' \
         whenever they press a button."
    );
}
