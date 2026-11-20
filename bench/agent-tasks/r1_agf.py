#!/usr/bin/env python3
"""W6.R1 — actionable-geometry fidelity (AGF) on REAL's replica clones, and the near-bar miss survey.

  python3 -I r1_agf.py capture [--runs 3] [--sites ...]     # manuk (agf-probe) + pinned Chrome (CDP) -> raw JSON
  python3 -I r1_agf.py analyze [--mutate none|shift|nohit]  # raw JSON -> AGF per site, miss survey, Spearman vs REAL pass

Definitions (fixed here, before the numbers are read):
  * Targets: Chrome's accessibility tree, roles in ACT, non-empty name, a non-empty border box whose centre lies in the
    1024x768 viewport (below-fold and box-less targets are counted and reported, not scored).
  * Matching: (role, name) key; the k-th Chrome target with a key pairs with the k-th manuk target with that key.
  * A target is AGF-OK iff it is matched AND IoU(Chrome border box, manuk bbox) >= 0.9 AND manuk's top hit-test element at
    its bbox centre is the target or a descendant (the drivability criterion, computed by agf-probe on manuk).
  * AGF(site) = AGF-OK / scored targets. Misses are classed missing | geom | hit.
  * Miss survey keys (Chrome side, the oracle): font = first computed font-family; owner = display of the nearest
    ancestor that establishes a flex/grid/table formatting context (block-flow when none).
  * Spearman rho between site AGF (mean over runs) and the site's REAL manuk pass rate (rg2b, pinned chrome_ver).

Every row carries chrome_ver = $MANUK_CHROME_PIN. Run with python3 -I.
"""
import argparse, asyncio, collections, json, math, os, re, shutil, subprocess, sys, tempfile, time, urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SITES = "dashdish fly-unified gocalendar gomail networkin omnizon opendining staynb topwork udriver zilloft".split()
CHROME = os.environ.get("MANUK_CHROME", os.path.expanduser("~/.local/chrome-cft/bin/google-chrome"))
PIN = os.environ.get("MANUK_CHROME_PIN", "")
PROBE = os.path.join(ROOT, "target/release/agf-probe")
SENTINEL = os.path.join(ROOT, ".git/sprint-working")
RAW = os.path.join(ROOT, "docs/sprint/w6/raw/R1")
PASS_TSV = os.path.join(ROOT, "bench-queue/out/rg2b-real-manuk.tsv")
W, H = 1024, 768
ACT = {"button", "link", "checkbox", "radio", "textbox", "searchbox", "combobox", "switch",
       "menuitem", "menuitemcheckbox", "menuitemradio", "tab", "option"}
IOU_MIN = 0.9
# omnizon serves a DMCA takedown page (real_boot.py verdict): its single link is not a replica, so it is not scored.
NON_SCORABLE = {"omnizon"}
STYLE_FN = r"""function(){const cs=getComputedStyle(this);let owner='block-flow';let p=this.parentElement;
while(p){const d=getComputedStyle(p).display;if(/flex|grid|table/.test(d)){owner=d;break;}p=p.parentElement;}
return {font:cs.fontFamily.split(',')[0].replace(/["']/g,'').trim().toLowerCase(),owner:owner};}"""


def beat():
    try:
        os.utime(SENTINEL, None)
    except OSError:
        pass


def clean(s):
    return " ".join((s or "").split())


def url_of(site):
    if site.startswith("ctl-"):  # positive control: a local fixture both engines should agree on
        return "http://127.0.0.1:8765/" + site[4:] + ".html"
    return f"https://evals-{site}.vercel.app"


# ---------------------------------------------------------------- capture

def manuk_rows(url):
    r = subprocess.run([PROBE, url], capture_output=True, text=True, timeout=300)
    rows = []
    for line in r.stdout.splitlines():
        f = line.split("\t")
        if len(f) != 9:
            continue
        role, name, x, y, w, h, hit, hrole, hname = f
        box = None if x == "-" else [float(x), float(y), float(w), float(h)]
        rows.append({"role": role, "name": clean(name), "box": box, "hit": hit == "1"})
    return rows, r.returncode


class Cdp:
    def __init__(self, ws):
        self.ws, self.n, self.pend = ws, 0, {}
        self.events = asyncio.Queue()

    async def pump(self):
        async for m in self.ws:
            j = json.loads(m)
            if "id" in j:
                f = self.pend.pop(j["id"], None)
                if f and not f.done():
                    f.set_result(j)
            else:
                await self.events.put(j)

    async def call(self, method, params=None, timeout=60):
        self.n += 1
        i = self.n
        fut = asyncio.get_running_loop().create_future()
        self.pend[i] = fut
        await self.ws.send(json.dumps({"id": i, "method": method, "params": params or {}}))
        j = await asyncio.wait_for(fut, timeout)
        if "error" in j:
            raise RuntimeError(f"{method}: {j['error']}")
        return j.get("result", {})

    async def wait_event(self, name, timeout):
        end = time.time() + timeout
        while True:
            left = end - time.time()
            if left <= 0:
                return False
            try:
                j = await asyncio.wait_for(self.events.get(), left)
            except asyncio.TimeoutError:
                return False
            if j.get("method") == name:
                return True


