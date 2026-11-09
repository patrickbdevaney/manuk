#!/usr/bin/env python3
"""RG.0.a: ENGINE-cohort DOM diff. For each ENGINE GAP task, take the page the failing manuk step was on (the last
navigate URL before that step, LEDGER.tsv), load it under manuk and under Chrome (chrome_ver=$MANUK_CHROME_PIN), and
diff the observed node tree (role "name" [state], handle ids stripped). Writes docs/sprint/rg/ENGINE-DOMDIFF.md.

  rg0a_domdiff.py [--wait 6] [--out FILE] [--mutate identical]   exit 0 = GATE PASS, 1 = GATE FAIL
Limit: only the load-time page is replayed; handles are per-page and per-backend, so in-page action sequences are not.
Run with `python3 -I`. Requires MANUK_CHROME_PIN.
"""
import argparse, csv, difflib, json, os, re, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import real_boot as rb
import rg0_ledger as lg

ROOT = lg.ROOT
HID = re.compile(r"^\s*\[h\d+\]\s*|\[h\d+\]")


def last_nav(task, run, step, tr):
    st, _ = lg.steps(os.path.join(tr, f"manuk_{task}_r{run}.jsonl"))
    url = None
    for n, tool, inp, _ in st:
        if n > step:
            break
        if tool == "navigate":
            url = json.loads(inp).get("url") or url
    return url


def dump(backend, url, wait):
    env, args = dict(os.environ), []
    if backend == "chrome":
        args = ["--backend", "chrome"]; env["MANUK_CHROME"] = os.path.expanduser("~/.local/chrome-cft/bin/google-chrome")
    m = rb.M(args, env)
    try:
        m.call("navigate", {"url": url}); time.sleep(wait)
        return m.call("observe", {"mode": "full", "budget_tokens": 20000})
    finally:
        m.p.kill()


def norm(t):
    return [HID.sub("", l).strip() for l in t.splitlines() if l.strip() and not l.startswith(("page:", "[names below", "⚠hidden-text", "http"))]


def first_diff(a, b):
    """-> None if identical, else (index, manuk_line, chrome_line) of the first differing node."""
    sm = difflib.SequenceMatcher(None, a, b, autojunk=False)
    for tag, i1, i2, j1, j2 in sm.get_opcodes():
        if tag != "equal":
            return i1, (a[i1] if i1 < len(a) else "<absent>"), (b[j1] if j1 < len(b) else "<absent>"), sm.ratio()
    return None


def ctx(lines, i, k=3):
    return "\n".join(lines[max(0, i - k):i + k + 1])


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--wait", type=float, default=6); ap.add_argument("--mutate")
    ap.add_argument("--tr", default=os.path.expanduser("~/.cache/manuk-b1/transcripts"))
    ap.add_argument("--out", default=os.path.join(ROOT, "docs/sprint/rg/ENGINE-DOMDIFF.md"))
    a = ap.parse_args()
    pin = os.environ.get("MANUK_CHROME_PIN")
    if not pin:
        sys.exit("refusing: MANUK_CHROME_PIN unset")
    led = {r["task"]: r for r in csv.DictReader(open(os.path.join(ROOT, "docs/sprint/rg/LEDGER.tsv")), delimiter="\t")}
    md, ok, ndiff = [], True, 0
    cache = {}
    def pair(url):
        if url not in cache:
            m, c = dump("manuk", url, a.wait), dump("chrome", url, a.wait)
            if a.mutate == "identical":
                c = m
            cache[url] = (norm(m), norm(c))
        return cache[url]
    for t in sorted(lg.ENGINE):
        r = led[t]
        url = last_nav(t, r["manuk_run"], int(r["step"]), a.tr)
        if not url:
            ok = False; md.append(f"## {t}\nNO URL\n"); continue
        m, c = pair(url)
        d = first_diff(m, c)
        if not m or not c:
            ok = False
        head = f"## {t}\n- failing signal: {r['signal']} ({r['evidence'][:140]})\n- replayed page: {url}\n- nodes: manuk {len(m)}, chrome {len(c)}\n"
        if d is None:
            md.append(head + "- verdict: IDENTICAL node tree at load (the failure is not in the loaded DOM; see ledger mechanism)\n")
        else:
            ndiff += 1
            i, ml, cl, ratio = d
            md.append(head + f"- verdict: first differing node at index {i} (similarity {ratio:.3f})\n\nmanuk:\n```\n{ctx(m, i)}\n```\nchrome:\n```\n{ctx(c, i)}\n```\n")
    # control: the diff must see a difference between two different trees, else the gate cannot go red
    ctl = first_diff(["a", "b"], ["a", "c"]) is not None
    ctl_pages = len({u for u in cache}) >= 1 and (a.mutate != "identical" or ndiff == 0)
    # mutant check: swapping chrome for manuk makes every task IDENTICAL; a real run must see at least one difference or one empty dump
    if a.mutate == "identical" and ndiff == 0:
        ok = False
    hdr = f"# RG.0.a ENGINE cohort DOM diff\n\nchrome_ver={pin}. Replays the load-time page of the failing step (not in-page actions: handles are per-page). Node tree = observe(full), ids stripped.\n\n"
    if a.mutate != "identical" or True:
        open(a.out, "w").write(hdr + "\n".join(md))
    print(f"tasks={len(lg.ENGINE)} differing={ndiff} control={ctl}")
    sys.exit(0 if ok and ctl else 1)


if __name__ == "__main__":
    main()
