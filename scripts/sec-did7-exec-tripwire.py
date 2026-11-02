#!/usr/bin/env python3 -I
"""SEC.DID.7 tripwire: no process exec or fork from shipped Rust code unless listed.

Census P7 (docs/sprint/SECURITY.md): a web page must have no path to process exec. This gate makes the
inventory of exec sites self-enforcing: every (file, token) pair in shipped Rust source that spawns a
process or forks must appear in docs/sprint/SEC-DID7-ALLOWLIST.tsv with the reviewed COUNT of occurrences.
A new site, or an extra occurrence in an allowlisted file, turns the gate RED; the fix is a security review
(a SEC.DID node), not a quiet allowlist edit.

Not scanned: tests/ and examples/ and benches/ directories at any depth, src/bin/ (developer tools), target/,
and any `#[cfg(test)]` item. Comments and string literals are stripped before matching, so a doc comment that
mentions Command::new is not a hit. Braces inside raw strings can confuse the `#[cfg(test)]` skip; that errs
toward scanning MORE, not less.

Usage: sec-did7-exec-tripwire.py [repo_root] [--list]   (--list prints every hit and exits 0)
"""
import os, re, subprocess, sys

args = [a for a in sys.argv[1:] if not a.startswith("--")]
list_only = "--list" in sys.argv
root = os.path.abspath(args[0]) if args else os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
allow_path = os.path.join(root, "docs/sprint/SEC-DID7-ALLOWLIST.tsv")
SKIP_DIRS = {"target", ".git", "__pycache__", "node_modules", "docs", "scripts", "tests", "examples", "benches", "bin"}
TOKENS = {
    "Command::new": re.compile(r"\bCommand::new\s*\("),
    "process::Command": re.compile(r"\bprocess::Command\b"),
    "exec": re.compile(r"\bexec(?:v|ve|vp|vpe|l|le|lp)\s*\("),
    "posix_spawn": re.compile(r"\bposix_spawnp?\s*\("),
    "libc::fork": re.compile(r"\blibc::fork\s*\("),
}
CFG_TEST = re.compile(r"#\[cfg\(test\)\]")
STRING = re.compile(r'"(?:\\.|[^"\\])*"')


def code_only(line):
    """Strip string literals and trailing // comments so only code is matched."""
    return re.sub(r"//.*$", "", STRING.sub('""', line))


def shipped_lines(path):
    """Yield (lineno, code) for lines outside any #[cfg(test)] item."""
    skipping = False      # inside a cfg(test) item
    pending = False       # saw #[cfg(test)], waiting for the item's opening brace or ';'
    depth = 0
    with open(path, encoding="utf-8", errors="replace") as f:
        for lineno, raw in enumerate(f, 1):
            code = code_only(raw)
            if not skipping and CFG_TEST.search(raw):
                pending = True
                continue
            if pending and not skipping:
                if code.strip().startswith("#["):
                    continue
                if "{" in code:
                    skipping, pending, depth = True, False, 0
                elif ";" in code:            # #[cfg(test)] use ...;  -- one line item
                    pending = False
                    continue
            if skipping:
                depth += code.count("{") - code.count("}")
                if depth <= 0 and ("}" in code):
                    skipping = False
                continue
            yield lineno, code


def scanned_files():
    """Tracked .rs files (gitignored vendored trees like mozjs/ and stylo/ are not shipped), else a walk."""
    try:
        out = subprocess.run(["git", "-C", root, "ls-files", "-z", "--", "*.rs"], capture_output=True, check=True).stdout
        files = [x for x in out.decode().split("\0") if x]
    except (OSError, subprocess.CalledProcessError):
        files = []
        for dirpath, dirnames, filenames in os.walk(root):
            dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS]
            files += [os.path.relpath(os.path.join(dirpath, fn), root) for fn in filenames if fn.endswith(".rs")]
    keep = []
    for rel in files:
        parts = rel.split("/")
        if any(p in SKIP_DIRS for p in parts[:-1]) or "bin" in parts[:-1]:
            continue
        keep.append(rel)
    return keep


hits = {}
for rel in scanned_files():
    for lineno, code in shipped_lines(os.path.join(root, rel)):
        for tok, rx in TOKENS.items():
            if rx.search(code):
                hits.setdefault((rel, tok), []).append(lineno)

if list_only:
    for (rel, tok), lines in sorted(hits.items()):
        print(f"{rel}\t{tok}\t{len(lines)}\tlines {','.join(map(str, lines[:5]))}")
    sys.exit(0)

allowed = {}
for line in open(allow_path):
    if not line.strip() or line.startswith("#"):
        continue
    path, tok, count = line.rstrip("\n").split("\t")[:3]
    allowed[(path, tok)] = int(count)

bad = []
for (rel, tok), lines in sorted(hits.items()):
    want = allowed.get((rel, tok))
    if want is None:
        bad.append(f"{rel} calls {tok!r} at line {lines[0]} ({len(lines)} hit(s)) with no allowlist row")
    elif want != len(lines):
        bad.append(f"{rel} {tok!r}: {len(lines)} hit(s), allowlist says {want} (lines {lines[:5]})")
# A stale row is harmless but means the inventory drifted; report it so it gets pruned.
for (path, tok) in sorted(allowed):
    if (path, tok) not in hits:
        bad.append(f"stale allowlist row: {path} {tok!r} no longer occurs")

for b in bad:
    print("RED", b)
print(f"{len(hits)} (file, token) exec sites in shipped code, {len(allowed)} allowlisted, {len(bad)} problems")
sys.exit(1 if bad else 0)
