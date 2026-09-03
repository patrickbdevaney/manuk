# `sizes` — the three tokenizer rules that run before the grammar

*Landed t1401. Mechanism gate: `engine/page/tests/g_sizes_tokenizer_recovery.rs`.
Sibling: `engine/page/tests/g_sizes_first_match.rs` (the list/value grammar, t1275).*

## The shape of the bug

HTML's [*parse a sizes attribute*](https://html.spec.whatwg.org/#parse-a-sizes-attribute) is
defined over a **CSS token stream**. `sizes_slot_width` (`engine/page/src/lib.rs`) implemented it
directly over the attribute's **characters**. That is correct for every `sizes` a human writes by
hand, and wrong for three constructs — and in all three the failure is the same and it is silent:

| `sizes=…` | Chrome | before t1401 | the tokenizer rule that had already run |
|---|---|---|---|
| `/* */1px/* */` | 1px | 100vw | comments are REMOVED before the grammar |
| `\(,1px` | 1px | 100vw | an ESCAPED bracket is an ident character |
| `min(1px, 200vw` | 1px | 100vw | EOF CLOSES an open function |

`sizes` is how a page states how big the image will *be*. When an entry is discarded the slot
collapses to the `100vw` default, so **a different bitmap is fetched** — and on a narrow viewport
that is the *larger* one, the exact inversion responsive images exist to prevent.

## 1. EOF closes an open block — it is not a parse error

The old code returned `None` here and reasoned it out in a comment: *"an unclosed `(` makes the
whole `<source-size>` a parse error"*. CSS's [consume a
function](https://drafts.csswg.org/css-syntax/#consume-a-function) step says the opposite:
reaching EOF ends the function, flags a parse error, **and returns the block anyway**. So
`min(1px, 200vw` resolves exactly as `min(1px, 200vw)` does — which is what headless Chrome
answers for all ten spellings WPT writes.

⭐ **The bug hid behind the spelling with no space.** `split_trailing_component` splits
`<media-condition>? <source-size-value>` by walking **right-to-left** and stopping at the first
top-level whitespace it crosses:

- `calc(1px` — no whitespace at all, so the walk runs to the `(`, sees `depth < 0`, and recovers.
  **This passed the whole time**, which is what made the area look finished.
- `min(1px, 200vw` — the walk crosses the space after the comma **before it has seen the `(`**,
  because the `(` is to its left. It split into a condition `min(1px,` and a value `200vw`, the
  bogus condition failed to match, and the entry vanished.

So the fix is not a new recovery arm, it is an **ordering**: measure the bracket balance in a
left-to-right pre-pass, and if anything is open, take the value from the last top-level space to
the end of the entry. Only then run the right-to-left walk.

> ⚠ **A green mutation then found the recovery arms INERT.** Mutation N4 replaced *both*
> in-walk `Some(("", entry))` recoveries with `None` and every gate row and all 696 WPT subtests
> were unchanged — the pre-walk check had already taken every unmatched-opener input, so those
> arms were a second copy of the rule with no reachable input left. They ship as `None`, which is
> also the correct answer for the imbalance that *does* reach them (extra **closers**: `1px)` is
> not a `<length>` however you close it).

## 2. A comment is a token BOUNDARY, not nothing

`strip_css_comments` runs first, before anything else parses. The obvious implementation — strip
`/* … */` to the empty string — is wrong: `1/**/px` would become `1px`, and Chrome answers
`100vw` for it, because `1` and `px` are two tokens with a comment between them and that is not a
`<length>`. The strip therefore emits a **single space**. `c_boundary` is the gate row that can
tell the two implementations apart, and mutation N2 (strip to nothing) flips it plus `c_between`.

An **unterminated** `/*` is closed by EOF, same rule as the block: `1px/*` is `1px`.

## 3. An escaped bracket is an ident character

`split_top_level_commas` counted brackets to find top-level commas. `\(` is not an opener — it is
an escaped ident character — so `sizes="\(,1px"` is **two** entries whose second is `1px`.
Counting the escaped `(` as depth swallowed the comma, made it one unparseable entry, and turned a
1px slot into the viewport width. WPT writes five spellings (`\(`, `\{`, `\[`, `x\(`, `1\p\x`).

⚠ **Measured, not built:** `1\p\x` — an escape *inside the unit* — still falls back. Chrome
resolves it as `1px`, because the escape is unescaped into the ident before the unit table is
consulted. The escape handling added here makes the backslash transparent to *depth counting*; it
does not **unescape** for the value resolver. That is one row, left out of the gate rather than
asserted at a wrong value, and it is the next thing in this function.

## How the expectations were arbitrated

Every row in `g_sizes_tokenizer_recovery` is headless Chrome's own `currentSrc` at an 800px
viewport, read off a file:// fixture. This matters here specifically: spec prose is what put the
wrong `unclosed=b.png` value into `g_sizes_first_match`, where it sat as a **gate pinning the
engine to a bug**. Chrome agrees with all 22 of that gate's rows now; before t1401 it disagreed
with exactly one, and the one it disagreed with was the one derived from prose.

Each of the three mechanisms also ships the row that must **still** fall back, because "recover
from a malformed attribute" is one edit away from "never reject anything": `c_boundary`,
`e_none` (an *un*escaped `(` really does swallow the comma) and `u_cond_skip` (the media condition
is still parsed, and can still fail, on the recovery path).
