# The end of the accessible-name chain, in the right order

> Landed t1386. Gate: `the_name_chain_ends_in_the_right_order`
> (`agent/tests/g_ax_name_fallback_chain.rs`), 19 rows, oracle = CDP
> `Accessibility.getFullAXTree`. WPT `accname` 438/484 → **445/484**.

## The four defects, and they are all "the end of the chain"

```text
                                                  chrome    before    after
  <input placeholder="PH" title="TT">             TT        PH        TT
  <textarea placeholder="TA">                     TA        (none)    TA
  <input type=submit>            (no value)       Submit    (none)    Submit
  <input type=reset>             (no value)       Reset     (none)    Reset
  <input type=submit value="">   CONTROL          (none)    (none)    (none)
  <input type=button>            CONTROL          (none)    (none)    (none)
  <table summary="TS">                            TS        (none)    TS
```

⭐⭐⭐ **`title` beats `placeholder`, and we had it the other way round.** HTML-AAM's input chain is
`<label>` → `aria-label` → **`title`** → `placeholder`; ours applied `placeholder` inside step 3
(host-language label), which put it *ahead* of the step-5 tooltip. A placeholder is the hint that
disappears the moment the user types; a `title` is the author's stated label. Announcing the
transient one is not a tie-break, it is the wrong answer.

⭐⭐ **`<input type=submit>` with no `value` is the commonest submit button on the web, and it was
nameless.** The UA renders the word *Submit* on it; HTML-AAM names it by that default. Without it
*"click Submit"* resolves to nothing — a form an agent can fill and cannot send.

⭐ **`type=button` is the control that stops this being a blanket rule.** Chrome-measured, a
valueless `<input type=button>` has no name at all, because the UA renders no default label on it.
Three button types, two defaults.

⭐ **`value=""` suppresses the default** — the same rule this file already carries for
`<img alt="">`: *an explicit empty host-language label is an answer, not a missing one.* The
attribute's PRESENCE is the discriminator, not its content.

⭐ **`<textarea placeholder>` was nameless because the rule lived inside an `el.name == "input"`
branch.** One rule, two elements, and only one of them had it.

## ⚠ The `<table summary>` row shadowed the `<caption>` arm

Written first as its own `"table" => …` match arm, it shadowed the `"fieldset" | "table"` arm below
that reads `<caption>` — so **every captioned table went nameless**, and `G_A11Y_LABEL`'s *"a
`<table>` is named by its `<caption>`"* row went red on the first run.

> **A new arm in a match on tag names is a SHADOWING hazard, and the tag that already has an arm is
> the one you are about to break.**

`summary` is a fallback BEHIND the caption, Chrome-measured across all four combinations:

```text
  summary + caption   -> CAP        caption wins
  summary alone       -> TS
  caption alone       -> CAP
  + aria-label        -> AL         which beats both
```

## The receipt — and this is the sweep's first suite movement

```text
  WPT accname   438/484 = 90.5%   ->   445/484 = 91.9%   (+7)
```

t1379, t1380, t1384 and t1385 all moved their suite by **zero**, because they were about mechanisms
no suite exercises (audit #80's finding). This one moves it, because the name chain is exactly what
`accname` tests. **Both outcomes are information**: a flat number after a real fix is a question
about the suite's aperture, and a moving one says the fix was inside it.

## ⚠ Measured and not built

```text
  <div title="DT">content</div>     chrome: ""        ours: "DT"
  <abbr title="Abbrev">AB</abbr>    chrome: "Abbrev"  ours: "Abbrev"   ✓
```

`title` is a name fallback only for elements HTML-AAM says so — on a plain `<div>` it is a
DESCRIPTION, not a name. Ours applies it universally. Narrowing it needs its own battery of which
elements title-names (the `<abbr>` row shows the rule is not simply *"generic cannot be named"*), and
getting that wrong DELETES names rather than adding them.

## How it was proven red

- **N1** — move `placeholder` back above `title`: only `n3` fails, at `"PH"`. Both single-source
  controls stay green, which says the defect was the ORDER and not either step.
- **N2** — drop the `value.is_none()` guard: only `n20` fails, at `"Submit"`. The three valued rows
  agree either way, which is why the empty-value row is in the fixture at all.
- **N3** — give `<table summary>` its own match arm again: `n30` and `n32` go nameless while `n31`
  passes. That is the shadowing, isolated — and it is what an existing gate caught for real.
