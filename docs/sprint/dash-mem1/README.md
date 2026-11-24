# DASH.MEM.1 — why one manuk-mcp --backend engine reaches ~4.5G RSS on a dashdish task

chrome_ver=145.0.7632.116 (n/a: no Chrome in the loop; pin recorded for the ledger). Driver: `scripts/dash-mem-trace.py`
(replays real_run.py's setup: /config, /finish, then the start URL, observe x2, idle; samples /proc RSS every 1s;
run under `systemd-run --user --scope -p MemoryMax=6G -p MemorySwapMax=0`). Task dashdish-5, binary target/release/manuk-mcp (built 2026-11-23 07:34).

## Traces (3 runs each, committed here)
| arm | file | max RSS | start-URL navigate |
|---|---|---|---|
| default (`agent profile=full`) | new-dashdish-5-r{1,2,3}.tsv | 4502 / 4569 / 4513 MB | 23.6 / 23.3 / 23.5 s |
| `MANUK_AGENT_PROFILE=lean` | new-lean-dashdish-5-r{1,2,3}.tsv | 514 / 518 / 520 MB | 12.0 / 11.1 / 11.0 s |

Setup calls (/config, /finish) sit at 43-53 MB. RSS is flat after the navigate returns (4.5G idle, 20-70s): a one-shot
spike during page load, not a leak over the session.

## Where it grows (phase-log-full-profile.txt aligned with the RSS trace)
- page JS renders 7531 DOM nodes; RSS is still ~170-270 MB at that point (t=6-9s).
- The 4.2 GB arrives after `subframes (pre-load)`, inside `initial images+masks` (11.7 s) and `load event` (3.6 s):
  `fetch_and_apply_images` decodes every `<img>`/mask to straight RGBA (`image::load_from_memory` -> `to_rgba8`,
  `decode_svg` rasterises each SVG at natural size up to 8192x8192). 372 usvg parses fire in that window.
- Lean profile skips image bodies (`lean::images_deferred`) and the same page peaks at ~515 MB: ~4.0 GB is decoded image pixels.
- Not named: the individual URLs/sizes (stripped binary, no perf/ptrace here, evaluate is chrome-backend only) -> follow-up .b.

## Verdict: ALWAYS SO (not a regression of the 07:34 binary)
`git diff c31b1481^ 59859130 -- engine Cargo.lock` is only engine/net/src/secrets.rs (+78) and one net test; the image
decode/apply path, the default profile (unchanged since 53c3b345, 2026-11-01) and mcp image handling are untouched. A
build of c31b1481^ was NOT made: the constraints demand >=20G MemAvailable (18G at last read) and the bench holds the
host. The A/B build is follow-up .a; by the diff it should reproduce ~4.5G.
Implication for the TIMEOUT rows: one task needs ~4.5G for ~25s then stays resident; two in parallel = ~9G > MemoryHigh=8G,
so cgroup throttling is consistent (still not proven here).

## DASH.MEM.1.a: A/B build of c31b1481^ (done)
Built `cargo build --release -p manuk-mcp` at c31b1481^ in a git worktree with a separate CARGO_TARGET_DIR (jobs=6,
MOZJS_ARCHIVE pointed at the verified archive; worktree and target dir removed afterwards). Same driver, same task,
3 runs (old-dashdish-5-r{1,2,3}.tsv): max RSS 4510 / 4510 / 4505 MB, start-URL navigate 21.2 / 19.6 / 17.5 s, setup 45-54 MB.
New (current) binary: 4502 / 4569 / 4513 MB. **Verdict confirmed: ALWAYS SO**, not a regression of the 07:34 binary;
the ~4.5G is the image decode on this page in both builds. chrome_ver=145.0.7632.116 (n/a, no Chrome in the loop).

