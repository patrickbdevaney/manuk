//! **G_VALUE_AS_NUMBER — `input.valueAsNumber` (get/set) + `stepUp`/`stepDown` for numeric inputs.**
//!
//! Every numeric spinner, range slider and quantity stepper reads/writes the NUMBER behind the control,
//! not its string. `input.valueAsNumber` was `undefined` and `stepUp`/`stepDown` threw (not a function),
//! so a "+"/"−" quantity button or a `valueAsNumber = total` assignment did nothing. Each claim is a way
//! this goes RED:
//!
//!   * `valueAsNumber` reads and writes the numeric value of a `type=number` / `type=range` input.
//!   * `stepUp(n)` / `stepDown(n)` add/subtract `n × step` and clamp to `min`/`max`.
//!   * `valueAsNumber` is `NaN` for an unsupported type (`text`) and `undefined` on a non-input.

use manuk_text::FontContext;

const HTML: &str = r#"<!doctype html><html><body>
<input id="n" type="number" value="42" step="2" min="0" max="100">
<input id="r" type="range" value="5" step="5" max="20">
<input id="lo" type="number" value="1" step="5" min="0">
<input id="txt" type="text" value="hi">
<div id="out">-</div><script>
var r=[];
function k(n,v){ r.push(n+':'+v); }
var n=document.getElementById('n');
k('read', String(n.valueAsNumber));   // 42
n.stepUp();  k('up', n.value);         // 44
n.stepDown(3); k('down', n.value);     // 38
n.valueAsNumber=10; k('set', n.value+'/'+n.valueAsNumber); // 10/10
var rg=document.getElementById('r');
rg.value='100'; rg.stepUp(); k('clampMax', rg.value); // clamped to 20
var lo=document.getElementById('lo');
lo.stepDown(5); k('clampMin', lo.value);   // 1 - 25 → clamped to min 0
k('txtNaN', String(document.getElementById('txt').valueAsNumber)); // NaN
k('divUndef', typeof document.createElement('div').valueAsNumber); // undefined
document.getElementById('out').textContent=r.join(' ');
</script></body></html>"#;

/// One test in the binary — two SpiderMonkey contexts tear down messily (see `g_globals`).
#[test]
fn value_as_number_and_step_up_down() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://value-as-number.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for claim in [
        "read:42",            // valueAsNumber getter
        "up:44",              // stepUp() = +step
        "down:38",            // stepDown(3) = −3×step
        "set:10/10",          // valueAsNumber setter round-trips
        "clampMax:20",        // stepUp clamps to max
        "clampMin:0",         // stepDown clamps to min
        "txtNaN:NaN",         // unsupported type → NaN
        "divUndef:undefined", // non-input has no valueAsNumber
    ] {
        assert!(
            got.contains(claim),
            "G_VALUE_AS_NUMBER: expected {claim} in {got:?}\n  \
             input.valueAsNumber (get/set) and stepUp/stepDown must work for number/range inputs with \
             min/max clamping — their absence blinds every numeric-form widget."
        );
    }
}
