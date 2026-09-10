//! **G_A_PAGE_REPORTS_ITS_OWN_BOOT_FAILURE — one rule had three implementations and no consumer.**
//!
//! The single most common way a real site fails in this engine is that its boot bundle throws on a
//! missing IDL member and the app never mounts. Until tick 1480 that fact went to **three different
//! places and reached no caller**:
//!
//! ```text
//!   a top-level classic <script> throw   ->  tracing::warn!("a page <script> threw")   (stderr)
//!   a type=module evaluation failure     ->  tracing::warn!("a page module failed")    (stderr)
//!   every DEFERRED throw (setTimeout,    ->  globalThis.__errors — a JS array read by
//!     microtask, listener, on* handler)      exactly one caller, `manuk-wpt diag`
//! ```
//!
//! So the fidelity instrument could report that a site rendered a shell and **never why**, which is
//! precisely the measurement the Phase-0 scorability ceiling needs: 24 of 135 in-scope sites never
//! yield a scored tree, and no instrument in the tree could name the first-failure cause.
//!
//! `Page::boot_errors()` is now that one list, and this gate pins the three properties that make it
//! worth having rather than merely present:
//!
//! 1. **ALL THREE PATHS LAND IN IT.** A gate that exercised only the classic top-level throw would
//!    pass with two thirds of the mechanism missing — and the deferred path is the majority of real
//!    boot failures, because real boot code runs from a `DOMContentLoaded`/`load` handler.
//! 2. **THE PAGE KEEPS RUNNING.** The harvest must not turn a reported error into a fatal one: the
//!    scripts after the throwing one still run and the DOM they build is still there. That is the
//!    invariant `run_one_script`'s own comment was written to protect, and a harvester is exactly
//!    the kind of change that could break it silently.
//! 3. **ORDER IS CAUSAL.** `boot_errors()[0]` is the FIRST thing that went wrong. Everything after
//!    it may be a consequence of the page having already lost its boot, so a histogram that keyed on
//!    the last error — or on an unordered set — would rank symptoms above causes.
//!
//! ⚠ **THE VACUITY ARM IS THE SECOND HALF OF THE GATE.** A clean page must report **zero**. Without
//! it, `boot_errors()` returning `["something"]` unconditionally passes every assertion above, and
//! the histogram this exists to feed would read "100% of the web fails to boot" — the shape t1470
//! caught in the a11y denominator, one instrument over.
//!
//! Mutations that must turn this red:
//!   1. drop `record_script_error` from the classic arm  -> `TypeError` row missing, n=2
//!   2. drop it from the module arm                      -> `module` phase missing, n=2
//!   3. drop `__hostScriptError` from `__reportError`    -> the deferred rows missing, n=1
//!   4. `clear_script_errors()` removed from `load`      -> the CLEAN page inherits this page's
//!                                                          errors and the vacuity arm fires
//!   5. record the watchdog-preemption branch too        -> (not reachable from this fixture; the
//!                                                          separation is asserted by comment at
//!                                                          the call site, which mutation 1 covers)

use manuk_text::FontContext;

/// Three throws, one per path, in a known order — and a `<div>` built by the script that runs
/// AFTER the top-level throw, which is how the "keeps running" arm is observable.
const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8></head><body>
<script>
  // (1) TOP-LEVEL CLASSIC. A missing global — the commonest real boot failure there is.
  __manuk_no_such_global_1480();
</script>
<script>
  // The page is NOT dead. This element is the proof.
  var d = document.createElement('div'); d.id = 'alive'; d.textContent = 'alive';
  document.body.appendChild(d);
  // (2) DEFERRED. A listener that throws — where real boot code actually lives.
  window.addEventListener('load', function () { (void 0).mount(); });
</script>
<script type="module">
  // (3) MODULE. An import the resolve hook cannot satisfy fails at LINK, which is a different
  //     native path from the classic throw and had its own separate warn! line.
  import { nothing } from './there-is-no-such-module-1480.js';
  nothing();
</script>
</body></html>"##;

/// A page with the same shape and no throws. Loaded SECOND, on the same thread, so it also proves
/// the per-document reset: without it this page inherits the fixture's three errors.
const CLEAN: &str = r##"<!doctype html><html><head><meta charset=utf-8></head><body>
<script>
  var d = document.createElement('div'); d.id = 'alive'; d.textContent = 'alive';
  document.body.appendChild(d);
  window.addEventListener('load', function () { d.textContent = 'mounted'; });
</script>
</body></html>"##;

/// ⚠⚠ **A SCRIPT-FREE DOCUMENT, AND IT IS THE ARM A TWO-PAGE GATE COULD NOT HAVE.**
///
/// `manuk-page` declines to build a JS context for a document with no `<script>` and no inline
/// handler, so `PageContext::load` — which was the only place clearing the harvest — never runs for
/// one. On the first real 40-site sweep this reported `marktplaats.nl`'s `TypeError: Invalid URL` as
/// the boot failure of two sites that run no script at all.
///
/// Both fixtures above have scripts, so both reached the clear and both passed. *The fixture is part
/// of the instrument* (t1424): a gate cannot see a hole its own inputs all avoid.
const SCRIPTLESS: &str = r##"<!doctype html><html><head><meta charset=utf-8></head><body><p id="alive">alive</p></body></html>"##;

