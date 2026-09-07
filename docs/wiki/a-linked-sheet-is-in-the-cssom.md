# A linked sheet is in the CSSOM

`document.styleSheets` was built by scanning `getElementsByTagName('style')`, so every
`<link rel=stylesheet>` was invisible to it and `<link>.sheet` was `undefined`. Measured against
Chrome on a sheet that demonstrably changes a colour:

```text
                                   Chrome              before        after
  document.styleSheets.length      2                   1             2
  …their ownerNodes                [STYLE, LINK]       [STYLE]       [STYLE, LINK]
  the linked rule's effect         color: rgb(1,2,3)   rgb(1,2,3)    rgb(1,2,3)    ✓ unchanged
  link.sheet                       object              undefined     object
  a rel=preload link's .sheet      null                undefined     null

  WPT css/cssom   593 failing -> 591   (2 real subtests, 0 regressions)
```

⭐⭐ **The third row is why this stayed invisible.** The rules were in the cascade the whole time —
pages rendered correctly — and absent only from the *view* of them. Every theme switcher, CSS-in-JS
runtime and `sheet.disabled` toggler iterates `document.styleSheets`, and on any page whose CSS is
external they saw an inline-only list and took their no-stylesheets branch.

## The text was never missing

t665 built `<style>.sheet` from the element's own text and recorded the gap as deliberate scope:
*"`<link>.sheet` stays `undefined`… for an applied linked sheet `null` would be a lie."*
`Page::external_css` has held the fetched text all along, keyed by resolved URL, and `link.href` is
already absolute — so the whole missing piece was a way for the prelude to reach it. That is one
binding (`document.__manukSheetText(href)`) plus the publish channel `set_grid_tracks` /
`set_scroll_geometry` already established.

## Four details that are easy to get subtly wrong

**Reuse the builder, do not write a second one.** The sheet is built from a *detached* `<style>` shim
carrying the fetched text, so it goes through the same `__makeSheet` as the inline path. Two parsers
for one grammar is how the two disagree later.

**`ownerNode` is the `<link>`, not the shim** — that is what Chrome reports and what a consumer walks
back to.

**Order is document order.** Chrome reports `[STYLE, LINK]` for a `<style>` written above a `<link>`.
One `querySelectorAll('style, link[rel]')` gives that for free; concatenating two
`getElementsByTagName` results would group by tag and silently reorder every page.

**A link without a sheet is `null`, not `undefined`.** `HTMLLinkElement.sheet` is `CSSStyleSheet?`,
and the standard guard `if (el.sheet === null)` is FALSE against `undefined` — the false-presence trap
t663 measured on `<style>`, which would otherwise have been reintroduced one tag over. The
`rel=preload` row is that assertion.

⚠ **`HTMLLinkElement` is not a global here**, so this cannot live on its own prototype — the
`HTMLElement.prototype` getter would shadow it. One getter, two tags.

## The gate that predicted this tick

`g_cssom_sheet_bridge` pinned `T__linksheet_undefined` with an instruction: *"This bridge is
`<style>`-only by decision… **If linked sheets landed, update this gate.**"* It fired, and was updated
as written. **Third time this session** a gate has named its own scope limit and thereby told the next
tick it had succeeded — after t1459's overlay row and t1465's peer row.

⚠ And a note on reading the WPT diff: the raw name-list diff showed **18 fixed / 16 new**, which looks
like churn-with-regressions. It is neither — the entries are `@IMPORT SHEET FAILED` **WARN log lines
carrying an ephemeral port number** (`:42431` vs `:46419`), so the same line reads as both fixed and
new across runs. Keyed on subtest name alone: 2 fixed, **0 new**.

See also [[the-base-url-was-the-disagreement]], [[cssom-views-and-terminators]].
