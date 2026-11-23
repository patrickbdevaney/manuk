#!/usr/bin/env python3
"""W5.G1.b.d.b.a.a.a.a.a: full-text scan of the failed runs' transcripts for refusal/no-effect vocabulary.

For every failed row of CAUSES (default: causes-pwfmt-v7r4.tsv) it reads the WHOLE transcript (no truncation) and reports
every hit of the rule vocabulary (docs/sprint/w5/G1.md, "Complete refusal-seen rule"):
  - result hits: tool results whose body matches w5g1_report.FAIL (failure-result vocabulary, line-wise anchors);
  - text hits: assistant text messages matching real_classify RX typed_fail (clause (g) vocabulary), final message flagged;
  - info hits: UNREACHABLE / OCCLUDED / read-only / is unknown: named by the rule but NOT counted as failures.
Each hit records whether it starts inside the first TRUNC characters of its tool result (the part a ~220-char labeller saw)
and the row's v7r4 class and hand label, so a hit missed by the labels is listed with its row id.

  w5g1_fulltext_scan.py [--causes F] [--labels F] [--runs target/real-runs] [--arm cdp-layer+pwfmt] [--trunc 220] [--out F]
  Exit 1 if a failed row has no transcript, or a hit the labels do not reflect is found AND --strict is given.
Only the banner line is removed before matching (the same BANNER as real_classify.parse); nothing else is cut.
"""
import argparse, csv, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import real_classify as rc  # parse(): full tool-result bodies, assistant texts, final answer
import w5g1_report as rep    # FAIL: the failure-result vocabulary the classifier uses

ROOT = os.path.join(HERE, "..", "..")
INFO = re.compile(r"UNREACHABLE|OCCLUDED|read-only|is unknown")


def scan_run(path, trunc):
    """Every hit in the whole transcript, plus the rule's view: a result hit is rule-counted iff it is an action-result
    failure AFTER the last successful action result (rule step 3); a text hit is rule-counted iff it is NOT the final answer
    message (clause (g) excludes it) and the run made a type/browser_type/browser_fill_form call (the code's own condition)."""
    _, final, results, texts, type_calls = rc.parse(path)
    last = max((i for i, (n, b) in enumerate(results) if n in rep.ACTS and b.strip() and not rep.FAIL.search(b)), default=-1)
    hits = []
    for i, (name, body) in enumerate(results):
        for m in rep.FAIL.finditer(body):
            hits.append(dict(kind="result", idx=i, tool=name, phrase=m.group(0), pos=m.start(), beyond=m.start() >= trunc,
                             final=False, rule=(i > last), code=(i > last)))
        for m in INFO.finditer(body):
            hits.append(dict(kind="info", idx=i, tool=name, phrase=m.group(0), pos=m.start(), beyond=m.start() >= trunc,
                             final=False, rule=False, code=False))
    for j, t in enumerate(texts):
        is_final = j == len(texts) - 1 and bool(final.strip())
        for m in rc.RX["typed_fail"].finditer(t):
            selferr = bool(rc.RX["selferr_text"].search(t))
            hits.append(dict(kind="text", idx=j, tool="-", phrase=m.group(0), pos=m.start(), beyond=False, final=is_final,
                             rule=(not is_final and type_calls > 0 and not selferr),
                             code=(type_calls > 0 and not selferr)))
    return hits, len(results), len(texts)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--causes", default=os.path.join(ROOT, "docs/sprint/w5/G1/causes-pwfmt-v7r4.tsv"))
    ap.add_argument("--labels", default=os.path.join(ROOT, "docs/sprint/w5/G1/hand-labels-mech-v7r4.tsv"))
    ap.add_argument("--runs", default=os.path.join(ROOT, "target/real-runs"))
    ap.add_argument("--arm", default="cdp-layer+pwfmt", help="runs-dir arm prefix")
    ap.add_argument("--trunc", type=int, default=220)
    ap.add_argument("--out", default="")
    a = ap.parse_args()
    rows = list(csv.DictReader(open(a.causes), delimiter="\t"))
    labels = {(r["arm"], r["task"], r["run"]): r["hand_label"] for r in csv.DictReader(open(a.labels), delimiter="\t")}
    out = ["arm\ttask\trun\tclass\thand_label\tkind\ttool_idx\ttool\tphrase\tpos\tbeyond_trunc\tfinal_msg\trule_counts\tcode_counts\tlabel_reflects"]
    missing, flagged, code_only, rows_with_hits, n_failed = [], [], [], 0, 0
    for r in rows:
        if r["class"] not in ("refusal-seen", "stop-by-choice"):
            continue
        n_failed += 1
        tp = os.path.join(a.runs, f"{a.arm}_{r['task']}_r{r['run']}", "transcript.jsonl")
        if not os.path.exists(tp):
            missing.append(f"{r['task']} r{r['run']}")
            continue
        hand = labels.get((r["arm"], r["task"], r["run"]), "-")
        hits, _, _ = scan_run(tp, a.trunc)
        ruled = [h for h in hits if h["rule"]]
        if ruled:
            rows_with_hits += 1
        for h in hits:
            reflects = "yes" if hand == "refusal-seen" else "no"
            tag = f"{r['task']} r{r['run']} {h['kind']} idx {h['idx']} {h['phrase']!r}"
            if h["rule"] and reflects == "no":
                flagged.append(tag + (" beyond-trunc" if h["beyond"] else ""))
            if h["code"] and not h["rule"] and h["kind"] != "info":
                code_only.append(tag + (" [final msg]" if h["final"] else ""))
            phrase = h["phrase"].replace("\n", "\\n")
            out.append("\t".join(map(str, [r["arm"], r["task"], r["run"], r["class"], hand, h["kind"], h["idx"], h["tool"],
                                           phrase, h["pos"], int(h["beyond"]), int(h["final"]), int(h["rule"]), int(h["code"]),
                                           reflects])))
    if a.out:
        open(a.out, "w").write("\n".join(out) + "\n")
    print(f"wrote {a.out}: {len(out)-1} hit rows" if a.out else f"{len(out)-1} hit rows (no --out)")
    print(f"failed rows scanned: {n_failed}; rows with >=1 counted hit: {rows_with_hits}; missing transcripts: {len(missing)}")
    for m in missing:
        print("MISSING", m)
    print(f"rule-counted hits on rows whose hand label is not refusal-seen (MISSED by labels): {len(flagged)}")
    for f in flagged:
        print("MISSED", f)
    print(f"code-counted but not rule-counted hits (G1.md disagreement): {len(code_only)}")
    for f in code_only:
        print("CODE-ONLY", f)
    sys.exit(1 if missing else 0)


if __name__ == "__main__":
    main()
