# Manuk — RESUME (deterministic resume pointer)

_A fresh session reads [[CONSTITUTION]] then this file, and resumes at the named item._

## Where the loop is

- **TICKS = 0** (about to run Tick 1).
- Working tree: clean, on `main`, pushed. Parity 72/72.

## Next action (Tick 1)

Select by UCB from [[LEDGER]]. At TICKS=0 the explore bonus is equal for all (T=0), so exploit
dominates: **L01 — `fetch()` + `XMLHttpRequest` in page JS** (V/C = 10/5 = 2.0, the highest;
unblocks SPA data loading, the biggest COMPAT lever). Plan: add a native `fetch`/`XHR` binding in
`engine/js/src/dom_bindings.rs` that performs the request via `manuk_net` and resolves a
Promise/`onreadystatechange`, draining through the existing event loop; verify headlessly with an
interactive page test (a script that `fetch()`es and writes the result into the DOM).

## Re-establish context

```
cd /home/patrickd/manuk
cargo build -q --workspace 2>&1 | grep -E "^error"      # expect none
cargo run -q -p manuk-wpt --release -- parity | tail -1 # expect 72/72
cat docs/loop/LEDGER.md docs/loop/STATE.md              # take stock
```
