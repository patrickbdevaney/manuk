#!/usr/bin/env python3
"""Pick the next node of the product-sprint graph (docs/sprint/GRAPH.tsv).

Prints `<id>\t<model>` for the node the loop should work next, or `NONE` when nothing is ready.
Order: an IN_PROGRESS node first (a crashed invocation resumes its node), then READY TODO nodes
(every dep DONE) by effective ring, then priority, then file order. BLOCKED nodes are skipped until
a human or a later node flips them back to TODO.

Two rules keep follow-ups from starving the plan (observer, 2026-10-25: one night produced a
13-deep ring-0 chain about a usa.gov coin flip while every ring-1 product node waited):
- a FOLLOW-UP of a BLOCKED node (`X.a` under `X`) may run: it is how its parent gets unblocked,
  so a dep on its own blocked ancestor counts as met;
- a follow-up two or more levels deep (`X.a.b`) is demoted two rings, unless some unfinished node
  depends on it or the observer placed it (a DIAGONAL note), so it never outranks the next ring's
  planned work.

A third rule keeps a BLOCKED node from being a dead end once its follow-ups have answered it (observer,
2026-10-29): a BLOCKED node with at least one follow-up, and no follow-up still TODO or IN_PROGRESS, is
offered for a REVISIT at its own ring and priority. It is offered while the loop log shows fewer than two
`⛔ node <id>` lines for it, so a node is revisited at most twice and can never cycle.
"""
import csv
import sys
from pathlib import Path

GRAPH = Path(__file__).resolve().parent.parent / "docs/sprint/GRAPH.tsv"
LOG = GRAPH.parent / "loop.log"


def block_count(node_id: str) -> int:
    """How many times the loop has logged this node as BLOCKED (agent- or auto-blocked)."""
    try:
        text = LOG.read_text(errors="replace")
    except OSError:
        return 0
    return sum(1 for line in text.splitlines() if f"\u26d4 node {node_id} " in line)


def depth(node_id: str) -> int:
    """Number of trailing single-letter follow-up segments: R0.4.a.b → 2, W3.1 → 0."""
    n = 0
    for seg in reversed(node_id.split(".")):
        if len(seg) == 1 and seg.isalpha() and seg.islower():
            n += 1
        else:
            break
    return n


def malformed(rows: list[dict]) -> list[str]:
    """Rows a hand edit broke: a stray column, a missing one, or a non-integer ring/pri."""
    bad = []
    for n, r in enumerate(rows, start=2):
        if None in r or None in r.values() or not r["ring"].isdigit() or not r["pri"].isdigit():
            bad.append(f"row {n}: {r.get('id')!r}")
    return bad


def main() -> int:
    rows = list(csv.DictReader(GRAPH.open(), delimiter="\t"))
    bad = malformed(rows)
    if bad:
        print("BADGRAPH " + "; ".join(bad))
        return 0
    status = {r["id"]: r["status"] for r in rows}
    for r in rows:
        if r["status"] == "IN_PROGRESS":
            print(f"{r['id']}\t{r['model_floor']}")
            return 0
    deps_of = {r["id"]: [d for d in r["deps"].split(",") if d and d != "-"] for r in rows}
    needed = {d for r in rows if r["status"] != "DONE" for d in deps_of[r["id"]]}

    def met(node_id: str, dep: str) -> bool:
        if status.get(dep) == "DONE":
            return True
        return status.get(dep) == "BLOCKED" and node_id.startswith(dep + ".")

    def revisit(r: dict) -> bool:
        kids = [x for x in rows if x["id"].startswith(r["id"] + ".")]
        return (bool(kids) and not any(x["status"] in ("TODO", "IN_PROGRESS") for x in kids)
                and block_count(r["id"]) < 2)

    ready = []
    for i, r in enumerate(rows):
        if r["status"] == "BLOCKED" and revisit(r):
            ready.append((int(r["ring"]), int(r["pri"]), i, r))
            continue
        if r["status"] != "TODO":
            continue
        if all(met(r["id"], d) for d in deps_of[r["id"]]):
            ring = int(r["ring"])
            planned = "DIAGONAL" in r["notes"]  # placed by the observer, so not drift
            if depth(r["id"]) >= 2 and r["id"] not in needed and not planned:
                ring += 2
            ready.append((ring, int(r["pri"]), i, r))
    if not ready:
        print("NONE")
        return 0
    r = min(ready)[3]
    print(f"{r['id']}\t{r['model_floor']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
