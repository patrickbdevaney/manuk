//! **G_A11Y_LAYOUT_TABLE_IS_NOT_A_TABLE — a `<table>` used for layout is not announced as a table.**
//!
//! Chrome does not expose a header-less, border-less, small `<table>` with the table roles at all: it
//! demotes the whole subtree to `LayoutTable`/`LayoutTableRow`/`LayoutTableCell`, which no assistive
//! technology reads as tabular. We announced every one of them as `table`/`row`/`cell`, so a page laid
//! out on tables told the agent it had found data — the oldest accessibility anti-pattern there is.
//!
//! ⭐ **THE THRESHOLD IS TWENTY ROWS, AND IT WAS FOUND BY BISECTION, NOT BY READING.** Borderless,
//! header-less tables of 2 / 4 / 10 / 19 / 20 / 21 / 25 rows: the first four are `LayoutTable` and the
//! last three are `table`. Nobody lays a page out in twenty rows. It is also the row that matters most
//! in practice — `blog.rust-lang.org`'s 403-row post archive has no `<th>`, no `<caption>` and no
//! border, and without this rule the demotion ate **1,211 real nodes** from that one page (a 99.9%
//! node match fell to 27.6%, which is how the missing rule announced itself).
//!
//! Every row below is headless Chrome 145.0.7632.116 on the fixture it names:
//!
//! ```text
//!   DATA                                        LAYOUT
//!   role=table | grid | treegrid                nothing at all
//!   a <caption>                                 a <tbody> and nothing else  ⚠ every table has one
//!   a <th>                                      aria-label alone  ⚠ names it, does not type it
//!   summary=                                    headers= on a <td>
//!   <thead> or <tfoot>                          width:100%
//!   <colgroup> / <col>                          role=presentation, even WITH a <th>
//!   >= 20 rows                                  <= 19 rows
//!   a border (attribute or CSS) AND >1 cell     a border on a 1x1 table   ⚠ both spellings
//! ```
//!
//! ⚠⚠ **PRICED AFTER BUILDING, WHICH IS THE WRONG ORDER, AND THE PRICE IS SMALL:** 52 freshly-fetched
//! CrUX corpus pages carry 6 `<table>`s between them and exactly **1** is a layout table (1.9% of
//! pages). Recorded here rather than buried: this is a CORRECTNESS tick with a Chrome-arbitrated rule,
//! not a corpus-moving one, and the CrUX head under-represents the legacy tail where layout tables
//! live.
//!
//! ⚠ The scan STOPS at a nested `<table>` — layout tables nest, and a data table inside a layout table
//! must not make its container data, nor the other way round.

use manuk_a11y::{build_tree_full, name_styles, A11yNode, Role};

/// Roles present in the tree built from `html`, with the real cascade behind it so the CSS-border and
/// `display` signals are the computed ones rather than an inline-style guess.
fn roles(html: &str) -> Vec<String> {
    let fonts = manuk_text::FontContext::new();
    let page = manuk_page::Page::load(html, "https://layout-table.test/", &fonts, 800.0);
    let tree: A11yNode = page.a11y_tree();
    tree.iter().map(|n| n.role.as_str().to_string()).collect()
}

fn tables(html: &str) -> usize {
    roles(html).iter().filter(|r| *r == "table").count()
}

fn cells(html: &str) -> usize {
    roles(html).iter().filter(|r| *r == "cell").count()
}

const ROWS: &str = "<tr><td>a</td><td>b</td></tr>";

fn n_rows(n: usize) -> String {
    format!("<table>{}</table>", ROWS.repeat(n))
}

