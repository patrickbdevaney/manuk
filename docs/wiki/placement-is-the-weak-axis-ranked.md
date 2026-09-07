# Placement is the weak axis, ranked

The observer's `tri-sweep.sh` (t1475, built on `drive-probe` and `a11y-score`) gave the first honest
read of all three exit legs on a 93-site de-botwalled corpus:

```text
  M1  coverage 88.5%  SHAPE 69.4%   ← "placement is the weak axis, long tail <40%"
  M2  rate 82.0%
  a11y F1 79.4%
```

Running a 24-site slice turns that sentence into a ranked list:

```text
  europa.eu                cov 99.2   SHAPE  0.0   ← every box drawn, none in the right place
  developer.mozilla.org    cov 67.8   SHAPE 35.6
  discuss.python.org       cov 100.0  SHAPE 36.6   ← full coverage, bad placement
  angular.io               cov 98.6   SHAPE 45.6
  a11yproject.com          cov 96.0   SHAPE 49.3
  basecamp.com             cov 96.2   SHAPE 56.9
  google.com               cov 93.1   SHAPE 66.7
  arxiv.org                cov 99.5   SHAPE 90.7
  forum.golangbridge.org   cov 100.0  SHAPE 93.5
  codeberg.org             cov 93.8   SHAPE 93.9
  lite.cnn.com             cov 100.0  SHAPE 100.0
```

⭐ **`cov 99.2 / SHAPE 0.0` is the most informative row on the board.** Coverage near 100 with shape
at zero is not a death-tail of many small bugs — it is one systematic placement error affecting
everything. The same shape appears at `discuss.python.org` (100.0 / 36.6).

## europa.eu's signature

`--shape-dump` names it without ambiguity. Chrome lays the lists out as **full-width vertical list
items**; we lay them out in **~305px columns**:

```text
                  Chrome                 Manuk
  li #1     [40   0  1144x18]      [0    0  305x48]
  li #10    [40 162  1144x18]      [305 48  305x48]
  li #17    [40 288  1144x18]      [610  0  305x48]
  ul        [ 0  47  1184x36]      [17  32  200x48]
  section   [ 0 499  1184x83]      [931  0  217x416]

  123 of 123 scored elements misplaced · median dx=281 dy=99
  86 sibling pairs read out of sequence · 1 element escapes the 1200px viewport
```

A whole section placed at `x=931, 217 wide` where Chrome has it at `x=0, 1184 wide` is a column
assignment, not an offset.

## ⚠ And two of our own instruments disagree about our own layout

Loading the *same* 18 KB document through `Page::load_async` + `finish_loading` at the same 1200px
viewport and asking the page itself:

```text
  fidelity says ours is   ul [17 32 200x48]
  a live probe says       ul [d=block, 1184x36]     ← which is Chrome's number
```

Same document (18 178 bytes, 2 `<ul>`, 37 `<li>` either way), same load path, same width — and two
different answers about our own boxes. **One of the two is measuring something else**, and until that
is settled the SHAPE column cannot direct work: it is the number the exit certificate is scored on.

⚠ Leading hypothesis, untested: the harness's own fetch resolves `europa.eu`'s redirect chain
differently from `curl -L`, so the two are laying out different documents that happen to have the
same byte count from my side. That is one `--dump-html` away from settled.

⚠⚠ This is the **third** instrument disagreement of the session, after the self-audit vs `wall-audit`
on the wall's own duration (3945s vs 256s) and the a11y score vs the rendered page (t1470). *When two
instruments disagree about one number, neither is evidence until the disagreement is explained.*

See also [[the-good-score-was-an-unrendered-page]], [[shape-dump-localises-the-mechanism]].
