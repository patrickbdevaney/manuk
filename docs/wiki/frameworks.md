# FRAMEWORKS — React, Svelte, Next, jQuery: what actually breaks

## Frameworks are debugged by RUNNING them, not by cloning their repos (settled)

The bug is never in React. It is in the platform API React assumes. **Cloning framework source teaches
nothing that running the framework against the engine does not teach faster and more honestly.**

## A missing constructor is a THROWN EXCEPTION, and its blast radius is whatever was rendering

This is the most expensive shape of gap in the project and it keeps recurring:

- `canvas.getContext` was used by **3%** of sites and **broke 100% of them** — `ctx.fillRect(…)` on the
  next line is a `TypeError`, and a charting library that boots on load takes the whole bundle with it.
- **`WebSocket` was missing and took an entire news front page with it.** aljazeera.com's 2,591
  server-rendered elements became **141**: a live-blog client constructed one at boot, React's render
  threw, its error boundary showed a skeleton, and the article was gone.

Fixing that one revealed `Blob`. Fixing `Blob` revealed `FileList`. **Each was a different library's
first line.** A page does not get to run its fallback path **if the check for the fallback throws.**

> **The rule: construct successfully, and answer honestly.** A blank canvas, an unopened socket, an
> empty `Blob` are all survivable — every library on the web is written to survive them, because real
> browsers produce exactly those behind captive portals, in private windows, and with permissions
> denied. **A `ReferenceError` is survivable by nothing.**

## USAGE IS NOT DAMAGE — rank by what happens to the user

`srcset` is used by **34%** of sites and damages **~1%** (the fallback `src` works). `canvas` was used
by **3%** and broke **100%** of them (it threw). *Ranking a backlog by usage frequency is ranking it by
the wrong number.*

## Server-rendered HTML renders with NO JavaScript at all

Hydration only attaches handlers — so Next.js/SvelteKit SSR pages are already *readable* before any JS
runs. **What breaks is interactivity and client-side routing**, not the content. This re-ordered the
whole roadmap: the History API turned out to be pure host state with zero SpiderMonkey dependency.

## `ownerDocument` must be read from the ROOTED global

Our `DOC_REFLECTOR` was an unrooted `*mut JSObject` — a use-after-GC. React got one of its own
MutationRecords back. Read `globalThis.document` instead; it is rooted by construction.

## jQuery is on ~74% of pages

Which makes the **jQuery-core surface** the empirically-justified first tranche of any binding work:
`querySelector`/`getElementById`, `createElement`/`appendChild`/`textContent`/`innerHTML`,
`addEventListener`, XHR/`fetch`, `classList`/`style`.

**jQuery survives a missing `DOMContentLoaded` by checking `document.readyState`** — which is precisely
why the missing document lifecycle went unnoticed for 40+ ticks: *it worked often enough to look fine.*
