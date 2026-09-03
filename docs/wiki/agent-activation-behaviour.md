# The agent's click was a second, wrong implementation of the click's activation behaviour

## The one-sentence mechanism

`manuk-page`'s `Page::dispatch_click` **is** the platform's click activation behaviour — built,
Chrome-arbitrated and gated over many ticks — and `manuk-agent`, the consumer this project exists to
serve, **never called it**: `AgentBrowser::activate` re-derived the whole rule from a twenty-line
`match` on the tag name, and got five things wrong that Chrome answers differently.

## Why this class of bug is invisible from inside either half

Both halves were individually correct-looking, and each had its own tests. The engine's dispatcher
has gates for `<label>` forwarding (`g_label_click`), the disclosure widget (`g_details`), the
submit path (`g_submit_click`) and the cancelled-activation undo (`g_click_activation`). The agent
had gates for *reachability* (`g_agent_drive_reaches_its_target`) and *aim*
(`g_agent_click_lands_on_its_target`). **Nothing anywhere asserted that the two implementations
agreed**, and because the agent's version answered `Toggled(true)` / `Inert` rather than throwing,
every divergence was a silent wrong answer.

> ⭐⭐⭐ **The generalisation: when a rule has two implementations, the tests of each one are
> evidence about that one only. The only thing that catches the divergence is a gate on the
> CONSUMER'S path, driven end-to-end.** The same shape as t720's *"one rule, N implementations"*,
> t1027's *"two copies of one fix"*, and t1355's *"a property with two entrances, one unguarded"* —
> here it was a whole subsystem with two entrances.

## What Chrome says, and what we said

