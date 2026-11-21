# W6.R1.a.a — why manuk never mounts udriver's MuiAlert banner

chrome_ver=145.0.7632.116 (MANUK_CHROME_PIN). Raw: `docs/sprint/w6/raw/R1aa/`. Tool: `bench/agent-tasks/r1aa_proxy.py`
(localhost proxy of evals-udriver.vercel.app, optional rewrite of one constant).

## The JS condition manuk fails

The page is client-rendered (SSR body is empty). In the `index-*.js` page chunk the banner component is:

    [e,i]=useState(!0); useEffect(()=>{let t=setTimeout(()=>{i(!1)},1e4); ...},[]) ... e&&jsx(Alert{severity:"info",...})

i.e. the Alert is **auto-dismissed by a 10 s `setTimeout`**. Other gates (`removePopup`, from localStorage `config`,
defaults false; `useMediaQuery(down("sm"))` false at 1024 px) pass in manuk. Chrome is sampled at ~4 s of real time, so
the timer has not fired and the Alert is there. manuk runs on a **virtual clock**: after `load`, timers up to a 60 s horizon
(`__VCLOCK_HORIZON_MS`, `engine/js/src/dom_bindings.rs`) fire immediately. Probe: a page with `setTimeout(f,10000)` run via
`dom-eval ... 2` logs "10s fired at 9991" (virtual ms) within a 2 s wait. So the 10 s dismissal fires before any
capture, `e` becomes false, and the wrapper `<div class="MuiBox-root css-0">` is left empty. Same state at wait 1, 3, 8 s.

## Evidence (manuk dom-eval, wait 3 s, via the proxy)

| arm | hasAlert | header y | alert h |
|---|---|---|---|
| proxy, unmodified | false | 0 | - |
| proxy, delay `1e4` -> `1e9` (beyond the horizon) | **true** | 188.1 | 188.1 |

Mutation: reverting the rewrite restores hasAlert false (the "none" arm above). Chrome: alert 204 px, header y 204.
The fixed arm is not within 2 px: 188.1 vs 204 (16 px shorter alert, presumably text wrapping/font metrics; not
investigated). So the gate's first branch is NOT met; this doc is the second branch (names the condition).

## AGF rerun (r1_agf capture, 3 runs, udriver only)

Same as W6.R1 baseline: scored 12, ok 0, geom 10, missing 2, AGF 0.0 in 3/3 runs (`raw/R1aa/agf-udriver.txt`).
Unchanged, as expected: no engine change was made.

## Not fixed (design call)

A fix means making the virtual clock not outrun a real-time observer (e.g. cap virtual advance during agent capture to
wall-clock, or horizon relative to real elapsed time). That touches the determinism/WPT contract (testharness needs a
10 s timer to fire), so it is a follow-up, W6.R1.a.a.a, not a drive-by edit.
