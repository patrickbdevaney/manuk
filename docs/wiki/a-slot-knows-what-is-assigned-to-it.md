# A slot knows what is assigned to it

*Tick 1483. `<slot>` was an element and a name in an interface list, and nothing a component could call.*

## The gap

```text
                         Chrome     before
  assignedElements()     s1+s2      MISSING (not a function)
  assignedNodes()        2          MISSING
  the DEFAULT slot       d1+d2 / 3  MISSING
  el.assignedSlot        "a"        null
```

`assignedElements()`, `assignedNodes()` and `element.assignedSlot` are how **every** web-component
library reads its own light-DOM children — Lit, Stencil, FAST, and every hand-rolled
`connectedCallback`. All three were absent, so the call was a `TypeError` and took the rest of the
component's boot with it.

This is the top nameable row of the t1482 boot histogram: **eight** `assignedElements is not a
function` rejections on `meet.google.com`, a site rendering 1,522 of Chrome's 4,238 boxes. After:
**zero**.

## Computed, not stored

The spec's assignment is *derived* from the tree and re-derived on every mutation. A cached map has
to be kept correct across `appendChild`, `slot=` writes and shadow-root `innerHTML` — three places
that would silently drift apart. A host's children are a handful of nodes and the shadow root's slots
fewer still, so the walk is cheap and **it cannot go stale**. Same call the repo made for
`document.styleSheets` at t1479, for the same reason.

## Ten rows, arbitrated against headless Chrome, byte-identical

```text
  named=s1+s2  dupe=-  def=d1+d2  defNodes=3  slotOf=true  d1Slot=true
  z=-  zFlat=zfb  loose=-/0  nonSlot=threw
```

Every row is a real component idiom, not a spec curiosity:

* **`dupe=-` — a node goes to the FIRST slot of its name** (DOM §4.2.2.4). A second
  `<slot name="a">` gets nothing. Pages write that deliberately as a fallback region; assigning to
  every matching slot would render the same children twice.
* **`defNodes=3` vs `def=d1+d2` — text nodes count.** A component that checks
  `assignedNodes().length` to decide whether it has content — the commonest empty-state test there
  is — gets the wrong answer if text is dropped.
* **`z=-` but `zFlat=zfb` — `flatten` changes the answer only when the slot is empty**, in which
  case it yields the slot's own fallback. That is exactly the question a component asks to decide
  *"am I showing the default?"*.
* **`loose=-/0` — a `<slot>` outside a shadow tree assigns nothing**, and its own children are not
  "assigned" to it. A walk that stopped at "nearest parent" rather than "the shadow root" would
  answer with the whole document.
* **`nonSlot=threw`** — the methods live on the shared element prototype here, so the tag check is
  what stops `div.assignedElements()` quietly answering `[]`: a wrong answer of the right type, which
  no `try` catches.

## ⚠ Residue, named, and deliberately NOT shimmed

`slot instanceof HTMLSlotElement` is **`false`** where Chrome says `true`, and
`slot.constructor.name` is `HTMLElement`. This engine gives every element reflector the one
`HTMLElement.prototype`; per-tag reflector prototypes are their own piece of work (lever board T2b,
sized `[L]`).

A `Symbol.hasInstance` shim would make `instanceof` answer `true` in an afternoon **and it is not
done**. The prototype chain would still be wrong, so a page patching `HTMLSlotElement.prototype.foo`
still would not reach instances. That is *correct in the one channel a human checks* (t1282), a trap
this project has already been caught by.

⚠ `slotchange` does not fire. Assignment is computed on demand, so the answer is never stale — but
there is no invalidation point to hang an event on. Components that re-read on `slotchange` re-read
late; components that read in `connectedCallback` (the majority) are correct.

## The gate

`g_a_slot_knows_what_is_assigned_to_it` — the ten Chrome rows exactly, with **two** vacuity arms: the
fixture must complete, and `named=s1+s2` must be non-empty, because "assigns nothing" would otherwise
satisfy every suppression row in the list. Red under five mutations.
