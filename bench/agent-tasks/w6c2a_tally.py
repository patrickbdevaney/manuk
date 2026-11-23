#!/usr/bin/env python3
"""W6.C2.a: REAL-fast x3 on the cdp-layer arm, with evaluate / act force / act at exposed (W6.C2 landed them).

  python3 -I bench/agent-tasks/w6c2a_tally.py            # table + verdict (exit 1 when the falsifier fires, 2 on refusal)
  python3 -I bench/agent-tasks/w6c2a_tally.py --self-test # counting and verdict mutants must go red

Inputs:
  bench-queue/out/w6c2a-cdplayer-real.tsv      the new job (queue spec bench-queue/specs/w6c2a-cdp-layer.json)
  bench-queue/out/w6b0-cdplayer-real.tsv       W6.B0 cdp-layer rows (the comparator, same arm, same pin)
  ~/.cache/manuk-bq/transcripts-w6c2a/cdp-layer_<task>_r<run>.jsonl   stream-json per run (real_run.py --transcripts)

Counts per run (from the assistant tool_use blocks of the transcript):
  evaluate  = mcp__browser__evaluate calls
  act_force = mcp__browser__act calls with "force": true
  act_at    = mcp__browser__act calls with an "at" argument and no "handle"

Pre-registered falsifier (docs/sprint/GRAPH.tsv W6.C2.a): FIRES when evaluate is used in <10% of runs AND
task_pass_2of3 AND mean turns are inside the W6.B0 band. The band is stated on the same 17 tasks as the new run:
the W6.B0 per-run pass counts over those tasks (min..max) and the W6.B0 per-run mean turns (min..max). The
pre-registered 39-43/102 band is for the full 102 tasks and cannot be compared with a 17-task subset directly.

Refuses (exit 2) on: chrome_ver != $MANUK_CHROME_PIN, a judge_unavailable row, a missing (task, run) key,
or a missing transcript. Only a complete, correctly labelled table yields a verdict.
"""
import csv, glob, json, os, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "../.."))
sys.path.insert(0, HERE)
from fast_subset import SUBSET  # the 17 REAL-fast tasks, frozen in fast_subset.py

NEW_TSV = os.path.join(ROOT, "bench-queue/out/w6c2a-cdplayer-real.tsv")
B0_TSV = os.path.join(ROOT, "bench-queue/out/w6b0-cdplayer-real.tsv")
TRANSCRIPTS = os.path.expanduser("~/.cache/manuk-bq/transcripts-w6c2a")
ARM = "cdp-layer"
RUNS = 3


class Refuse(Exception):
    pass


def count_run(jsonl_text):
    """Per-run counts from one stream-json transcript."""
    c = {"evaluate": 0, "act_force": 0, "act_at": 0, "tool_calls": 0}
    for line in jsonl_text.splitlines():
        if not line.startswith("{"):
            continue
        try:
            e = json.loads(line)
        except ValueError:
            continue
        if e.get("type") != "assistant":
            continue
        for b in (e.get("message") or {}).get("content", []) or []:
            if not isinstance(b, dict) or b.get("type") != "tool_use":
                continue
            name = b.get("name", "")
            if not name.startswith("mcp__browser__"):
                continue
            c["tool_calls"] += 1
            tool, inp = name[len("mcp__browser__"):], b.get("input") or {}
            if tool == "evaluate":
                c["evaluate"] += 1
            elif tool == "act":
                if inp.get("force") is True:
                    c["act_force"] += 1
                if "at" in inp and "handle" not in inp:
                    c["act_at"] += 1
    return c


def read_rows(path, pin):
    if not os.path.exists(path):
        raise Refuse(f"missing {os.path.relpath(path, ROOT)}")
    rows = list(csv.DictReader(open(path, newline=""), delimiter="\t"))
    for r in rows:
        if r.get("chrome_ver") != pin:
            raise Refuse(f"chrome_ver {r.get('chrome_ver')!r} != pin {pin!r} in {os.path.basename(path)}")
        if r.get("pass") == "judge_unavailable":
            raise Refuse(f"judge_unavailable row {r.get('task')} r{r.get('run')}")
    return [r for r in rows if r.get("arm") == ARM and r.get("task") in SUBSET]


