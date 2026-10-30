#!/usr/bin/env python3
"""W3.18 gate 1: does the backend boot REAL's 11 v1 web clones? One TSV row per clone; exit 1 when booted < --need.

  real_boot.py [--backend manuk|chrome] [--need 10] [--wait 6] [--out FILE]

booted = after --wait seconds the observation has no 'Application error' / 'DMCA Takedown' text and is >= 400 chars
(a blank or error shell is shorter). The clone list and URLs come from the pinned agisdk 0.3.5 v1 task set.
Run with `python3 -I`. Requires MANUK_CHROME_PIN.
"""
import argparse, json, os, subprocess, sys, time

SITES = "dashdish fly-unified gocalendar gomail networkin omnizon opendining staynb topwork udriver zilloft".split()
EXE = os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "target/release/manuk-mcp")


class M:
    def __init__(s, args, env):
        s.p = subprocess.Popen([EXE, *args], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, env=env)
        s.i = 0
        s.rpc("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "real_boot", "version": "0"}})
        s.p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n"); s.p.stdin.flush()

    def rpc(s, m, pa):
        s.i += 1
        s.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": s.i, "method": m, "params": pa}) + "\n"); s.p.stdin.flush()
        while True:
            l = s.p.stdout.readline()
            if not l:
                return None
            j = json.loads(l)
            if j.get("id") == s.i:
                return j

    def call(s, n, a):
        r = s.rpc("tools/call", {"name": n, "arguments": a})
        return "\n".join(c.get("text", "") for c in r["result"]["content"]) if r and "result" in r else ""


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--backend", default="manuk"); ap.add_argument("--need", type=int, default=10)
    ap.add_argument("--wait", type=float, default=6); ap.add_argument("--out")
    a = ap.parse_args()
    pin = os.environ.get("MANUK_CHROME_PIN")
    if not pin:
        sys.exit("refusing: MANUK_CHROME_PIN unset")
    env = dict(os.environ)
    args = []
    if a.backend == "chrome":
        args = ["--backend", "chrome"]; env["MANUK_CHROME"] = os.path.expanduser("~/.local/chrome-cft/bin/google-chrome")
    rows, ok = [], 0
    for s in SITES:
        m = M(args, env)
        m.call("navigate", {"url": f"https://evals-{s}.vercel.app"}); time.sleep(a.wait)
        o = m.call("observe", {}); m.p.kill()
        why = "Application error" if "Application error" in o else "DMCA takedown" if "DMCA" in o else "short observation" if len(o) < 400 else ""
        ok += not why
        rows.append((s, a.backend, pin, len(o), "BOOT" if not why else "FAIL", why))
        print("\t".join(map(str, rows[-1])), flush=True)
        try: os.utime(os.path.join(os.path.dirname(EXE), "../../.git/sprint-working"), None)
        except OSError: pass
    print(f"booted {ok}/{len(SITES)} on {a.backend} (need {a.need})")
    if a.out:
        with open(a.out, "w") as f:
            f.write("site\tbackend\tchrome_ver\tobs_len\tverdict\treason\n" + "\n".join("\t".join(map(str, r)) for r in rows) + "\n")
    sys.exit(0 if ok >= a.need else 1)


if __name__ == "__main__":
    main()
