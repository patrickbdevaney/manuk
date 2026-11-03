# Sample 40 of the 142 construction-vs-render disagreement rows (seed 315), dump page HTML + payload segments.
import json, random, sys, csv, os
sys.path.insert(0, "/home/patrickd/manuk/docs/sprint/inj-census")
import census as C
rows = [l.rstrip("\n").split("\t") for l in open("/home/patrickd/manuk/docs/sprint/inj-census/labels.tsv")][1:]
dis = [r for r in rows if r[1] == "yes" and r[2] not in ("unlocated", "error") and r[2] != r[3]]
print("disagreement population:", len(dis))
random.seed(315)
samp = random.sample(dis, 40)
sigs = {g for g, v in json.load(open("/home/patrickd/manuk/docs/sprint/inj-census/sigs.json")).items() if v >= C.RUN_DF}
t = json.load(open("/tmp/bs/test.json"))
out = []
for r in samp:
    i = int(r[0].split("-")[1])
    html = t[i]["content"]
    pl = C.payload(html, sigs)
    segs = []
    for k, g in pl:
        # recover the segment text for this kind (first segment with these grams)
        for kk, v in C.segments(html):
            if kk == k and (set(g) & C.grams(v)):
                segs.append({"kind": k, "text": v.strip()[:400], "grams": g[:3]})
    with open(f"/tmp/w315b/pages/{r[0]}.html", "w") as f: f.write(html)
    out.append({"sample_id": r[0], "construction": r[2], "manuk_render": r[3], "segments": segs})
json.dump(out, open("/tmp/w315b/sample.json", "w"), indent=1)
from collections import Counter
print("sampled pairs (construction|render):", Counter(f"{o['construction']}|{o['manuk_render']}" for o in out))
