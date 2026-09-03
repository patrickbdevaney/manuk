//! **G_DYNAMIC_SCRIPT_INSERTION — a `<script>` element inserted by script never executed.**
//!
//! Not a MIME table, though the failing test NAMES looked like one. `scripting-1`'s type matrix
//! reported **193 distinct failing `type` values, every one of them a "should run" and not a single
//! "should not run"** — which is the signature of a path that runs *nothing*, not of a list that is
//! missing entries. Those tests do `document.createElement('script')`, set `textContent`, append, and
//! assert **synchronously**; the type is only their parameter space.
//!
//! ⭐⭐⭐ **The population is every script loader on the web.** `document.createElement('script')` +
//! `appendChild` is how analytics tags, ad tags, A/B frameworks, payment SDKs and lazily-loaded
//! widgets boot. A page whose loader injected the real application script did **nothing at all**,
//! silently — which is the shape of the board's "booted but thin" cohort.
//!
//! Every rule below Chrome-measured, each in its own probe:
//!
//! ```text
//!   appendChild into the document        runs SYNCHRONOUSLY (true on the very next line)
//!   appended to a DETACHED parent        does NOT run …
//!   …then connecting that parent         …runs THEN — the trigger is BECOMING CONNECTED
//!   re-appending an already-run script   does NOT run again
//!   setting .textContent after insert    runs
//!   innerHTML                            does NOT run
//!   text/javascript;charset=utf-8        does NOT run — an ESSENCE match, not a prefix test
//! ```
//!
//! ## ⚠⚠⚠ AND THE FIRST IMPLEMENTATION WAS A BAR 0
//!
//! It walked the mutation's `added` list and every descendant, asking each node whether it was a
//! script — O(nodes inserted) on every childList mutation. `span-limits.html` inserts **65,532
//! `<tr><td>` rows in one `innerHTML +=`** and the area went `HANG/CRASH 0` → `HANG/CRASH 1`.
//!
//! **The loop is inverted**: iterate the small set of script-created `<script>` elements that have
//! not yet run — usually empty, always a handful — instead of the large set of inserted nodes. Cost
//! per mutation became O(pending scripts) and independent of how much was inserted. *A capability
//! bought with a hang is refused; the ratchet is not negotiable.*
//!
//! ## ⚠ The eligibility flag is POSITIVE on purpose
//!
//! The spec says it negatively — fragment parsing marks scripts "already started" — which would mean
//! marking at all **seven** `set_inner_html` call sites. The designs fail in opposite directions: a
//! missed mark there makes `innerHTML` EXECUTE A SCRIPT, which is a load-bearing security invariant;
//! a missed case here merely leaves a script not running. Fail-safe wins, and it needs one marking
//! site instead of seven a future caller can skip.
//!
//! ⚠ Known gap, measured and named: `cloneNode` of a parser-created `<script>` runs in Chrome and
//! does not here. (`<template>` content is `false` in Chrome even when cloned, so that half agrees.)
//!
//! ⚠ ONE `#[test]` in this binary: two SpiderMonkey contexts in one manuk-page binary tear each other
//! down and the second test reads the first one's empty output.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body><div id="slot"></div><div id="out">-</div>
<script>
var r = [];
function k(n, v) { r.push(n + ':' + JSON.stringify(v)); }
var slot = document.getElementById('slot');
function mk(txt, ty) {
  var s = document.createElement('script');
  if (ty !== undefined) { s.setAttribute('type', ty); }
  s.textContent = txt || 'window.ran = true;';
  return s;
}

// ── 1. IT RUNS, AND IT RUNS SYNCHRONOUSLY ─────────────────────────────────────────────
window.ran = false; slot.appendChild(mk());
k('a_syncOnAppend', window.ran);

// ── 2. THE TRIGGER IS BECOMING CONNECTED, NOT appendChild ─────────────────────────────
window.ran = false;
var detached = document.createElement('div');
detached.appendChild(mk());
k('b_detachedParent', window.ran);
document.body.appendChild(detached);
k('c_afterConnectingParent', window.ran);

