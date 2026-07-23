//! **G_TABLE_DOM — `table.rows` / `tr.cells` and the row/cell indices a table library walks.**
//!
//! The whole `<table>` read surface was `undefined`. `table.rows` and `tr.cells` are how a data-grid /
//! sortable-table widget and every "what row/column is this cell" accessibility walk read a table, and
//! `rowIndex`/`cellIndex` are the coordinates they report. Each claim is a way the missing surface goes RED:
//!
//!   * `table.rows` is a live HTMLCollection in LOGICAL order — thead rows, then tbody + direct `<tr>`
//!     rows, then tfoot rows — NOT document order. The fixture writes `<tfoot>` BEFORE `<tbody>` so a
//!     document-order implementation mis-numbers it: the tfoot row must be LAST (index 3), and its
//!     `rowIndex` proves the ordering.
//!   * `table.tBodies` / `table.tHead` / `table.tFoot`, section `.rows`, and `tr.cells` all resolve.
//!   * `tr.rowIndex` (index in `table.rows`), `tr.sectionRowIndex` (index in its section), and
//!     `td.cellIndex` (index in `tr.cells`) are the coordinates.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
<table id="t">
  <thead><tr><th>H1</th><th>H2</th></tr></thead>
  <tfoot><tr><td>f</td><td>g</td></tr></tfoot>
  <tbody><tr><td>a</td><td>b</td></tr><tr><td>c</td><td>d</td></tr></tbody>
</table>
<div id="out">-</div><script>
var r=[]; var t=document.getElementById('t');
r.push('rows:'+t.rows.length);                       // 1 thead + 2 tbody + 1 tfoot
r.push('tbodies:'+t.tBodies.length);
r.push('thead:'+t.tHead.tagName);
r.push('tfoot:'+t.tFoot.tagName);
r.push('secRows:'+t.tBodies[0].rows.length);         // the tbody's own rows
// logical order: [thead-tr, tbody-tr-a, tbody-tr-c, tfoot-tr] despite tfoot written before tbody
r.push('headText:'+t.rows[0].cells[0].textContent.trim());
r.push('footIsLast:'+(t.rows[3].cells[0].textContent.trim()==='f'?'yes':'no'));
r.push('cells:'+t.rows[1].cells.length);
r.push('rowIndex:'+t.rows[1].rowIndex);              // 1
r.push('footRowIndex:'+t.rows[3].rowIndex);          // 3 (last)
r.push('secIdx:'+t.rows[2].sectionRowIndex);         // 2nd tbody row → 1 within its section
r.push('cellIdx:'+t.rows[1].cells[1].cellIndex);     // 1
document.getElementById('out').textContent=r.join(' ');
</script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn table_rows_and_cells_expose_logical_order_and_indices() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://table-dom.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "rows:4", // thead + 2 tbody + tfoot
        "tbodies:1",
        "thead:THEAD",
        "tfoot:TFOOT",
        "secRows:2",      // the tbody's own rows
        "headText:H1",    // rows[0] is the thead row (logical order)
        "footIsLast:yes", // tfoot row is LAST despite being written before tbody
        "cells:2",
        "rowIndex:1",
        "footRowIndex:3", // the ordering proof
        "secIdx:1",       // 2nd row within its tbody section
        "cellIdx:1",
    ] {
        assert!(
            got.contains(claim),
            "G_TABLE_DOM: expected {claim} in {got:?}\n  \
             table.rows (logical order: thead, tbody+tr, tfoot) / tr.cells / rowIndex / sectionRowIndex / \
             cellIndex must expose a table's structure — their absence blinds every table/grid library."
        );
    }
}
