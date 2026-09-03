//! **G_A11Y_NAME_FROM_CONTENT_CONTEXT — `listitem` never takes its name from its contents, and
//! `row` takes its name from its contents ONLY INSIDE A GRID.**
//!
//! ⭐⭐⭐ **THE A11Y TREE HAD NEVER BEEN MEASURED AGAINST A REAL SITE.** Track B's `>=90% node match`
//! bar was quoted from WPT `wai-aria` / `html-aam` / `accname` — and constitution check #131 had
//! already recorded why that is a different claim: Interop 2026 lists accessibility testing as an
//! *investigation effort*, which is the platform saying *no suite can decide this yet*. So the first
//! real-site measurement was taken, six pages, Manuk's tree vs headless Chrome's CDP
//! `Accessibility.getFullAXTree`, matched as a `(role, name)` multiset:
//!
//! ```text
//!   page                    chrome nodes    node match
//!   danluu.com                       414         51.2%
//!   a11yproject.com                  162         67.9%
//!   blog.rust-lang.org              1673         74.9%
//!   news.ycombinator.com             490         84.5%
//!   whatwg.org                        32         90.6%
//!   martinfowler.com                 297         95.6%
//!   AGGREGATE                       3068         75.0%      <- against a bar quoted as ">=90%"
//! ```
//!
//! ⭐⭐⭐ **AND 721 OF THE 766 MISSED NODES — 94% — WERE ONE EXPRESSION.** `Role::name_from_content()`
//! listed `ListItem` and `Row`. Chrome names both `""`: 462 `row` and 259 `listitem`. A `<li>`
//! wrapping a link was announced as the whole sentence instead of as the link inside it, and every
//! table row was announced as its entire row of text — so a data table read as a wall of duplicated
//! prose, and an agent matching on the name got the row rather than the cell.
//!
//! ### ⭐⭐ `row` IS NOT SIMPLY WRONG — ITS ANSWER DEPENDS ON WHERE THE ROW IS
//!
//! Headless Chrome 145.0.7632.116, one fixture per row, every arm below taken from it:
//!
//! ```text
//!   <div role=table><div role=row><div role=cell>X        row  name=""       static structure
//!   <div role=grid><div role=row><div role=gridcell>X     row  name="X"      ⭐ FROM CONTENT
//!   <div role=treegrid><div role=row><div role=gridcell>  row  name="TG-CELL"
//!   <div role=grid><div role=rowgroup><div role=row>      row  name="RG-CELL"  rowgroup TRANSPARENT
//!   <table role=grid><tbody><tr><td>NATIVE-GRID-CELL      row  name="NATIVE-GRID-CELL"
//!   <div role=grid><div role=row aria-label=RowLabel>     row  name="RowLabel" aria still wins
//! ```
//!
//! **A grid is the interactive widget and a table is static content** — the distinction `Role::Grid`
//! was split out of `Role::Table` to preserve, and this is the first rule that consumes it. A method
//! on the role alone cannot express it, so the name computation now asks
//! `takes_name_from_content(dom, node, role)`.
//!
//! ⚠ `treegrid` was ABSENT from the role vocabulary — `role="treegrid"` fell through to the
//! element's implicit role and a `<div>` announced as `generic`. The rule cannot be written without
//! it, so it is added here and asserted below.
//!
//! **After the fix, the same six pages: 75.0% -> 97.0% aggregate** (danluu 51.2 -> 100.0,
//! blog.rust-lang 74.9 -> 99.9). WPT `accname`/`wai-aria`/`html-aam` unchanged, checked by name list.

use manuk_a11y::{accessible_name, role_of, Role};

fn name_of(html: &str, id: &str) -> String {
    let dom = manuk_html::parse(html);
    let n = dom
        .get_element_by_id(dom.root(), id)
        .unwrap_or_else(|| panic!("VACUOUS: the fixture has no #{id}"));
    let role = role_of(&dom, n).unwrap_or_else(|| panic!("VACUOUS: #{id} maps to no ARIA role"));
    accessible_name(&dom, n, &role)
}

