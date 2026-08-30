# `counter-set`, and the counter properties a pseudo was never asked about

> Landed t1374. Gate: `a_pseudo_s_counter_actions_run_and_counter_set_exists` (`agent/tests/`).
> Every expected value headless-Chrome-measured.

## Two defects in the same walk, both in the construct authors actually write

```css
li::before { counter-increment: item; content: counter(item) "." }   /* the numbered list */
```

`counter_snapshots` read `counter-reset` / `counter-increment` from the **ELEMENT only**, so a
pseudo's own counter actions were ignored entirely and every item rendered the same number. And
`counter-set` — CSS Lists 3's third action — was not implemented on either cascade.

## ⚠ A third shape in the "why is this property missing" family

`counter-set` is `engine = "gecko"` in stylo's `longhands.toml`: it does **not exist** in the servo
build, so there is no `clone_counter_set()` and **no pref that brings one back**. It is recovered
from `MinimalCascade` the way `appearance` and `field-sizing` already are — onto the element **and
onto its pseudos**, because the pseudo is where it is written. (Recovering onto the element alone is
the same half-fix t1372 made with `attr()` one tick earlier, in the same file.)

That completes a family of three, distinguished by where the gate lives:

```text
  servo_pref row in longhands.toml           t1358  multicol            — a pref flip reaches it
  static_prefs::pref! inside a value parser  t1369  content alt-text    — a pref flip reaches it
  engine = "gecko"                           t1374  counter-set         — NO flag reaches it
```

## ⭐⭐⭐ Two spec claims written from memory, both measured wrong

The first draft of this code asserted, in prose, with a worked example:

> *"the three run **reset, then set, then increment** … `counter-reset: c; counter-set: c 5;
> counter-increment: c` ends at 6"* — and *"`counter-set` **assigns** to a counter that already
> exists and does nothing if it does not."*

Both are false, and one Chrome measurement each settled it:

```text
  counter-reset: a 0; counter-set: a 99; counter-increment: a 1   renders TWO digits (99)
                                                                  — not three (100)
  counter-set: b 99   with no reset anywhere                      renders TWO digits (99)
                                                                  — not one (0)
```

So the order is **reset → increment → SET**, and `counter-set` **creates** a counter that does not
exist. The reset/set distinction is about **scoping** — `counter-reset` opens a new nested counter —
not about whether the value lands.

> ⭐ **A spec sentence recalled is a hypothesis; the fixture is the test.** Both wrong claims were
> already written into a doc comment with a worked example before either was measured. The example
> is what made them checkable.

## Reading a counter when the engine will not show you one

Chrome does not expose a pseudo's *resolved* text to script — `innerText` omits generated content and
`getComputedStyle(el, '::before').content` returns the specified value, `counter(x)` and all. So a
rendered counter is read here as **the element's width in a `max-content` box** at `16px monospace`
(9.639 px/char), and **every row is chosen so a wrong answer changes the DIGIT COUNT**:

```text
  a 12-item list, li::before{counter-increment:item; content:counter(item) "."}
    items 1-9    28.92  ("1.x")      items 10-12  38.55  ("10.x")
    if the pseudo's increment were ignored, EVERY item is "0.x" and item 10 reads 28.92

  reset a 0 · set a 99 · increment a 1     28.91  ("99y")   <- 100 would be 38.55
  set b 99, no reset                       28.91  ("99y")   <- 0 would be 19.28
  reset c 0 · set c 99                     28.91  ("99y")   <- 0 would be 19.28
```

⚠ An earlier version of this fixture used single-digit values, and every wrong answer was the same
*width* as the right one. **The digit boundary at 10 is the whole reason the list has twelve items.**

## ⚠ NAMED, MEASURED, NOT BUILT — the nine accname rows this did NOT fix

`accname` is flat at **438/484**. The nine `alt counter` subtests use
`content: "" / counter(cnt)` — a counter in the **alt** half — and stylo 0.19's value parser accepts
`counter()` only *before* the alt marker:

```rust
"counter" if alt_start.is_none() => input.parse_nested_block(…)
```

so the whole declaration is an unexpected-token error and the pseudo does not exist at all. Verified
by narrowing: `content: "" / "y"` works, `content: counter(cnt)` works, `content: "" / counter(cnt)`
produces **no pseudo**. That is a limit of the vendored dependency, not of this engine, and closing
it would mean recovering the pseudo's whole `content` from `MinimalCascade` when stylo produced none
— a much larger change than recovering one property onto an existing pseudo.
