#!/usr/bin/env python3
"""sprint-graph — validated edits of docs/sprint/GRAPH.tsv (9 tab-separated columns).

  sprint-graph.py check
  sprint-graph.py add  <id> <ring> <pri> <model_floor> <deps|-> <status> <title> <gate> <notes>
  sprint-graph.py set  <id> <column> <value>

Every write goes through validate(): exactly 9 columns per row, integer ring/pri, floor in haiku|sonnet
(opus is never a floor), status in the known set, no tab/newline inside a field, unique ids. A rejected
edit (one that introduces a violation not already in the file) leaves the file byte-for-byte untouched (exit 2). GRAPH env var overrides the path (tests).
"""
import csv
import os
import sys
from pathlib import Path

GRAPH = Path(os.environ.get("GRAPH") or Path(__file__).resolve().parent.parent / "docs/sprint/GRAPH.tsv")
COLS = ["id", "ring", "pri", "model_floor", "deps", "status", "title", "gate", "notes"]
STATUS = {"TODO", "IN_PROGRESS", "DONE", "BLOCKED"}
FLOORS = {"haiku", "sonnet"}


def validate(rows: list[list[str]]) -> list[str]:
    errs, seen = [], set()
    if rows[0] != COLS:
        errs.append(f"header must be {COLS}")
    for n, r in enumerate(rows[1:], start=2):
        if len(r) != 9:
            errs.append(f"row {n} ({r[0] if r else '?'}): {len(r)} columns, need 9")
            continue
        d = dict(zip(COLS, r))
        if not d["id"] or d["id"] in seen:
            errs.append(f"row {n}: empty or duplicate id {d['id']!r}")
        seen.add(d["id"])
        if not d["ring"].isdigit() or not d["pri"].isdigit():
            errs.append(f"row {n} ({d['id']}): ring/pri must be integers")
        if d["model_floor"] not in FLOORS:
            errs.append(f"row {n} ({d['id']}): model_floor {d['model_floor']!r} not in {sorted(FLOORS)}")
        if d["status"] not in STATUS:
            errs.append(f"row {n} ({d['id']}): status {d['status']!r} not in {sorted(STATUS)}")
        if not d["title"] or not d["gate"]:
            errs.append(f"row {n} ({d['id']}): title and gate must be non-empty")
        for f in r:
            if "\t" in f or "\n" in f:
                errs.append(f"row {n} ({d['id']}): tab/newline inside a field")
                break
    ids = {r[0] for r in rows[1:] if r}
    for r in rows[1:]:
        if len(r) == 9:
            for dep in r[4].split(","):
                if dep and dep != "-" and dep not in ids:
                    errs.append(f"{r[0]}: dep {dep!r} is not a node id")
    return errs


def load() -> list[list[str]]:
    with GRAPH.open(newline="") as f:
        return list(csv.reader(f, delimiter="\t", quoting=csv.QUOTE_NONE))


def save(rows: list[list[str]]) -> int:
    # legacy rows that already violate a rule (check lists them) do not block unrelated edits: only NEW errors reject
    errs = [e for e in validate(rows) if e not in set(validate(load()))]
    if errs:
        print("REJECTED:\n  " + "\n  ".join(errs), file=sys.stderr)
        return 2
    tmp = GRAPH.with_suffix(".tmp")
    tmp.write_text("".join("\t".join(r) + "\n" for r in rows))
    tmp.replace(GRAPH)
    return 0


def main(argv: list[str]) -> int:
    if not argv:
        print(__doc__)
        return 1
    rows = load()
    cmd = argv[0]
    if cmd == "check":
        errs = validate(rows)
        print("\n".join(errs) if errs else f"ok: {len(rows) - 1} rows")
        return 2 if errs else 0
    if cmd == "add":
        if len(argv) != 10:
            print(f"add needs 9 fields, got {len(argv) - 1}", file=sys.stderr)
            return 2
        return save(rows + [argv[1:]])
    if cmd == "set":
        if len(argv) != 4 or argv[2] not in COLS:
            print(f"usage: set <id> <{'|'.join(COLS)}> <value>", file=sys.stderr)
            return 2
        hit = [r for r in rows[1:] if r[0] == argv[1]]
        if not hit:
            print(f"no node {argv[1]}", file=sys.stderr)
            return 2
        hit[0][COLS.index(argv[2])] = argv[3]
        return save(rows)
    print(__doc__)
    return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
