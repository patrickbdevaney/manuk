import csv, glob, json, sys
d = sys.argv[1:]
if not d: sys.exit(1)
for dd in d:
    rows = list(csv.DictReader(open(dd + "/rows.tsv"), delimiter="\t"))
    if not rows: sys.exit(1)
    per = {}
    for r in rows:
        p = per.setdefault(r["run"], [0, 0]); p[0] += int(r["pass"] or 0); p[1] += int(r["input_tokens"]) + int(r["output_tokens"])
    ts = 0
    for f in glob.glob(dd + "/transcripts/*.jsonl"):
        ts += open(f).read().count('"name":"ToolSearch"')
    print(dd, {k: (v[0], v[1]) for k, v in sorted(per.items())}, "ToolSearch calls", ts, "chrome", {r["chrome_ver"] for r in rows})
