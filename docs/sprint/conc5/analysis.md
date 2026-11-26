## Success (mean [min-max] over surviving runs; infra = cells OOM-killed by the 8 GB cage; counted as success 0 in capacity)

| arm | N=1 | N=5 | N=10 | N=25 | N=50 | N=100 |
|---|---|---|---|---|---|---|
| engine-proc-full | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=2 (+1 infra) |
| engine-proc-lean | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | 0.99 [0.98-1.00] n=3 |
| engine-fork | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 |
| chrome-mcp | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | - (+3 infra) | - (+3 infra) | - (+3 infra) |
| pw-proc | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | - (+3 infra) | - (+3 infra) | - (+3 infra) |
| pw-shared | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | - (+3 infra) | - (+3 infra) |

## Peak PSS of process tree, MB (mean [min-max])

| arm | N=1 | N=5 | N=10 | N=25 | N=50 | N=100 |
|---|---|---|---|---|---|---|
| engine-proc-full | 108 [106-112] | 500 [450-594] | 938 [873-1057] | 2257 [2062-2590] | 4282 [3666-5012] | 7376 [7141-7611] |
| engine-proc-lean | 111 [110-112] | 452 [451-455] | 878 [874-880] | 2084 [2043-2111] | 3976 [3781-4112] | 7091 [5850-7900] |
| engine-fork | 121 [112-139] | 492 [446-583] | 952 [860-1133] | 2146 [2003-2400] | 4111 [3818-4528] | 7660 [7464-7915] |
| chrome-mcp | 644 [634-653] | 2403 [2385-2423] | 4470 [4436-4513] | - | - | - |
| pw-proc | 708 [699-714] | 2532 [2437-2602] | 4679 [4522-4779] | - | - | - |
| pw-shared | 706 [689-721] | 1645 [1615-1669] | 2707 [2633-2750] | 5590 [5130-5851] | - | - |

## Successful traces per minute (mean [min-max])

| arm | N=1 | N=5 | N=10 | N=25 | N=50 | N=100 |
|---|---|---|---|---|---|---|
| engine-proc-full | 100.7 [87.1-113.8] | 441.8 [396.9-502.2] | 670.4 [583.5-740.0] | 1028.5 [970.7-1091.1] | 1177.1 [1004.5-1329.0] | 1318.2 [1271.8-1364.6] |
| engine-proc-lean | 112.5 [108.7-115.5] | 412.1 [363.4-472.9] | 705.5 [670.4-767.3] | 965.3 [923.4-1003.6] | 1200.4 [1155.6-1240.8] | 1224.9 [1153.1-1278.7] |
| engine-fork | 94.6 [82.9-110.6] | 386.7 [360.8-420.6] | 702.1 [636.2-789.7] | 1074.4 [992.0-1161.8] | 1221.6 [1069.9-1377.2] | 1261.9 [1109.7-1375.7] |
| chrome-mcp | 15.3 [15.3-15.3] | 72.9 [72.5-73.7] | 135.3 [133.0-138.8] | - | - | - |
| pw-proc | 44.8 [40.1-47.4] | 202.0 [185.0-215.8] | 335.8 [321.2-352.3] | - | - | - |
| pw-shared | 45.7 [39.6-50.3] | 177.1 [156.9-189.0] | 293.0 [265.9-310.0] | 368.4 [346.9-410.8] | - | - |

## p95 ms per tool call (mean [min-max])

| arm | N=1 | N=5 | N=10 | N=25 | N=50 | N=100 |
|---|---|---|---|---|---|---|
| engine-proc-full | 517 [410-635] | 503 [480-536] | 1014 [913-1067] | 1422 [1268-1614] | 3142 [2626-3647] | 5500 [4540-6460] |
| engine-proc-lean | 463 [409-493] | 756 [735-780] | 874 [776-1013] | 1343 [1235-1458] | 2516 [2333-2733] | 5153 [3712-6058] |
| engine-fork | 588 [425-735] | 607 [542-698] | 810 [668-927] | 1356 [1218-1445] | 2774 [2347-3100] | 5865 [5485-6462] |
| chrome-mcp | 2563 [2535-2604] | 3231 [3189-3256] | 3283 [3238-3370] | - | - | - |
| pw-proc | 673 [612-774] | 1035 [917-1100] | 1162 [1127-1196] | - | - | - |
| pw-shared | 756 [588-1090] | 1105 [1081-1138] | 1236 [1164-1363] | 2717 [2300-2984] | - | - |

## CPU seconds (mean [min-max])

