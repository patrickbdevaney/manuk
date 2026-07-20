# PARKED — `position:absolute` + `width:max-content` (diagnosed tick 272, NOT landed)

**Status: correct, gated, proven RED, and BLOCKED on a second defect.** Apply
`tick272-abspos-maxcontent.patch` (same directory) once the panel-x defect below is fixed.

## Why it is parked and not shipped

The fix is right and it moves the number: local repro 28.6% → 100.0% placement, and
**wikipedia 7.2% → 10.1%, `mdy` 45 → 30** — the first movement on the sweep's highest-sample site in
four placement-targeted ticks.

It also takes **G6 clickability from 98.9% to 97.9% (4 misses → 8)**, over the ≤5 threshold. Per THE
RATCHET a capability is never traded for a regression, so it waits.

### The blocker is NOT a separate x defect — that was my first reading and it was wrong

The tick-271 dump reads `vector-page-tools cx=778 cw=150 · mx=823 mw=105 · dx=45 dw=-45`, and I
first filed that as "the panel is also 45px to the right". Check the arithmetic: `778 + 150 = 928`
and `823 + 105 = 928`. **The right edges agree exactly.** The panel is right-anchored, so `dx` is
not an independent error at all — it is `-dw` wearing a different sign. This patch fixes both at
once, and there is no panel-x tick to do first.

`dx = -dw` on a right-anchored box is worth recognising on sight: two columns of a placement dump
that look like two bugs and are one.

### What the blocker actually is (re-measured after applying the patch)

With the patch applied, the four new G6 misses are all *inside or beneath panels Chrome renders as
`visibility:hidden`* — verified by asking Chrome directly: `.vector-dropdown-content` computes
`visibility:hidden` on the Terrier page, laid out at 232×32, and `#vector-main-menu` inside it
likewise. Tick 272 taught the a11y tree to skip those. So two things are still true and they need
separating:

1. **"Main page" being unclickable is CORRECT.** It sits inside a closed, `visibility:hidden` menu.
   Chrome cannot click it either. G6 counts every `<a href>` with a box and calls this a miss —
   the metric does not know about hidden containers. ⚠ This is exactly the shape of "retune the gate
   to land your own tick", so it must NOT be fixed by editing the harness as part of landing this
   patch. If the metric is wrong it is wrong on its own merits, on its own tick, argued separately.
2. **"vermin" / "type" being unclickable is a REAL bug**, and it is the one to chase: they sit under
   the page-tools panel, which Chrome computes hidden and we apparently do not. An isolated repro of
   Wikipedia's exact dropdown CSS (`.vector-dropdown .vector-dropdown-checkbox:checked ~
   .vector-dropdown-content`) computes hidden correctly in our engine, so the divergence is
   something the real 206KB stylesheet does that the repro does not — a cascade question, not a
   layout one.

**NEXT STEP:** find why `.vector-dropdown-content` computes visible for us on the live page when it
computes hidden in the repro and in Chrome. That is a bounded cascade probe. When it is fixed, this
patch should land green.

## What is in the patch

- `layout_abspos`: the missing `s.width_keyword` arm (`min-`/`max-`/`fit-content`).
- The gate `abspos_intrinsic_width_keyword_sizes_to_content_not_the_anchor`, proven RED
  ("abspos=44.5 but the identical in-flow box is 139.6").

## The diagnosis, kept because it is the expensive part


---

## `position:absolute` + `width:max-content` — the anchored-panel bug (tick 272)

`layout_abspos` resolved width through arms for `stretch`, both-insets, and aspect-ratio transfer,
then fell through to shrink-to-fit. It had **no arm for `s.width_keyword`** — the field carrying
`min-content` / `max-content` / `fit-content` — which the in-flow block path had had all along.

```css
.trigger        { position: relative; width: 20px }     /* a 20px icon button */
.trigger .panel { position: absolute; width: max-content }
```

Shrink-to-fit sizes against the *containing block*, and an anchored panel's containing block is the
trigger it hangs off. So the panel came out **114px where Chrome says 180px** — sized to its anchor
instead of to its content.

### Why this is a whole class, not one site

`position:absolute` + `width:max-content`, anchored to a small trigger, is the structure of nearly
every **dropdown, popover, menu, tooltip, autocomplete panel and context menu** on the web. It is
Wikipedia's sidebar verbatim:

```css
.vector-dropdown-content { position:absolute; width:max-content; max-width:200px; padding:16px 16px }
```

93px against Chrome's 186px.

### Why it read as a *vertical* bug

The panel is not missing and not empty — it renders, at about half width, and every label inside
wraps to two lines. Each wrap adds ~16px, and the accumulated height is what a fidelity sweep
reports: `mdx=0, mdy=45`. **A width bug presenting as a vertical-placement statistic**, which is the
same misread that cost tick 270 and, one layer down, three ticks before it. A median offset cannot
say that the cause is a width; per-element boxes plus a static control can.

### The static control is the diagnosis

The first repro reproduced the CSS faithfully — `max-content`, `max-width`, a flex `<a>` with icon
and label — and scored **100% Chrome-exact**, because it omitted `position:absolute`. Adding that one
property dropped it to 28.6%. Keeping a `position:static` sibling in the same file is what turns
"our `max-content` is broken" into "`position:absolute` changes what `max-content` means":

```
                       Chrome    before    after
