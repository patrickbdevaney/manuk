#!/usr/bin/env python3
"""W5.G1: why cdp-layer trails Playwright on the same Chrome. Per failed run, one of five agent-side classes (perceive,
target, act, tool-choice, turn-limit), from the stream-json transcript; then the A/B table for the 10 REAL tasks that
Playwright passes and cdp-layer fails.

  w5g1_report.py [--runs-dir target/real-runs] [--dir docs/sprint/w5/G1] [--mutate drop-act]

Inputs: DIR/<arm>.tsv for arm in cdp-layer (a: current observation) and cdp-layer+pwfmt (b: Playwright-shaped snapshot,
real_run.pw_snapshot); transcripts RUNS/<arm>_<task>_r<run>/transcript.jsonl. Output: DIR/causes.tsv, DIR/table.tsv, and
the table on stdout. Exit 1 if a failed run gets no class, a (arm,task) key is not 3 runs, or a run has no transcript.
Precedence (first match wins): turn-limit > tool-choice > act > target > perceive. Classes are read from the agent's side:
`act` marks a browser-side refusal or no-effect the agent could see in a result, the rest is the model's choice.
"""
import argparse, collections, csv, json, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import real_classify as rc  # parse() and features(): the W4.A0 reader, reused unchanged

ROOT = os.path.join(HERE, "..", "..")
ACTS = {"act", "type", "fill_form", "browser_click", "browser_type", "browser_fill_form", "browser_select_option",
        "browser_press_key", "click", "press", "select"}
ARMS = ["cdp-layer", "cdp-layer+pwfmt"]
CLASSES = ["perceive", "target", "act", "tool-choice", "turn-limit"]


def action_calls(path):
    n = 0
    for line in open(path, encoding="utf-8", errors="replace"):
        try:
            e = json.loads(line)
        except ValueError:
            continue
        if e.get("type") == "assistant":
            n += sum(1 for b in e["message"].get("content", []) if b.get("type") == "tool_use" and b["name"].split("__")[-1] in ACTS)
    return n


def classify(path, mutate=None):
    f = rc.features(path)
    if f["is_error"]:
        return "turn-limit"
    if action_calls(path) == 0:
        return "tool-choice"
    act = f["nochange"] >= 2 or f["stale"] >= 2 or f["unsup"] or f["enter_gap"] or (f["typed_fail_text"] and f["type_calls"]) or f["timeout"]
    if act:
        return None if mutate == "drop-act" else "act"  # mutant: the act rule is gone, so its runs stay unclassified
    if f["strict"] or f["ambig_text"] or f["occl"] or f["dialog_text"]:
        return "target"
    return "perceive"


def load(dirpath, arm):
    rows = list(csv.DictReader(open(os.path.join(dirpath, f"{arm}.tsv")), delimiter="\t"))
    return {(r["task"], int(r["run"])): r for r in rows}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--runs-dir", default=os.path.join(ROOT, "target/real-runs"))
    ap.add_argument("--dir", default=os.path.join(ROOT, "docs/sprint/w5/G1"))
    ap.add_argument("--mutate", choices=["drop-act"])
    a = ap.parse_args()
    bad, causes, rows = [], [], {}
    for arm in ARMS:
        rows[arm] = load(a.dir, arm)
        keys = collections.Counter(k[0] for k in rows[arm])
        for task, n in keys.items():
            if n != 3:
                bad.append(f"{arm} {task}: {n} runs, not 3")
        for (task, run), r in sorted(rows[arm].items()):
            p = os.path.join(a.runs_dir, f"{arm}_{task}_r{run}", "transcript.jsonl")
            if not os.path.exists(p):
                bad.append(f"{arm} {task} r{run}: no transcript")
                continue
            if r["pass"] == "0":
                c = classify(p, a.mutate)
                if c not in CLASSES:
                    bad.append(f"{arm} {task} r{run}: unclassified")
                causes.append((arm, task, run, c))
    with open(os.path.join(a.dir, "causes.tsv"), "w") as fh:
        fh.write("arm\ttask\trun\tclass\n" + "".join("\t".join(map(str, c)) + "\n" for c in causes))
    tasks = sorted({t for arm in ARMS for (t, _) in rows[arm]})
    lines = ["task\t" + "\t".join(f"{arm} (runs pass / 3)" for arm in ARMS)]
    for t in tasks:
        cells = []
        for arm in ARMS:
            ps = [int(rows[arm][(t, r)]["pass"]) for r in (1, 2, 3) if (t, r) in rows[arm]]
            cells.append(f"{sum(ps)}/{len(ps)}")
        lines.append(t + "\t" + "\t".join(cells))
    with open(os.path.join(a.dir, "table.tsv"), "w") as fh:
        fh.write("\n".join(lines) + "\n")
    print("\n".join(lines))
    for arm in ARMS:
        per_run = [sum(int(rows[arm][(t, r)]["pass"]) for t in tasks if (t, r) in rows[arm]) for r in (1, 2, 3)]
        task_2of3 = sum(1 for t in tasks if sum(int(rows[arm][(t, r)]["pass"]) for r in (1, 2, 3) if (t, r) in rows[arm]) >= 2)
        cnt = collections.Counter(c for (ar, _, _, c) in causes if ar == arm)
        print(f"{arm}: pass per run {per_run} (band {min(per_run)}-{max(per_run)}), tasks passing >=2/3: {task_2of3}/{len(tasks)}; "
              f"failed runs by class: " + ", ".join(f"{k} {cnt[k]}" for k in CLASSES))
    if bad:
        print("GATE FAIL:\n  " + "\n  ".join(bad))
        sys.exit(1)
    print("GATE PASS")


if __name__ == "__main__":
    main()