| arm | N=1 | N=5 | N=10 | N=25 | N=50 | N=100 |
|---|---|---|---|---|---|---|
| engine-proc-full | 3 [2-4] | 15 [14-17] | 36 [29-46] | 115 [109-125] | 259 [242-286] | 524 [471-577] |
| engine-proc-lean | 2 [2-2] | 14 [13-15] | 29 [27-30] | 106 [89-120] | 211 [200-219] | 458 [414-484] |
| engine-fork | 3 [3-4] | 16 [14-18] | 41 [34-47] | 116 [96-131] | 242 [227-250] | 494 [481-512] |
| chrome-mcp | 6 [6-6] | 36 [36-38] | 91 [81-97] | - | - | - |
| pw-proc | 8 [7-9] | 43 [38-49] | 95 [90-98] | - | - | - |
| pw-shared | 8 [6-9] | 35 [32-41] | 66 [62-71] | 189 [174-197] | - | - |

## PSS per successful trace, MB (mean [min-max])

| arm | N=1 | N=5 | N=10 | N=25 | N=50 | N=100 |
|---|---|---|---|---|---|---|
| engine-proc-full | 14 [13-14] | 13 [11-15] | 12 [11-13] | 11 [10-13] | 11 [9-13] | 9 [9-10] |
| engine-proc-lean | 14 [14-14] | 11 [11-11] | 11 [11-11] | 10 [10-11] | 10 [9-10] | 9 [7-10] |
| engine-fork | 15 [14-17] | 12 [11-15] | 12 [11-14] | 11 [10-12] | 10 [10-11] | 10 [9-10] |
| chrome-mcp | 80 [79-82] | 60 [60-61] | 56 [55-56] | - | - | - |
| pw-proc | 89 [87-89] | 63 [61-65] | 58 [57-60] | - | - | - |
| pw-shared | 88 [86-90] | 41 [40-42] | 34 [33-34] | 28 [26-29] | - | - |

## Agent capacity (largest ladder N with mean success within 5 points of the arm's N=1; ladder only, no bisection)

- engine-proc-full: capacity 100 (N=1 success 1.00)
- engine-proc-lean: capacity 100 (N=1 success 1.00)
- engine-fork: capacity 100 (N=1 success 1.00)
- chrome-mcp: capacity 10 (N=1 success 1.00)
- pw-proc: capacity 10 (N=1 success 1.00)
- pw-shared: capacity 25 (N=1 success 1.00)

## Agent capacity on real-page traces only (rfc, w3c, mdn; the registered basis)

- engine-proc-full: capacity 100 (N=1 1.00; by N: 1:1.00, 5:1.00, 10:1.00, 25:1.00, 50:1.00, 100:1.00)
- engine-proc-lean: capacity 100 (N=1 1.00; by N: 1:1.00, 5:1.00, 10:1.00, 25:1.00, 50:1.00, 100:1.00)
- engine-fork: capacity 100 (N=1 1.00; by N: 1:1.00, 5:1.00, 10:1.00, 25:1.00, 50:1.00, 100:1.00)
- chrome-mcp: capacity 10 (N=1 1.00; by N: 1:1.00, 5:1.00, 10:1.00, 25:0.00, 50:0.00, 100:0.00)
- pw-proc: capacity 10 (N=1 1.00; by N: 1:1.00, 5:1.00, 10:1.00, 25:0.00, 50:0.00, 100:0.00)
- pw-shared: capacity 25 (N=1 1.00; by N: 1:1.00, 5:1.00, 10:1.00, 25:1.00, 50:0.00, 100:0.00)

## Bisection (CONC.2.a): real-page success by tested N, OOM-killed runs counted 0; bracket = [largest ok N, first bad N)

- engine-proc-full: capacity in [93,99] (width 6); by N: 1:1.00, 5:1.00, 10:1.00, 25:1.00, 50:1.00, 75:1.00, 87:1.00, 93:1.00, 100:0.67
- engine-proc-lean: capacity in [100,inf] (open); by N: 1:1.00, 5:1.00, 10:1.00, 25:1.00, 37:1.00, 43:1.00, 46:1.00, 50:1.00, 100:1.00
- engine-fork: capacity in [100,inf] (open); by N: 1:1.00, 5:1.00, 10:1.00, 25:1.00, 50:1.00, 75:1.00, 87:1.00, 93:1.00, 100:1.00
- chrome-mcp: capacity in [21,22] (width 1); by N: 1:1.00, 5:1.00, 10:1.00, 17:1.00, 21:1.00, 23:0.00, 25:0.00, 50:0.00, 100:0.00
- pw-proc: capacity in [18,18] (width 0); by N: 1:1.00, 5:1.00, 10:1.00, 17:1.00, 18:1.00, 19:0.00, 21:0.00, 25:0.00, 50:0.00, 100:0.00
- pw-shared: capacity in [37,39] (width 2); by N: 1:1.00, 5:1.00, 10:1.00, 25:1.00, 37:1.00, 40:0.67, 43:0.00, 50:0.00, 100:0.00

