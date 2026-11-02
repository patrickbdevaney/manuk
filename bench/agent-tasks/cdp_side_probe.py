#!/usr/bin/env python3 -I
"""W4.P0.f probe: drive the pinned Chrome over raw CDP (the cdp-layer's input path: Input.insertText + real mouse events)
through opendining's reservation flow, then read the clone's /finish state the way real_run.py does.
Answers two questions with no LLM: (a) does the phone field accept a typed number, (b) does a completed booking
reach /finish as bookingDetailsDiff.added[0]. Prints one JSON line. Needs MANUK_CHROME_PIN and the pinned chrome on PATH.
Usage: cdp_side_probe.py [--mutate no-submit]    (no-submit skips the Complete click: (b) must then be false)
"""
import asyncio, json, os, subprocess, sys, tempfile, time, urllib.request
import websockets

SITE = "https://evals-opendining.vercel.app"
CHROME = os.environ.get("MANUK_CHROME", "/home/patrickd/.local/chrome-cft/bin/google-chrome")
PIN = os.environ.get("MANUK_CHROME_PIN", "145.0.7632.116")


class Cdp:
    def __init__(self, ws):
        self.ws, self.n, self.pend = ws, 0, {}

    async def pump(self):
        async for m in self.ws:
            j = json.loads(m)
            if "id" in j and j["id"] in self.pend:
                self.pend.pop(j["id"]).set_result(j)

    async def send(self, method, **params):
        self.n += 1
        f = asyncio.get_event_loop().create_future()
        self.pend[self.n] = f
        await self.ws.send(json.dumps({"id": self.n, "method": method, "params": params}))
        return (await asyncio.wait_for(f, 30)).get("result", {})

    async def ev(self, js):
        r = await self.send("Runtime.evaluate", expression=js, returnByValue=True, awaitPromise=True)
        return r.get("result", {}).get("value")

    async def goto(self, url, wait=4):
        await self.send("Page.navigate", url=url)
        await asyncio.sleep(wait)

    async def click(self, js_rect):
        r = await self.ev(f"(()=>{{const e={js_rect};if(!e)return null;e.scrollIntoView({{block:'center'}});const b=e.getBoundingClientRect();return [b.x+b.width/2,b.y+b.height/2]}})()")
        if not r:
            return False
        for t in ("mousePressed", "mouseReleased"):
            await self.send("Input.dispatchMouseEvent", type=t, x=r[0], y=r[1], button="left", clickCount=1)
        return True


async def main(mutate):
    d = tempfile.mkdtemp()
    p = subprocess.Popen([CHROME, "--headless=new", "--no-sandbox", "--disable-gpu", "--remote-debugging-port=9333",
                          f"--user-data-dir={d}", "about:blank"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    out = {"chrome_ver": PIN}
    try:
        for _ in range(40):
            try:
                tabs = json.load(urllib.request.urlopen("http://127.0.0.1:9333/json"))
                break
            except Exception:
                time.sleep(0.5)
        wsurl = next(t for t in tabs if t["type"] == "page")["webSocketDebuggerUrl"]
        async with websockets.connect(wsurl, max_size=2**26) as ws:
            c = Cdp(ws)
            asyncio.ensure_future(c.pump())
            await c.send("Page.enable")
            await c.goto(f"{SITE}/config?run_id=0&task_id=opendining-8&latency=0")
            await c.goto(f"{SITE}/finish")
            rid = await c.ev("(()=>{const m=document.body.innerText;return null})()")
            await c.goto(SITE, 6)
            link = await c.ev("document.querySelector('a[href^=\"/booking?index=\"]').getAttribute('href')")
            out["booking_link"] = bool(link)
            await c.goto(SITE + link, 6)
            out["form_before"] = await c.ev("document.querySelector('input[type=tel]').value")
            await c.click("document.querySelector('input[type=tel]')")
            await c.send("Input.insertText", text=os.environ.get("PROBE_TEL", "2024561111"))
            await asyncio.sleep(0.5)
            out["tel_after_insert"] = await c.ev("document.querySelector('input[type=tel]').value")
            out["tel_invalid"] = await c.ev("document.querySelector('input[type=tel]').getAttribute('aria-invalid')")
            if mutate != "no-submit":
                ok = await c.click("[...document.querySelectorAll('button')].find(b=>/complete reservation/i.test(b.innerText))")
                out["clicked_complete"] = ok
                await asyncio.sleep(4)
            out["page_text"] = (await c.ev("document.body.innerText.includes('Booking Complete')"))
            out["body_tail"] = (await c.ev("document.body.innerText.slice(-400)"))
            await c.goto(f"{SITE}/finish", 4)
            txt = await c.ev("(document.querySelector('pre')||document.body).innerText")
            try:
                st = json.loads(txt)
            except Exception:
                st = {}
            added = ((st.get("bookingDetailsDiff") or {}).get("added") or {})
            out["state_booking_added"] = bool(added.get("0"))
            out["state_tel_len"] = len(((added.get("0") or {}).get("tel")) or "")
            out["state_keys"] = sorted(st.keys())[:6]
    finally:
        p.kill()
    print(json.dumps(out))
    return 0


sys.exit(asyncio.run(main(sys.argv[2] if len(sys.argv) > 2 and sys.argv[1] == "--mutate" else "")))
