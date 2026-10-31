chrome_ver=145.0.7632.116; suite = full (own 40 + external 10), Haiku, 3 runs; source docs/sprint/w4/B4/

| arm | tasks x runs | pass per run (of tasks) | pass band | tasks passing >=2/3 | turns/task | out tok/task | in+out tok/task (band) | total tok/task (band) | USD/task (band) |
|---|---|---|---|---|---|---|---|---|---|
| manuk | 54 x 3 | 48, 48, 48 | 88.9-88.9% | 48/54 | 5.38 | 717 | 727 [718, 733] | 126,540 [122,667, 132,230] | 0.0026 [0.0025, 0.0029] |
| cdp-layer | 54 x 3 | 50, 49, 51 | 90.7-94.4% | 51/54 | 4.58 | 596 | 604 [574, 637] | 108,586 [105,057, 110,983] | 0.0027 [0.0022, 0.0036] |
| playwright | 54 x 3 | 49, 49, 48 | 88.9-90.7% | 50/54 | 6.27 | 686 | 696 [669, 721] | 122,686 [121,452, 124,331] | 0.0031 [0.0026, 0.0040] |
| playwright+preload | 54 x 3 | 51, 50, 48 | 88.9-94.4% | 51/54 | 5.35 | 509 | 517 [498, 542] | 169,018 [165,806, 175,135] | 0.0031 [0.0026, 0.0039] |

Bar 2.2 tokens branch (manuk vs the best Playwright arm, per metric):
- in+out: manuk 726.8 vs playwright+preload 516.8 -> -40.6% (need >=30% fewer); bands overlap; success within 10 pts: yes -> NOT MET
- total: manuk 1.265e+05 vs playwright 1.227e+05 -> -3.1% (need >=30% fewer); bands overlap; success within 10 pts: yes -> NOT MET
- USD: manuk 0.002628 vs playwright 0.003069 -> +14.4% (need >=30% fewer); bands overlap; success within 10 pts: yes -> NOT MET
- incumbent per metric is the lower of playwright, playwright+preload; per-arm numbers are in the table above
