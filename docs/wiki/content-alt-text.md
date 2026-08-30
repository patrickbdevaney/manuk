# `content: "drawn" / "announced"` — one declaration, two answers

> Landed t1369. Gate: `the_content_alt_half_is_announced_not_painted` (`agent/tests/`).
> Every number headless-Chrome-measured on the gate's exact fixture.

## The one-sentence mechanism

> **CSS Content 3 lets one `content` declaration say what is DRAWN and what is ANNOUNCED, and this
> engine painted the half meant to be announced** — on one cascade because a pref refused the
> declaration outright, and on the other because it never looked for the `/`.

```css
::before { content: "★" / "" }             /* draw a star, announce nothing */
::before { content: "" / counter(step) }   /* announce a step number that is not drawn */
```

Rendering the announcement is the exact opposite of what the author asked for, in both directions.

## ⭐⭐⭐ The fourth property found switched off — and the first one `longhands.toml` could not name

Stylo parses the syntax, but the `/` arm of its **value parser** carries

```rust
Token::Delim('/') if … && static_prefs::pref!("layout.css.content.alt-text.enabled") => …
```

With the pref off the whole declaration is an unexpected-token error, so the author's fallback line
wins — and where there is no fallback, the pseudo vanishes entirely.

t1358's rule was *"when a CSS feature looks absent, read `longhands.toml` for a `servo_pref` before
concluding anything about layout."* **That rule could not have found this one**: it is not a property
gate in a table, it is a `pref!` call site inside a parser. The rule generalises:

> ⭐ **The gates are `static_prefs::pref!` call sites ANYWHERE in stylo, not just `servo_pref` rows
> in `longhands.toml`.** A sweep of the crate finds **53** of them; this engine flips **six**.

The 47 unflipped ones include several real features — `layout.css.attr.enabled`,
`layout.css.relative-color-syntax.enabled`, `layout.css.at-scope.enabled`,
`layout.css.properties-and-values.enabled`, `layout.css.scroll-driven-animations.enabled`,
`layout.css.system-ui.enabled`, `layout.css.webkit-fill-available.enabled` — priced on the corpus at
`attr(` 14/39, `system-ui` 13/39, `color-mix(` 9/39, `@property` 5/39.

## ⚠⚠ And the sweep's second result, which keeps the rule honest

Two of the highest-priced were measured against Chrome **before** being flipped, and both are
**already correct with their prefs off**:

```text
  font-family: system-ui, "Hamburgefonstiv"     Chrome 131.61   ours 131.61
  font-family: monospace  (control)             Chrome 144.50   ours 144.50
  width: -webkit-fill-available in a 400px CB   Chrome 400.00   ours 400.00
```

> **An unflipped pref is not evidence that a feature is broken.** It is a place to look. A tick that
> flipped all 47 on the strength of the sweep would have been changing 47 behaviours on the evidence
> of one. Only the row with a measured divergence was flipped.

## The other cascade did not parse it at all

`MinimalCascade` handed the whole value to `parse_content_parts`, so `"before" / "alt"` rendered as
`beforealt`. Two cascades disagreeing about what a pseudo paints is the `<source>` bug this project
keeps re-finding (t1361 `font-size`/`line-height`, t1364 `border-spacing`), so both are fixed here.

⚠ The split is at a `/` **outside a quoted string**. `content: "and/or" / "and or"` has two, and
taking the first renders `and` (77.06 wide) instead of `and/or` (105.97) — that is mutation N2, and
`e3` is the only row that separates a correct split from `v.split('/')`.

## Chrome-measured

```text
  content: "before" / ""                 renders "beforelabel"   105.97   <- ours was 48.17
  content: "before" / "alt"              renders "beforelabel"   105.97   <- ours was 134.86
  content: "and/or" / "x"                renders "and/orlabel"   105.97
  content: "plain"   (no slash)          renders "plainlabel"     96.34   CONTROL
  no ::before at all                     renders "label"          48.17   CONTROL
```

## ⚠ NAMED, MEASURED, NOT BUILT

