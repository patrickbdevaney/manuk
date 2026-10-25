#!/usr/bin/env python3
"""Pick the next node of the product-sprint graph (docs/sprint/GRAPH.tsv).

Prints `<id>\t<model>` for the node the loop should work next, or `NONE` when nothing is ready.
Order: an IN_PROGRESS node first (a crashed invocation resumes its node), then READY TODO nodes
(every dep DONE) by ring, then priority, then file order. BLOCKED nodes are skipped until a human
or a later node flips them back to TODO.
"""
import csv
import sys
from pathlib import Path

GRAPH = Path(__file__).resolve().parent.parent / "docs/sprint/GRAPH.tsv"


def main() -> int:
    rows = list(csv.DictReader(GRAPH.open(), delimiter="\t"))
    status = {r["id"]: r["status"] for r in rows}
    for r in rows:
        if r["status"] == "IN_PROGRESS":
            print(f"{r['id']}\t{r['model_floor']}")
            return 0
    ready = []
    for i, r in enumerate(rows):
        if r["status"] != "TODO":
            continue
        deps = [d for d in r["deps"].split(",") if d and d != "-"]
        if all(status.get(d) == "DONE" for d in deps):
            ready.append((int(r["ring"]), int(r["pri"]), i, r))
    if not ready:
        print("NONE")
        return 0
    r = min(ready)[3]
    print(f"{r['id']}\t{r['model_floor']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
