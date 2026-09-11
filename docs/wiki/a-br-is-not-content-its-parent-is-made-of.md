# A `<br>` is not content its parent is made of — two places, one rule, 23 of 23

**t1512.** t1511 fixed an inline whose content is *only* `<br>`, and left four rows it could not
explain — pinned in the gate at what the engine produced, each carrying Chrome's number and the
instruction *"if you have just fixed it: this failure is the good outcome."* This is that fix, and
**the gate is how it was found to be finished.**

```text
                                   Chrome            t1511            t1512
  XX<span><br><br></span>YY     [ 0, 60, 0,19]   [ 0, 60,19,19]   [ 0, 60, 0,19] ✓
  XX<span><br>Q</span>YY        [ 0,220,10,19]   [ 0,220,19,19]   [ 0,220,10,19] ✓
  XX<span><br><i></i></span>YY  [ 0,300,12,19]   [ 0,280,19,39]   [ 0,300,12,19] ✓
  XX<span><i></i><br></span>YY  [19,320,12,19]   [ 0,320,31,39]   [19,320,12,19] ✓
```

**The whole 23-value fixture is now Chrome-exact** — 8 inline rects, 3 atomic children, and all 10
containing-block heights. It was 11 of 16 before t1511.

## The rule, and it was in two places

t1511 named the surplus and could not explain it: *"in every one the extra width is 19 — the width
of the `XX` before the span — and in every one the content spans more than one line."* Both halves
are the same sentence: **a `<br>` is not content its parent is made of.** It has geometry of its own
(t380 — `getBoundingClientRect` on a `<br>` is how caret libraries find line ends, and that is kept),
and it is not part of its parent's box.

### 1. A `<br>`'s fragment does not LIFT into its ancestors

`node_rects` propagates every fragment up to its ancestors. The break's zero-width fragment sits at
the **end of the previous line**, so lifting it reached the span's box back to `x = 19` — the far
side of the `XX` it had broken after. That is where the 19 came from, twice.

### 2. The two reporters land INSIDE the content, skipping leading and trailing breaks

An inline with no fragment of its own gets a head and a tail reporter, *"because an inline that wraps
spans several lines and Chrome's rect covers the first line's content top through the last line's
bottom."* A **leading** break dragged the head onto the previous line; a **trailing** one pushed the
tail onto the next. So a span whose drawing content is on ONE line measured two, in both directions.

## The measurement

```text
  serennu.com    73.8  ->  77.0 (t1511)  ->  78.7 (t1512)      misplaced 12 -> 12 -> 11
                 two readings before (two binaries), two after at each step
```

⭐ **+4.9 shape on a real site across the two ticks, and it crossed the Phase-0 0.75 floor at t1511.**

## ⭐⭐⭐ The gate told the next tick what to write

`KNOWN_WRONG` is gone, and how it went is the point. t1511 could not derive Chrome's model for the
multi-line rows and refused to guess — *a plausible rule that fits every fixture you happened to
write is the most expensive kind of wrong* (t1418). Instead it pinned each row at the engine's own
value with Chrome's beside it. When t1512's fix landed the gate went red and printed:

```text
  G_AN_INLINE_OF_ONLY_BREAKS: #s2 moved.
    If you have just fixed it: this failure is the good outcome. Move #s2 into CHROME
    with [0, 60, 0, 19] and delete it from KNOWN_WRONG.
```

**A gate that names what it cannot catch does not merely avoid pretending (t1417) — it hands the next
tick a finished target and then confirms the hand-off.** One tick later the table is empty.

## Where it is

`engine/layout/src/lib.rs` — the `lift` loop in `node_rects`, and the two-reporter branch.
Gate: `engine/page/tests/g_an_inline_of_only_breaks.rs`, red under **seven** mutations, including
letting a `<br>` fragment lift, putting the head reporter before a leading break, and the tail after
a trailing one.

⚠ **The flow still must not move**, and all ten containing-block heights are Chrome-identical through
both ticks. 191 `manuk-layout` tests green.
