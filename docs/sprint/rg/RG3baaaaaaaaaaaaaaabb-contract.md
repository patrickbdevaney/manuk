# RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a.a.b.b — one contract for content scrolled out of a scroll container

chrome_ver=145.0.7632.116 (all rows below).

## The asymmetry (from RG3baaaaaaaaaaaaaaab-clock.md)
On a 400px `overflow:auto` listbox of 48 x 36px options scrolled to scrollTop 504, the Chrome backend listed 35
options (opt 13..47) and manuk 48 (opt 0..47). Cause: the Chrome backend's shield judged "offscreen" on Chrome's
*scrolled* layout bounds (`y + h <= 0` in page coordinates), so options scrolled above the list's view sat at negative
y and were withheld as hidden text — but options scrolled below the view (positive y) were kept. The in-process shield
judges its layout fragments, which are positioned with every scroll container unscrolled, so it kept all 48.

## The contract
Content a scroll container has scrolled out of its view — above or below — **stays listed and actionable** on both
backends. It is not hidden text: a human (and a click, which scrolls a wholly-clipped target in — `A11yNode` resolve,
RG.S1.a) reaches it by scrolling the container. The `offscreen` taint is for content no scroll can reveal (negative
page coordinates, `left:-9999px`), so it is judged on the unscrolled position. This is what manuk already did; the
Chrome backend now adds each clipping ancestor's own scroll offset (`DOMSnapshot` `scrollRects`, html/body excluded as
in `clip_of`) back before the offscreen test. Withholding only what is above the view (Chrome's old behaviour) was not a
contract at all — it was symmetric neither in direction nor with the in-process backend.

## Evidence (`bench/agent-tasks/rg3baaaaaaaaaaaaaaabb_contract.py`, full output in the .txt)
| mode | manuk | chrome (fix) | chrome (`MANUK_CDP_MUTATE=scrolled-offscreen`) |
|---|---|---|---|
| none (scrollTop 0) | 48, opt 0..47 | 48, opt 0..47 | 48, opt 0..47 |
| scrollTop 504 | 48, opt 0..47 | 48, opt 0..47 | 35, opt 13..47 |
| focus() opt 26 (scrollTop 754) | 48, opt 0..47 | 48, opt 0..47 | 28, opt 20..47 |

Gate exit 0 with the change, 1 with it reverted, 1 with `--mutate drop-above`.
opendining-4 after step #7 (`rg3baaaaaaaaaaaaaaab_clock.py`): both backends list 48 options, first '12:00 AM'
(before: manuk '12:00 AM', chrome '7:00 AM').

The focus() scrollTop of 754 on manuk comes from RG...b.a (focus scrolls into view), landed in the same tick.
