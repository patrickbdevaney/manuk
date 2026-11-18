#!/usr/bin/env python3
"""W4.E2.b.a: check the post-JS image count (summary.tsv `post_js_img`) against an independent DOM count.

The independent count is pinned headless Chrome (CDP, the same driver r01-baseline uses): the number of
<img> start tags in document.documentElement.outerHTML after the load event, median of RUNS loads.
Chrome shares no code with manuk's DOM, so agreement is evidence that the manuk count is a real DOM count.

Per URL: AGREE if |manuk - chrome| <= max(0.10 x chrome, 2), else DISAGREE; NA if either side has no count.
The 2-image slack only matters for pages with a handful of images. Gate (exit 1 = RED): every URL with a
LEAN_LOWER verdict in summary.tsv must AGREE. Rows are written to <dir>/imgcheck.tsv.

Modes (run with -I so no local module can shadow the stdlib):
  python3 -I scripts/w4e2b-imgcheck.py --dir docs/sprint/w4e2b          # real check, MANUK_CHROME_PIN required
  python3 -I scripts/w4e2b-imgcheck.py --dir docs/sprint/w4e2b --mutant-static   # must exit 1 (static <img> count)
  python3 -I scripts/w4e2b-imgcheck.py --selftest
"""
import argparse
import html.parser
import importlib.util
import os
import re
import statistics
import sys
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
PIN = os.environ.get("MANUK_CHROME_PIN", "")
RUNS = 3
TOL_REL = 0.10
TOL_ABS = 2

_spec = importlib.util.spec_from_file_location("w4e0", os.path.join(HERE, "w4e0-footprint.py"))
w0 = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(w0)


class _ImgCounter(html.parser.HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.n = 0

    def handle_starttag(self, tag, attrs):
        if tag == "img":
            self.n += 1


def count_img_tags(doc):
    """Number of <img> start tags in an HTML string (a parser count, not a regex over the text)."""
    p = _ImgCounter()
    p.feed(doc)
    p.close()
    return p.n


def static_img_count(url):
    """The OLD count (raw HTML before JS). Only used by --mutant-static to prove the gate can go red."""
    try:
        req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0 (X11; Linux x86_64)"})
        with urllib.request.urlopen(req, timeout=20) as r:
            doc = r.read(2_000_000).decode("utf-8", "replace")
        return count_img_tags(doc)
    except Exception as e:
        return f"ERR:{type(e).__name__}"


def chrome_post_js_count(url):
    """<img> count in the pinned Chrome DOM after load. ERR:<status> if Chrome did not load the page."""
    cap = []
    _wall, _rss, _pss, _rc, status = w0.r01.run_chrome(w0.CHROME, url, 90, cap)
    if status != "OK" or not cap:
        return f"ERR:{status}"
    return count_img_tags(cap[0])


def compare(manuk, chrome):
    """AGREE / DISAGREE / NA for one URL (manuk and chrome are ints or ERR strings)."""
    if not isinstance(manuk, int) or not isinstance(chrome, int):
        return "NA"
    tol = max(TOL_REL * chrome, TOL_ABS)
    return "AGREE" if abs(manuk - chrome) <= tol else "DISAGREE"


def read_summary(path):
    """(url -> manuk post_js_img) and (url -> set of verdicts), from summary.tsv."""
    counts, verdicts = {}, {}
    with open(path) as f:
        header = f.readline().rstrip("\n").split("\t")
        for ln in f:
            row = dict(zip(header, ln.rstrip("\n").split("\t")))
            counts.setdefault(row["url"], row["post_js_img"])
            verdicts.setdefault(row["url"], set()).add(row["verdict"])
    return counts, verdicts


def as_count(v):
    return int(v) if isinstance(v, str) and v.isdigit() else v


def selftest():
    # the counter must see <img> through nesting, uppercase, and ignore look-alike text and comments
    doc = "<HTML><body><IMG src=a><div><img/></div><p>&lt;img&gt;</p><!-- <img> --><script>'<img>'</script></body></HTML>"
    assert count_img_tags(doc) == 2, count_img_tags(doc)
    # the comparison must go red on a clearly different count and green on a close one
    assert compare(190, 200) == "AGREE"
    assert compare(0, 325) == "DISAGREE"          # the static-count mutant on comix.to
    assert compare(2, 4) == "AGREE"              # small-count slack
    assert compare(5, 9) == "DISAGREE"
    assert compare("ERR:URLError", 10) == "NA"
    print("selftest GREEN")
    return True


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", default=os.path.join(ROOT, "docs", "sprint", "w4e2b"))
    ap.add_argument("--runs", type=int, default=RUNS)
    ap.add_argument("--mutant-static", action="store_true", help="use the static raw-HTML count as manuk's; must be RED")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()
    if a.selftest:
        return 0 if selftest() else 1
    if not PIN:
        print("MANUK_CHROME_PIN is not set", file=sys.stderr)
        return 1
    counts, verdicts = read_summary(os.path.join(a.dir, "summary.tsv"))
    rows, gate_ok = [], True
    for url, manuk in counts.items():
        w0.touch()
        manuk_v = static_img_count(url) if a.mutant_static else as_count(manuk)
        loads = [chrome_post_js_count(url) for _ in range(a.runs)]
        ok_loads = [x for x in loads if isinstance(x, int)]
        chrome_v = int(statistics.median(ok_loads)) if len(ok_loads) == a.runs else f"ERR:{len(ok_loads)}/{a.runs}"
        v = compare(manuk_v, chrome_v)
        required = "LEAN_LOWER" in verdicts.get(url, set())
        if required and v != "AGREE":
            gate_ok = False
        rows.append({"url": url, "manuk_post_js": manuk_v, "chrome_runs": ",".join(str(x) for x in loads),
                     "chrome_median": chrome_v, "verdict": v, "gate_required": "yes" if required else "no",
                     "chrome_ver": PIN})
        print(f"{v:9} manuk={manuk_v} chrome={chrome_v} runs={rows[-1]['chrome_runs']} {url}", file=sys.stderr)
    header = ["url", "manuk_post_js", "chrome_runs", "chrome_median", "verdict", "gate_required", "chrome_ver"]
    out = os.path.join(a.dir, "imgcheck-mutant.tsv" if a.mutant_static else "imgcheck.tsv")
    with open(out, "w") as f:
        f.write("\t".join(header) + "\n")
        for r in rows:
            f.write("\t".join(str(r[h]) for h in header) + "\n")
    print("\t".join(header))
    for r in rows:
        print("\t".join(str(r[h]) for h in header))
    print(f"gate {'GREEN' if gate_ok else 'RED'}: every LEAN_LOWER URL must AGREE"
          f"{' (mutant: static count)' if a.mutant_static else ''}; chrome_ver={PIN}")
    return 0 if gate_ok else 1


if __name__ == "__main__":
    sys.exit(main())
