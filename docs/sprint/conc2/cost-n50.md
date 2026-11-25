### CONC.2.b cost bar at N=50 (DEVIATION: Chrome arms measured in a 24 GB cage, engine rows in the registered 8 GB cage; chrome_ver=145.0.7632.116)

| arm | cage GB | runs | mean PSS/ok MB | success mean |
|---|---|---|---|---|
| chrome-mcp | 24 | 3 | 51.0 | 1.00 |
| pw-proc | 24 | 3 | 56.0 | 1.00 |
| pw-shared | 24 | 3 | 25.6 | 1.00 |
| engine-proc-full | 8 | 3 | 9.8 | 0.94 |
| engine-proc-lean | 8 | 3 | 10.4 | 0.88 |
| engine-fork | 8 | 3 | 9.8 | 0.97 |

best Chrome arm: pw-shared 25.6 MB/ok
- **Cost win (engine-proc-full) 9.8 vs 25.6 MB/ok: 62% lower (need >=30%): PASS** (deviation: cage 24 GB for Chrome vs 8 GB for engine)
- **Cost win (engine-proc-lean) 10.4 vs 25.6 MB/ok: 59% lower (need >=30%): PASS** (deviation: cage 24 GB for Chrome vs 8 GB for engine)
- **Cost win (engine-fork) 9.8 vs 25.6 MB/ok: 62% lower (need >=30%): PASS** (deviation: cage 24 GB for Chrome vs 8 GB for engine)
