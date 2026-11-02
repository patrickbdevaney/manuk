#!/usr/bin/env python3
"""R4.2 gate: README's scoreboard table is regenerated from docs/sprint/SCOREBOARD.tsv and must match byte for byte;
no unmeasured claim words may appear outside the Known-gaps quotes.

  readme-check.py --emit        print the table block
  readme-check.py               check README.md (exit 1 on mismatch or banned word)
  readme-check.py --selftest    prove the check goes red on a changed cell and on a banned word
Rule: the latest row in file order per (arm, slice, task, run) is the measurement; rows at another chrome_ver
(or suite) are ignored. Pass per run = tasks with pass==1 in that run. Run with `python3 -I`.
"""
import csv, os, re, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PIN = open(os.path.join(ROOT, "docs/sprint/CHROME_PIN")).read().strip()
SUITES = {"own": "69903c", "external": "061ee2"}
ARMS = [("playwright", "Playwright MCP + Chrome"), ("cdp-layer", "manuk-mcp `--backend chrome`"), ("manuk", "manuk-mcp (in-process engine)")]
BEGIN, END = "<!-- scoreboard:begin -->", "<!-- scoreboard:end -->"
BANNED = re.compile(r"\b(faster|lighter|more reliable|fastest|better than|outperforms?)\b", re.I)


def table():
    last = {}
    for r in csv.DictReader(open(os.path.join(ROOT, "docs/sprint/SCOREBOARD.tsv")), delimiter="\t"):
        if r["chrome_ver"] != PIN or r["slice"] not in SUITES or not r["suite_hash"].startswith(SUITES[r["slice"]]):
            continue
        last[(r["arm"], r["slice"], r["task"], r["run"])] = r
    out = [BEGIN, "", f"chrome_ver={PIN}, driving model Haiku 5.5, pass count per run (3 runs), from `docs/sprint/SCOREBOARD.tsv`:", "",
           "| arm | own 40 tasks, runs 1/2/3 | external 10 tasks, runs 1/2/3 |", "|---|---|---|"]
    for arm, label in ARMS:
        cells = []
        for sl in ("own", "external"):
            rs = [r for k, r in last.items() if k[0] == arm and k[1] == sl and not r["task"].startswith(("X11", "X12", "X13", "X14"))]
            tasks = {r["task"] for r in rs}
            runs = sorted({r["run"] for r in rs})
            cells.append(" / ".join(f"{sum(r['pass'] == '1' for r in rs if r['run'] == n)}" for n in runs) + f" of {len(tasks)}")
        out.append(f"| {label} | {cells[0]} | {cells[1]} |")
    out += ["", END]
    return "\n".join(out)


def check(text):
    errs = []
    m = re.search(re.escape(BEGIN) + r".*?" + re.escape(END), text, re.S)
    if not m:
        errs.append("scoreboard markers missing")
    elif m.group(0) != table():
        errs.append("scoreboard table differs from SCOREBOARD.tsv")
    body = re.sub(re.escape(BEGIN) + r".*?" + re.escape(END), "", text, flags=re.S)
    for i, line in enumerate(body.splitlines(), 1):
        if BANNED.search(line):
            errs.append(f"banned claim word line {i}: {line.strip()[:80]}")
    return errs


def main():
    if "--emit" in sys.argv:
        print(table()); return
    text = open(os.path.join(ROOT, "README.md")).read()
    if "--selftest" in sys.argv:
        t = table()
        bad1 = text.replace(t, t.replace("| 3", "| 4", 1) if "| 3" in t else t + "x")
        bad1 = re.sub(r"(\| manuk-mcp \(in-process engine\) \| )\d+", lambda m: m.group(1) + "99", text)
        bad2 = text + "\nManuk is faster than Chrome.\n"
        assert not check(text), check(text)
        assert check(bad1), "changed cell not caught"
        assert check(bad2), "banned word not caught"
        print("selftest ok: clean=green, changed cell=red, banned word=red"); return
    errs = check(text)
    print("\n".join(errs) or "README ok")
    sys.exit(1 if errs else 0)


main()