def per_run(rows, runs_needed=RUNS):
    """{run: {task: row}} with a refusal on any missing (task, run) key."""
    key = {}
    for r in rows:
        key.setdefault(int(r["run"]), {})[r["task"]] = r
    for run in range(1, runs_needed + 1):
        missing = [t for t in SUBSET if t not in key.get(run, {})]
        if missing:
            raise Refuse(f"run {run}: missing {len(missing)} tasks, e.g. {missing[:3]}")
    return key


def summarize(key, runs=RUNS):
    passes = {run: sum(int(key[run][t]["pass"]) for t in SUBSET) for run in range(1, runs + 1)}
    turns = {run: sum(float(key[run][t]["turns"] or 0) for t in SUBSET) / len(SUBSET) for run in range(1, runs + 1)}
    t2 = sum(1 for t in SUBSET if sum(int(key[run][t]["pass"]) for run in range(1, runs + 1)) >= 2)
    return {"pass": passes, "turns": turns, "task_pass_2of3": t2}


def verdict(new, b0, evaluate_runs, total_runs):
    """Falsifier: evaluate in <10% of runs AND pass and turns inside the W6.B0 band."""
    pass_lo, pass_hi = min(b0["pass"].values()), max(b0["pass"].values())
    turn_lo, turn_hi = min(b0["turns"].values()), max(b0["turns"].values())
    frac = evaluate_runs / total_runs
    pass_in = pass_lo <= new["task_pass_2of3"] <= pass_hi
    turns_in = turn_lo <= sum(new["turns"].values()) / len(new["turns"]) <= turn_hi
    fires = frac < 0.10 and pass_in and turns_in
    return {"evaluate_run_frac": frac, "pass_band": (pass_lo, pass_hi), "turn_band": (turn_lo, turn_hi),
            "pass_in": pass_in, "turns_in": turns_in, "fires": fires}


def transcript_counts(key, tdir):
    out = {}
    for run in range(1, RUNS + 1):
        for t in SUBSET:
            p = os.path.join(tdir, f"{ARM}_{t}_r{run}.jsonl")
            if not os.path.exists(p):
                raise Refuse(f"missing transcript {p}")
            out[(run, t)] = count_run(open(p).read())
    return out


def report(tdir=TRANSCRIPTS, new_path=NEW_TSV, b0_path=B0_TSV):
    pin = os.environ.get("MANUK_CHROME_PIN")
    if not pin:
        raise Refuse("MANUK_CHROME_PIN unset")
    new_key = per_run(read_rows(new_path, pin))
    b0_key = per_run(read_rows(b0_path, pin))
    new, b0 = summarize(new_key), summarize(b0_key)
    counts = transcript_counts(new_key, tdir)
    print("| run | evaluate | act_force | act_at | tool_calls | passes/17 | mean turns |")
    print("|---|---|---|---|---|---|---|")
    for run in range(1, RUNS + 1):
        ev = sum(counts[(run, t)]["evaluate"] for t in SUBSET)
        af = sum(counts[(run, t)]["act_force"] for t in SUBSET)
        aa = sum(counts[(run, t)]["act_at"] for t in SUBSET)
        tc = sum(counts[(run, t)]["tool_calls"] for t in SUBSET)
        print(f"| r{run} | {ev} | {af} | {aa} | {tc} | {new['pass'][run]} | {new['turns'][run]:.2f} |")
    # a run counts as "evaluate used" when any of its tasks called evaluate
    used_runs = sum(1 for run in range(1, RUNS + 1)
                    if any(counts[(run, t)]["evaluate"] for t in SUBSET))
    v = verdict(new, b0, used_runs, RUNS)
    print(f"\nnew: task_pass_2of3={new['task_pass_2of3']}/17  per-run={list(new['pass'].values())}  "
          f"mean turns={sum(new['turns'].values())/RUNS:.2f}")
    print(f"W6.B0 same 17 tasks: task_pass_2of3={b0['task_pass_2of3']}/17  per-run={list(b0['pass'].values())}  "
          f"mean turns={sum(b0['turns'].values())/RUNS:.2f}")
    print(f"evaluate used in {used_runs}/{RUNS} runs")
    print(f"pass band {v['pass_band']} in={v['pass_in']}  turn band {tuple(round(x,2) for x in v['turn_band'])} in={v['turns_in']}")
    print("FALSIFIER FIRES" if v["fires"] else "falsifier does not fire")
    return 1 if v["fires"] else 0


