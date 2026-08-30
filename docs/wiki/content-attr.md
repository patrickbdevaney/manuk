# `content: attr(href)` — the term that met its element on one cascade and not the other

> Landed t1372. Gate: `content_attr_meets_its_element` (`agent/tests/`).
> Every number headless-Chrome-measured, `16px/24px monospace`.

## The one-sentence mechanism

> **The cause is a signature, not a missing feature.** `ContentPart::Text`'s own doc has always said
> an `attr()` is *"already resolved against the element — the attribute is right there on the
> element"*, and on the Stylo path it is. `MinimalCascade` could not: `apply_declaration` takes a
> `&Declaration` and a parent font size, **with no element in sight**.

`a::after { content: " (" attr(href) ")" }` — printing a link's target after its text — is the idiom
`attr()` exists for, and `attr(` is on **14 of 39** sampled CrUX sites: the highest-priced row in
t1369's sweep of stylo's pref gates.

The term now survives the value parser unresolved (`ContentPart::Attr`) and meets its element one
layer out in `cascade_node`.

## ⚠ The half-fix that would have passed a lesser gate

Resolving only the ELEMENT's own `content` fixes the case nobody writes. **`attr()` is almost never
on an element's `content` — it is on a pseudo's**, and a pseudo is cascaded by `cascade_pseudo` into
`s.before` / `s.after`, which are separate `ComputedStyle`s. The first version of this fix did
exactly that, and the `attr()` row of `g_ax_name_content_alt` stayed red until the pseudo path was
resolved too.

⚠ The pseudo is resolved **against its ORIGINATING element** — a pseudo has no attributes of its own.

## Chrome-measured

```text
  <a href="/docs">link</a>  with  ::after { content: " (" attr(href) ")" }        115.59
  <span data-x="VAL">x</span>     ::before { content: attr(data-x) }               38.55
  <span>x</span>                  ::before { content: attr(data-missing) }          9.64  NEGATIVE
  <span>x</span>                  ::before { content: "[" attr(data-missing) "]" }  28.91
```

⚠ **The last two rows are CSS 2.1 §12.2, and they are why a miss is the EMPTY STRING rather than a
dropped term.** Row 3 is 9.64 — one character, the `x` — so a missing attribute contributes nothing
*visible*. But row 4 is 28.91, three characters: **the literals around it still render.** An
implementation that drops the whole declaration on a missing attribute passes row 3 and fails row 4 —
and `a::after{content:" ("attr(href)")"}` on an `<a>` with no `href` is exactly that case.

## ⚠ What this does and does not buy

```text
  accname   438/484  FLAT — the Stylo path already resolved attr(), which is why t1371's three
                     `mixing attr() and strings` rows passed on it
```

Like t1361 (`font-size` clobbering an inherited `line-height`) and t1364 (`border-spacing` in one UA
sheet), this is a **`MinimalCascade`-only** divergence. Its user-visible value is the
`--no-default-features` build; its larger value is instrument fidelity, because `MinimalCascade` is
the cascade `engine/layout`'s 191 unit tests and everything under `agent/tests/` run on — so a
fixture using `attr()` measured a different page from the one the browser renders.

That is the twin-drift class for the **fourth** time in a week — t1361, t1364, t1369, now this — and
the rule it keeps proving is the one `engine/css/src/lib.rs`'s float half already states: *a cascade
that disagrees with its twin is the `<source>` bug again.*

⭐ The four instances share a shape worth naming: **each was found by a gate placed on the harness
that has the WEAKER cascade.** None of them was visible from the Stylo path, and none would have been
found by measuring the shipping browser against Chrome. The gates that catch them are the ones in
`agent/tests/` — which is where they go anyway, because that is where the wall looks (surface audit
#78).
