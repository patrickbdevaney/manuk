# W6.J1.b.a.a.a: does the TransitionEnd gap follow from the value-parse gap?

chrome_ver=145.0.7632.116 (MANUK_CHROME_PIN). Measurement only; no engine/ or tests/wpt change. Binary: `target/release/manuk-wpt`.
Probes: `raw/J1b-drain-cause/probe-001.html` (synchronous, `getAnimations().currentTime=1000` as the WPT helper does) and
`probe-002.html` (real time, sampled at 1 s and 2.6 s, 2 s linear transition). Run as `manuk-wpt wpt css/zzprobe --show-failures`
(probes were temporarily placed in `~/wpt/css/zzprobe/`, removed afterwards). Output: `raw/J1b-drain-cause/probe.out`.
Values are printed through a failing `assert_equals` so `--show-failures` shows them.

## Result (background-color rgb -> rgb/rgba, and padding-left 10px -> 50px)

| case | from | right after class change | computed at t=1 s (mid) | at end | transitionend | document.getAnimations() |
|---|---|---|---|---|---|---|
| background-color (rgba target, sync probe) | rgb(255, 0, 0) | rgba(0, 0, 255, 0.5) | rgba(0, 0, 255, 0.5) | same | n/a | 0 |
| background-color (real time) | rgb(255, 0, 0) | rgb(0, 0, 255) | **rgb(0, 0, 255)** | rgb(0, 0, 255) | **none** | 0 |
| padding-left (length, real time) | 10px | 50px | **50px** | 50px | **none** | 0 |
| opacity (real time) | 1 | 0.2 | 0.2 | 0.2 | none | 0 |
| width (sync) | 100px | 300px | 300px | 300px | n/a | 0 |

The computed value is already the target at t=1 s: **no CSS transition starts** (the target value is applied on the
first style read). That is true while the value reads back parseable and non-empty, i.e. with the value-parse gap not in
play in these probes.

## Mutant (a case where the transition demonstrably starts must read as not-value-parse-caused / distinguishable)

Control in `probe-002.html`: `element.animate([{opacity:1},{opacity:0.2}],{duration:2000})` with `currentTime=1000` reads
mid opacity = **0.6** (an in-between value, so the probe can tell a running animation from a jump). Note
`document.getAnimations().length` is 0 even there, so the WPT helper's `document.getAnimations()` loop also sees nothing
for CSS transitions *and* for script animations.

## Verdict

Falsifier met: with the computed value parsing fine (probe values are non-empty and correct), the events outcome is
still "no transitionend", because no transition is created. The two gaps are **independent**:
1. CSS transitions are not started by style change in this engine (no interpolation over time, no transitionend).
2. Separately, the WPT file reads `""` from `getPropertyValue` (the "got disallowed value \"\"" message), which a probe
   with plain selectors does not reproduce; cause unmeasured (helper's vendor-prefix / injected `setStyle` path).
Fixing (2) alone cannot make the 280 `events` subtests pass. Caveat: single-engine probe of two property kinds; I did
not fix (2) in isolation in the engine, the probe stands in for it.

## W6.J1.b.a.a.a.a: why no transition ever starts on a style change (root cause, by source inspection)

chrome_ver=145.0.7632.116 (MANUK_CHROME_PIN). No engine change in this row; evidence is greps over `engine/` plus the probe-002 numbers above.

Three missing pieces, each independently sufficient to produce "computed value jumps to target, no transitionend":

1. **The transition clock never moves.** `animation::set_time_ms` (engine/css/src/animation.rs:62) has zero callers; `time_ms()` is 0
   forever. `transition::sample` (engine/css/src/transition.rs) only interpolates when `elapsed = now - delay > 0`, i.e. only for a
   NEGATIVE `transition-delay` (the WPT interpolation-testcommon static trick, covered by g_transition_interpolation.rs). An ordinary
   `transition: all 2s linear` has delay 0 so `may_be_running()` is false and the sampler is never entered. That is why the probe reads the target at t=1 s.
2. **No running-transition state.** `PREV` holds only the last cascade's style, replaced each pass; there is no per-node record of
   (property, from, to, start time). A real-time transition needs `start` captured at the style change, which exists nowhere.
3. **No events, no re-cascade on time.** `grep transitionend|transitionrun|transitionstart engine/` finds only an event-name list
   (event_loop.rs:8338), no dispatcher. And `forced_reflow` (page/src/lib.rs:2552) short-circuits when `mutation_seq` is unchanged,
   so a `getComputedStyle` read at t=1 s with no DOM mutation would not re-cascade even if (1) and (2) existed.

So g_transition_interpolation.rs does NOT cover the probe's path (checked): it only exercises negative-delay transitions.
Fixing this is four pieces of work (clock+running table, reflow-on-time, events, end-to-end gate); they are filed as W6.J1.b.a.a.a.a.a-.d rather than landed here.
