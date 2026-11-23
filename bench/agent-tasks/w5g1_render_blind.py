#!/usr/bin/env python3
"""W5.G1.b.d.b.a: render stream-json transcripts for a blind labeller: calls, full results (paired by tool_use_id), assistant texts, final answer.
  python3 -I w5g1_render_blind.py SRC_DIR_OF_JSONL DST_DIR"""
import json, sys, os
src, dst = sys.argv[1], sys.argv[2]
for f in sorted(os.listdir(src)):
    out = []; n = 0; final = None; ids = {}
    for l in open(os.path.join(src, f)):
        o = json.loads(l)
        if o.get("type") == "result": final = o.get("result")
        m = o.get("message")
        if not isinstance(m, dict): continue
        for c in m.get("content", []) if isinstance(m.get("content"), list) else []:
            if not isinstance(c, dict): continue
            t = c.get("type")
            if t == "text" and o["type"] == "assistant":
                out.append(f"=== ASSISTANT TEXT ===\n{c['text']}")
            elif t == "tool_use":
                n += 1; ids[c.get("id")] = (n, c['name'].replace('mcp__browser__','')); out.append(f"=== CALL {n}: {ids[c.get('id')][1]} ===\n{json.dumps(c.get('input'))}")
            elif t == "tool_result":
                cc = c.get("content")
                if isinstance(cc, list): cc = "\n".join(x.get("text", "") for x in cc if isinstance(x, dict))
                k, nm = ids.get(c.get("tool_use_id"), ("?", "?")); out.append(f"=== RESULT of CALL {k} ({nm}){' (is_error)' if c.get('is_error') else ''} ===\n{cc}")
    out.append(f"=== FINAL ANSWER (run result) ===\n{final}")
    open(os.path.join(dst, f.replace(".jsonl", ".txt")), "w").write("\n".join(out) + "\n")
