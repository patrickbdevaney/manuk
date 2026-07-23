//! **G_TABLE_WRITE — `table.insertRow`/`deleteRow` + `tr.insertCell`/`deleteCell` build a table.**
//!
//! The whole write side was `undefined`, so any code that constructs table rows in JS (the classic
//! non-framework pattern, still emitted by many grid/spreadsheet widgets) threw. The index rules are the
//! spec's and exact — each is a way this goes RED:
//!
//!   * `insertRow(-1)` appends; inserting into an EMPTY table MATERIALISES a `<tbody>` (not a bare `<tr>`).
//!   * `insertRow(i)` inserts before row i; `insertCell` is the same on a `<tr>`.
//!   * an out-of-range index is an `IndexSizeError` (a THROW code branches on, not a clamp).
//!   * `deleteRow(-1)` removes the last row; `createTHead()` REUSES an existing thead; `createCaption()`
//!     inserts a `<caption>` as the first child.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
<table id="empty"></table>
<table id="t"><tbody><tr><td>a</td><td>b</td></tr></tbody></table>
<div id="out">-</div><script>
var r=[];
var empty=document.getElementById('empty'), t=document.getElementById('t');
// insert into an empty table → a tbody is materialised and the row lands in it
var er=empty.insertRow(-1);
r.push('emptyTag:'+er.tagName);
r.push('madeTbody:'+(empty.tBodies.length===1 && er.parentNode.tagName==='TBODY'?'yes':'no'));
r.push('emptyRows:'+empty.rows.length);
// insertCell on a fresh row, then read it back
var c0=er.insertCell(-1); c0.textContent='X';
r.push('cellBack:'+(er.cells.length===1 && er.cells[0].textContent==='X'?'yes':'no'));
// insertRow at index 0 in #t inserts before the existing row
var top=t.insertRow(0);
r.push('insertedTop:'+(t.rows[0]===top?'yes':'no'));
r.push('tRows:'+t.rows.length);          // original 1 + inserted 1
// out-of-range → IndexSizeError
var threw='no'; try{ t.insertRow(99); }catch(e){ threw=e.name; }
r.push('oob:'+threw);
// deleteRow(-1) removes the last row
t.deleteRow(-1);
r.push('afterDel:'+t.rows.length);
// createTHead reuses
var h1=t.createTHead(), h2=t.createTHead();
r.push('theadReuse:'+(h1===h2 && t.tHead===h1?'yes':'no'));
// createCaption is the first child
var cap=t.createCaption();
r.push('caption:'+(t.firstElementChild===cap && cap.tagName==='CAPTION'?'yes':'no'));
document.getElementById('out').textContent=r.join(' ');
</script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn table_insert_delete_row_cell_and_sections() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://table-write.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "emptyTag:TR",   // insertRow returns a <tr>
        "madeTbody:yes", // an empty table materialises a <tbody> for the row
        "emptyRows:1",
        "cellBack:yes",    // insertCell adds a cell that reads back through tr.cells
        "insertedTop:yes", // insertRow(0) lands before the existing row
        "tRows:2",
        "oob:IndexSizeError", // out-of-range index throws, not clamps
        "afterDel:1",         // deleteRow(-1) removed the last row
        "theadReuse:yes",     // createTHead reuses an existing thead
        "caption:yes",        // createCaption is inserted as the first child
    ] {
        assert!(
            got.contains(claim),
            "G_TABLE_WRITE: expected {claim} in {got:?}\n  \
             table.insertRow/deleteRow + tr.insertCell/deleteCell + createTHead/createCaption must build a \
             table programmatically, with an out-of-range index raising IndexSizeError, not clamping."
        );
    }
}
