# `scrollIntoView` honours block and inline alignment

> Landed t1434. Gate: `scroll_into_view_honours_block_and_inline_alignment`
> (`engine/page/tests/g_scroll_into_view_alignment.rs`), six rows, red under three mutations.
> `css/cssom-view/scrollintoview.html` **0/40**; the area **1074 → 1118 (+44)**.

## The defect

`el_scroll_into_view` ignored its argument entirely and pushed the element's document top-left as the
scroll request. That is `{block: "start", inline: "start"}` — and it is **not the default**:

```text
  arg                     block     inline
  omitted / undefined     start     nearest
  true                    start     nearest
  false                   end       nearest
  {block:…, inline:…}     per-key overrides, defaulting as above
```

So even `el.scrollIntoView()` with no argument was wrong on the horizontal axis.

## Why the no-argument form could not be right by accident

⭐ **`"nearest"` is the only alignment that needs the CURRENT scroll position.** It scrolls the
*minimum*: nothing at all if the box already fits on that axis, otherwise just enough to bring the
nearer edge in. Two calls that differ only in where the page was already scrolled must give different
answers — which is the pair `omit_tl` / `omit_right` in the gate, and the reason a fix that hard-codes
either edge fails one of them.

It is also the alignment an agent wants by default: it brings the target into view **without throwing
away the reader's context**.

```text
  WPT's fixture: a 200x200 box in a `padding: 4000px` body, viewport 800x720
                              want x/y      before
  scrollIntoView()  @ 0,0      3400/4000   4000/4000   ← inline `nearest`, not `start`
  …                @ 12000,0   4000/4000   4000/4000   CONTROL — nearest picks the OTHER edge
  scrollIntoView(false)        3400/3480   4000/4000   ← block `end`
  {block:center,inline:center} 3700/3740   4000/4000
  {block:start,inline:start}   4000/4000   4000/4000   CONTROL — the old behaviour, when asked for
  {block:end,inline:end}       3400/3480   4000/4000
```

The result is a REQUEST, exactly as `window.scrollTo` is — the host owns the viewport and clamps —
with the same optimistic `SCROLL` write and `SCROLL_SEQ` bump, so `window.scrollY` reads back
synchronously on the next line and the next geometry read lays out against it.
