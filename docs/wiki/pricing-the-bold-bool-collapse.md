# Pricing the `bold: bool` collapse

*Tick 1491. A 30-call-site refactor, priced before it was written — and the answer was mixed in a way
only a measurement gives.*

## The candidate

`manuk_text::FontKey` carries `bold: bool` at a 600 threshold, so weights 300/400/500 collapse to one
face and 600/700/800 to another. Confirmed against headless Chrome on a clean system-font fixture — no
webfont, no network, no race:

```text
  font-family: "Lato", 32px      400    500    700
  Chrome                         312    314    320
  ours                           312    312    320
                                        ^^^ weight 500 gets our 400 face
```

Replacing it with a numeric weight plus CSS Fonts §5.2's closest-match rule means **30 construction
sites across 9 files**. So the question that has to come first is the one t1367–1374 made a rule —
*price the mechanism on the corpus before building* (three of four candidates then priced at ~0):
**how many of the advance disagreements are even AT a weight the collapse touches?**

A miss at weight 400 or 700 cannot be explained by it.

## The price

```text
                       at a COLLAPSING weight    at 400/700
  payb.jp                      0                    223      <- explains NONE of the worst site
  puentedemando              217                     65
  pivaldi                     46                      4
  ru.restaurantguru           16                      0
  razaoautomovel               6                      7
  ─────────────────────────────────────────────────────────
                             285                    299

  the collapse can explain AT MOST 49% of the font-disagreeing misses
```

⚠ **AT MOST is the honest word.** "Sits at a collapsing weight" is an **upper bound**, not a count of
misses the collapse explains — a 500-weight element can diverge for some other reason too. *A
failure-signature count is an upper bound, not an estimate* (t1347, where 144 became 12).

## What that decides

**The refactor is justified and it is not the whole answer.** It is worth ~half the term on four of
five sites, and it is worth **nothing** on `payb.jp` — the site with the highest font-disagreement
rate in the cohort (420 of 432 misses), whose text is entirely at 400/700 in `Noto Sans JP`. That is a
second, separate mechanism, and it was hiding behind an aggregate that looked like one problem.

Had the refactor been written first, it would have been correct, landed, and left the worst site
exactly where it was — with no way to tell whether it had helped, because the aggregate would have
moved.

## The instrument

`--shape-dump` gains a `SHAPE MISS WEIGHT` line: of the misses whose font strings disagree, how many
sit at a weight the `bold: bool` key collapses. It reads our own computed `font-weight` from the same
style map the producer measured with.

⚠ That needed a path→weight map keyed exactly as `mseen` ends up keyed, so the `.SIG`-stripping key
transform was extracted from `strip_sigs` into `strip_sig_key`. **One implementation, two callers** —
two copies of that transform is precisely how two maps keyed by "the same path" stop agreeing, which
is the failure the signature itself caused (t550).
