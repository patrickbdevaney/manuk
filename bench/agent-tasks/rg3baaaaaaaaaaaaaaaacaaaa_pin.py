#!/usr/bin/env python3
"""Gate for RG.3...c.a.a.a.a: frozen opendining-4 transcript and bar constants match the pins in the decision doc.
--mutate transcript|const|doc: run the check against a deliberately broken temp copy (must exit 1)."""
import hashlib, os, re, sys, tempfile
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
DOC = os.path.join(ROOT, "docs/sprint/rg/RG3baaaaaaaaaaaaaaaacaaaa-decision.md")
TR = os.path.expanduser("~/.cache/manuk-rgb2/transcripts/manuk_opendining-4_r1.jsonl")
SP = os.path.join(ROOT, "bench/agent-tasks/rg3baaaaaaa_spread.py")
MA = os.path.join(ROOT, "bench/agent-tasks/rg3baaaaaaa_match.py")
sha = lambda p: hashlib.sha256(open(p, "rb").read()).hexdigest()
mut = sys.argv[sys.argv.index("--mutate") + 1] if "--mutate" in sys.argv else None
tmp = tempfile.mkdtemp()
def broken(p, f):
    q = os.path.join(tmp, os.path.basename(p)); d = open(p, "rb").read(); open(q, "wb").write(f(d)); return q
if mut == "transcript": TR = broken(TR, lambda d: d + b"\n")
if mut == "const": SP = broken(SP, lambda d: d.replace(b"13, 9, 12", b"13, 9, 10"))
if mut == "doc": DOC = broken(DOC, lambda d: d.replace(b"KEEP", b"x"))
doc = open(DOC).read(); errs = []
pins = re.findall(r"[0-9a-f]{64}", doc)
for name, p in (("transcript", TR), ("spread", SP), ("match", MA)):
    if sha(p) not in pins: errs.append(f"{name} sha256 {sha(p)} not pinned in decision doc")
if "EXPECT, MIN_MANUK, MIN_CHROME = 13, 9, 12" not in open(SP).read(): errs.append("bar constants changed")
for need in ("7b556dab", "KEEP", "chrome_ver=145.0.7632.116"):
    if need not in doc: errs.append(f"doc lacks {need}")
print("\n".join(errs) or "OK: transcript, bars, scoring pinned and unchanged"); sys.exit(1 if errs else 0)
