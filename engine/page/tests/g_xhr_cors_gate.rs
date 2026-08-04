//! **G_XHR_CORS_GATE — `'withCredentials' in xhr` is jQuery's ENTIRE cross-origin capability, and it
//! answered NO.**
//!
//! ⚠⚠⚠ **ONE ABSENT BOOLEAN KILLS EVERY CROSS-ORIGIN `$.ajax` ON EVERY jQUERY PAGE.** jQuery 3.7.1
//! decides once, at load, whether it can do cross-origin AJAX at all — and it decides it by asking an
//! `XMLHttpRequest` whether it has one property. Verbatim from `beb88run.xyz`'s own `desktop-js`
//! bundle:
//!
//! ```js
//!   le.cors = "withCredentials" in Qt;              // Qt = new XMLHttpRequest()
//!   ce.ajaxTransport(function (i) {
//!     if (le.cors || Qt && !i.crossDomain) return { send: …, abort: … };
//!   });                                             // …otherwise: l(-1, "No Transport")
//! ```
//!
//! `done(-1, "No Transport")` sets `jqXHR.readyState = 0` and rejects the request. The chain, read out
//! of one real page over three ticks:
//!
//! ```text
//!   'withCredentials' in xhr  ==  false        (Chrome: true)
//!     -> jQuery support.cors = false
//!     -> no transport for ANY cross-domain $.ajax
//!     -> done(-1, "No Transport") -> jqXHR rejects at readyState 0
//!     -> `await $.ajax(…)` throws inside the 4s progressive-jackpot poll
//!     -> SIXTEEN unhandled rejections per navigation, and no cross-origin data ever arrives
//! ```
//!
//! t891 got the sixteen rejections named (they printed as `[object Object]`); t894 identified them as
//! jQuery `jqXHR`s at `readyState: 0` and **refuted** the load-budget explanation with a 60s control;
//! this gate is the mechanism underneath both. The rest of jQuery's detection was already correct on
//! this engine — `new XMLHttpRequest()` succeeds, `<a>.protocol`/`.host` resolve, and jQuery's
//! `crossDomain` computation returns the right answer for relative, same-origin-absolute,
//! protocol-relative and foreign URLs — which is exactly why the single missing member survived.
//!
//! **Every expectation below is Chrome's, captured from a real `google-chrome --headless --dump-dom`
//! run of this fixture**, not recalled: the constants' descriptor shape
//! (`{writable:false, enumerable:true, configurable:false}`), `withCredentials` defaulting to `false`,
//! its WebIDL boolean coercion (`'yes'` → `true`), and that Chrome accepts the set in UNSENT and
//! OPENED but throws `InvalidStateError` once the send() flag is set.
//!
//! **Proven RED**: delete the `withCredentials` accessor and `jq-support.cors` reads `false` and
//! `jq-transport-for-crossdomain` reads `MISSING` — the two claims with teeth. Delete the constants
//! block and `done-idiom` reads `false`, which is the branch every hand-rolled XHR wrapper's
//! completion handler lives in.
//!
//! ⚠ **The last claim is a GUARD, not a capability.** t892 removed this engine's private slots from a
//! page's view of an XHR (`JSON.stringify(xhr)` must not advertise our internals). A new member added
//! as an own enumerable data property would silently undo that, so the serialisation is asserted here
//! too — the accessor lives on the prototype precisely so it cannot.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<script>
  var R = [];
  var p = function (k, v) { R.push(k + '=' + v); };

  // ── jQuery 3.7.1's support detection, verbatim in shape.
  var Qt = null;
  try { Qt = new window.XMLHttpRequest(); } catch (e) {}
  var support = { ajax: !!Qt, cors: !!Qt && ('withCredentials' in Qt) };
  p('jq-support.ajax', support.ajax);
  p('jq-support.cors', support.cors);

  // ── THE FAILING SELECTION ITSELF, transcribed from the shipped bundle. This is the claim with
  //    teeth: every property below could be individually right and this one still wrong.
  var pickTransport = function (opts) {
    if (support.cors || (Qt && !opts.crossDomain)) { return 'TRANSPORT'; }
    return 'MISSING';                                  // == done(-1, "No Transport")
  };
  p('jq-transport-for-crossdomain', pickTransport({ crossDomain: true }));
  p('jq-transport-for-samedomain',  pickTransport({ crossDomain: false }));

  // ── The property itself, against Chrome's answers.
  var x = new XMLHttpRequest();
  p('in-instance',  'withCredentials' in x);
  p('in-prototype', 'withCredentials' in XMLHttpRequest.prototype);
  p('default',      x.withCredentials);
  p('typeof',       typeof x.withCredentials);
  x.withCredentials = 'yes';                           // WebIDL boolean coercion
  p('coerced',      x.withCredentials);
  p('set-unsent',   x.withCredentials === true);
  x.open('GET', '/api/thing');
  x.withCredentials = false;
  p('set-opened',   x.withCredentials);

  // The send() flag closes the window — and `open()` on a reused object must re-open it, which is
  // what every connection-pooling wrapper depends on.
  var y = new XMLHttpRequest();
  y.open('GET', '/api/thing');
  y.send();
  try { y.withCredentials = true; p('set-after-send', 'NO-THROW'); }
  catch (e) { p('set-after-send', e.name); }
  y.abort();
  try { y.withCredentials = true; p('set-after-abort', y.withCredentials); }
  catch (e) { p('set-after-abort', 'THREW:' + e.name); }

  // ── The readyState constants. `xhr.readyState === XMLHttpRequest.DONE` is THE completion idiom,
  //    and `4 === undefined` is false silently, forever.
  p('ctor.DONE',   XMLHttpRequest.DONE);
  p('ctor.UNSENT', XMLHttpRequest.UNSENT);
  p('proto.DONE',  XMLHttpRequest.prototype.DONE);
  p('ctorKeys',    Object.keys(XMLHttpRequest).join(','));
  // Guarded: an ABSENT constant makes the descriptor `undefined`, and reading `.value` off it would
  // throw and take the whole probe down — so a regression in ONE claim would be reported as all
  // twenty-two missing at once, which is a gate that cannot say what broke.
  var d = Object.getOwnPropertyDescriptor(XMLHttpRequest, 'DONE');
  p('descDONE', d ? (d.value + '|' + d.writable + '|' + d.enumerable + '|' + d.configurable) : 'ABSENT');
  var z = new XMLHttpRequest();
  z.open('GET', '/api/thing');
  z.send();
  z.readyState = 4;                                    // stand in for a delivered response
  p('done-idiom', z.readyState === XMLHttpRequest.DONE);

  // ── GUARD (t892): a page's view of an XHR must not gain a new own enumerable field.
  var s = JSON.stringify(new XMLHttpRequest());
  p('serialised-wc', s.indexOf('withCredentials') >= 0);
  p('serialised-slots', s.indexOf('"_wc"') >= 0 || s.indexOf('"_sent"') >= 0);

  document.getElementById('out').textContent = R.join(' ');
