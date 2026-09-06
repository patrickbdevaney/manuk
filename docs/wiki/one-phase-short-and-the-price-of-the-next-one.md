# One phase short, and the price of the next one

t1461 moved `AgentBrowser` from `Page::load` to `Page::load_async` and the Track B bar went from
76.6% F1 to 94.8%. This tick asked what the *next* phase is worth. The answer is: it fixes the
largest remaining defect and costs more than it buys.

## Where the missing names came from

The biggest single item left in Wikipedia's excess was **42 `link "↑"` against Chrome's 42
`link "Jump up"`** — matched exactly, so obviously the same nodes named differently. The static HTML
has neither string; Chrome's post-JS DOM has `aria-label="Jump up" title="Jump up"` on every one.
MediaWiki's cite enhancement adds them.

Asking the page itself, rather than reverse-engineering MediaWiki, took one eval:

```text
  mw=object | jq=undefined | RLQ=object | cite=loaded | col=loaded | backlinks=83 | label=NONE
  jqstate=loading | scripts=6 | srcscripts=1 | SRC[…&modules=jquery&skin=vector-2022…]
```

**`jqstate=loading`.** MediaWiki injected the jQuery `<script src>`; it never executed; every
jQuery-dependent enhancement therefore did nothing while `mw.loader` still reported its modules
`loaded`.

⚠ And the reason is *not* that injected scripts don't work — a minimal fixture run through
`load_async` **and `finish_loading`** produces `jq=function sink=[start ONLOAD]`, Chrome-exact. The
drain works. `g_script_load_event`'s own comment says where it lives: *"the dynamic-script phase
lives in `finish_loading`, not in `load_async` — a gate that stops at `load_async` never reaches the
code under test and would pass by not looking."* **The agent stopped one phase short of it.**

⚠⚠ Two of this tick's probes read the world on a `setTimeout` and reported the drain as broken. The
timer fires before the drain settles, so the fixture measured the *pre-drain* page and called it a
missing capability. **When a phase is what you are testing, do not observe from inside the page's
own timers.**

## What the next phase costs

```text
                    precision   recall      F1     wikipedia nodes
  without           93.2%       96.4%    94.8%     865   (Chrome: 779)
  with              70.9%       97.9%    82.2%    2191
```

Recall improves — the 42 names arrive, and `jumpup=48 arrow=0`. Precision collapses, and the excess
names itself: **`486 listitem ""`**, the same signature t1461 removed by getting the collapse to run
at all. `finish_loading` restores the names *and undoes the collapse*.

F1 is the steering metric (see [[recall-is-not-node-match]]) and it falls, so **the ratchet
refuses**. The line is written into `AgentBrowser::load_url` as a comment carrying this table, with
the blocker named: once the collapse survives the module's own JavaScript, the call goes in.

## The shape worth keeping

A phase boundary is not a detail of the loader — it is **which browser the agent is**. Three ticks
have now found capability differences that were really phase differences:

| tick | boundary | what was missing |
|---|---|---|
| t1461 | `load` → `load_async` | subresources, the lifecycle, page scripts |
| t1467 | `load_async` → `finish_loading` | the dynamic-script drain, hence every module loader |

Each looked like a missing feature and was a missing *call*. And each one has to be priced, not
assumed: the second is a regression at today's fidelity even though it is strictly more of what a
browser does.

See also [[the-agents-browser-had-no-javascript]].
