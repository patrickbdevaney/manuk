//! **G_CLOSE_WATCHER — a close request dismisses the TOPMOST dismissable, and exactly one of them.**
//!
//! Escape (and, on a phone, the back gesture) is a *close request*. The platform asks the page to
//! dismiss **one** thing, and the spec says which one: the topmost — whichever of the open modal
//! dialogs, open `auto` popovers and live `CloseWatcher`s was activated last.
//!
//! Two defects are closed here and they are the same defect:
//!
//! 1. **`CloseWatcher` was absent** (measured and pinned at tick 568). It is the close-request
//!    actuator for an overlay that is neither a `<dialog>` nor a `[popover]` — a hand-rolled drawer,
//!    lightbox or command palette, which is still most of what ships. Without it such an overlay is
//!    undismissable by anything but a mouse, and `manuk-agent` has no verb for "dismiss the topmost
//!    thing" that works across all three mechanisms.
//!
//! 2. **Escape dismissed everything at once.** `__dialogEscape` and `__popEscape` were two
//!    independent capture listeners on `document`, each unconditional, so one Escape over a modal
//!    that had opened a menu closed BOTH — and the popover handler looped over every open `auto`
//!    popover closing all of them. A close request answered N times is not a close request.
//!
//! Being unable to express "the topmost, once" is exactly why `CloseWatcher` was added to the
//! platform, so the API and the ordering fix are one mechanism: a single shared stack.
//!
//! Claims:
//! - the constructor exists and requires `new`;
//! - Escape reaches a live watcher: `close` fires;
//! - `cancel` is **cancelable** and `preventDefault()` VETOES — the watcher stays live and the next
//!   Escape asks it again rather than falling through to what is underneath it;
//! - `destroy()` fires no `close` and takes the watcher out of the stack;
//! - **ONE Escape dismisses ONE thing**: with a modal dialog open and an `auto` popover opened over
//!   it, the first Escape closes only the popover and the second closes the dialog;
//! - a `manual` popover is never dismissed by Escape (that is what `manual` means).

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body style="margin:0">
<dialog id="dlg">modal</dialog>
<div id="pop" popover="auto">menu</div>
<div id="man" popover="manual">pinned</div>
<div id="out">-</div>
<script>
  var R = [];
  var esc = function() {
    var ev = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true });
    ev.key = 'Escape';
    document.dispatchEvent(ev);
  };

  R.push('ctor:' + (typeof CloseWatcher === 'function'));
  var threw = false;
  try { CloseWatcher(); } catch (e) { threw = true; }
  R.push('needsnew:' + threw);

  // (1) A live watcher answers the close request.
  var w1 = new CloseWatcher();
  var w1closed = false, w1cancelable = null;
  w1.addEventListener('close', function() { w1closed = true; });
  w1.addEventListener('cancel', function(e) { w1cancelable = e.cancelable; });
  esc();
  R.push('w1close:' + w1closed);
  R.push('w1cancelable:' + w1cancelable);

  // (2) A `cancel` handler that preventDefaults VETOES — and the veto must NOT fall through to the
  //     watcher underneath it. w2 is pushed first, w3 on top of it and vetoing.
  var w2 = new CloseWatcher();
  var w2closed = false;
  w2.onclose = function() { w2closed = true; };
  var w3 = new CloseWatcher();
  var w3closed = false, w3asked = 0;
  w3.onclose = function() { w3closed = true; };
  w3.oncancel = function(e) { w3asked++; if (w3asked === 1) { e.preventDefault(); } };
  esc();
  R.push('w3vetoed:' + (w3closed === false));      // the veto held
  R.push('w2untouched:' + (w2closed === false));   // ...and did NOT fall through to w2
  esc();
  R.push('w3second:' + w3closed);                  // asked again, closed this time
  R.push('w2still:' + (w2closed === false));       // w2 is next, not yet asked

  // (3) destroy() fires no close and leaves the stack — the next Escape reaches w2.
  var w4 = new CloseWatcher();
  var w4closed = false;
  w4.onclose = function() { w4closed = true; };
  w4.destroy();
  esc();
  R.push('w4destroyed:' + (w4closed === false));
  R.push('w2reached:' + w2closed);                 // Escape fell through the destroyed watcher

  // (4) THE ORDERING FIX. A modal dialog, then an auto popover opened over it. One Escape must
  //     dismiss the popover ONLY; the dialog survives to be dismissed by the next one.
  var dlg = document.getElementById('dlg');
  var pop = document.getElementById('pop');
  var man = document.getElementById('man');
  man.showPopover();
  dlg.showModal();
  pop.showPopover();
  esc();
  R.push('popgone:' + (pop.hasAttribute('data-manuk-popover-open') === false));
  R.push('dlgkept:' + dlg.hasAttribute('open'));
  esc();
  R.push('dlggone:' + (dlg.hasAttribute('open') === false));
  R.push('mankept:' + man.hasAttribute('data-manuk-popover-open'));

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn a_close_request_dismisses_the_topmost_dismissable_and_only_it() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://cw.test/", &fonts, 400.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("ctor:true", "`CloseWatcher` must exist — a feature-detecting overlay takes its fallback without it, and a hand-rolled drawer has no way to answer Escape at all"),
        ("needsnew:true", "it is a constructor: calling it without `new` is a TypeError"),
        ("w1close:true", "Escape must reach a live watcher and fire `close`"),
        ("w1cancelable:true", "the `cancel` it fires first must be CANCELABLE — that is the veto hook, the whole reason `requestClose` is not `close`"),
        (
            "w3vetoed:true",
            "a `cancel` handler calling preventDefault() must VETO the dismissal — the watcher stays live",
        ),
        (
            "w2untouched:true",
            "a VETOED close request must NOT fall through to the watcher underneath. The scan stops at the first ACTIVE entry, not the first entry that actually closed — otherwise an \"unsaved changes\" guard on top would silently dismiss the thing below it",
        ),
        ("w3second:true", "the next Escape asks the vetoed watcher again"),
        ("w2still:true", "…and only that one: w2 is next in the stack, not yet reached"),
        ("w4destroyed:true", "destroy() must NOT fire `close` — it is the \"went away for another reason\" exit, and firing close would re-run the page's teardown twice"),
        ("w2reached:true", "a destroyed watcher is out of the stack, so the close request falls through to the next live one"),
        (
            "popgone:true",
            "ONE close request dismisses ONE thing, and it is the TOPMOST: the popover opened over the modal goes first",
        ),
        (
            "dlgkept:true",
            "…and the modal underneath it SURVIVES that Escape. Two independent keydown listeners closed both at once — one keypress, two dismissals",
        ),
        ("dlggone:true", "the second Escape then dismisses the dialog, which is now topmost"),
        (
            "mankept:true",
            "a `manual` popover is never light-dismissed — Escape must not touch it at any point",
        ),
    ] {
        assert!(
            got.contains(claim),
            "G_CLOSE_WATCHER: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
