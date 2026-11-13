#!/usr/bin/env python3
"""RG.3.0.b.a.a.b.a: what is Chrome's `button "close"` and why does manuk never list it? For each site (zilloft, udriver,
gocalendar, gomail) load the start page under manuk (target/release/dom-eval, in-engine JS) and Chrome
(chrome_ver=$MANUK_CHROME_PIN, raw CDP) and run bench/agent-tasks/rg3ba_close.js after --wait seconds: every <button>/role=button
whose aria-label/title/text is "close" with its tag, class, computed display/visibility/opacity/rect and its parent's.
  rg3ba_close_dom.py [--waits 3,12] [--sites a,b] [--mutate blank-manuk] [--out FILE]   exit 0 = GATE PASS, 1 = FAIL
Writes docs/sprint/rg/RG3ba-close-dom.md. Run with `python3 -I`; needs MANUK_CHROME_PIN."""
import argparse, asyncio, json, os, subprocess, sys
HERE = os.path.dirname(os.path.abspath(__file__)); ROOT = os.path.dirname(os.path.dirname(HERE))
import importlib.util
spec = importlib.util.spec_from_file_location("cdpx", os.path.join(HERE, "rg3ba_cdp.py")); cdpx = importlib.util.module_from_spec(spec); spec.loader.exec_module(cdpx)
SITES = "zilloft udriver gocalendar gomail".split()
JS = os.path.join(HERE, "rg3ba_close.js")


def manuk(site, wait):
    r = subprocess.run([os.path.join(ROOT, "target/release/dom-eval"), f"https://evals-{site}.vercel.app", str(wait), JS], capture_output=True, text=True, timeout=300)
    return json.loads(r.stdout.strip().splitlines()[-1]) if r.stdout.strip() else {}


def chrome(site, wait):
    return json.loads(asyncio.run(cdpx.run(f"https://evals-{site}.vercel.app", open(JS).read(), wait)))


def fmt(e):
    if not e: return "-"
    return f"`<{e['tag']} class=\"{e['cls'][:40]}\" aria-label=\"{e['aria']}\">` display={e['display']} visibility={e['visibility']} opacity={e['opacity']} rect={e['rect']}"


def rows(site, backend, d):
    if not d: return [f"| {site} | {backend} | (probe returned nothing) | | | |"]
    meta = f"{d['n']} elements; banner text in body: {d['banner_text']}; role=alert nodes: {d['alerts']}"
    if not d["hits"]: return [f"| {site} | {backend} | NO close button in DOM ({meta}) | - | - | - |"]
    return [f"| {site} | {backend} | {fmt(h['el'])} | {fmt(h['parent'])} | {meta} | {len(d['hits'])} |" for h in d["hits"]]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--waits", default="3,12"); ap.add_argument("--sites", default=",".join(SITES))
    ap.add_argument("--mutate", choices=["blank-manuk"]); ap.add_argument("--out", default=os.path.join(ROOT, "docs/sprint/rg/RG3ba-close-dom.md"))
    a = ap.parse_args()
    pin = os.environ.get("MANUK_CHROME_PIN") or sys.exit("refusing: MANUK_CHROME_PIN unset")
    out, ok = [], True
    for s in a.sites.split(","):
        for w in a.waits.split(","):
            c, m = chrome(s, float(w)), manuk(s, float(w))
            if a.mutate == "blank-manuk": m = {}
            out += rows(s + f" @{w}s", "chrome", c) + rows(s + f" @{w}s", "manuk", m)
            ok &= bool(c) and bool(m)
            print(out[-2], "\n", out[-1], flush=True)
    md = ["# RG.3.0.b.a.a.b.a: the DOM node behind Chrome's `button \"close\"`", "",
          f"chrome_ver={pin}. DOM dump {a.waits} s after navigate on each backend (`bench/agent-tasks/rg3ba_close_dom.py`, JS `rg3ba_close.js`; manuk = `dom-eval`, in-engine JS). Columns: the close button (tag, class, aria-label, computed style, rect), its parent, page facts, number of matches.", "",
          "| site | backend | close button | parent | page | hits |", "|---|---|---|---|---|---|", *out, ""]
    if a.mutate is None: open(a.out, "w").write("\n".join(md))
    print("GATE", "PASS" if ok else "FAIL"); sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
