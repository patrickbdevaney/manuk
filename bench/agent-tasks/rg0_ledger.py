#!/usr/bin/env python3
"""RG.0: REAL gap ledger. Per GAP task (playwright passes >=2 of 3, manuk <=1 of 3, W5.B1 rows), put the manuk
transcript beside a Playwright transcript, find the first manuk step whose result carries a failure signal, and
classify the mechanism: engine-render | observation | action | navigation-settle | agent.

  rg0_ledger.py [--tr DIR] [--out docs/sprint/rg] [--mutate drop-task]   exit 0 = GATE PASS, 1 = GATE FAIL
"""
import argparse, collections, csv, json, os, re, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import real_classify as rc

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")
W5 = os.path.join(ROOT, "docs/sprint/w5")
ENGINE = {"dashdish-6", "fly-unified-11", "gocalendar-2", "gocalendar-3", "gocalendar-4", "staynb-2", "staynb-4", "udriver-7"}
AHEAD = ["gomail-8", "networkin-2", "networkin-3", "networkin-4", "opendining-2", "opendining-7", "opendining-8"]
# (mechanism, signal name, regex over a tool result), first match in transcript order wins
SIGNALS = [
    ("action", "stale-handle", rc.RX["stale"]),
    ("engine-render", "occluded-hit-test", rc.RX["occl"]),
    ("action", "unsupported-action", rc.RX["unsup"]),
    ("action", "enter-without-form", rc.RX["enter_gap"]),
    ("engine-render", "js-or-render-error", re.compile(r"Uncaught|TypeError|ReferenceError|is not a function|not implemented|blank page|empty page|0 elements", re.I)),
    ("navigation-settle", "act-no-effect-url-unchanged", rc.RX["nochange"]),
    ("action", "typed-not-registered", rc.RX["typed_fail"]),
    ("observation", "target-absent-from-observe", re.compile(r"^no match for")),
]
# page-content tools return page text, so only structural (non-prose) signals are read from them
PAGE_TEXT_TOOLS = {"navigate", "extract", "observe", "page_data", "screenshot", "recall"}
PROSE_SIGNALS = {"js-or-render-error", "typed-not-registered"}
# the agent's own final answer, read only when no step signal fired
ANSWER_SIGNALS = [
    ("engine-render", "answer-reports-overlay", re.compile(r"modal|overlay|covers|blocks everything|blocked by", re.I)),
    ("navigation-settle", "answer-reports-page-never-ready", re.compile(r"never (reached|loaded)|did not load|didn'?t load|no results|never appeared|stayed (blank|loading)", re.I)),
]


def rows(arm):
    return list(csv.DictReader(open(os.path.join(W5, f"B1-real-{arm}.tsv")), delimiter="\t"))


def gap_tasks():
    p = {}
    for a in ("manuk", "playwright"):
        for r in rows(a):
            p.setdefault(a, collections.defaultdict(int))[r["task"]] += int(r["pass"])
    gap = sorted(t for t in p["playwright"] if p["playwright"][t] >= 2 and p["manuk"][t] <= 1)
    return gap, p


def steps(path):
    """-> [(n, tool, input, result_text)] in call order, plus the final answer."""
    calls, order, res, final = {}, [], {}, ""
    for line in open(path, encoding="utf-8", errors="replace"):
        try:
            e = json.loads(line)
        except ValueError:
            continue
        if e.get("type") == "assistant":
            for b in e["message"].get("content", []):
                if b.get("type") == "tool_use":
                    calls[b["id"]] = (b["name"].split("__")[-1], json.dumps(b["input"])); order.append(b["id"])
        elif e.get("type") == "user" and isinstance(e["message"].get("content"), list):
            for b in e["message"]["content"]:
                if b.get("type") == "tool_result":
                    c = b["content"]
                    res[b["tool_use_id"]] = rc.BANNER.sub("", c if isinstance(c, str) else " ".join(x.get("text", "") for x in (c or []) if isinstance(x, dict)))
        elif e.get("type") == "result":
            final = e.get("result") or ""
    return [(i + 1, calls[k][0], calls[k][1], res.get(k, "")) for i, k in enumerate(order)], final


def first_signal(path):
    st, final = steps(path)
    nochange = 0
    for n, tool, inp, text in st:
        for mech, name, rx in SIGNALS:
            if tool in PAGE_TEXT_TOOLS and name in PROSE_SIGNALS:
                continue
            if name == "act-no-effect-url-unchanged":
                if rx.search(text):
                    nochange += 1
                    if nochange >= 2:
                        return mech, name, n, tool, text
                continue
            if rx.search(text):
                return mech, name, n, tool, text
    for mech, name, rx in ANSWER_SIGNALS:
        if rx.search(final):
            return mech, name, len(st), "final-answer", final
    return "agent", "same-page-state-different-decision", len(st), "-", final


