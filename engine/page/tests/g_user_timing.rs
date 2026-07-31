//! **G_USER_TIMING — `performance.mark`/`measure` existed as no-ops, `clearMarks` did not exist at
//! all, and the second fact is what kills the page.**
//!
//! Found on `www.trivago.de` in the CrUX corpus sweep, whose row read `render-failed`:
//!
//! ```text
//!   uncaught (reported): performance.clearMarks is not a function
//!   structural: 0.0% (1410 paths, 1410 missing, 0 misplaced)
//! ```
//!
//! **1410 of 1410 elements never rendered** — a top-1000 travel site as a blank document — and the
//! same bundle serves `.be`, `.fr`, `.jp` and `.pl`, so it is *five* corpus origins from one line.
//!
//! ## The shape, which is the reusable part
//!
//! `mark` and `measure` were present and inert. So the bundle's feature-detect
//! (`typeof performance.mark === 'function'`) answered **yes**, it took its instrumented path, and
//! it then reached for the *other half* of the same API — which nobody had stubbed. **A
//! half-installed API is worse than an absent one:** absence routes a caller to its fallback,
//! half-presence routes it into a wall it cannot see coming. (`innerText`, t612, is the same
//! failure with the halves being getter and setter.)
//!
//! Inert was also wrong on its own terms, and this gate asserts that too. `mark('a')` then
//! `measure('m','a')` is what every scheduler does; with `getEntriesByName` hard-coded to `[]` the
//! measure resolved against a mark that "did not exist". A *recorded* mark is the feature — the
//! function merely existing is not.
//!
//! ## Why the errors are asserted
//!
//! `measure(n, 'never-marked')` is a **SyntaxError** in Chrome, and a library's `try/catch` around
//! it is a real code path that decides whether instrumentation is enabled. Returning a plausible
//! measure of duration 0 there would be a wrong answer of the right type — the dominant bug shape
//! in this project, and one no feature-detect can see.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
  <div id="out">-</div>
  <script>
    var R = [];
    var P = performance;

    // ── THE TRIVAGO LINE. Every one of these is a call a real bundle makes, and every one of them
    // is a TypeError if the API is only half-installed. They are checked as a GROUP because that
    // is how the failure arrives: the page does not survive the first one that is missing.
    var missing = [];
    ['now','mark','measure','clearMarks','clearMeasures','getEntries','getEntriesByType',
     'getEntriesByName','clearResourceTimings','setResourceTimingBufferSize','toJSON']
      .forEach(function (n) { if (typeof P[n] !== 'function') { missing.push(n); } });
    R.push('missing:' + (missing.length ? missing.join(',') : 'none'));

    // The exact expression that took the site down, in the position it took it down from.
    var died = false;
    (function () { 'use strict'; try { P.clearMarks('boot'); } catch (e) { died = true; } })();
    R.push('clearThrew:' + died);

    // ── A MARK IS RECORDED, NOT DISCARDED. This is the claim the old no-op failed while passing
    // every `typeof` check that exists.
    P.clearMarks(); P.clearMeasures();
    var m = P.mark('alpha');
    R.push('markType:' + (m && m.entryType));
    R.push('markIsEntry:' + (m instanceof PerformanceMark));
    R.push('markDur:' + (m && m.duration));
    R.push('byName:' + P.getEntriesByName('alpha').length);
    R.push('byType:' + P.getEntriesByType('mark').length);

    // `detail` and an explicit `startTime` are the Level-3 options analytics bundles pass.
    var d = P.mark('withDetail', { startTime: 40, detail: { k: 7 } });
    R.push('markStart:' + d.startTime);
    R.push('markDetail:' + (d.detail && d.detail.k));
    // Absent detail is `null`, not `undefined` — consumers serialise it.
    R.push('markNoDetail:' + (m.detail === null));

    // ── MEASURE RESOLVES AGAINST THE MARK. The whole point of recording them.
    var span = P.measure('span', 'withDetail', 'withDetail');
    R.push('measType:' + span.entryType);
    R.push('measStart:' + span.startTime);
    R.push('measDur:' + span.duration);
    R.push('measIsMeasure:' + (span instanceof PerformanceMeasure));

    // Numeric endpoints, and the options form both frameworks use.
    var n1 = P.measure('num', 10, 25);
    R.push('numDur:' + n1.duration + ':' + n1.startTime);
    var n2 = P.measure('opt', { start: 5, duration: 12 });
    R.push('optDur:' + n2.duration + ':' + n2.startTime);
    var n3 = P.measure('opt2', { end: 30, duration: 10 });
    R.push('opt2:' + n3.startTime + ':' + n3.duration);

    // ── AN UNKNOWN MARK IS A SyntaxError, NOT A ZERO. A try/catch around this is a live code path
    // in real instrumentation, and a silent duration-0 measure would tell it everything is fine.
    var errName = 'none';
    try { P.measure('bad', 'never-marked-at-all'); } catch (e) { errName = e.name; }
    R.push('unknownMark:' + errName);
    // ...and the failed measure must not have been recorded.
    R.push('badRecorded:' + P.getEntriesByName('bad').length);

    // A negative numeric endpoint is a TypeError, likewise per spec.
    var negName = 'none';
    try { P.measure('neg', -5, 10); } catch (e) { negName = e.name; }
    R.push('negative:' + negName);

    // ── `navigationStart` IS NOT A MARK, AND IT MUST NOT THROW. This is trivago's next rung after
    // clearMarks: "time since the navigation started" is the most common measure on the web, and
    // nobody ever calls `mark('navigationStart')` — the legacy PerformanceTiming attribute names
    // resolve ahead of the mark buffer, and this one is 0 by definition because it IS timeOrigin.
    var navErr = 'none', navStart = -1;
    try { navStart = P.measure('sinceNav', 'navigationStart').startTime; }
    catch (e) { navErr = e.name; }
    R.push('navMark:' + navErr + ':' + navStart);
    // A phase the host never observed is an InvalidAccessError — the spec's answer for an empty
    // timing value, and the honest one: a fabricated 0 for responseStart is a confident wrong TTFB.
    var phaseErr = 'none';
    try { P.measure('ttfb', 'responseStart'); } catch (e) { phaseErr = e.name; }
    R.push('unobserved:' + phaseErr);
    P.clearMeasures('sinceNav');

    // ── CLEARING IS SELECTIVE, AND IT ONLY CLEARS ITS OWN KIND. A `clearMarks()` that also ate
    // measures would silently disarm a page's own timeline halfway through.
    R.push('marksBefore:' + P.getEntriesByType('mark').length);
    P.clearMarks('alpha');
    R.push('afterOne:' + P.getEntriesByType('mark').length);
    R.push('measuresIntact:' + P.getEntriesByType('measure').length);
    P.clearMarks();
    R.push('afterAll:' + P.getEntriesByType('mark').length);
    R.push('measuresStill:' + (P.getEntriesByType('measure').length > 0));
    P.clearMeasures();
    R.push('measuresGone:' + P.getEntriesByType('measure').length);

    // ── ORDER IS BY startTime, because consumers zip these against their own timeline and read
    // `[0]` as "the first thing that happened".
    P.mark('late', { startTime: 900 });
    P.mark('early', { startTime: 3 });
    P.mark('mid', { startTime: 100 });
    R.push('order:' + P.getEntriesByType('mark').map(function (e) { return e.name; }).join(','));

    // ── THE NAVIGATION ENTRY STILL WORKS. It is owned by the host, not this buffer, and the
    // previous implementation's one real answer must not have been traded away for the new one.
    var nav = P.getEntriesByType('navigation');
    R.push('navLen:' + nav.length);
    R.push('navType:' + (nav[0] && nav[0].entryType));
    // ...and it appears in the unfiltered list alongside the marks.
    R.push('allHasNav:' + P.getEntries().some(function (e) { return e.entryType === 'navigation'; }));
    R.push('allHasMark:' + P.getEntries().some(function (e) { return e.entryType === 'mark'; }));

    // An unknown entry type is an empty list, not a throw — this is the honest absence.
    R.push('resourceLen:' + P.getEntriesByType('resource').length);

    // ── `toJSON` on an entry, which is how these reach a RUM beacon.
    var j = P.getEntriesByType('mark')[0].toJSON();
    R.push('json:' + (j.name + '/' + j.entryType + '/' + j.duration));

    document.getElementById('out').textContent = R.join(' ');
  </script>
