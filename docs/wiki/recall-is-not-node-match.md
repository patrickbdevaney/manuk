# Recall is not node match

Track B's bar is *">=90% node match against Chrome's accessibility tree."* Until t1458 **no
instrument in the repository could compute it.** Every number the loop has quoted for that bar —
63.8%, 75.0%, 97.0%, and the per-site rows in `g_a11y_name_from_content_context` — came out of a
script under `/tmp` that no longer exists.

Two separate defects hid inside that.

## 1. A gate whose number cannot be recomputed is a memory of a gate

Nothing could re-run it, so nothing could question it, and the value aged into a fact. `a11y-score`
(`agent/src/bin/a11y-score.rs`, arithmetic in `manuk_agent::a11y_score`) is the versioned
replacement: it launches headless Chrome, reads `Accessibility.getFullAXTree` over CDP, builds
Manuk's tree through `AgentBrowser`, and prints both sides.

## 2. The half it computed was recall

A multiset match taken over **the oracle's** nodes answers only *how many of Chrome's nodes did we
produce.* It cannot see nodes we invent, and it **improves as the projection gets noisier** — the
wrong direction for a bar to move under a mistake.

```text
                                manuk  chrome  match     prec   recall      F1
  martinfowler.com                425     297    290    68.2%    97.6%   80.3%
  news.ycombinator.com            479     497    478    99.8%    96.2%   98.0%
  blog.rust-lang.org             1678    1673   1672    99.6%    99.9%   99.8%
  www.a11yproject.com             173     158    146    84.4%    92.4%   88.2%
  danluu.com                      414     416    414   100.0%    99.5%   99.8%
  en.wikipedia.org/wiki/…        2629     779    682    25.9%    87.5%   40.0%
  TOTAL (pooled)                 5798    3820   3682    63.5%    96.4%   76.6%
```

**`96.4%` is the number the loop has been reporting.** Precision is `63.5%`; F1 — the one to steer
on — is `76.6%` against a `>=90%` bar. Wikipedia publishes **2,629 nodes where Chrome publishes
779**: an agent resolving a target on that page is choosing from a list that is three-quarters
phantom, and a screen reader reads every one of them aloud.

The instrument reproduces the only hand-computed data point that existed — martinfowler at
**68.2% / 97.6%** against the remembered **67.7% / 97.3%**. That agreement is what licenses the rest
of the table; without it this would be a new number rather than a corrected one.

## The drops are listed because they flatter

Both sides drop `generic` / `none` / `presentation` (no role, no name — a pure wrapper-count
difference) and `StaticText` / `InlineTextBox` (Chrome's text leaves, which Manuk folds into its
parent's name), plus anything Chrome marks `ignored`. Every one of those makes the score kinder, so
the binary prints how many it dropped on each side. **A drop that flatters has to stay visible or it
stops being a modelling decision and becomes a thumb on the scale** — note that Chrome drops far
more than Manuk does on every row, which is itself the phantom story in miniature.

## Two things this cost, both worth remembering

**An unbounded read inside a bounded call reports the deadline, never the reason.** Chrome's
DevTools HTTP server ignores `Connection: close` and never closes the socket, so `read_to_end`
blocks forever. Wrapped in a timeout that presented as a *connect* failure, and sent several
attempts chasing stray processes and port collisions before the body was read by `Content-Length`.

**The port in `Host:` is load-bearing.** Chrome builds each target's `webSocketDebuggerUrl` by
echoing back the request's `Host` header, so `Host: 127.0.0.1` yields `ws://127.0.0.1/devtools/...`
with no port at all.

## And the gate's own fixture had the same shape as the bug

`multiset_overlap` iterates its *second* argument, so a fixture with the extra duplicates on **our**
side scores 2 under both a multiset and a set intersection — the mutation came back **green**. The
duplicates have to sit on the **oracle's** side to discriminate. Found by running the mutation, not
by reading the fixture; see [[the-fixture-is-part-of-the-instrument]].

See also [[accesskit-the-interop-shape]], [[a11y-tree-meets-a-real-website]].
