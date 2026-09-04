//! **G_FORCED_REFLOW_BUDGET — one task forced a thousand full re-layouts and nothing could stop it.**
//!
//! `timeout-150s` was the largest engine-owned bucket in the t1406 corpus sweep (9 of 200 sites, and
//! an unscored site is a ZERO — the M1 *ceiling* rather than a point on it). t1408 measured
//! `morikoshi.net`, one of the nine:
//!
//! ```text
//!   load phase "cascade+layout+blocking scripts"   191,235 ms      (every other phase: seconds)
//!   event loop hit its TIME budget:  count=1  elapsed_ms=125235  budget_ms=5000
//!                                    reflow_n=1054  reflow_ms=124477    ← 99.4% of it
//!   89 x SLOW FORCED REFLOW  ~2,600 ms each   cascade_ms ~550  layout_ms ~2,000   nodes=4375
//! ```
//!
//! ⭐⭐⭐ **ONE TASK RAN 125 SECONDS AGAINST A 5-SECOND BUDGET, AND BOTH GUARDS WERE LOOKING
//! ELSEWHERE.** The drain reads its clock on a TASK BOUNDARY, and a single task that forces a
//! thousand reflows never reaches one. The `ScriptDeadline` watchdog interrupts JS — and this time is
//! spent in **Rust**, inside the native geometry read, where an interrupt callback cannot fire. A
//! budget that can only be checked where the overrun is not is not a budget.
//!
//! The fix gives the forced reflow the drain's OWN number (`manuk_js::drain_budget_ms`, shared rather
//! than re-invented) as a cumulative per-script-round budget. Past it, a geometry read is answered
//! from the layout already published. Measured on the same site:
//!
//! ```text
//!   phase "cascade+layout+blocking scripts"   191,235 ms  ->  11,185 ms      (17x)
//!   elapsed at the load event                 210,019 ms  ->  27,794 ms
//!   reflow_ms inside the offending task        124,477    ->   5,805
//!   the run                                   TIMED OUT   ->  COMPLETED
//! ```
//!
//! ⚠ **This is not performance bought with capability, and the direction is the whole argument.**
//! These pages hit the harness timeout today and score **zero** — no geometry, no paint, no DOM. The
//! bound turns *"no answer after 150 s"* into *"an answer from a layout a few mutations old"*, which
//! is the doctrine the drain already states everywhere else: painting what we have beats a frozen
//! tab. **A page that finishes inside the budget is bit-identical**, which is what ARM 1 below exists
//! to prove — and it is the arm that would catch this fix if it ever started charging ordinary pages.

use manuk_text::FontContext;

