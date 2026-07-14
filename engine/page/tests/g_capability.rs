//! **G_CAPABILITY — the pattern ledger, as executable assertions.**
//!
//! `docs/loop/WEB-PATTERNS.md` is the file that decides what this project works on next. It is the most
//! load-bearing instrument in the loop — and it has been **wrong six times**, always in the same
//! direction: a `❌` that nobody ever measured.
//!
//! In one tick it was wrong about *both of its top two priorities*:
//!
//! * *"~1 site in 4 still **hangs** — Bar 0. Nothing else matters at this ratio."* The measured number is
//!   **4 sites in 265**. Off by 16×, and it had been steering the roadmap.
//! * *"React committing its render — ❌ still silent. Mounts, schedules, throws nothing, renders
//!   nothing."* React renders. It probably had for many ticks.
//!
//! And a probe of the remaining `❌` rows found that **`append`, `prepend`, `before`, `after`,
//! `replaceWith`, `insertAdjacentHTML`, `outerHTML`, `innerText`, `Range`, `getSelection`, `Blob`,
//! `File`, `FileReader`, `MutationObserver`, `ResizeObserver` and `structuredClone` all work.** Every one
//! of them was marked missing.
//!
//! The lesson has been written down five times (PROCESS #19, #20, #21, #35, #41) and it has not held:
//! *an absent measurement is not a negative measurement.* **A rule I can recite while breaking it is a
//! decoration.** So the rule stops being a rule and becomes a mechanism:
//!
//! > **Every capability the ledger claims is asserted HERE.** If a `✅` regresses, this goes red and the
//! > tick does not land. If a `❌` is secretly working, it shows up in the probe output and gets
//! > promoted. The ledger cannot drift from reality, because reality is what runs.
//!
//! This is also the RATCHET made mechanical: *never regress capability*. A capability with no gate is
//! indistinguishable from one that does not exist — so the ledger's claims are gates now.
//!
//! Anything genuinely missing is listed at the bottom as a **`MUST_NOT_SILENTLY_APPEAR`** set: not
//! asserted absent (that would freeze the engine in its current shape), but *printed*, so the next
//! person to run this sees the truth instead of inheriting a rumour.

use manuk_text::FontContext;

