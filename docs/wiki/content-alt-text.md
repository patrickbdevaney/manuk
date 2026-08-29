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
