# AXES.1.a p95 attribution (chrome_ver=145.0.7632.116)

## engine-fork N=1: 81 calls, p50 33.0 ms, p95 602.2 ms

| op | calls | p50 ms | p95 ms | share of calls >= overall p95 |
|---|---|---|---|---|
| click | 15 | 40.0 | 515.7 | 0/5 |
| find | 3 | 71.9 | 72.0 | 0/5 |
| nav | 27 | 51.8 | 1174.8 | 5/5 |
| snap | 33 | 1.9 | 99.2 | 0/5 |
| type | 3 | 10.5 | 13.9 | 0/5 |

| trace:op:phase | calls | p95 ms | share of tail |
|---|---|---|---|
| mdn:nav:step | 3 | 1397.4 | 3/5 |
| heavy:nav:step | 3 | 606.9 | 2/5 |
| iso:nav:step | 6 | 68.8 | 0/5 |
| form:nav:step | 3 | 34.0 | 0/5 |
| form:snap:step | 6 | 5.1 | 0/5 |
| form:type:step | 3 | 13.9 | 0/5 |

## pw-proc N=1: 96 calls, p50 61.9 ms, p95 941.1 ms

| op | calls | p50 ms | p95 ms | share of calls >= overall p95 |
|---|---|---|---|---|
| click | 15 | 640.0 | 1118.2 | 6/6 |
| nav | 27 | 340.1 | 835.6 | 0/6 |
| snap | 51 | 12.3 | 483.6 | 0/6 |
| type | 3 | 56.0 | 62.6 | 0/6 |

| trace:op:phase | calls | p95 ms | share of tail |
|---|---|---|---|
| late:click:step | 3 | 1105.8 | 3/6 |
| heavy:click:step | 3 | 1301.2 | 3/6 |
| iso:nav:step | 6 | 767.3 | 0/6 |
| iso:snap:poll | 6 | 25.1 | 0/6 |
| form:nav:step | 3 | 132.9 | 0/6 |
| form:snap:poll | 6 | 19.3 | 0/6 |

## engine-proc-full N=25: 2025 calls, p50 52.0 ms, p95 1969.5 ms

| op | calls | p50 ms | p95 ms | share of calls >= overall p95 |
|---|---|---|---|---|
| click | 375 | 85.6 | 1835.2 | 11/102 |
| find | 75 | 243.4 | 407.5 | 0/102 |
| nav | 675 | 627.8 | 2942.9 | 91/102 |
| snap | 825 | 2.3 | 404.1 | 0/102 |
| type | 75 | 14.4 | 25.7 | 0/102 |

| trace:op:phase | calls | p95 ms | share of tail |
|---|---|---|---|
| mdn:nav:step | 75 | 4562.9 | 66/102 |
| heavy:nav:step | 75 | 2552.1 | 17/102 |
| heavy:click:step | 75 | 2113.1 | 11/102 |
| iso:nav:step | 150 | 1353.5 | 7/102 |
| rfc:nav:step | 75 | 1448.9 | 1/102 |
| form:nav:step | 75 | 76.6 | 0/102 |

## pw-shared N=25: 2400 calls, p50 674.2 ms, p95 3152.1 ms

| op | calls | p50 ms | p95 ms | share of calls >= overall p95 |
|---|---|---|---|---|
| click | 375 | 1502.6 | 3323.1 | 27/121 |
| nav | 675 | 1359.0 | 3989.6 | 94/121 |
| snap | 1275 | 185.8 | 1954.2 | 0/121 |
| type | 75 | 346.0 | 627.7 | 0/121 |

| trace:op:phase | calls | p95 ms | share of tail |
|---|---|---|---|
| mdn:nav:step | 75 | 5695.6 | 35/121 |
| heavy:nav:step | 75 | 4298.1 | 28/121 |
| heavy:click:step | 75 | 4137.3 | 27/121 |
| iso:nav:step | 150 | 3561.1 | 25/121 |
| rfc:nav:step | 75 | 3143.4 | 4/121 |
| w3c:nav:step | 75 | 2767.1 | 2/121 |

