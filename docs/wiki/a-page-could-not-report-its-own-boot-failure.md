# A page could not report its own boot failure

*Tick 1480. One rule, four implementations, and the only silent one covered `window`.*

## The rule, and where it lived

*"A script on this page died and the page is now less than it should be."* That is one fact. This
engine wrote it to four different places, and no caller read any of them:

| path | where it went | who read it |
|---|---|---|
| a top-level classic `<script>` throw | `tracing::warn!("a page <script> threw")` | stderr |
| a `type=module` evaluation failure | `tracing::warn!("a page module failed")` | stderr |
| any DEFERRED throw — `setTimeout`, a microtask, a `MutationObserver`, an element listener, an `on*` handler | `globalThis.__errors`, a JS array | exactly one caller, `manuk-wpt diag` |
| **any `window`-level handler throw** — `window.addEventListener(…)` and `window.onX` | **nowhere at all** | **nobody** |

The fourth row is the finding. `__fireWindowEvent` invoked handlers inside a bare `catch (e) {}`,
twice. A throw from a `load`, `resize`, `popstate`, `message`, `hashchange` or
`unhandledrejection` handler was discarded whole — no `window.onerror`, no `error` event, no
`__errors` entry, no log line, nothing.

## Why it matters twice over

**As a capability.** HTML §8.1.7.1 says an uncaught exception in a listener is *reported*: fire
`error` at the global, run `onerror`. Every error-reporting SDK on the web (Sentry, Bugsnag,
Rollbar) and every app's own fallback UI installs exactly `window.onerror`, and for this entire
class of throw they saw a page that was working fine. `window` is also where a page's boot code
listens (`window.addEventListener('load', …)`) and where its cross-origin plumbing lives
(`message`), so the silent class was not a corner — it was the middle.

**As a measurement.** The Phase-0 exit metric is capped by SCORABILITY, not by placement: a site
whose boot bundle throws renders a shell and contributes zero however good layout gets. The observer's
P0 order is to *histogram the first-failure cause*. The instrument could report **that** a site
rendered a shell and never **why**, because the answer was in stderr and a JS array.

## What was built

`Page::boot_errors()` — **one** list, populated by all four paths, reset per document.

```text
  engine/js/dom_bindings.rs   SCRIPT_ERRORS thread-local + record_script_error + ScriptError
                              __hostScriptError native binding (the way OUT of the JS world)
                              clear_script_errors() at the top of PageContext::load
  engine/js/event_loop.rs     __reportError calls __hostScriptError — the deferred half
  engine/page/lib.rs          Page.boot_errors, snapshotted right after the document's own
                              load_document, merged again after `load` fires
  tests/wpt/fidelity.rs       boot_class(msg) — the HISTOGRAM KEY
  tests/wpt/main.rs           a BOOT line per site, printed BEFORE the four refusals
```

### Three details the gates pin

⚠ **A WATCHDOG PREEMPTION IS NOT A PAGE ERROR.** `run_one_script` already separates *"the page
threw"* from *"we cut it off at the deadline"*; only the first is recorded. Booking one as the other
would make every slow site read as a broken one.

⚠ **SNAPSHOT WHERE THE DOCUMENT'S OWN PASS ENDS, NOT AT THE STRUCT LITERAL.** The harvest is
thread-local and reset per `load_document`; the `<iframe>` pass runs `load_document` again for every
frame on the same thread. Reading it three hundred lines lower reports the LAST FRAME's errors as
this document's — the exact shape of bug this list exists to make visible, reintroduced by the
instrument that reports it.

⚠ **THE KEY IS THE SYMBOL, NOT THE EXPRESSION.** `boot_class` collapses
`"TypeError: n.matchMedia is not a function"` to `missing-member:matchMedia`. Minified production
code mints a different receiver on every site, so keying on the whole expression gives one bucket per
site — a list wearing a histogram's clothes. Six shapes are recognised
(`missing-global`, `missing-member`, `missing-ctor`, `undefined-deref`, `syntax-error`); anything
else becomes `other:` **carrying its own text**, because a bucket that swallows what it cannot parse
reports full coverage of nothing.

