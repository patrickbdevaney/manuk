# The agent's drive path — activating a node is not clicking it

> Landed t1366. Gate: `the_drive_path_scrolls_to_its_target_and_refuses_a_covered_one`
> (`agent/tests/`). Track C.

## The one-sentence mechanism

> **`click_by_name` resolved a name to a `NodeId` and fired the element's activation behaviour
> structurally** — follow the `href`, submit the form, flip the checkbox — **so the agent succeeded
> in two places a user could not click.**

## What was there

```rust
pub async fn click_by_name(&mut self, role: &Role, name: &str) -> Result<Activation> {
    let node = self.resolve(role, name)?;
    self.activate(node).await          // ← never asks whether a pointer could reach it
}
```

This is the production entry point behind `Action::ClickText` — the one an agent loop actually
drives. Two consequences, both silent:

* **A target below the fold was activated with the viewport never moving to it.** Every screenshot
  and every subsequent `observe()` then showed a part of the page the agent had not acted on: the
  perception channel and the actuation channel describing different documents.
* **A target under a consent banner reported success.** `Navigated(..)` for a click no user could
  have made — and `to_viewport_lines` had been printing `obstructed` beside that very element since
  t1356. The drive path did not read its own warning.

t1356 built the verification (`A11yNode::landing`) and t1359 gave it the off-screen answer
(`Landing::OffScreen`). t1356's own doc **recorded this hole rather than closing it**:

> *"A caller that holds a node handle may still activate the node directly (`Browser::click_by_name`
> does) — that path is unchanged."*

The rule it violates is already in the wiki index at L62: *agent actions must go through the REAL
hit-test, or agent testing is a privileged bypass.*

## The fix, and why only one of the two answers is an error

`click_by_name` now calls `reach(node, name)` before activating:

| landing | response | why |
| --- | --- | --- |
| `Clear` | activate, viewport unmoved | the common path is unchanged |
| **`OffScreen { dy }`** | **scroll by `dy`, then ask again** | the agent is driving a browser |
| **`Obstructed { by }`** | **error naming `by`** | so the agent can dismiss it and retry |
| `Unreachable` | activate anyway | no geometry ≠ unclickable |

⭐ **`OffScreen` is not a refusal, it is a scroll.** The honest response to *"the thing you named is
900px down"* is to go there — and going there is also what leaves the viewport showing what was
acted on. The scroll goes through `scroll_by`, so it is clamped to the page exactly as a user's
would be, and the landing is re-asked **in the viewport the click now happens in**, which matters
because a `position:sticky` header's document rect moves with the scroll (t1359). The loop runs at
most twice and stops early if the page cannot scroll further, so a target the page genuinely cannot
reach does not spin.

⚠ **`Obstructed` IS refused, and that is the capability rather than a limitation.** An agent told
*"Sign in is covered by `generic "We use cookies"`"* can dismiss the banner and retry. An agent
handed a silent success clicks nothing, sees nothing change, and has no way to find out why.

⚠ **`Unreachable` is deliberately not an error.** It means no box, or on-screen `pointer-events:
none`. An element the layout gave no geometry must not become unclickable for the agent, and the
structural activation is still the best available answer for it.

## The gate

Three arms on one 2400px page, driven through the real `AgentBrowser`:

- **CONTROL** — an on-screen target toggles and the viewport does **not** move. This tick adds a
  reachability step; it must not turn every click into a scroll.
- **THE DRIVE REACHES BELOW THE FOLD** — a target at y=1400 in a 600px viewport toggles, *and*
  `scroll_offset() > 600`. Both halves, because activating without scrolling passes the first.
- **HONEST REFUSAL** — a covered target returns an error whose message contains `covered by`.

⚠ The targets are **checkboxes, not links**, and that is not incidental: `activate` on an `<a>`
performs a real navigation, so a link fixture would make this a network test. A checkbox activates
locally and reports `Toggled`, so every arm measures the drive and nothing else. (The first version
of this gate used links and failed on `fetching https://ex.test/near`.)

Proven red by two mutations. **N1 — drop the `reach` call** (the pre-tick behaviour): the
below-the-fold arm reads `scroll 0` *and* the covered arm returns `Ok(Toggled)` instead of an error.
One call, both defects, which is what makes it the whole tick. **N2 — treat `OffScreen` as an error
rather than a scroll**: the below-the-fold arm fails where browser-like behaviour is to scroll and
click. That is the mutation separating *"verify before acting"* from *"act like a browser"*.

## The new public surface, and why

`AgentBrowser::scroll_offset()` — the document `y` of the viewport's top edge. Exposed because the
drive loop now **moves** it: without it, a click that succeeded structurally and a click a pointer
could have made are indistinguishable from outside, and the gate's load-bearing assertion could not
be written at all.
