#!/usr/bin/env python3
"""W5.T2.a.a: classify live act results by shape; report median tokens (chars/4) per shape."""
import glob, json, os, statistics, sys
d = sys.argv[1] if len(sys.argv) > 1 else os.path.expanduser("~/manuk-sprint-transcripts/w5t2a")
def shape(inp, t):
    if "steps" in inp: return "multi-step"
    if "\nnavigated: " in t or t.startswith("navigated: "): return "navigated"
    if t.startswith("ok: "): return "concise-ok"
    if "NO EFFECT" in t: return "no-effect"
    return "other-full-diff"
by = {}
for f in sorted(glob.glob(d + "/manuk_*.jsonl")):
    uses = {}
    for line in open(f):
        try: e = json.loads(line)
        except Exception: continue
        c = (e.get("message") or {}).get("content")
        if not isinstance(c, list): continue
        for b in c:
            if b.get("type") == "tool_use" and b.get("name") == "mcp__browser__act":
                uses[b["id"]] = b.get("input", {})
            elif b.get("type") == "tool_result" and b.get("tool_use_id") in uses:
                r = b.get("content")
                t = "".join(x.get("text", "") for x in r) if isinstance(r, list) else str(r)
                by.setdefault(shape(uses[b["tool_use_id"]], t), []).append(len(t) / 4)
tot = [x for v in by.values() for x in v]
print("shape\tn\tmedian_tokens\tmean_tokens\tsum_share")
for k, v in sorted(by.items(), key=lambda kv: -sum(kv[1])):
    print(f"{k}\t{len(v)}\t{statistics.median(v):.1f}\t{statistics.mean(v):.1f}\t{sum(v)/sum(tot):.2f}")
print(f"ALL\t{len(tot)}\t{statistics.median(tot):.1f}\t{statistics.mean(tot):.1f}\t1.00")
