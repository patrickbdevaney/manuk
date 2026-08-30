# The landmark map, swept — and the one row that was wrong

> Landed t1387. Gate: `a_landmark_is_a_landmark_only_when_the_spec_says_so`
> (`agent/tests/g_ax_landmark_roles.rs`), 20 rows, oracle = CDP `Accessibility.getFullAXTree`.
> Surface audit #80's ranked #1, fourth pass.

## ⭐⭐⭐ `<form>` is a landmark only when named, and the rule was written down next door

```text
                                       chrome     before    after
  <form>                       plain   generic    form      generic
  <form aria-label="FL">               form       form      form
  <form title="FT">                    form       form      form
  <form aria-labelledby=…>             form       form      form
  <form name="n">              CONTROL generic    form      generic
  <div role=form>              CONTROL generic    generic   generic
  <div role=form aria-label>   CONTROL form       form      form
```

The `<section>` arm **three lines below** the `<form>` arm carries the identical clause, and
`role_of`'s explicit-role path in the same function carries it for exactly these two roles:

```rust
if matches!(r, Role::Region | Role::Form) && !has_attribute_name(dom, node) { continue; }
```

> **The same rule, guarded at one entrance of one function and unguarded at the other** — and the
> guarded entrance is `role="form"`, which almost nobody writes, while the unguarded one is
> `<form>`, which is on nearly every page.

⭐ **Why it matters to an agent and not only to a spec: a landmark list is a JUMP LIST.** Every
`<form>` on a page — the newsletter box, the search field, the login — appeared in it, so *"go to the
form"* was ambiguous exactly when there is more than one, which is the case the list exists for.
t1375 made the drive path REFUSE an ambiguous target, so this converted into a refusal rather than a
wrong click — the same conversion t1380's phantom menu link made.

⭐ **`name="n"` is the row that stops *"has any nameish attribute"* from being the rule.** A form's
`name` is its submission name, not an accessible one; Chrome reports `generic`.

## The nineteen that were already right, and are now banked

```text
  <section>  plain / aria-label / title / aria-labelledby   generic / region ×3
  <nav> <main> <aside>                    navigation / main / complementary
  <header> <footer> at top level          banner / contentinfo
  <header> inside a <div>                 banner        (a div is not sectioning content)
  <header> inside an <article>            sectionheader (scoped — NOT a landmark)
  <div role=region> unnamed / named       generic / region
```

⭐⭐ **The `<header>` pair is the sharpest of them.** A `<header>` inside a `<div>` is still the
page's `banner`, because a `<div>` is not sectioning content; the same element inside an `<article>`
is a scoped `sectionheader` and must not appear in the landmark list. Backwards, this either hides
the page banner or puts every card's header into the jump list. It was already right and had no gate.

⚠ This is the shape t1377 named: **most of a swept surface is usually correct, and the value of the
sweep is the one row PLUS the banking of the rest.** A sweep that reports only its finding leaves
nineteen behaviours as unguarded as it found them.

## ⚠⚠ And a latent wrong answer that t1384 made visible

```text
  <fieldset disabled>          chrome: role=group, NO `disabled` property
    <input type=checkbox>      chrome: disabled: True
```

The native `disabled` attribute belongs to the *listed form elements*; `<fieldset>` carries it as a
PROPAGATOR and is not itself disabled. Ours reported it on the fieldset too — and **as a nameless
`generic` that node was never printed in the observation lines, so the wrong state could not be
seen.** t1384 promoted `<fieldset>` to `group`, which is correct, and the promotion PUBLISHED the
wrong state: `g_disabled_inert`, which counts the `disabled` lines, went red.

> **A latent wrong answer surfaces when the node it lives on becomes visible**, so a correctness fix
> can look like the thing that broke a gate when it is the thing that exposed it.

⚠ `aria-disabled` is NOT scoped this way and must not be: `<div role=button aria-disabled=true>`
reports `disabled` in Chrome on any element, because the author said so explicitly. Only the NATIVE
attribute belongs to controls.

## ⚠⚠⚠ How it stayed hidden for three ticks — and it was not the wall

`engine/page/tests/` is not in the wall's crate list (surface audit #78: 502 of 522 gate files run
nowhere), so `g_disabled_inert` was never executed by `verify.sh`. It WAS executed by the manual
`cargo test -p manuk-page` this loop runs each tick — and the check that reported it looked like
this:

```sh
cargo test -q -p manuk-page … | grep -E "^test result" | grep -v "0 failed" | head -5
echo "page done (empty=green)"
```

The `echo` runs **unconditionally**. The failing line was printed, and directly under it a
hard-coded reassurance. Three ticks landed on top of it.

> ⭐⭐⭐ **A CHECK THAT PRINTS ITS SUCCESS MESSAGE UNCONDITIONALLY IS NOT A CHECK.** The eye lands on
> the last line. The fix is to make the *shape of the output* carry the verdict — count the result
> lines and print the counts — so there is no sentence to read instead of the data.

## How it was proven red

- **N1** — restore `"form" => Role::Form`: `f_plain` and `f_name` fail, and the tree row reports six
  form landmarks instead of four. Every named row stays green, which says the defect was the GUARD.
- **N2** — guard on `aria-label` alone: `f_title` fails. Three spellings name a landmark, not one.
- **N3** — let a `<div>` count as sectioning content: only `l_div_header` fails, at `sectionheader`.
  That row and `l_art_header` are the pair that decides the rule, and each is inert without the
  other.
- **N3b** — restore `inherits_disabled` to walk from ANY element: `d_fs` fails (the fieldset reports
  a `disabled` it does not have) while `d_in` and `d_aria` stay green, which says the SCOPE is the
  fix and not the removal.
