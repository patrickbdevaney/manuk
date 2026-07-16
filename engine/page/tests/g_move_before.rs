//! **G_MOVE_BEFORE — `Node.prototype.moveBefore`, the atomic move, and its pre-move validity.**
//!
//! `moveBefore` relocates a connected node **without** the remove-then-insert side effects that reset
//! the moved subtree's state (an iframe reloads, an animation restarts, focus/selection is lost). Manuk
//! has no such state to lose, so the *observable relocation* equals `insertBefore`'s — what the platform
//! gains is the method's existence and its **stricter pre-move validity**, the throws real code (framework
//! reconcilers, feature detection) branches on. Each assertion below is one of those spec rules:
//!
//! * **WebIDL arg coercion** — a non-`Node` first argument, a missing second argument, or a non-`Node`
//!   second argument is a `TypeError` before any DOM step.
//! * **both nodes connected** — the constraint that separates an atomic move from `insertBefore` (which
//!   happily inserts a disconnected node): a disconnected *destination* OR a disconnected *target* throws
//!   `HierarchyRequestError`.
//! * **no cycle** — moving a node into its own descendant throws `HierarchyRequestError`.
//! * **reference child must belong to the destination** — else `NotFoundError`.
//! * **the move actually happens** — `box.moveBefore(b, a)` reorders `[a,b]` into `[b,a]`.
//!
//! Own binary: two SpiderMonkey-backed `Page::load`s in one process reuse the JS runtime and can trip the
//! tracked reflector-teardown UAF (see the flexbox-relayout Bar-0 note). One JS gate = one process.
//!
//! **Falsifiable:** before the native existed `moveBefore` was `undefined`, so every `Hierarchy*`/
//! `NotFound` case read back `TypeError` ("moveBefore is not a function") and the bare reorder call threw,
//! leaving `#out` at its `-` sentinel — RED. The full method turns it GREEN.

use manuk_text::FontContext;

const HTML: &str = r##"<!doctype html><html><body>
<div id="out">-</div>
<div id="host"></div>
<div id="other"></div>
<script>
  var R = [];
  function thrown(fn){ try { fn(); return 'NO_THROW'; } catch(e){ return e.name; } }
  var body = document.body;
  var host = document.getElementById('host');
  var other = document.getElementById('other');

  // ── WebIDL argument coercion (TypeError, before any DOM step) ──
  R.push('t1:' + thrown(function(){ body.moveBefore(null, null); }));                 // arg0 not a Node
  R.push('t2:' + thrown(function(){ body.moveBefore(document.createElement('span')); })); // 2nd arg missing
  R.push('t3:' + thrown(function(){ body.moveBefore({a:1}, null); }));                // arg0 plain object

  // ── Both destination and target must be connected (HierarchyRequestError) ──
  var conn = host.appendChild(document.createElement('div'));   // connected target
  var discon = document.createElement('div');                   // disconnected destination
  R.push('h1:' + thrown(function(){ discon.moveBefore(conn, null); }));               // parent disconnected
  R.push('h2:' + thrown(function(){ host.moveBefore(document.createElement('div'), null); })); // target disconnected

  // ── No cycle: moving a node into its own descendant (HierarchyRequestError) ──
  var p = host.appendChild(document.createElement('div'));
  var ch = p.appendChild(document.createElement('div'));
  R.push('h3:' + thrown(function(){ ch.moveBefore(p, null); }));                      // ancestor into descendant

  // ── Reference child must be a child of the destination (NotFoundError) ──
  var refOutside = other.appendChild(document.createElement('div'));
  var mover = host.appendChild(document.createElement('div'));
  R.push('n1:' + thrown(function(){ host.moveBefore(mover, refOutside); }));

  // ── The move actually happens: [a,b] → [b,a], and returns undefined ──
  var box = host.appendChild(document.createElement('div'));
  var a = box.appendChild(document.createElement('span')); a.id = 'a';
  var b = box.appendChild(document.createElement('span')); b.id = 'b';
  var ret = box.moveBefore(b, a);
  R.push('order:' + box.firstChild.id + '/' + box.firstChild.nextSibling.id);
  R.push('ret:' + (ret === undefined));

  document.getElementById('out').textContent = R.join(' ');
</script></body></html>"##;

#[test]
fn move_before_implements_the_atomic_move_and_its_pre_move_validity() {
    let fonts = FontContext::new();
    let page = manuk_page::Page::load(HTML, "https://move.test/mb/", &fonts, 800.0);
    let root = page.dom().root();
    let out = manuk_css::query_selector_all(page.dom(), root, "#out")[0];
    let got = page.dom().text_content(out);

    for (claim, why) in [
        ("t1:TypeError", "a non-Node first argument is a WebIDL TypeError"),
        ("t2:TypeError", "a missing second argument (both are required) is a TypeError"),
        ("t3:TypeError", "a plain object first argument is a TypeError"),
        ("h1:HierarchyRequestError", "a disconnected DESTINATION throws — the atomic-move connectivity rule insertBefore does not have"),
        ("h2:HierarchyRequestError", "a disconnected TARGET throws — same rule, the other side"),
        ("h3:HierarchyRequestError", "moving a node into its own descendant would cycle the tree"),
        ("n1:NotFoundError", "a reference child that is not a child of the destination throws NotFoundError"),
        ("order:b/a", "the move reorders [a,b] into [b,a] — moveBefore(b, a) puts b before a"),
        ("ret:true", "moveBefore returns undefined"),
    ] {
        assert!(
            got.contains(claim),
            "G_MOVE_BEFORE: expected `{claim}`\n  got: {got}\n\n  {why}."
        );
    }
}
