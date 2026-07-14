//! **G_MUTATION — `MutationObserver` observed nothing, and said `function` the whole time.**
//!
//! `new MutationObserver(cb)` constructed. `observe()` returned. `takeRecords()` returned `[]`. The
//! callback **never fired.** And `typeof MutationObserver === 'function'` was `true` throughout — which is
//! exactly why the stub survived: *a check that only asks whether a name exists is satisfied by a stub.*
//!
//! **A stub is worse than an absence here.** The library feature-detects, finds it, registers, and then
//! silently never reacts. Vue, Alpine and lit use it to notice DOM changes they did not make; every
//! analytics and consent script uses it to see injected content.
//!
//! **Delivery is a microtask, and that is not a detail.** A loop that appends 100 nodes must produce
//! **one** callback with 100 records, not 100 callbacks. Deliver synchronously and every observer on the
//! page runs 100× per frame — a performance collapse, not a conformance bug. The `batched:` assertion is
//! that.
//!
//! **Stated limit:** only mutations made by a *script* are observed — they are caught by wrapping the DOM
//! prototypes' mutating methods. Engine-internal edits (the parser, the deferred-script pass) do not go
//! through those wrappers. That is mostly correct behaviour (an observer registered after parsing should
//! not see the parse), but it is a limit, and it is written here rather than discovered later.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div><div id="e" class="old"></div>
<script>
  var R = [];
  var e = document.getElementById('e');

  // ── Records are queued and readable synchronously via takeRecords().
  var mo = new MutationObserver(function () {});
  mo.observe(e, { attributes: true, childList: true, attributeOldValue: true });

  e.setAttribute('class', 'new');
  e.appendChild(document.createElement('i'));

  var recs = mo.takeRecords();
  R.push('n:' + recs.length);
  R.push('t0:' + recs[0].type + ',' + recs[0].attributeName + ',' + recs[0].oldValue);
  R.push('t1:' + recs[1].type + ',' + recs[1].addedNodes.length);
  R.push('drained:' + mo.takeRecords().length);

  // ── `oldValue` is only supplied when the registration ASKED for it.
  var mo2 = new MutationObserver(function () {});
  mo2.observe(e, { attributes: true });           // no attributeOldValue
  e.setAttribute('class', 'newer');
  R.push('noOld:' + mo2.takeRecords()[0].oldValue);

  // ── attributeFilter
  var mo3 = new MutationObserver(function () {});
  mo3.observe(e, { attributes: true, attributeFilter: ['data-x'] });
  e.setAttribute('class', 'ignored');
  e.setAttribute('data-x', 'seen');
  var f = mo3.takeRecords();
  R.push('filter:' + f.length + ',' + f[0].attributeName);

  // ── subtree
  var child = e.firstChild;
  var mo4 = new MutationObserver(function () {});
  mo4.observe(e, { attributes: true, subtree: true });
  child.setAttribute('id', 'deep');
  R.push('subtree:' + mo4.takeRecords().length);

  // ── disconnect
  mo4.disconnect();
  child.setAttribute('id', 'deeper');
  R.push('disconnected:' + mo4.takeRecords().length);

  // ── DELIVERY IS A MICROTASK, AND IT BATCHES. 100 appends must give ONE callback with 100 records.
  var calls = 0, total = 0;
  var mo5 = new MutationObserver(function (rs) { calls++; total += rs.length; });
  mo5.observe(e, { childList: true });
  for (var i = 0; i < 100; i++) { e.appendChild(document.createElement('b')); }

  globalThis.__report = function () {
    R.push('batched:' + calls + ',' + total);
    document.getElementById('out').textContent = R.join(' ');
  };
</script></body></html>"##;

#[test]
fn mutation_observer_records_batches_filters_and_disconnects() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(HTML, "https://mo.test/", &fonts, 800.0);
    page.eval_for_test("globalThis.__report && __report()");

    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("n:2", "the observer recorded NOTHING — it was an inert stub that said `function`"),
        ("t0:attributes,class,old", "an attribute record carries its name and (when asked) its old value"),
        ("t1:childList,1", "a childList record carries the added nodes"),
        // 0, not 1: nothing was mutated between the two calls, so the second drains an empty queue. The
        // engine was right and the expectation was mine.
        ("drained:0", "takeRecords() DRAINS — a second call with no new mutations returns nothing"),
        (
            "noOld:null",
            "`oldValue` is supplied ONLY when the registration asked for it. Handing it over unasked is a \
             conformance failure that looks like generosity",
        ),
        ("filter:1,data-x", "attributeFilter must actually filter"),
        ("subtree:1", "subtree must reach descendants"),
        ("disconnected:0", "and disconnect must stop it"),
        (
            "batched:1,100",
            "**DELIVERY IS A MICROTASK, AND IT BATCHES.** 100 appends give ONE callback with 100 records. \
             Deliver synchronously and every observer on the page runs 100× per frame — a performance \
             collapse, not a conformance bug",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_MUTATION: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
