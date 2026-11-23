# Overlap a script chain, and store a memo only when it can hit

> Landed PERF.1.2 (second pass; partial, bar not met: see docs/sprint/PERF.md "PERF.1.2 second pass"). Gates:
> `engine/page/tests/g_dyn_script_prefetch.rs`, `memo_is_stored_only_when_the_key_comes_round_again` (lib tests),
> `scripts/perf1-settle-gate.py`.

## Mechanism (engine/page/src/lib.rs)

- `fetch_and_run_dynamic_scripts` fetched a round with `join_all` and only then ran it, and only looked for the scripts those
  inserted after the whole round. A chain hop therefore waited for the slowest unrelated fetch of its round
  (puentedemando.com: cookieyes -> banner.js waited ~0.9 s on a clarity.ms connect). Now the round is a `FuturesOrdered`
  (each script runs once it and its predecessors have arrived, same order as before) and after each script the new
  `<script src>` it inserted are fetched at once (`pending_dynamic_scripts`, same CSP/mixed-content filter). They still RUN in
  the next round; `manuk_net::fetch` single-flights the URL, so the round's fetch joins the early one. Switch:
  `MANUK_NO_DYN_PREFETCH=1`.
- The layout memo (PERF.1.3.a) stored two whole `StyleMap` clones and a box-tree clone after every slow pass. On Wikipedia
  (~19k nodes) that was 110-190 ms per forced reflow, never repaid: each of those passes lays out a tree a script has just
  grown, so the key is new. `memo_worth_storing` stores only when the key repeats the previous pass's key (comix.to's
  carousel still stores on pass 2 and hits from pass 3; it loses one hit). Switch: `MANUK_MEMO_STORE_ALWAYS=1`.

## Not fixed

Each forced reflow is a full MinimalCascade (the agent build has no Stylo): 250-400 ms on Wikipedia's 19k nodes, and the
`apply_stylesheets` pass that closes `dynamic scripts` is the same again with no trace line (it lands in settle).
An incremental restyle (re-cascade only dirty subtrees) is the remaining lever.