Headless Chrome 145.0.7632.116, `element.click()` on each construct, state dumped from the same
document (`/tmp/arb/act.html`, reproduced in the gate's module doc):

| click | Chrome | agent, before |
|---|---|---|
| `<summary>` of a closed `<details>` | opens it | `Inert` — the section could never be opened |
| `<summary>` of an open one | closes it | `Inert` |
| `<label for=cb>` | ticks `cb` | `Inert` |
| markup inside a wrapping `<label>` | ticks its control | `Inert` |
| second radio of a `name` group | selects it, **deselects the first** | **both** end up checked |
| the same radio twice | stays checked | unchecks it |
| `disabled` checkbox | nothing | ticks it, reports `Toggled(true)` |
| `disabled` submit button | nothing | submits the form |
| `<span>` inside `<button>` in a form | submits | nothing |
| `<span>` inside `<a href>` | navigates | nothing |
| `<button>` outside any form | nothing observable | `Err("NoForm")` |
| `<option>` inside a `<select>` | `select.value` **unchanged** | (we agree — leave it alone) |

⚠ **The cost is not that the actions failed; it is that nothing could tell.** `Toggled(true)` on a
disabled consent box means an agent reads its own success out of a form the server will reject.
`Inert` on a disclosure is indistinguishable from *"this page has nothing to open"*. A retry loop
cannot recover from either, and both are ordinary markup — `<details>`/`<summary>` is on 3 of 36
CrUX-sampled pages (51 hits), `<label>` on 6, `disabled` on 8, `<button>` on 12.

## The fix — one implementation, and the host keeps only what is genuinely the host's

`AgentBrowser::activate` now dispatches through `Page::dispatch_click` and **observes** the result.
What stays in the agent is the part that belongs to the embedder rather than the document:

1. **Following a link.** `dispatch_click` deliberately does not navigate — it returns whether the
   default action survived `preventDefault()`, and fetching is the host's job. So a link's handler
   can now cancel the navigation, which is what every SPA router does.
2. **Performing a queued submission**, by draining `Page::take_form_submits()`. That is also how the
   engine hands a *script's* `form.requestSubmit()` to its host, so the agent honours both through
   one path — and the submission is queued as *requested*, so the page's own `submit` validator runs
   first.
3. **Reporting.** `Activation` is now *measured*, never assumed: the checkedness and the `open` flag
   are read back out of the DOM **after** the dispatch.

```rust
Activation::Toggled(bool)     // ONLY when the checkedness actually CHANGED
Activation::Disclosed(bool)   // a <details> opened or closed (new)
Activation::Inert             // including: a disabled control, and a formless <button>
```

⭐ **`Toggled` reporting an observed CHANGE rather than an intent is what makes the disabled arm
falsifiable.** "I clicked a checkbox" and "a checkbox changed" are different claims, and only the
second is checkable from outside.

## And the same rule, three times: a click's activation behaviour belongs to the nearest ANCESTOR

A click lands on whatever is under the pointer — the `<span>`, the `<b>`, the `<svg>` chevron —
essentially **never** on the control's own box. `Page::summary_details_target` already walked up for
exactly this reason and said so in its doc comment. Its two siblings did not:

| query | before | after |
|---|---|---|
| `summary_details_target` | walks up ✅ | unchanged |
| `labeled_control` | `el.name != "label" → None` | walks up, **stopping at a labelable element** |
| `submit_target` | exact match on the clicked node | walks up, **stopping at a non-submitting button/input/form** |

⚠ **Each walk needs its own termination rule, and they are not the same rule.**

- `labeled_control` stops at a **labelable** element (`input`/`select`/`textarea`/`button`/`meter`)
  because otherwise `<label><input></label>` clicked on the input would travel up to the label and
  forward back to itself forever. The control being the *nearer* ancestor is both the right answer
  and the termination condition.
- `submit_target` stops at a **non-submitting** `button`/`input`, and at the `form`. Walking past a
  `<button type=button>` inside a form would find the form through some *other* submit button and
  submit it — a page that meant "do not submit" would submit.

⭐ **And once the walk exists, `submit_target` must return the SUBMITTER, not the clicked node.**
Two things depend on the difference: the recorded name/value (`<button name=action value=delete>`
beside `value=save` is how many forms say what the user asked for), and disabledness —
`Page::is_disabled` only propagates through a `<fieldset>`, so a `<span>` inside
`<button disabled>` is **not** disabled, and asking the hit node made every disabled icon-button
live again.

## What this tick did NOT prove, and why

`dispatch_click` also fires `mousedown` → `mouseup` → `click` → `input` → `change` and honours
`preventDefault()`. The agent inherits all of it *by construction* — but **`manuk-agent` builds
`manuk-page` without the `spidermonkey` feature and has no way to enable it** (`agent/Cargo.toml`
declares no features), so no page script runs in an agent build at all and the gate can prove none
of the event half. Every arm asserted is pure UA activation behaviour, on a page with no `<script>`.
Recorded rather than claimed; giving `manuk-agent` opt-in `stylo`/`spidermonkey` features is its own
tick.

## Two defects measured here and left open

1. ⭐⭐⭐ **`resolve_handle` returns the WRONG element for names that differ by a short token.**
   `targeting::keywords` drops tokens shorter than two characters (`agent/src/targeting.rs:24`), so
   the distinguishing token of `"Radio B"` disappears and all three radios score *identically*:

   ```text
     INTENT "Radio A" -> "Radio C"   score 0.73571503
     INTENT "Radio B" -> "Radio C"   score 0.73571503
     INTENT "Radio C" -> "Radio C"   score 0.73571503
     INTENT "Sign in" -> "Sign in"   INTENT "Delete" -> "Delete"    (>=2-char tokens are fine)
   ```

   An exact, complete, unambiguous name match **does not win**, and the tie falls to tree order —
   so an agent told *"select Option A"* selects Option C. Single-character tokens are pagination
   (`Page 2`), wizard steps (`Step 1`), sizes (`S`/`M`/`L`) and option letters. This is the same
   family as t1389's *"the drive path picked the first substring match"* and it survived that fix.

2. **`<summary>` and `<label>` are `Generic ""` in our accessibility tree** — no role, no name. The
   `<details>` is `Group` with `expanded`, correctly, but the *control* that operates it is
   invisible to the agent's perception channel, so the disclosure is drivable only by COORDINATE.
   Chrome exposes `<summary>` as `DisclosureTriangle`. Arbitrating that needs the CDP a11y oracle
   (`getPartialAXTree`), which is why it is a separate tick rather than a guess made here.

3. **`<input type=reset>` does not reset.** Chrome restores each control's default value; we do
   nothing. Priced at 0 of 36 CrUX-sampled pages, so recorded and not built.

## The gate

`G_AGENT_ACTIVATION_BEHAVIOUR` (`agent/tests/g_agent_activation_behaviour.rs`) — nine arms plus
controls, driven through the real `AgentBrowser`, **observed through the accessibility tree**
(`state.checked`, `state.expanded`, and the observation text the model actually reads). No script on
any fixture page, so nothing asserted depends on a feature the agent build does not have, and a
green result cannot be an artefact of a test poking the DOM.

Every coordinate is **derived from perceived geometry** (`inner_point`, and offsets from the named
control's own box) rather than hard-coded, with a `hit_role_at` vacuity assertion proving the point
really lands on descendant markup — because two of the six mutations below were GREEN on the first
version of the gate, for exactly the reason that the inner `<span>` has no accessible NAME to
resolve by and the arms had silently fallen back to the control itself.

> ⚠ **That is the reusable warning: an arm that resolves its target by NAME cannot test
> descendant-markup behaviour, because the descendant has no name.** The mutation that stays green
> is the only thing that says so.

**PROVEN RED by six mutations.**

| # | mutation | red arm |
|---|---|---|
| M1 | `activate` does not call `dispatch_click` at all (the defect's core) | both tests, 6 arms |
| M2 | `submit_target` reverted to an exact match on the clicked node | ARM 9 |
| M3 | disabledness asked of the hit node instead of the submitter | ARM 6b |
| M4 | `labeled_control` reverted to `el.name != "label" → None` | ARM 3 — **and ARM 2 stays green**, which is exactly the shape that hid this bug: the bare-text label worked, the wrapped-span one did not |
| M5 | radio-group exclusivity removed (`if false` on the peer loop) | ARM 4, reproducing the old signature `(True, True, False)` |
| M6 | `Toggled` reported unconditionally rather than on an observed change | ARM 4b |
