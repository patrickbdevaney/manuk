#!/usr/bin/env python3
"""W4.P0 engine compass: outcome agreement between the manuk arm and the cdp-layer arm.

Same tasks, same model (Haiku), same chrome_ver. For each task, an arm's outcome is PASS when
at least 2 of its 3 runs pass (the SUCCESS.md rule). Agreement = the two arms reach the same outcome.
Per-run agreement (run r of manuk vs run r of cdp-layer) gives the band.

Slices: own (live + fixture, suite 69903c7d), external (live, same suite hash check), REAL (A6-real.tsv).
Latest row wins for each (arm, kind, task, run) key, so a re-run replaces an earlier campaign.

Usage:
  python3 bench/agent-tasks/engine_compass.py            # print the table and disagreements
  python3 bench/agent-tasks/engine_compass.py --write    # also write docs/sprint/w4p0/COMPASS.tsv
  python3 bench/agent-tasks/engine_compass.py --selftest # mutant check: a flipped row must move the rate
"""
import csv
import os
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
SCOREBOARD = os.path.join(ROOT, "docs", "sprint", "SCOREBOARD.tsv")
REAL = os.path.join(ROOT, "docs", "sprint", "w4", "A6-real.tsv")
OUT = os.path.join(ROOT, "docs", "sprint", "w4p0", "COMPASS.tsv")
PIN = os.environ.get("MANUK_CHROME_PIN", "145.0.7632.116")
OWN_SUITE = "69903c7d"  # suite-v1.json
EXT_SUITE = "061ee22b"  # suite-v1-external.json (suite v1.1)
ARMS = ("manuk", "cdp-layer")


# Mechanism per disagreeing task. Labels are read from the failing rows' answer text (SCOREBOARD `answer`,
# A6-real `answer`); none is traced in code yet. "suspected" = a hypothesis that follow-up nodes must trace.
# "verifier" = the pass/fail is wrong, so the disagreement is not an engine gap.
MECHANISM = {
    ("external-live", "X01"): "verifier + site: cdp-layer hits Cloudflare challenge; manuk pass not checkable (stored answer cut at 300 chars)",
    ("external-live", "X02"): "site: cdp-layer hits Cloudflare challenge; manuk 2/3 pass on answers that report failure (verifier)",
    ("external-live", "X03"): "site: Cambridge returns no page content (manuk r1-2, cdp r1,r3); mixed, unclassified",
    ("external-live", "X04"): "verifier-brittle: manuk r2 says 'Monday, 27 March 2023', verifier wants '27 Mar 2023'",
    ("external-live", "X06"): "wolfram-js-gate (suspected engine JS gap): 'doesn't run without JavaScript'; manuk pass r1/r3 is own calculation (verifier)",
    ("external-live", "X07"): "wolfram-js-gate (suspected engine JS gap): results page does not show the answer on manuk",
    ("external-live", "X08"): "wolfram-js-gate (suspected engine JS gap): results page does not show the answer on manuk",
    ("own-fixture", "F08"): "verifier-brittle: manuk r3 'Item 45 is amber' (correct) fails the exact string 'Item 45: amber'",
    ("own-live", "L12"): "scroll-trigger (suspected): second quote batch never loads on manuk 3/3; cdp-layer scroll loads it",
    ("real", "dashdish-2"): "click-family (suspected): store card click does not open the store page",
    ("real", "dashdish-7"): "click-family (suspected): restaurant card click never reaches the menu",
    ("real", "dashdish-10"): "click-family (suspected): store card click does not open the store page",
    ("real", "gocalendar-1"): "click-family (suspected): time controls and Save do not take effect on manuk",
    ("real", "gocalendar-3"): "click-family (suspected): event tile click does not open the detail view",
    ("real", "gocalendar-4"): "click-family (suspected): event edit does not take on manuk",
    ("real", "gomail-5"): "click-family (suspected): compose window never opens on manuk (cdp-layer r2 scheduled)",
    ("real", "gomail-8"): "verifier + enumeration: manuk trashed 1 of 4 GitHub mails and passes 3/3; cdp-layer r2 found none",
    ("real", "networkin-3"): "run-flaky: manuk r3 2/3 evals on the same post; unclassified",
    ("real", "opendining-3"): "cdp-side: cdp r3 says booked, evals 0/5; verifier or state mismatch, unclassified",
    ("real", "opendining-4"): "cdp-side: cdp r1-2 reservation form rejects every phone number; unclassified",
    ("real", "opendining-5"): "cdp-side: cdp r1 says booked, evals 0/5; verifier or state mismatch, unclassified",
    ("real", "opendining-8"): "cdp-side: cdp r1 booked the right place but evals 0/3; r2 booked another restaurant; unclassified",
    ("real", "opendining-10"): "cdp-side: cdp r1,r3 find the menus empty; unclassified",
    ("real", "staynb-2"): "click-family (suspected): guest-count stepper does not change on manuk",
    ("real", "udriver-1"): "typeahead (suspected): manuk typing offers 4 fixed addresses; cdp-layer picks the same ones",
    ("real", "udriver-6"): "typeahead (suspected): as udriver-1",
    ("real", "udriver-10"): "typeahead (suspected): as udriver-1",
    ("real", "udriver-11"): "typeahead (suspected): as udriver-1",
}


