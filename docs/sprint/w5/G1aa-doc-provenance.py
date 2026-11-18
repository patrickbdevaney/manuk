#!/usr/bin/env python3
"""W5.G1.a.a.a.a.a gate: every run/pass claim and quote in the G1aa doc must name the archive it came from.

Usage: G1aa-doc-provenance.py [--doc PATH] [--root REPO] [--tsv PATH]

Defaults: doc = docs/sprint/w5/G1aa-stop-before-send.md, tsv = docs/sprint/w5/G1aa-cdp-layer.tsv, root = .

The doc has two fenced blocks:

  ```claims     one line per run:  <run N or ->TAB<archive dir>TAB<pass=0|1>
  ```quotes     one line per quote: <archive dir or task>TAB<text>

Archive dirs are named exactly as under target/real-runs/ and carry a timestamp suffix
(cdp-layer_gomail-3_r<N>.<YYYYMMDDTHHMMSS>). Un-suffixed dirs are live run directories that later
runs overwrite, so they are rejected as provenance.

Checks (any failure -> exit 1):
  1. The doc has a claims block and a quotes block; no line outside the blocks states pass=/fail
     (every pass claim must be in the claims block).
  2. Each claim's archive exists, has state.json and transcript.jsonl, and its run number matches N.
  3. If N is not '-', the TSV row for run N is cdp-layer / gomail-3 / gomail, its timestamp is within
     3 s of the archive timestamp, and its pass column equals the claimed pass.
  4. The archive's state agrees with the claim: pass=1 needs differences.emails.added[0] to
     jonathan.smith@example.com, subject "Meeting Notes", content containing the body text (the three
     evals of gomail-3); pass=0 needs no added email.
  5. Every quote is verbatim in a decoded string of its archive's transcript.jsonl (task: the goal of
     gomail-3.json).
Exit 0 = all checks pass.
"""
import datetime
import json
import os
import re
import sys

BODY = "Please find the meeting notes attached."
TO = "jonathan.smith@example.com"
SUBJECT = "Meeting Notes"
ARCHIVE_RE = re.compile(r"^cdp-layer_gomail-3_r(\d+)\.(\d{8}T\d{6})$")


def strings(node):
    if isinstance(node, str):
        yield node
    elif isinstance(node, dict):
        for v in node.values():
            yield from strings(v)
    elif isinstance(node, list):
        for v in node:
            yield from strings(v)


def read_blocks(doc):
    """Return (claims, quotes, outside_lines). Lines outside the fenced blocks are returned for scanning."""
    claims, quotes, outside = [], [], []
    block = None
    with open(doc, encoding="utf-8") as f:
        for line in f:
            line = line.rstrip("\n")
            s = line.strip()
            if block is None and s in ("```claims", "```quotes"):
                block = s[3:]
                continue
            if block is not None:
                if s == "```":
                    block = None
                    continue
                if "\t" in line:
                    if block == "claims":
                        parts = line.split("\t")
                        if len(parts) == 3:
                            claims.append(tuple(p.strip() for p in parts))
                    else:
                        src, text = line.split("\t", 1)
                        quotes.append((src.strip(), text))
                continue
            outside.append(line)
    return claims, quotes, outside


def load_tsv(path):
    rows = {}
    with open(path, encoding="utf-8") as f:
        header = f.readline().rstrip("\n").split("\t")
        for line in f:
            if not line.strip():
                continue
            rec = dict(zip(header, line.rstrip("\n").split("\t")))
            rows[rec.get("run", "")] = rec
    return rows


def archive_ts(stamp):
    return datetime.datetime.strptime(stamp, "%Y%m%dT%H%M%S")


