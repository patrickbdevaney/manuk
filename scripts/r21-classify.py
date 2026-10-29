#!/usr/bin/env python3
"""R2.1 failure classifier. Reads the manuk-arm rows of docs/sprint/SCOREBOARD.tsv (suite hash prefix SUITE,
slice own) that did not pass, tags each from its `reasons` + `answer` text with one of
perceive/target/act/script/render/net/botwall (+ a cause id), writes docs/sprint/FAILURES.tsv.

Cause rules are ordered; the first symptom regex that matches wins. The symptoms were tied to engine causes by
driving manuk-mcp against the fixtures (docs/sprint/r21/probe.py, docs/sprint/R2.1-FAILURES.md).
Gate (exit 1 on failure): every failed row gets a class (100%) and the classifier agrees with
docs/sprint/r21/hand-labels.tsv on >= 8 of 10 rows.
  --mutate drop-post   delete the POST rule: coverage or agreement must go red.
  --mutate drop-js-notice  delete the Wolfram no-result rule: external rows must go unclassified.

EXT.RUN.b: the full-suite hash (061ee22b3f67, slice external) is classified too, into FAILURES-EXTERNAL.tsv, so
FAILURES.tsv (own suite) is byte-identical to before. External rows use RULES + EXT_RULES. SCOREBOARD answers are
truncated to 300 chars, so rules see only that prefix; the transcripts hold the full answers.
"""
import csv, os, re, sys
HERE = os.path.dirname(os.path.abspath(__file__)); REPO = os.path.abspath(os.path.join(HERE, ".."))
SB = os.path.join(REPO, "docs/sprint/SCOREBOARD.tsv"); OUT = os.path.join(REPO, "docs/sprint/FAILURES.tsv")
HAND = os.path.join(REPO, "docs/sprint/r21/hand-labels.tsv")
SUITE = "69903c"; EXT_SUITE = "061ee22b3f67"; CHROME = os.environ.get("MANUK_CHROME_PIN", "")
EXT_OUT = os.path.join(REPO, "docs/sprint/FAILURES-EXTERNAL.tsv")
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
EXT_RULES = [  # EXT.RUN.b: external slice only; symptoms from the 300-char SCOREBOARD answer
    ("wolfram-no-result-pods", "script", r"(couldn.t|could not) get Wolfram|doesn.t run without JavaScript|need JavaScript"),
    ("verifier-literal-mismatch", "judge-error", r"missing '27 Mar 2023'"),
    ("translation-not-reached", "target", r"could not read the (Chinese|French)|returned no page content|could not locate a search box"),
    ("blank-page-render", "render", r"rendered a blank"),
]
DROP = {"drop-post": "agent-post-unsupported", "drop-js-notice": "wolfram-no-result-pods"}
def classify(r, rules):
    t = r["reasons"] + " " + r["answer"]
    for cause, cls, rx in rules:
        if re.search(rx, t, re.I): return cause, cls, rx
    return "", "", ""
def main():
    mutate = sys.argv[2] if len(sys.argv) > 2 and sys.argv[1] == "--mutate" else None
    rules = [x for x in RULES if x[0] != DROP.get(mutate)]
    ext_rules = [x for x in rules + EXT_RULES if x[0] != DROP.get(mutate)]
    rows = [r for r in csv.DictReader(open(SB), delimiter="\t")
            if r["arm"] == "manuk" and r["slice"] == "own" and r["suite_hash"].startswith(SUITE) and r["pass"] != "1"]
    ext_rows = [r for r in csv.DictReader(open(SB), delimiter="\t")
                if r["arm"] == "manuk" and r["slice"] == "external" and r["suite_hash"].startswith(EXT_SUITE) and r["pass"] != "1"]
    out, miss = [], 0
    for r in rows:
        cause, cls, rx = classify(r, rules)
        miss += not cls
        out.append((r["task"], r["run"], cls or "UNCLASSIFIED", cause, rx, r["turns"], r["reasons"][:80]))
    ext_out, ext_miss = [], 0
    for r in ext_rows:
        cause, cls, rx = classify(r, ext_rules)
        ext_miss += not cls
        ext_out.append((r["task"], r["run"], cls or "UNCLASSIFIED", cause, rx, r["turns"], r["reasons"][:80]))
    if mutate is None:  # a mutant run must never overwrite the committed tables
        with open(OUT, "w") as f:
            f.write("task\trun\tclass\tcause\tsymptom_rule\tturns\treasons\n")
            for o in out: f.write("\t".join(o) + "\n")
        with open(EXT_OUT, "w") as f:
            f.write(f"suite_hash_prefix\t{EXT_SUITE}\tchrome_ver\t{CHROME}\n")
            f.write("task\trun\tclass\tcause\tsymptom_rule\tturns\treasons\n")
            for o in ext_out: f.write("\t".join(o) + "\n")
    hand = list(csv.DictReader(open(HAND), delimiter="\t"))
    got = {(o[0], o[1]): o[2] for o in out}
    agree = sum(got.get((h["task"], h["run"])) == h["class"] for h in hand)
    print(f"failed rows {len(rows)} unclassified {miss}; hand agreement {agree}/{len(hand)}; chrome_ver={CHROME}")
    print(f"external failed rows {len(ext_rows)} unclassified {ext_miss}; chrome_ver={CHROME}")
    from collections import Counter
    for k, v in Counter((o[2], o[3]) for o in out).most_common(): print(v, k)
    for k, v in Counter((o[2], o[3]) for o in ext_out).most_common(): print("ext", v, k)
    ok = miss == 0 and agree >= 8 and len(hand) == 10 and ext_miss == 0 and len(ext_rows) == 11
    print("GATE", "PASS" if ok else "FAIL"); sys.exit(0 if ok else 1)
main()
