# RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a.a.c.b.a — why chrome exposes 123 buttons and manuk 51 on gocalendar-2

chrome_ver=145.0.7632.116. Gate: `bench/agent-tasks/rg3baaaaaaaaaaaaaaaacba_buttons.py` (raw: `RG3baaaaaaaaaaaaaaaacba-buttons.txt`).
Measured on the interactive observation that ordinal step #6 of the gocalendar-2 replay resolved against, on each backend.
Measure only: no engine or match-rule change.

| backend | button nodes | nameless | distinct names | date cells `"01"`-`"31"` | `Event color indicator …` | other |
|---|---|---|---|---|---|---|
| manuk | 51 | 1 | 50 | 22 | 5 | 23 |
| chrome | 123 | 1 | 96 | 57 | 20 | 45 |
| diff (chrome − manuk) | +72 | 0 | | +35 | +15 | +22 |

Name sets: 53 names are on chrome only and 7 on manuk only. The 53 chrome-only names are mostly calendar date cells
(`01`–`09`, `10`–`24`, `30` appears twice more on chrome) and event labels whose chrome name repeats the text
(`Client Call 12pm - 1pm Client Call 12pm - 1pm`, where manuk has `Event color indicator Client Call 12pm - 1pm`).
So part of the set difference is a naming-format difference (chrome's accessible name duplicates the content and
the observe output truncates it with `…`), not a different set of elements. The gate reads the count gap as explained
because the name sets differ.

Gate: GREEN (exit 0) on the real run: counts differ, name sets differ (53 chrome-only, 7 manuk-only). Mutation
`--mutate samenames` (chrome's name set replaced by manuk's, counts unchanged) exits 1 with
`RED: button counts differ and the name sets are identical`.
