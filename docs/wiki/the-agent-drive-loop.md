# Perceive, act, observe — the loop closed, and closing it found three defects

**Tick 1455 — Track C started.** Every component had been built and gated for a long time and nothing
had ever composed them. Surface audit #87 named that one tick earlier: *"assembly is exactly the work
that stays undone, because no single piece of it looks like a tick."*

## The loop

```text
  1. PERCEIVE  read the a11y tree; find the target by ROLE + ACCESSIBLE NAME
               — never by selector, id or DOM order, because an agent has none of those
  2. GROUND    take the node's own bounding box and aim at its CENTRE
  3. ACTUATE   dispatch a real click at that COORDINATE, through hit-testing
  4. OBSERVE   re-read the tree and confirm the state CHANGED
```

> ⭐⭐ **Step 4 is the one that makes the other three worth anything.** An agent that clicks and cannot
> see the result either proceeds on faith or clicks again and undoes itself. The M2 milestone is not
> "can it click" — it is **can it verify its own action**.

## Assembling it found three defects, and none of the pieces was wrong on its own

**1. `checkbox` and `radio` were not name-from-content, while `switch` — the third member of the same
ARIA family — was.** ARIA 1.2 gives all three `nameFrom: contents`, and CDP
`Accessibility.getFullAXTree` confirms Chrome does:

```text
  <div role=checkbox>Remember me</div>   chrome  checkbox "Remember me"   ours ""
  <div role=switch>Dark</div>            chrome  switch   "Dark"          ours "Dark"
```

⭐ **A native `<input type=checkbox>` has no content, so this was invisible to every native-control
fixture.** It only bites the `<div role="checkbox">` modern web apps actually ship — the one an agent
most needs to name. The agent could *see* the control, *ground* it and *click* it, and had no way to
**refer** to it.

**2. A natively disabled control dispatched every listener on it.** HTML makes a disabled
`button`/`input`/`select`/`textarea`/`fieldset` inert to pointer activation — the event is not
dispatched at all, not even through `element.click()`. Chrome-measured:

```text
  <button disabled> + .click()            chrome: handler does NOT run   ours: RAN
  <div role=button aria-disabled=true>    chrome: handler DOES run       ours: ran ✓
```

> ⚠⚠ **This is an agentic-SAFETY defect, not a conformance one.** An agent clicks a disabled "Submit",
> the page's own handler runs, the observable state changes, and the agent concludes the action
> **worked**. Nothing in 1,400 ticks had found it, because *a positive control cannot fail this way* —
> it took a negative row in an end-to-end loop.

**3. `aria-disabled` must still fire**, and suppressing both would have been a different bug wearing
the same fix. It is advisory: it tells assistive technology the control is unavailable without
changing what the DOM dispatches, and many real UIs use it precisely so their own handler can explain
why.

## Gate

`engine/page/tests/g_agent_drive_loop.rs` — the four steps over three control shapes (`aria-pressed`
toggle, `aria-checked` checkbox, `aria-expanded` disclosure), plus a **native-disabled negative** and
an **`aria-disabled` control on that negative**. Red under C1 (click a no-op), C4 (let native-disabled
through), C5 (suppress `aria-disabled` too) and C6 (drop `CheckBox`/`Radio` from name-from-content —
the agent cannot find its target at all).

## What this does NOT prove

The board asks for the drive loop on **one real site**, and a gate cannot depend on the network. This
proves the four components compose. It does not prove a real page's markup is reachable this way —
`agent-run` is where that measurement belongs, and it is the next Track C tick.
