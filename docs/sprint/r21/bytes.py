#!/usr/bin/env python3
"""R2.1 token attribution: bytes of manuk observe/extract vs Playwright's ARIA snapshot (the text the
playwright-mcp browser_snapshot tool returns) for the same page, plus the tools/list schema size.
Run: MANUK_CHROME_PIN=145.0.7632.116 /tmp/audit-venv/bin/python -I bytes.py"""
import json, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import probe
from playwright.sync_api import sync_playwright
CHROME = "/home/patrickd/.local/chrome-cft/bin/chromium"
_, base = probe.fixtures.serve()
urls = ["https://books.toscrape.com/", "https://quotes.toscrape.com/", "https://en.wikipedia.org/wiki/Alan_Turing",
        base + "/shop", base + "/login", base + "/catalog", base + "/product"]
m = probe.Mcp()
tl = m.rpc("tools/list", {})["result"]["tools"]
print("manuk tools/list bytes", len(json.dumps(tl)), "tools", len(tl))
os.environ["SHOW"] = "0"
rows = []
with sync_playwright() as p:
    b = p.chromium.launch(executable_path=CHROME, args=["--no-sandbox"]); pg = b.new_page()
    for u in urls:
        pg.goto(u, wait_until="load")
        snap = pg.locator("body").aria_snapshot()
        nav = m.call("navigate", url=u); obs = m.call("observe"); ext = m.call("extract", format="text")
        rows.append((u, len(nav), len(obs), len(ext), len(snap)))
    b.close()
print("url\tmanuk_navigate\tmanuk_observe\tmanuk_extract_text\tplaywright_aria_snapshot")
for r in rows: print("\t".join(map(str, r)))
