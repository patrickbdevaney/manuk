# `location.href = "/x"` did nothing — four spellings, one missing capability, no error

**t1505.** The most common redirect idiom on the web could not be performed by this browser. A user
typed `house.udn.com`, the engine fetched 195 bytes whose entire content is
`window.location.href="/house/index"`, ran it, and painted the empty stub. Nothing threw.

## What the probe found, before any patch

A fixture that assigns through each spelling and encodes the result in an element id, read back with
`manuk-wpt boxes`:

```text
                                    BEFORE                                  CHROME
  location.href = "/dest1"          href reads back "/dest1", UNRESOLVED     navigates
  location.assign("/dest2")         href reads the absolute url              navigates
  window.location = "/dest3"        typeof location becomes "string"         navigates
```

The BOM shim built `window.location` as a **plain object**. `href` was a data property, so the
assignment wrote it and returned. `assign`/`replace` reached `__applyUrl`, which is the *single-page
app* path — it rewrites the URL object and never touches the network. And the bare assignment
replaced the whole Location object **with a string**, after which every later `location.pathname` on
the page read `undefined`.

⚠ **A fourth reading in that probe was an ARTEFACT, and checking it is why it is not in this page as
a finding.** `document.URL` came back `undefined` — but only because the row before it had just
destroyed `location`. Re-run in isolation it is correct. A probe that runs its cases in one document
shares state between them, and the cheapest way to tell a finding from a consequence is to run the
suspicious one alone.

## The fix, and the line it must not cross

`location` is now an **accessor on the global** (the only way `window.location = u` can be anything
but a clobber), `href` is an **accessor on the Location object**, and all four spellings route
through a new `__navigateTo`, which records a pending navigation for the host.

> ⚠⚠⚠ **`__applyUrl` CHANGES THE URL. `__navigateTo` GOES THERE. They are not the same thing.**

`history.pushState`, `history.replaceState` and the host's own `popstate` replay all call
`__applyUrl`, and **none of them may touch the network**. A fix that made the shared function report
a navigation would turn every SPA route change into a full re-fetch — a far worse regression than the
bug, and completely invisible in a test that only checks that redirects now work. That is why the
gate's `pushState`/`replaceState` rows exist, and mutation M4 (report from `__applyUrl`) turns them
both red.

⚠ **A fragment is not a navigation either.** `location.href = "#top"` is same-document in every
browser; reporting it would send the host back to the network for the document it is already
displaying — on a page that does it in a scroll handler, forever. Both that refusal and the
self-target refusal live in `Page::take_script_navigation` rather than in each host, where a new host
would have to rediscover them.

⚠ **THE HOST PERFORMS IT, NOT THE PAGE** — the same contract as `Page::meta_refresh`,
`take_scroll_requests` and `take_form_submits`.

## ⭐⭐⭐ It closed an asymmetry that ONE TICK EARLIER had been written down as a rule

t1504 taught the fidelity **oracle** to follow a script-redirect stub. The fidelity loop's own side
follows `<meta refresh>` and nothing else, under a comment that says, verbatim:

> *"BOTH SIDES MUST FOLLOW THE SAME REDIRECT (t1487) … an engine that stops at the stub is being
> diffed against a DIFFERENT DOCUMENT."*

So for one tick the oracle scored `house.udn.com/house/index` (441 elements) and our side scored the
195-byte stub (1), and the row moved from `shell-only-1` — *"the ORACLE failed"* — to
`thin-overlap-1`, whose own text blames the engine. **t1504's journal read that relabelling as the
engine's bug becoming visible. The attribution was right and the mechanism was wrong**: the engine
was not rendering the page badly, it was never arriving at the page, and the instrument was diffing
two different documents.

The honest reason it was one-sided is in the fix: our side *could not* have been symmetric, because
following a script redirect requires the engine to perform one, and it could not.

## The measurement

```text
                          t1503           t1504 (oracle only)   t1505 (both + capability)
  house.udn.com           shell-only-1    thin-overlap-1        SCORED
  venus.zeronline.cloud   probe-blocked   render-failed         SCORED
```