</body></html>"#;

#[test]
fn user_timing_records_marks_and_the_whole_api_is_callable() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://ut.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        // THE headline: the whole surface is callable. `missing:clearMarks` is www.trivago.de's
        // blank page.
        "missing:none",
        "clearThrew:false",
        // a mark is RECORDED — the claim the no-op failed while passing every typeof check
        "markType:mark",
        "markIsEntry:true",
        "markDur:0",
        "byName:1",
        "byType:1",
        "markStart:40",
        "markDetail:7",
        "markNoDetail:true",
        // measure resolves against the recorded mark
        "measType:measure",
        "measStart:40",
        "measDur:0",
        "measIsMeasure:true",
        "numDur:15:10",
        "optDur:12:5",
        "opt2:20:10",
        // the spec's errors, because a try/catch around them is a real code path
        "unknownMark:SyntaxError",
        "badRecorded:0",
        "negative:TypeError",
        // the legacy timing names resolve ahead of the mark buffer — trivago's SECOND rung
        "navMark:none:0",
        "unobserved:InvalidAccessError",
        // clearing is selective and kind-scoped
        "marksBefore:2",
        "afterOne:1",
        "measuresIntact:4",
        "afterAll:0",
        "measuresStill:true",
        "measuresGone:0",
        // sorted by startTime
        "order:early,mid,late",
        // the navigation entry was not traded away for the new buffer
        "navLen:1",
        "navType:navigation",
        "allHasNav:true",
        "allHasMark:true",
        "resourceLen:0",
        "json:early/mark/0",
    ] {
        assert!(
            got.contains(claim),
            "G_USER_TIMING: expected `{claim}`\n  got: {got}\n\n  \
             `performance.clearMarks` did not exist while `mark`/`measure` did, so a bundle's \
             feature-detect said yes and its next line threw: www.trivago.de rendered 0 of 1410 \
             elements — a blank top-1000 page, and the same bundle serves four more corpus \
             origins. A HALF-INSTALLED API IS WORSE THAN AN ABSENT ONE, because absence routes a \
             caller to its fallback and half-presence routes it into a wall. `byName:1` is the \
             second half of the fix: a `mark()` that records nothing passes every `typeof` check \
             ever written and still breaks the measure that follows it."
        );
    }
}
