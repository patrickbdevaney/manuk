#!/usr/bin/env python3
"""GUARD.R0.a.a: every a11y-spread site must carry a classified row with sha256 evidence.
usage: python3 -I scripts/guard-spread-check.py [GUARD-SPREAD.tsv]   exit 1 if a site lacks evidence."""
import csv, os, re, sys
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
path = sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "docs/sprint/GUARD-SPREAD.tsv")
SITES = ["tracker.shadowfax.in", "meuconsultordigital.com.br", "mobcup.fm",
         "portagelearning.edu", "possssno.sbs", "sestra.cc"]
rows = {r["url"]: r for r in csv.DictReader(open(path), delimiter="\t")}
bad = 0
for s in SITES:
    r = next((v for k, v in rows.items() if s in k), None)
    if not r or r["class"] not in ("INSTRUMENT", "ORIGIN", "ENGINE") or not re.search(r"[0-9a-f]{16}", r["evidence"]):
        print("RED:", s); bad += 1
print("rows ok" if not bad else "%d sites lack evidence" % bad)
sys.exit(1 if bad else 0)
