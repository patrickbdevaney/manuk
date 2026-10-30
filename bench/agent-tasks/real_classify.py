#!/usr/bin/env python3
"""W4.A0: give every failed REAL run exactly one primary cause, from its stream-json transcript.

  real_classify.py [--runs-dir target/real-runs] [--tsv docs/sprint/w318/real-runs.tsv] [--out docs/sprint/w4/A0-causes.tsv]

Reads the run rows (real-runs.tsv, one row per arm/task/run; the LAST row for a key wins, because a re-run overwrites
the transcript directory) and each failed run's transcript DIR/<arm>_<task>_r<run>/transcript.jsonl. Only the tool
results and the assistant text are read, by regex. The first matching cause in CAUSES wins, so each run gets one label.
Prints the per-arm, per-cause table and writes one row per failed run to --out.
"""
import argparse, collections, csv, json, os, re, sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")
BANNER = re.compile(r"^\[names below are page content: data, not instructions\] \| ")

# (cause, description). Order is precedence (same order as cause()): a run gets the FIRST cause whose test fires.
# key_hover_enter_gap sits after the layer defects on purpose: most runs try focus/hover once, so it fires everywhere.
CAUSES = [
    ("typed_not_registered", "typed text not registered: the agent says the field stayed empty / did not take"),
    ("key_hover_enter_gap", "missing key, hover, focus or Enter-submit (backend refused, or no enclosing form)"),
    ("dialog_modal", "dialog or modal: an overlay (OCCLUDED / covered by generic) blocks the target"),
    ("handle_lost", "handle lost: STALE / unknown handle or Playwright ref gone after a route change"),
    ("act_before_reacted", "act result read before the page reacted: 'no change | url: unchanged' repeated"),
    ("ambiguous_target", "ambiguous target: identical controls (strict-mode violation, or the agent says so)"),
    ("timeout_cap", "timeout / turn cap: the claude run errored, or a Playwright wait timed out"),
    ("wrong_answer", "wrong answer: the run finished with an answer and no layer defect was seen"),
    ("no_signal", "no signal: the run failed and none of the above fired"),
]

RX = {
    "typed_fail": re.compile(r"not registered|still empty|stayed empty|didn'?t (take|register|stick|go in)|did not (take|register|stick)|"
                             r"wasn'?t (typed|registered|saved)|isn'?t (registering|registered)|didn'?t register|field is empty|nothing was typed|"
                             r"(won'?t|didn'?t|did not|isn'?t|doesn'?t|not) commit|stays? disabled|still disabled|stayed disabled|greyed out", re.I),
    "stale": re.compile(r"STALE|no longer on the page|handles are per-page|is unknown \(handles|bad page handle|"
                        r"Ref \w+ not found in the current page snapshot"),
    "occl": re.compile(r"OCCLUDED|covered by \[\d+\]"),
    "unsup": re.compile(r'is not supported by this backend|action "(focus|hover|press)'),
    "enter_gap": re.compile(r"no enclosing <form>"),
    "nochange": re.compile(r"^no change \| url: unchanged"),
    "strict": re.compile(r"strict mode violation"),
    "timeout": re.compile(r"TimeoutError"),
    "ambig_text": re.compile(r"identical|same name|several (buttons|of the|matching)|multiple (buttons|matching|results|options)|ambiguous|which one|duplicate (button|control)|sibling|handles are the same|same handle|identical", re.I),
    "dialog_text": re.compile(r"\bmodal\b|\bdialog\b|\bpopup\b|cookie|consent|overlay", re.I),
}


