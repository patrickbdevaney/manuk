//! # G_SPAN_CLAMP — Bar 0: a `colspan` of two billion is not a big table, it is a hang
//!
//! `colspan` and `rowspan` are HTML **"clamped unsigned long"** attributes. `<td colspan="2147483648">`
//! parses cleanly as a `usize` on a 64-bit target, and the table builder then tries to make **two
//! billion columns**. The page never finishes.
//!
//! ## How it hid for as long as it has existed
//!
//! `engine/page/tests/g_reflect_numeric.rs` has carried `cs.setAttribute('colspan','2147483648')`
//! since it was written, and it **did not fail — it spun**: `user 2m57s` of a 3m00s cap, on a
//! four-element fixture. A hang is not a red assertion, so it read as *a slow gate*, and the wall
//! runs 19 of 104 gates so nothing else was looking. It surfaced only because t853 ran the whole
//! `manuk-page` suite for an unrelated regression sweep, and the **old binary reproduced it
//! identically** (3m00.2s / `user 2m56.7s`), which is what made it a defect rather than a symptom of
//! that tick.
//!
//! ## One rule, two implementations — and only one of them had it
//!
//! `engine/js/src/reflect_js.rs` implements `clamped unsigned long` correctly, and its own comment
//! says so: *"a colspan of a billion is 1000, not the default"*. So `td.colSpan` answered **1000**
//! while the layout that actually builds the table read **2,147,483,648**. The IDL was right, the
//! geometry was hung, and nothing compared the two — which is precisely why this gate asserts them
//! **together**.
//!
//! Chrome-measured (`--headless=new --dump-dom` + `getBoundingClientRect`):
//!
//! ```text
//!   <td colspan="2147483648">   colSpan 1000    a 2-column table: the cell is 46px — TWO cells wide
//!   <td colspan="1000">         colSpan 1000
//!   <td rowspan="2147483648">   rowSpan 65534
//! ```
//!
//! ⚠ **The widths are asserted against a CONTROL CELL IN THE SAME DOCUMENT, not against Chrome's
//! 46.** Our `border-collapse` cell comes out 24px where Chrome's is 23 — a 1px-per-cell residual
//! that predates this and belongs to the collapsing-border model, not to the span. Pinning 46 would
//! make this gate fail for a reason it does not test, and pinning our own 48 would freeze that
//! residual as if it were correct. The question the gate asks is *"did the span apply and get
//! bounded?"*, and `2 × unit` versus `1 × unit` answers it in either engine.
//!
//! ## The gate cannot be allowed to hang, or it is not a gate
//!
//! A Bar-0 gate for a hang that *hangs* stalls the wall instead of failing it — the exact shape that
//! let this survive. So the load runs on its own thread behind a channel timeout: if it does not
//! finish, the gate **fails with a message**, which is a red the wall can read.
//!
//! ## How it goes RED
//!
//! Restore `.unwrap_or(1).max(1)` in `LayoutBox::cell_span` (drop the `.clamp(1, max)`) and this
//! gate times out at 20s instead of completing in well under one.

use manuk_text::FontContext;

/// The overflow value the IDL clamps and layout did not. Two rows so the table has real column
/// structure to widen, and a second cell so "spans the whole table" is distinguishable from
/// "spans one cell".
const HTML: &str = r##"<!DOCTYPE html><html><head><style>
 body{margin:0} table{border-collapse:collapse} td{border:1px solid #000;width:20px}
</style></head><body>
<table><tr><td id="a" colspan="2147483648">x</td><td>y</td></tr><tr><td>1</td><td>2</td></tr></table>
<table><tr><td id="c" rowspan="2147483648">x</td><td>y</td></tr></table>
<table><tr><td id="u">x</td><td>y</td></tr></table>
</body></html>"##;

#[test]
fn g_span_clamp_is_bar_zero() {
    let (tx, rx) = std::sync::mpsc::channel();
    // Own thread: a Bar-0 gate for a hang must FAIL, not hang. `Page::load` is not `UnwindSafe`
    // across a panic here, but it does not need to be — a panic on the worker drops the sender and
    // the receiver reports the disconnect as the same failure.
    std::thread::spawn(move || {
        let fonts = FontContext::new();
        let page = manuk_page::Page::load(HTML, "https://span.test/", &fonts, 1200.0);
        let dom = page.dom();
        let rects = page.root_box.node_rects(dom);
        let w = |sel: &str| -> f32 {
            manuk_css::query_selector_all(dom, dom.root(), sel)
                .first()
                .and_then(|n| rects.get(n))
                .map(|r| r.width)
                .unwrap_or(-1.0)
        };
        let _ = tx.send((w("#a"), w("#c"), w("#u")));
    });

    let (wa, wc, unit) = rx.recv_timeout(std::time::Duration::from_secs(20)).expect(
        "G_SPAN_CLAMP (Bar 0): laying out `<td colspan=\"2147483648\">` did not finish in 20s. \
             `colspan`/`rowspan` are HTML CLAMPED unsigned longs — [1,1000] and [1,65534]. \
             Unclamped, the table builder is asked for two billion columns and the page hangs \
             forever. This is the defect `g_reflect_numeric` spun on for its whole existence.",
    );

    // ── THE UNIT: an ordinary cell in an ordinary two-column table, same sheet, same document.
    // Everything below is a multiple of this, so the collapsing-border residual cancels.
    assert!(
        unit > 1.0,
        "G_SPAN_CLAMP: the control cell `#u` has no width ({unit}) — the fixture did not lay out at \
         all, and every ratio below would be measuring nothing."
    );

    // ── The clamp REACHED LAYOUT, not just the IDL. `colspan: 1000` on a two-column table spans
    // BOTH cells, so `#a` is two units. Chrome reads 46 against our 48 for the same reason `#u`
    // reads 23 against 24. A cell that spanned only itself would be one unit; a cell that was never
    // clamped never returns at all.
    assert!(
        (wa - 2.0 * unit).abs() < 1.51,
        "G_SPAN_CLAMP: `#a` (colspan=2147483648, two-column table) is {wa}px wide against a \
         {unit}px control cell — it must be TWO units ({}), because the span clamps to 1000 and is \
         then bounded by the table's actual 2 columns. One unit means the span was dropped \
         entirely; no answer at all means it was never clamped.",
        2.0 * unit
    );
    // ── `rowspan` takes a DIFFERENT bound (65534, not 1000), so one shared constant is wrong for
    // one of them — the same shape as the two form-control intercepts (t851).
    assert!(
        (wc - unit).abs() < 1.51,
        "G_SPAN_CLAMP: `#c` (rowspan=2147483648) is {wc}px wide against a {unit}px control cell. A \
         ROW span must not widen the cell, so this is the control that catches a fix which clamped \
         the wrong attribute — or clamped `rowspan` to 1000."
    );
}