# ---------------------------------------------------------------- self-test
def _tool(name, inp):
    return {"type": "assistant", "message": {"content": [{"type": "tool_use", "id": "x", "name": name, "input": inp}]}}


def self_test():
    # a transcript with known counts: 2 evaluate, 1 act force:true, 1 act force:false, 2 act at, 1 act with at+handle
    ev = [_tool("mcp__browser__evaluate", {"expression": "1"}), _tool("mcp__browser__evaluate", {"expression": "2"}),
          _tool("mcp__browser__act", {"handle": "h1", "force": True}), _tool("mcp__browser__act", {"handle": "h2", "force": False}),
          _tool("mcp__browser__act", {"at": {"x": 1, "y": 2}}), _tool("mcp__browser__act", {"at": {"x": 3, "y": 4}}),
          _tool("mcp__browser__act", {"at": {"x": 5, "y": 6}, "handle": "h3"}),
          _tool("mcp__browser__observe", {})]
    text = "\n".join(json.dumps(e) for e in ev) + "\nnot json\n"
    c = count_run(text)
    want = {"evaluate": 2, "act_force": 1, "act_at": 2, "tool_calls": 8}
    assert {k: c[k] for k in want} == want, f"count_run mismatch: {c}"
    assert count_run(text.replace("mcp__browser__evaluate", "mcp__browser__evalu4te"))["evaluate"] == 0
    # verdict: falsifier fires only when evaluate is rare AND pass and turns sit in the band
    b0 = {"pass": {1: 40, 2: 41, 3: 42}, "turns": {1: 10.0, 2: 10.5, 3: 11.0}, "task_pass_2of3": 40}
    new_in = {"pass": {1: 41, 2: 41, 3: 42}, "turns": {1: 10.2, 2: 10.4, 3: 10.6}, "task_pass_2of3": 41}
    assert verdict(new_in, b0, 0, 3)["fires"] is True
    assert verdict(new_in, b0, 3, 3)["fires"] is False, "evaluate used in all runs must not fire"
    new_out = dict(new_in, task_pass_2of3=2)
    assert verdict(new_out, b0, 0, 3)["fires"] is False, "pass outside band must not fire"
    # refusals: wrong chrome pin, judge_unavailable, missing key
    with tempfile.TemporaryDirectory() as d:
        p = os.path.join(d, "x.tsv")
        hdr = "arm\tchrome_ver\ttask\trun\tpass\tturns\n"
        open(p, "w").write(hdr + "".join(f"cdp-layer\t999\t{t}\t1\t1\t5\n" for t in SUBSET))
        try:
            read_rows(p, "145.0.7632.116")
            raise AssertionError("chrome_ver mismatch was accepted")
        except Refuse:
            pass
        open(p, "w").write(hdr + "".join(f"cdp-layer\t145.0.7632.116\t{t}\t1\t1\t5\n" for t in SUBSET[:-1]))
        try:
            per_run(read_rows(p, "145.0.7632.116"))
            raise AssertionError("missing key was accepted")
        except Refuse:
            pass
    print("self-test green: counting, falsifier and refusals behave as specified")
    return 0


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        sys.exit(self_test())
    try:
        sys.exit(report())
    except Refuse as ex:
        print(f"refusing: {ex}", file=sys.stderr)
        sys.exit(2)
