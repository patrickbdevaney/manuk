# What `input.value` MEANS, as opposed to what is stored

> Landed t1390. Gate: `the_api_value_is_the_sanitised_one`
> (`engine/page/tests/g_input_value_sanitization.rs`).
> WPT `html/semantics/forms/the-input-element` 624/1452 → **866/1452**;
> `html/semantics` 4922 → **5183**.

## Found by surveying, not by grinding

`html/semantics` became the board's largest failing area the moment t1388 put it on the board. The
survey ranked its subtrees, then histogrammed the biggest one's failure MESSAGES:

```text
  html/semantics/forms/the-input-element        828 failing  (43.0%)   ← the largest
    …of which "input.value should be '…' after change of state"   280   ← ONE mechanism, 34%
  html/semantics/scripting-1/the-script-element 484
  html/semantics/forms/textfieldselection       385
  html/semantics/forms/constraints              338
```

> **An area percentage is not a work item; a message histogram is.**

## The mechanism

An `<input>` has two values: the **content attribute**, which keeps whatever was written, and the
**API value**, which is the content run through the *value sanitization algorithm* for the element's
current `type`. We had only the first.

⭐ **A paste from a spreadsheet carries a `\r`.** It came back out of `.value` with the carriage
return still in it, was submitted with it, and compared unequal to the same text typed by hand — a
bug that looks like a server problem from every angle except this one.

## The table, every row Chrome-measured, from `"  foo\rbar  "`

```text
  text · search · tel · password · hidden · checkbox ·
  radio · submit · image · reset · button      "  foobar  "   strip CR/LF only
  url · email                                  "foobar"       strip CR/LF, then TRIM
  date · month · week · time ·
  datetime-local · number                      ""             valid-or-empty
  range                                        "50"           valid-or-default, then CLAMP
  color                                        "#000000"      valid-or-black, LOWERCASED

  number  "50" → "50"      " 50 " → ""     ⚠ number does NOT trim
  range   "200" → "100"    "-5" → "0"      ⚠ range CLAMPS to [min, max]
  color   "#ABCDEF" → "#abcdef"            ⚠ and lowercases
  date    "2020-01-02" → "2020-01-02"      "nope" → ""
```

⭐⭐ **`number` does not trim and `url`/`email` do — that pair is the whole shape of the algorithm.**
It is not *"clean up the string"*; it is a per-type definition of what the string is allowed to BE,
and the two trimming types are the two whose values are conventionally pasted with surrounding
space. A single "trim and strip" implementation passes the text rows, the url rows and the email
rows, and is wrong on `number` — which is mutation N2.

⭐ **`range` is valid-or-DEFAULT, and the default is the MIDPOINT.** An unset range reads `50`, not
`0` and not `""`, which is why a slider rendered from `.value` before the user touches it has its
thumb in the middle. An empty string would put it nowhere. That is mutation N3.

## ⚠ Applied on READ, with one named consequence

Sanitising on read keeps the content attribute intact — so `getAttribute("value")` returns what the
author wrote while `.value` returns what the type means, **which is exactly the spec's split** — and
it makes a `type` change re-sanitise for free, because the next read asks the new type.

⚠ The divergence is a MULTI-STEP type change: the spec's sanitiser is destructive, so
`text → number → text` leaves `""` in Chrome where reading through the raw content gives the text
back. Measured, named, not built — it needs a separate dirty-value store, a different mechanism from
the algorithm itself.

## The receipt

```text
  WPT html/semantics/forms/the-input-element   624/1452 = 43.0%  →  866/1452 = 59.6%   (+242)
  WPT html/semantics (whole area)             4922/11264 = 43.7% → 5183/11262 = 46.0%  (+261)
```

## How it was proven red

- **N1** — return the raw content attribute: every row but the three "valid input survives" controls
  fails. That is the defect.
- **N2** — one "strip CR/LF then trim" for all types: the `strip_*` rows lose their surrounding space
  and `num_spaced` becomes valid, while url/email still pass.
- **N3** — `range` valid-or-empty instead of valid-or-default: `range_junk` reads `""`.
