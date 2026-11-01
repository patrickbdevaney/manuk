#!/usr/bin/env python3
"""W4.E2.b: peak summed PSS of manuk-mcp, lean vs full profile, at actionable and with a screenshot.

Matrix: 10 baseline URLs x {full, lean} x {actionable, shot} x RUNS. Every run is a fresh manuk-mcp
process with one page. `actionable` = navigate + observe; `shot` = navigate + observe + screenshot.
The two modes are separate processes so the screenshot path cannot change the actionable number.
A band is min-max PSS (MB) over the OK runs of one (url, mode, arm).

Verdict per (url, mode), read from the band only:
  LEAN_LOWER   lean max < full min   -> the only case in which a memory claim is made
  LEAN_HIGHER  lean min > full max
  OVERLAP      bands touch           -> no claim
  INCOMPLETE   fewer than RUNS OK runs in an arm -> no claim

Gate (exit 1 = RED): a canary must show the lean profile is actually engaged. On the W4.E2 fixture
(12 noisy PNGs), `lean_probe` wire bytes under lean must be < 0.5 x full. `--mutant-canary` strips
MANUK_AGENT_PROFILE from the lean arm; it must come out RED. Matrix results never turn the gate red.

Usage (run with -I so no local module can shadow the stdlib):
  MANUK_CHROME_PIN=145.0.7632.116 python3 -I scripts/w4e2b-memory.py --out docs/sprint/w4e2b
  python3 -I scripts/w4e2b-memory.py --canary-only
  python3 -I scripts/w4e2b-memory.py --canary-only --mutant-canary     # must exit 1
  python3 -I scripts/w4e2b-memory.py --selftest
Needs target/release/manuk-mcp and target/release/examples/lean_probe.
"""
import argparse
import importlib.util
import os
import re
import socket
import statistics
import subprocess
import sys
import tempfile
import time
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
URLS = os.path.join(ROOT, "docs", "sprint", "BASELINE-URLS.txt")
PROBE = os.path.join(ROOT, "target", "release", "examples", "lean_probe")
FIXTURE = os.path.join(HERE, "w4e2-fixture.py")
PIN = os.environ.get("MANUK_CHROME_PIN", "")
RUNS = 3
LOG_ROOT = tempfile.mkdtemp(prefix="w4e2b-logs-")

# w4e0-footprint.py has the MCP stdio client, the PSS sampler and the challenge check. Reuse, do not fork it.
_spec = importlib.util.spec_from_file_location("w4e0", os.path.join(HERE, "w4e0-footprint.py"))
w0 = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(w0)


def touch():
    """The watchdog reads this mtime to see the sprint agent is alive."""
    w0.touch()


def load_urls():
    with open(URLS) as f:
        return [ln.strip() for ln in f if ln.strip() and not ln.startswith("#")]


def static_img_tags(url):
    """Count of <img> tags in the raw HTML (before any JS). A rough image-heaviness label, not a render count."""
    try:
        req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0 (X11; Linux x86_64)"})
        with urllib.request.urlopen(req, timeout=20) as r:
            html = r.read(2_000_000).decode("utf-8", "replace")
        return str(len(re.findall(r"<img\b", html, re.I)))
    except Exception as e:  # network is live; a dead host is recorded, not fatal
        return f"ERR:{type(e).__name__}"


def one_run(url, lean, shot):
    """One fresh manuk-mcp process. Returns (peak_pss_kb, wall_s, status)."""
    env = w0.mcp_env(tempfile.mkdtemp(prefix="w4e2b-log-", dir=LOG_ROOT))
    env.pop("MANUK_AGENT_PROFILE", None)
    if lean:
        env["MANUK_AGENT_PROFILE"] = "lean"
    m = w0.Mcp(env)
    sam = w0.Sampler(lambda: [m.p.pid])
    status, wall, peak = "OK", 0.0, 0
    try:
        sam.start()
        t0 = time.monotonic()
        err, text, _ = m.call("navigate", {"url": url})
        if err:
            status = "FAIL"
        elif w0.challenge_marker(text):
            status = "CHALLENGE"
        if status == "OK":
            err, _, _ = m.call("observe", {})
            status = "FAIL_OBSERVE" if err else status
        if status == "OK" and shot:
            err, _, _ = m.call("screenshot", {})
            status = "FAIL_SHOT" if err else status
        wall = time.monotonic() - t0
        time.sleep(w0.SETTLE_S)  # same settle for both arms, before the peak is read
    except (TimeoutError, OSError):
        status = "TIMEOUT"
    finally:
        peak = sam.finish()
        m.kill()
    return peak, wall, status


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def canary(mutant=False):
    """True if lean cuts wire bytes on the fixture to < 0.5 x full. Prints the numbers either way."""
    port = free_port()
    srv = subprocess.Popen([sys.executable, "-I", FIXTURE, str(port)])
    try:
        time.sleep(0.5)
        url = f"http://127.0.0.1:{port}/index.html"
        wire = {}
        for arm in ("full", "lean"):
            env = dict(os.environ)
            env.pop("MANUK_AGENT_PROFILE", None)
            if arm == "lean" and not mutant:
                env["MANUK_AGENT_PROFILE"] = "lean"
            out = subprocess.run([PROBE, url], env=env, capture_output=True, text=True, timeout=120).stdout
            kv = dict(re.findall(r"(\w+)=(\S+)", out))
            wire[arm] = int(kv.get("wire_bytes", "-1"))
            print(f"canary arm={arm} ok={kv.get('ok')} wire_bytes={wire[arm]} blocked={kv.get('blocked')}")
    finally:
        srv.kill()
        srv.wait()
    ok = wire["full"] > 0 and 0 <= wire["lean"] < 0.5 * wire["full"]
    print(f"canary {'GREEN' if ok else 'RED'}: lean/full wire = {wire['lean']}/{wire['full']}"
          f"{' (mutant: lean env stripped)' if mutant else ''}")
    return ok


