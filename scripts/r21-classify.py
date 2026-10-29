#!/usr/bin/env python3
"""R2.1 failure classifier. Reads the manuk-arm rows of docs/sprint/SCOREBOARD.tsv (suite hash prefix SUITE,
slice own) that did not pass, tags each from its `reasons` + `answer` text with one of
perceive/target/act/script/render/net/botwall (+ a cause id), writes docs/sprint/FAILURES.tsv.

Cause rules are ordered; the first symptom regex that matches wins. The symptoms were tied to engine causes by
driving manuk-mcp against the fixtures (docs/sprint/r21/probe.py, docs/sprint/R2.1-FAILURES.md).
Gate (exit 1 on failure): every failed row gets a class (100%) and the classifier agrees with
docs/sprint/r21/hand-labels.tsv on >= 8 of 10 rows.
  --mutate drop-post   delete the POST rule: coverage or agreement must go red.
"""
import csv, os, re, sys
HERE = os.path.dirname(os.path.abspath(__file__)); REPO = os.path.abspath(os.path.join(HERE, ".."))
SB = os.path.join(REPO, "docs/sprint/SCOREBOARD.tsv"); OUT = os.path.join(REPO, "docs/sprint/FAILURES.tsv")
HAND = os.path.join(REPO, "docs/sprint/r21/hand-labels.tsv")
SUITE = "69903c"; CHROME = os.environ.get("MANUK_CHROME_PIN", "")
RULES = [  # (cause, class, symptom regex over reasons+answer)
    ("agent-post-unsupported", "act", r"(method=post|won.t submit|refus\w* (to )?(submit|the (sign|click))|submission attempts failed|fails with .fo|blocked at step|could not complete the order|add to cart.{0,40}fail|couldn.t sign in|couldn.t complete the sign-in|could not get to the dashboard)"),
    ("async-never-pumped", "script", r"(never saw it load more|only ever shows items|did not dismiss|Reject all.{0,80}no change|banner is still)"),
    ("cssom-style-write-dead", "script", r"(dialog never opened|subscribe dialog|tab did not switch|clicking the shipping tab|did not switch the panel|stayed on the description|panel)"),
    ("document-write-head", "script", r"(second batch never loaded|Steve Martin|only the first 10)"),
    ("model-miscount", "target", r"fourth in the listing|counted from the top"),
    # R2.1.6: F08 answers "Item 45 is amber" (correct value) but the verifier wants the literal "Item 45: amber";
    # a verifier string-format mismatch, not an engine fault. Suite NOT changed (SUCCESS.md); see V1.4.md.
    ("verifier-literal-mismatch", "judge-error", r"missing 'Item 45: amber'"),
]
def classify(r, rules):
    t = r["reasons"] + " " + r["answer"]
    for cause, cls, rx in rules:
        if re.search(rx, t, re.I): return cause, cls, rx
    return "", "", ""
def main():
    rules = [x for x in RULES if not (len(sys.argv) > 2 and sys.argv[1] == "--mutate" and x[0] == "agent-post-unsupported")]
    rows = [r for r in csv.DictReader(open(SB), delimiter="\t")
            if r["arm"] == "manuk" and r["slice"] == "own" and r["suite_hash"].startswith(SUITE) and r["pass"] != "1"]
    out, miss = [], 0
    for r in rows:
        cause, cls, rx = classify(r, rules)
        miss += not cls
        out.append((r["task"], r["run"], cls or "UNCLASSIFIED", cause, rx, r["turns"], r["reasons"][:80]))
    with open(OUT, "w") as f:
        f.write("task\trun\tclass\tcause\tsymptom_rule\tturns\treasons\n")
        for o in out: f.write("\t".join(o) + "\n")
    hand = list(csv.DictReader(open(HAND), delimiter="\t"))
    got = {(o[0], o[1]): o[2] for o in out}
    agree = sum(got.get((h["task"], h["run"])) == h["class"] for h in hand)
    print(f"failed rows {len(rows)} unclassified {miss}; hand agreement {agree}/{len(hand)}; chrome_ver={CHROME}")
    from collections import Counter
    for k, v in Counter((o[2], o[3]) for o in out).most_common(): print(v, k)
    ok = miss == 0 and agree >= 8 and len(hand) == 10
    print("GATE", "PASS" if ok else "FAIL"); sys.exit(0 if ok else 1)
main()
