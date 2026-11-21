#!/usr/bin/env python3
"""RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a.a: re-measure the 13-step ordinal replay (rg3baaaaaaa_match.py) 3x after the
Chrome backend's pointer:fine fix, record for each run and backend what opendining-4 step #7 (the click on 'Select
time') did, and summarise with the existing spread check (rg3baaaaaaa_spread.py, thresholds untouched).
  rg3baaaaaaaaaaaaaa_remeasure.py [--dir DIR]           run 3x (~10 min each), then check
  rg3baaaaaaaaaaaaaa_remeasure.py --check [--dir DIR]   only check/summarise existing runs (no replay)
Runs land in DIR (default docs/sprint/rg/RG3baaaaaaaaaaaaaa/) as RG3baaaaaaa-match-runN.{md,json} (the names
rg3baaaaaaa_spread.py reads) plus step7-runN.json. Writes DIR/summary.md.
Gate (exit 1 = red, a broken run): a run missing/unreadable, chrome_ver != $MANUK_CHROME_PIN, != 13 table rows per
backend, table match count != json count, or no step-#7 record for a backend. The spread script's own verdict (its
pre-registered MIN_MANUK/MIN_CHROME/agreement bars) is reported, not used as this gate. Run with `python3 -I`."""
import argparse, json, os, re, shutil, subprocess, sys
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
RG = os.path.join(ROOT, "docs/sprint/rg")
ROW = re.compile(r"^\| (\S+) \| (manuk|chrome) \| #(\d+) \| `[^`]*` \| `([^`]*)` \| (yes|no) \|$", re.M)
STEP = 7
LAST = [None]