</script>
</body></html>"##;

#[test]
fn jquery_cross_origin_capability_is_admitted_by_the_xhr_interface() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://beb88.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("XHR CORS GATE: {got}");

    for (claim, why) in [
        // ── The two with teeth: jQuery's own expressions, on jQuery's own values.
        (
            "jq-support.cors=true",
            "THE GATE: `support.cors = 'withCredentials' in new XMLHttpRequest()`. False here is \
             every cross-origin $.ajax on every jQuery page in the corpus",
        ),
        (
            "jq-transport-for-crossdomain=TRANSPORT",
            "THE FAILING CALL: with support.cors false jQuery returns NO transport for a \
             cross-domain request and calls done(-1, 'No Transport') — a jqXHR rejecting at \
             readyState 0, which is what beb88run.xyz produced sixteen times per navigation",
        ),
        (
            "jq-transport-for-samedomain=TRANSPORT",
            "the same-origin half was always fine, and must stay fine — a fix that only moved the \
             failure would still read green on the line above",
        ),
        ("jq-support.ajax=true", "the constructor itself; a regression here fails everything"),
        // ── The property, against Chrome's measured answers.
        ("in-instance=true", "jQuery asks the INSTANCE, not the prototype"),
        (
            "in-prototype=true",
            "…and Chrome answers yes on both, because it is a prototype accessor — an own data \
             property on the instance would pass the line above and still be the wrong shape",
        ),
        ("default=false", "Chrome's default is false, not undefined and not true"),
        ("typeof=boolean", "a boolean, so `if (xhr.withCredentials)` reads what the page wrote"),
        ("coerced=true", "WebIDL boolean coercion: the string 'yes' becomes true, not 'yes'"),
        ("set-unsent=true", "settable in UNSENT — jQuery's xhrFields assignment happens before open()"),
        ("set-opened=false", "…and in OPENED, which is where the transport actually assigns it"),
        (
            "set-after-send=InvalidStateError",
            "Chrome throws once the send() flag is set. A property that accepts a write it cannot \
             honour is the failure mode this whole interface was fixed for",
        ),
        (
            "set-after-abort=true",
            "abort() unsets the send() flag, so a REUSED XMLHttpRequest accepts it again — a \
             one-way latch would break every request-pooling wrapper on the second request",
        ),
        // ── The constants.
        (
            "done-idiom=true",
            "`xhr.readyState === XMLHttpRequest.DONE` — the completion branch of every hand-rolled \
             XHR wrapper. Against an absent constant it is `4 === undefined`: false, silently",
        ),
        ("ctor.DONE=4", "on the INTERFACE OBJECT, which is where the idiom above reads it"),
        ("ctor.UNSENT=0", "a second constant, so a hard-coded DONE fails here"),
        ("proto.DONE=4", "Chrome puts them on the prototype too — `xhr.DONE` is also valid"),
        (
            "ctorKeys=UNSENT,OPENED,HEADERS_RECEIVED,LOADING,DONE",
            "all five, in WebIDL order — a partial set is the half-installed shape that routes a \
             caller into a wall instead of a fallback",
        ),
        (
            "descDONE=4|false|true|false",
            "Chrome's exact descriptor, read off getOwnPropertyDescriptor: WebIDL constants are \
             enumerable and non-writable, so a `for…in` over the interface object sees them",
        ),
        // ── The guard (t892).
        (
            "serialised-wc=false",
            "GUARD: `JSON.stringify(xhr)` must not gain a `withCredentials` field. Chrome's is `{}` \
             — the accessor is on the prototype exactly so a new member cannot undo t892",
        ),
        (
            "serialised-slots=false",
            "GUARD: and the two new internal slots (`_wc`, `_sent`) stay out of the page's view for \
             the same reason every other slot does",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_XHR_CORS_GATE: missing `{claim}`{}\n  got: {got}",
            if why.is_empty() { String::new() } else { format!("\n  — {why}") }
        );
    }
}
