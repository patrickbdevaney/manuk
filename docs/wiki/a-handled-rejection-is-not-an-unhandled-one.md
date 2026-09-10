# A handled rejection is not an unhandled one

*Tick 1482. The report fired at the wrong moment, and two alarms in three were false.*

## The fifth path

t1480 built `Page::boot_errors` over the four ways a page's script can die, and left out the one
whose own log line says why it matters most:

> *"UNHANDLED PROMISE REJECTION — a page's async code threw and nothing was listening. **Every modern
> framework renders inside an async function, so this is where their failures go to die.**"*

Measured on the 40-site CrUX slice immediately after t1481: **80 unhandled rejections, none of which
reached the harvest**, against 11 errors from every other path combined. The histogram built to rank
the web's boot failures was seeing roughly a tenth of them — and the rows it was missing were the
nameable ones:

```text
  20  IndexedDB          ("no object store named e" ×16, "e.open is not a function" ×4)
   8  HTMLSlotElement    ("e.assignedElements is not a function" ×4 + intermediate-value ×4)
```

## And routing them in immediately exposed an older bug

The boot-error gate asserts that a **clean page reports zero**. The clean fixture gained a caught
async throw — `(async () => { throw … })().catch(() => {})` — and the clean page reported one.

```text
  A  rejected, `.catch` attached SYNCHRONOUSLY on the same line
  B  rejected, `.catch` attached from a LATER MICROTASK (still inside the checkpoint)
  C  rejected, never handled at all

           Chrome              before                        after
  n=       1                   3                             1
  msgs     [C-never]           [A-caught, B-late, C-never]   [C-never]
```

**Two false alarms out of three.** On the one event every error-reporting SDK (Sentry, Bugsnag,
Rollbar) and every app's own *"something went wrong"* UI listens to — and an app that BRANCHES on it
(shows an error screen, logs the user out, retries a request) did the wrong thing on a working page.

## Why the timing is the algorithm

HTML §8.1.7.5 keeps a list of *about-to-be-notified rejected promises* and drains it in **"notify
about rejected promises"**, at the **end of a microtask checkpoint**. The delay is not an
optimisation. The overwhelmingly common shape

```js
  (async function () { … })().catch(handle);
```

rejects the promise **before** `.catch` is attached: the async function returns an already-rejected
promise and the handler goes on the next expression. An engine that reports at the instant
SpiderMonkey's tracker fires necessarily reports failures the page has fully handled.

## How it is decided

The parked list holds the **promise object**, in a `RootedTraceableBox<Heap<*mut JSObject>>` so a GC
between the rejection and the checkpoint cannot collect it, and asks SpiderMonkey at drain time
whether it is *still* unhandled (`GetPromiseIsHandled`) — rather than tracking the `Handled`
transition ourselves, which would be a second and weaker copy of a fact the engine already owns.

⚠ **The drain runs AFTER the checkpoint's second microtask pass, not between the two.** A microtask
run by that pass is exactly where case B's `.catch` is attached.

⚠ The list is capped at 256 per checkpoint. Past the cap a rejection is reported immediately — the
OLD behaviour, and therefore never worse than dropping it.

## ⚠ Found by a vacuity arm, not by reading the tracker

Nothing in this tick set out to fix the rejection timing. The harvest did not create the bug; it made
an existing one observable **at a boundary that already had an assertion on it**. That is the fourth
time in three ticks that a gate's *"and a clean input must report nothing"* half has been the half
that found something, after the a11y denominator (t1470), the script-free document (t1480) and this.

*An arm that only asserts the positive case cannot tell a working detector from one that fires on
everything.*

## The gates

`g_a_handled_rejection_is_not_reported` — the Chrome row above exactly, with **case C as the vacuity
arm**: deferring a report is trivially "correct" if you defer it forever, so the gate that only
asserted A and B are silent would be passed by deleting the reporter. Red under four mutations
(report from the tracker; skip the handled check; never drain; drain before the second pass).

`g_a_page_reports_its_own_boot_failure` gains a fourth path and a fourth mutation: a rejection must
reach the harvest, and a **handled** one must not.