## Pre-registered bars (after bisection)

best Chrome arm by capacity: pw-shared (in [37,39]).
- **Capacity win (engine-proc-full): capacity in [93,99] vs pw-shared in [37,39]; ratio between 2.38x and 2.68x (need >=1.5x): PASS**
- **Capacity win (engine-proc-lean): capacity in [100,None] vs pw-shared in [37,39]; ratio between 2.56x and 27.00x (need >=1.5x): PASS**
- **Capacity win (engine-fork): capacity in [100,None] vs pw-shared in [37,39]; ratio between 2.56x and 27.00x (need >=1.5x): PASS**
- **Cost win (registered, N=50): NOT EVALUABLE - no Chrome arm survived N=50 in the 8 GB cage (24 cells OOM-killed; journal shows 24 oom-kill scopes). Not scored as a pass.**

Exploratory, NOT the registered test (registered N=50 is unmeasurable: every Chrome arm is OOM-killed at N=50 in the 8 GB cage). Largest N where a Chrome arm survived all runs (pw-shared N=25):
- engine-proc-full N=25: 11 MB/ok vs pw-shared 28: 60% lower
- engine-proc-lean N=25: 10 MB/ok vs pw-shared 28: 63% lower
- engine-fork N=25: 11 MB/ok vs pw-shared 28: 62% lower

Success within 5 points of best Chrome arm at the same N (n/a where every Chrome run was OOM-killed):
- engine-proc-full N=1: 1.00 vs best Chrome 1.00 -> ok
- engine-proc-full N=5: 1.00 vs best Chrome 1.00 -> ok
- engine-proc-full N=10: 1.00 vs best Chrome 1.00 -> ok
- engine-proc-full N=25: 1.00 vs best Chrome 1.00 -> ok
- engine-proc-full N=50: 1.00 vs best Chrome n/a (all Chrome arms OOM-killed)
- engine-proc-full N=100: 1.00 vs best Chrome n/a (all Chrome arms OOM-killed)
- engine-proc-lean N=1: 1.00 vs best Chrome 1.00 -> ok
- engine-proc-lean N=5: 1.00 vs best Chrome 1.00 -> ok
- engine-proc-lean N=10: 1.00 vs best Chrome 1.00 -> ok
- engine-proc-lean N=25: 1.00 vs best Chrome 1.00 -> ok
- engine-proc-lean N=50: 1.00 vs best Chrome n/a (all Chrome arms OOM-killed)
- engine-proc-lean N=100: 0.99 vs best Chrome n/a (all Chrome arms OOM-killed)
- engine-fork N=1: 1.00 vs best Chrome 1.00 -> ok
- engine-fork N=5: 1.00 vs best Chrome 1.00 -> ok
- engine-fork N=10: 1.00 vs best Chrome 1.00 -> ok
- engine-fork N=25: 1.00 vs best Chrome 1.00 -> ok
- engine-fork N=50: 1.00 vs best Chrome n/a (all Chrome arms OOM-killed)
- engine-fork N=100: 1.00 vs best Chrome n/a (all Chrome arms OOM-killed)

## Infra / died cells (listed, not dropped)

- chrome-mcp N=50 run=1: cell died: 
- pw-proc N=25 run=1: cell died: 
- pw-proc N=100 run=1: cell died: 
- engine-proc-full N=100 run=1: cell died: 
- pw-shared N=50 run=1: cell died: 
- chrome-mcp N=100 run=1: cell died: 
- pw-proc N=50 run=1: cell died: 
- pw-shared N=100 run=1: cell died: 
- chrome-mcp N=25 run=1: cell died: 
- chrome-mcp N=25 run=2: cell died: 
- pw-proc N=50 run=2: cell died: 
- pw-shared N=100 run=2: cell died: 
- chrome-mcp N=100 run=2: cell died: 
- pw-shared N=50 run=2: cell died: 
- pw-proc N=100 run=2: cell died: 
- chrome-mcp N=50 run=2: cell died: 
- pw-proc N=25 run=2: cell died: 
- pw-proc N=100 run=3: cell died: 
- chrome-mcp N=100 run=3: cell died: 
- chrome-mcp N=50 run=3: cell died: 
- pw-shared N=50 run=3: cell died: 
- pw-proc N=50 run=3: cell died: 
- chrome-mcp N=25 run=3: cell died: 
- pw-proc N=25 run=3: cell died: 
- pw-shared N=100 run=3: cell died: 
