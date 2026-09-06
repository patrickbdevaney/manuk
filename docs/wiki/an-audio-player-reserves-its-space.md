# An audio player reserves its space

t1464 corrected the UA sheet: `audio { display: none }` unqualified hid the one form of the element
anybody ever sees, and Chrome's rule is `audio:not([controls])`. That fixed the computed value and
explicitly did not fix the box. The residue it recorded:

> ours is `0x17` against Chrome's `300x54`, because there is no audio control-bar widget with an
> intrinsic size.

⭐⭐ **That diagnosis was at the wrong level, and the wrong level is what made it look expensive.** A
widget was never the missing piece — `<video>` has no widget either and is `300x150`, because it is
an **atomic inline replaced** box and therefore takes CSS 2.1 §10.3.2's *default object size*.
`<audio controls>` was an ordinary inline, so no default object size could ever apply to it and the
`17` was just a line box. The fix is three predicate entries, not a widget.

```text
                           Chrome         before          after
  <audio>                  none 0x0       none 0x0        none 0x0       ✓ (t1464)
  <audio controls>         inline 300x54  inline 0x17     inline 300x54
  <video>       CONTROL     inline 300x150 inline 300x150  inline 300x150 ✓

  local clickability       100.0% (0 missed of 477)  — unchanged
  WPT html/semantics/embedded-content   863 -> 863   0 fixed / 0 new
      css/css-display                   211 -> 211   0 fixed / 0 new
      css/css-sizing                   1329 -> 1329  0 fixed / 0 new
```

## Three details that keep it scoped

**`<video>` is the row that keeps the size honest.** An audio control bar is *not* the shared
`300x150` default object size — Chrome draws it at `300x54`, and taking the shared default would
reserve nearly three times too much height. Both numbers are asserted so a later tick cannot collapse
them into one constant.

**The bare `<audio>` row is what keeps the change narrow.** `is_atomic_inline_replaced` tests
`display: Inline`, and the UA sheet makes a bare `<audio>` `display: none`, so only the `controls`
form is ever reached.

**`audio` is deliberately absent from `is_replaced_element`.** Like `iframe` / `object` / `embed` it
is atomic in a line without taking §10.4's proportional constraint adjustment, because a control bar
has no aspect ratio. The two lists differ on purpose and this one respects that.

## What it cost to find, which is the reusable part

Four attempts, each aimed at a different layer, and only the last was the right one:

| attempt | why it did nothing |
|---|---|
| the intrinsic-size function's tag list | never called for `<audio>` |
| its `is_audio` height arm | same — dead code, reverted rather than shipped |
| `default_object_tag` | reached only by boxes that are already replaced |
| `is_atomic_inline_replaced` | ✅ the gate that decides whether any of the above applies |

⚠ **The first two were reverted rather than left in.** An edit that changes no behaviour is not
harmless: it is machinery a later reader has to explain. When a fix does not move the number, remove
it before trying the next layer — otherwise the shipped change contains three guesses and one
mechanism.

See also [[a-lockstep-gate-scoped-to-one-value]].