def main(argv):
    def opt(name, default):
        return argv[argv.index(name) + 1] if name in argv else default

    doc = opt("--doc", "docs/sprint/w5/G1aa-stop-before-send.md")
    root = opt("--root", ".")
    tsv_path = opt("--tsv", os.path.join(root, "docs/sprint/w5/G1aa-cdp-layer.tsv"))
    runs_dir = os.path.join(root, "target/real-runs")
    fails = []

    claims, quotes, outside = read_blocks(doc)
    if not claims:
        fails.append("no ```claims block with run/archive/pass lines")
    if not quotes:
        fails.append("no ```quotes block")

    for line in outside:
        if re.search(r"pass\s*=\s*[01]", line) or re.search(r"\b[rR]\d\s*\((pass|fail)\)", line):
            fails.append(f"pass claim outside the claims block: {line[:70]!r}")

    tsv = load_tsv(tsv_path) if os.path.exists(tsv_path) else {}
    if not tsv:
        fails.append(f"TSV not readable: {tsv_path}")

    for run, archive, claim in claims:
        m = ARCHIVE_RE.match(archive)
        if not m:
            fails.append(f"{archive}: not a timestamped archive name (live dirs are overwritten)")
            continue
        arch_run, stamp = m.group(1), m.group(2)
        if run != "-" and run != arch_run:
            fails.append(f"{archive}: claimed run {run} but archive is run {arch_run}")
        adir = os.path.join(runs_dir, archive)
        state_path = os.path.join(adir, "state.json")
        tr_path = os.path.join(adir, "transcript.jsonl")
        if not (os.path.exists(state_path) and os.path.exists(tr_path)):
            fails.append(f"{archive}: missing state.json or transcript.jsonl")
            continue
        if claim not in ("pass=0", "pass=1"):
            fails.append(f"{archive}: bad claim {claim!r}")
            continue
        want = claim == "pass=1"

        if run != "-":
            row = tsv.get(run)
            if row is None:
                fails.append(f"run {run}: no TSV row")
            else:
                if (row.get("arm"), row.get("task"), row.get("site")) != ("cdp-layer", "gomail-3", "gomail"):
                    fails.append(f"run {run}: TSV row is not cdp-layer/gomail-3")
                tsv_ts = datetime.datetime.strptime(row["ts"], "%Y-%m-%dT%H:%M:%S")
                if abs((tsv_ts - archive_ts(stamp)).total_seconds()) > 3:
                    fails.append(f"run {run}: TSV ts {row['ts']} is not the ts of {archive}")
                if row.get("pass") != ("1" if want else "0"):
                    fails.append(f"run {run}: TSV pass={row.get('pass')} but doc claims {claim}")

        with open(state_path, encoding="utf-8") as f:
            added = (((json.load(f).get("state") or {}).get("differences") or {}).get("emails") or {}).get("added") or []
        if want:
            e = added[0] if added else {}
            ok = (
                bool(added)
                and (e.get("to") or [None])[0] == TO
                and e.get("subject") == SUBJECT
                and BODY in (e.get("content") or "")
            )
            if not ok:
                fails.append(f"{archive}: claims pass=1 but state has no matching sent email")
        elif added:
            fails.append(f"{archive}: claims pass=0 but state has {len(added)} added email(s)")

    if quotes:
        task_path = os.path.join(root, "bench/agent-tasks/real/tasks/gomail-3.json")
        with open(task_path, encoding="utf-8") as f:
            goal = json.load(f)["goal"]
        cache = {}
        for src, text in quotes:
            if src == "task":
                hit = text in goal
            else:
                if not ARCHIVE_RE.match(src):
                    fails.append(f"quote source {src!r} is not a timestamped archive")
                    continue
                if src not in cache:
                    tr = os.path.join(runs_dir, src, "transcript.jsonl")
                    vals = []
                    if os.path.exists(tr):
                        with open(tr, encoding="utf-8") as f:
                            for line in f:
                                if line.strip():
                                    vals.extend(strings(json.loads(line)))
                    cache[src] = vals
                hit = any(text in v for v in cache[src])
            if not hit:
                fails.append(f"quote not verbatim in {src}: {text[:60]!r}")

    for msg in fails:
        print("FAIL:", msg)
    if fails:
        return 1
    print(f"PASS: {len(claims)} run/pass claims and {len(quotes)} quotes match their named archives and TSV rows")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
