# `<span><br><br></span>` reported a box three line boxes tall and as wide as the text before it

**t1511.** An inline whose entire content is `<br>` elements is the oldest vertical-spacer idiom on
the web. `serennu.com` uses `<span class="tiny"><br /><br /></span>` twice in its nav; the
hand-written long tail is full of it.

## Why it was wrong, and why the branch that got it wrong is right

Such an element **owns no fragment**: the break belongs to the `<br>`. So it fell into the
*"an inline that carries no fragment of its own still has an inline box"* branch — written for
`<span><i class="icon"></i></span>`, where the content is an atomic descendant — which emits **two**
reporters, one at the head of the element's items and one at the tail, precisely because *"an inline
that wraps spans several lines and Chrome's rect covers the first line's content top through the last
line's bottom."*

For a br-only inline that is wrong three ways at once: **the head lands at the END of the PREVIOUS
content and the tail at the START of the NEXT line.** The union comes out several line boxes tall,
offset upward, and as wide as the text before it.

## The measurement

`google-chrome-stable --headless=new --dump-dom`, `16px/20px monospace`, `div {width:600px}`.
Transcribed, never derived:

```text
                                      Chrome            before          after
  XX<span><br></span>YY            [19,  0,0,19]    [ 0,  0,19,39]   [19,  0,0,19] ✓
  XX<span><br><br></span>YY        [ 0, 60,0,19]    [ 0, 40,19,59]   [ 0, 60,19,19] ~
  <span><br></span>YY              [ 0,100,0,19]    [ 0,100, 0,39]   [ 0,100,0,19] ✓
  XX<span><br></span>              [19,140,0,19]    [ 0,140,19,20]   [19,140,0,19] ✓
```

⭐⭐⭐ **Chrome's rect is ONE line box tall and ZERO wide in every one of them, and it sits where the
LAST `<br>` sits.** A single reporter inserted immediately before that break reproduces all four
positions and heights. On the 23-value fixture the engine went from **11 of 16 agreeing to 19 of 23**,
with the four remaining rows named below.

**Real site, live and from a local copy of the same document, both readings identical:**

```text
  serennu.com   shape 73.8%  ->  77.0%      coverage 100.0% both ways, 12 misplaced both ways
```

⭐⭐ **It crosses the Phase-0 shape floor of 0.75.** Before: 73.8% at t1496 and 73.8% again in
t1507's slice — two readings, two binaries. After: 77.0% live and 77.0% on a locally served copy.
Stable on both sides of the change, which is what makes a 3.2-point move on one site readable at all
(t1410: repeat before landing a WIN, not only before believing a loss).

## ⚠⚠⚠ THE FLOW WAS ALREADY RIGHT AND MUST NOT MOVE — that is half the gate

All **ten** containing-block heights in the fixture are Chrome-identical before *and* after. This
rule changes what the element REPORTS, never what it contributes to the flow: the reporter is
zero-width and `holds_line: false`, exactly like the two it replaces. A fix that moved a `<div>`
height would be trading a reported rect for real layout, and the `DIV` rows refuse it.

## ⚠⚠ FOUR ROWS ARE STILL WRONG AND THEY ARE IN THE GATE ON PURPOSE

```text
  XX<span><br><br></span>YY        ours [ 0, 60,19,19]   chrome [ 0, 60, 0,19]
  XX<span><br>Q</span>YY           ours [ 0,220,19,19]   chrome [ 0,220,10,19]
  XX<span><br><i></i></span>YY     ours [ 0,280,19,39]   chrome [ 0,300,12,19]
  XX<span><i></i><br></span>YY     ours [ 0,320,31,39]   chrome [19,320,12,19]
```

In every one the surplus width is **19 — the width of the `XX` before the span** — and in every one
the element's content spans **more than one line**. That is a SECOND defect, in the multi-line inline
union rather than in this branch, and it is *stated rather than guessed at*: **a plausible rule that
fits every fixture you happened to write is the most expensive kind of wrong** (t1418). The rows are
asserted against what we produce **today**, with Chrome's number in the failure message and an
instruction to move them into the Chrome table — so a fix fails the gate and is told what to write.

## ⚠ TWO MUTATIONS CAME BACK GREEN, AND THEY CHANGED BOTH THE CODE AND THE GATE

* **The guard had a redundant clause.** It read `any(Break) && all(Break|Spacer)`, and `any` could
  not be falsified: `rposition` already answers `None` when there is no break. **Fifth instance of
  t1403's rule in this arc.** Deleted.
* **The gate could not tell a br-only inline from one that also DRAWS.** Removing the whole guard
  left it green, because no fixture mixed a `<br>` with an atomic. `XX<span><br><i></i></span>YY` and
  its mirror were added for exactly that, and they now turn it red.

⚠ **A third stayed green and is RECORDED rather than gated.** `holds_line: true` on the reporter
changes nothing on any fixture here or any this tick could construct, and the reason is structural:
**the reporter is inserted immediately before a `Break`, and a `Break` brings its own line into
existence.** The flag cannot be falsified at this call site by any input. Kept `false` to match the
reporters it replaces, and the coupling is written down instead of being asserted by a row that would
only look like a control (t1417).

## Where it is

`engine/layout/src/lib.rs` — the no-fragment reporter branch.
Gate: `engine/page/tests/g_an_inline_of_only_breaks.rs`, red under five mutations.
