# THE WIKI — what we durably KNOW, organised by subsystem

This directory is **structurally different from everything else in the constellation**, and the
difference is the whole point:

| File | Shape | Answers |
|---|---|---|
| `JOURNAL.md` | a **log**, chronological, append-only | *"what happened in tick N?"* |
| `STATUS.md` | a **snapshot**, regenerated | *"where does the project stand right now?"* |
| **`docs/wiki/`** | **topic files, continuously revised** | ***"what do we now durably know about how this engine — and the web platform — actually work?"*** |

Neither a log nor a snapshot can answer the third question, and that knowledge is exactly the kind
that **ebbs and flows with the context window instead of accumulating anywhere.**

## The rules, and they are what keep this from becoming a second journal

1. **Organised by TOPIC, never by tick.** A tick that learns something about font metrics **edits
   `text-layout.md`**. It does **not** create `tick-247-font-notes.md`. *If this directory ever starts
   accumulating one file per tick, it has become the write-heavy/read-light failure the journal already
   diagnosed, rebuilt in a new location — and it is worthless.*

2. **State the MECHANISM, not the event.** The bar is deliberately higher than a journal note:

   > ✅ *"SpiderMonkey's `JS::JobQueue` must be installed with `SetJobQueue` **before** `InitSelfHostedCode`,
   >    and mozjs's `Runtime::create` calls the latter unconditionally — so the internal job queue can
   >    never be used from mozjs, and a newer mozjs cannot fix it."*
   >
   > ❌ *"Fixed the promise queue bug in tick 30."*

   If an entry only restates the journal in different words, **it does not belong here.** If it teaches
   something a future session — **or an external reader who was never here** — could act on without
   having lived through the tick, it does.

3. **Write for a COLD READER.** Someone with zero memory of any conversation opens the topic file for
   the area they are working in and learns what is actually known. Audited the same way as everything
   else: pick a topic, simulate a cold read, confirm it teaches what it should.

4. **Every tick either updates a topic file or says it did not.** Enforced by the pre-commit hook via a
   `WIKI:` trailer. **"No durable knowledge this tick" is a legitimate and expected outcome** — plenty
   of ticks are mechanical. The point is not to force every commit to invent something; it is to make
   **capture the default** and **skipping an explicit, auditable choice** rather than a silent gap
   nobody notices until it is gone.

## How this relates to the rest of the constellation

- **The lesson-promotion mechanism** (recurring patterns → `CLAUDE.MD` / a gate) does a *different*
  job: it is for the few things short enough and critical enough to load into **every** session's
  initial context. **This wiki is the full cumulative record**, organised for **lookup when working in
  a given area** — not for blanket loading.
- **`docs/loop/PROCESS.md`** records *process* defects (how we work). This records *domain* knowledge
  (how the web and the engine work). A tick can produce both.

## The topics

| File | Covers |
|---|---|
| [`js-engine.md`](js-engine.md) | SpiderMonkey/mozjs integration realities, the event loop, timers, globals |
| [`dom-semantics.md`](dom-semantics.md) | DOM spec behaviour, mutation, tree edge cases |
| [`css-cascade.md`](css-cascade.md) | Stylo realities, selector/cascade quirks actually encountered |
| [`text-layout.md`](text-layout.md) | fonts, shaping, measurement, layout |
| [`networking.md`](networking.md) | HTTP, caching, and how real sites actually load |
| [`frameworks.md`](frameworks.md) | React/Svelte/Next/jQuery compatibility findings |
| [`architecture.md`](architecture.md) | concurrency, process model, memory — decisions *and their reasoning* |
| [`conformance-and-oracles.md`](conformance-and-oracles.md) | how we MEASURE: WPT, the Chromium oracle, falsification |
| [`interaction-surface.md`](interaction-surface.md) | the interaction/automation surface (agent-native mission) |