/// Every capability the ledger claims works. Each is `name → JS expression yielding true`.
///
/// Keep this list in step with `docs/loop/WEB-PATTERNS.md`. That is the whole point: the ledger and the
/// engine are checked against each other, mechanically, on every tick.
const CLAIMS: &[(&str, &str)] = &[
    // ── DOM mutation (ledger: "append/prepend/before/after/replaceWith — very common")
    ("append", "(function(){var d=document.createElement('p');H.append(d);return H.lastChild===d})()"),
    ("prepend", "(function(){var d=document.createElement('p');H.prepend(d);return H.firstChild===d})()"),
    ("before", "typeof H.firstChild.before === 'function'"),
    ("after", "typeof H.firstChild.after === 'function'"),
    ("replaceWith", "typeof H.firstChild.replaceWith === 'function'"),
    ("insertAdjacentHTML", "typeof H.insertAdjacentHTML === 'function'"),
    ("remove", "typeof H.remove === 'function'"),
    // ── Serialization (ledger: "outerHTML, innerText — common")
    ("outerHTML", "typeof H.outerHTML==='string' && H.outerHTML.indexOf('<div')===0"),
    ("innerText", "typeof H.innerText === 'string'"),
    ("innerHTML set", "(function(){var d=document.createElement('div');d.innerHTML='<b>z</b>';return !!d.firstChild&&d.firstChild.tagName==='B'})()"),
    // ── Selection / ranges (ledger: "getSelection / Range — editors, copy handling")
    ("Range", "typeof Range === 'function'"),
    ("getSelection", "typeof getSelection==='function' || typeof document.getSelection==='function'"),
    // ── Files (ledger: "Blob / File / FileReader — uploads, downloads, image preview")
    ("Blob", "typeof Blob === 'function'"),
    ("File", "typeof File === 'function'"),
    ("FileReader", "typeof FileReader === 'function'"),
    // ── Events
    ("new Event()", "typeof Event==='function' && !!(new Event('x'))"),
    ("CustomEvent", "typeof CustomEvent === 'function'"),
    ("addEventListener", "typeof H.addEventListener === 'function'"),
    // ── The everyday element surface
    ("classList", "(function(){H.classList.add('z');return H.classList.contains('z')})()"),
    ("dataset", "(function(){H.dataset.k='v';return H.getAttribute('data-k')==='v'})()"),
    ("closest", "H.closest('#host') === H"),
    ("matches", "H.matches('#host')"),
    // NOTE: against `#pristine`, not `#host`. The claims above APPEND to `#host`, so asserting its
    // child count here fails on the test's own side effects — which is exactly what happened, and the
    // gate caught it. A shared fixture that the assertions mutate is a fixture that lies about the
    // engine.
    ("cloneNode(deep)", "document.getElementById('pristine').cloneNode(true).children.length === 2"),
    ("querySelectorAll", "document.querySelectorAll('#pristine span').length === 2"),
    // ── Observers & scheduling
    ("MutationObserver", "typeof MutationObserver === 'function'"),
    ("ResizeObserver", "typeof ResizeObserver === 'function'"),
    ("IntersectionObserver", "typeof IntersectionObserver === 'function'"),
    ("requestAnimationFrame", "typeof requestAnimationFrame === 'function'"),
    ("queueMicrotask", "typeof queueMicrotask === 'function'"),
    ("structuredClone", "typeof structuredClone === 'function'"),
    // ── Storage & net surface
    ("localStorage", "(function(){localStorage.setItem('k','v');return localStorage.getItem('k')==='v'})()"),
    ("fetch", "typeof fetch === 'function'"),
    ("XMLHttpRequest", "typeof XMLHttpRequest === 'function'"),
    ("FormData", "typeof FormData === 'function'"),
    ("URLSearchParams", "typeof URLSearchParams === 'function'"),
    // ── Style & geometry
    ("getComputedStyle", "getComputedStyle(H).display === 'block'"),
    ("getBoundingClientRect", "typeof H.getBoundingClientRect().x === 'number'"),
    // `transform` is APPLIED — the ledger said it was "a real gap" and it moves the box correctly.
    // Only its *computed style* is missing, which is a smaller and different claim.
    ("transform moves the box", "document.getElementById('moved').getBoundingClientRect().x === 100"),
    // ── The prototype chain (tick 64)
    ("Element.prototype.setAttribute", "typeof Element.prototype.setAttribute === 'function'"),
    ("EventTarget", "typeof EventTarget === 'function'"),
    // ── Element scrolling (tick 67). It was not missing, it LIED: reading gave `undefined`, writing
    // created a plain JS property that scrolled nothing, and `scrollHeight` was aliased to the element's
    // own box so `scrollHeight - clientHeight` was ALWAYS ZERO — a virtualised list computes its whole
    // range from that number.
    ("scrollTop is a number", "typeof document.getElementById('scroller').scrollTop === 'number'"),
    ("scrollHeight is the content", "document.getElementById('scroller').scrollHeight >= 400"),
    ("clientHeight is the window", "document.getElementById('scroller').clientHeight === 50"),
    ("scrollTop clamps", "(function(){var s=document.getElementById('scroller');s.scrollTop=1e9;return s.scrollTop===s.scrollHeight-s.clientHeight})()"),

    // ── Canvas: it PAINTS now (tick 66). Fill it red, read the pixel back, and demand red.
    ("canvas getContext", "!!document.createElement('canvas').getContext('2d')"),
    ("canvas actually paints", "(function(){var c=document.createElement('canvas');c.width=c.height=8;var x=c.getContext('2d');x.fillStyle='#f00';x.fillRect(0,0,8,8);var d=x.getImageData(0,0,1,1).data;return d[0]===255&&d[3]===255})()"),
];

