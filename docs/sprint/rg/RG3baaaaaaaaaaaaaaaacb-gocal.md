# RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a.a.c.b — gocalendar-2 ordinal #6 on both backends

chrome_ver=145.0.7632.116. Gate: `bench/agent-tasks/rg3baaaaaaaaaaaaaaaacb_gocal.py` (raw: `RG3baaaaaaaaaaaaaaaacb-gocal.txt`).

Transcript step: `act h76 click` (replay #6), recorded against `[h76] button [clickable] ↑offscreen`, the 47th (0-based) button of its observation, nameless.

| backend | classification | live line | buttons live | nameless `[clickable] ↑offscreen` buttons at ordinal |
|---|---|---|---|---|
| manuk | DIFFERENT | `[h269] button "Event color indicator Team Brainstorming Session 3pm - 5pm"` | 51 | 50 |
| chrome | DIFFERENT | `[h126] button "30"` | 123 | none |

Both are different elements from the one recorded. The ordinal is a position among same-role nodes, so it lands wherever the live list puts it.
- manuk: the live button list is almost the transcript's shape (51 buttons) but the nameless offscreen button sits at ordinal 50, three places after the ordinal-47 slot, which holds an event button. The live page has more event buttons than the recorded session did, which is a data/clock difference, not a resolution fault.
- chrome: its tree exposes 123 buttons (calendar date cells `"30"`, `"01"`, ...), and has no nameless offscreen button at all, so no ordinal can be the same element.

Neither is a manuk-specific regression; the miss is the ordinal rule applied to a differently-shaped tree. Match gate and MIN_CHROME not touched (chrome stays 11/13 of the stated rule).

Mutation: `--mutate unresolved` (chrome #6 dropped from the resolved set) exits 1 with `UNCLASSIFIED`; unmutated exits 0.
