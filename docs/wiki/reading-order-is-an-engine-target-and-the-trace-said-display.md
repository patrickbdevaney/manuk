# `reading-order` is an ENGINE target, not an instrument property — and the decision was 420 ticks overdue

**t1508.** t1507's corrected work order put `reading-order` **second**, at 52 addressable sites,
one behind shape — and it has never been worked. The first question was whether it is even ours.

## The decision the loop has owed since t1084

`jarring_reading_order` carries a report-only diagnostic partition, added over three ticks (t1034
counted zero-area and off-screen-parked pairs, t1041 counted containers, t1084 added the in-flow /
out-of-flow split), under a comment that says in as many words:

> *"**THE THIRD PARTITION, AND IT IS THE ONE THE LOOP OWES A DECISION ON (t1084).** … An out-of-flow
> box has no reading order relative to its in-flow siblings … Counted, not filtered — t1034's rule,
> and it is the rule precisely because this is the shape where a filter would be a threshold tuned
> to move a number."*

The worry was real: if most inversions involve a zero-area box, a box parked off-screen, or an
in-flow/out-of-flow pair, then `reading-order` is measuring the **instrument**, and its 52 sites are
a mirage. **It was never measured on the cohort that decides the bar.**

## The measurement — and it is unambiguous

The certificate's jarring bar is per-site **clean**, so the sites that decide it are the ones a pair
or two away. Of 52 scored sites with inversions, **17 have ≤2** — and 7 of those already pass the
shape floor.

```text
  MANUK_RO_PARTITION=1 on the four cheapest:

  www.lyreco.com     1 inversion  = 1 on-screen · 0 zero-area · 0 parked · 0 mixed-flow
  www.jatekshop.eu   1 inversion  = 1 on-screen · 0 zero-area · 0 parked · 0 mixed-flow
  rockstaractu.com   2 inversions = 2 on-screen · 0 zero-area · 0 parked · 0 mixed-flow
  oilprice.com       2 inversions = 2 on-screen · 0 zero-area · 0 parked · 0 mixed-flow
```

⭐⭐⭐ **Six of six on-screen. Zero artifacts of any kind.** The t1084 concern is real for the
big-count sites (`www.ta3lemkonline.com` alone contributes 415 pairs of a corpus-wide 1457) and
**does not apply at all to the tail that decides the bar**. `reading-order` is an engine target, the
partition is a decision rather than a threshold, and the term keeps its rank.

⚠ Each of those four is **one or two containers**, not one or two dozen problems — `RO-GROUPS` says
the biggest contributes 1 of 1 (a 2-sibling group) on both lyreco and jatekshop.

## ⚠⚠⚠ And then the trace named the wrong subsystem

`MANUK_RO_TRACE=1` on `www.lyreco.com` printed, on consecutive lines:

```text
  chrome  h3:nth-of-type(1) [127 205 747 42] static/block   div:nth-of-type(1) [886 187 187 63] static/block
  chrome reads div first, we read h3 first  (BLOCK in Chrome, INLINE here - we collapsed two rows onto one)
```

The first line is each box's real `position/display`: **`block` in Chrome, on both boxes.** The
second says *"BLOCK in Chrome, INLINE here"* — and it does **not mean `display`**. It meant *which
axis separates the pair*: stacked in different rows, or side by side in one.

> ⭐⭐⭐ **Two adjacent lines, the same two words, two different meanings.** This loop read the second
> as a computed-`display` divergence and went looking for a `display` bug that is not there.

t1415's rule, and this is its cleanest instance: *a diagnostic that reports the wrong thing does not
merely fail to help, it **accuses**.* And the collision survived because **the sentence had no way to
be contradicted by the data it sits next to** — `Seen::display` was already carried, already printed
one line above, and never read here.

The axis is now named as an axis, and `display` is consulted and reported **separately**:

```text
  chrome reads div first, we read h3 first
      (STACKED in Chrome, SIDE BY SIDE here - we collapsed two rows onto one;
       computed display AGREES on both boxes)
```

⭐⭐ **The negative is the useful half.** *"Computed display AGREES on both boxes"* closes an entire
line of investigation in one line of output, and the old wording had actively opened it.

## What the corrected trace then says the mechanism is

```text
  www.lyreco.com  h3   chrome [127 205 747 42]   ours [127 184 759 84]   dy -21
```

Our `<h3>` is **12px wider and exactly twice as tall** — two line boxes where Chrome has one. A
two-line `h3` fills the flex row and starts at its top, so it now reads before the `div` beside it
that Chrome reads first. The inversion is a **consequence**; the defect is that our text measures
wide enough to wrap where Chrome's does not — the font-metrics class (t1342-1343), reached from a
reading-order symptom.

⚠ `www.jatekshop.eu` is the mirror (`SIDE BY SIDE in Chrome, STACKED here`), also with `display`
agreeing, and its diverging box is 79 tall against Chrome's 64. **Same class, opposite sign.**

## Where it is

`tests/wpt/src/oracle.rs` — `inversion_note`, extracted from `ro_trace` so it can be gated at all.

Gate: `an_inversion_note_names_an_axis_and_not_a_display`, five rows (the lyreco shape, its mirror, a
real `display` divergence on the first box, one on the **second** box, and an ABSENT display which is
not half of a divergence). Red under six mutations — `BLOCK`/`INLINE` put back in the axis wording,
the display comparison dropped, the agreement case silenced, only the first box compared, an empty
display treated as a value, and the axis ignoring its tolerance.

⚠ **The fixture's TOLERANCE is load-bearing and the first draft had it wrong.** Chrome separates the
lyreco pair by 18px and we separate it by 3px; at `tol = 2` both engines read as STACKED and the
interesting case never appears. *A fixture with the wrong tolerance tests a different question.*

⚠ **No engine behaviour changed and no site moved.**