def one_line(s, n=170):
    return re.sub(r"\s+", " ", s).strip()[:n].replace("\t", " ")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--tr", default=os.path.expanduser("~/.cache/manuk-b1/transcripts"))
    ap.add_argument("--out", default=os.path.join(ROOT, "docs/sprint/rg"))
    ap.add_argument("--mutate")
    a = ap.parse_args()
    gap, p = gap_tasks()
    if a.mutate == "drop-task":
        gap = gap[:-1]
    fails = {}
    for r in rows("manuk"):
        if r["pass"] == "0":
            fails.setdefault(r["task"], []).append(r["run"])
    pwpass = {}
    for r in rows("playwright"):
        if r["pass"] == "1":
            pwpass.setdefault(r["task"], []).append(r["run"])
    out, cv = [], rows("manuk")[0]["chrome_ver"]
    for t in gap:
        sigs = []
        for run in fails.get(t, []):
            f = os.path.join(a.tr, f"manuk_{t}_r{run}.jsonl")
            if os.path.exists(f):
                sigs.append((run,) + first_signal(f))
        if not sigs:
            out.append(dict(task=t)); continue
        mech = collections.Counter(s[1] for s in sigs).most_common(1)[0][0]
        run, m, name, n, tool, text = next(s for s in sigs if s[1] == mech)
        pr = pwpass[t][0]
        pst, _ = steps(os.path.join(a.tr, f"playwright_{t}_r{pr}.jsonl"))
        cdp = p.get("cdp-layer")
        out.append(dict(task=t, cohort="ENGINE" if t in ENGINE else "SHARED", mechanism=mech, signal=name,
                        manuk_run=run, step=n, tool=tool, manuk_steps=len(steps(os.path.join(a.tr, f"manuk_{t}_r{run}.jsonl"))[0]),
                        pw_run=pr, pw_steps=len(pst), runs_failed=len(sigs), mech_votes="/".join(s[1][:3] for s in sigs),
                        evidence=f"manuk r{run} step {n} ({tool}): {one_line(text)}", chrome_ver=cv))
    cols = ["task", "cohort", "mechanism", "signal", "manuk_run", "step", "tool", "manuk_steps", "pw_run", "pw_steps", "runs_failed", "mech_votes", "evidence", "chrome_ver"]
    os.makedirs(a.out, exist_ok=True)
    with open(os.path.join(a.out, "LEDGER.tsv"), "w") as fh:
        w = csv.DictWriter(fh, cols, delimiter="\t", extrasaction="ignore", restval=""); w.writeheader(); w.writerows(out)
    good = [o for o in out if o.get("evidence")]
    rank = collections.Counter(o["mechanism"] for o in good)
    md = ["# RG.0 REAL gap ledger", "", f"chrome_ver={cv}, W5.B1 (3 runs x 102, Haiku). GAP = playwright passes >=2 of 3 and manuk <=1 of 3: {len(gap)} tasks.",
          "Mechanism = class of the FIRST failure signal in a failed manuk run (majority over its failed runs); rules in `bench/agent-tasks/rg0_ledger.py`.", "",
          "## Mechanisms ranked by tasks explained", "", "| mechanism | tasks | ENGINE | SHARED |", "|---|---|---|---|"]
    for m, c in rank.most_common():
        md.append(f"| {m} | {c} | {sum(1 for o in good if o['mechanism']==m and o['cohort']=='ENGINE')} | {sum(1 for o in good if o['mechanism']==m and o['cohort']=='SHARED')} |")
    for coh in ("ENGINE", "SHARED"):
        md += ["", f"## {coh} cohort", "", "| task | mechanism | signal | evidence |", "|---|---|---|---|"]
        md += [f"| {o['task']} | {o['mechanism']} | {o['signal']} | {o['evidence'].replace('|', '/')} |" for o in good if o["cohort"] == coh]
    open(os.path.join(a.out, "LEDGER.md"), "w").write("\n".join(md) + "\n")
    bad = [o["task"] for o in out if not o.get("evidence")]
    ok = len(gap) == 23 and not bad
    print(f"gap={len(gap)} classified={len(good)} unclassified={bad} ranking={dict(rank)}")
    print("GATE PASS" if ok else "GATE FAIL")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
