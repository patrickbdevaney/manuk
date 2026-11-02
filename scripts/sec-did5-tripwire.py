#!/usr/bin/env python3 -I
"""SEC.DID.5 tripwire: no password manager or autofill sink may appear in shell/ engine/ mcp/ unless listed.

Census P5 (docs/sprint/SECURITY.md) says a web page has no path to a credential store. This gate makes that
claim self-enforcing: any source file under the scanned roots that names a credential-store or autofill
identifier must have a (path, identifier) row in docs/sprint/SEC-DID5-ALLOWLIST.tsv. A new one turns it RED,
and the fix is a security review (a SEC.DID node), not a quiet allowlist edit.

Usage: sec-did5-tripwire.py [repo_root]   (repo_root defaults to the checkout this script lives in)
"""
import os, re, sys

root = os.path.abspath(sys.argv[1]) if len(sys.argv) > 1 else os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
allow_path = os.path.join(root, "docs/sprint/SEC-DID5-ALLOWLIST.tsv")
SCAN_ROOTS = ["shell", "engine", "mcp"]
SCAN_EXT = {".rs", ".toml", ".html", ".js", ".ts", ".py", ".sh", ".md", ".txt"}
SKIP_DIRS = {"target", "__pycache__", ".git"}
IDENT = re.compile(r"keyring|secret-tool|libsecret|secret service|password_?store|password_?manager|autofill|credential_?store", re.I)

allowed = set()
for line in open(allow_path):
    if not line.strip() or line.startswith("#"):
        continue
    path, ident = line.rstrip("\n").split("\t")[:2]
    allowed.add((path, ident.lower()))

hits = {}
for scan in SCAN_ROOTS:
    base = os.path.join(root, scan)
    for dirpath, dirnames, filenames in os.walk(base):
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS]
        for fn in filenames:
            if os.path.splitext(fn)[1] not in SCAN_EXT:
                continue
            full = os.path.join(dirpath, fn)
            rel = os.path.relpath(full, root)
            with open(full, encoding="utf-8", errors="replace") as f:
                for lineno, text in enumerate(f, 1):
                    for m in IDENT.finditer(text):
                        hits.setdefault((rel, m.group(0).lower()), []).append(lineno)

bad = []
for (rel, ident), lines in sorted(hits.items()):
    if (rel, ident) not in allowed:
        bad.append(f"{rel} names {ident!r} at line {lines[0]} ({len(lines)} hit(s)) with no allowlist row")
# A stale allowlist row is harmless but means the census has drifted; report it so it gets pruned.
for (path, ident) in sorted(allowed):
    if (path, ident) not in hits:
        bad.append(f"stale allowlist row: {path} {ident!r} no longer occurs")

for b in bad:
    print("RED", b)
print(f"{len(hits)} (file, identifier) pairs scanned, {len(allowed)} allowlisted, {len(bad)} problems")
sys.exit(1 if bad else 0)
