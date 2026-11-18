#!/usr/bin/env python3
"""W4.E0.b.a.a: chrome-proc PSS on the 7 paired URLs, 3 runs, one session. Output: TSV on stdout.
Same driver as scripts/w4e0-footprint.py run_urls (r01-baseline.run_chrome), chrome arm only.
Usage: MANUK_CHROME_PIN=145.0.7632.116 python3 -I docs/sprint/w4ebaa/measure.py [--run-tag TAG] > out.tsv
"""
import importlib.util, os, sys, time
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(HERE)))
PIN = os.environ.get("MANUK_CHROME_PIN", "")
CHROME = os.environ.get("CHROME_BIN") or os.path.join(os.path.expanduser("~/.local/chrome-cft"), PIN, "chrome-linux64", "chrome")
PAIRED = ["https://admin.zoomph.com/", "https://app.ordertime.com/", "https://client.dkitrxmdwoqruvsi.net/",
          "https://comix.to/", "https://ebanking.bdm-sa.com/", "https://www.mobile.bg/", "https://www.puentedemando.com/"]
spec = importlib.util.spec_from_file_location("r01", os.path.join(ROOT, "scripts", "r01-baseline.py"))
r01 = importlib.util.module_from_spec(spec); spec.loader.exec_module(r01)
spec2 = importlib.util.spec_from_file_location("w4e0", os.path.join(ROOT, "scripts", "w4e0-footprint.py"))
w4e0 = importlib.util.module_from_spec(spec2); spec2.loader.exec_module(w4e0)
TAG = sys.argv[sys.argv.index("--run-tag") + 1] if "--run-tag" in sys.argv else "now"
print("axis\tarm\tn\trun\ttarget\twall_s\tpss_kb\tstatus\tnote\tchrome_ver\tsession")
print(f"# chrome={CHROME} pin={PIN} started={time.strftime('%Y-%m-%dT%H:%M:%S')}", file=sys.stderr)
for run in range(1, 4):
    for url in PAIRED:
        html = []
        w, rss, pss, _, st = r01.run_chrome(CHROME, url, 60.0, html)
        note = ""
        if st == "OK" and html and w4e0.challenge_marker(html[0]):
            st = "CHALLENGE"
        print(f"urls\tchrome-proc\t1\t{run}\t{url}\t{w:.3f}\t{pss}\t{st}\t{note}\t{PIN}\t{TAG}", flush=True)
        print(f"run{run} {st} {w:.2f}s pss={pss/1024:.1f}MB {url}", file=sys.stderr)
        try:
            os.utime(os.path.join(ROOT, ".git", "sprint-working"))
        except OSError:
            pass