// ── 3. ONCE, AND ONLY ONCE ────────────────────────────────────────────────────────────
window.ran = false; var once = mk(); slot.appendChild(once);
k('d_firstRun', window.ran);
window.ran = false; document.body.appendChild(once);
k('e_reAppendRunsAgain', window.ran);

// ── 4. OTHER INSERTION PATHS — the hook is at the choke point, not on one method ───────
window.ran = false; document.head.appendChild(mk());
k('f_headAppend', window.ran);
window.ran = false; document.body.insertBefore(mk(), document.body.firstChild);
k('g_insertBefore', window.ran);
window.ran = false;
var victim = document.createElement('span'); slot.appendChild(victim);
slot.replaceChild(mk(), victim);
k('h_replaceChild', window.ran);

// ── 5. innerHTML MUST NOT RUN — the security invariant ────────────────────────────────
window.ran = false;
slot.innerHTML = '<scr' + 'ipt>window.ran = true;<\/scr' + 'ipt>';
k('i_innerHtmlNeverRuns', window.ran);

// ── 6. TEXT SUPPLIED AFTER INSERTION ──────────────────────────────────────────────────
window.ran = false;
var late = document.createElement('script'); slot.appendChild(late);
late.textContent = 'window.ran = true;';
k('j_textAfterInsert', window.ran);

// ── 7. THE TYPE GATE — an ESSENCE match against the legacy JavaScript MIME types ───────
function runsWith(ty) {
  window.ran = false; slot.appendChild(mk(undefined, ty)); return window.ran;
}
k('k_legacyAliases', [
  'text/javascript', 'application/javascript', 'application/ecmascript', 'text/ecmascript',
  'application/x-javascript', 'text/x-javascript', 'text/livescript', 'text/jscript',
  'text/javascript1.5',
].map(runsWith).join(''));
k('l_emptyType', runsWith(''));
k('m_whitespacePadded', runsWith('  text/javascript  '));
k('n_uppercase', runsWith('TEXT/JAVASCRIPT'));
k('o_withParameter', runsWith('text/javascript;charset=utf-8'));
k('p_rejected', ['text/plain', 'application/json', 'text/babel', 'javascript'].map(runsWith).join(''));
window.ran = false;
var noType = document.createElement('script'); noType.textContent = 'window.ran = true;';
slot.appendChild(noType);
k('q_noTypeAttribute', window.ran);

document.getElementById('out').textContent = r.join(' ');
</script></body></html>"##;

/// One test in the binary — see the module note.
#[test]
fn a_script_element_inserted_by_script_runs_on_becoming_connected_exactly_once() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://dynamic-script.test/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("DYNAMIC-SCRIPT RESULT: {got}");

    for claim in [
        // 1 — it runs, synchronously
        "a_syncOnAppend:true",
        // 2 — the trigger is CONNECTEDNESS
        "b_detachedParent:false",
        "c_afterConnectingParent:true",
        // 3 — once, and only once
        "d_firstRun:true",
        "e_reAppendRunsAgain:false",
        // 4 — every insertion path, because the hook is at the shared choke point
        "f_headAppend:true",
        "g_insertBefore:true",
        "h_replaceChild:true",
        // 5 — and innerHTML never does
        "i_innerHtmlNeverRuns:false",
        // 6 — text supplied after insertion
        "j_textAfterInsert:true",
        // 7 — the type gate
        "k_legacyAliases:\"truetruetruetruetruetruetruetruetrue\"",
        "l_emptyType:true",
        "m_whitespacePadded:true",
        "n_uppercase:true",
        "o_withParameter:false",
        "p_rejected:\"falsefalsefalsefalse\"",
        "q_noTypeAttribute:true",
    ] {
        assert!(
            got.contains(claim),
            "G_DYNAMIC_SCRIPT_INSERTION: expected `{claim}`\n  got: {got}\n\n  \
             A <script> the page's own script created runs SYNCHRONOUSLY when it becomes connected, \
             exactly once, for any legacy JavaScript MIME type (an ESSENCE match — a `;charset=` \
             parameter is not one). `innerHTML` must NEVER run one. Every row is Chrome-measured."
        );
    }
}
