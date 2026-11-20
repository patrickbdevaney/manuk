#!/usr/bin/env python3
"""W6.J1.b.a.a: split FAIL subtests in a manuk-wpt runner log by message type.

Each FAIL header line ("    FAIL <name>") is followed by its assertion message line.
Categories:
  disallowed  -> message contains 'got disallowed value'
  transitionend -> message contains 'Expected TransitionEnd events'
  other       -> any FAIL whose message matches neither (or has no message)
Usage: categorise.py LOG [LOG ...]   (prints one TSV row per log)
       categorise.py --selftest
"""
import re
import sys

FAIL_RE = re.compile(r"^\s+FAIL\s")
DISALLOWED = "got disallowed value"
TRANSITIONEND = "Expected TransitionEnd events"


def categorise(lines):
    counts = {"disallowed": 0, "transitionend": 0, "other": 0, "fail_total": 0}
    for i, line in enumerate(lines):
        if not FAIL_RE.match(line):
            continue
        counts["fail_total"] += 1
        msg = ""
        if i + 1 < len(lines) and not FAIL_RE.match(lines[i + 1]) and lines[i + 1].strip():
            msg = lines[i + 1]
        if DISALLOWED in msg:
            counts["disallowed"] += 1
        elif TRANSITIONEND in msg:
            counts["transitionend"] += 1
        else:
            counts["other"] += 1
    return counts


def selftest():
    base = [
        "    FAIL a / values",
        "         assert_not_equals: x got disallowed value \"\"",
        "    FAIL a / events",
        "         assert_equals: Expected TransitionEnd events triggered on .transition expected \"a:2s\" but got \"\"",
    ]
    c = categorise(base)
    assert c == {"disallowed": 1, "transitionend": 1, "other": 0, "fail_total": 2}, c
    # Mutant: a third message type must land in 'other', not in either named category.
    mutant = base + [
        "    FAIL b / values",
        "         assert_true: something else entirely",
        "    FAIL c / values",
    ]
    c = categorise(mutant)
    assert c["other"] == 2 and c["disallowed"] == 1 and c["transitionend"] == 1, c
    assert c["fail_total"] == 4, c
    print("selftest ok: mutant third type -> other=2")


if __name__ == "__main__":
    if sys.argv[1:] == ["--selftest"]:
        selftest()
        sys.exit(0)
    for path in sys.argv[1:]:
        with open(path, encoding="utf-8", errors="replace") as fh:
            lines = fh.read().splitlines()
        c = categorise(lines)
        print("\t".join([path, str(c["fail_total"]), str(c["disallowed"]),
                         str(c["transitionend"]), str(c["other"])]))
