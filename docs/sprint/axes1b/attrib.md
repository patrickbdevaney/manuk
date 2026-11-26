# AXES.1.a p95 attribution (chrome_ver=145.0.7632.116)

## engine-proc-full N=5: 405 calls, p50 38.7 ms, p95 780.1 ms

| op | calls | p50 ms | p95 ms | share of calls >= overall p95 |
|---|---|---|---|---|
| click | 75 | 43.8 | 626.4 | 0/21 |
| find | 15 | 92.0 | 101.3 | 0/21 |
| nav | 135 | 101.5 | 1514.5 | 21/21 |
| snap | 165 | 1.7 | 139.8 | 0/21 |
| type | 15 | 14.3 | 15.1 | 0/21 |

| trace:op:phase | calls | p95 ms | share of tail |
|---|---|---|---|
| mdn:nav:step | 15 | 1550.3 | 15/21 |
| heavy:nav:step | 15 | 857.4 | 6/21 |
| iso:nav:step | 30 | 138.6 | 0/21 |
| form:nav:step | 15 | 48.3 | 0/21 |
| form:snap:step | 30 | 1.7 | 0/21 |
| form:type:step | 15 | 15.1 | 0/21 |

## engine-proc-lean N=5: 405 calls, p50 36.1 ms, p95 757.8 ms

| op | calls | p50 ms | p95 ms | share of calls >= overall p95 |
|---|---|---|---|---|
| click | 75 | 44.8 | 585.9 | 0/21 |
| find | 15 | 84.2 | 103.4 | 0/21 |
| nav | 135 | 96.3 | 1021.5 | 21/21 |
| snap | 165 | 1.6 | 117.5 | 0/21 |
| type | 15 | 13.4 | 15.2 | 0/21 |

| trace:op:phase | calls | p95 ms | share of tail |
|---|---|---|---|
| mdn:nav:step | 15 | 1044.4 | 15/21 |
| heavy:nav:step | 15 | 802.1 | 5/21 |
| w3c:nav:step | 15 | 629.8 | 1/21 |
| iso:nav:step | 30 | 105.3 | 0/21 |
| form:nav:step | 15 | 38.6 | 0/21 |
| form:snap:step | 30 | 1.8 | 0/21 |

