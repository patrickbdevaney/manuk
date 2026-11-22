#!/usr/bin/env python3
"""RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a.a.c.a.a: why opendining-4 step #7 now lists its time options from '12:00 AM'
on the Chrome backend (earlier: '7:00 AM'), and is that a clock/page effect that makes the ordinal replay unfit as a
fixed bar?

Reads (all in DIR, default docs/sprint/rg/RG3baaaaaaaaaaaaaaaacaa/):
  step7-run{1,2}.json                 two fresh step-#7 re-runs at MANUK_CHROME_PIN (this node)
  RG3baaaaaaa-match-run{1,2}.json     their chrome_ver
  RG3baaaaaaa-match-run{1,2}.md       their 13-row tables (chrome #13/#14 liveness)
  earlier-26d5b8da-step7-run1.json   the 7:00 AM run (commit 26d5b8da, copied verbatim)
  earlier-d38119e6-step7-run1.json   the 12:00 AM run at d38119e6 (committed, copied verbatim)
  option-source.txt                   the page's module 81628 time-option array, verbatim, with chunk sha256
  decision: RG3baaaaaaaaaaaaaaaacaa-cause.md (the named cause and its evidence)

Checks (any failure -> exit 1, GATE FAIL):
  1. both re-runs carry chrome_ver == $MANUK_CHROME_PIN and 13 rows per backend.
  2. the static option array in option-source.txt has 48 entries, '12:00 AM' first, '7:00 AM' at index 14, and the
     module has no clock read (Date(), Date.now, getHours, getMinutes, toLocaleTimeString). So the list is static.
  3. for every step-#7 record (earlier 26d5b8da, earlier d38119e6, rerun1, rerun2) the chrome listing is a SUFFIX of the
     static array: listed options (= '+N' added nodes minus the one button) == 48 - index(first listed option).
     The earlier record is '7:00 AM' (index 14, 34 listed); the reruns are '12:00 AM' (index 0, 48 listed).
  4. the two re-runs agree on the chrome '+N ~N -N' counts and the first listed option.
  5. the named cause is real: commit 94c5e2fd changed mcp/src/cdp.rs shield_of so 'offscreen' is judged with the
     scroll offsets removed (the word 'unscroll' is in that diff), and the decision doc names 94c5e2fd and shield_of.
  6. chrome #13/#14 are unresolved in both re-runs (table live '-'), and the decision doc names the aria-hidden commit
     7b556dab, whose diff adds the aria-hidden honouring to the Chrome backend.
  7. MIN_CHROME/MIN_MANUK/EXPECT are unedited: bench/agent-tasks/rg3baaaaaaa_spread.py has no diff against HEAD.

  rg3baaaaaaaaaaaaaaaacaa_cause.py [--dir DIR]                 exit 0 = GATE PASS (cause named and checked), 1 = FAIL
  rg3baaaaaaaaaaaaaaaacaa_cause.py --mutate clock              the option array is injected with a clock read -> must exit 1
  rg3baaaaaaaaaaaaaaaacaa_cause.py --mutate nocause            the cause sentence is removed from the decision doc -> must exit 1
Run with `python3 -I`; needs MANUK_CHROME_PIN."""
import argparse, json, os, re, subprocess, sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
RG = os.path.join(ROOT, "docs/sprint/rg")
N_OPTS, FIRST, SEVENAM, CHUNK_OPTS = 48, "12:00 AM", "7:00 AM", None
CLOCK = re.compile(r"Date\s*\(|Date\.now|getHours|getMinutes|toLocaleTimeString")
RESULT = re.compile(r"ok: dispatched click at?\. (\+\d+ ~\d+ -\d+) text changed; new \(names are page content\): ([^;]*);")
ROW = re.compile(r"^\| (\S+) \| (manuk|chrome) \| #(\d+) \| `[^`]*` \| `([^`]*)` \| (yes|no) \|$", re.M)
CAUSE_COMMIT, CAUSE_FN, CAUSE_WORD = "94c5e2fd", "shield_of", "unscroll"
ARIA_COMMIT = "7b556dab"


def git(*a):
    return subprocess.run(["git", "-C", ROOT, *a], capture_output=True, text=True)


