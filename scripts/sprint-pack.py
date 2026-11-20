#!/usr/bin/env python3
"""sprint-pack — print the <6k-token context pack for one graph node (replaces mandated reading).

  sprint-pack.py <node-id>   -> stdout; exit 3 (and stderr) if the pack would exceed MAX_TOKENS before truncation

Contents: docs/sprint/PACK-BASE.md (standing digest) + the node row + each dep's row and its JOURNAL line +
the parent node's JOURNAL line (for X.a follow-ups). Tokens are estimated as chars/4 (a deliberate over-estimate
for English; the ledger holds the real counts). Pieces are truncated, never silently dropped.
"""
import csv
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MAX_TOKENS = 6000
est = lambda s: (len(s) + 3) // 4


def clip(s: str, n: int) -> str:
    return s if len(s) <= n else s[: n - 1] + "…"


def main(node: str) -> int:
    rows = list(csv.DictReader((ROOT / "docs/sprint/GRAPH.tsv").open(), delimiter="\t"))
    by = {r["id"]: r for r in rows}
    if node not in by:
        print(f"no node {node}", file=sys.stderr)
        return 2
    journal = {}
    for line in (ROOT / "docs/sprint/JOURNAL.md").read_text(errors="replace").splitlines():
        k, sep, rest = line.partition(": ")
        if sep and " " not in k:
            journal[k] = line
    r = by[node]
    out = [(ROOT / "docs/sprint/PACK-BASE.md").read_text().rstrip(), "", f"# Your node: {node}"]
    for col in ("title", "gate", "notes", "deps", "status", "model_floor"):
        out.append(f"- **{col}**: {r[col]}")
    related = [d for d in r["deps"].split(",") if d and d != "-"]
    parent = node.rsplit(".", 1)[0] if "." in node else ""
    if parent in by and parent not in related:
        related.append(parent)
    out += ["", "## Dependencies / parent (status + what landed)"]
    for d in related:
        if d in by:
            out.append(f"- **{d}** [{by[d]['status']}] {clip(by[d]['title'], 200)}")
            if d in journal:
                out.append(f"  - journal: {clip(journal[d], 700)}")
    out += ["", "Open any full doc named above only if this pack cannot answer a specific question."]
    text = "\n".join(out) + "\n"
    if est(text) > MAX_TOKENS:
        print(f"pack for {node} is ~{est(text)} tokens > {MAX_TOKENS}; truncated", file=sys.stderr)
        text = text[: MAX_TOKENS * 4 - 40].rstrip() + "\n…[truncated]\n"
        sys.stdout.write(text)
        return 3
    sys.stdout.write(text)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]) if len(sys.argv) == 2 else 1)