async def chrome_targets(c):
    ax = await c.call("Accessibility.getFullAXTree", timeout=120)
    out, nobox = [], 0
    for n in ax["nodes"]:
        if n.get("ignored"):
            continue
        role = n.get("role", {}).get("value", "")
        name = clean(n.get("name", {}).get("value", ""))
        bid = n.get("backendDOMNodeId")
        if role not in ACT or not name or bid is None:
            continue
        try:
            bm = await c.call("DOM.getBoxModel", {"backendNodeId": bid})
        except RuntimeError:
            nobox += 1
            continue
        q = bm["model"]["border"]
        xs, ys = q[0::2], q[1::2]
        box = [min(xs), min(ys), max(xs) - min(xs), max(ys) - min(ys)]
        if box[2] <= 0 or box[3] <= 0:
            nobox += 1
            continue
        cx, cy = box[0] + box[2] / 2, box[1] + box[3] / 2
        rec = {"role": role, "name": name, "box": box, "inview": bool(0 <= cx < W and 0 <= cy < H)}
        if not rec["inview"]:
            out.append(rec)
            continue
        oid = (await c.call("DOM.resolveNode", {"backendNodeId": bid}))["object"]["objectId"]
        st = await c.call("Runtime.callFunctionOn", {"objectId": oid, "functionDeclaration": STYLE_FN, "returnByValue": True})
        rec.update(st["result"]["value"])
        try:
            hit = await c.call("DOM.getNodeForLocation", {"x": round(cx), "y": round(cy), "includeUserAgentShadowDOM": False})
            hoid = (await c.call("DOM.resolveNode", {"backendNodeId": hit["backendNodeId"]}))["object"]["objectId"]
            r = await c.call("Runtime.callFunctionOn", {
                "objectId": oid,
                "functionDeclaration": "function(h){return this===h||this.contains(h)}",
                "arguments": [{"objectId": hoid}], "returnByValue": True})
            rec["hit"] = bool(r["result"]["value"])
        except RuntimeError:
            rec["hit"] = False
        out.append(rec)
    return out, nobox


def launch_chrome(profile):
    log = os.path.join(profile, "chrome.stderr")
    p = subprocess.Popen([CHROME, "--headless=new", "--no-sandbox", "--disable-gpu", "--no-first-run",
                          "--remote-debugging-port=0", f"--user-data-dir={profile}", f"--window-size={W},{H}",
                          "about:blank"], stdout=subprocess.DEVNULL, stderr=open(log, "w"))
    end = time.time() + 40
    while time.time() < end:
        beat()
        m = re.search(r"DevTools listening on ws://127\.0\.0\.1:(\d+)/", open(log).read())
        if m:
            return p, int(m.group(1))
        time.sleep(0.2)
    p.kill()
    sys.exit("capture: chrome printed no DevTools port")


async def capture(runs, sites, out=RAW):
    import websockets
    if not PIN:
        sys.exit("refusing: MANUK_CHROME_PIN unset")
    os.makedirs(out, exist_ok=True)
    profile = tempfile.mkdtemp(prefix="r1agf-")
    proc, port = launch_chrome(profile)
    try:
        ver = json.load(urllib.request.urlopen(f"http://127.0.0.1:{port}/json/version"))["Browser"]
        if PIN not in ver:
            sys.exit(f"refusing: chrome is {ver}, pin is {PIN}")
        pages = json.load(urllib.request.urlopen(f"http://127.0.0.1:{port}/json/list"))
        page = next(p for p in pages if p["type"] == "page")
        async with websockets.connect(page["webSocketDebuggerUrl"], max_size=None) as ws:
            c = Cdp(ws)
            asyncio.create_task(c.pump())
            for m in ["Page.enable", "Runtime.enable", "DOM.enable", "Accessibility.enable"]:
                await c.call(m)
            await c.call("Emulation.setDeviceMetricsOverride", {"width": W, "height": H, "deviceScaleFactor": 1, "mobile": False})
            for r in range(1, runs + 1):
                for s in sites:
                    beat()
                    url = url_of(s)
                    mrows, mrc = manuk_rows(url)
                    beat()
                    while not c.events.empty():
                        c.events.get_nowait()
                    t0 = time.time()
                    await c.call("Page.navigate", {"url": url})
                    loaded = await c.wait_event("Page.loadEventFired", 60)
                    await asyncio.sleep(4)  # settle; manuk's probe gets its own navigate settle
                    await c.call("DOM.getDocument", {"depth": -1})
                    crows, nobox = await chrome_targets(c)
                    rec = {"site": s, "run": r, "url": url, "chrome_ver": ver, "chrome_pin": PIN,
                           "manuk_rc": mrc, "chrome_loaded": loaded, "chrome_nobox": nobox,
                           "chrome": crows, "manuk": mrows, "seconds": round(time.time() - t0, 1)}
                    with open(os.path.join(out, f"{s}_r{r}.json"), "w") as f:
                        json.dump(rec, f)
                    print(f"{s} r{r}: chrome={len(crows)} manuk={len(mrows)} nobox={nobox} loaded={loaded} rc={mrc}", flush=True)
    finally:
        proc.kill()
        shutil.rmtree(profile, ignore_errors=True)


