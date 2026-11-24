## Success (mean [min-max] over surviving runs; infra = cells OOM-killed by the 8 GB cage; counted as success 0 in capacity)

| arm | N=1 | N=5 | N=10 | N=25 | N=50 | N=100 |
|---|---|---|---|---|---|---|
| engine-proc-full | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | 0.94 [0.92-0.95] n=3 | 0.82 [0.75-0.90] n=3 |
| engine-proc-lean | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | 0.98 [0.95-1.00] n=3 | 0.88 [0.84-0.90] n=3 | 0.79 [0.77-0.82] n=3 |
| engine-fork | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | 0.97 [0.96-0.97] n=3 | 0.85 [0.83-0.88] n=3 |
| chrome-mcp | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | - (+3 infra) | - (+3 infra) | - (+3 infra) |
| pw-proc | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | - (+3 infra) | - (+3 infra) | - (+3 infra) |
| pw-shared | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | 1.00 [1.00-1.00] n=3 | - (+3 infra) | - (+3 infra) |

## Peak PSS of process tree, MB (mean [min-max])

| arm | N=1 | N=5 | N=10 | N=25 | N=50 | N=100 |
|---|---|---|---|---|---|---|
| engine-proc-full | 126 [113-148] | 542 [489-641] | 1029 [884-1261] | 2216 [2134-2341] | 3668 [3504-3921] | 7241 [6877-7749] |
| engine-proc-lean | 115 [111-117] | 457 [453-460] | 880 [877-884] | 2118 [2068-2152] | 3640 [3542-3801] | 6986 [6884-7068] |
| engine-fork | 127 [113-149] | 538 [469-664] | 1027 [879-1264] | 2137 [2045-2201] | 3785 [3517-4101] | 7040 [6801-7175] |
| chrome-mcp | 657 [646-672] | 2326 [2320-2330] | 4369 [4350-4379] | - | - | - |
| pw-proc | 706 [700-708] | 2550 [2540-2562] | 4812 [4806-4822] | - | - | - |
| pw-shared | 711 [700-725] | 1652 [1587-1689] | 2745 [2644-2832] | 5896 [5724-6021] | - | - |

## Successful traces per minute (mean [min-max])

| arm | N=1 | N=5 | N=10 | N=25 | N=50 | N=100 |
|---|---|---|---|---|---|---|
| engine-proc-full | 22.7 [22.6-22.7] | 103.6 [95.0-115.5] | 169.5 [143.6-196.9] | 231.8 [215.9-240.5] | 170.0 [166.7-174.1] | 219.5 [141.3-328.1] |
| engine-proc-lean | 21.8 [21.1-22.8] | 75.8 [69.9-79.5] | 111.4 [108.1-114.5] | 122.9 [105.4-135.6] | 105.5 [96.5-113.7] | 133.2 [123.0-149.5] |
| engine-fork | 22.3 [19.9-23.5] | 96.7 [89.6-109.6] | 170.8 [165.0-180.5] | 223.1 [198.5-240.7] | 183.2 [174.1-197.8] | 166.2 [153.9-187.5] |
| chrome-mcp | 14.2 [13.3-15.1] | 37.9 [34.8-40.9] | 66.7 [56.3-76.9] | - | - | - |
| pw-proc | 39.6 [35.3-41.9] | 124.5 [112.8-137.4] | 218.2 [215.5-220.9] | - | - | - |
| pw-shared | 39.9 [37.6-43.3] | 113.3 [107.8-123.6] | 181.8 [160.1-210.4] | 301.6 [259.2-326.5] | - | - |

## p95 ms per tool call (mean [min-max])

| arm | N=1 | N=5 | N=10 | N=25 | N=50 | N=100 |
|---|---|---|---|---|---|---|
| engine-proc-full | 2637 [2392-3037] | 2761 [2400-2973] | 3272 [2524-3738] | 6345 [6234-6418] | 27739 [25569-30107] | 45417 [36050-50101] |
| engine-proc-lean | 2564 [2451-2729] | 8382 [7936-8751] | 10890 [10617-11164] | 21366 [20720-21968] | 42094 [37081-49577] | 50095 [50086-50100] |
| engine-fork | 2419 [2329-2465] | 3236 [3150-3352] | 3207 [2851-3448] | 6719 [5530-8000] | 24025 [18702-30891] | 57731 [52994-60100] |
| chrome-mcp | 3068 [2768-3487] | 12708 [11015-13964] | 15780 [10547-25989] | - | - | - |
| pw-proc | 957 [878-1100] | 2929 [1412-3794] | 5190 [5177-5209] | - | - | - |
| pw-shared | 1012 [894-1073] | 4562 [3322-5189] | 4119 [1743-5459] | 5555 [5386-5892] | - | - |

