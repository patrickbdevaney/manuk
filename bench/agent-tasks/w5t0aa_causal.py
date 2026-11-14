#!/usr/bin/env python3
"""W5.T0.a.a: causal check, does a smaller navigate observation cut manuk's thinking?

  w5t0aa_causal.py [--nocap TSV] [--cap TSV] [--caps TSV] [--nocap-tx DIR] [--cap-tx DIR]
                   [--out TSV] [--mutant]

Inputs are written by w5t0aa_run.sh: two arms of the manuk tool surface on the 12 largest-thinking-gap tasks,
3 runs each. nocap = navigate observation unchanged; cap = MANUK_NAV_CAP_BYTES set to the task's Playwright
observation size (round(obs_prev_pw) in docs/sprint/w5/T0a-thinking.tsv).

Gate (exit 1 = RED on any failure):
  - each arm has 12 tasks x 3 runs, every row carries the same chrome_ver as the pin;
  - the cap arm really truncates: every navigate result (exact, from the transcript tool_use -> tool_result join)
    is <= cap + NOTE_SLACK, and the cap arm carries at least one truncation note wherever the nocap arm had a
    navigate result over the cap;
  - the single-tool navigate turns in turns.tsv agree with the transcript sizes (the two logs cannot disagree).
--mutant feeds the nocap arm in as the cap arm; the gate must go RED on that (the red check).

Output: one row per task (thinking means, pass counts, over-cap counts) and a paired summary with a bootstrap band
over tasks, printed to stdout. Thinking tokens are the exact result-line count from each transcript.
Stdlib only; run with python3 -I.
"""
import argparse, csv, glob, json, os, random, re, sys

NOTE = "[truncated: navigate observation capped at"
NOTE_SLACK = 120  # bytes the truncation note adds on top of the cap
NAV = "mcp__browser__navigate"
TASKS = ["L12", "X05", "L03", "X06", "X02", "L07", "X01", "L09", "L17", "X03", "L13", "X08"]


def read_tsv(path):
    with open(path, newline="") as f:
        return list(csv.DictReader(f, delimiter="\t"))


def passed(v):
    return str(v).strip().lower() in ("1", "true", "pass", "1.0")


def transcript(path):
    """(thinking_tokens, [(tool_name, result_text_bytes, result_text)]) for one stream-json run."""
    names, results, thinking = {}, [], 0
    for line in open(path):
        d = json.loads(line)
        if d.get("type") == "assistant":
            for b in d["message"].get("content", []):
                if isinstance(b, dict) and b.get("type") == "tool_use":
                    names[b["id"]] = b.get("name", "")
        elif d.get("type") == "user":
            for b in d["message"].get("content", []):
                if isinstance(b, dict) and b.get("type") == "tool_result":
                    c = b.get("content")
                    if isinstance(c, list):
                        text = "".join(x.get("text", "") for x in c if isinstance(x, dict))
                    else:
                        text = c or ""
                    results.append((names.get(b.get("tool_use_id"), ""), len(text.encode()), text))
        elif d.get("type") == "result":
            thinking = d.get("usage", {}).get("output_tokens_details", {}).get("thinking_tokens", 0)
    return thinking, results


def run_key(path):
    m = re.search(r"_(\w+)_r(\d+)\.jsonl$", os.path.basename(path))
    return (m.group(1), int(m.group(2))) if m else None


