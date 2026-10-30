#!/usr/bin/env bash
# ── gate for bench/agent-tasks/real_report.py (W3.18 gate 2): green on a synthetic slice that meets the bar,
#    red on each way of missing it. usage: real-gate2-test.sh
set -uo pipefail
R=/home/patrickd/manuk
T=$(mktemp -d /tmp/real-gate2-test.XXXXXX); trap 'rm -rf "$T"' EXIT
fail=0
ok()  { echo "  ok   $*"; }
bad() { echo "  FAIL $*"; fail=1; }
# gen OUT [mutant]: 10 tasks x 3 runs x 4 arms. manuk passes only t9; rollback also passes t0,t1 with a restore.
gen() { python3 -I - "$@" <<'PY'
import sys
out, mut = sys.argv[1], (sys.argv[2:] or [""])[0]
cols = ["ts","arm","suite","chrome_ver","task","site","run","model","pass","evals_ok","evals_n","state_ok","checkpoints",
        "restores","forks","input_tokens","output_tokens","cost_usd","seconds","turns","is_error","answer"]
rows = []
for arm in ["manuk", "manuk+rollback.v2", "cdp-layer", "playwright"]:
    for i in range(10):
        for k in (1, 2, 3):
            if mut == "missing" and arm == "manuk+rollback.v2" and i == 0 and k == 3:
                continue
            p = i == 9 or (arm == "manuk+rollback.v2" and i in (0, 1) and not (mut == "one-flip" and i == 1))
            if mut == "overlap" and arm == "manuk" and k == 1 and i in (5, 6):
                p = True
            rst = int(arm == "manuk+rollback.v2" and i in (0, 1) and mut != "no-restore")
            ver = "145.0.0.0" if mut == "ver" and i == 3 and k == 2 else "145.0.7632.116"
            r = dict(ts="t", arm=arm, suite="real-v1-det", chrome_ver=ver, task=f"t{i}", site="s", run=k, model="haiku",
                     **{"pass": int(p)}, evals_ok=int(p), evals_n=1, state_ok=1, checkpoints=rst, restores=rst, forks=0,
                     input_tokens=1, output_tokens=1, cost_usd=0, seconds=1, turns=1, is_error=0, answer="")
            rows.append("\t".join(str(r[c]) for c in cols))
open(out, "w").write("\t".join(cols) + "\n" + "\n".join(rows) + "\n")
PY
}
run() { python3 -I "$R/bench/agent-tasks/real_report.py" --tsv "$1" >"$T/out" 2>&1; echo $?; }

gen "$T/g.tsv";              rc=$(run "$T/g.tsv");  [ "$rc" -eq 0 ] && ok "green: M/N=2/9, bands 3 vs 1 (rc=0)" || { bad "green rc=$rc"; cat "$T/out"; }
gen "$T/a.tsv" no-restore;   rc=$(run "$T/a.tsv");  [ "$rc" -eq 1 ] && ok "mutant: flips without restore do not count (rc=1)" || bad "no-restore rc=$rc (want 1)"
gen "$T/b.tsv" one-flip;     rc=$(run "$T/b.tsv");  [ "$rc" -eq 1 ] && ok "mutant: M/N=1/9 < 15% (rc=1)" || bad "one-flip rc=$rc (want 1)"
gen "$T/c.tsv" overlap;      rc=$(run "$T/c.tsv");  [ "$rc" -eq 1 ] && ok "mutant: bands overlap 1-3 vs 3-3 (rc=1)" || bad "overlap rc=$rc (want 1)"
gen "$T/d.tsv" missing;      rc=$(run "$T/d.tsv");  [ "$rc" -eq 2 ] && ok "mutant: a missing run is incomplete (rc=2)" || bad "missing rc=$rc (want 2)"
gen "$T/e.tsv" ver;          rc=$(run "$T/e.tsv");  [ "$rc" -eq 1 ] && grep -q 'chrome_ver' "$T/out" && ok "mutant: mixed chrome_ver refused" || bad "ver rc=$rc"

[ "$fail" -eq 0 ] && echo "real-gate2-test: PASS" || { echo "real-gate2-test: FAIL"; exit 1; }