## CPU seconds (mean [min-max])

| arm | N=1 | N=5 | N=10 | N=25 | N=50 | N=100 |
|---|---|---|---|---|---|---|
| engine-proc-full | 19 [19-19] | 105 [94-114] | 246 [211-276] | 794 [789-798] | 2009 [1973-2037] | 3313 [1826-4544] |
| engine-proc-lean | 19 [18-20] | 105 [100-113] | 235 [229-244] | 814 [781-856] | 1950 [1908-1986] | 2980 [2670-3271] |
| engine-fork | 20 [18-22] | 113 [99-122] | 241 [233-249] | 813 [777-874] | 1974 [1961-1984] | 3708 [3366-3932] |
| chrome-mcp | 8 [6-9] | 42 [40-44] | 92 [86-95] | - | - | - |
| pw-proc | 7 [7-9] | 41 [40-43] | 90 [85-95] | - | - | - |
| pw-shared | 8 [7-8] | 31 [28-33] | 73 [52-86] | 190 [180-208] | - | - |

## PSS per successful trace, MB (mean [min-max])

| arm | N=1 | N=5 | N=10 | N=25 | N=50 | N=100 |
|---|---|---|---|---|---|---|
| engine-proc-full | 16 [14-18] | 14 [12-16] | 13 [11-16] | 11 [11-12] | 10 [9-11] | 11 [10-13] |
| engine-proc-lean | 14 [14-15] | 11 [11-11] | 11 [11-11] | 11 [10-11] | 10 [10-11] | 11 [11-11] |
| engine-fork | 16 [14-19] | 13 [12-17] | 13 [11-16] | 11 [10-11] | 10 [9-11] | 10 [10-11] |
| chrome-mcp | 82 [81-84] | 58 [58-58] | 55 [54-55] | - | - | - |
| pw-proc | 88 [88-89] | 64 [64-64] | 60 [60-60] | - | - | - |
| pw-shared | 89 [87-91] | 41 [40-42] | 34 [33-35] | 29 [29-30] | - | - |

## Agent capacity (largest ladder N with mean success within 5 points of the arm's N=1; ladder only, no bisection)

- engine-proc-full: capacity 25 (N=1 success 1.00)
- engine-proc-lean: capacity 25 (N=1 success 1.00)
- engine-fork: capacity 50 (N=1 success 1.00)
- chrome-mcp: capacity 10 (N=1 success 1.00)
- pw-proc: capacity 10 (N=1 success 1.00)
- pw-shared: capacity 25 (N=1 success 1.00)

## Agent capacity on real-page traces only (rfc, w3c, mdn; the registered basis)

- engine-proc-full: capacity 50 (N=1 1.00; by N: 1:1.00, 5:1.00, 10:1.00, 25:1.00, 50:1.00, 100:0.85)
- engine-proc-lean: capacity 25 (N=1 1.00; by N: 1:1.00, 5:1.00, 10:1.00, 25:0.96, 50:0.88, 100:0.75)
- engine-fork: capacity 50 (N=1 1.00; by N: 1:1.00, 5:1.00, 10:1.00, 25:1.00, 50:1.00, 100:0.91)
- chrome-mcp: capacity 10 (N=1 1.00; by N: 1:1.00, 5:1.00, 10:1.00, 25:0.00, 50:0.00, 100:0.00)
- pw-proc: capacity 10 (N=1 1.00; by N: 1:1.00, 5:1.00, 10:1.00, 25:0.00, 50:0.00, 100:0.00)
- pw-shared: capacity 25 (N=1 1.00; by N: 1:1.00, 5:1.00, 10:1.00, 25:1.00, 50:0.00, 100:0.00)

## Pre-registered bars

