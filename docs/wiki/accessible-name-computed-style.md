# The accessible name is a function of the COMPUTED STYLE

> Landed t1365. Gate: `the_accessible_name_reads_the_computed_style_through_both_entrances`
> (`agent/tests/`). Every value is Chrome's, taken from `accname/name/comp_name_from_content.html`.

## The one-sentence mechanism

> **The same markup names two different things, and only a stylesheet separates them** — so a name
> walk that reads only the DOM cannot be right, whatever else it gets correct.

```html
<button><span>one</span><span>two</span><span>three</span></button>
```

is `"onetwothree"` when the spans are inline and `"one two three"` the moment CSS makes them
`display:block`. accname's *Computed Name from Content* appends a separator around a child that is
not an inline box.

The same is true of `text-transform`: a heading styled `uppercase` is named `"CALL US"`, because
**the name is the text a user is read**, not the text an author typed.

## The receipt

```text
  accname   423/484 (87.4%)  →  432/484 (89.3%)   +9, and ZERO newly failing
    button/heading/link name from content for each child (no space, display:block)        ×3
    button/heading/link name from content for each child (no space, display:inline-block) ×3
    heading name from content with text-transform:uppercase / lowercase / capitalize       ×3
  wai-aria  399/434 (91.9%)  CONTROL, unchanged
  html-aam  310/335 (92.5%)  CONTROL, unchanged
```

The fixed set is exactly the nine rows check #128 predicted when it wrote *"9 need the computed
style threaded into `accessible_name`"* — measured by diffing the failing **name lists**, not the
totals.

## The two rules, and the rows that decide them

⚠ **`inline-block` separates TOO.** The WPT fixture asserts `"one two three"` for `display:inline-block`
spans as well as `display:block` ones. The rule is *"not an inline box"* — an inline-block is an
**atomic inline**, a block box that participates in a line rather than an inline box. A predicate
written as `matches!(d, Display::Block)` passes three of the six spacing rows and fails the other
three; that is mutation N2 in the gate.

⚠ **`capitalize` upper-cases the first typographic letter of each word and leaves the rest as
authored** — `"Call us"` → `"Call Us"`, not `"Call US"`. It is not `to_uppercase` on each
whitespace-separated word; that is mutation N4.

⚠ **`text-transform` is inherited**, so a `NameStyles` built from a partial style map must not read
a missing entry as `none`. The lookup walks up to the nearest ancestor that has one.

## ⚠⚠⚠ Both entrances, asserted against each other

The name walk is reached through **two doors**: the AX tree builder (what a live page's agent reads)
and the bare `accessible_name` behind `test_driver.get_computed_label()` (what the conformance suite
reads). This subsystem has been caught by that split **three times** — t1097's generated content,
t1350's case fold, t1355's name entry — which is what check #128 sharpened I3 for:

> *"exposed" MUST MEAN "EXPOSED THROUGH EVERY ENTRANCE THE SEMANTIC API IS READ THROUGH."*

So every row of the gate asserts that **the two doors agree, and that the agreed value is Chrome's**.
Mutation N5 threads the styles into the bare entrance only: every `bare` assertion passes, every
`AX TREE` assertion fails, and the failure message names the shape. That is what makes the two-door
structure load-bearing rather than decorative — a fix wired to one door fails here even though its
own suite number moves by the full +9.

## How it is plumbed, and why that shape

`NameStyles` is a `HashMap<NodeId, (Display, TextTransform)>` — deliberately the same shape as
`GeneratedText`, built once by `manuk_a11y::name_styles` and passed in. One builder, so the two
entrances cannot drift; `manuk-a11y` gained a `manuk-css` dependency to name the two enums (no
cycle — `manuk-css` does not depend on `manuk-a11y`).

⚠ This is the **third** fact threaded into this walk one at a time (t1097 `GeneratedText`, t1355 the
`NameIndex` widening, now this), and each one has left a caller behind. The one at
`engine/a11y/src/lib.rs`'s own unit test has now been left behind twice, and its comment says why:

> *"t1355 widened this parameter … and left this caller behind, which broke the WHOLE crate's
> `cfg(test)` build — invisibly, because `manuk-a11y` is not in the wall's crate list."*

Surface audit #78 measured that: `manuk-a11y` is a suite in no wall and no CI job. **A fourth fact
should become a context struct rather than a fourth parameter** — the signature already carries an
`#[allow(clippy::too_many_arguments)]`.

## What is still open in accname

52 rows remain. The largest coherent group is check #128's STEER #1 — CSS `content` features:
`attr()`, `counter()` with alt text, and the `/alt-text` syntax (~18 rows, and `content` appears in
26 failing names). Then `::marker` (8), shadow DOM and slots (6), `aria-owns` (3).
