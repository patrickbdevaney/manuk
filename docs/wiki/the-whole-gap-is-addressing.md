# The whole gap is addressing

`drive-probe` classifies every actionable target an agent can perceive into `Drivable` /
`Ungrounded` / `Ambiguous` / `MisHit`. Adding a **ceiling** column — what the rate would be if every
duplicate could be addressed by ordinal, so only grounding and occlusion can fail — turns the
remaining gap from a number into a decomposition:

```text
                         rate    +landmark   +heading   ceiling(ordinal)
  martinfowler.com      84.3%       89.3%     100.0%        100.0%
  news.ycombinator      66.3%       66.3%      66.3%        100.0%
  blog.rust-lang.org    96.9%       96.9%      98.6%        100.0%
  www.a11yproject       77.6%       77.6%      86.2%         89.7%
  danluu.com           100.0%      100.0%     100.0%        100.0%
  en.wikipedia.org      67.8%       73.5%      75.5%         99.5%
  TOTAL                 78.5%       81.7%      84.3%         99.5%
```

⭐⭐⭐ **The ceiling is 99.5%.** Essentially every target an agent can see is already grounded and
unoccluded — so the entire 21-point shortfall is *which one did you mean*, and none of it is
geometry. That is worth stating plainly because every instinct says the hard part of driving a
browser is hitting the right pixel. On this corpus, hitting the pixel is solved.

## Where the 21 points go

| term | recovers | on which sites |
|---|---|---|
| landmark (t1462) | +3.2 | header-vs-footer duplication |
| heading | +2.6 | martinfowler **84.3 → 100%**, a11yproject +8.6, wikipedia +2.0 |
| ordinal | **+15.2** | everywhere, and it is the only thing that moves news.ycombinator |

**Two semantic terms recover 5.8 points; a positional one recovers 15.2.** `news.ycombinator.com`
has neither landmarks nor headings and does not move for either — 66.3% at every naming term and
100% at the ceiling. So an ordinal is not a convenience API: on this corpus it is the majority of
the fix, and no further *naming* term can substitute for it.

## Two things about the terms that are not obvious

**A heading is a preceding sibling, not an ancestor.** `<h2>` and the content it introduces are
siblings in HTML, so an ancestor walk finds nothing at all. The scope has to be carried through a
flat pre-order scan, unlike the landmark's, which really is an enclosing container.

**`nth` counts document order, not score.** "The third Edit link" means the third one on the page —
what a caller counting them saw. Indexing the score-sorted list returns "the third best match",
which is a different and unusable thing, and on a page of identical links it is arbitrary.

## The method, repeated

This is the third time the loop has priced a term before building it (t1459's landmark column, this
tick's two). It keeps paying because **a priced term arrives with its own limits attached**: the
`66.3 → 66.3 → 66.3` row said, before any code was written, that no amount of semantic addressing
would help a page with no landmarks and no headings.

See also [[the-landmark-is-the-missing-term]], [[role-plus-name-is-not-an-address]].
