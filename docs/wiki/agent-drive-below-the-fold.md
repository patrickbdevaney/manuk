# The agent drive loop below the fold — a verified click point is verified *in one viewport*

> Landed t1359. Track C. Gate: `an_agent_reaches_a_target_below_the_fold` (`agent/tests/`).
> The companion to the drive-loop material in [the interaction surface](./interaction-surface.md)
> (t1356) — and the branch that fix did not cover.

## The one-sentence mechanism

> **The obstruction map at scroll 0 is not the obstruction map at the scroll where the click
> happens** — so a click point verified against the viewport a target was *found* in says nothing
> about the viewport the click actually occurs in, and a below-the-fold target is by definition
> clicked after a scroll.

## What was there

t1356 made a click point a checked claim. `A11yNode::landing(target, viewport)` hit-tests candidate
points back to the target before an agent is handed one, so a *Sign in* button under a consent
banner grounds as `Obstructed` rather than as a confident coordinate that activates the banner.

That function has three outcomes and the fix wired two of them:

```rust
let (point, obstructed_by) = match tree.landing(best.node, Some(viewport)) {
    Landing::Clear      { point }     => (point, None),
    Landing::Obstructed { by, point } => (point, Some(by)),
    Landing::Unreachable              => (bbox.center(), None),   // ← here
};
```

`Landing::Unreachable` is what a target gets when **no part of its box is inside the viewport**.
Which is to say: everything below the fold, on every page on the web. The third arm handed back the
bare box centre with **no flag**, so `ground_action` reported

```text
Ready { node: "Far target", point: (140, 1030), confidence: 1.0 }
```

for a coordinate the viewport does not contain.

## Why that is invisible rather than merely approximate

The engine's a11y boxes and `Page::dispatch_click_at` both work in **document** coordinates, so a
naive reading says an off-screen document coordinate is harmless — the click "reaches" the box
anyway. It does not, and the reason is scroll-dependent geometry.

`Page::restick` re-bakes `position:sticky` shifts into the box tree for the current scroll. A sticky
header's *document* rect therefore **moves with the view**. The probe, verbatim, on a checkbox at
y=1000 under a `position:sticky; top:0` header, viewport `0..700`:

```text
landing(target, viewport 0..700)  = Unreachable
ground_action                     = Ready { point: (140,1030), confidence: 1.0 }
  ... the agent scrolls to y=1000, which is the only way a pointer reaches the target
  the header's document rect re-sticks to y=1000..1070 — ON TOP OF THE TARGET
hit_test(140, 1030)               = the header
dispatch_click_at(140, 1030)      = proceed: true
state.checked                     = False
```

Right node, maximum confidence, wrong coordinate, `proceed = true`, nothing happened. The same
silent-misfire signature t1356 was written to eliminate, one branch over. And it is not a corner
case: *scroll to the control, then click it* is how the web is driven, and it is exactly the motion
that slides a sticky header over the thing you scrolled to.

## The fix — a variant that carries no point

`Landing` splits the conflated case:

| variant | meaning |
| --- | --- |
| `Clear { point }` | verified: hit-tests back to the target or a descendant |
| `Obstructed { by, point }` | on screen, something is on top of it — `by` is what to dismiss |
| **`OffScreen { dy }`** | **outside the viewport — scroll by `dy` and ask again** |
| `Unreachable` | no box, or on screen and nothing in it hit-tests back (`pointer-events: none`) |

and `Grounded::OffScreen { node, name, dy, confidence }` **carries no point at all**. That is the
load-bearing design choice: a caller cannot act on a coordinate that does not exist, so it is forced
to scroll and re-ground, which re-runs the verification in the viewport the click will happen in.
Re-grounding is not politeness; it is the correctness argument.

Three consequences worth stating:

1. **`dy` is a proposal, not a promise.** It aligns the target's top edge with the viewport top —
   the alignment `Element.scrollIntoView()` defaults to (`block: "start"`). A caller that cannot
   scroll that far (the document ends first) still gets a truthful answer, because it re-grounds
   against wherever it landed rather than against this number.
2. ⚠ **A vertical scroll only helps a target already inside the viewport's HORIZONTAL band.** A box
   parked off to the side comes no closer for any `dy`, and reporting one would send an agent
   scrolling the whole document and asking again forever. That case stays `Unreachable`, and it is
   a pinned negative row.
3. **"Where is it" is asked before "what is on top of it."** An off-screen target has no obstruction
   answer yet, because the obstruction map belongs to a scroll position the caller has not reached.

Narrowing `Unreachable` also turns a standing comment into a checked claim: `to_viewport_lines`
filters to boxes intersecting the viewport and its `Unreachable` arm said *"filtered above, so this
is the `pointer-events: none` target"*. With `OffScreen` split out, that sentence is now enforced by
the type rather than asserted in prose.

## The gate

`an_agent_reaches_a_target_below_the_fold` (`agent/tests/`) — one page, five arms, driven end to end
through `manuk_page::Page`: **perceive → scroll → perceive again → act → observe**, with the
observable being `state.checked` read back out of the *same* a11y tree the agent aimed with. No
script on the page, so it holds in a build without SpiderMonkey and a green result cannot be an
artefact of the test poking the DOM.

- **CONTROL** — an on-screen target is unchanged: box centre, one round, toggles.
- **THE LOOP CLOSES** — below the fold, and the 140px sticky header re-sticks over its centre once
  you get there. `OffScreen{dy}` → scroll → re-ground → the ladder rescues a point **below** the
  header → the checkbox toggles. A vacuity assert pins that the header really did move.
- **THE OLD WAY, ON AN IDENTICAL TARGET** — aim at the bare box centre, scroll, click: it does
  **not** toggle. The failure is demonstrated in the same run as the fix, so no later edit can
  satisfy the other arms by renaming something.
- **HONEST REFUSAL AFTER THE SCROLL** — off screen *and* covered gets `OffScreen` first and
  `Obstructed` second, naming the wall.
- **PINNED NEGATIVE** — the horizontally-parked target is `Unreachable`, never `OffScreen`.

Proven red by five mutations: the defect itself (`OffScreen` falling back to the box centre
unflagged), `landing` never reporting `OffScreen`, `dy = 0`, dropping the horizontal-band guard, and
swapping the arm order so obstruction is asked first.

## The rule this generalises to

⭐ **A fix that adds a check to a `match` is finished only when every arm has been asked what it now
means.** t1356 added the hit-test to two arms and left the third holding the pre-fix fallback; the
third arm's meaning ("no part of its box is inside the viewport") was never the same question as the
other two, and the shared variant hid that. The tell is a fallback expression that is *identical to
the pre-fix code* — `bbox.center()` with no flag — sitting inside the match that removed it
everywhere else.

⚠ A unit test can pin the bug's own premise. `a_click_point_is_inside_the_part_of_the_target_that_is_on_screen`
asserted `Unreachable` for "scrolled past it entirely", with the comment *"there is nothing to aim
at, and a coordinate would be a guess"* — which is the conflation, written down and gated. It was
corrected to `OffScreen { dy: -1200.0 }` (negative: an agent that scrolled too far is told to come
back up) and given a genuine `Unreachable` row of its own.