**Same-binary control, both sides built from source in this tick** — the WIP moved to `/tmp`, the
tracked files `git checkout --`'d back to HEAD, `manuk-wpt` rebuilt, measured, and the WIP restored.
Not a stored row from a previous tick diffed against a fresh number (t1407: that attributes someone
else's work):

```text
                          BEFORE (HEAD)                  AFTER (this tick)
  house.udn.com           UNMEAS  coverage  0.2%   441 missing      ok  coverage  81.7%   82 missing
  venus.zeronline.cloud   UNMEAS  coverage  3.8%    25 missing      ok  coverage 100.0%    0 missing
                          sites 2 · scored 0                       sites 2 · scored 2
                                                                   coverage 90.8% · shape 70.8%
```

Two rows that had never yielded a scored tree now do, and `venus.zeronline.cloud` renders **every
element the oracle does**. That is the mandate's binding constraint — the scorability ceiling — and
not a decimal on an already-scorable site.

Neither reaches `shape >= 0.75`, so the certificate does not move. **Scorability is upstream of
shape, and the exit conjunction cannot be worked in the other order**: a site that is not scored
contributes a zero to the shape bar that no amount of layout work can lift.

`venus.zeronline.cloud` needed **two** hops (`/` → `/administrator/` → `login.php`), both scripted,
which the hop bound handles for the same reason `<meta refresh>` has one.

## Where it is

* `engine/js/src/dom_bindings.rs` — the BOM shim: `__parseUrl` accessor `href`, `__navigateTo`,
  `location` as a global accessor, `document.location` setter; `PageContext::take_pending_nav`.
* `engine/js/src/lib.rs` — `take_script_navigation` (+ the JS-less stub).
* `engine/page/src/lib.rs` — `Page::take_script_navigation`, `strip_fragment`.
* `shell/src/gui.rs` — `follow_script_navigation`, sharing `redirect_hops` with the declarative
  follower because a chain that alternates the two spellings is ONE chain.
* `tests/wpt/src/main.rs` — the fidelity loop's own follower, closing the t1504 asymmetry.

Gate: `engine/page/tests/g_a_script_navigation_is_a_navigation.rs`.

```text
  M1  href a data property again                RED   hrefSet reports none
  M2  location a data property on the global    RED   bareSet reports none
  M3  assign/replace back on __applyUrl         RED   assign + replace report none
  M4  report the navigation from __applyUrl     RED   pushState/replaceState start navigating
  M5  drop the fragment refusal                 RED   hashOnly navigates
  M6  drop the SELF-TARGET refusal              GREEN <- INERT GUARD, deleted
  M7  report the raw string, unresolved         RED   every row loses its origin
  clean                                         GREEN
```

## ⚠ M6 — the refusal had two clauses and one of them could never be checked

The guard read:

```rust
if next == self.final_url || strip_fragment(&next) == strip_fragment(&self.final_url) {
```

**Equal strings have equal fragment-stripped prefixes.** The second comparison had always answered
every case the first one did, so the first clause could not be falsified by any input — deleting it
left the gate green. It is the same shape as t1504's `==`-refusal clause (`string_literal_at` already
demanded a quote, so a by-name refusal of `==`/`=>` was unreachable) and t1403's original: **a clause
whose precondition is implied by the clause beside it is not defence in depth, it is a claim nobody
can check.** Fourth instance in this arc, second in two ticks. Deleted, and the coupling is now
written where the surviving clause is.

The host-side twin in `follow_script_navigation` (`if url == self.url`) is *not* inert: the shell's
`self.url` and the page's `final_url` differ across a redirect chain, so it is a second question, not
a second spelling of the same one.

⚠ The gate is not named in `scripts/verify.sh`, which lists ~20 of `manuk-page`'s ~500 gate files
individually. `scripts/gate-sweep.sh` runs the whole package and will pick it up. `scripts/` is
observer-owned; this is the standing "gates the wall does not run" item, not a new one.
