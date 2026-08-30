# A space at the edge of a line is not drawn — and `content`'s spaces were

> Landed t1370. Gate: `a_pseudo_s_edge_space_is_a_gap_not_a_glyph` (`agent/tests/`).
> Every number headless-Chrome-measured, `16px/24px monospace` in a 400px block.

## The one-sentence mechanism

> CSS Text 3 §4.1.3 removes a line's leading and trailing white space, **and it does not care that
> the space came from `content`.**

Ordinary text gets this for free here: it is split on white space into words plus `PendingGap`s, so a
space at a line edge is simply never drawn. **Generated content took the other route** — t1107
emitted it as one unbreakable word with its spaces baked into the string — so `content: " before "`
carried its outer spaces as *width*, and a line whose first or last item was a pseudo came out one
space too wide at each end.

```text
                                                     Chrome    before    after
  ::before " before " + "label" + ::after " after "  173.41   192.66   173.39
  ::before " before " + "label"                      115.61   125.23   115.59
  "label" + ::after " after "                        105.97   115.59   105.96
  ::before "before" + "label" + ::after "after"      154.14   154.12   154.12   CONTROL
  "label" with no pseudos at all                      48.17    48.16    48.16   CONTROL
```

Exactly one space per edge, every time.

## ⭐ The interior space is KEPT, and that is the whole difference

Chrome's 12 characters for row 2 are `before` + **one space** + `label`: the space *between* the
pseudo and the text survives as the ordinary inter-word gap, and only the one at the line edge goes.

So the fix is **not** "trim generated content" — it is **hand the edge spaces to the gap machinery**,
which already knows what a line edge is. Trimming without re-emitting the gap deletes the interior
space too: mutation N2 collapses the two-pseudo row to 154.125, the same width as the space-free
control.

⚠ t1107 baked these spaces in **deliberately** — *"the generated text is emitted as ONE unbreakable
word with its spaces baked in, because Chrome bills a trailing collapsible space into the preceding
inline's rect"* — and that reasoning **still holds for the spaces inside the string**, which are
untouched. `lead_ws`/`trail_ws` were already being read off the string for their break
opportunities; they now carry the space as well. The `.hlist` separator gate t1107 landed is green
across this change and is the control that says so.

## ⚠ It did not move the headline, and that is reported rather than buried

```text
                      before   after
  a11yproject          49.3%   49.3%
  martinfowler         89.8%   89.8%
  news.ycombinator     99.9%   99.9%
```

Three anchor sites, unchanged to the decimal. The divergence is real, Chrome-measured and now
gated — and those three pages do not write `content` with outer spaces, so it buys them nothing. A
correctness fix that moves no number is still a correctness fix; claiming otherwise, or quietly not
measuring, is what the fidelity rig's own history warns about.

⚠ Where it *does* matter is the accname fixture that led here: `accname/name/comp_name_from_content`
writes `content: " before "` with deliberate outer spaces on six `fallback content` rows. Those rows
still fail, because they need the **alt** half in the accessible NAME — the t1369 remainder — not the
rendered width. `accname` is flat at 432/484 across this change too.