#[test]
fn a_layout_table_is_not_announced_as_a_table() {
    // ── 1. THE BARE CASE. No header, no border, two cells: Chrome says LayoutTable, and the cells'
    // CONTENT is reparented rather than lost.
    assert_eq!(
        tables("<table><tr><td>plain a</td><td>plain b</td></tr></table>"),
        0,
        "a header-less, border-less two-cell `<table>` is LAYOUT — Chrome exposes no table role for \
         it at all, and announcing one tells an agent it has found data."
    );
    assert_eq!(
        cells("<table><tr><td>plain a</td><td>plain b</td></tr></table>"),
        0,
        "and its cells go with it — the demotion drops the node and REPARENTS its children, exactly \
         as `role=presentation` does."
    );

    // ── 2. EVERY MARKUP SIGNAL THAT MAKES IT DATA. Each is one Chrome-measured row.
    for (html, why) in [
        ("<table><tr><th>hdr</th><td>c</td></tr></table>", "a <th>"),
        (
            "<table><caption>CAP</caption><tr><td>c</td></tr></table>",
            "a <caption>",
        ),
        (
            r#"<table summary="SUM"><tr><td>c</td></tr></table>"#,
            "summary=",
        ),
        (
            r#"<table role="table"><tr><td>c</td></tr></table>"#,
            "an explicit role=table",
        ),
        (
            "<table><thead><tr><td>h</td></tr></thead><tr><td>b</td></tr></table>",
            "a <thead>",
        ),
        (
            "<table><tfoot><tr><td>f</td></tr></tfoot><tr><td>b</td></tr></table>",
            "a <tfoot>",
        ),
        (
            "<table><colgroup><col></colgroup><tr><td>c</td></tr></table>",
            "a <colgroup>",
        ),
    ] {
        assert_eq!(
            tables(html),
            1,
            "{why} makes it a DATA table (Chrome-measured on this exact markup) — the demotion must \
             not eat a real table."
        );
    }

    // ── 3. THE SIGNALS THAT LOOK LIKE DATA AND ARE NOT. Without these the rule passes by calling
    // every table data, which is the bug.
    for (html, why) in [
        ("<table><tbody><tr><td>a</td><td>b</td></tr><tr><td>c</td><td>d</td></tr></tbody></table>",
         "a <tbody> — EVERY table has one, implicitly"),
        (r#"<table aria-label="LBL"><tr><td>c</td></tr></table>"#,
         "aria-label NAMES a table, it does not make it one"),
        (r#"<table><tr><td headers="h1">x</td></tr></table>"#, "a headers= attribute"),
        (r#"<table style="width:100%"><tr><td>a</td><td>b</td></tr></table>"#, "width:100%"),
        (r#"<table role="presentation"><tr><th>h</th><td>c</td></tr></table>"#,
         "role=presentation, EVEN WITH a <th>"),
    ] {
        assert_eq!(
            tables(html),
            0,
            "{why} does NOT make it a data table (Chrome-measured)."
        );
    }

    // ── 4. THE ROW THRESHOLD, BY BISECTION. 19 is layout, 20 is data.
    for n in [2usize, 4, 10, 19] {
        assert_eq!(
            tables(&n_rows(n)),
            0,
            "a borderless, header-less {n}-row table is LAYOUT (Chrome-measured)."
        );
    }
    for n in [20usize, 21, 25] {
        assert_eq!(
            tables(&n_rows(n)),
            1,
            "⭐ at {n} rows it becomes DATA — the threshold is exactly 20, found by bisecting Chrome. \
             Without it the demotion eats blog.rust-lang.org's 403-row post archive whole."
        );
    }

    // ── 5. A BORDER COUNTS ONLY ON A TABLE WITH MORE THAN ONE CELL — both spellings, all four
    // Chrome-measured.
    assert_eq!(
        tables(r#"<table border="1"><tr><td>1x1</td></tr></table>"#),
        0,
        "`border=1` on a ONE-CELL table is still LAYOUT: a single cell is not a table of anything."
    );
    assert_eq!(
        tables(r#"<table border="1"><tr><td>a</td><td>b</td></tr></table>"#),
        1,
        "`border=1` on 1x2 IS data."
    );
    assert_eq!(
        tables(r#"<table border="1"><tr><td>a</td></tr><tr><td>b</td></tr></table>"#),
        1,
        "`border=1` on 2x1 IS data — it is the CELL COUNT, not the row count, that gates the border."
    );
    assert_eq!(
        tables(
            r#"<style>.bd td{border:1px solid #000}</style><table class="bd"><tr><td>a</td><td>b</td></tr></table>"#
        ),
        1,
        "a CSS border on the CELLS is the same signal as the attribute — and it is the one signal \
         markup alone cannot answer, which is why `NameStyle` carries `has_border`."
    );
    assert_eq!(
        tables(r#"<table><tr><td style="border:1px solid #000">only</td></tr></table>"#),
        0,
        "and the CSS spelling obeys the same one-cell rule (Chrome-measured)."
    );

    // ── 6. NESTING. A data table inside a layout table stays data, and does not make its container
    // data either.
    let nested =
        "<table><tr><td><table><tr><th>inner-h</th><td>inner-c</td></tr></table></td></tr></table>";
    assert_eq!(
        tables(nested),
        1,
        "the INNER table has a <th> and is data; the OUTER one has neither, and the scan must stop \
         at the nested `<table>` rather than borrow its header."
    );

    // ── 7. THE ARIA SPELLINGS ARE UNTOUCHED. A `<div role=row>` has no `<table>` above it, so the
    // demotion must leave it exactly alone — otherwise this rule would silently delete every ARIA
    // grid on the web.
    let aria = r#"<div role="table"><div role="row"><div role="cell">AriaCell</div></div></div>"#;
    assert!(
        roles(aria).iter().any(|r| r == "table") && roles(aria).iter().any(|r| r == "cell"),
        "an ARIA table built from `<div>`s has no `<table>` element and is never demoted."
    );
    let _ = (name_styles, build_tree_full, Role::Table);
}
