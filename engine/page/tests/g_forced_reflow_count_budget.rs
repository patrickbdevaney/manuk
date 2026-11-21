//! Load-independent forced-reflow bound: `MANUK_REFLOW_BUDGET_RELAYOUTS=N` allows exactly N forced
//! relayouts per script round, regardless of wall-clock. Reads past N are answered from the last layout.
use manuk_text::FontContext;

fn fixture(iterations: usize) -> String {
    format!(
        r##"<!doctype html><html><body><div id="out">-</div><div id="host"><p>x</p></div><script>
        var host = document.getElementById('host'), seen = {{}}, d = 0;
        for (var i = 0; i < {iterations}; i++) {{
          host.style.paddingTop = (i * 11) + 'px';
          var last = host.getBoundingClientRect().height;
          if (!seen[last]) {{ seen[last] = 1; d++; }}
        }}
        document.getElementById('out').textContent = 'distinct:' + d;
        </script></body></html>"##
    )
}

fn distinct(html: &str) -> String {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(html, "https://rc.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    page.dom().text_content(out)
}

#[test]
fn count_budget_bounds_relayouts_independent_of_wall_clock() {
    // SAFETY: single #[test] in this binary.
    unsafe { std::env::set_var("MANUK_REFLOW_BUDGET_RELAYOUTS", "100") };
    assert_eq!(
        distinct(&fixture(8)),
        "distinct:8",
        "under the count every read is fresh"
    );
    unsafe { std::env::set_var("MANUK_REFLOW_BUDGET_RELAYOUTS", "3") };
    let got = distinct(&fixture(8));
    assert_eq!(
        got, "distinct:3",
        "count budget 3 must stop fresh answers after 3 relayouts (got {got})"
    );
}
