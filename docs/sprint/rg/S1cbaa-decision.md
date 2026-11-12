# RG.S1.c.b.a.a: why the OpenDining carousel cards are absent from observe (chrome_ver=145.0.7632.116)

Measured with Playwright on the pinned CfT 145, 1280x800, 6 s settle (script: /tmp-style one-off, query below).
For each `a[aria-label^="View details for"]`, the nearest ancestor with an `aria-hidden` attribute:

| card | nearest aria-hidden ancestor | x |
|---|---|---|
| Fancy Lights Bistro (visible) | LI.react-multi-carousel-item aria-hidden=false | 1204 |
| Bar Central | LI.react-multi-carousel-item aria-hidden=**true** | 1501 |
| Ocean Breeze | LI.react-multi-carousel-item aria-hidden=**true** | 2986 |

## Finding
- The 15 clipped cards are marked `aria-hidden="true"` by the page (react-multi-carousel marks every slide outside the
  active window). Chrome's accessibility tree therefore omits them, as does Playwright's snapshot. manuk's observe omits
  them for the same reason, including with include_hidden=true (that flag only governs the paint-based text shield).
  The "observe lists 5" result is Chrome-faithful, not an engine bug.
- The default text extract withholds them through the shield's offscreen rule (f.x >= viewport width). Text a human cannot
  see without sliding is exactly what an offscreen hidden-instruction looks like (left:-9999px / right of canvas).
  Relaxing that rule for "text inside an overflow:hidden box" is not separable from an attacker wrapping an instruction in
  an overflow:hidden div, so it is not done here; no injection-slice run could justify it without a new external case.

## Decision
No engine change. The node gate (observe lists >=20 names) asks observe to contradict the page's own aria-hidden and
Chrome's AX tree, so it is not met. Follow-up RG.S1.c.b.a.a.a adds a notice to observe instead: the agent is told that
a clipped carousel holds aria-hidden slides and how to reach them (Next slide, or extract include_hidden=true).