def verdict(full, lean):
    """Band verdict from the PSS lists (MB). Only LEAN_LOWER is a memory claim."""
    if len(full) < RUNS or len(lean) < RUNS:
        return "INCOMPLETE"
    if max(lean) < min(full):
        return "LEAN_LOWER"
    if min(lean) > max(full):
        return "LEAN_HIGHER"
    return "OVERLAP"


def band(xs):
    return f"{min(xs):.0f}-{max(xs):.0f}" if xs else "-"


def write_tsv(path, header, rows):
    with open(path, "w") as f:
        f.write("\t".join(header) + "\n")
        for r in rows:
            f.write("\t".join(str(r[h]) for h in header) + "\n")


def summarize(rows, out):
    header = ["url", "mode", "full_pss_mb", "lean_pss_mb", "full_median_mb", "lean_median_mb", "verdict", "static_img_tags"]
    summary = []
    urls = list(dict.fromkeys(r["url"] for r in rows))
    for url in urls:
        imgs = next(r["img_tags"] for r in rows if r["url"] == url)
        for mode in ("actionable", "shot"):
            pts = {arm: [r["pss_mb"] for r in rows if r["url"] == url and r["mode"] == mode
                         and r["arm"] == arm and r["status"] == "OK"] for arm in ("full", "lean")}
            v = verdict(pts["full"], pts["lean"])
            med = {a: f"{statistics.median(x):.0f}" if x else "-" for a, x in pts.items()}
            summary.append({"url": url, "mode": mode, "full_pss_mb": band(pts["full"]),
                            "lean_pss_mb": band(pts["lean"]), "full_median_mb": med["full"],
                            "lean_median_mb": med["lean"], "verdict": v, "static_img_tags": imgs})
    write_tsv(os.path.join(out, "summary.tsv"), header, summary)
    print("\t".join(header))
    for s in summary:
        print("\t".join(str(s[h]) for h in header))
    claims = [s for s in summary if s["verdict"] == "LEAN_LOWER"]
    print(f"memory claims (LEAN_LOWER bands): {len(claims)} of {len(summary)} url-mode cells; chrome_ver={PIN or '-'}")


def selftest():
    assert verdict([100, 110, 120], [50, 60, 70]) == "LEAN_LOWER"
    assert verdict([100, 110, 120], [105, 115, 125]) == "OVERLAP"
    assert verdict([100, 110, 120], [130, 140, 150]) == "LEAN_HIGHER"
    # mutant input: an arm with too few OK runs must not be read as a claim
    assert verdict([100, 110, 120], [50, 60]) == "INCOMPLETE"
    print("selftest GREEN")
    return True


def main():
    global RUNS
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=os.path.join(ROOT, "docs", "sprint", "w4e2b"))
    ap.add_argument("--runs", type=int, default=RUNS)
    ap.add_argument("--canary-only", action="store_true")
    ap.add_argument("--mutant-canary", action="store_true", help="gate check: lean env stripped, must be RED")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()
    if a.selftest:
        return 0 if selftest() else 1
    if a.canary_only or a.mutant_canary:
        return 0 if canary(mutant=a.mutant_canary) else 1
    RUNS = a.runs
    if not PIN:
        print("MANUK_CHROME_PIN is not set; refusing to write unpinned rows", file=sys.stderr)
        return 1
    os.makedirs(a.out, exist_ok=True)
    if not canary():
        print("canary RED: the lean profile is not engaged; no memory numbers are written", file=sys.stderr)
        return 1
    urls = load_urls()
    img = {u: static_img_tags(u) for u in urls}
    rows = []
    for run in range(1, RUNS + 1):
        for url in urls:
            for mode in ("actionable", "shot"):
                for arm in ("full", "lean"):
                    peak, wall, st = one_run(url, arm == "lean", mode == "shot")
                    rows.append({"url": url, "mode": mode, "arm": arm, "run": run, "status": st,
                                 "pss_kb": peak, "pss_mb": round(peak / 1024, 1), "wall_s": round(wall, 2),
                                 "img_tags": img[url], "chrome_ver": PIN})
                    print(f"run{run} {mode:10} {arm:4} {st:9} {peak/1024:7.1f}MB {wall:6.2f}s {url}", file=sys.stderr)
                    touch()
    write_tsv(os.path.join(a.out, "runs.tsv"),
              ["url", "mode", "arm", "run", "status", "pss_kb", "pss_mb", "wall_s", "img_tags", "chrome_ver"], rows)
    summarize(rows, a.out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