## The harvester's first act was to name a hole in what it was harvesting

The gate fixture threw three times — once from a top-level `<script>`, once from a module, once from
a `window.addEventListener('load')` handler. Two reached the harvest. That is how the fourth row
above was found: not by reading `__fireWindowEvent`, but by *consuming* the thing it feeds.

*Instruments are validated by consumption, not by inspection* (t1419), for the fifth time.

## Arbitrated against Chrome, and two more divergences fell out

The fixture: a throwing `load` listener, a `window.onerror`, an `error` listener, and a second
`load` listener after the throwing one.

```text
  Chrome    onerror:Uncaught TypeError: winload-th | errevt:… | second-listener-ran
  before    (nothing — the throw was swallowed)
  after 1   errevt:winload-threw | onerror:winload-threw | onerror:[object Object] | second-…
  after 2   errevt:winload-threw | onerror:winload-threw | second-listener-ran
```

`after 1` exposed a **pre-existing** double fire. `__reportError` called `globalThis.onerror`
directly *and* dispatched an `error` event, whose dispatcher reads `g['on' + type]` and calls it
again — with the EVENT rather than the unpacked argument list. `window.onerror` is an
`ErrorEventHandler` (HTML §8.1.7.2.1): `(message, source, lineno, colno, error)`, invoked exactly
once, BY the dispatch. Two invocations means every SDK on the page double-counts and half its reports
carry `[object Object]` as the message.

The fix puts the unpacking in the dispatcher — the one place that already had to know about
`on<type>` — and demotes the direct call to a fallback for a realm with no `dispatchEvent`.

### And routing window throws back here closes a loop

Reporting an exception dispatches an `error` event. That handler can throw. As of this tick that
throw is routed back to `__reportError` — so a throwing `window.onerror` would report its own throw,
forever. HTML has exactly one flag for exactly this: *"If the global object is in error reporting
mode, abort these steps."* Set on entry, cleared in `finally`, so SEQUENTIAL reports are unaffected
and only a NESTED one is dropped. Chrome-measured on a throwing `onerror`: `onerror-calls=1`. Ours,
after: `onerror-calls=1`.

This is not defence in depth against a hypothetical. Without the flag the gate's fixture is an
infinite recursion, and mutation 8 shows it.

## Named residue

⚠ **ORDER.** Chrome runs the `onerror` PROPERTY handler before the `addEventListener('error')` one,
because an event-handler attribute is a listener registered at the position it was assigned.
`__fireWindowEvent` runs its listener list first and the property handler last, for **every** window
event. Real, separate, and fixing it means giving the two lists one order.

⚠ **THE `"Uncaught "` PREFIX IS CHROME'S, NOT THE SPEC'S.** Firefox reports the bare message, which
is what we do. The gate asserts the message *contains* the thrown text and nothing about its framing
— pinning Chrome's wording would assert a thing the standard does not say.

⚠ **A MODULE LINK FAILURE ARRIVES WITH NO EXCEPTION OBJECT.** An unresolvable import specifier is
reported inside `module_resolve_hook` and reaches `run_one_script` as a bare `false`, so
`pending_exception` answers `"(no exception object)"` — which in a histogram reads as a defect in the
reporter rather than a fact about the page. The message now names the *stage*; carrying the
*specifier* out to the call site is the next step and is not done.

⚠ **`run_scripts` IS A SECOND COPY OF THE SCRIPT LOOP.** `dom_bindings::run_scripts` and
`PageContext::load` → `run_one_script` both run a document's scripts and both had their own copy of
the two `tracing::warn!` lines. Both now record. Mutating one of them was an INERT mutation against
the page path, which is how the duplication surfaced.

## The gates

