# Two objects called `HTMLElement.prototype`, and `super.x()` goes through the empty one

> Landed t1407. Gate: `a_custom_element_can_call_super_dot_getattribute`
> (`engine/page/tests/g_htmlelement_prototype_join.rs`), 11 rows, red under 4 mutations.

The custom-elements shim gives the `HTMLElement` **constructor** a fresh prototype on purpose: an
upgrade grafts members onto the host object, because a reflector's prototype cannot be swapped. So
there are two objects entitled to the name, and only one of them is in the chain elements use:

```text
  a <div>:  instance -> __protoHTMLElement -> Element.prototype -> Node.prototype -> EventTarget.prototype
            ...and globalThis.HTMLElement.prototype IS NOT IN IT
```

Everything the engine has spent ticks adding — `currentSrc`, `complete`, `naturalWidth`,
`checkValidity`, `showModal`, the popover four — went on `__protoHTMLElement`. `class X extends
HTMLElement`, `super.<method>()`, and every `'feature' in HTMLElement.prototype` detection read the
other one.

```text
                                                    chrome     before
  this.getAttribute('data-x')      inside a CE      'hello'    'hello'
  super.getAttribute('data-x')     ← THE IDIOM      'hello'    THREW: not a function
  super.setAttribute / closest / addEventListener   work       THREW
  typeof HTMLElement.prototype.getAttribute         function   undefined
```

`super.<method>()` is the ordinary way a custom element extends a DOM method. It throws inside
`connectedCallback`, so **the element never upgrades and its whole subtree stays inert.**

## ⭐⭐⭐ The fix is a JOIN, not a MIRROR — and two derived attempts failed first

1. **Forwarders derived from a probe element** installed on `__HP`. `__HP.getAttribute` became a
   function and `super.getAttribute` still threw.
2. **A derived mirror of `__HP`'s own names** onto the constructor's prototype. Also failed, and the
   reason is the point: `getAttribute` is not `__HP`'s own property — it lives on `Element.prototype`,
   two links further up, so `'getAttribute' in __HP` is `true` and no copy of `__HP`'s own names can
   carry it.

> **The constructor's prototype was not missing a list. It was missing a chain.** One
> `Object.setPrototypeOf(HTMLElement.prototype, __protoHTMLElement)` gives it every member of `__HP`,
> `Element.prototype`, `Node.prototype` and `EventTarget.prototype` at once — including everything
> added after that line ever runs.

Two *derived* lists failed where one link succeeded: **derived was not the property that mattered.**
A cycle guard walks first — if `__HP` ever chains through the constructor's prototype, joining them
makes an infinite chain and every property lookup on every element hangs, a Bar 0 from one line.

t1395 hit the same wall for `'popover' in HTMLElement.prototype`, hand-mirrored eight names, and wrote
the residue note itself: *"the two prototypes being different objects at all is a broader divergence
than this tick."* Correct, never actioned, and the eight-name list was the [plural-asserts-a-sample
shape](../loop/WEB-PATTERNS.md). It is deleted.

## ⭐⭐ The control row: both apparent WPT movements were the STORED number

```text
                  WPT-AREAS.tsv   measured, join OFF   measured, join ON
  dom                      8170                 8173                8173
  html/dom                56454                56451               56451
```

The same binary with and without the change gives identical results. **A fresh number diffed against a
stored one attributes someone else's work — or the instrument's drift — to this tick.** Only the
same-binary control can say what a change did.

## Named non-claims, measured

* `HTMLElement.prototype.getAttribute.call({}, 'x')` throws `TypeError: Illegal invocation` in Chrome
  and returns quietly here — the native binding does not brand-check its `this`. Pre-existing.
* `'checkValidity' in HTMLElement.prototype` now reads `true`; Chrome says `false` (a form-control
  member). The engine already put form-control members on the shared `__protoHTMLElement`, so
  `'checkValidity' in div` was true before; the join only makes the two objects agree with each other.
