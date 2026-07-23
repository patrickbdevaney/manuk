//! **G_VALUE_AS_DATE — `input.valueAsDate` / `valueAsNumber` for date/time/month inputs.**
//!
//! Every date picker reads `input.valueAsDate` for a real `Date` (and `valueAsNumber` for the epoch), and
//! writes `valueAsDate = d` to set the control. Both were `undefined`/`NaN` for date-family types. All
//! arithmetic is UTC (a date control has no timezone), so a `type=date` round-trips regardless of host TZ.
//! Each claim is a way this goes RED:
//!
//!   * `type=date` → `valueAsDate` is UTC midnight of the day; `valueAsNumber` is that epoch in ms.
//!   * setting `valueAsDate`/`valueAsNumber` writes the control's string back (UTC).
//!   * `type=time` → ms-since-midnight and a 1970-01-01 Date; `type=month` → a month index.
//!   * `valueAsDate` is `null` on a `type=number` input (does not apply).

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
<input id="d" type="date" value="2026-07-23">
<input id="t" type="time" value="14:30">
<input id="mo" type="month" value="2026-07">
<input id="n" type="number" value="5">
<div id="out">-</div><script>
var r=[];
function k(n,v){ r.push(n+':'+v); }
var d=document.getElementById('d');
k('dDate', d.valueAsDate.toISOString());        // 2026-07-23T00:00:00.000Z
k('dNum', String(d.valueAsNumber));             // Date.UTC(2026,6,23)
d.valueAsDate=new Date(Date.UTC(2020,0,15));
k('dSet', d.value);                             // 2020-01-15
d.valueAsNumber=Date.UTC(1999,11,31);
k('dSetNum', d.value);                          // 1999-12-31
var t=document.getElementById('t');
k('tNum', String(t.valueAsNumber));             // 52200000
k('tDate', t.valueAsDate.toISOString());        // 1970-01-01T14:30:00.000Z
var mo=document.getElementById('mo');
k('moNum', String(mo.valueAsNumber));           // 678
mo.valueAsNumber=0; k('moSet', mo.value);       // 1970-01
k('nDate', String(document.getElementById('n').valueAsDate)); // null
document.getElementById('out').textContent=r.join(' ');
</script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn value_as_date_for_date_time_month_inputs() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://value-as-date.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "dDate:2026-07-23T00:00:00.000Z", // date → UTC-midnight Date
        "dNum:1784764800000",             // date → epoch ms
        "dSet:2020-01-15",                // valueAsDate setter
        "dSetNum:1999-12-31",             // valueAsNumber setter (date)
        "tNum:52200000",                  // time → ms since midnight
        "tDate:1970-01-01T14:30:00.000Z", // time → 1970 Date
        "moNum:678",                      // month → month index
        "moSet:1970-01",                  // valueAsNumber setter (month)
        "nDate:null",                     // number → valueAsDate does not apply
    ] {
        assert!(
            got.contains(claim),
            "G_VALUE_AS_DATE: expected {claim} in {got:?}\n  \
             input.valueAsDate/valueAsNumber must resolve date/time/month controls in UTC — their absence \
             blinds every date picker."
        );
    }
}
