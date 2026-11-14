# RG.S1.a.a.a.a.a.a.a.c.a.a: per run, turn index of first typed 'San Francisco, CA' vs first tool_result with a result count
# usage: python3 -I S1aaaaaaacaa-order.py <jsonl>...
import json, sys, re
CNT = re.compile(r'(\d[\d,]*)\s+results?\b', re.I)
for f in sys.argv[1:]:
    turn = 0; use = {}; first_ca = first_cnt = first_cnt_val = first_any = None; typed_after_cnt = 0
    for line in open(f, encoding="utf-8", errors="replace"):
        try: o = json.loads(line)
        except Exception: continue
        msg = o.get("message") or {}
        if not isinstance(msg, dict): continue
        cs = msg.get("content")
        if not isinstance(cs, list): continue
        for c in cs:
            if not isinstance(c, dict): continue
            if c.get("type") == "tool_use":
                turn += 1
                v = (c.get("input") or {}).get("text")
                if isinstance(v, str) and "san francisco" in v.lower():
                    if first_any is None: first_any = (turn, v)
                    if v.strip() == "San Francisco, CA" and first_ca is None: first_ca = turn
            elif c.get("type") == "tool_result" and first_cnt is None:
                t = c.get("content")
                if isinstance(t, list): t = " ".join(x.get("text", "") for x in t if isinstance(x, dict))
                m = CNT.search(t or "")
                if m: first_cnt, first_cnt_val = turn, m.group(1)
    print(f.split('/')[-1], "first_SF_type", first_any, "first_typed_CA_turn", first_ca, "first_count_turn", first_cnt, "count", first_cnt_val, "total_turns", turn, sep="\t")