def load_rows(path, mutate=None):
    with open(path, newline="") as f:
        rows = list(csv.DictReader(f, delimiter="\t"))
    if mutate:
        rows = mutate(rows)
    return rows


def latest_by_key(rows, slice_of):
    """Keep the latest row per (slice, arm, task, run). Rows are kept in file order, so the
    last one seen is the most recent campaign for that key."""
    best = {}
    for r in rows:
        if r["chrome_ver"] != PIN or r["arm"] not in ARMS:
            continue
        s = slice_of(r)
        if s is None:
            continue
        key = (s, r["arm"], r["task"], r["run"])
        best[key] = r
    return best


def slice_scoreboard(r):
    if r["slice"] == "own" and r["suite_hash"].startswith(OWN_SUITE):
        return "own-" + r["kind"]  # own-live, own-fixture
    if r["slice"] == "external" and r["kind"] == "live" and r["suite_hash"].startswith(EXT_SUITE):
        return "external-live"
    return None


def slice_real(r):
    return "real" if r["suite"] == "real-v1-det" else None


def outcomes(best):
    """{(slice, task): {arm: {run: pass(bool)}}}"""
    out = {}
    for (s, arm, task, run), r in best.items():
        out.setdefault((s, task), {}).setdefault(arm, {})[run] = r["pass"] == "1"
    return out


def compare(outs):
    """Return per-slice stats and the list of disagreements."""
    stats = {}
    disagree = []
    all_rows = []
    for (s, task), arms in sorted(outs.items()):
        if set(arms) != set(ARMS):
            continue
        runs = sorted(set(arms["manuk"]) & set(arms["cdp-layer"]))
        if len(runs) != 3:
            continue
        st = stats.setdefault(s, {"tasks": 0, "agree": 0, "run_agree": {r: [0, 0] for r in ("1", "2", "3")}})
        pm = sum(arms["manuk"][r] for r in runs) >= 2
        pc = sum(arms["cdp-layer"][r] for r in runs) >= 2
        st["tasks"] += 1
        st["agree"] += pm == pc
        for r in runs:
            st["run_agree"][r][1] += 1
            st["run_agree"][r][0] += arms["manuk"][r] == arms["cdp-layer"][r]
        all_rows.append((s, task, pm, pc, runs, arms))
        if pm != pc or any(arms["manuk"][r] != arms["cdp-layer"][r] for r in runs):
            disagree.append((s, task, pm, pc, runs, arms))
    return stats, disagree, all_rows


def band(stats):
    rates = [stats["run_agree"][r][0] / stats["run_agree"][r][1] for r in ("1", "2", "3") if stats["run_agree"][r][1]]
    return min(rates), max(rates)