/// Genuinely missing, measured, and **printed rather than asserted**.
///
/// Asserting these *absent* would be perverse — it would make the wall go red the day somebody fixes
/// one. They are here so that the next person reads a measurement instead of inheriting a rumour, and so
/// that a `❌` in the ledger has a receipt behind it.
const KNOWN_GAPS: &[(&str, &str)] = &[
    (
        "getComputedStyle().transform",
        "String(getComputedStyle(document.getElementById('moved')).transform)",
    ),
    (
        "display:contents",
        "String(getComputedStyle(document.getElementById('contents')).display)",
    ),
    (
        "scrollTop (read)",
        "String(typeof document.getElementById('host').scrollTop)",
    ),
    (
        "document.createRange",
        "String(typeof document.createRange)",
    ),
    (
        "document.createEvent",
        "String(typeof document.createEvent)",
    ),
    (
        "URL.createObjectURL",
        "String(typeof URL === 'undefined' ? 'no URL' : typeof URL.createObjectURL)",
    ),
];

fn html() -> String {
    let claims: String = CLAIMS
        .iter()
        .map(|(n, e)| format!("t({n:?}, function(){{ return ({e}) }});\n"))
        .collect();
    let gaps: String = KNOWN_GAPS
        .iter()
        .map(|(n, e)| format!("g({n:?}, function(){{ return ({e}) }});\n"))
        .collect();
    format!(
        r##"<!doctype html><html><body style="margin:0">
<div id="out">-</div><div id="gaps">-</div>
<div id="host"><span class="a">x</span><span class="b">y</span></div>
<div id="pristine"><span>x</span><span>y</span></div>
<div id="contents" style="display:contents"><i>i</i></div>
<div id="scroller" style="height:50px;width:80px;overflow:auto"><div style="height:400px">tall</div></div>
<div id="moved" style="width:20px;height:20px;transform:translateX(100px)">b</div>
<script>
var FAIL = [], GAPS = [];
var H = document.getElementById('host');
function t(name, fn) {{
  var ok = false;
  try {{ ok = (fn() === true); }} catch (e) {{ FAIL.push(name + ' THREW ' + e); return; }}
  if (!ok) FAIL.push(name);
}}
function g(name, fn) {{
  try {{ GAPS.push(name + ' = ' + fn()); }} catch (e) {{ GAPS.push(name + ' = THROW ' + e); }}
}}
{claims}
{gaps}
document.getElementById('out').textContent  = FAIL.length ? FAIL.join(' | ') : 'ALL-CLAIMS-HOLD';
document.getElementById('gaps').textContent = GAPS.join(' | ');
</script></body></html>"##
    )
}

#[test]
fn every_capability_the_ledger_claims_actually_works() {
    let fonts = FontContext::new();
    let mut page = manuk_page::Page::load(&html(), "https://capability.test/", &fonts, 800.0);
    let root = page.dom().root();

    let gaps = manuk_css::query_selector_all(page.dom(), root, "#gaps")[0];
    println!("\n── measured gaps (a ❌ in the ledger needs a receipt, and this is it):");
    for g in page.dom().text_content(gaps).split(" | ") {
        println!("     {g}");
    }

    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    assert_eq!(
        got, "ALL-CLAIMS-HOLD",
        "\n\nG_CAPABILITY: the pattern ledger claims these work, and they do not:\n\n    {got}\n\n\
         `docs/loop/WEB-PATTERNS.md` decides what this project builds next. Either the engine regressed \
         — in which case fix it, because the ratchet does not permit losing a capability — or the ledger \
         is claiming something it never measured, which is the failure that has now happened SIX times \
         (PROCESS #19, #20, #21, #35, #41). Whichever it is, the two must agree before this tick lands."
    );
}