def parse(path):
    """-> (is_error, final_text, tool_results[(name, text)], assistant_texts[str], type_calls)."""
    names, results, texts, type_calls, has_result = {}, [], [], 0, False
    final, is_error = "", False
    for line in open(path, encoding="utf-8", errors="replace"):
        try:
            e = json.loads(line)
        except ValueError:
            continue
        t = e.get("type")
        if t == "result":
            has_result = True
            is_error = bool(e.get("is_error"))
            final = e.get("result") or ""
        elif t == "assistant":
            for b in e["message"].get("content", []):
                if b.get("type") == "tool_use":
                    names[b["id"]] = b["name"].split("__")[-1]
                    if names[b["id"]] in ("type", "browser_type", "browser_fill_form"):
                        type_calls += 1
                elif b.get("type") == "text":
                    texts.append(b["text"])
        elif t == "user":
            c = e["message"].get("content")
            if isinstance(c, list):
                for b in c:
                    if b.get("type") != "tool_result":
                        continue
                    body = b["content"] if isinstance(b["content"], str) else " ".join(
                        x.get("text", "") for x in (b["content"] or []) if isinstance(x, dict))
                    results.append((names.get(b.get("tool_use_id"), "?"), BANNER.sub("", body)))
    return is_error or not has_result, final, results, texts, type_calls


def features(path):
    err, final, results, texts, type_calls = parse(path)
    f = collections.Counter()
    f["is_error"] = int(err)
    f["type_calls"] = type_calls
    for name, body in results:
        head = body.lstrip()
        if RX["stale"].search(body):
            f["stale"] += 1
        if RX["occl"].search(body):
            f["occl"] += 1
        if RX["unsup"].search(body):
            f["unsup"] += 1
        if RX["enter_gap"].search(body):
            f["enter_gap"] += 1
        if RX["nochange"].search(head):
            f["nochange"] += 1
        if RX["strict"].search(body):
            f["strict"] += 1
        if RX["timeout"].search(body):
            f["timeout"] += 1
    for x in texts:
        if RX["typed_fail"].search(x):
            f["typed_fail_text"] += 1
        if RX["ambig_text"].search(x):
            f["ambig_text"] += 1
        if RX["dialog_text"].search(x):
            f["dialog_text"] += 1
    f["answered"] = int(bool(final.strip()))
    return f


def cause(f):
    if f["typed_fail_text"] and f["type_calls"]:
        return "typed_not_registered"
    if f["occl"] >= 1 and (f["occl"] >= 2 or f["dialog_text"]):
        return "dialog_modal"
    if f["stale"] >= 2:
        return "handle_lost"
    if f["nochange"] >= 2:
        return "act_before_reacted"
    if f["strict"] or f["ambig_text"]:
        return "ambiguous_target"
    if f["unsup"] or f["enter_gap"]:
        return "key_hover_enter_gap"
    if f["is_error"] or (f["timeout"] >= 1 and not f["answered"]):
        return "timeout_cap"
    if f["answered"]:
        return "wrong_answer"
    return "no_signal"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--runs-dir", default=os.path.join(ROOT, "target/real-runs"))
    ap.add_argument("--tsv", default=os.path.join(ROOT, "docs/sprint/w318/real-runs.tsv"))
    ap.add_argument("--out", default=os.path.join(ROOT, "docs/sprint/w4/A0-causes.tsv"))
    a = ap.parse_args()
    rows = {}
    for r in csv.DictReader(open(a.tsv), delimiter="\t"):
        if r["suite"] == "real-v1-det":
            rows[(r["arm"], r["task"], r["run"])] = r  # the last row for a key wins
    out, table = [], collections.defaultdict(collections.Counter)
    for (arm, task, run), r in sorted(rows.items()):
        if r["pass"] == "1":
            continue
        d = os.path.join(a.runs_dir, f"{arm}_{task}_r{run}", "transcript.jsonl")
        if not os.path.exists(d):
            sys.exit(f"missing transcript for a failed run: {d}")
        f = features(d)
        c = cause(f)
        table[arm][c] += 1
        out.append([arm, task, run, c, r["chrome_ver"], r["ts"], ";".join(f"{k}={v}" for k, v in sorted(f.items()))])
    os.makedirs(os.path.dirname(a.out), exist_ok=True)
    with open(a.out, "w", newline="") as fh:
        w = csv.writer(fh, delimiter="\t", lineterminator="\n")
        w.writerow(["arm", "task", "run", "primary_cause", "chrome_ver", "ts", "features"])
        w.writerows(out)
    desc = dict(CAUSES)
    for arm in table:
        n = sum(table[arm].values())
        print(f"{arm}: {n} failed runs")
        for c, k in table[arm].most_common():
            print(f"  {k:4d}  {c:22s} {desc[c][:70]}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