`g_a_page_reports_its_own_boot_failure` — all three script paths land in one ordered list; the page
keeps running; `boot_errors()[0]` is causal; **and a clean page reports zero** (the vacuity arm,
loaded second on the same thread, which is also the per-document-reset arm).

`g_window_handler_throws_are_reported` — Chrome-byte-identical: `calls=1 typ=string evt=1 after=ran
msg=has-text`.

Eight named mutations, eight red.

## The measurement it was built for (P0, observer mandate 2026-09-10)

40 sites, the head of `docs/bench/corpus-crux-trend.txt`, one `manuk-wpt fidelity` pass.

```text
  documents with a BOOT verdict     39
    boot CLEAN                      28   (72%)
    >=1 uncaught error              11   (28%)

  FIRST-FAILURE CLASS
    2   undefined-deref:hasAttribute                       ikea.com, otomoto.pl
    2   other:WebAssembly-Promise-APIs-not-supported…      (JSPI)
    1   undefined-deref:undefined                          paypal.com
    1   undefined-deref:marginTop                          mangago.me
    1   syntax-error                                       repubblica.it
    1   other:no-exception-object                          (see residue)
    1   other:Minified-React-error--446…                   (hydration mismatch)
    1   missing-member:d
    1   missing-global:gsuite                              workspace.google.com
```

### ⭐ The top row is ONE THIRD-PARTY BUNDLE, at a byte-identical offset

```text
  OneTrustStub</S.prototype.setOTDataLayer@https://www.ikea.com/  inline#156:1:12608
  OneTrustStub</S.prototype.setOTDataLayer@https://www.otomoto.pl/ inline#66:1:12608
                                                                          ^^^^^^^
  TypeError: can't access property "hasAttribute", p.stubScriptElement is null
```

Two unrelated sites, the same minified byte offset: this is the **OneTrust consent SDK stub**, which
is on a very large fraction of the GDPR-facing web. It dies at the top level of a classic script, so
the consent gate never resolves and the page stays behind it. `otomoto.pl` is already named in
`dom_bindings.rs` as a whole-page wipe from a *different* cause (`toString` branding, t862) — this is
its second.

**Hypothesis for the next tick, not a conclusion:** `fetch_external_scripts` inlines a
`<script src>`'s source into the element and **drops `src`**, and `fire_external_script_load`'s own
doc comment already says *"after which nothing in the pipeline remembers the element was ever
external."* A stub that finds its own tag with `document.querySelector('script[src*="otSDKStub"]')`
would get `null` from a DOM where no script has a `src`. Probe before patching.

## ⚠ The instrument's own first reading was contaminated, and the second is the one above

The first 40-site run reported `marktplaats.nl`'s `TypeError: Invalid URL` as the boot failure of
`stdn.iau.ir` and `awlyaa.education.dz` — **neither of which runs any script.** `PageContext::load`
cleared the harvest and was the only place that did, but `manuk-page` declines to build a context at
all for a script-free document, so those pages inherited the previous one's list. 15 of 33 became 11
of 39 once fixed.

Isomorphic to the frame case the snapshot site was already moved for, one tick earlier, in the same
file. Same rule, second entrance — and **the gate could not see it, because both its fixtures had
scripts.** The script-free third document is now the arm; a clean *scripted* page in that slot clears
the harvest on its own way through and leaves nothing to inherit, so the ORDER is the arm.

*An instrument you repair invalidates its own earlier readings* (t1242). Both runs are on the same
binary in every other respect; only the second is quoted.

## ⚠ And the histogram key had a space in it

`other:TypeError: Invalid URL×3` arrived at every whitespace-splitting consumer as four buckets.
The FIRST-failure tally was bracketed and therefore fine; the CLASSES tally was nonsense, and it read
as nonsense rather than failing. Keys are now one token, and the `other:` bucket strips the
`TypeError: `/`Error: ` prefix so one sentence thrown as two error types lands in one bucket.
