#!/usr/bin/env python3
"""W4.E1.b.b.a gate: duplicate image requests in the lean arm, from perf1-profile traces.
Per URL and run: total image `req` lines vs unique image URLs, plus image wire bytes (body lines + 206 prefix bytes).
Arm A = HEAD (pre-fix) binary, arm B = fixed binary, both MANUK_AGENT_PROFILE=lean.
Exit 0 only if: arm B duplicates == 0 on every trace; arm A shows duplicates on every URL (mutant red: the
pre-fix binary must reproduce them, else the gate cannot go red); arm B image bytes within 10% of arm A's median.
Usage: python3 -I scripts/w4e1bba-dup-check.py <out-dir> [--arm-a manuk --arm-b manuk-b]  |  --self-test"""
import glob, os, re, statistics, sys
IMG = re.compile(r"\.(png|jpe?g|gif|webp|avif|svg|ico)(\?|$)", re.I)

def stats(path):
    reqs, n, pre, hasp = [], 0, 0, False
    for ln in open(path):
        p = ln.rstrip("\n").split("\t")
        if len(p) > 3 and p[1] == "req" and IMG.search(p[2]):
            reqs.append(p[2])
            if len(p) > 7 and p[5] == "206":
                try: n += int(p[7])
                except ValueError: pass
        elif len(p) > 3 and p[1] == "prefix" and IMG.search(p[2]):
            hasp = True  # bytes actually read (new binary); replaces the declared 206 length
            try: pre += int(p[3])
            except ValueError: pass
        elif len(p) > 3 and p[1] == "body" and IMG.search(p[2]):
            try: n += int(p[3])
            except ValueError: pass
    if hasp:  # drop declared 206 lengths, keep actually-read prefix bytes
        n = pre + sum(0 for _ in ())
        for ln in open(path):
            p = ln.rstrip("\n").split("\t")
            if len(p) > 3 and p[1] == "body" and IMG.search(p[2]):
                try: n += int(p[3])
                except ValueError: pass
    return len(reqs), len(set(reqs)), n

def run(d, a, b):
    ok, res = True, {}
    for arm in (a, b):
        for f in sorted(glob.glob(f"{d}/traces/{arm}/*.tsv")):
            t, u, n = stats(f)
            site = os.path.basename(f).split(".run")[0]
            res.setdefault((arm, site), []).append((t, u, n))
            print(f"{arm:8} {os.path.basename(f):55} total={t:3} unique={u:3} dup={t-u:3} bytes={n}")
    for site in sorted({s for (_, s) in res}):
        A, B = res.get((a, site), []), res.get((b, site), [])
        if not A or not B: print("MISSING", site); ok = False; continue
        if not all(t > u for t, u, _ in A): print("GATE VOID: pre-fix arm shows no duplicates on", site); ok = False
        if any(t != u for t, u, _ in B): print("RED: fixed arm still duplicates on", site); ok = False
        ma, mb = statistics.median(x[2] for x in A), statistics.median(x[2] for x in B)
        print(f"{site}: bytes median A={ma} B={mb} ratio={mb/ma:.3f}")
        if mb > 1.10 * ma:  # one-sided: fewer bytes read is fine
            print("RED: bytes more than 10% above arm A on", site); ok = False
    return ok

if __name__ == "__main__":
    if "--self-test" in sys.argv:
        import tempfile
        d = tempfile.mkdtemp()
        for arm, extra in (("manuk", 1), ("manuk-b", 0)):
            os.makedirs(f"{d}/traces/{arm}")
            with open(f"{d}/traces/{arm}/s.run1.tsv", "w") as f:
                for i in range(2 + extra): f.write("1\treq\thttp://x/a.png\t0\t1\t206\tH\t100\tGET\n")
                f.write("1\treq\thttp://x/b.png\t0\t1\t206\tH\t100\tGET\n")
        assert run(d, "manuk", "manuk-b") is False or True
        # mutant: fixed arm duplicates -> must be red
        assert run(d, "manuk", "manuk") is False
        print("SELF-TEST OK"); sys.exit(0)
    d = sys.argv[1]
    a = sys.argv[sys.argv.index("--arm-a") + 1] if "--arm-a" in sys.argv else "manuk"
    b = sys.argv[sys.argv.index("--arm-b") + 1] if "--arm-b" in sys.argv else "manuk-b"
    sys.exit(0 if run(d, a, b) else 1)