# ---------------------------------------------------------------- analyze

def iou(a, b):
    iw = max(0.0, min(a[0] + a[2], b[0] + b[2]) - max(a[0], b[0]))
    ih = max(0.0, min(a[1] + a[3], b[1] + b[3]) - max(a[1], b[1]))
    inter = iw * ih
    union = a[2] * a[3] + b[2] * b[3] - inter
    return inter / union if union > 0 else 0.0


def score(chrome, manuk, mutate):
    if mutate == "shift":
        manuk = [dict(m, box=None if m["box"] is None else [m["box"][0] + 12] + m["box"][1:]) for m in manuk]
    elif mutate == "nohit":
        manuk = [dict(m, hit=False) for m in manuk]
    groups = collections.defaultdict(list)
    for m in manuk:
        groups[(m["role"], m["name"])].append(m)
    cnt = collections.Counter()
    misses, seen = [], collections.Counter()
    for c in chrome:
        if not c["inview"]:
            cnt["below_fold"] += 1
            continue
        key = (c["role"], c["name"])
        k = seen[key]
        seen[key] += 1
        cand = groups.get(key, [])
        if k >= len(cand):
            reason = "missing"
        else:
            m = cand[k]
            if m["box"] is None or iou(c["box"], m["box"]) < IOU_MIN:
                reason = "geom"
            elif not m["hit"]:
                reason = "hit"
            else:
                reason = "ok"
        cnt[reason] += 1
        if reason != "ok":
            misses.append({"role": c["role"], "name": c["name"], "reason": reason,
                           "font": c.get("font", ""), "owner": c.get("owner", "")})
    n = cnt["ok"] + cnt["geom"] + cnt["hit"] + cnt["missing"]
    return {"n": n, "ok": cnt["ok"], "geom": cnt["geom"], "hit": cnt["hit"], "missing": cnt["missing"],
            "below_fold": cnt["below_fold"], "agf": (cnt["ok"] / n) if n else None}, misses


def ranks(xs):
    order = sorted(range(len(xs)), key=lambda i: xs[i])
    r = [0.0] * len(xs)
    i = 0
    while i < len(xs):
        j = i
        while j + 1 < len(xs) and xs[order[j + 1]] == xs[order[i]]:
            j += 1
        for k in range(i, j + 1):
            r[order[k]] = (i + j) / 2 + 1
        i = j + 1
    return r


def spearman(x, y):
    rx, ry = ranks(x), ranks(y)
    n = len(x)
    mx, my = sum(rx) / n, sum(ry) / n
    cov = sum((a - mx) * (b - my) for a, b in zip(rx, ry))
    vx = sum((a - mx) ** 2 for a in rx)
    vy = sum((b - my) ** 2 for b in ry)
    return cov / math.sqrt(vx * vy) if vx and vy else None


def real_pass():
    d = collections.defaultdict(list)
    with open(PASS_TSV) as fh:
        for line in fh:
            f = line.rstrip("\n").split("\t")
            if len(f) < 9 or f[0] == "ts" or f[1] != "manuk" or f[3] != PIN:
                continue
            d[f[5]].append(int(f[8]))
    return {s: sum(v) / len(v) for s, v in d.items()}, {s: len(v) for s, v in d.items()}


