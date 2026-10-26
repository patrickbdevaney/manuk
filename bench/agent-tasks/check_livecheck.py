"""Gate for docs/sprint/EXTERNAL-LIVECHECK.md: every X01-X10 row is OK/DRIFTED with a non-empty
observed value, and an OK row's observed value contains the frozen answer."""
import json, re, sys, pathlib
here = pathlib.Path(__file__).resolve().parent
doc = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else here.parents[1] / "docs/sprint/EXTERNAL-LIVECHECK.md"
tasks = {t["id"]: t for t in json.load(open(here / "suite-v1-external.json"))["tasks"]}
rows = {}
for l in doc.read_text().split("\n"):
    c = [x.strip() for x in l.strip().strip("|").split("|")]
    if len(c) >= 4 and re.fullmatch(r"X\d\d", c[0]):
        rows[c[0]] = (c[2], c[3])
bad = []
for i in (f"X{n:02d}" for n in range(1, 11)):
    st, ob = rows.get(i, ("MISSING", ""))
    if st not in ("OK", "DRIFTED") or not ob.strip("` "):
        bad.append(f"{i}: status={st!r} observed={ob!r}")
    elif st == "OK":
        miss = [a for a in tasks[i]["verifier"]["answer_all"] if a.lower() not in ob.lower()]
        if miss:
            bad.append(f"{i}: OK but observed lacks {miss}")
print("\n".join(bad) or "livecheck OK: X01-X10 all verdicts with observed values")
sys.exit(1 if bad else 0)
