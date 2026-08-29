# `line-height` in the MinimalCascade — the property a `font-size` declaration threw away

> Landed t1361. Gate: `font_size_keeps_an_inherited_line_height_and_the_font_shorthand_sets_one`
> (`engine/css`). Every number below is headless-Chrome-measured via `getComputedStyle` in a
> `16px/24px monospace` context; the shipping (Stylo) cascade agrees with Chrome on every row.

## The one-sentence mechanism

> **Two ways to set the same thing, and this cascade got both wrong in opposite directions** —
> `font-size` re-derived an *inherited* `line-height` from the new size, and the `font` shorthand set
> nothing at all.

## The defect

```rust
"font-size" => {
    s.font_size = values::resolve_font_size(v, parent_fs).unwrap_or(s.font_size);
    s.line_height = s.font_size * 1.2;      // ← unconditional
}
```

`line-height` is inherited **as a computed length**. A child that changes its font size does not get
a new line box; it gets the one its ancestor authored. This line threw that away:

```text
  <div style="line-height:24px"><div style="font-size:40px">A</div></div>
     Chrome 24      Stylo (shipping) 24      MinimalCascade 48
```

A `line-height` in px with a differing font-size somewhere under it is most of the web. Priced on
the corpus before building: **34 of 39** sampled CrUX sites declare a `line-height` below 1.2
(`line-height: 1` alone appears 163 times), and **13 of 39** use the `font` shorthand.

The fix is one condition, and the flag it needs already existed for exactly this reason —
`line_height_normal` is inherited beside the value, and its comment says why:

> *"The FLAG is inherited with the value. Inheriting the number but not 'was this authored?' means a
> child re-derives its line box from the font while its parent uses the author's."*

⚠ **The naive fix is a different bug, and it has its own pinned row.** "Never touch `line-height` in
the `font-size` arm" breaks `normal`: a `line-height: normal` block whose font grows to 40px must
get a *bigger* line box (Chrome measures 46). `normal` is the font's own metric and must keep
tracking the font; an authored length must not. That is precisely the question the flag answers, and
it is why the same guard is applied to the UA font-scaling path (an `<h1>` inside a
`line-height: 24px` block keeps the authored 24 — the UA sheet scales the FONT, it does not
re-author the leading).

## The other entrance — the `font` shorthand was not implemented

`apply_declaration` had no `"font"` arm, so `font: 16px/24px monospace` set **nothing**: the element
kept the default size, the default family and the default leading. This is the t1353/t1355 shape —
one property, two entrances, and only one of them wired.

```text
                                           size   line-height   weight   style
  font: 20px/30px monospace                20px      30px         400    normal
  font: italic bold 20px/30px monospace    20px      30px         700    italic
  font: 20px monospace                     20px      NORMAL       400    normal   <- reset
  font: bold 20px/1.5 monospace            20px      30px         700    normal
  font: 16px/1.5 monospace                 16px      24px         400    normal
```

⭐ **Row 3 decides the design: an OMITTED `line-height` means `normal`, not "keep what was
inherited".** The shorthand resets every longhand it can carry before applying the ones present. A
shorthand that only sets what it mentions leaves the inherited 24px in place and is wrong on the
commonest spelling of all.

⚠ The reset can only be *observed* on a row that sets a weight **before** the shorthand — the
initial value and "reset to the initial value" are the same number, so asserting `400` on a row that
never set bold proves nothing. The gate varies declaration order on one rule and Chrome arbitrates:
`font-weight:bold; font-style:italic; font:20px/30px monospace` → 400/normal, and the reverse order
→ 700/normal.

⚠ The size token is **found**, not counted: it is the first whitespace-separated token whose leading
part parses as a length or font-size keyword, which separates it from the
`<font-style> || <font-variant> || <font-weight> || <font-stretch>` block in front without
enumerating that block. Everything after it is the family list, returned as raw text because a
family name may contain spaces.

⚠ The system-font keywords (`caption`, `icon`, `menu`, `message-box`, `small-caption`, `status-bar`)
are a different production — they *name* a platform font rather than describing one — and are left
untouched. That guard is **defence in depth**, and the gate says so: deleting `menu` from the list
leaves its row green, because `split_font_shorthand` refuses a value with no length token one layer
down.

## ⚠⚠⚠ Why a MinimalCascade-only bug is more than a rendering bug

The shipping browser cascades through Stylo, and Stylo was right on every row. But
**`MinimalCascade` is the cascade every layout gate runs on**: `engine/layout`'s 191 unit tests and
everything under `agent/tests` take `manuk-page` with default features. A cascade that disagrees with
its twin means those gates silently measure a different page from the one the browser renders.

This was found the expensive way. t1360's table gate, written with `font: 16px/24px monospace`, read
a **54px** row where Chrome and Stylo both read **57**. The confound was designed out of the fixture
(longhands instead of the shorthand) so the gate could land, and the cause was chased down here the
next tick. The code already states the rule this violates, in the float half of the same file:

> *"a cascade that disagrees with its twin about whether an image is out of flow is the `<source>`
> bug again"*

## ⚠⚠ And the instrument finding that came with it

`scripts/verify.sh` runs its crate suites as bare `cargo test -q -p <crate>` — **no features**. So
for `manuk-css` the entire `stylo_engine` test module is `#[cfg(feature = "stylo")]`-ed out of the
wall, including t1358's `multicol_longhands_survive_the_stylo_cascade`, which that tick landed
specifically because it is *"the second entrance, the door every real page comes through"*. It has
never run in a wall.

That is the third instance in three ticks of the same class — t1360 found `g_table_cell_valign` red
for twenty-three days because `manuk-page` gates run only when `verify.sh` names them individually.
`scripts/` is observer-owned, so this is **recorded, not worked around**: this tick's gate is placed
in `manuk-css`'s default-feature test module, where the wall already looks.
