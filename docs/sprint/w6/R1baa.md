# W6.R1.b.a.a — what gates opendining's client render at 50-61 s of manuk page time

chrome_ver=145.0.7632.116 (Chrome side not re-measured here; Chrome's 109 targets are from R1b). Raw: `docs/sprint/w6/raw/R1baa/`.
Tool: `bench/agent-tasks/r1baa_proxy.py` (localhost proxy of evals-opendining.vercel.app that injects a setTimeout/setInterval
logger and a capture-phase `load`/`error` logger first in `<head>`; mode `log+short:60000` rewrites every 60000 ms timeout to 0).

## The gate

A **60 000 ms `setTimeout` armed by React DOM's "suspensey commit" path** (`function(){if(e.stylesheets&&cL(e,e.stylesheets),
e.unsuspend){var t=e.unsuspend; ...`). It is armed at ~300 ms of page time and fires at 59 997 ms (log: `to|60000|303|59997`);
at 59 997 ms the pending commit is released and the 10 s MuiAlert-style timers etc. run (`to|10000|59997|69997`).

Why it is pending: React inserts `<link rel=stylesheet data-precedence=next href=/_next/static/css/09a6c8321e7bfbc2.css>` and
holds the whole commit until that link's `load` (or `error`) event. In manuk that link has **no `.sheet` and no `load` event
ever fires** (a minimal page that appends a `<link rel=stylesheet>` and listens with `onload`, `addEventListener('error')` and a
capture listener sees nothing; `engine/page/src/lib.rs` ~7554 dispatches load/error for dynamic `<script src>` only, not for
dynamic stylesheet links). The CSS itself is fetched fine (HTTP 200, 2403 bytes). On Chrome the load event arrives within the
settle, React releases the commit at once, and the 60 s timer never has to fire (not measured on Chrome here; inferred from Chrome
rendering within 4 s).

## Gate evidence (3 agf-probe runs per arm, `raw/R1baa/runs.txt`)

| arm | agf-probe targets (3 runs) | dom-eval wait 3 s: text chars / elements | 60000 ms timer |
|---|---|---|---|
| proxy, logger only | 0, 0, 0 | 0 / 86 | scheduled 309, never fired (page time 662) |
| mutation: 60000 -> 0 | 10, 10, 10 | 1381 / 260 | fired 315 |

Mutation (shortening the timer) turns 0 into 10 targets, the number the historical rendered captures had; reverting gives 0.

## Consequence

The 50-61 s gate is a symptom of a missing engine behaviour: **`load`/`error` events on dynamically inserted
`<link rel=stylesheet>`**. Every Next.js app-router page (React 19 stylesheet precedence) that adds a stylesheet client-side
waits the full 60 s. Follow-up W6.R1.b.a.a.a implements it.
