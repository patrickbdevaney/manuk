# RG.S1.a.a.a.a.a.a.a.c: lists the text typed into the page by each zilloft-2 transcript (usage: python3 -I S1aaaaaaac-typed.py "<glob>")
import json, sys, glob
for f in sorted(glob.glob(sys.argv[1])):
    typed = []
    for line in open(f, encoding="utf-8", errors="replace"):
        try: o = json.loads(line)
        except Exception: continue
        msg = o.get("message") or {}
        for c in (msg.get("content") or []) if isinstance(msg, dict) else []:
            if isinstance(c, dict) and c.get("type") == "tool_use":
                inp = c.get("input") or {}
                for k in ("text", "value", "keys", "input"):
                    v = inp.get(k) if isinstance(inp, dict) else None
                    if isinstance(v, str) and len(v) < 60:
                        typed.append(f"{c.get('name')}:{v!r}")
    print(f.split('/')[-1], typed[:12])
