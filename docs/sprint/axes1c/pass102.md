# AXES.1.c per-task pass attribution (chrome_ver=145.0.7632.116)

Sources: `bench-queue/out/w6f1ab-manuk.tsv`, `bench-queue/out/w6f1aa-playwright.tsv` (3 runs x 102 tasks each; the arms ran at different times).

- manuk per-run passes [47, 50, 45] (mean 47.33); playwright [51, 53, 50] (mean 51.33); mean diff -4.00.
- sum of per-task deltas = -12 -> /3 = -4.00.
- manuk-only failures (manuk passes <=1 of 3, playwright >=2 of 3): 7 tasks, net delta -16 of the -12 total (the other tasks net +4, i.e. manuk wins elsewhere).
- tasks with any delta: 26; the rest of the gap is single-run flips (not attributable to one task from 3 runs).

| task | manuk/3 | pw/3 | class |
|---|---|---|---|
| gocalendar-6 | 0 | 2 | declined-no-control |
| networkin-4 | 0 | 3 | declined-no-control |
| opendining-10 | 1 | 3 | partial-state-mismatch |
| opendining-8 | 1 | 3 | partial-state-mismatch |
| opendining-9 | 1 | 3 | answered-wrong |
| staynb-4 | 1 | 3 | stopped-before-payment |
| udriver-7 | 0 | 3 | answered-wrong |
