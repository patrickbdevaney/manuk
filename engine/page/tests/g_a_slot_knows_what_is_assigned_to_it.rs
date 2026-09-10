//! **G_A_SLOT_KNOWS_WHAT_IS_ASSIGNED_TO_IT — `<slot>` was an element and a name in an interface
//! list, and nothing a component could call.**
//!
//! `assignedElements()`, `assignedNodes()` and `element.assignedSlot` are how **every** web-component
//! library reads its own light-DOM children — Lit, Stencil, FAST, and every hand-rolled
//! `connectedCallback`. All three were absent, so the call was a `TypeError` and took the rest of the
//! component's boot with it.
//!
//! It is the top nameable row of the t1482 boot histogram: **eight** `assignedElements is not a
//! function` rejections on `meet.google.com`, a site rendering 1,522 of Chrome's 4,238 boxes.
//!
//! ## Arbitrated against headless Chrome, ten rows, one fixture
//!
//! ```text
//!   named=s1+s2  dupe=-  def=d1+d2  defNodes=3  slotOf=true  d1Slot=true
//!   z=-  zFlat=zfb  loose=-/0  nonSlot=threw
//! ```
//!
//! Each row is a rule that is easy to get subtly wrong, and every one of them is a real component
//! idiom rather than a spec curiosity:
//!
//! * **`dupe=-` — A NODE GOES TO THE *FIRST* SLOT OF ITS NAME** (DOM §4.2.2.4). A second
//!   `<slot name="a">` gets nothing. Pages write that deliberately as a fallback region, and an
//!   implementation that assigned to every matching slot would render the same children twice.
//! * **`defNodes=3` vs `def=d1+d2` — TEXT NODES COUNT.** `assignedNodes()` includes the host's bare
//!   text child; `assignedElements()` does not. A component that checks `assignedNodes().length` to
//!   decide whether it has content — the commonest empty-state test there is — gets the wrong answer
//!   if text is dropped.
//! * **`z=-` but `zFlat=zfb` — `flatten` CHANGES THE ANSWER ONLY WHEN THE SLOT IS EMPTY**, in which
//!   case it yields the slot's own fallback content. That is exactly the case a component asks about
//!   to decide *"am I showing the default?"*.
//! * **`loose=-/0` — A `<slot>` OUTSIDE A SHADOW TREE ASSIGNS NOTHING**, and its own children are
//!   not "assigned" to it. Legal markup, and a walk that stopped at "find the nearest parent" rather
//!   than "find the shadow root" would answer with the document.
//! * **`nonSlot=threw` — the methods are `HTMLSlotElement`'s.** They live on the shared element
//!   prototype here (see the residue below), so the tag check is what keeps `div.assignedElements()`
//!   from quietly answering `[]` — a wrong answer of the right type, which is worse than a missing
//!   one because no `try` catches it.
//!
//! ## ⚠ Residue, named, and NOT shimmed
//!
//! `slot instanceof HTMLSlotElement` is **`false`** here where Chrome says `true`, and
//! `slot.constructor.name` is `HTMLElement`. This engine gives every element reflector the one
//! `HTMLElement.prototype`; per-tag reflector prototypes are their own piece of work (the lever
//! board's T2b, sized `[L]`).
//!
//! A `Symbol.hasInstance` shim would make `instanceof` answer `true` in an afternoon **and it is
//! deliberately not done**: the prototype chain would still be wrong, so a page that patches
//! `HTMLSlotElement.prototype.foo` would still not reach instances. That is *correct in the one
//! channel a human checks* (t1282), which this project has been bitten by before.
//!
//! ⚠ `slotchange` does not fire. Assignment here is COMPUTED on demand — derived from the tree, as
//! the spec defines it, rather than cached — so the answer is never stale, but there is no
//! invalidation point to hang an event on. Components that re-read on `slotchange` will re-read late;
//! components that read in `connectedCallback` (the majority) are correct.
//!
//! Mutations that must turn this red:
//!   1. drop the first-slot-of-its-name filter   -> `dupe=s1+s2`
//!   2. filter `assignedNodes` to elements       -> `defNodes=2`
//!   3. return the fallback unconditionally      -> `z=zfb`
//!   4. walk to the document root, not the       -> `loose=` picks up the page's own children
//!      shadow root
//!   5. drop the tag check                       -> `nonSlot=ANSWERED`

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><head><meta charset=utf-8></head><body>
<div id="host"><span slot="a" id="s1">A1</span><span slot="a" id="s2">A2</span><b id="d1">D1</b>text<i id="d2">D2</i></div>
<div id="empty"><span slot="a" id="e1">E1</span></div>
<slot id="loose" name="a">FB</slot>
<div id="out">-</div>
<script>
var log = [];
function ids(a) { return a.map(function (e) { return e.id || e.nodeName; }).join('+') || '-'; }
try {
  var host = document.getElementById('host');
  var root = host.attachShadow({ mode: 'open' });
  root.innerHTML = '<slot name="a"></slot><slot name="a" id="dupe"></slot><slot><em id="fb">FB</em></slot>';
  var slots = root.querySelectorAll('slot');
  log.push('named=' + ids(slots[0].assignedElements()));
  log.push('dupe=' + ids(slots[1].assignedElements()));
  log.push('def=' + ids(slots[2].assignedElements()));
  log.push('defNodes=' + slots[2].assignedNodes().length);
  log.push('slotOf=' + (document.getElementById('s2').assignedSlot === slots[0]));
  log.push('d1Slot=' + (document.getElementById('d1').assignedSlot === slots[2]));

  var eh = document.getElementById('empty');
  var er = eh.attachShadow({ mode: 'open' });
  er.innerHTML = '<slot name="z"><i id="zfb">Z</i></slot>';
  var z = er.querySelector('slot');
  log.push('z=' + ids(z.assignedElements()));
  log.push('zFlat=' + ids(z.assignedElements({ flatten: true })));

  var loose = document.getElementById('loose');
  log.push('loose=' + ids(loose.assignedElements()) + '/' + loose.assignedNodes().length);
  log.push('nonSlot=' + (function () {
    try { document.getElementById('d1').assignedElements(); return 'ANSWERED'; }
    catch (e) { return 'threw'; }
  })());
} catch (e) { log.push('THREW:' + e.message); }
document.getElementById('out').textContent = log.join(' ');
</script></body></html>"##;

