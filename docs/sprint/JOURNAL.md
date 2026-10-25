# Sprint journal — one line per landed graph node

Format: `<node>: <what now works, user-visible> | gate: <check> | evidence: <numbers/transcript>`

R0.1: BLOCKED (manuk half landed) — a cold release build of manuk, manuk-agent and manuk-bidi now completes in 4m23s with 2.2 GB under target/release, and a headless manuk page load is measured per URL with memory bands; the Chrome column is not measured because headless Chrome never exits on this box | gate: docs/sprint/BASELINE.md with bands + `cargo build --release` green — the build gate passes, the Chrome half of the title does not (split to R0.1.a) | evidence: build 264 s wall, 486 crates, 0 errors; a11y-dump on 7 of 10 corpus URLs OK, PSS sum 1544/1568/1556 MB over 3 runs, wall 49.0/47.6/49.7 s; 3 URLs FAIL (connection refused, TLS UnknownIssuer, 30 s timeout); gate red-checks: nonexistent binary → SPAWN, dead URL → FAIL rc 1, 200 MB allocator → 209 MB RSS; chrome_ver=145.0.7632.116 (Chrome dump-dom hangs, 0/30 usable rows)