best Chrome arm by capacity: pw-shared (25). capacity above uses real-page traces; ladder resolution only.
- **Capacity win (engine-proc-full): capacity in [50,99] vs pw-shared in [25,49]; ratio between 1.02x and 3.96x (need >=1.5x): UNRESOLVED (ladder bracket straddles 1.5x; bisection needed)**
- **Capacity win (engine-proc-lean): capacity in [25,49] vs pw-shared in [25,49]; ratio between 0.51x and 1.96x (need >=1.5x): UNRESOLVED (ladder bracket straddles 1.5x; bisection needed)**
- **Capacity win (engine-fork): capacity in [50,99] vs pw-shared in [25,49]; ratio between 1.02x and 3.96x (need >=1.5x): UNRESOLVED (ladder bracket straddles 1.5x; bisection needed)**
- **Cost win (registered, N=50): NOT EVALUABLE - no Chrome arm survived N=50 in the 8 GB cage (24 cells OOM-killed; journal shows 24 oom-kill scopes). Not scored as a pass.**

Exploratory, NOT the registered test (registered N=50 is unmeasurable: every Chrome arm is OOM-killed at N=50 in the 8 GB cage). Largest N where a Chrome arm survived all runs (pw-shared N=25):
- engine-proc-full N=25: 11 MB/ok vs pw-shared 29: 62% lower
- engine-proc-lean N=25: 11 MB/ok vs pw-shared 29: 63% lower
- engine-fork N=25: 11 MB/ok vs pw-shared 29: 64% lower

Success within 5 points of best Chrome arm at the same N (n/a where every Chrome run was OOM-killed):
- engine-proc-full N=1: 1.00 vs best Chrome 1.00 -> ok
- engine-proc-full N=5: 1.00 vs best Chrome 1.00 -> ok
- engine-proc-full N=10: 1.00 vs best Chrome 1.00 -> ok
- engine-proc-full N=25: 1.00 vs best Chrome 1.00 -> ok
- engine-proc-full N=50: 0.94 vs best Chrome n/a (all Chrome arms OOM-killed)
- engine-proc-full N=100: 0.82 vs best Chrome n/a (all Chrome arms OOM-killed)
- engine-proc-lean N=1: 1.00 vs best Chrome 1.00 -> ok
- engine-proc-lean N=5: 1.00 vs best Chrome 1.00 -> ok
- engine-proc-lean N=10: 1.00 vs best Chrome 1.00 -> ok
- engine-proc-lean N=25: 0.98 vs best Chrome 1.00 -> ok
- engine-proc-lean N=50: 0.88 vs best Chrome n/a (all Chrome arms OOM-killed)
- engine-proc-lean N=100: 0.79 vs best Chrome n/a (all Chrome arms OOM-killed)
- engine-fork N=1: 1.00 vs best Chrome 1.00 -> ok
- engine-fork N=5: 1.00 vs best Chrome 1.00 -> ok
- engine-fork N=10: 1.00 vs best Chrome 1.00 -> ok
- engine-fork N=25: 1.00 vs best Chrome 1.00 -> ok
- engine-fork N=50: 0.97 vs best Chrome n/a (all Chrome arms OOM-killed)
- engine-fork N=100: 0.85 vs best Chrome n/a (all Chrome arms OOM-killed)

## Infra / died cells (listed, not dropped)

- pw-proc N=50 run=1: cell died: 
- chrome-mcp N=25 run=1: cell died: 
- chrome-mcp N=100 run=1: cell died: 
- pw-shared N=100 run=1: cell died: 
- chrome-mcp N=50 run=1: cell died: 
- pw-proc N=25 run=1: cell died: 
- pw-proc N=100 run=1: cell died: 
- pw-shared N=50 run=1: cell died: 
- pw-shared N=50 run=2: cell died: 
- pw-proc N=25 run=2: cell died: 
- chrome-mcp N=25 run=2: cell died: 
- pw-shared N=100 run=2: cell died: 
- chrome-mcp N=50 run=2: cell died: 
- chrome-mcp N=100 run=2: cell died: 
- pw-proc N=50 run=2: cell died: 
- pw-proc N=100 run=2: cell died: 
- chrome-mcp N=25 run=3: cell died: 
- chrome-mcp N=50 run=3: cell died: 
- pw-proc N=100 run=3: cell died: 
- chrome-mcp N=100 run=3: cell died: 
- pw-proc N=50 run=3: cell died: 
- pw-shared N=100 run=3: cell died: 
- pw-proc N=25 run=3: cell died: 
- pw-shared N=50 run=3: cell died: 
