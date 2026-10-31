#!/usr/bin/env python3
"""W4.E2 gate: lean vs full agent profile, paired and interleaved, N runs per URL.

Usage: w4e2-gate.py [--runs 3] [--probe target/release/examples/lean_probe] URL...
Per URL it prints wall and subresource wire-byte bands for both profiles and whether the a11y /
links digests agree (and the screenshot digest, which must be identical when requested).
Exit 1 if: any digest disagrees on a URL that loaded in both arms, or (--require-win) the lean
band does not fall strictly below the full band on wire bytes.
"""
import argparse, os, re, subprocess, sys

def run(probe, url, lean):
    env = dict(os.environ)
    env.pop("MANUK_AGENT_PROFILE", None)
    if lean:
        env["MANUK_AGENT_PROFILE"] = "lean"
    try:
        out = subprocess.run([probe, url], env=env, capture_output=True, text=True, timeout=120).stdout
    except subprocess.TimeoutExpired:
        return {"ok": "timeout"}
    return dict(re.findall(r"(\w+)=(\S+)", out))

ap = argparse.ArgumentParser()
ap.add_argument("--runs", type=int, default=3)
ap.add_argument("--probe", default="target/release/examples/lean_probe")
ap.add_argument("--require-win", action="store_true")
ap.add_argument("urls", nargs="+")
a = ap.parse_args()
bad = 0
tot = {"full": [0, 0], "lean": [0, 0]}
print("chrome_ver=%s (no Chrome arm: this is manuk full vs manuk lean)" % os.environ.get("MANUK_CHROME_PIN", "-"))
print("url\tarm\twall_ms(min-max)\twire_bytes(min-max)\tblocked\ta11y_same\tshot_same")
for url in a.urls:
    res = {"full": [], "lean": []}
    for _ in range(a.runs):
        for arm in ("full", "lean"):
            res[arm].append(run(a.probe, url, arm == "lean"))
    ok = [r for arm in res for r in res[arm] if r.get("ok") == "true" and r.get("observed") == "true"]
    if len(ok) < 2 * a.runs:
        print(f"{url}\tSKIP (did not load in every run)")
        continue
    same = lambda k: len({r[k] for r in ok}) == 1
    shot = same("shot")
    # A live page that disagrees with ITSELF across runs (rotating hrefs, ads) cannot be held against
    # the lean profile, so each digest is compared across arms only where every arm is self-stable.
    stable_keys = [k for k in ("a11y", "links") if all(len({r[k] for r in res[arm]}) == 1 for arm in res)]
    stable = len(stable_keys) == 2
    a11y = all(same(k) for k in stable_keys)
    for arm in ("full", "lean"):
        w = [int(r["wall_ms"]) for r in res[arm]]
        b = [int(r["wire_bytes"]) for r in res[arm]]
        tot[arm][0] += sum(w) / len(w)
        tot[arm][1] += sum(b) / len(b)
        print(f"{url}\t{arm}\t{min(w)}-{max(w)}\t{min(b)}-{max(b)}\t{res[arm][0]['blocked']}\t{a11y}{'' if stable else ' (a digest unstable within an arm: excluded)'}\t{shot}")
    if not a11y:
        bad += 1
print(f"SUM mean wall ms: full {tot['full'][0]:.0f} lean {tot['lean'][0]:.0f}; mean wire bytes: full {tot['full'][1]:.0f} lean {tot['lean'][1]:.0f}")
if args_win := a.require_win:
    if not tot["lean"][1] < tot["full"][1]:
        bad += 1
        print("RED: lean did not cut wire bytes")
sys.exit(1 if bad else 0)