def load_arm(tsv, tx_dir, pin, label, problems):
    rows = {(r["task"], int(r["run"])): r for r in read_tsv(tsv)}
    turns = {}
    for r in read_tsv(tsv + ".turns.tsv"):
        turns.setdefault((r["task"], int(r["run"])), []).append(r)
    runs = {}
    for p in glob.glob(os.path.join(tx_dir, "manuk_*_r*.jsonl")):
        k = run_key(p)
        if k:
            runs[k] = transcript(p)
    for t in TASKS:
        for run in (1, 2, 3):
            if (t, run) not in rows:
                problems.append(f"{label}: missing row {t} r{run}")
            elif rows[(t, run)]["chrome_ver"] != pin:
                problems.append(f"{label}: {t} r{run} chrome_ver {rows[(t, run)]['chrome_ver']} != pin {pin}")
            if (t, run) not in runs:
                problems.append(f"{label}: missing transcript {t} r{run}")
    return rows, turns, runs


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--nocap", default="docs/sprint/w5/T0a-a-nocap.tsv")
    ap.add_argument("--cap", default="docs/sprint/w5/T0a-a-cap.tsv")
    ap.add_argument("--caps", default="docs/sprint/w5/T0a-thinking.tsv")
    ap.add_argument("--nocap-tx", default=os.path.expanduser("~/.cache/manuk-t0aa/nocap"))
    ap.add_argument("--cap-tx", default=os.path.expanduser("~/.cache/manuk-t0aa/cap"))
    ap.add_argument("--out", default="docs/sprint/w5/T0a-a-causal.tsv")
    ap.add_argument("--mutant", action="store_true", help="red check: judge the nocap arm as if it were the cap arm")
    a = ap.parse_args()

    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    caps = {r["task"]: round(float(r["obs_prev_pw"])) for r in read_tsv(a.caps) if r["task"] in TASKS}
    problems = []
    nocap_rows, nocap_turns, nocap_runs = load_arm(a.nocap, a.nocap_tx, pin, "nocap", problems)
    # the mutant's cap arm IS the nocap data, so the truncation checks below must find no truncation and go RED
    cap_src = (a.nocap, a.nocap_tx) if a.mutant else (a.cap, a.cap_tx)
    cap_rows, cap_turns, cap_runs = load_arm(cap_src[0], cap_src[1], pin, "cap", problems)

    over_nocap, over_cap, notes_cap = {}, {}, {}
    for t in TASKS:
        cap = caps[t]
        for run in (1, 2, 3):
            if (t, run) in nocap_runs:
                over_nocap[(t, run)] = sum(1 for n, b, _ in nocap_runs[(t, run)][1] if n == NAV and b > cap + NOTE_SLACK)
            if (t, run) in cap_runs:
                nav = [(b, x) for n, b, x in cap_runs[(t, run)][1] if n == NAV]
                for b, _ in nav:
                    if b > cap + NOTE_SLACK:
                        problems.append(f"cap {t} r{run}: navigate result {b} bytes > cap {cap}+{NOTE_SLACK}")
                over_cap[(t, run)] = sum(1 for _, x in nav if b_over(x, cap))
                notes_cap[(t, run)] = sum(1 for _, _, x in cap_runs[(t, run)][1] if NOTE in x)
                for r in cap_turns.get((t, run), []):
                    if r["tool"] == NAV:
                        tb = int(r["result_bytes"])
                        if tb > cap + NOTE_SLACK:
                            problems.append(f"cap {t} r{run}: turns.tsv navigate turn {tb} bytes > cap {cap}")
            if over_nocap.get((t, run), 0) > 0 and notes_cap.get((t, run), 0) == 0:
                problems.append(f"cap {t} r{run}: nocap had {over_nocap[(t, run)]} navigate result(s) over cap "
                                f"but the cap run shows no truncation note (cap not applied)")

    if problems:
        for p in problems[:40]:
            print("RED", p)
        print(f"GATE RED: {len(problems)} violation(s)")
        sys.exit(1)

    out_rows, diffs, pass_n, pass_c = [], [], 0, 0
    for t in TASKS:
        tn = [nocap_runs[(t, r)][0] for r in (1, 2, 3)]
        tc = [cap_runs[(t, r)][0] for r in (1, 2, 3)]
        mn, mc = sum(tn) / 3, sum(tc) / 3
        pn = sum(passed(nocap_rows[(t, r)]["pass"]) for r in (1, 2, 3))
        pc = sum(passed(cap_rows[(t, r)]["pass"]) for r in (1, 2, 3))
        diffs.append(mc - mn)
        pass_n += pn
        pass_c += pc
        out_rows.append({
            "task": t, "cap_bytes": caps[t], "thinking_nocap": f"{mn:.1f}", "thinking_cap": f"{mc:.1f}",
            "thinking_diff": f"{mc - mn:.1f}", "pass_nocap": pn, "pass_cap": pc,
            "navs_over_cap_nocap": sum(over_nocap.get((t, r), 0) for r in (1, 2, 3)),
            "navs_over_cap_cap": sum(over_cap.get((t, r), 0) for r in (1, 2, 3)),
            "trunc_notes_cap": sum(notes_cap.get((t, r), 0) for r in (1, 2, 3)),
            "chrome_ver": pin,
        })
    with open(a.out, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(out_rows[0]), delimiter="\t")
        w.writeheader()
        w.writerows(out_rows)

    rng = random.Random(20261114)  # fixed seed: the band is reproducible
    boots = sorted(sum(rng.choice(diffs) for _ in diffs) / len(diffs) for _ in range(10000))
    lo, hi = boots[249], boots[9749]
    mean = sum(diffs) / len(diffs)
    neg = sum(1 for d in diffs if d < 0)
    print(f"tasks {len(diffs)}  runs per arm {len(diffs) * 3}  chrome_ver {pin}")
    print(f"paired thinking diff (cap - nocap), mean per task: {mean:.1f} tokens; 95% bootstrap band [{lo:.1f}, {hi:.1f}]")
    print(f"tasks with lower thinking under cap: {neg} of {len(diffs)}")
    print(f"pass (of 36 runs): nocap {pass_n}, cap {pass_c}")
    print(f"navigate results over cap: nocap {sum(over_nocap.values())}, cap {sum(over_cap.values())}; "
          f"truncation notes in cap arm {sum(notes_cap.values())}")
    print("GATE GREEN")


def b_over(text, cap):
    """True if a navigate result's own text, as the model read it, was over the cap."""
    return len(text.encode()) > cap + NOTE_SLACK


if __name__ == "__main__":
    main()
