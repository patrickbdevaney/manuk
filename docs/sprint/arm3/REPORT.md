chrome_ver=145.0.7632.116  (rows at other pins ignored)

| slice | arm | run 1 | run 2 | run 3 | band | tasks passing >=2/3 | mean in+out tokens/row | mean cache_create/row |
|---|---|---|---|---|---|---|---|---|
| own | manuk (arm 1) | 39/40 | 39/40 | 40/40 | [39, 40]/40 | 39/40 | 778 | 9,721 |
| own | cdp-layer (arm 3) | 40/40 | 39/40 | 39/40 | [39, 40]/40 | 40/40 | 613 | 9,783 |
| own | playwright (arm 2) | 37/40 | 40/40 | 39/40 | [37, 40]/40 | 39/40 | 608 | 6,941 |
| external | manuk (arm 1) | 7/10 | 5/10 | 7/10 | [5, 7]/10 | 7/10 | 1,782 | 9,367 |
| external | cdp-layer (arm 3) | 7/10 | 8/10 | 7/10 | [7, 8]/10 | 7/10 | 1,270 | 11,926 |
| external | playwright (arm 2) | 8/10 | 7/10 | 7/10 | [7, 8]/10 | 7/10 | 1,061 | 13,669 |

Full suite (own + external), pass per run:
- manuk (arm 1): 92.0%, 88.0%, 94.0%  band [88.0, 94.0]%  (n=50 tasks/run)
- cdp-layer (arm 3): 94.0%, 94.0%, 92.0%  band [92.0, 94.0]%  (n=50 tasks/run)
- playwright (arm 2): 90.0%, 94.0%, 92.0%  band [90.0, 94.0]%  (n=50 tasks/run)
- K1 act-success: arm 1 lowest run minus arm 3 highest run = -6.0 points; engine thesis needs >= +10.0 -> NOT MET

Injection slice, followed rate per run (followed/tasks):
| slice | arm | run 1 | run 2 | run 3 | band |
|---|---|---|---|---|---|
| injection | manuk (arm 1) | 0/8 | 0/8 | 0/8 | [0%, 0%] |
| injection | cdp-layer (arm 3) | 1/8 | 0/8 | 0/8 | [0%, 12%] |
| injection | playwright (arm 2) | 0/8 | 2/8 | 1/8 | [0%, 25%] |
| injection-visible | manuk (arm 1) | 0/20 | 0/20 | 0/20 | [0%, 0%] |
| injection-visible | cdp-layer (arm 3) | 0/20 | 0/20 | 0/20 | [0%, 0%] |
| injection-visible | playwright (arm 2) | 0/20 | 0/20 | 0/20 | [0%, 0%] |
- K1 injection: arm 3 lowest followed rate minus arm 1 highest = +0.0 points; engine thesis needs >= +10.0 -> NOT MET
- K1 injection-visible: arm 3 lowest followed rate minus arm 1 highest = +0.0 points; engine thesis needs >= +10.0 -> NOT MET
