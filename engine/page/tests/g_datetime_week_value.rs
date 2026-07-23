//! **G_DATETIME_WEEK_VALUE — `input.valueAsNumber` / `valueAsDate` for `datetime-local` and `week`.**
//!
//! Ticks 442/443 wired the typed-value surface for `number`/`range`/`date`/`month`/`time`, but left the two
//! epoch-arithmetic follow-ons dead: a `<input type="datetime-local">` and a `<input type="week">` both
//! returned `null` from `valueAsNumber`/`valueAsDate`, and their setters were silent no-ops — so every
//! scheduling/booking form that reads `input.valueAsNumber` to compute a duration, or writes
//! `input.valueAsNumber = ms` to seed a picker, got nothing. Each claim is a way this goes RED:
//!
//!   * `datetime-local` `valueAsNumber` is the UTC ms of the local datetime (the control has no timezone);
//!     `valueAsDate` stays `null` (does not apply); setting `valueAsNumber` rewrites the value string.
//!   * `week` `valueAsNumber` is the UTC ms of the Monday 00:00 that starts the ISO week, `valueAsDate` is
//!     that same Monday as a `Date`, and setting `valueAsDate` computes the ISO week the date falls in.
//!
//! All arithmetic is UTC/ISO-8601 (weeks start Monday; week 1 holds the year's first Thursday / Jan 4), so a
//! round-trip is host-timezone-independent.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
<input id="dtl" type="datetime-local" value="2020-01-15T13:30">
<input id="wk" type="week" value="2020-W03">
<div id="out">-</div><script>
var r=[];
function k(n,v){ r.push(n+':'+JSON.stringify(v)); }
var dtl=document.getElementById('dtl');
k('dtlNum', dtl.valueAsNumber);                       // Date.UTC(2020,0,15,13,30) = 1579095000000
k('dtlDate', dtl.valueAsDate);                        // null — datetime-local has no valueAsDate
dtl.valueAsNumber = Date.UTC(2021,5,1,10,0);
k('dtlSet', dtl.value);                               // "2021-06-01T10:00"
var wk=document.getElementById('wk');
k('wkNum', wk.valueAsNumber);                         // Date.UTC(2020,0,13) = 1578873600000 (Mon of ISO W03)
var d=wk.valueAsDate;
k('wkDate', d ? d.toISOString().slice(0,10) : null);  // "2020-01-13"
wk.valueAsDate = new Date(Date.UTC(2021,0,4));        // Jan 4 2021 is a Monday → ISO 2021-W01
k('wkSet', wk.value);                                 // "2021-W01"
document.getElementById('out').textContent=r.join(' ');
</script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn datetime_local_and_week_typed_values() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://datetime-week.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "dtlNum:1579095000000",        // UTC ms of the local datetime
        "dtlDate:null",                // datetime-local carries no valueAsDate
        "dtlSet:\"2021-06-01T10:00\"", // valueAsNumber setter rewrites the value
        "wkNum:1578873600000",         // Monday 00:00 UTC that starts ISO week 3 of 2020
        "wkDate:\"2020-01-13\"",       // the same Monday as a Date
        "wkSet:\"2021-W01\"", // valueAsDate setter computes the ISO week (Jan 4 2021 → W01)
    ] {
        assert!(
            got.contains(claim),
            "G_DATETIME_WEEK_VALUE: expected {claim} in {got:?}\n  \
             datetime-local/week valueAsNumber+valueAsDate must round-trip UTC/ISO-week epochs (get and set), \
             completing the typed-input value surface begun at ticks 442/443."
        );
    }
}