#[test]
fn a_page_reports_every_uncaught_error_its_own_script_produced() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Runtime::new().unwrap();

    let (broken, clean, scriptless) = rt.block_on(async {
        let mut b =
            manuk_page::Page::load_async(HTML, "https://boot.test/broken", &fonts, 800.0).await;
        b.finish_loading(&fonts, 800.0).await;
        // SAME THREAD, SECOND DOCUMENT — mutation 4's arm.
        // SECOND, still the same thread, and with NO script at all. **The order is the arm.** A
        // clean SCRIPTED page in this slot would clear the harvest on its own way through
        // `PageContext::load` and leave nothing for the script-free page to inherit — which is
        // exactly how the first version of this gate passed while the sweep was misattributing.
        let mut n =
            manuk_page::Page::load_async(SCRIPTLESS, "https://boot.test/scriptless", &fonts, 800.0)
                .await;
        n.finish_loading(&fonts, 800.0).await;
        // THIRD.
        let mut c =
            manuk_page::Page::load_async(CLEAN, "https://boot.test/clean", &fonts, 800.0).await;
        c.finish_loading(&fonts, 800.0).await;
        (b, c, n)
    });

    let errs = broken.boot_errors();
    let report: Vec<String> = errs
        .iter()
        .map(|e| {
            format!(
                "{}/{}",
                e.phase,
                manuk_wpt_boot_class(e.message.lines().next().unwrap_or(""))
            )
        })
        .collect();
    println!("BOOT ERRORS: {report:?}");

    // ── (2) THE PAGE KEPT RUNNING. Asserted FIRST: if the throw killed the document, every
    //    assertion below is about a page that does not exist.
    let root = broken.dom().root();
    let alive = manuk_css::query_selector_all(broken.dom(), root, "#alive");
    assert_eq!(
        alive.len(),
        1,
        "a reported error must not become a fatal one — the script after the throwing one still \
         runs, and its <div> is how we know"
    );

    // ── (1) ALL THREE PATHS.
    let phases: Vec<&str> = errs.iter().map(|e| e.phase).collect();
    assert!(
        phases.contains(&"script"),
        "the TOP-LEVEL classic throw is missing from the harvest: {report:?}"
    );
    assert!(
        phases.contains(&"module"),
        "the MODULE link failure is missing from the harvest — it is a separate native path with \
         its own warn! line, and a gate that only covered the classic arm would pass without it: \
         {report:?}"
    );
    assert!(
        phases.contains(&"deferred"),
        "the DEFERRED throw (a load listener) is missing — this is where real boot code lives, so \
         it is the majority of what this list exists to see: {report:?}"
    );

    // ── (3) ORDER IS CAUSAL. The top-level throw happened before the listener could be registered,
    //    let alone fired, so it must be first. A histogram keyed on a later row ranks a symptom.
    assert_eq!(
        errs[0].phase, "script",
        "boot_errors()[0] must be the FIRST thing that went wrong — everything after it may be a \
         consequence of the page having already lost its boot: {report:?}"
    );
    assert!(
        errs[0].message.contains("__manuk_no_such_global_1480"),
        "the first error must name the symbol that was missing, not merely its type — a message \
         without the symbol cannot be histogrammed: {:?}",
        errs[0].message
    );

    // ── THE VACUITY ARM. Without this, `boot_errors()` returning a constant passes everything above.
    assert!(
        clean.boot_errors().is_empty(),
        "a page that throws NOTHING must report NOTHING. A non-empty list here is either a \
         fabricated error or the previous document's, and either way the histogram this feeds \
         would read '100% of the web fails to boot': {:?}",
        clean
            .boot_errors()
            .iter()
            .map(|e| format!("{}/{}", e.phase, e.message))
            .collect::<Vec<_>>()
    );
    // …and the clean page's own script ran, or "no errors" is the truth about nothing.
    let croot = clean.dom().root();
    let cnode = manuk_css::query_selector_all(clean.dom(), croot, "#alive");
    assert_eq!(
        cnode.len(),
        1,
        "VACUOUS CONTROL: the clean page's script never ran, so its empty error list proves nothing"
    );
    assert_eq!(
        clean.dom().text_content(cnode[0]),
        "mounted",
        "VACUOUS CONTROL: the clean page's LOAD listener never ran, so the deferred path was not \
         actually exercised on the clean side"
    );

    // ── THE SCRIPT-FREE ARM. A page that runs no script cannot have a boot failure, and the ONLY
    //    way it can report one is by inheriting somebody else's.
    assert!(
        scriptless.boot_errors().is_empty(),
        "a document with NO SCRIPT reported {} boot error(s) — it has no script to throw, so every \
         one of them belongs to a previous page on this thread. This is the arm the two scripted \
         fixtures above cannot provide, because both of them reach the context that does the \
         clearing: {:?}",
        scriptless.boot_errors().len(),
        scriptless
            .boot_errors()
            .iter()
            .map(|e| format!("{}/{}", e.phase, e.message.lines().next().unwrap_or("")))
            .collect::<Vec<_>>()
    );
    let sroot = scriptless.dom().root();
    assert_eq!(
        manuk_css::query_selector_all(scriptless.dom(), sroot, "#alive").len(),
        1,
        "VACUOUS CONTROL: the script-free page did not parse, so its empty error list is a \
         statement about nothing"
    );
}

/// The classifier lives in `manuk-wpt` (the instrument that histograms), which `manuk-page` must not
/// depend on. This is the same collapse, spelled locally, for the printed diagnostic only — no
/// assertion reads it, so the two cannot silently drift into disagreeing about a verdict.
fn manuk_wpt_boot_class(first_line: &str) -> String {
    let head = first_line.split(" at ").next().unwrap_or(first_line).trim();
    head.chars().take(48).collect()
}
