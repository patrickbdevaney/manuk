# A lockstep gate scoped to one value

This project keeps its UA defaults in **two cascades** — `stylo_engine.rs`'s `UA_CSS` and
`apply_ua_defaults`' `MinimalCascade` — and a comment has said *"keep in lockstep"* the whole time.
A comment cannot go red, so t853 turned it into a gate:
`both_ua_sheets_agree_on_which_elements_are_block`.

It reads `UA_CSS`'s **`display: block`** rule. Nothing else.

`option, optgroup { display: none }` sat one screen above it, absent from the minimal cascade, for
as long as both existed. **A gate named for a plural asserts a sample and reads as a population** —
and this one was built specifically to enforce the lockstep it was not enforcing.

## What the uncovered half cost

A `<select>` exposed **no options at all** under Stylo and every option under MinimalCascade. Which
you got depended on a cargo feature, so no suite could see it: WPT runs Stylo and never asks an
accessibility question; the agent asks the accessibility question and was running MinimalCascade.

## And the drift recurred within one tick

t1463 corrected the Stylo sheet to Chrome's `block` and left `apply_ua_defaults` at its `inline`
fallback — the same divergence in the opposite direction, **one tick after the journal entry quoting
the lockstep comment**. A rule that is only prose gets obeyed only when someone remembers it.

## The `display: none` half found a third drift on its first run

```text
                         Chrome                     UA_CSS      MinimalCascade
  <audio>                none, 0x0                  none        inline      ← drift
  <audio controls>       inline, 300x54             none        inline      ← and WRONG in both
  <video>                inline, 300x150            —           —           ✓
```

`audio { display: none }` unqualified hid the one form of the element anybody ever sees. Chrome's
sheet is `audio:not([controls])`, and the difference is a rendered control bar. Both cascades were
wrong, in opposite directions, and neither suite had noticed.

⚠ Fixed to `audio:not([controls])` in the sheet and a matching conditional arm in
`apply_ua_defaults`. The computed value is now Chrome-exact in both cascades; the **box** is not —
ours is `0x17` against Chrome's `300x54`, because there is no audio control-bar widget with an
intrinsic size. Named, not claimed.

## Measured

```text
  WPT html/semantics/embedded-content   863 failing -> 863   0 fixed / 0 new
  WPT css/css-display                   211 failing -> 211   0 fixed / 0 new
```

## ⚠ And a harness lesson that cost most of the tick

Two background control-build jobs were alive at once. Each does *revert → build → restore*, so they
took turns reverting each other's restore, and the working tree silently returned to HEAD **three
times** while `grep -c` on a string that also appears in a nearby comment said the change was still
there.

- **Verify a restore by the LINE, not by a count.** `grep -n "^audio"` distinguishes the rule from
  the comment that names it; `grep -c "audio:not(\[controls\])"` does not.
- **Never run two in-place control builds concurrently.** They share one working tree.
- **`pkill -f <pattern>` matches your own shell**, because the pattern is in its command line — one
  of these killed the script that was about to restore the tree. Kill by PID.
- **Cargo reused the control build's artifacts** after the restore, so the tests reported HEAD's
  behaviour from correct sources. `touch` the files, or check what you are actually running.

See also [[a-crate-that-omits-a-feature-substitutes-an-engine]],
[[an-option-is-not-hidden-by-a-stylesheet]].
