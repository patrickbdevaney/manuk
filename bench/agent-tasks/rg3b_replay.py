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
# Pinned verdicts. zilloft-2: under Chrome the replay's search steps are unresolved, so the Beds & Baths click is
# not comparable (RG.3.0.b.a evidence). Any other task is only required to run, not to have a given verdict.
EXPECT = {"zilloft-2": "not comparable"}


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


def resolve(obs, rn, text=False, force=False):
    """Exact role+name first. A typed step may also match by role when one name is a prefix of the other (the
    transcript's field name can be a truncated or extended form). `force` is the mutant: resolution always succeeds,
    to the same-role node or else the first node, and also for a handle the transcript never named (rn None)."""
    nodes = [(m.group(1), m.group(2), m.group(3)) for m in NODE.finditer(obs)]
    if rn is None:
        return nodes[0][0] if force and nodes else None
    role, name = rn
    for h, r, n in nodes:
        if r == role and n == name:
            return h
    if text and name:
        for h, r, n in nodes:
            if r == role and n and (n.startswith(name) or name.startswith(n)):
                return h
    if force and nodes:  # mutant: resolution always succeeds, to the same-role node or else the first node
        return next((h for h, r, n in nodes if r == role), nodes[0][0])
    return None


def replay(backend, seq, wait, force=False):
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
                    text = t == "type"
                    h = resolve(obs, rn, text, force) or resolve(m.call("observe", {"mode": "interactive"}), rn, text, force)
                    if not h:
                        why = "handle unseen in transcript" if rn is None else f"unresolved: {rn[0]} \"{rn[1]}\""
                        log.append((i, t, why, None)); continue
                    a["handle"] = h
                r = m.call("type" if t == "type" else "key" if t == "key" else "act" if t != "scroll" else "scroll", a)
            time.sleep(wait)
            obs = m.call("observe", {"mode": "interactive"})
            log.append((i, f"{t} {rn[0] + ' ' + repr(rn[1]) if rn else ''}".strip(), r[:260].replace("\n", " "), dd.norm(obs)))
        return log
    finally:
        m.p.kill()


def cutoff(ml, cl):
    """Index of the first step unresolved (or skipped) on either backend, else None. Steps after it are not compared:
    once one side's page state is missing, a later difference says nothing about the page."""
    return next((i for (i, _, _, mo), (_, _, _, co) in zip(ml, cl) if mo is None or co is None), None)


def compare(ml, cl):
    """-> ('diff', detail) | ('not comparable', (step, label, why, pre)) | ('identical', None).
    A task with any unresolved step on either side is not comparable: the replayed click sequence has a gap, so no
    verdict is reported from it. A difference before the gap is kept as `pre` for information only.
    detail = (step, label, manuk_result, chrome_result, manuk_line, chrome_line, ndiff, nm, nc)."""
    cut = cutoff(ml, cl)
    pre = None
    for (i, lab, mr, mo), (_, _, cr, co) in zip(ml, cl):
        if cut is not None and i >= cut:
            break
        d = dd.first_diff(mo, co)
        if d:
            if cut is None:
                return "diff", (i, lab, mr, cr, d[1], d[2], d[3], len(mo), len(co))
            pre = (i, lab, mr, cr, d[1], d[2], d[3], len(mo), len(co))
            break
    if cut is not None:
        i, lab, mres, mo = ml[cut]
        side, why = ("manuk", mres) if mo is None else ("chrome", cl[cut][2])
        return "not comparable", (cut, lab, f"{side} {why}", pre)
    return "identical", None