abspos  max-content     180       114       180
static  max-content     180       180       180   ← the control, correct throughout
```

**Measured effect:** wikipedia placement 7.2% → 10.1%, `mdy` 45 → 30 — the first movement on the
sweep's highest-sample site (138 ids) in four placement-targeted ticks.

## Anchored panels — dropdowns, popovers, menus, tooltips (tick 272)

**Pattern:** `position:absolute; width:max-content` on a panel anchored to a small
`position:relative` trigger. Every dropdown menu, every popover, every tooltip, every autocomplete
list, every context menu — the panel must be as wide as its own longest row, and it must not be
constrained by the 20px icon button it hangs off.

**The class this unlocks:** anchored panels being the right width. We sized them to the *anchor*
instead of the content, because the absolutely-positioned width path had no arm for intrinsic sizing
keywords and fell through to shrink-to-fit against the containing block. A panel came out at roughly
half width with every row wrapped to two lines.

**Why it hid:** the panel is present, styled, and full of the right content — it is just narrow. No
coverage gate can see it (nothing is missing), no crash, no error. And because wrapped rows are
taller, the visible consequence is *vertical*: everything below drifts down, and a fidelity sweep
reports `mdx=0, mdy=45`, which reads as a vertical-drift bug and sends the next tick after the wrong
organ. This is the second time in three ticks that a width bug has been mis-attributed to vertical
drift.

**The trap:** a repro that reproduces the *sizing* CSS faithfully but omits `position:absolute`
scores 100% Chrome-exact and proves the engine is fine. Keep a `position:static` sibling in the same
file — the control is what localises the bug to what `position:absolute` does to `max-content`,
rather than to `max-content` itself.

---

## The cascade probe, run (tick 273) — narrowed to a reproducible repro, NOT yet fixed

The step this document asked for is done. The divergence is real, it is reproducible in one command,
and it is **not** where three plausible guesses said it was. Written down at the point it reached
before a fix, because the eliminations are most of the work.

### The measurement

With **every `<link rel=stylesheet>` stripped** from the saved Terrier page, so the injected chunk is
the only source of these rules, and reading our own computed `visibility` per element:

```
no CSS at all (control)                    Visible=8  Hidden=0     ← as expected
.vector-dropdown .vector-dropdown-content{...visibility:hidden...}
                                           Visible=0  Hidden=8     ← correct
@media screen{ ...the identical rule... }  Visible=8  Hidden=0     ← WRONG
```

Same page, same rule, same engine. The only difference is the `@media screen { }` wrapper, and it
loses the rule. Chrome computes `hidden` for all eight. This is why the parked patch regresses G6:
those panels are supposed to be non-hit-testable, tick 272 taught the a11y tree to skip them, and
they never get marked hidden in the first place.

### What it is NOT — four eliminations, each measured

- **Not `@media screen` in general.** A synthetic page with `@media screen`, `@media all`,
  `@media (min-width)`, `@media screen and (min-width)` and `@media print` scores **100% against
  Chrome**, print correctly excluded.
- **Not descendant selectors inside `@media`.** `.p .c`, `.p > .c`, `div .c` and a bare class inside
  `@media screen` are all 100% Chrome-exact on a synthetic page.
- **Not the `:checked ~` sibling rule wrongly matching.** Tested with the live markup, including the
  intervening `<label>` and the trailing space in `class="vector-dropdown-checkbox "`. Correct.
- **Not a rule-count or stylesheet-size cap.** 8,000 synthetic rules (147KB) prepended: the rule
  still applies.

It needs the **live page**. A minimal reproduction on a synthetic document does not exist yet, and
finding one is the next step.

### ⚠ The harness bug that produced two false answers first

My first two bisections both reported confident "minimal breaking regions" and **both were
artifacts.** Two separate mistakes, worth naming because each is easy to repeat:

1. **Injected CSS was ADDITIVE, not a replacement.** The page's own `<link rel=stylesheet>` elements
   still loaded, so every chunk under test was appended to the full 206KB sheet. Chunks appeared to
   "fix" the bug by adding a later equal-specificity rule that won on document order. Every
   conclusion drawn before the links were stripped is void.
2. **Cutting the stylesheet at any `}` splits `@media` blocks in half.** An unclosed brace changes
   parsing arbitrarily, so the bisection was measuring its own truncation. The fix is a
   brace-depth-aware, string-aware splitter that only cuts at depth 0 — and it is what finally
   showed the rule was *inside* an `@media` block at all.

**Every number has a harness, and the harness is part of the number.** Two independent harness
defects in one probe, each producing a specific and plausible wrong answer.

### The repro, for the next tick

```
python3 - <<'PY'
import re
h = open('/tmp/manuk-g6.html').read()                 # curl of en.wikipedia.org/wiki/Terrier
open('/tmp/x.html','w').write(
    re.sub(r'<link[^>]*rel="stylesheet"[^>]*>', '', h).replace(
        '</head>',
        '<style>@media screen{.vector-dropdown .vector-dropdown-content{visibility:hidden}}</style></head>'))
PY
```

Then compare the computed `visibility` of `.vector-dropdown-content` with and without the `@media`
wrapper. Removing the wrapper must be the only change that matters — if it still is, bisect the live
page's **markup** (not its CSS: that ground is now covered), since the wrapper only fails there.
