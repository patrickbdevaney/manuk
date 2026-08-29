# Table structure geometry — rowspan, `<caption>`, `<thead>`, and a backlog that was false for 427 ticks

> Landed t1362. Gate: `rowspan_caption_and_thead_ordering_match_chrome` (`agent/tests/`).
> Every number is headless-Chrome-measured, `16px/24px monospace`, `border-collapse: separate`.

## The one-sentence finding

> **A "NAMED, MEASURED, NOT BUILT" entry is a claim about the present tense, and nothing re-runs
> it** — three table defects sat in a gate's own header, and in `CONSTITUTION.MD` VI.2, for ~427
> ticks after they were fixed, while none of the three had a gate.

## What the ledger said

t933 measured a sixteen-row table battery, fixed one mechanism, and left three named:

```text
  ROWSPAN row-height distribution   a 60px rowspan=2 cell must give 30/30 to its two rows;
                                    we give 24/36 — the overflow all lands on the LAST row
  CAPTION                           `<caption>` reserves no space and does not widen the
                                    table: the first cell belongs at y=20 and 29 wide,
                                    reads y=0 and 10 wide
  THEAD ORDERING                    a `<thead>` written AFTER a `<tbody>` must still render
                                    FIRST; we render in source order (y=24, Chrome y=0)
```

`CONSTITUTION.MD` VI.2 carries *"t933 row-height distribution"* among the box types that opt out of
ordinary block sizing, and `CONSTITUTION-CHECK.md` lists it as **UNMEASURED**.

## What re-measuring found

All three are correct, to the pixel:

```text
 A  ROWSPAN     <td rowspan=2 height:60px> + two rows
                  the spanning cell  [0  0 10x60]      the rows get 30 and 30, NOT 24/36
                  row-1 neighbour    [10 0 10x30]
                  row-2 neighbour    [10 30 10x30]     table 60
 B  CAPTION     <caption>Cap</caption> + one <td>cell</td>
                  the caption        [0 0 39x24]       it RESERVES a band above the rows
                  the cell           [0 24 39x24]      pushed down by exactly the caption
                  table width        39                the WIDER of caption and cells
 C  THEAD       <tbody> written BEFORE <thead> in source
                  the tbody cell     [0 24 39x24]      source order is NOT render order
                  the thead cell     [0  0 39x24]      the head renders FIRST
```

They were built somewhere in the ~427 ticks since, by ticks that fixed them without going back to
cross off the list. Nothing is wrong with that except that the list stayed, and work gets ranked
against lists.

## Why the retraction is not the point — the banking is

Three real behaviours were one edit away from silently regressing, in a subsystem that has now been
shown **twice in three ticks** to regress silently for weeks:

- t1360 found `g_table_cell_valign` RED and drifting for twenty-three days, because it is not in
  `scripts/verify.sh`'s launch list.
- t1361 found `manuk-css`'s entire Stylo test module `#[cfg]`-ed out of the wall, including the gate
  t1358 landed specifically as *"the second entrance, the door every real page comes through."*

Implemented-but-ungated is not banked. Under this project's own definition a ratchet tooth is the
gate, not the code change, and these three now have one — proven red by the three pre-t933 rules the
retracted entries describe.

## The three distinctions worth keeping

1. ⭐ **Arm B is two claims in one fixture and they fail separately.** A caption that reserved its
   band but did not participate in the table's width leaves the table sized by its cells; one that
   widened the table but reserved no band leaves the first cell at y=0. t933's note described this
   as one bug (*"reserves no space and does not widen"*) and it is really two.
2. ⚠ **Arm C's fixture puts `<tbody>` FIRST on purpose.** With `<thead>` written first, source order
   and render order agree and the arm is vacuous — it would pass against an engine that had never
   heard of `<thead>`. The inverted spelling is also the one real pages produce, because a template
   that appends rows to a `<tbody>` and then prepends a header emits exactly it.
3. **Rowspan distribution is PROPORTIONAL, not even** — and the equal-rows case cannot tell the two
   rules apart, which is why the gate's fixture would not have caught an even split on its own. It
   catches the *pre-t933* rule (all excess to the last row), which is what actually regressed.

## ⚠ The anchor-site survey that led here, and the four hypotheses it REFUSED

This tick opened where the board's CO-#1 points — `fidelity --shape-dump` on the worst anchor site,
`www.a11yproject.com` (shape 43.3%, coverage 96.0%, 123 of 217 elements missed). The dominant
signature was **width**, always in one direction: our boxes narrower than Chrome's (+12 ×13, +19
×11, +17 ×11, +36 ×8), with a matching `nav ol` that is 88 tall where Chrome wraps to 176. Same
width, double height — so Chrome's text wraps where ours does not, i.e. **our text is systematically
narrower on this page**. Four mechanisms were proposed and all four were killed by measurement:

```text
  1  the fallback FACE differs           REFUTED  sans-serif / serif / monospace at 100px measure
                                                  755.92 / 698.05 / 903.08 in BOTH — ratio 1.0000
  2  we apply the site's webfont,        REFUTED  Chrome refuses the cross-origin @font-face from a
     Chrome (CORS from file://) does not          null origin — and so do we: both give 755.92
  3  `rem` resolves against 16px, not    REFUTED  html{font-size:20px}: 2rem/1rem/1.6rem/nested-em
     the root's declared 20px                     all Chrome-exact (302.38 / 151.19 / 241.89)
  4  line-box overflow for text taller   REFUTED  a 40px span in a 24px line box lands at dy -11
     than its line-height                         with a 46px box in a 33px line — Chrome-exact
                                                  (this was t1360's mis-attribution; see t1361)
```

The narrowing is real and unexplained, and it is written down here so the next tick starts from four
closed doors rather than re-opening them. ⭐ The method that produced them is worth more than the
result: **the fidelity dump names a SITE, and a site is not a mechanism** — every hypothesis above
was killed by a four-line fixture measured against Chrome directly, in minutes, without touching the
sweep.