def parse_record(j):
    """-> (counts string, first listed option or None, listed option count) from a step-#7 chrome record."""
    res = j["chrome"]["result"]
    m = RESULT.search(res + ";")
    if not m:
        return None, None, None
    counts, names = m.group(1), m.group(2)
    opts = re.findall(r'option "([^"]+)"', names)
    added = int(counts.split(" ")[0][1:])
    button = "button [clickable]" in names
    return counts, (opts[0] if opts else None), added - (1 if button else 0)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", default=os.path.join(RG, "RG3baaaaaaaaaaaaaaaacaa"))
    ap.add_argument("--mutate", choices=("clock", "nocause"))
    a = ap.parse_args()
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    errs, notes = [], []

    # --- 2. the page's option list is static
    src = open(os.path.join(a.dir, "option-source.txt"), encoding="utf-8").read()
    arr = re.search(r'let o=(\[[^\]]*\]),i=\[', src)
    if not arr:
        errs.append("option-source.txt: time-option array not found")
        opts = []
    else:
        opts = json.loads(arr.group(1))
    body = src
    if a.mutate == "clock":  # deliberately broken input: the list is built from the clock
        body = src.replace('let o=["12:00 AM"', 'let o=(new Date().getHours()>0?["12:00 AM"')
        body = body.replace('"11:30 PM"],i=[', '"11:30 PM"]:[]),i=[')
    if len(opts) != N_OPTS or opts[0] != FIRST or opts[14] != SEVENAM:
        errs.append(f"static array: {len(opts)} entries, first {opts[:1]}, index 14 {opts[14:15]} (want {N_OPTS}, {FIRST}, {SEVENAM})")
    if CLOCK.search(body):
        errs.append("option module reads the clock (" + ", ".join(sorted(set(CLOCK.findall(body)))) + "): list is time-dependent")
    notes.append(f"static list: {len(opts)} options, first {opts[0] if opts else '-'}, index 14 {opts[14] if len(opts) > 14 else '-'}, clock read: {'no' if not CLOCK.search(body) else 'YES'}")

    # --- 1, 3, 4, 6: the step-#7 records and their chrome_ver / table
    recs = {}
    for name, path in (("earlier 26d5b8da", "earlier-26d5b8da-step7-run1.json"),
                       ("earlier d38119e6", "earlier-d38119e6-step7-run1.json"),
                       ("rerun1", "step7-run1.json"), ("rerun2", "step7-run2.json")):
        p = os.path.join(a.dir, path)
        try:
            recs[name] = json.load(open(p))
        except (OSError, ValueError) as e:
            errs.append(f"{name}: unreadable {path} ({e})")
    for n in (1, 2):
        mj = os.path.join(a.dir, f"RG3baaaaaaa-match-run{n}.json")
        mm = os.path.join(a.dir, f"RG3baaaaaaa-match-run{n}.md")
        try:
            cv = json.load(open(mj)).get("chrome_ver")
            rows = ROW.findall(open(mm).read())
        except (OSError, ValueError) as e:
            errs.append(f"rerun{n}: unreadable match files ({e})"); continue
        if cv != pin:
            errs.append(f"rerun{n}: chrome_ver {cv!r} != MANUK_CHROME_PIN {pin!r}")
        for s in ("manuk", "chrome"):
            if sum(1 for r in rows if r[1] == s) != 13:
                errs.append(f"rerun{n}: {s} table is not 13 rows")
        dead = [f"{s} #{i}" for _, s, i, live, _ in rows if live == "-" and i in ("13", "14")]
        notes.append(f"rerun{n} unresolved ordinals: {', '.join(dead) or 'none'}")
        if sorted(dead) != ["chrome #13", "chrome #14", "manuk #13", "manuk #14"]:
            errs.append(f"rerun{n}: expected chrome and manuk #13/#14 unresolved, got {dead}")

    parsed = {}
    for name, j in recs.items():
        counts, first, listed = parse_record(j)
        if counts is None:
            errs.append(f"{name}: chrome step-#7 result has no 'ok: dispatched click' record"); continue
        parsed[name] = (counts, first, listed)
        if first not in opts:
            errs.append(f"{name}: first listed option {first!r} is not in the static list")
            continue
        idx = opts.index(first)
        # 3. the listing is the suffix of the static list from the first listed option
        if listed != N_OPTS - idx:
            errs.append(f"{name}: lists {listed} options from {first!r} (index {idx}), want {N_OPTS - idx}")
        notes.append(f"{name}: chrome step #7 {counts}, first listed {first!r} (index {idx}), listed {listed}")
    if len(parsed) == 4 and parsed["rerun1"] != parsed["rerun2"]:  # 4. reproducible
        errs.append(f"reruns disagree: {parsed['rerun1']} vs {parsed['rerun2']}")

    # --- 5, 6: the named cause is in the decision doc and in the cited commits
    doc_path = os.path.join(a.dir, "RG3baaaaaaaaaaaaaaaacaa-cause.md")
    doc = open(doc_path, encoding="utf-8").read() if os.path.exists(doc_path) else ""
    if a.mutate == "nocause":
        doc = doc.replace(CAUSE_COMMIT, "[removed]").replace(CAUSE_FN, "[removed]")
    for needle in (CAUSE_COMMIT, CAUSE_FN, ARIA_COMMIT):
        if needle not in doc:
            errs.append(f"decision doc does not name {needle}: no cause named")
    diff = git("show", "--format=", CAUSE_COMMIT, "--", "mcp/src/cdp.rs").stdout
    if CAUSE_WORD not in diff or CAUSE_FN not in diff:
        errs.append(f"{CAUSE_COMMIT} does not change {CAUSE_FN} in mcp/src/cdp.rs (cause not in the code)")
    adiff = git("show", "--format=", ARIA_COMMIT).stdout
    if "aria-hidden" not in adiff:
        errs.append(f"{ARIA_COMMIT} does not touch aria-hidden")

    # --- 7. the bars are unedited
    if git("diff", "--quiet", "HEAD", "--", "bench/agent-tasks/rg3baaaaaaa_spread.py").returncode != 0:
        errs.append("rg3baaaaaaa_spread.py differs from HEAD: bars may have been edited")

    for n in notes:
        print("  " + n)
    if errs:
        print("GATE FAIL:"); [print("  " + e) for e in errs]
        sys.exit(1)
    print("GATE PASS: cause named (94c5e2fd shield_of offscreen judged unscrolled) and checked against both step-#7 records; "
          "the page list is static; bars unedited")


if __name__ == "__main__":
    main()