#[test]
fn a_slot_reports_the_light_dom_nodes_assigned_to_it() {
    let fonts = FontContext::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let page = rt.block_on(async {
        let mut p = manuk_page::Page::load_async(HTML, "https://slot.test/", &fonts, 800.0).await;
        p.finish_loading(&fonts, 800.0).await;
        p
    });
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);
    println!("SLOT ASSIGNMENT: {got}");

    // ── VACUITY. The script must have run to completion. Without this, a fixture that threw on
    //    line 1 leaves `#out` at `-` and every claim below is about a page that never ran.
    assert!(
        !got.contains("THREW") && got.contains("nonSlot="),
        "VACUOUS: the fixture did not complete, so none of the rows below mean anything — {got:?}"
    );
    // …and the POSITIVE case must be non-empty, or "assigns nothing" satisfies every other row.
    assert!(
        got.contains("named=s1+s2"),
        "VACUOUS: nothing was assigned to the named slot at all, so the suppression rows below are \
         passing on an implementation that simply returns [] — {got:?}"
    );

    // Chrome headless, every row.
    let want = "named=s1+s2 dupe=- def=d1+d2 defNodes=3 slotOf=true d1Slot=true \
                z=- zFlat=zfb loose=-/0 nonSlot=threw";
    assert_eq!(
        got,
        want.split_whitespace().collect::<Vec<_>>().join(" "),
        "\n  a <slot> must report the light-DOM nodes assigned to it, and only those\n  want: \
         {want}\n  got:  {got}"
    );

    // No boot error: the whole point is that this call stops taking a component's boot with it.
    assert!(
        page.boot_errors().is_empty(),
        "the slot API must not throw during boot — that is the failure this tick exists to remove: \
         {:?}",
        page.boot_errors()
            .iter()
            .map(|e| format!("{}/{}", e.phase, e.message))
            .collect::<Vec<_>>()
    );
}