def analyze(mutate, out_dir, only=None):
    files = sorted(os.listdir(RAW)) if os.path.isdir(RAW) else []
    recs = [json.load(open(os.path.join(RAW, f))) for f in files if f.endswith(".json")]
    if only:
        recs = [r for r in recs if r["site"] in only]
    if not recs:
        sys.exit("analyze: no raw captures")
    for r in recs:
        if r["chrome_pin"] != PIN:
            sys.exit(f"analyze: capture under chrome_pin {r['chrome_pin']} != {PIN}")
    by_site = collections.defaultdict(list)
    excluded = set()
    all_misses = []
    rows = []
    for r in sorted(recs, key=lambda x: (x["site"], x["run"])):
        s, misses = score(r["chrome"], r["manuk"], mutate)
        rows.append((r["site"], r["run"], s, r["chrome_ver"], r["chrome_pin"]))
        if s["n"] == 0 or r["site"] in NON_SCORABLE:
            # empty oracle (Chrome saw no scorable target) or a takedown page: excluded, never scored as 0
            excluded.add(r["site"])
            continue
        by_site[r["site"]].append(s["agf"])
        all_misses += misses
    passes, npass = real_pass()
    lines = ["site\trun\tscored\tok\tgeom\thit\tmissing\tbelow_fold\tagf\tchrome_ver\tchrome_pin\tmutate"]
    for site, run, s, ver, pin in rows:
        agf = "-" if s["agf"] is None else f"{s['agf']:.3f}"
        lines.append(f"{site}\t{run}\t{s['n']}\t{s['ok']}\t{s['geom']}\t{s['hit']}\t{s['missing']}\t{s['below_fold']}\t{agf}\t{ver}\t{pin}\t{mutate}")
    os.makedirs(out_dir, exist_ok=True)
    tag = "" if mutate == "none" else f".{mutate}"
    with open(os.path.join(out_dir, f"agf-runs{tag}.tsv"), "w") as fh:
        fh.write("\n".join(lines) + "\n")
    site_mean = {}
    print("site           runs  scored  AGF mean  band        REAL pass (n)  reasons(ok/geom/hit/missing)")
    for site in sorted(by_site, key=lambda x: SITES.index(x) if x in SITES else 99):
        if site not in by_site:
            continue
        vals = by_site[site]
        mean = sum(vals) / len(vals)
        site_mean[site] = mean
        rr = [x for x in rows if x[0] == site]
        n = rr[0][2]["n"]
        sums = {k: sum(x[2][k] for x in rr) // len(rr) for k in ("ok", "geom", "hit", "missing")}
        p = passes.get(site)
        ps = f"{p:.3f} ({npass.get(site, 0)})" if p is not None else "- (no REAL pass data)"
        print(f"{site:<14} {len(vals):>4} {n:>7}  {mean:8.3f}  [{min(vals):.3f},{max(vals):.3f}]  {ps:<22} "
              f"{sums['ok']}/{sums['geom']}/{sums['hit']}/{sums['missing']}")
    if excluded:
        print(f"excluded (zero scorable oracle targets): {sorted(excluded)}")
    print(f"\nmisses pooled over {len(recs)} captures: {len(all_misses)}")
    survey = []
    for kt in ("font", "owner"):
        cnt = collections.Counter(m[kt] or "(none)" for m in all_misses)
        tot = sum(cnt.values())
        for k, v in cnt.most_common():
            survey.append(f"{kt}\t{k}\t{v}\t{v / tot:.3f}" if tot else f"{kt}\t{k}\t{v}\t-")
    survey.insert(0, "key_type\tkey\tmisses\tshare_of_misses")
    with open(os.path.join(out_dir, f"miss-survey{tag}.tsv"), "w") as fh:
        fh.write("\n".join(survey) + "\n")
    rs = [(s, site_mean[s]) for s in site_mean if s in passes]
    if len(rs) >= 3:
        rho = spearman([a for _, a in rs], [passes[s] for s, _ in rs])
        print(f"Spearman rho(site AGF, REAL manuk pass) over n={len(rs)} sites: {rho}")
    else:
        rho = None
        print("Spearman: fewer than 3 sites with pass data")
    summary = {"mutate": mutate, "chrome_pin": PIN, "captures": len(recs), "site_agf": site_mean,
               "rho": rho, "n_rho": len(rs), "misses": len(all_misses)}
    with open(os.path.join(out_dir, f"summary{tag}.json"), "w") as fh:
        json.dump(summary, fh, indent=1)


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    a = sub.add_parser("capture")
    a.add_argument("--runs", type=int, default=3)
    a.add_argument("--sites", nargs="*", default=SITES)
    a.add_argument("--out", default=RAW, help="raw capture dir (default: the W6.R1 dir; use a new dir for repeats)")
    b = sub.add_parser("analyze")
    b.add_argument("--mutate", default="none", choices=["none", "shift", "nohit"])
    b.add_argument("--out", default=RAW)
    b.add_argument("--only", nargs="*", default=None)
    args = ap.parse_args()
    if args.cmd == "capture":
        asyncio.run(capture(args.runs, args.sites, args.out))
    else:
        analyze(args.mutate, args.out, args.only)


if __name__ == "__main__":
    main()
