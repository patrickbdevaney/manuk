# `validity` describes the VALUE; `willValidate` says whether anyone will act on it

> Landed t1391. Gate: `validity_describes_the_value_and_will_validate_says_who_acts_on_it`
> (`engine/page/tests/g_constraint_validation.rs`).
> WPT `html/semantics/forms/constraints` 600/915 → **785/915**; `html/semantics` 5183 → **5314**.

## ⭐⭐⭐ The central defect — one early return conflating two questions

`__computeValidity` returned an **all-false** `ValidityState` for any element barred from constraint
validation. Chrome-measured, a DISABLED `pattern="[a-z]+"` input holding `"123"`:

```text
  willValidate      false      it is barred — nothing will act on it
  patternMismatch   TRUE       the VALUE is still wrong, and the object still says so
  valid             false
  checkValidity()   true       the method asks "will this block submission", not "is this good"
```

**All four at once**, and every validation library — the browser's own native UI, React Hook Form,
Formik, VeeValidate — reads exactly that combination to decide whether to draw its own message.
Collapsing them loses the only signal it has.

## The six rules, every row Chrome-measured

```text
 1  the flags are computed even when BARRED                    disabled+pattern → pm TRUE
 2  …EXCEPT valueMissing, which needs the element to be MUTABLE
                      <fieldset disabled> required empty  → vm FALSE, valid TRUE
                      readonly required empty             → vm FALSE
                      inside <datalist>, required empty   → vm TRUE  (barred but MUTABLE)
 3  checkValidity() is TRUE for a barred element, whatever `valid` says
 4  tooLong / tooShort need the DIRTY VALUE flag — only the USER sets it
                      <input maxlength=2 value="abcdef">  → tooLong FALSE
                      …and still FALSE after el.value = 'xyzxyz'
 5  `pattern` applies to six types only (text search url tel email password)
                      <input type=number pattern="[a-z]+" value="123"> → pm FALSE
 6  min/max apply to the TEMPORAL types too, compared LEXICOGRAPHICALLY
                      type=date  min=2020-01-01 value=2019-06-01 → rangeUnderflow TRUE
```

⭐⭐ **Rule 2 is the row that proves rule 1 is about the OBJECT, not the element.** Two disabled
controls disagree: the one with a pattern mismatch reports `valid: false`, the one that is merely
required-and-empty reports `valid: true`. That is only possible if the mutability clause lives on
`valueMissing` — where the spec puts it — rather than on the whole computation.

⭐ **Rule 4 is a rule about who typed, not about how long the string is.** `maxlength` does not make
a value invalid; it stops the USER typing past it. A value from the markup or from a script is
over-length and VALID. There is no user typing in this engine yet, so both flags are currently
unreachable — **and that is the correct answer, not a missing feature.**

⭐ **Rule 6's comparison is lexicographic on purpose.** `YYYY-MM-DD`, `YYYY-MM`, `YYYY-Www`, `HH:MM`
and `YYYY-MM-DDTHH:MM` are fixed-width and zero-padded, so string order IS chronological order.
`parseFloat` on a date gives `2019`, so a month-early value compares by year and a `type=time` value
does not compare at all.

⚠ **`disabled` inherits from an ancestor `<fieldset disabled>` and the IDL attribute does not
reflect it** — `el.disabled` is the element's OWN attribute. Disabling a whole step of a form with
one fieldset is the idiomatic way to do it, so reading only the control's own attribute leaves every
control in that step validating. (Same defect, same shape, as the a11y `disabled` state at t1387 —
one HTML rule, two subsystems, each having to learn it.)

## The receipt

```text
  WPT html/semantics/forms/constraints  600/915 = 65.6%  →  785/915 = 85.8%   (+185)
  WPT html/semantics (whole area)      5183/11262 = 46.0% → 5314/11263 = 47.2% (+131)
```

⭐ Together with t1390's sanitiser, `constraints` has gone **577 → 785 (+208)** in two ticks: the
validity algorithm reads the SANITISED value, so fixing the value first made the validity fix
land against the right input.

## How it was proven red

- **N1** — restore the barred early return: `a` reads `pm=false/valid=true` and `k` reads `vm=false`.
  The two barred-but-different rows collapse into one answer.
- **N2** — drop the mutability clause: `l` and `m` read `vm=true/valid=false`; `a` stays green, which
  is what says the clause is on the FLAG.
- **N3** — compute `tooLong` without the dirty flag: `b` and `b_scriptset` read `tl=true`, so a
  `maxlength` attribute would invalidate every server-rendered value longer than it.
- **N4** — disable the temporal min/max arm: the four temporal rows lose their range flags.
