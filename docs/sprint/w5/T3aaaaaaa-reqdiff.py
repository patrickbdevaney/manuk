#!/usr/bin/env python3
"""W5.T3.a.a.a.a.a.a.a: were the requests of the ToolSearch runs and the no-ToolSearch runs the same?

The claude child writes every API request it sends into ~/.claude/projects/<cwd-slug>/<session>.jsonl
(api-request-shape: system + tools; api-request: params). This script groups the sessions of two or more
(directory, time window) pairs and reports, per group, how many runs called ToolSearch and whether the requests
were identical after the per-run working directory path is normalised out.

Refuses (exit 2) when a group has no sessions, because an empty group proves nothing.
The --self-test mode edits one tool description in a copy of one request and requires the diff to fire (rc=1).

Usage: python3 docs/sprint/w5/T3aaaaaaa-reqdiff.py --group=DIR@HH:MM:SS-HH:MM:SS [--group=...] [--out TSV]
       python3 docs/sprint/w5/T3aaaaaaa-reqdiff.py --self-test=DIR@HH:MM:SS-HH:MM:SS  (the = form: a DIR starts with a dash)
"""
import argparse, glob, hashlib, json, os, re, sys, time

PROJECTS = os.path.expanduser("~/.claude/projects")
COLS = ["group", "runs", "runs_with_toolsearch", "distinct_shapes", "distinct_params", "distinct_tool_schemas",
        "identical_to_first_group"]


class Refused(Exception):
    pass


def parse_group(spec):
    if "@" not in spec or spec.count("@") != 1:
        raise Refused(f"bad group spec {spec!r}; want DIR@HH:MM:SS-HH:MM:SS")
    d, win = spec.split("@")
    lo, hi = win.split("-")
    return d, lo, hi


def hm(path):
    return time.strftime("%H:%M:%S", time.localtime(os.path.getmtime(path)))


def norm(obj):
    # the per-run working directory is the only thing that differs between runs of the same task
    s = json.dumps(obj, sort_keys=True)
    return re.sub(r"w5(t3aa|t3aaaaaa|envprobe)-cwd-[A-Za-z0-9_-]+", "CWD", s)


def sha(s):
    return hashlib.sha256(s.encode()).hexdigest()[:16]


def session_facts(path):
    shape = params = None
    toolsearch = 0
    for line in open(path, encoding="utf-8", errors="replace"):
        try:
            e = json.loads(line)
        except ValueError:
            continue
        if e.get("type") == "api-request-shape" and shape is None:
            shape = e.get("shape")
        if e.get("type") == "api-request" and params is None:
            params = e.get("params")
        if e.get("type") == "assistant":
            for b in (e.get("message") or {}).get("content") or []:
                if isinstance(b, dict) and b.get("type") == "tool_use" and b.get("name") == "ToolSearch":
                    toolsearch += 1
    if shape is None or params is None:
        raise Refused(f"{os.path.basename(path)} has no api-request-shape/api-request record")
    return shape, params, toolsearch


def group_facts(spec):
    d, lo, hi = parse_group(spec)
    files = [p for p in glob.glob(os.path.join(PROJECTS, d, "*.jsonl")) if lo <= hm(p) <= hi]
    if not files:
        raise Refused(f"no sessions in {d} between {lo} and {hi}")
    facts = [session_facts(p) for p in sorted(files)]
    shapes = {sha(norm(s["system"])) for s, _, _ in facts}
    params = {sha(norm(p)) for _, p, _ in facts}
    tools = {sha(norm(t)) for s, _, _ in facts for t in [s.get("tools")]}
    ts = sum(1 for _, _, n in facts if n > 0)
    return {"group": spec, "runs": len(facts), "runs_with_toolsearch": ts, "distinct_shapes": len(shapes),
            "distinct_params": len(params), "distinct_tool_schemas": len(tools),
            "tools_digest": sha(norm(facts[0][0].get("tools"))), "system_digest": sha(norm(facts[0][0]["system"])),
            "params_digest": sha(norm(facts[0][1]))}


def self_test(spec):
    # mutate one tool description in a copy of a real request; the comparison must report a difference
    d, lo, hi = parse_group(spec)
    files = [p for p in glob.glob(os.path.join(PROJECTS, d, "*.jsonl")) if lo <= hm(p) <= hi]
    if not files:
        raise Refused(f"no sessions in {d} between {lo} and {hi}")
    shape, _, _ = session_facts(sorted(files)[0])
    tools = json.loads(json.dumps(shape["tools"]))
    tools[0]["description"] = tools[0].get("description", "") + " (mutated)"
    if sha(norm(shape["tools"])) == sha(norm(tools)):
        raise Refused("mutation did not change the digest: the comparison is blind")
    print(f"self-test: mutated tool {tools[0].get('name')} changes the digest; diff fires")
    return 1


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--group", action="append", default=[])
    ap.add_argument("--out")
    ap.add_argument("--self-test")
    a = ap.parse_args()
    try:
        if a.self_test:
            return self_test(a.self_test)
        if len(a.group) < 2:
            raise Refused("need at least two --group specs to compare")
        rows = [group_facts(g) for g in a.group]
    except Refused as e:
        print(f"refused: {e}")
        return 2
    first = rows[0]
    for r in rows:
        r["identical_to_first_group"] = int(
            r["distinct_shapes"] == 1 and r["distinct_params"] == 1 and r["distinct_tool_schemas"] == 1
            and r["tools_digest"] == first["tools_digest"] and r["system_digest"] == first["system_digest"]
            and r["params_digest"] == first["params_digest"])
        print(f"{r['group']}: runs={r['runs']} runs_with_toolsearch={r['runs_with_toolsearch']} "
              f"identical_to_first={r['identical_to_first_group']} tools={r['tools_digest']} "
              f"system={r['system_digest']} params={r['params_digest']}")
    if a.out:
        with open(a.out, "w") as f:
            f.write("\t".join(COLS) + "\n")
            for r in rows:
                f.write("\t".join(str(r[c]) for c in COLS) + "\n")
        print(f"written={a.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
