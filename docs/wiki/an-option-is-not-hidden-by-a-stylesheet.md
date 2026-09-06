# An option is not hidden by a stylesheet

The Stylo UA sheet carried `option, optgroup { display: none; }`. So under that cascade a
`<select>` exposed **no options at all** — every dropdown on the web, missing from an agent's
perception, and `getComputedStyle(option).display` answering `none` where Chrome answers `block`.

## The comment directly above the offending line already recorded the fix

One paragraph earlier, in the same sheet, about `<source>`/`<picture>`:

> Hiding them here produced the right box and the wrong answer, and
> `getComputedStyle(source).display` is exactly what a responsive-image shim reads. The structural
> rule now lives in `layout::never_rendered`, where an author's `source { display: block }` cannot
> override it either.

An `<option>` is the same shape. It is not hidden by a stylesheet — a `<select>` **draws its own
text instead of its children** (`control_text`), which is a structural fact about the widget. **The
same bug survived one line below its own lesson.**

## And the two cascades disagreed, which is the failure the lockstep note warns about

`apply_ua_defaults`' `MinimalCascade` never listed `option`; only the Stylo sheet did. Its own
comment says why that matters:

> The two cascades disagreeing about which elements render at all is how a `<source>` ends up with
> 19px of height in one configuration and none in the other.

So the accessibility tree contained every dropdown's options under one build and none under the
other, and **which one you got depended on a cargo feature**. That is how the defect stayed
invisible: WPT runs Stylo and does not ask an a11y question; the agent asks the a11y question and
was running MinimalCascade.

## Measured

```text
                                            Chrome    before    after
  getComputedStyle(option).display           block      none     block
  the <select>'s height                       19px      19px      19px  ✓
  gap between the paragraphs around it        54px      54px      54px  ✓
  options in the accessibility tree              3         0         3

  WPT html/semantics/forms   1225 failing -> 1225   0 fixed / 0 new
  WPT css/css-display         211 failing ->  211   0 fixed / 0 new
```

**The two geometry rows are the whole risk and they do not move.** The rule was added for a real
reason the sheet still records — *"left as plain `inline`, the inline collector recurses into a
`<select>`'s `<option>`s and paints every one of them into the surrounding line — rust-lang.org's
language picker rendered as a row of twelve language names"*. The gate therefore measures the
select's height and the flow around it, not just the computed value: if options ever start
generating boxes, the gap moves from 54 to something much larger.

## What it unblocks, and what still blocks it

This was the **first** named blocker on giving `manuk-agent` the production cascade (t1461 refused
`stylo` because it deleted every dropdown's options). With it cleared,
`g_ax_tree_excludes_display_none` passes under Stylo — and a **second** blocker appeared:
`g_counter_set_and_pseudo_counters` fails there, because a pseudo-element's own `counter-increment`
is ignored on the Stylo path, so a counter never passes 9.

So `stylo` is still not enabled for the agent. The difference is that the list is now one item long
and named, instead of unknown. See [[a-crate-that-omits-a-feature-substitutes-an-engine]].
