//! **G_MOUSE_ACTUATION — double-click is a SEQUENCE, and a right-click's answer is its verdict.**
//!
//! The two agent-actuation gaps left after tick 251's re-probe cleared three phantom rows out of
//! `docs/loop/DAILY-DRIVER-EDGES.md` §1c. `dblclick` and `contextmenu` had **no dispatcher at all**
//! — zero hits across the engine — so double-click-to-select and every custom right-click menu were
//! undrivable by an agent.
//!
//! ## How each assertion here can go RED
//!
//! - **The click sequence.** A real double-click fires `click`, `click`, `dblclick`. RED, run: drop
//!   the two `dispatch_click` calls from `Page::dispatch_dblclick` and fire only the `dblclick` —
//!   the `dblclick` handler still runs, the event still arrives, and the gate fails only on the
//!   click count. That is the exact shape of a fix that reads as complete from the outside.
//!
//! - **`detail` is the click count.** `if (e.detail === 2)` on an ordinary `click` listener is the
//!   idiomatic way to handle double-click, precisely because it needs no second listener. RED, run:
//!   pass `detail: 0` (the `UIEvent` default) in `dispatch_mouse` — every listener still runs and
//!   that branch becomes permanently unreachable.
//!
//! - **The contextmenu verdict.** RED, run: have `dispatch_contextmenu` discard `proceed` and
//!   return `true`. A browser that did this would draw its native menu over the page's own.
//!
//! - **`button` is 2 for a right-click.** RED, run: leave it at the left-button default and the
//!   `e.button === 2` guard — common in real menu code — never fires.

use manuk_page::Page;
use manuk_text::FontContext;

const W: f32 = 800.0;

fn page(body: &str) -> (Page, FontContext) {
    let fonts = FontContext::new();
    let p = Page::load(body, "https://actuation.test/", &fonts, W);
    (p, fonts)
}

fn node(p: &Page, sel: &str) -> manuk_dom::NodeId {
    let root = p.dom().root();
    manuk_css::query_selector_all(p.dom(), root, sel)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("selector {sel} matched nothing"))
}

/// Read a value the page recorded into a `<div id=log>`'s text.
fn log(p: &Page) -> String {
    let n = node(p, "#log");
    p.dom().text_content(n)
}

/// **One test, deliberately.** Every gate in this directory that drives JS is a single `#[test]`,
/// and that is not stylistic: a `PageContext` is per-process here, so a second `Page::load` in the
/// same binary races the first one's runtime — the four-test version of this file passed each test
/// in isolation and **SIGSEGV'd when run together**, which is a Bar-0 signature produced entirely by
/// the harness rather than by the engine. Discovering that cost real time and is worth the comment.
#[test]
fn double_click_is_a_sequence_and_a_right_click_verdict_is_honoured() {
    let (mut p, fonts) = page(
        r#"<!doctype html><body>
<div id="t">target</div>
<input type="checkbox" id="c"><label for="c" id="l">Remember me</label>
<div id="log"></div>
<script>
  var clicks = 0, dbl = 0, sawDetail2OnClick = false, details = [];
  var ctxType = '', ctxButton = -1, ctxButtons = -1, ctxCancelable = null;

  document.getElementById('t').addEventListener('click', function (e) {
    clicks++; details.push(e.detail);
    // The idiomatic double-click handler: no dblclick listener at all.
    if (e.detail === 2) { sawDetail2OnClick = true; }
  });
  document.getElementById('t').addEventListener('dblclick', function (e) {
    dbl++; window.__dblDetail = e.detail;
  });
  document.getElementById('t').addEventListener('contextmenu', function (e) {
    ctxType = e.type; ctxButton = e.button; ctxButtons = e.buttons;
    ctxCancelable = e.cancelable;
    e.preventDefault();          // "I am drawing my own menu"
  });

  window.__report = function () {
    document.getElementById('log').textContent =
      'clicks=' + clicks + ' dbl=' + dbl + ' detail2=' + sawDetail2OnClick +
      ' details=' + details.join(',') + ' dblDetail=' + window.__dblDetail +
      ' ctxType=' + ctxType + ' ctxButton=' + ctxButton +
      ' ctxButtons=' + ctxButtons + ' ctxCancelable=' + ctxCancelable;
  };
</script></body>"#,
    );

    let t = node(&p, "#t");
    let l = node(&p, "#l");

    // ── 1. DOUBLE-CLICK ──────────────────────────────────────────────────────────────────────
    let dbl_proceed = p.dispatch_dblclick(t, &fonts, W);

    // ── 2. RIGHT-CLICK ───────────────────────────────────────────────────────────────────────
    let ctx_proceed = p.dispatch_contextmenu(t, &fonts, W);

    p.eval_for_test("window.__report();");
    let out = log(&p);

    assert!(
        out.contains("clicks=2"),
        "a double-click runs the click handler TWICE — the two clicks ARE the interaction and \
         `dblclick` is only the notification that it happened. got: {out}"
    );
    assert!(
        out.contains("dbl=1"),
        "exactly one dblclick follows the pair; got: {out}"
    );
    assert!(
        out.contains("details=1,2"),
        "`detail` is the CLICK COUNT: the first click is 1, the second is 2. This was the tick's \
         real finding — the clicks fired correctly and carried NO detail at all. got: {out}"
    );
    assert!(
        out.contains("detail2=true"),
        "`if (e.detail === 2)` on a plain click listener is how most pages handle double-click, \
         and it must be reachable WITHOUT a dblclick listener. got: {out}"
    );
    assert!(
        out.contains("dblDetail=2"),
        "the dblclick carries detail 2; got: {out}"
    );
    assert!(dbl_proceed, "nothing cancelled the dblclick");

    assert!(out.contains("ctxType=contextmenu"), "got: {out}");
    assert!(
        out.contains("ctxButton=2"),
        "a right-click is button 2; menu code commonly guards on it. got: {out}"
    );
    assert!(
        out.contains("ctxButtons=4"),
        "`buttons` is a BITMASK, not `button`'s encoding — right is index 2, bit 4. got: {out}"
    );
    assert!(out.contains("ctxCancelable=true"), "got: {out}");
    assert!(
        !ctx_proceed,
        "THE VERDICT IS THE CAPABILITY: the page called preventDefault(), so the browser must NOT \
         show its native menu. Returning true would draw the native menu over the page's own."
    );

    // ── 3. THE TWO CLICKS REACH A LABEL'S CONTROL, observed through the A11Y TREE ────────────
    // Not through JS, on purpose: `dispatch_click` documents that activation does NOT require a
    // JS context, and the a11y tree is how an AGENT confirms its own action. This also exercises
    // `A11yState.checked` — the row this tick's re-probe found the ledger calling "missing".
    let checked = |p: &Page| {
        p.a11y_tree()
            .to_observation_lines()
            .iter()
            .any(|l| l.contains("checkbox") && l.contains("checked") && !l.contains("unchecked"))
    };

    assert!(!checked(&p), "the checkbox starts unchecked");
    p.dispatch_click(l, &fonts, W);
    assert!(checked(&p), "one click through the label checks it");
    p.dispatch_dblclick(l, &fonts, W);
    assert!(
        checked(&p),
        "a double-click is TWO activations, so a checkbox ends where it began — if only one click \
         reached the control it would now read unchecked"
    );
}
