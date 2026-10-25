#!/usr/bin/env python3
"""NET.REACH.1: build the outbound-reach table (url, chrome, manuk) and fail on any
URL that Chrome loads and manuk does not. Usage: reach-table.py RAW_LOG [--retest url=OK ...] OUT.tsv"""
import re, sys, os
ver = os.environ.get("MANUK_CHROME_PIN") or sys.exit("MANUK_CHROME_PIN unset")
raw, out, retest = sys.argv[1], sys.argv[-1], dict(a.split("=") for a in sys.argv[2:-1] if "=" in a)
d = {}
for l in open(raw):
    m = re.match(r"run1 (\w+)\s+(\w+)\s+([\d.]+)s.*?(https?://\S+)", l)
    if m: d.setdefault(m.group(4), {})[m.group(1)] = m.group(2)
rows, gaps = [], []
for u, v in sorted(d.items()):
    c, m = v.get("chrome", "?"), v.get("manuk", "?")
    re_ = retest.get(u, "")
    if c == "OK" and m != "OK" and re_ != "OK": gaps.append(u)
    rows.append((u, c, m, re_, ver))
with open(out, "w") as f:
    f.write("url\tchrome\tmanuk\tmanuk_retest\tchrome_ver\n")
    for r in rows: f.write("\t".join(r) + "\n")
print(f"{len(rows)} urls, {len(gaps)} reach gaps")
for g in gaps: print("GAP", g)
sys.exit(1 if gaps else 0)