fn role_str(html: &str, id: &str) -> String {
    let dom = manuk_html::parse(html);
    let n = dom.get_element_by_id(dom.root(), id).expect("fixture id");
    role_of(&dom, n)
        .map(|r| r.as_str().to_string())
        .unwrap_or_default()
}

#[test]
fn listitem_is_never_named_from_content_and_row_is_only_inside_a_grid() {
    // ── 1. `listitem` — NEVER from content, in any spelling.
    assert_eq!(
        name_of(r#"<ul><li id="li">Alpha</li></ul>"#, "li"),
        "",
        "Chrome names a `<li>` \"\" — `listitem` is not a name-from-content role. Announcing the \
         item's whole sentence turns every navigation list into duplicated prose, and it was 259 of \
         the 766 nodes the real-site survey found missing."
    );
    assert_eq!(
        name_of(
            r#"<div role="list"><div role="listitem" id="li">AriaItem</div></div>"#,
            "li"
        ),
        "",
        "the ARIA spelling must agree with the native one — one rule, not two."
    );
    // The control that keeps arm 1 from being "listitem is always nameless": an explicit author
    // name still wins, because steps 1-3 run BEFORE name-from-content.
    assert_eq!(
        name_of(
            r#"<ul><li id="li" aria-label="ItemLabel">ItemText</li></ul>"#,
            "li"
        ),
        "ItemLabel",
        "`aria-label` on a `<li>` still names it — the fix removes step 2F for this role, not the \
         steps above it."
    );
    // And the thing that DOES carry the name is the link inside, which is what an agent clicks.
    assert_eq!(
        name_of(
            r##"<ul><li id="li"><a href="#a" id="a">InnerLink</a></li></ul>"##,
            "a"
        ),
        "InnerLink",
        "the LINK inside the list item keeps its name — that is the actionable node."
    );

    // ── 2. `row` — from content INSIDE A GRID, and not inside a table.
    assert_eq!(
        name_of(
            r#"<div role="table"><div role="row" id="r"><div role="cell">X</div></div></div>"#,
            "r"
        ),
        "",
        "a row in a static `table` is structure, not a label (Chrome: name=\"\"). It was 462 of the \
         766 missing nodes — the single largest mechanism in the real-site survey."
    );
    assert_eq!(
        name_of(
            r#"<div role="grid"><div role="row" id="r"><div role="gridcell">GridCellText</div></div></div>"#,
            "r"
        ),
        "GridCellText",
        "⭐ a row in a GRID *is* named from its contents (Chrome-measured). A grid is the \
         interactive widget; its rows are things a user selects and hears described."
    );
    assert_eq!(
        name_of(
            r#"<div role="treegrid"><div role="row" id="r"><div role="gridcell">TG-CELL</div></div></div>"#,
            "r"
        ),
        "TG-CELL",
        "`treegrid` is a grid for this rule — and the role had to be ADDED to the vocabulary for \
         the rule to be expressible at all."
    );
    assert_eq!(
        name_of(
            r#"<div role="grid"><div role="rowgroup"><div role="row" id="r"><div role="gridcell">RG-CELL</div></div></div></div>"#,
            "r"
        ),
        "RG-CELL",
        "`rowgroup` (`<tbody>`) is a grouping LEVEL, not a container kind — the walk must see \
         through it or every real grid's rows lose their names."
    );
    assert_eq!(
        name_of(
            r#"<table role="grid"><tbody><tr id="r"><td>NATIVE-GRID-CELL</td></tr></tbody></table>"#,
            "r"
        ),
        "NATIVE-GRID-CELL",
        "a NATIVE `<table role=grid>` is a grid too — the rule is about the declared container, not \
         about the tag."
    );
    assert_eq!(
        name_of(
            r#"<div role="grid"><div role="row" id="r" aria-label="RowLabel"><div role="gridcell">IGNORED</div></div></div>"#,
            "r"
        ),
        "RowLabel",
        "`aria-label` still wins over the grid rule, as it wins over every step 2F."
    );
    // ⭐ THE ARM A GREEN MUTATION ASKED FOR. Deleting the `Table` stop from the ancestor walk left
    // every arm above green, because none of them nests a static table inside a grid. Chrome does:
    // `<div role=grid><div role=table><div role=row>` names the row `""` — **a static table nested
    // inside a grid is still a static table**, so the walk must STOP at the nearest declared
    // container rather than search upward for a grid.
    assert_eq!(
        name_of(
            r#"<div role="grid"><div role="table"><div role="row" id="r"><div role="cell">NESTED-TABLE-CELL</div></div></div></div>"#,
            "r"
        ),
        "",
        "a `table` nested inside a `grid` still makes its rows structural (Chrome: name=\"\"). The \
         ancestor walk answers with the NEAREST declared container, it does not go looking for a grid."
    );

    // ── 3. THE ROLES THAT MUST NOT HAVE MOVED. Without these, arms 1-2 are satisfied by deleting
    // name-from-content entirely.
    for (html, id, want) in [
        (r#"<button id="x">Go</button>"#, "x", "Go"),
        (r##"<a href="#h" id="x">LinkText</a>"##, "x", "LinkText"),
        (r#"<h2 id="x">HeadingText</h2>"#, "x", "HeadingText"),
        (r#"<div role="tab" id="x">TabText</div>"#, "x", "TabText"),
        (r#"<div role="option" id="x">OptText</div>"#, "x", "OptText"),
        (
            r#"<div role="menuitem" id="x">MenuText</div>"#,
            "x",
            "MenuText",
        ),
        (
            r#"<div role="treeitem" id="x">TreeText</div>"#,
            "x",
            "TreeText",
        ),
        (
            r#"<div role="switch" id="x">SwitchText</div>"#,
            "x",
            "SwitchText",
        ),
        (
            r#"<div role="tooltip" id="x">TipText</div>"#,
            "x",
            "TipText",
        ),
        (
            r#"<table><tr><th id="x">head-a</th><td>c</td></tr></table>"#,
            "x",
            "head-a",
        ),
        (
            r#"<table><tr><th>h</th><td id="x">cell-a</td></tr></table>"#,
            "x",
            "cell-a",
        ),
        (
            r#"<div role="grid"><div role="row"><div role="gridcell" id="x">GC</div></div></div>"#,
            "x",
            "GC",
        ),
    ] {
        assert_eq!(
            name_of(html, id),
            want,
            "CONTROL: `{id}` in `{html}` must still be named from its contents — every one of these \
             is Chrome-measured on the same fixture, and without them arms 1-2 pass by deleting \
             step 2F outright."
        );
    }
    // And the roles Chrome leaves nameless, which were already right and must stay right.
    for (html, id) in [
        (r#"<div role="group" id="x">GroupText</div>"#, "x"),
        (r#"<article id="x">ArticleText</article>"#, "x"),
        (r#"<p id="x">ParaText</p>"#, "x"),
    ] {
        assert_eq!(
            name_of(html, id),
            "",
            "CONTROL: `{id}` is not a name-from-content role and never was."
        );
    }

    // ── 4. THE ROLE THAT DID NOT EXIST. `role="treegrid"` fell through to the element's implicit
    // role, so a data grid announced itself as a `<div>`.
    assert_eq!(
        role_str(r#"<div role="treegrid" id="x"></div>"#, "x"),
        "treegrid",
        "`treegrid` must be in the role vocabulary — it is an ARIA 1.2 widget role, and the row \
         rule above cannot be stated without it."
    );
    assert_eq!(
        Role::parse("treegrid"),
        Some(Role::TreeGrid),
        "and it must round-trip through the token parser the agent calls."
    );
}
