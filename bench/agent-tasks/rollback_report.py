#!/usr/bin/env python3
"""ROLLBACK.RUN: score the manuk+rollback arm against the manuk baseline on the same 50 tasks (own 40 + X01-X10).

  rollback_report.py [--arm manuk+rollback|manuk+rollback.v2] [--scoreboard FILE] [--transcripts DIR]

N = tasks that fail the Bar pass rule (pass in >=2 of 3 runs) without rollback.
M = those that pass with rollback AND whose transcripts call restore (a pass that never restored does not count).
Prints the table V1.4.md quotes. Run with `python3 -I`.
"""
import argparse, collections, csv, glob, json, os

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
OWN_HASH, FULL_HASH = "69903c", "061ee2"
BASE_TASKS = {f"X{i:02d}" for i in range(1, 11)}


def tok(r):
    def f(k):
        return int(r[k]) if r[k].strip().isdigit() else 0
    return f("input_tokens") + f("output_tokens")


def grid(rows):
    g = collections.defaultdict(dict)
    for r in rows:
        g[r["task"]][int(r["run"])] = int(r["pass"])
    return g


def passes(d):
    return sum(d.values()) >= 2


def restore_calls(path):
    return tool_uses(path)["restore"]


def tool_uses(path):
    """Counts per transcript: restore/checkpoint/fork calls, and ToolSearch calls whose query names the rollback tools."""
    n = collections.Counter()
    for line in open(path):
        if not line.startswith("{"):
            continue
        m = json.loads(line).get("message")
        if not isinstance(m, dict) or not isinstance(m.get("content"), list):
            continue
        for b in m["content"]:
            if not (isinstance(b, dict) and b.get("type") == "tool_use"):
                continue
            name = str(b.get("name", ""))
            for tool in ("restore", "checkpoint", "fork"):
                if name.endswith("__" + tool):
                    n[tool] += 1
            if name == "ToolSearch" and any(t in str(b.get("input", {}).get("query", "")) for t in ("restore", "checkpoint", "fork")):
                n["toolsearch_rollback"] += 1
    return n


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--arm", default="manuk+rollback", choices=["manuk+rollback", "manuk+rollback.v2"])
    ap.add_argument("--scoreboard", default=os.path.join(ROOT, "docs/sprint/SCOREBOARD.tsv"))
    ap.add_argument("--transcripts", default=os.path.join(ROOT, "docs/sprint/rollback-run/transcripts"))
    a = ap.parse_args()
    rows = list(csv.DictReader(open(a.scoreboard), delimiter="\t"))
    base = [r for r in rows if r["arm"] == "manuk" and r["chrome_ver"] == os.environ.get("MANUK_CHROME_PIN", r["chrome_ver"])]
    base_own = [r for r in base if r["suite_hash"].startswith(OWN_HASH) and r["slice"] == "own"][-120:]
    base_ext = [r for r in base if r["suite_hash"].startswith(FULL_HASH) and r["slice"] == "external" and r["task"] in BASE_TASKS][-30:]
    rb = [r for r in rows if r["arm"] == a.arm and r["task"] not in {"X11", "X12", "X13", "X14"}]
    assert rb, f"no {a.arm} rows in the scoreboard"
    B, RB = grid(base_own + base_ext), grid(rb)
    tasks = sorted(set(B) & set(RB))
    assert len(tasks) == 50, f"expected the 50 common tasks, got {len(tasks)}"
    failed = [t for t in tasks if not passes(B[t])]
    flipped = [t for t in failed if passes(RB[t])]
    calls = {}
    for t in flipped:
        for run in (1, 2, 3):
            p = os.path.join(a.transcripts, f"{a.arm}_{t}_r{run}.jsonl")
            calls[(t, run)] = restore_calls(p) if os.path.exists(p) else -1
    counted = [t for t in flipped if any(calls[(t, r)] > 0 for r in (1, 2, 3))]
    print(f"tasks {len(tasks)}  baseline per-run {[sum(B[t].get(k, 0) for t in tasks) for k in (1, 2, 3)]}  "
          f"rollback per-run {[sum(RB[t].get(k, 0) for t in tasks) for k in (1, 2, 3)]}")
    print(f"tasks passing >=2/3: baseline {sum(passes(B[t]) for t in tasks)}  rollback {sum(passes(RB[t]) for t in tasks)}")
    print(f"N (failed without rollback) = {len(failed)} {failed}")
    print(f"flipped to pass with rollback = {len(flipped)} {flipped}")
    print(f"M (flipped AND restore called in the transcript) = {len(counted)} {counted}")
    print("restore calls per flipped task and run:", calls)
    # ROLLBACK.RUN.a gate: the arm must exercise the capability, or the branch is unmeasured, not refuted
    used, runs_called, runs_restored = collections.Counter(), 0, 0
    for r in rb:
        p = os.path.join(a.transcripts, f"{a.arm}_{r['task']}_r{r['run']}.jsonl")
        if os.path.exists(p):
            u = tool_uses(p)
            used.update(u)
            runs_called += bool(u["checkpoint"] or u["restore"] or u["fork"])
            runs_restored += u["restore"] > 0
    print(f"capability use over {len(rb)} rows: {dict(used)}; runs that called checkpoint/restore/fork = {runs_called}; runs that called restore = {runs_restored}")
    if runs_called == 0:  # the gate: a rollback arm where nothing was checkpointed did not exercise the capability
        print("RED: no run called checkpoint, restore or fork; the arm did not exercise the capability (branch unmeasured, not refuted)")
        raise SystemExit(1)
    tot = collections.Counter()
    for r in rb:
        tot[r["run"]] += tok(r)
    btot = collections.Counter()
    for r in base_own + base_ext:
        btot[r["run"]] += tok(r)
    print("in+out tokens per run, rollback", [tot[k] for k in "123"], "baseline", [btot[k] for k in "123"])
    print("rollback harness errors", sum(int(r["is_error"] or 0) for r in rb))
    print(f"M/N = {len(counted)}/{len(failed)}  (Bar 2.2 rollback needs >= 15%)")


if __name__ == "__main__":
    main()
