#!/usr/bin/env python3
"""W6.R1.b.a.a: localhost proxy of evals-opendining.vercel.app. Injects, first in <head>, a setTimeout/setInterval/rAF logger
(window.__tl: [kind, delay, scheduled_at, fired_at, fn-head]). MODE: log | log+short:<delay_ms>  (short: every timer whose
delay is exactly <delay_ms> is rewritten to 0, the mutation). usage: r1baa_proxy.py PORT MODE"""
import http.server, sys, urllib.error, urllib.request
PORT = int(sys.argv[1]); MODE = sys.argv[2] if len(sys.argv) > 2 else "log"
SHORT = int(MODE.split(":")[1]) if MODE.startswith("log+short:") else -1
UP = "https://evals-opendining.vercel.app"
INJ = ("<script>(function(){var tl=window.__tl=[];var st=window.setTimeout,si=window.setInterval;var now=function(){return Math.round(performance.now())};"
"function w(orig,kind){return function(f,d){var a=[].slice.call(arguments);var dd=+d||0;var rec=[kind,dd,now(),null,String(f).slice(0,90)];tl.push(rec);"
"if(typeof f==='function'){a[0]=function(){rec[3]=now();return f.apply(this,arguments)}}"
"if(" + str(SHORT) + ">=0&&dd===" + str(SHORT) + "&&kind==='to')a[1]=0;return orig.apply(this,a)}}"
"document.addEventListener('load',function(e){var t=e.target;if(t&&t.tagName==='LINK')tl.push(['ev',0,now(),null,'load '+t.getAttribute('href').slice(0,60)])},true);document.addEventListener('error',function(e){var t=e.target;if(t&&t.tagName==='LINK')tl.push(['ev',0,now(),null,'error '+t.getAttribute('href').slice(0,60)])},true);"
"window.setTimeout=w(st,'to');window.setInterval=w(si,'iv')})()</script>").encode()
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        try: r = urllib.request.urlopen(urllib.request.Request(UP + self.path, headers={"User-Agent": "Mozilla/5.0"}))
        except urllib.error.HTTPError as e: self.send_response(e.code); self.send_header("Content-Length", "0"); self.end_headers(); return
        b = r.read(); ct = r.headers.get("Content-Type", "application/octet-stream")
        if "text/html" in ct and b"<head>" in b: b = b.replace(b"<head>", b"<head>" + INJ, 1)
        self.send_response(r.status); self.send_header("Content-Type", ct)
        self.send_header("Content-Length", str(len(b))); self.end_headers(); self.wfile.write(b)
    def log_message(self, *a): pass
http.server.ThreadingHTTPServer(("127.0.0.1", PORT), H).serve_forever()
