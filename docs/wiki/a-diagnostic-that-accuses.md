# A diagnostic that cannot report a non-zero is worse than none — it accuses

> Landed t1415. Gate: `the_diagnostic_reads_the_scorer_s_own_payload`
> (`tests/wpt/tests/g_diag_reports_what_the_scorer_reports.rs`), 4 arms, red under 4 mutations.

`manuk-wpt diag <file>` exists to answer one question: *a test file produced nothing — why?* Its
headline field counted the file's tests itself:

```js
  testsCreated: (globalThis.tests && globalThis.tests.length) || 0
```

testharness.js keeps `tests` **inside its own closure** and `expose()`s only its public API — which is
why the field beside it, `harness: (typeof add_completion_callback === 'function')`, reads `true`
while `globalThis.tests` is `undefined`. So the expression is `undefined || 0` — **zero, forever, for
every file.**

```text
  css/cssom-view/elementFromPoint.html   real runner 8/11 = 72.7%     diag  testsCreated: 0
```

## ⚠⚠ The `tests.tests` spelling still answers 0

That was the first fix attempted, and it was *measured before it was believed*. `globalThis.tests` is
undefined, so nothing rooted there can work. **The field was never one typo away from working**, and
an expression-level repair could never have discovered that — only asking the page what
`globalThis.tests` actually was did.

## ⭐⭐⭐ The runner already had the answer

`harness.rs`'s `REPORT_JS` registers an `add_completion_callback` and emits
`<script id="__wpt_results__">` with every test's name and status — the payload **the score is
computed from**. `diag` had invented a second way to count the same thing and got a worse one: *one
rule, two implementations*, in the diagnostic itself.

```text
  {"errors":[],"loadFired":true,"hasIframe":true,"frameDoc":"OK","frameNodes":4,
   "harness":true,"results":{"harness":"OK","tests":11}}
```

The diagnostic and the scorer now cannot disagree, because there is one payload.

⭐ **`results: null` is a statement about the TOOL.** *"I did not look"* must not be spelled the same
way as *"the file created nothing"* — that spelling is the whole defect.

⚠ `onloadCalls` was **deleted rather than fixed**: `globalThis.__onCalls` has no writer anywhere in
the repository, so it was a second permanent zero. **A field nothing populates is not a measurement.**

## What it cost

This field steered three ticks of one session. It sent t1412 hunting `<body onload>` — which found a
real defect, by luck — and very nearly sent t1414 hunting 1,546 phantom `cssom-view` bugs. A silent
instrument wastes a tick; an accusing one wastes several and looks like progress while it does.