def report(stats, disagree):
    lines = []
    total_t = total_a = 0
    for s in sorted(stats):
        st = stats[s]
        lo, hi = band(st)
        total_t += st["tasks"]
        total_a += st["agree"]
        lines.append(
            f"{s}: task-outcome agreement {st['agree']}/{st['tasks']} = {st['agree']/st['tasks']:.1%}; "
            f"per-run band {lo:.1%}..{hi:.1%}"
        )
    if total_t:
        lines.append(f"ALL: task-outcome agreement {total_a}/{total_t} = {total_a/total_t:.1%}")
    lines.append("")
    lines.append("disagreements (task-level outcome differs, or any run differs):")
    for s, task, pm, pc, runs, arms in disagree:
        m = "".join("P" if arms["manuk"][r] else "f" for r in runs)
        c = "".join("P" if arms["cdp-layer"][r] else "f" for r in runs)
        flag = "OUTCOME" if pm != pc else "run-flaky"
        why = MECHANISM.get((s, task), "UNCLASSIFIED")
        lines.append(f"  {flag:9} {s:14} {task:14} manuk={m} cdp={c} | {why}")
    return "\n".join(lines)


def run_agreement(stats):
    return sum(v[0] for st in stats.values() for v in st["run_agree"].values())


def selftest():
    """Mutant check: flipping one manuk run that matches cdp-layer must lower the run-level agreement
    count by exactly 1, and every disagreement must carry a mechanism label."""
    best = latest_by_key(load_rows(SCOREBOARD), slice_scoreboard)
    best.update(latest_by_key(load_rows(REAL), slice_real))
    base_stats, base_dis, _ = compare(outcomes(best))
    missing = [(s, t) for s, t, *_ in base_dis if (s, t) not in MECHANISM]
    if missing:
        print("SELFTEST FAIL: no mechanism label for", missing)
        return 1
    key = next(k for k, r in best.items() if k[1] == "manuk"
               and (k[0], "cdp-layer", k[2], k[3]) in best
               and best[(k[0], "cdp-layer", k[2], k[3])]["pass"] == r["pass"])
    mutant = dict(best)
    mutant[key] = dict(best[key], **{"pass": "0" if best[key]["pass"] == "1" else "1"})
    m_stats, _, _ = compare(outcomes(mutant))
    drop = run_agreement(base_stats) - run_agreement(m_stats)
    if drop != 1:
        print(f"SELFTEST FAIL: flipping {key} moved run agreement by {drop}, expected 1")
        return 1
    print(f"SELFTEST OK: flip {key[:1] + key[2:]} lowers run agreement by 1; "
          f"base task agreement {sum(st['agree'] for st in base_stats.values())}; "
          f"{len(base_dis)} disagreements, all labelled")
    return 0


def main(argv):
    if "--selftest" in argv:
        return selftest()
    sb = load_rows(SCOREBOARD)
    real = load_rows(REAL)
    best = latest_by_key(sb, slice_scoreboard)
    best.update(latest_by_key(real, slice_real))
    outs = outcomes(best)
    stats, disagree, all_rows = compare(outs)
    text = report(stats, disagree)
    print(text)
    if "--write" in argv:
        os.makedirs(os.path.dirname(OUT), exist_ok=True)
        with open(OUT, "w") as f:
            w = csv.writer(f, delimiter="\t", lineterminator="\n")
            w.writerow(["chrome_ver", "slice", "task", "manuk_runs", "cdp_runs", "manuk_outcome", "cdp_outcome", "agree", "mechanism"])
            for s, task, pm, pc, runs, arms in all_rows:
                m = "".join("1" if arms["manuk"][r] else "0" for r in runs)
                c = "".join("1" if arms["cdp-layer"][r] else "0" for r in runs)
                agree = int(pm == pc)
                w.writerow([PIN, s, task, m, c, int(pm), int(pc), agree, MECHANISM.get((s, task), "")])
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
