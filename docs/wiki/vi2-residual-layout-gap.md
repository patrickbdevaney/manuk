# CONSTITUTION VI.2's residual layout gap — re-measured, banked, and narrowed to one row

> Landed t1364. Gate: `vi2_named_residuals_match_chrome` (`agent/tests/`).
> Eleven cases, one per category VI.2 names, every number headless-Chrome-measured.

## Why this exists

`CONSTITUTION.MD` VI.2's H0.1 row is the loop's ranking instrument for *where the residual layout gap
lives*. It names the box types that opt **out** of ordinary block sizing: tables, inline composition,
floats and `clear`, out-of-flow boxes under a transformed containing block, and the intrinsic
measurement pass. Check #129's STEER #2 was *"re-measure VI.2's remaining named residuals before
ranking against them"* — written after t1362 found three of its table entries had been false for
~427 ticks.

## The result: nine correct, one fixed, one real

```text
                                                           manuk         chrome
  1  anonymous table row (<table><td> with no <tr>)      [2 2 12x26]   [2 2 12x26]   ✓
  2  anonymous table (display:table-cell, no table)      [0 0 39x24]   [0 0 39x24]   ✓
  3  inline box holding no text of its own (t934/t935)   [0 -11 5x46]  same          ✓
  4  self-collapsing box between two margins (t1001)     [0 54 …]      same          ✓
  5  float placed at the TOP of its line (t1002, §9.5)   [0 0 80x20]   same          ✓
  6  abspos under a TRANSFORMED containing block (t1012) [-10 -5 60x20] same         ✓
  7  percentage height in an auto-height parent          [0 0 400x24]  same          ✓
  8  shrink-to-fit width of a float                      [0 0 241x24]  same          ✓
  9  clear:left past a float                             [0 40 400x24] same          ✓
 10  inline-block baseline with overflow:hidden          [10 0 30x40]  same          ✓
 11  UA `table { border-spacing: 2px }`                  [0 0] h=26 → [2 2] h=30     FIXED HERE
 ──  a float that FOLLOWS text: the block's height          48            24         OPEN
```

The HTML parser is not implicated: `<table><td>a</td></table>` produces `<table><tbody><tr><td>` in
both engines. Rows 1 and 2 were the *anonymous-row* suspicion and it was wrong — what they actually
caught was row 11.

## ⚠⚠⚠ Row 11 — a UA-sheet twin drift that mis-measured every table gate on one harness

`stylo_engine.rs` gained `table { display: table; border-spacing: 2px }` at t908, with a note saying
it had been missing. **Its hand-maintained twin in `MinimalCascade` never did**, and the two sheets
have disagreed ever since:

```text
                     cell offset in the table     table height
  Chrome                    [2, 2]                     30
  Stylo (shipping)          [2, 2]                     30
  MinimalCascade            [0, 0]                     26
```

That is not only a rendering bug in the JS-less build. `engine/layout`'s 191 unit tests and
everything under `agent/tests/` cascade through `MinimalCascade`, so **every table fixture on those
harnesses measured 4px short in both axes** unless it set `border-spacing` itself. The t923 rule
stands and now has a third instance: *a UA declaration lives in BOTH sheets or in NEITHER.*

⭐ **It was found by running one battery on BOTH paths and getting different answers.** The first run
was on `agent/tests` alone and reported the table rows as an engine defect; the identical fixture
through `manuk-page --features stylo` returned Chrome's numbers. That is t1361's lesson applied to
the **measurement** rather than to the engine:

> **Measure on the shipping path, or say which cascade you measured.** A battery run on one harness
> attributes that harness's cascade bugs to the engine, and they look exactly like layout bugs.

⚠ A gate named `g_table_border_spacing_ua_default` exists and was **green throughout** — it runs on
the Stylo path, where the property was never missing. A gate can be correct, well-named, and blind to
the same property on the other cascade.

## ⭐ THE ONE ROW THAT WAS A REAL SHIPPING DIVERGENCE — BUILT AT t1378

```text
  A FLOAT THAT FOLLOWS INLINE TEXT DOES NOT RE-FLOW THE LINE IT JOINS

    <div style="width:400px">xxxx xxxx xxxx<div style="float:left;width:80px;height:20px">
    </div>yyyy</div>
                                       Chrome       before    after
      the float's own rect            [0 0 80x20]    same      same   (t1002)
      the BLOCK's height                  24           48        24

    the same with a 380px float that cannot fit beside the text:
      the float drops to y=24 in both [0 24 380x20]  same      same
      the BLOCK's height                  24           48        24

    CONTROL — float FIRST, then the text: 24 in all three
```

t1002 fixed *where the float goes* (§9.5 rule 6 — the top of the line, not below the run). The
remaining half is that **the inline content already flushed onto that line is not re-laid around the
float**: the text keeps its original x positions, the float overlaps it, and the trailing text is
pushed to a second line. `layout_block`'s own comment names it exactly:

> *"`place()` cannot see this for us: it scans bands of FLOATS, and what is in the way here is the
> line's own already-placed inline content, which is not a float and is not in the context."*

Doubling the height of any block whose paragraph contains a mid-text float is a large `dy`, and
floats are on 60.4% of the declared corpus, which is why this was the ranked next tick rather than
something to attempt at the end of a battery. **t1378 built it**: the float arm no longer commits
that flush — it lays the run out as a trial, throws the trial's boxes away, places the float, and
lets the single real flush at the end line-break the whole run around it. Full mechanism, the
ten-shape Chrome battery and the two red mutations: `docs/wiki/float-line-reflow.md`.

## ⭐ How a known divergence is handled in a gate

`c5`'s height was **absent from the asserted set on purpose** until t1378, and the reasoning is worth
stating because both alternatives were wrong:

- asserting Chrome's 24 lands a **RED gate** — a gate is a ratchet tooth, not a wish list;
- asserting our 48 **PINS THE BUG**, which is the t1004 shape this project has caught before (*a gate
  can pin the engine to a bug*).

So the number lived in this file and in the gate's header, the row was not asserted, and the promise
was that `(5, 24.0)` joins the list the day the re-flow lands. **t1378 landed it and the row is now
asserted** — which is what makes this handling a deferral with a receipt rather than an excuse. The
float's *placement* half stayed asserted throughout, because that is the part that must not regress
while the other half is outstanding.

⚠ `c8`'s height is asserted as **zero**, which a blanket `height > 0` vacuity check called a missing
box on this gate's first run. A block whose only child is a float, and which is not a BFC root, does
not contain it. Every wrapper height here is a Chrome-measured claim rather than a sanity threshold,
which is what makes that row a statement instead of an exception.