def one_run(n, d):
    """Run rg3baaaaaaa_match.py in-process with r.replay wrapped so the opendining-4 step-#7 log entry is kept."""
    import rg3baaaaaaa_match as mm
    s7, orig, calls = {}, mm.r.replay, [0]

    def wrap(side, seq, wait, **kw):
        task = mm.TASKS[calls[0] // 2]; calls[0] += 1  # main() replays TASKS in order, manuk then chrome
        log = orig(side, seq, wait, **kw)
        if task == "opendining-4":
            e = next((x for x in log if x[0] == STEP), None)
            raw = kw.get("raw") or []
            gap = any(x[0] == STEP and x[3] for x in raw)
            s7[side] = {"step": STEP, "tool": seq[STEP][0], "rn": list(seq[STEP][2]) if seq[STEP][2] else None,
                        "resolved": not gap and e is not None and not str(e[2]).startswith("unresolved"),
                        "result": (str(e[2]) if e else "(no log entry)")[:260],
                        "occluded": bool(e and "OCCLUDED" in str(e[2]).upper())}
        return log
    mm.r.replay = wrap
    oc = mm.r.rb.M.call

    def call(self, name, a):  # remember the last MCP call so a hang names its step
        LAST[0] = (name, json.dumps(a)[:160]); return oc(self, name, a)
    mm.r.rb.M.call = call
    sys.argv = [os.path.join(HERE, "rg3baaaaaaa_match.py")]
    try:
        for attempt in (1, 2, 3):
            s7.clear(); calls[0] = 0
            try:
                mm.main()
            except SystemExit as e:
                if e.code not in (0, None):
                    print(f"run{n}: match script exit {e.code}")
            except TimeoutError as e:  # a hung MCP server is a broken run: retry, report, never count it
                print(f"run{n} attempt {attempt}: {e}; last call {LAST[0]}", flush=True)
                if attempt == 3:
                    raise
                continue
            break
    finally:
        mm.r.replay = orig; mm.r.rb.M.call = oc
    for e in ("md", "json"):
        shutil.copy(os.path.join(RG, f"RG3baaaaaaa-match.{e}"), os.path.join(d, f"RG3baaaaaaa-match-run{n}.{e}"))
    json.dump(s7, open(os.path.join(d, f"step7-run{n}.json"), "w"), indent=1)


def main():
    ap = argparse.ArgumentParser(); ap.add_argument("--check", action="store_true")
    ap.add_argument("--dir", default=os.path.join(RG, "RG3baaaaaaaaaaaaaa"))
    ap.add_argument("--label", default="RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a.a")
    ap.add_argument("--baseline", default="Prior baseline (before the fix): manuk 9/13 match 10/13 resolved, chrome 12/13 match 13/13.")
    ap.add_argument("--what", default="after the Chrome backend pointer:fine fix"); a = ap.parse_args()
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    os.makedirs(a.dir, exist_ok=True)
    if not a.check:
        for n in (1, 2, 3):
            one_run(n, a.dir)
    errs, sums = [], []
    for n in (1, 2, 3):
        jp, mp, sp = (os.path.join(a.dir, f) for f in (f"RG3baaaaaaa-match-run{n}.json", f"RG3baaaaaaa-match-run{n}.md", f"step7-run{n}.json"))
        if not all(os.path.exists(p) for p in (jp, mp, sp)):
            errs.append(f"run{n}: missing"); continue
        try:
            j, rows, s7 = json.load(open(jp)), ROW.findall(open(mp).read()), json.load(open(sp))
        except (ValueError, OSError) as e:
            errs.append(f"run{n}: unreadable ({e})"); continue
        if j.get("chrome_ver") != pin:
            errs.append(f"run{n}: chrome_ver {j.get('chrome_ver')!r} != {pin!r}")
        c = {}
        for s in ("manuk", "chrome"):
            rs = [r for r in rows if r[1] == s]
            m, res = sum(r[4] == "yes" for r in rs), sum(r[3] != "-" for r in rs)
            if len(rs) != 13: errs.append(f"run{n}: {s} {len(rs)} rows != 13")
            if m != j.get("counts", {}).get(s): errs.append(f"run{n}: {s} table {m} != json {j.get('counts', {}).get(s)}")
            if s not in s7: errs.append(f"run{n}: no step-#{STEP} record for {s}")
            od = [(r[2], r[3], r[4]) for r in rs if r[0] == "opendining-4" and r[2] in ("13", "14")]
            c[s] = (m, res, od, s7.get(s, {}))
        sums.append((n, c))
    if errs:
        print("GATE FAIL:"); [print("  " + e) for e in errs]; sys.exit(1)
    sp = subprocess.run([sys.executable, "-I", os.path.join(HERE, "rg3baaaaaaa_spread.py"), "--dir", a.dir],
                        capture_output=True, text=True)
    out = ["## " + a.label, "",
           f"chrome_ver={pin}. Three runs of `rg3baaaaaaa_match.py` {a.what} (13 ordinal "
           f"steps per backend). {a.baseline}", "",
           "| run | manuk match | manuk resolved | chrome match | chrome resolved |", "|---|---|---|---|---|"]
    for n, c in sums:
        out.append(f"| run{n} | {c['manuk'][0]}/13 | {c['manuk'][1]}/13 | {c['chrome'][0]}/13 | {c['chrome'][1]}/13 |")
    out += ["", f"opendining-4 step #{STEP} (click on 'Select time') and the dropdown ordinals #13/#14 it feeds:", ""]
    for n, c in sums:
        for s in ("manuk", "chrome"):
            v = c[s][3]
            out.append(f"- run{n} {s}: step #{STEP} resolved={v.get('resolved')} occluded={v.get('occluded')} "
                       f"result `{v.get('result', '')[:140]}`; " + "; ".join(f"#{x} live `{y}` match={z}" for x, y, z in c[s][2]))
    out += ["", f"`rg3baaaaaaa_spread.py --dir` (pre-registered bars MIN_MANUK=9, MIN_CHROME=12, runs agree) exit {sp.returncode}:", "",
            "```", (sp.stdout + sp.stderr).strip(), "```", ""]
    open(os.path.join(a.dir, "summary.md"), "w").write("\n".join(out) + "\n")
    print("\n".join(out)); print("GATE PASS (runs well-formed)")


if __name__ == "__main__":
    main()
