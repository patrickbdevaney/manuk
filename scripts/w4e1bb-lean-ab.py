#!/usr/bin/env python3
"""W4.E1.b.b: paired lean (MANUK_AGENT_PROFILE=lean) vs full, from perf1-profile's paired.jsonl plus the
per-run net traces it keeps (<out>/traces/<arm>/<slug>.run<N>.tsv).

Per URL it reports the subres_fetch band (min..max over OK runs) for each arm and the image BODY bytes per arm
(full image bodies plus the 206 ranged header-read prefixes lean issues instead of bodies).

Gate (must be able to go red):
  * treatment-applied: on every URL where both arms have image bytes, the lean arm must carry FEWER image
    body bytes than the full arm; otherwise the env var did not reach the binary and the A/B is void (rc 2).
  * sample: a URL needs 3 OK runs in both arms to get a band verdict; fewer -> INSUFFICIENT (reported, no verdict).
  * band rule: a difference is claimed only if the bands are disjoint (B max < A min, or A max < B min).
Usage:  python3 -I scripts/w4e1bb-lean-ab.py <out-dir> [--arm-a manuk --arm-b manuk-b]
        python3 -I scripts/w4e1bb-lean-ab.py --self-test
"""
import collections, os, re, statistics, sys

IMG = re.compile(r"\.(png|jpe?g|gif|webp|avif|svg|ico)(\?|$)", re.I)


def trace_slug(url):
    return "".join(ch if ch.isalnum() else "_" for ch in url)[:80]


def image_body_bytes(trace_path):
    """Image bytes that came off the wire in one trace (None if the trace is missing): full bodies (`body`
    lines) plus the prefix a 206 ranged header read returned (`req` lines, status 206, field index 7 = bytes)."""
    if not os.path.exists(trace_path):
        return None
    n = 0
    with open(trace_path) as f:
        for ln in f:
            p = ln.rstrip("\n").split("\t")
            if len(p) > 3 and p[1] == "body" and IMG.search(p[2]):
                try:
                    n += int(p[3])
                except ValueError:
                    pass
            elif len(p) > 7 and p[1] == "req" and p[5] == "206" and IMG.search(p[2]):
                try:
                    n += int(p[7])
                except ValueError:
                    pass
    return n


def analyze(rows, trace_dir, arm_a, arm_b):
    """rows: dicts from paired.jsonl. Returns (report_lines, exit_code): 0 settled, 2 NOT_SETTLED."""
    sub = collections.defaultdict(lambda: collections.defaultdict(list))   # url -> arm -> [subres_fetch]
    img = collections.defaultdict(lambda: collections.defaultdict(list))   # url -> arm -> [image body bytes]
    for r in rows:
        if r.get("status") != "OK":
            continue
        sub[r["url"]][r["engine"]].append(r["phases_s"]["subres_fetch"])
        tp = os.path.join(trace_dir, r["engine"], f"{trace_slug(r['url'])}.run{r['run']}.tsv")
        b = image_body_bytes(tp)
        if b is not None:
            img[r["url"]][r["engine"]].append(b)
    out, rc, not_applied, insufficient = [], 0, [], []
    for url in sorted(sub):
        a, b = sub[url][arm_a], sub[url][arm_b]
        ia, ib = img[url][arm_a], img[url][arm_b]
        mia = statistics.median(ia) if ia else None
        mib = statistics.median(ib) if ib else None
        if mia and mib is not None and not mib < mia:
            not_applied.append(url)
        if len(a) < 3 or len(b) < 3:
            insufficient.append(url)
            out.append(f"{url}: INSUFFICIENT (OK runs {arm_a}={len(a)} {arm_b}={len(b)}, need 3 each); "
                       f"img body MB {arm_a}={mia and mia/1e6:.2f} {arm_b}={mib and mib/1e6:.2f}")
            continue
        if max(b) < min(a):
            band = "B faster (bands disjoint)"
        elif max(a) < min(b):
            band = "A faster (bands disjoint)"
        else:
            band = "bands overlap: no difference claimed"
        out.append(f"{url}: subres_fetch {arm_a} {min(a):.2f}..{max(a):.2f} med {statistics.median(a):.2f} | "
                   f"{arm_b} {min(b):.2f}..{max(b):.2f} med {statistics.median(b):.2f} -> {band}; "
                   f"img body MB {arm_a}={mia and mia/1e6:.2f} {arm_b}={mib and mib/1e6:.2f}")
    if not_applied:
        out.append("TREATMENT NOT APPLIED (lean arm did not carry fewer image body bytes) on: " + ", ".join(not_applied))
        rc = 2
    if insufficient:
        out.append("SAMPLE SHORT on: " + ", ".join(insufficient) + " (no band verdict on those URLs)")
    out.append("VERDICT: " + ("GATE VOID" if rc else ("NOT_SETTLED" if insufficient else "SETTLED")))
    return out, rc