def first_action_diff(ml, cl):
    """-> first non-navigate step before the cutoff whose result verdict (handles/digits/page ids stripped) differs."""
    def v(r):
        r = re.sub(r"\[names below[^\]]*\]|\bh\d+\b|\d+|page: \S+", "", r)
        return re.sub(r"\s+", " ", r)[:90]
    cut = cutoff(ml, cl)
    for (i, lab, mr, mo), (_, _, cr, co) in zip(ml, cl):
        if cut is not None and i >= cut:
            break
        if not lab.startswith("navigate") and v(mr) != v(cr):
            return i, lab, mr, cr
    return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--tasks", default=",".join(TASKS)); ap.add_argument("--wait", type=float, default=2)
    ap.add_argument("--mutate", choices=["identical", "force-resolve"]); ap.add_argument("--out", default=os.path.join(ROOT, "docs/sprint/rg/LEDGER-3.md"))
    a = ap.parse_args()
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    intro = [MARK, "", f"chrome_ver={pin}. Replay of each task's manuk r1 transcript (`~/.cache/manuk-rgb2/transcripts`) under manuk and Chrome (`bench/agent-tasks/rg3b_replay.py`); handles re-resolved by role+name. First differing step = first step after which the interactive observation differs (handle ids stripped).", ""]
    verdicts, blocks = {}, {}
    for t in a.tasks.split(","):
        seq = flatten(lg.steps(os.path.join(TR, f"manuk_{t}_r1.jsonl"))[0])
        ml = replay("manuk", seq, a.wait, force=a.mutate == "force-resolve")
        if a.mutate == "identical":  # mutant: both sides identical and fully resolved, so no cutoff can apply
            ml = [(i, lab, r, o if o is not None else "") for i, lab, r, o in ml]; cl = ml
        else:
            cl = replay("chrome", seq, a.wait, force=a.mutate == "force-resolve")  # force: on both backends
        skipped = [x for x in ml if x[3] is None]; cskipped = [x for x in cl if x[3] is None]
        verdict, c = compare(ml, cl)
        verdicts[t] = verdict
        md = [f"### {t}",f"- steps replayed: {len(seq)}; manuk skipped/unresolved {len(skipped)}, chrome {len(cskipped)}"]
        if verdict == "diff":
            i, lab, mr, cr, ln, cn, ratio, nm, nc = c
            md += [f"- first differing step: #{i} `{lab}` (observation lines manuk {nm} vs chrome {nc})",
                   f"  - manuk result: {mr}", f"  - chrome result: {cr}",
                   f"  - first differing node: manuk `{ln}` vs chrome `{cn}`"]
        elif verdict == "not comparable":
            md += [f"- not comparable: unresolved from step #{c[0]} `{c[1]}` ({c[2]}); no differing step is reported for this task"]
            if c[3]:
                md += [f"  - before the gap (information only): #{c[3][0]} `{c[3][1]}` node manuk `{c[3][4]}` vs chrome `{c[3][5]}` (lines {c[3][7]} vs {c[3][8]})",
                       f"    - manuk result: {c[3][2]}", f"    - chrome result: {c[3][3]}"]
        else:
            md += ["- identical: no step produced a differing interactive observation under manuk and Chrome"]
        f = first_action_diff(ml, cl)
        if f:
            md += [f"- first action step with a different result: #{f[0]} `{f[1]}`", f"  - manuk: {f[2]}", f"  - chrome: {f[3]}"]
        else:
            md.append("- no action step returned a different result verdict")
        for x in skipped[:3]:
            md.append(f"  - skipped step #{x[0]} {x[1]}: {x[2]}")
        blocks[t] = "\n".join(md) + "\n"
        print(t, verdict.upper(), c[:2] if verdict == "diff" else c[:1] if c else "", flush=True)
    # The gate: every task whose expected verdict is pinned must get it. Under --mutate identical or force-resolve
    # the pinned verdict must not hold, so the gate goes red.
    ok = all(verdicts[t] == EXPECT.get(t, verdicts[t]) for t in verdicts)
    if a.mutate is None:
        # replace only the task blocks that ran: keep the other tasks' blocks, everything before this section,
        # and the next "## " heading onwards
        try:
            cur = open(a.out).read()
        except OSError:
            cur = ""
        head, _, rest = cur.partition(MARK)
        idx = rest.find("\n## ")
        sec, tail = (rest[:idx], rest[idx:]) if idx >= 0 else (rest, "")
        keep = {k: "### " + k + "\n" + v for k, v in re.findall(r"(?ms)^### (\S+)\n(.*?)(?=^### |\Z)", sec)}
        keep.update(blocks)
        body = "\n".join(intro) + "\n" + "\n".join(v.rstrip("\n") + "\n" for v in keep.values())
        open(a.out, "w").write(head.rstrip() + "\n\n" + body.rstrip() + "\n" + tail)
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
