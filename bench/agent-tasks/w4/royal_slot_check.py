#!/usr/bin/env python3 -I
"""W4.P0.f.a.a.a.a.a.a: does The Royal Dine show a 7:00 PM slot for 4 people on 2024-07-20 on the OpenDining clone?

Drives the clone's search form with the PINNED Chrome over CDP (fresh profile per run): date -> party size -> time,
then reads the Royal Dine card's slot buttons. Writes one row per run to stdout (TSV, chrome_ver=$MANUK_CHROME_PIN).

  royal_slot_check.py [--runs N] [--day D] [--mutate-date] [--mutate-restaurant]

--mutate-restaurant  looks for a restaurant that does not exist: found must be False and satisfiable_7pm False (red).
--day D        July day to pick (default 20 = the task's date). --mutate-date types day 21 (a deliberately wrong date);
               the gate is only meaningful if the wrong date's card differs from the right one.
Exit 0 always after measuring; the verdict is read from the rows (7:00 PM present => task target satisfiable).
"""
import asyncio, json, os, subprocess, sys, tempfile, time, urllib.request
import websockets

PIN = os.environ.get("MANUK_CHROME_PIN") or sys.exit("MANUK_CHROME_PIN unset")
CHROME = os.path.expanduser(f"~/.local/chrome-cft/{PIN}/chrome-linux64/chrome")
SITE = "https://evals-opendining.vercel.app"
RESTAURANT = "The Royal Dine" if "--mutate-restaurant" not in sys.argv else "No Such Restaurant"  # mutant: card must be absent


def arg(name, default):
    return sys.argv[sys.argv.index(name) + 1] if name in sys.argv else default


async def center(p, js):
    return await p.ev(f"(()=>{{const e={js}; if(!e) return null; e.scrollIntoView({{block:'center'}}); const b=e.getBoundingClientRect(); return [b.x+b.width/2,b.y+b.height/2]}})()")


class Page:
    def __init__(self, ws):
        self.ws, self.n = ws, 0

    async def send(self, method, **params):
        self.n += 1
        i = self.n
        await self.ws.send(json.dumps({"id": i, "method": method, "params": params}))
        while True:
            r = json.loads(await self.ws.recv())
            if r.get("id") == i:
                return r.get("result", r)

    async def ev(self, expr):
        r = await self.send("Runtime.evaluate", expression=expr, returnByValue=True, awaitPromise=True)
        return r.get("result", {}).get("value")

    async def click_xy(self, xy):
        if not xy:
            raise RuntimeError("click target not found")
        for t in ("mousePressed", "mouseReleased"):
            await self.send("Input.dispatchMouseEvent", type=t, x=xy[0], y=xy[1], button="left", clickCount=1)


async def pick(p, combo_text, option):
    await p.click_xy(await center(p, f"[...document.querySelectorAll('[role=combobox]')].filter(e=>e.innerText.trim()=={json.dumps(combo_text)})[0]"))
    await asyncio.sleep(1)
    await p.click_xy(await center(p, f"[...document.querySelectorAll('li[role=option]')].filter(l=>l.innerText.trim()=={json.dumps(option)} && l.getBoundingClientRect().width>0)[0]"))
    await asyncio.sleep(1)


async def one_run(run, day):
    prof = tempfile.mkdtemp(prefix="royal-prof-")
    proc = subprocess.Popen([CHROME, "--headless=new", "--no-sandbox", "--remote-debugging-port=0", f"--user-data-dir={prof}",
                             "--window-size=1280,1000", "about:blank"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        port = None
        for _ in range(200):
            f = os.path.join(prof, "DevToolsActivePort")
            if os.path.exists(f):
                port = open(f).read().split()[0]
                break
            await asyncio.sleep(0.1)
        if not port:
            raise RuntimeError("no DevToolsActivePort")
        tgt = [x for x in json.load(urllib.request.urlopen(f"http://127.0.0.1:{port}/json")) if x["type"] == "page"][0]
        async with websockets.connect(tgt["webSocketDebuggerUrl"], max_size=None) as ws:
            p = Page(ws)
            await p.send("Page.enable")
            await p.send("Page.navigate", url=SITE)
            await asyncio.sleep(5)
            await p.click_xy(await center(p, "document.getElementById(':r1:')"))
            await asyncio.sleep(1)
            await p.click_xy(await center(p, f"[...document.querySelectorAll('button')].filter(b=>b.innerText.trim()=='{day}' && b.getBoundingClientRect().width>0)[0]"))
            await asyncio.sleep(0.5)
            await p.click_xy(await center(p, "[...document.querySelectorAll('button')].filter(b=>b.innerText.trim()=='OK' && b.getBoundingClientRect().width>0)[0]"))
            await asyncio.sleep(1)
            date_shown = await p.ev("document.getElementById(':r1:').value")
            await pick(p, "2 people", "4 people")
            await pick(p, "1:00 PM", "7:00 PM")
            await p.click_xy(await center(p, "[...document.querySelectorAll('button')].filter(b=>b.innerText.trim()==\"Let's go\")[0]"))
            await asyncio.sleep(3)
            url = await p.ev("location.href")
            card = await p.ev(f"""(()=>{{const h=[...document.querySelectorAll('h1,h2,h3,h4,h5,h6,p,span,div')].filter(e=>e.innerText&&e.innerText.trim()=={json.dumps(RESTAURANT)}&&e.children.length==0)[0];
              if(!h) return {{found:false}}; let c=h; for(let i=0;i<12&&c;i++){{ const bs=[...c.querySelectorAll('button')].filter(b=>/\\d+:\\d\\d [AP]M/.test(b.innerText)); if(bs.length) return {{found:true, slots:bs.map(b=>b.innerText.trim()), disabled:bs.filter(b=>b.disabled).length}}; c=c.parentElement;}} return {{found:true, slots:[]}};}})()""")
            return {"run": run, "date_shown": date_shown, "url": url, "card": card}
    finally:
        proc.kill()


async def main():
    runs = int(arg("--runs", "3"))
    day = arg("--day", "21" if "--mutate-date" in sys.argv else "20")
    print("run\tchrome_ver\tdate_shown\tfound\tslots\tdisabled\tsatisfiable_7pm")
    for r in range(1, runs + 1):
        out = await one_run(r, day)
        c = out["card"] or {}
        slots = c.get("slots", [])
        print(f"{r}\t{PIN}\t{out['date_shown']}\t{c.get('found')}\t{','.join(slots) or 'NONE'}\t{c.get('disabled', 0)}\t{'7:00 PM' in slots}")
        print("#url " + out["url"], file=sys.stderr)


if __name__ == "__main__":
    asyncio.run(main())
