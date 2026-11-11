#!/usr/bin/env python3
"""RG.3.0.b: replay the 7 'agent'-labelled round-3 tasks' manuk r1 action sequence under manuk and under Chrome
(chrome_ver=$MANUK_CHROME_PIN) and name the first step after which the observed interactive tree differs.
Handles are per-page and per-backend, so each recorded handle is mapped to the role+"name" it had in the transcript
and re-resolved from a fresh observe on each backend (first match; unresolved steps are skipped and logged).
  rg3b_replay.py [--tasks a,b] [--wait 2] [--mutate identical] [--out FILE]   exit 0 = GATE PASS, 1 = GATE FAIL
Writes the section '## RG.3.0.b' of docs/sprint/rg/LEDGER-3.md (replaced in place). Run with `python3 -I`; needs MANUK_CHROME_PIN.
"""
import argparse, json, os, re, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import real_boot as rb
import rg0_ledger as lg
import rg0a_domdiff as dd

ROOT = lg.ROOT
TASKS = "zilloft-2 opendining-4 gomail-3 opendining-7 udriver-9 udriver-7 gocalendar-2".split()
TR = os.path.expanduser("~/.cache/manuk-rgb2/transcripts")
NODE = re.compile(r'\[(h\d+)\] (\w+) "([^"]*)"')
SITE = {"zilloft": "zilloft", "opendining": "opendining", "gomail": "gomail", "udriver": "udriver", "gocalendar": "gocalendar"}
MARK = "## RG.3.0.b"


def flatten(st):
    """-> [(tool, args)] with act/fill_form batches expanded; handle->(role,name) as last seen before each step."""
    seen, out = {}, []
    for n, tool, inp, res in st:
        try:
            a = json.loads(inp)
        except ValueError:
            a = {}
        subs = a.get("steps") if isinstance(a.get("steps"), list) else None
        items = []
        if subs:
            for s in subs:
                t = s.get("tool"); t = "act" if t == "click" else t
                items.append((t, {k: v for k, v in s.items() if k != "tool"}))
        else:
            items.append((tool, a))
        for t, args in items:
            if t in ("act", "type", "key", "navigate", "scroll", "select", "check", "press", "commit", "back"):
                h = args.get("handle")
                out.append((t, args, seen.get(h) if h else None))
        for m in NODE.finditer(res):
            seen[m.group(1)] = (m.group(2), m.group(3))
    return out


def resolve(obs, rn):
    role, name = rn
    for m in NODE.finditer(obs):
        if m.group(2) == role and m.group(3) == name:
            return m.group(1)
    return None


def replay(backend, seq, wait):
    env, args = dict(os.environ), []
    if backend == "chrome":
        args = ["--backend", "chrome"]; env["MANUK_CHROME"] = os.path.expanduser("~/.local/chrome-cft/bin/google-chrome")
    m = rb.M(args, env); log = []
    try:
        obs = ""
        for i, (t, a, rn) in enumerate(seq):
            a = dict(a); note = ""
            if t == "navigate":
                r = m.call("navigate", a)
            else:
                if "handle" in a:
                    if rn is None:
                        log.append((i, t, "skipped: handle unseen in transcript", None)); continue
                    h = resolve(obs, rn) or resolve(m.call("observe", {"mode": "interactive"}), rn)
                    if not h:
                        log.append((i, t, f"unresolved: {rn[0]} \"{rn[1]}\"", None)); continue
                    a["handle"] = h
                r = m.call("type" if t == "type" else "key" if t == "key" else "act" if t != "scroll" else "scroll", a)
            time.sleep(wait)
            obs = m.call("observe", {"mode": "interactive"})
            log.append((i, f"{t} {rn[0] + ' ' + repr(rn[1]) if rn else ''}".strip(), r[:260].replace("\n", " "), dd.norm(obs)))
        return log
    finally:
        m.p.kill()


def compare(ml, cl):
    """-> None if identical, else (step, label, manuk_result, chrome_result, manuk_line, chrome_line, ndiff)."""
    for (i, lab, mr, mo), (_, _, cr, co) in zip(ml, cl):
        if mo is None or co is None:
            continue
        d = dd.first_diff(mo, co)
        if d:
            return i, lab, mr, cr, d[1], d[2], d[3], len(mo), len(co)
    return None


def first_action_diff(ml, cl):
    """-> first non-navigate step whose result verdict (handles/digits/page ids stripped) differs, else None."""
    def v(r):
        r = re.sub(r"\[names below[^\]]*\]|\bh\d+\b|\d+|page: \S+", "", r)
        return re.sub(r"\s+", " ", r)[:90]
    for (i, lab, mr, mo), (_, _, cr, co) in zip(ml, cl):
        if mo is not None and co is not None and not lab.startswith("navigate") and v(mr) != v(cr):
            return i, lab, mr, cr
    return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--tasks", default=",".join(TASKS)); ap.add_argument("--wait", type=float, default=2)
    ap.add_argument("--mutate"); ap.add_argument("--out", default=os.path.join(ROOT, "docs/sprint/rg/LEDGER-3.md"))
    a = ap.parse_args()
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    md = [MARK, "", f"chrome_ver={pin}. Replay of each task's manuk r1 transcript (`~/.cache/manuk-rgb2/transcripts`) under manuk and Chrome (`bench/agent-tasks/rg3b_replay.py`); handles re-resolved by role+name. First differing step = first step after which the interactive observation differs (handle ids stripped).", ""]
    named = 0
    for t in a.tasks.split(","):
        seq = flatten(lg.steps(os.path.join(TR, f"manuk_{t}_r1.jsonl"))[0])
        ml = replay("manuk", seq, a.wait)
        cl = ml if a.mutate == "identical" else replay("chrome", seq, a.wait)
        skipped = [x for x in ml if x[3] is None]; cskipped = [x for x in cl if x[3] is None]
        c = compare(ml, cl)
        md += [f"### {t}", f"- steps replayed: {len(seq)}; manuk skipped/unresolved {len(skipped)}, chrome {len(cskipped)}"]
        if c:
            named += 1
            i, lab, mr, cr, ln, cn, ratio, nm, nc = c
            md += [f"- first differing step: #{i} `{lab}` (observation lines manuk {nm} vs chrome {nc})",
                   f"  - manuk result: {mr}", f"  - chrome result: {cr}",
                   f"  - first differing node: manuk `{ln}` vs chrome `{cn}`"]
        else:
            named += 1
            md += ["- identical: no step produced a differing interactive observation under manuk and Chrome"]
            if a.mutate == "identical":
                named -= 1
        f = first_action_diff(ml, cl)
        if f:
            md += [f"- first action step with a different result: #{f[0]} `{f[1]}`", f"  - manuk: {f[2]}", f"  - chrome: {f[3]}"]
        else:
            md.append("- no action step returned a different result verdict")
        for x in skipped[:3]:
            md.append(f"  - skipped step #{x[0]} {x[1]}: {x[2]}")
        md.append("")
        print(t, "DIFF" if c else "IDENTICAL", c[:2] if c else "", flush=True)
    body = "\n".join(md)
    try:
        cur = open(a.out).read()
    except OSError:
        cur = ""
    head = cur.split(MARK)[0].rstrip() + "\n\n"
    ok = named == len(a.tasks.split(",")) and a.mutate != "identical"
    if a.mutate != "identical":
        open(a.out, "w").write(head + body + "\n")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
