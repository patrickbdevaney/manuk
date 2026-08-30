# A name fragment hidden by a CSS RULE — the prune that only read the `style=` attribute

> Landed t1379. Gate: `a_name_fragment_hidden_by_a_stylesheet_is_not_announced`
> (`agent/tests/g_ax_name_hidden_by_stylesheet.rs`), ten rows, Chrome-measured through CDP
> `Accessibility.getFullAXTree`. Found by surface audit #79's ranked #1.

## The defect

accname §4.3 step 2A prunes a hidden node from the accessible name. The engine's prune called
`inline_visibility`, which parses the element's own **inline `style=` attribute** and nothing else.
Its doc-comment named the gap honestly — *"a `display:none` applied by a CLASS is still missed"* —
and the gap was real:

```text
  <style>.h { display: none }</style>
  <button>Save <span class="h">SECRET</span></button>

                             chrome        before          after
    the button's name        "Save"        "Save SECRET"   "Save"
```

## ⭐⭐⭐ Why nothing in the tree could find it

```text
  every hidden-node fixture in WPT accname/name/comp_labelledby_hidden_nodes.html
                                                       style="display: none"   INLINE
  G_AX_NAME_COMPUTED_STYLE's own `t_hidden` CONTROL row style="display:none"   INLINE
  the five a11y gates in agent/tests/ built on manuk_html::parse
                                                       NO CASCADE AT ALL
```

> **A rule with two sources, where the weaker source is the one every test uses, is invisible to the
> whole suite.** Visibility toggles are authored in stylesheets on the real web and inline in
> conformance fixtures, so a conformance-shaped test can sit at 100% on a mechanism that is wrong on
> every page.

And the receipt says so out loud: after the fix, WPT `accname` reads **438/484 = 90.5%, unchanged to
the subtest**. The number could not move, because the suite has no fixture that exercises the
spelling the web uses.

## The fix — the map was already in the context

t1365 threaded `NameStyles` (per-node computed `display` + `text-transform`) into the name walk so a
non-inline child could contribute a separator. **`display: none` was in that same map the whole
time and the prune never asked it.** `node_visibility` now prefers the computed pair and falls back
to `inline_visibility` when there is no style map (a `manuk_html::parse` fixture, a unit test).
`NameStyles` gained `visibility` for the other half of the rule, and — a third fact on a positional
tuple destructured at five sites — became a named struct, `NameStyle`.

⭐ **`visibility` is inherited and undoable, and reading the computed value gets the undo for free.**
`visibility: hidden` does not prune; `visibility: visible` inside it is announced. The inline reader
had to return `None` for *"not declared here"* and flow a flag down by hand; the cascade has already
resolved it, so every node in the map answers `Some(_)` and the `visible` state is simply read.

## The battery — Chrome via CDP `Accessibility.getFullAXTree`

```text
                                                        chrome    before      after
 b1  .h{display:none}            STYLESHEET             "Save"  "Save SECRET" "Save"
 b2  style="display:none"        inline        CONTROL  "Save"  "Save"        "Save"
 b3  .h{visibility:hidden}       STYLESHEET             "Save"  "Save SECRET" "Save"
 b4  style="visibility:hidden"   inline        CONTROL  "Save"  "Save"        "Save"
 b5  aria-labelledby → a display:none span (stylesheet) "foo bar"  same       same
 b6  stylesheet none, child display:inline               "Save"  "Save SHOWN" "Save"
 b7  the `hidden` ATTRIBUTE                             "Save"  "Save"        "Save"
 b8  stylesheet visibility:hidden, child visible   "Save SHOWN"  "Save SHOWN" same
 b9  aria-hidden                               CONTROL  "Save"  "Save"        "Save"
b10  class none + inline display:inline        CONTROL  "Save SHOWN"   same    same
```

⭐ **`b6` and `b8` are the pair that make this one rule rather than two predicates.**
`display: none` PRUNES — a `display: inline` child inside it is still gone. `visibility: hidden`
does not — the child undoes it. Getting that backwards either loses text the user can read or
announces text they cannot.

⭐ **`b5` was the right answer for the wrong reason.** A referenced node that is itself hidden is
EXEMPT (§4.3 step 2A): its text is what the author pointed at. Before this tick the engine could not
see that the span was hidden at all, walked it as an ordinary visible node, and arrived at the same
string. Two errors that cancel — asserted now so the cancellation cannot come apart.

⭐ **`b7` looked like the DOM-reader control and is not, and this gate's own vacuity assert caught
it on the first run.** The UA sheet carries `[hidden] { display: none }`, so the `hidden` attribute
IS a computed `display: none`; b7 is a control for the two sources *agreeing*. `b9` (`aria-hidden`,
which no stylesheet can express) is the control for the DOM reader alone, and `b10` — a class saying
`none` beaten by an inline `display: inline` — separates *reading the computed value* from
*"either source says none"*, which is the wrong fix an implementer reaches for first.

⚠ Chrome's CDP `name.value` for `b3`/`b4`/`b9` is `"Save "` **with a trailing space**: the space in
`"Save "` survives when the following fragment contributes nothing. accname §4.3 step 2 trims the
total and `normalize` does, so the gate asserts the trimmed string.

## How it was proven red

- **N1** — `node_visibility` returns `inline_visibility` unconditionally (the pre-tick behaviour):
  b1, b3, b6 fail and all four controls stay green, which identifies the mechanism as *which source
  is consulted* rather than *the prune is broken*.
- **N2** — report `display` but always `Some(true)` for visibility: only b3 fails.
- **N3** — let `display: none` flow down as a flag instead of returning: only b6 fails.

## Related

- `docs/wiki/accessible-name-computed-style.md` — t1365, which threaded the map this tick finally
  reads at the prune.
- `docs/loop/SURFACE-AUDIT.md` #79 ranked #1 — the sweep that aimed this tick.