def self_test():
    import tempfile
    d = tempfile.mkdtemp(prefix="w4e1bb-st-")
    url = "https://x.test/"
    slug = trace_slug(url)

    def write(arm, run, img_bytes):
        os.makedirs(os.path.join(d, "traces", arm), exist_ok=True)
        with open(os.path.join(d, "traces", arm, f"{slug}.run{run}.tsv"), "w") as f:
            f.write(f"1\treq\thttps://x.test/app.js\t0\t0\t200\tHTTP/2.0\t-\tGET\n")
            f.write(f"2\tbody\thttps://x.test/app.js\t5000\n")
            if img_bytes:
                f.write(f"3\tbody\thttps://x.test/a.png\t{img_bytes}\n")

    rows = []
    for run in (1, 2, 3):
        for eng, sub, ib in (("manuk", 5.0 + run * 0.1, 900000), ("manuk-b", 2.0 + run * 0.1, 100000)):
            write(eng, run, ib)
            rows.append({"engine": eng, "url": url, "status": "OK", "run": run,
                         "phases_s": {"subres_fetch": sub}})
    rep, rc = analyze(rows, os.path.join(d, "traces"), "manuk", "manuk-b")
    ok = rc == 0 and any("B faster" in l for l in rep)
    print("\n".join(rep))
    # mutant 1: the env did not reach arm B -> identical image bytes must go red
    for run in (1, 2, 3):
        write("manuk-b", run, 900000)
    rep2, rc2 = analyze(rows, os.path.join(d, "traces"), "manuk", "manuk-b")
    ok &= rc2 == 2 and any("TREATMENT NOT APPLIED" in l for l in rep2)
    print("mutant (no treatment):", rep2[-2])
    # mutant 2: only 2 OK runs in an arm -> must not produce a verdict
    for run in (1, 2, 3):
        write("manuk-b", run, 100000)
    rows2 = [r for r in rows if not (r["engine"] == "manuk" and r["run"] == 3)]
    rep3, rc3 = analyze(rows2, os.path.join(d, "traces"), "manuk", "manuk-b")
    ok &= rc3 == 0 and any("INSUFFICIENT" in l and "no band" not in l for l in rep3) and not any("B faster" in l or "A faster" in l for l in rep3)
    print("mutant (n=2):", rep3[0])
    print("SELF-TEST", "OK" if ok else "FAIL")
    return 0 if ok else 1


def main():
    if "--self-test" in sys.argv:
        return self_test()
    import json
    out_dir = sys.argv[1]
    arm_a = sys.argv[sys.argv.index("--arm-a") + 1] if "--arm-a" in sys.argv else "manuk"
    arm_b = sys.argv[sys.argv.index("--arm-b") + 1] if "--arm-b" in sys.argv else "manuk-b"
    rows = [json.loads(l) for l in open(os.path.join(out_dir, "paired.jsonl"))]
    pin = os.environ.get("MANUK_CHROME_PIN")
    if pin:
        rows = [r for r in rows if r.get("chrome_ver") == pin]
    rep, rc = analyze(rows, os.path.join(out_dir, "traces"), arm_a, arm_b)
    print("\n".join(rep))
    return rc


if __name__ == "__main__":
    sys.exit(main())