- **The alt text is not yet in the accessible NAME.** This tick makes the two halves *separable*;
  threading the alt half into `accessible_name` is the next tick — and t1365's own note applies, that
  a fourth fact through that walk should become a context struct rather than a fourth parameter.
  `accname` is flat at **432/484** across this change, deliberately.
- **White space at the pseudo/text boundary.** The WPT fixture writes `content: " before "` with
  deliberate outer spaces, and Chrome collapses them against the adjacent text: `" before " +
  "label" + " after "` is **18** characters wide in Chrome (173.4) and **20** here (192.7). Exactly
  two spaces, and a separate mechanism.
- ⚠ **The Stylo half of this fix is not gated where the wall can see it.** `manuk-agent` takes
  `manuk-page` with default features, so this gate runs on `MinimalCascade`; the pref flip was
  verified by direct measurement instead. That asymmetry is surface audit #78's finding — no wall
  runs a Stylo-path gate — and it is recorded rather than worked around.

---

## t1371 — the announced half reaches the NAME

t1369 (above) made the two halves *separable*. This is the other end: the announced half now **is**
the accessible name.

```text
  accname   432/484 (89.3%)  →  438/484 (90.5%)   +6, and ZERO newly failing
    button/heading/link name from fallback content with ::before and ::after       ×3
    button/heading/link name from fallback content mixing attr() and strings …     ×3
```

Verified by diffing the failing **name lists**: the `fallback content` rows are gone and nothing else
moved.

### ⭐ `Some("")` is a real answer, and that is the whole design

```text
  no `/` in the declaration   ->  the name falls back to the RENDERED text
  `/ "alt"`                   ->  the name is "alt"
  `/ ""`                      ->  the name is EMPTY — and must NOT fall back
```

`content: "★" / ""` means *draw a star, announce nothing*. Collapsing the last two cases — by storing
only non-empty alt strings, say — silently turns every decorative pseudo back into an announced one,
which is the exact request the empty alt was written to make. It is a three-way choice, not
`unwrap_or(rendered)` on a string, and the gate's N2 mutation fires on that row alone.

### ⭐⭐⭐ And the fourth fact became a context struct, because t1365 said it would

Three facts had been threaded through the accessible-name walk one parameter at a time — t1097's
`GeneratedText`, t1355's `NameIndex` widening, t1365's `NameStyles` — and **each one left a caller
behind**, twice in the same unit test, invisibly, because `manuk-a11y` is a suite in no wall (surface
audit #78). t1365's own note read: *"a fourth fact should become a context struct rather than a
fourth parameter — the signature already carries an `#[allow(clippy::too_many_arguments)]`."*

`NameCtx { generated, alt, styles }` replaces two parameters across eleven signatures. The win is not
tidiness: **adding a fifth is now a one-line change to the struct and its two construction sites**,
instead of an edit to twenty call sites where missing one compiles fine on every path but the one
that matters. A prediction the loop wrote down and then met on schedule.

### ⚠ NAMED, MEASURED, NOT BUILT — `attr()` in `content` on the other cascade

`content: "x " / "start " attr(data-alt) " end "` is named `"start MID end label"` by Chrome, and
**the shipping (Stylo) path agrees** — which is precisely why the three `mixing attr()` rows are among
the six fixed here. `MinimalCascade` gives `"start end label"`.

The cause is structural rather than an oversight: `ContentPart` has no `Attr` variant **by design** —
its own doc says an `attr()` is *"already resolved against the element (that one CAN be resolved in
the cascade — the attribute is right there on the element)"* — and Stylo's mapper does resolve it,
while `parse_content_parts` is a free function with no element in hand. Closing it means threading
the element into that parser or giving `ContentPart` a term layout resolves: a design decision, not a
line change.

The row is therefore left in the gate's fixture and **out of the asserted set** — asserting Chrome's
answer lands a RED gate, asserting MinimalCascade's pins the bug — with a vacuity assert keeping it
alive so the note is about something that exists. `attr(` prices at **14 of 39** corpus sites, the
highest row in t1369's pref sweep, so this is a ranked item rather than a curiosity.