/// A document big enough that a full re-layout is not free, with a script that does the
/// `measure -> mutate -> measure` loop every virtualised list is built out of — which is exactly
/// the shape that forces a reflow per iteration.
fn fixture(iterations: usize, rows: usize) -> String {
    let mut html = String::from(
        r##"<!doctype html><html><head><style>
        .r { display:flex; padding:2px; border:1px solid #ccc; }
        .r span { flex:1; }
        </style></head><body><div id="out">-</div><div id="host">"##,
    );
    for i in 0..rows {
        html.push_str(&format!(
            "<div class=r><span>cell {i} a</span><span>cell {i} b</span><span>cell {i} c</span></div>"
        ));
    }
    html.push_str(&format!(
        r##"</div><script>
        var host = document.getElementById('host'), n = 0, last = 0, seen = {{}}, d = 0;
        for (var i = 0; i < {iterations}; i++) {{
          host.style.paddingTop = (i * 11) + 'px';         // mutate: CHANGES the measured height
          last = host.getBoundingClientRect().height;      // measure: forces a reflow
          if (last > 0) {{ n++; }}
          if (!seen[last]) {{ seen[last] = 1; d++; }}
        }}
        document.getElementById('out').textContent = 'reads:' + n + ' h:' + (last > 0) + ' distinct:' + d;
        </script></body></html>"##
    ));
    html
}

#[test]
fn a_thousand_forced_reflows_in_one_task_cannot_run_forever() {
    let fonts = FontContext::new();
    // ⚠⚠⚠ **THE BUDGET IS OVERRIDDEN, AND A GREEN MUTATION IS WHY.** The first version of this gate
    // drove the same loop at the production budget and passed under FOUR mutations, including
    // deleting the budget check outright — it was measuring the `ScriptDeadline`, not the bound.
    // Reflow time is a SUBSET of script time and both budgets are the same number, so a JS loop with
    // frequent interrupt points is always terminated by the deadline first. The real site reaches
    // this bound because its task spends its time in RUST, where no interrupt point exists, and a
    // fixture cannot arrange that. So the budget is set small here and the mutations bite.
    // SAFETY: one `#[test]` in this binary, so no other thread is reading the environment.
    unsafe { std::env::set_var("MANUK_REFLOW_BUDGET_MS", "150") };

    // ── ARM 1: THE CONTROL, AND IT IS THE IMPORTANT ONE. A short loop on a small document finishes
    // far inside the budget, so the bound must be invisible: every read answered, every read
    // non-zero. If this arm ever goes red the fix has started charging ordinary pages.
    let t0 = std::time::Instant::now();
    let page = manuk_page::Page::load(&fixture(6, 12), "https://rb.test/", &fonts, 800.0);
    let small_ms = t0.elapsed().as_millis();
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("FORCED-REFLOW-BUDGET control: {got}  ({small_ms} ms)");
    assert!(
        got.contains("reads:6") && got.contains("h:true") && got.contains("distinct:6"),
        "CONTROL: a 6-iteration loop on a 12-row document is nowhere near even the small test \
         budget, so every read must be answered from a FRESH layout and none may be zero (got {got:?}).\n         \
         ⭐ `distinct:6` is the arm that catches an OVER-EAGER budget: each iteration changes the \
         padding, so six reads must return six DIFFERENT heights. A bound that fires too early \
         answers them all from the same published layout and they collapse to one — which a \
         `reads:6` count alone cannot see, and a green mutation proved it could not."
    );

    // ── ARM 2: THE BOUND, AND ARM 3: THE PAGE STILL RENDERS.
    //
    // ⚠ **The script is still PREEMPTED here, and that is the designed behaviour, not a shortfall.**
    // The reflow budget and the `ScriptDeadline` are the same number by construction, and reflow time
    // is a SUBSET of script time — so a round that exhausts the reflow budget is a round already at
    // its script deadline. What the bound changes is not WHETHER the round ends but WHEN: the
    // deadline can only fire at a JS interrupt point, and an uncapped reflow loop spends its time in
    // Rust where no interrupt point exists, so the round ran 25x past its own budget. The observable
    // is therefore a BOUNDED load that still paints — which is the drain's stated doctrine
    // everywhere else — and NOT "the script completes".
    let t1 = std::time::Instant::now();
    let page = manuk_page::Page::load(&fixture(4000, 400), "https://rb2.test/", &fonts, 800.0);
    let big_ms = t1.elapsed().as_millis();
    println!("FORCED-REFLOW-BUDGET bounded: {big_ms} ms");

    // The ceiling is deliberately generous — several times the drain's own budget — because this
    // asserts the SHAPE (bounded) on whatever machine runs it, not a stopwatch. Uncapped, the same
    // shape took `morikoshi.net` 125 SECONDS in ONE task and the site scored zero.
    let ceiling_ms = 4_000u128;
    assert!(
        big_ms < ceiling_ms,
        "a script forcing 4000 full re-layouts of a 400-row document took {big_ms} ms, over a \
         ceiling of {ceiling_ms} ms — generous against the ~2s this takes bounded, and far under the \
         MINUTES it takes unbounded. The drain cannot catch this \
         — it reads its clock on a TASK BOUNDARY and this is one task — and the script watchdog \
         cannot either, because the time is spent in Rust inside the native geometry read."
    );

    // ⭐ AND IT MUST STILL HAVE PAINTED A DOCUMENT. A bound that produced a blank page would be the
    // "fast because we never did the work" trap the North Star names by name; the 400 rows must be
    // laid out with real geometry.
    let root = page.dom().root();
    let host = manuk_css::query_selector_all(page.dom(), root, "#host")[0];
    let h = page
        .node_rects()
        .get(&host)
        .map(|r| r.height)
        .unwrap_or(0.0);
    println!("FORCED-REFLOW-BUDGET host height: {h}");
    assert!(
        h > 100.0,
        "the bounded load must still PAINT: #host holds 400 rows and laid out to {h}px. A bound \
         that returns a blank page is the 'fast because we never did the work' trap, not a fix."
    );
}
