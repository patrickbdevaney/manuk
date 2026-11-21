# W6.R1.a.a.a — a 10 s UI auto-dismiss timer no longer fires before the agent's first snapshot

chrome_ver=145.0.7632.116 (MANUK_CHROME_PIN). Raw: `docs/sprint/w6/raw/R1aaa/`. JS: `bench/agent-tasks/r1aaa_alert.js`.

## Change

1. `engine/js/src/dom_bindings.rs`: the post-load virtual-clock horizon (`__VCLOCK_HORIZON_MS`) is 60 s only on a page
   that loaded testharness.js (`add_completion_callback` defined; its harness timeout is 10 s). Every other page gets
   6 s, the Chrome oracle's own `--virtual-time-budget`. Each agent `settle()` still opens the clock a further 1.8 s,
   so a 10 s timer fires after the agent's 3rd settle, not during load.
2. `agent/src/bin/dom-eval.rs`: `wait` is now page time. The old loop called `settle()` every 100 ms for `wait` real
   seconds; each settle adds 1.8 s virtual, so it ran ~18x real time by a host-speed-dependent count. Probe (local
   page, `setTimeout(f, 10000)`, engine fix applied, old dom-eval): wait 1 s -> f fired at now=9997, budget=24000.
   New: ceil(wait / 1.8) settles. Same probe, new dom-eval: wait 0/1/3 -> not fired (budget 6000/7800/9600);
   wait 8 -> fired at 9997.

## Gate (dom-eval https://evals-udriver.vercel.app, wait 3, unmodified page, 3 runs)

| build | hasAlert | header y | alert h |
|---|---|---|---|
| fix (6 s horizon) | true 3/3 | 188.1 | 188.1 |
| mutant: horizon restored to 60 s | false 3/3 | 0 | - |
| fix engine, OLD dom-eval polling loop | false 3/3 | 0 | - |

Engine test `engine/page/tests/g_vclock_ui_horizon.rs`: a 3 s timer runs during load and a 10 s one does not
("early"); a testharness page still reaches its 10 s timer. Mutant (60 s): fails with left "dismissed", right "early".

Not fixed: alert height 188.1 vs Chrome 204 (follow-up W6.R1.a.a.a.a).

## W6.R1.a.a.a.a — the 16 px alert-height gap (188.1 vs 204)

chrome_ver=145.0.7632.116. Per-box dump: `bench/agent-tasks/r1aaaa_boxes.js` (manuk via dom-eval, Chrome via CDP, same page, wait ~4 s).

Per-box diff, `.MuiAlert-root` children (manuk | Chrome): root 188.1 | 204.4; message div 116.1 | 130.4 (5 lines);
`<br>` 17 | 22; line-height 20.02 px | 22.88 px. Cause: the rule is emotion-merged to
`font-size:.875rem; line-height:1.43; …; font-size:16px`. manuk's MinimalCascade (the agent build has no Stylo)
turned the unitless `1.43` into px at the declaration (1.43 x 14 = 20.02) and never re-derived it when the later
`font-size:16px` arrived (nor for a child's own font-size). Chrome applies the number to the final font-size
(1.43 x 16 = 22.88). Not font metrics, wrapping or padding.

Fix: `ComputedStyle.line_height_num` (inherited multiplier) re-applied in the `font-size` arm, the UA scale, and
inheritance (`engine/css/src/lib.rs`). Test `unitless_line_height_follows_the_final_font_size`; mutant (drop the
re-derive in the font-size arm) fails with 20.019999.

| build | hasAlert | alertH (3 runs) | Chrome (same run) |
|---|---|---|---|
| before | true | 188.1 | 204.4 |
| fix | true | 202.4, 202.4, 202.4 | 204.4 |

Residual 2.0 px (inside the gate's 2 px): the 24 px icon box is 22 px tall in manuk (svg display block in Chrome, inline-block in manuk) — not chased.
