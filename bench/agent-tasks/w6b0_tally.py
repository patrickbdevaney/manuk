#!/usr/bin/env python3
"""W6.B0 tally: the cdp-layer arm (manuk tool surface over pinned Chrome) on REAL-102 x3 against
RG.2.B manuk (docs/sprint/rg/B2-real-manuk.tsv, 40/102) and the Playwright band (W5.B1 + RG.2.B drift, 44-53).

Falsifier of 'engine beats cdp' (SYNTHESIS kill rule, feeds W6.D1): cdp-layer task_pass_2of3 >= 40.

Refuses (exit 2) on: a row judged by a non-primary judge, a chrome_ver other than $MANUK_CHROME_PIN,
or a missing (task, run) key -- an incomplete or mis-judged table never yields a verdict.

  python3 -I bench/agent-tasks/w6b0_tally.py [CDP_TSV]
  python3 -I bench/agent-tasks/w6b0_tally.py --mutate N [CDP_TSV]   # gate mutant: +N failing tasks pass / -N passing tasks fail
  python3 -I bench/agent-tasks/w6b0_tally.py --self-test [CDP_TSV]  # the falsifier and the refusals can go red
"""
import argparse, collections, copy, csv, os, sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "../.."))
CDP = os.path.join(ROOT, "bench-queue/out/w6b0-cdplayer-real.tsv")
MANUK = os.path.join(ROOT, "docs/sprint/rg/B2-real-manuk.tsv")
SPEC = os.path.join(ROOT, "bench-queue/specs/w6b0-cdp-layer.json")
JUDGE = "openai/gpt-oss-120b"
KILL = 40          # RG.2.B manuk task_pass_2of3, pre-registered in the W6.B0 / W6.D1 rows
PW_BAND = (44, 53)  # Playwright per-run band, W5.B1 (48/44/50) + RG.2.B drift control (53)


class Refuse(Exception):
    pass


def load(p):
    with open(p, newline="") as f:
        return list(csv.DictReader(f, delimiter="\t"))


def tasks_expected():
    import json
    return json.load(open(SPEC))["band"]["tasks"]


def table(rows, tasks, runs=3):
    bad = sorted({r["judge_model"] for r in rows if r.get("judge_model") not in ("", JUDGE)})
    if bad:
        raise Refuse(f"non-primary judge rows: {bad}")
    pin = os.environ.get("MANUK_CHROME_PIN", "")
    cv = sorted({r["chrome_ver"] for r in rows})
    if not pin or cv != [pin]:
        raise Refuse(f"chrome_ver {cv} != pin {pin!r}")
    got = {(r["task"], int(r["run"])): int(r["pass"]) for r in rows if r.get("pass") in ("0", "1")}
    missing = [(t, k) for t in tasks for k in range(1, runs + 1) if (t, k) not in got]
    if missing:
        raise Refuse(f"{len(missing)} (task,run) keys unscored, e.g. {missing[:3]}")
    return got


def summarise(got, tasks, runs=3):
    per_run = [sum(got[(t, k)] for t in tasks) for k in range(1, runs + 1)]
    ok = {t: sum(got[(t, k)] for k in range(1, runs + 1)) >= 2 for t in tasks}
    return per_run, ok


def _mean(rows, k):
    v = [float(r[k]) for r in rows if r.get(k)]
    return round(sum(v) / len(v), 1) if v else None


def report(cdp_path, mutate=0, quiet=False):
    tasks = tasks_expected()
    rows = load(cdp_path)
    if mutate:
        rows = copy.deepcopy(rows)
        _, ok0 = summarise(table(rows, tasks), tasks)
        # +N: N failing tasks pass every run; -N: N passing tasks fail every run
        flip = sorted(t for t, v in ok0.items() if v == (mutate < 0))[:abs(mutate)]
        for r in rows:
            if r["task"] in flip:
                r["pass"] = "1" if mutate > 0 else "0"
    cdp_runs, cdp_ok = summarise(table(rows, tasks), tasks)
    m_runs, m_ok = summarise(table(load(MANUK), tasks), tasks)
    t2 = sum(cdp_ok.values())
    out = {
        "cdp_per_run": cdp_runs, "cdp_task_pass_2of3": t2,
        "manuk_rg2b_per_run": m_runs, "manuk_rg2b_task_pass_2of3": sum(m_ok.values()),
        "playwright_band": list(PW_BAND),
        "falsifier_fired": t2 >= KILL,
        "cdp_vs_playwright": ("ABOVE" if min(cdp_runs) > PW_BAND[1] else
                              "BELOW" if max(cdp_runs) < PW_BAND[0] else "OVERLAP"),
        "cdp_vs_manuk_runs": ("ABOVE" if min(cdp_runs) > max(m_runs) else
                              "BELOW" if max(cdp_runs) < min(m_runs) else "OVERLAP"),
        "cdp_only": sorted(t for t in tasks if cdp_ok[t] and not m_ok[t]),
        "manuk_only": sorted(t for t in tasks if m_ok[t] and not cdp_ok[t]),
        "both": sum(1 for t in tasks if cdp_ok[t] and m_ok[t]),
        "turns_mean": _mean(rows, "turns"),
        "cost_usd_total": round(sum(float(r["cost_usd"] or 0) for r in rows), 2),
    }
    if not quiet:
        for k, v in out.items():
            print(f"{k}: {v}")
    return out


def self_test(cdp_path):
    import tempfile
    fails = []
    base = report(cdp_path, quiet=True)
    # 1) the falsifier flips under a mutant that moves the count across KILL (up if below, down if at/above)
    t2 = base["cdp_task_pass_2of3"]
    need = KILL - t2 if t2 < KILL else -(t2 - KILL + 1)
    mut = report(cdp_path, mutate=need, quiet=True)
    if mut["falsifier_fired"] == base["falsifier_fired"] or mut["cdp_task_pass_2of3"] != t2 + need:
        fails.append(f"mutant {need:+d} did not flip falsifier ({t2} -> {mut['cdp_task_pass_2of3']})")
    # 2) broken inputs must refuse: a fallback-judged row, a dropped row, a foreign chrome_ver
    rows = load(cdp_path); fields = list(rows[0].keys())
    for name, edit in [("fallback judge", lambda rs: rs[0].__setitem__("judge_model", "fallback/x")),
                       ("missing row", lambda rs: rs.pop()),
                       ("chrome_ver drift", lambda rs: rs[0].__setitem__("chrome_ver", "146.0.0.0"))]:
        rs = copy.deepcopy(rows); edit(rs)
        with tempfile.NamedTemporaryFile("w", suffix=".tsv", delete=False, newline="") as f:
            w = csv.DictWriter(f, fieldnames=fields, delimiter="\t"); w.writeheader(); w.writerows(rs)
        try:
            report(f.name, quiet=True); fails.append(f"{name}: not refused")
        except Refuse:
            pass
        finally:
            os.unlink(f.name)
    if fails:
        print("self-test RED: " + "; ".join(fails)); return 1
    print(f"self-test GREEN: falsifier {base['cdp_task_pass_2of3']}->{mut['cdp_task_pass_2of3']} "
          f"fired {base['falsifier_fired']}->{mut['falsifier_fired']}; fallback-judge, missing-row, chrome-drift inputs refused")
    return 0


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("tsv", nargs="?", default=CDP)
    ap.add_argument("--mutate", type=int, default=0)
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args()
    try:
        sys.exit(self_test(a.tsv) if a.self_test else (report(a.tsv, a.mutate) and 0))
    except Refuse as e:
        print(f"REFUSED: {e}"); sys.exit(2)
