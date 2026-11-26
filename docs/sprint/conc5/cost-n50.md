### CONC.2.b cost bar at N=50 (DEVIATION: Chrome arms measured in a 24 GB cage, engine rows in the registered 8 GB cage; chrome_ver=145.0.7632.116)

| arm | cage GB | runs | mean PSS/ok MB | success mean |
|---|---|---|---|---|
| chrome-mcp | 24 | 3 | 46.7 | 1.00 |
| pw-proc | 24 | 3 | 47.8 | 1.00 |
| pw-shared | 24 | 3 | 26.1 | 1.00 |
| engine-proc-full | 8 | 3 | 10.7 | 1.00 |
| engine-proc-lean | 8 | 3 | 9.9 | 1.00 |
| engine-fork | 8 | 3 | 10.3 | 1.00 |

best Chrome arm: pw-shared 26.1 MB/ok
- **Cost win (engine-proc-full) 10.7 vs 26.1 MB/ok: 59% lower (need >=30%): PASS** (deviation: cage 24 GB for Chrome vs 8 GB for engine)
- **Cost win (engine-proc-lean) 9.9 vs 26.1 MB/ok: 62% lower (need >=30%): PASS** (deviation: cage 24 GB for Chrome vs 8 GB for engine)
- **Cost win (engine-fork) 10.3 vs 26.1 MB/ok: 61% lower (need >=30%): PASS** (deviation: cage 24 GB for Chrome vs 8 GB for engine)
