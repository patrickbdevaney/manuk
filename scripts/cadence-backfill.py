#!/usr/bin/env python3
"""Reconstruct the cadence ledger from git history — so the dataset starts with seventy points, not one.

The loop has been running for sixty-nine ticks and recorded none of its own vitals. But it did not lose
them: **every tick is a commit**, and a commit carries its timestamp, its diff, and its message. The
journal carries the shape and the headline. That is most of the row already, and it is all ground truth.

What CANNOT be recovered, and is therefore left blank rather than invented:

* **the verify-wall time** of a past tick. `STATUS.md` records only the LATEST wall, so a backfilled row
  would be a fabrication. Blank.
* **the WPT figure** at a past tick. It was measured a handful of times, at ticks that said so, and
  copying today's number backwards would produce a beautiful flat line that never happened.
* **gates / claims / ✅-rows** as they stood *then*. These are counted by grepping the tree, and the tree
  is now. Counting today's tree and labelling it "tick 42" is exactly the kind of number this project
  refuses to produce.

So a backfilled row carries what git actually knows — **when, how big, what shape, and what it claimed to
do** — and leaves the rest empty. An empty cell is a fact. A guessed one is a lie that will be quoted back
later as evidence.
"""

import re
import subprocess
import sys
from pathlib import Path

TSV = Path("docs/loop/CADENCE.tsv")
JOURNAL = Path("docs/loop/JOURNAL.md")

HEADER = (
    "tick\tsha\tepoch\tiso\tdelta_s\tshape\twall_s\tfiles\tadded\tdeleted\t"
    "gates\tclaims\twpt_pass\twpt_total\twpt_fresh\tpatterns_ok\thangs\theadline"
)


def sh(*args: str) -> str:
    return subprocess.run(args, capture_output=True, text=True, check=False).stdout


def main() -> None:
    journal = JOURNAL.read_text(encoding="utf8") if JOURNAL.exists() else ""

    # Every commit whose subject names a loop tick. That IS the tick's landing, by construction: the
    # pre-commit hook refuses any tick commit whose gates are not green on exactly that tree.
    log = sh("git", "log", "--reverse", "--format=%H\x01%at\x01%aI\x01%s")
    ticks = {}
    for line in log.strip().split("\n"):
        if not line:
            continue
        sha, epoch, iso, subject = line.split("\x01", 3)
        m = re.search(r"loop tick (\d+)", subject, re.I)
        if not m:
            continue
        n = int(m.group(1))
        # A tick can appear more than once if it was amended; the LAST commit is the one that landed.
        ticks[n] = (sha, int(epoch), iso, subject)

    if not ticks:
        print("no tick commits found", file=sys.stderr)
        return

    rows = []
    prev_epoch = None
    for n in sorted(ticks):
        sha, epoch, iso, subject = ticks[n]

        stat = sh("git", "show", "--stat", "--format=", sha).strip().split("\n")[-1]
        files = int(m.group(1)) if (m := re.search(r"(\d+) files? changed", stat)) else 0
        added = int(m.group(1)) if (m := re.search(r"(\d+) insertions?", stat)) else 0
        deleted = int(m.group(1)) if (m := re.search(r"(\d+) deletions?", stat)) else 0

        # Shape and headline from the journal — the tick's own account of itself.
        shape, headline = "unknown", ""
        sec = re.search(rf"^## Tick {n}\b(.*?)(?=^## Tick |\Z)", journal, re.M | re.S)
        if sec:
            body = sec.group(0)
            if s := re.search(r"TICK SHAPE:\s*\**\s*([a-z0-9-]+)", body, re.I):
                shape = s.group(1).lower()
            if h := re.match(rf"^## Tick {n}\s*[—-]\s*(.+)", body):
                headline = h.group(1).strip()[:160]
        if not headline:
            headline = re.sub(r"^\w+\(loop tick \d+\):\s*", "", subject)[:160]

        delta = (epoch - prev_epoch) if prev_epoch else 0
        prev_epoch = epoch

        # EVERY other column is left blank ON PURPOSE. See the module docstring: an empty cell is a fact,
        # a guessed one is a lie that gets quoted back later as evidence.
        rows.append(
            f"{n}\t{sha[:7]}\t{epoch}\t{iso}\t{delta}\t{shape}\t\t{files}\t{added}\t{deleted}"
            f"\t\t\t\t\t0\t\t\t{headline}"
        )

    TSV.parent.mkdir(parents=True, exist_ok=True)
    TSV.write_text(HEADER + "\n" + "\n".join(rows) + "\n", encoding="utf8")
    print(f"backfilled {len(rows)} ticks (ticks {min(ticks)}–{max(ticks)}) → {TSV}", file=sys.stderr)


if __name__ == "__main__":
    main()
