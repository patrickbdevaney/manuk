#!/usr/bin/env python3
"""A localhost OpenAI-compatible relay to Groq for agent evals: rate limits become WAITS, never task failures.

Why: `manuk agent-task` gives up on its first non-200 (agent/src/local.rs), so Groq's per-key tokens-per-minute cap
turned every multi-step task into "infra" (w6h1b-ab 2026-11-25: 36 of 80 runs). The relay sits between the agent and
Groq (the agent reaches it through MANUK_LLAMA_PORT, so no Rust changes) and:

  * paces requests on Groq's own x-ratelimit-* headers, and on a 429 waits out `retry-after` and retries — the agent
    never sees a rate limit, it only gets slower. Waits are counted per run, so the harness can take them OUT of the
    task's time budget (a slow model queue is not a slow engine);
  * uses ONE key at a time (owner rule 2026-11-25): it moves to the next GROQ_API_KEY_n only when the current key's
    wait would exceed SWITCH_AFTER_S or the key is refused (401/403). Key NAMES are logged, never values;
  * pins the request to the Groq preset (model, max_tokens 2048, temperature 0.2), so both drivers send identical
    request shapes whatever the local-port preset defaults to;
  * logs every call (the messages the model saw, its reply, tokens, waits) to the run's trace JSONL. That trace is the
    evidence `h1_attrib.py` uses to decide whether a failure was the engine's or the model's.

A request bigger than the key's whole per-minute budget can never succeed by waiting (Groq answers 413). It is passed
through as an error and logged as `oversize`: the run is a CAPACITY outcome, neither an engine nor a model failure.

  python3 -I bench/agent-tasks/groq_relay.py self-test     # offline, against a fake upstream
"""
import http.server, json, os, re, threading, time, urllib.error, urllib.request

GROQ = "https://api.groq.com/openai/v1"
PRESET = {"max_tokens": 2048, "temperature": 0.2}  # agent/src/local.rs OpenAiCompatBackend::groq + default temperature
MAX_WAIT_PER_CALL_S = 300   # total waiting for one call before it is handed back to the agent as infra
SWITCH_AFTER_S = 120        # a per-minute cap always clears within ~60 s, so a key is only left on a longer wait
                            # (a per-DAY cap) or a refusal: one key at a time, not rotation for throughput
SERVER_RETRIES = 3          # 5xx / connection errors


def parse_duration(s):
    """Groq reset headers: '7.66s', '1m2.5s', '450ms'. Seconds as float, or None."""
    if not s:
        return None
    s = s.strip()
    try:
        return float(s)
    except ValueError:
        pass
    total, ok = 0.0, False
    for num, unit in re.findall(r"([\d.]+)(ms|h|m|s)", s):
        total += float(num) * {"ms": 0.001, "s": 1, "m": 60, "h": 3600}[unit]
        ok = True
    return total if ok else None


class Relay:
    def __init__(self, keys, model, upstream=GROQ, sleep=time.sleep):
        """keys: [(name, value)] in fallback order."""
        self.keys, self.model, self.upstream, self.sleep = list(keys), model, upstream.rstrip("/"), sleep
        self.k = 0
        self.lock = threading.Lock()
        self.ready_at = 0.0        # monotonic time before which the current key should not be sent a large request
        self.remaining = None      # x-ratelimit-remaining-tokens from the last response
        self.trace, self.stats = None, {}
        self.begin(None)

    # ---- per-run bookkeeping (runs are sequential in the harnesses) ----
    def begin(self, trace_path):
        self.trace = trace_path
        self.stats = dict(calls=0, wait_s=0.0, model_s=0.0, prompt_tokens=0, completion_tokens=0,
                          max_prompt_tokens=0, rate_limited=0, oversize=0, infra=0, keys_used=[])
        if trace_path:
            os.makedirs(os.path.dirname(os.path.abspath(trace_path)), exist_ok=True)
            open(trace_path, "w").close()

    def end(self):
        s = dict(self.stats)
        s["wait_s"], s["model_s"] = round(s["wait_s"], 1), round(s["model_s"], 1)
        return s

    def _log(self, rec):
        if self.trace:
            with open(self.trace, "a") as f:
                f.write(json.dumps(rec) + "\n")

    def _key_name(self):
        return self.keys[self.k][0]

    def _wait(self, secs, why, waited):
        secs = max(0.0, secs)
        if secs:
            self.sleep(secs)
            self.stats["wait_s"] += secs
        return waited + secs

    def complete(self, body):
        """body: the agent's chat/completions JSON. Returns (status, response_bytes)."""
        with self.lock:
            body = dict(body, model=self.model, **PRESET)
            raw = json.dumps(body).encode()
            est = len(raw) // 3  # rough tokens; only used to decide whether to wait for the window first
            waited, server_errs, attempts = 0.0, 0, 0
            if self._key_name() not in self.stats["keys_used"]:
                self.stats["keys_used"].append(self._key_name())
            while True:
                attempts += 1
                if self.remaining is not None and est > self.remaining and self.ready_at > time.monotonic():
                    waited = self._wait(self.ready_at - time.monotonic(), "window", waited)
                t0 = time.monotonic()
                status, hdrs, data = self._post(raw)
                dt = time.monotonic() - t0
                if status == 200:
                    self._note_limits(hdrs)
                    break
                if status == 413 or (status in (400, 429) and b"Request too large" in data):
                    self.stats["oversize"] += 1
                    break
                if status == 429:
                    self.stats["rate_limited"] += 1
                    wait = parse_duration(hdrs.get("retry-after")) or parse_duration(
                        hdrs.get("x-ratelimit-reset-tokens")) or 10.0
                    if wait > SWITCH_AFTER_S and len(self.keys) > 1:
                        self._next_key()
                        continue
                    if waited + wait > MAX_WAIT_PER_CALL_S:
                        self.stats["infra"] += 1
                        break
                    waited = self._wait(wait + 0.5, "429", waited)
                    continue
                if status in (401, 403) and len(self.keys) > 1 and attempts <= len(self.keys):
                    self._next_key()
                    continue
                if (status >= 500 or status == 0) and server_errs < SERVER_RETRIES:
                    server_errs += 1
                    waited = self._wait(2.0 * server_errs, "5xx", waited)
                    continue
                if status >= 500 or status == 0:
                    self.stats["infra"] += 1
                break
            self.stats["calls"] += 1
            self.stats["model_s"] += dt
            reply, usage = None, {}
            if status == 200:
                try:
                    v = json.loads(data)
                    reply = v["choices"][0]["message"]["content"]
                    usage = v.get("usage") or {}
                except Exception:
                    pass
            pt = int(usage.get("prompt_tokens") or 0)
            self.stats["prompt_tokens"] += pt
            self.stats["completion_tokens"] += int(usage.get("completion_tokens") or 0)
            self.stats["max_prompt_tokens"] = max(self.stats["max_prompt_tokens"], pt)
            self._log(dict(call=self.stats["calls"], status=status, key=self._key_name(), attempts=attempts,
                           wait_s=round(waited, 1), model_s=round(dt, 2), usage=usage,
                           messages=body.get("messages"), reply=reply,
                           error=None if status == 200 else data[:400].decode("utf-8", "replace")))
            return status, data

    def _next_key(self):
        self.k = (self.k + 1) % len(self.keys)
        self.remaining, self.ready_at = None, 0.0
        if self._key_name() not in self.stats["keys_used"]:
            self.stats["keys_used"].append(self._key_name())

    def _note_limits(self, hdrs):
        rem = hdrs.get("x-ratelimit-remaining-tokens")
        reset = parse_duration(hdrs.get("x-ratelimit-reset-tokens"))
        if rem is not None:
            try:
                self.remaining = int(float(rem))
            except ValueError:
                self.remaining = None
        if reset is not None:
            self.ready_at = time.monotonic() + reset

    def _post(self, raw):
        req = urllib.request.Request(self.upstream + "/chat/completions", data=raw, method="POST", headers={
            "Content-Type": "application/json", "Authorization": "Bearer " + self.keys[self.k][1],
            "User-Agent": "manuk-bench-relay"})
        try:
            r = urllib.request.urlopen(req, timeout=120)
            return r.status, {k.lower(): v for k, v in r.headers.items()}, r.read()
        except urllib.error.HTTPError as e:
            return e.code, {k.lower(): v for k, v in e.headers.items()}, e.read()
        except Exception as e:  # connection reset, DNS, timeout
            return 0, {}, str(e).encode()


def serve(relay, port=0):
    """Start the relay on 127.0.0.1; returns (server, port). /v1/models advertises exactly the pinned model."""
    class H(http.server.BaseHTTPRequestHandler):
        def _send(self, status, data, ctype="application/json"):
            self.send_response(status if status else 502)
            self.send_header("Content-Type", ctype)
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

        def do_GET(self):
            if self.path.rstrip("/").endswith("/models"):
                return self._send(200, json.dumps({"data": [{"id": relay.model}]}).encode())
            self._send(404, b"{}")

        def do_POST(self):
            n = int(self.headers.get("Content-Length") or 0)
            try:
                body = json.loads(self.rfile.read(n) or b"{}")
            except ValueError:
                return self._send(400, b'{"error":"bad json"}')
            if not self.path.rstrip("/").endswith("/chat/completions"):
                return self._send(404, b"{}")
            status, data = relay.complete(body)
            self._send(status, data)

        def log_message(self, *a):
            pass

    srv = http.server.ThreadingHTTPServer(("127.0.0.1", port), H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv, srv.server_address[1]


def self_test():
    """A fake upstream scripts 429s, a 413 and a 5xx; the relay must wait, retry, switch keys and log names only."""
    import tempfile
    script, seen = [], []

    class Up(http.server.BaseHTTPRequestHandler):
        def do_POST(self):
            n = int(self.headers.get("Content-Length") or 0)
            body = json.loads(self.rfile.read(n))
            seen.append((self.headers.get("Authorization"), body))
            status, hdrs = script.pop(0)
            data = (json.dumps({"choices": [{"message": {"content": "ok"}}],
                                "usage": {"prompt_tokens": 100, "completion_tokens": 5}}) if status == 200
                    else '{"error":{"message":"%s"}}' % ("Request too large" if status == 413 else "x")).encode()
            self.send_response(status)
            for k, v in hdrs.items():
                self.send_header(k, v)
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

        def log_message(self, *a):
            pass

    up = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Up)
    threading.Thread(target=up.serve_forever, daemon=True).start()
    slept = []
    relay = Relay([("GROQ_API_KEY_1", "sk-one"), ("GROQ_API_KEY_2", "sk-two")], "qwen/test",
                  upstream="http://127.0.0.1:%d" % up.server_address[1], sleep=slept.append)
    srv, port = serve(relay)
    fails = 0

    def check(name, cond):
        nonlocal fails
        print(("ok   " if cond else "RED  ") + name)
        fails += not cond

    def call():
        req = urllib.request.Request("http://127.0.0.1:%d/v1/chat/completions" % port, method="POST",
                                     data=json.dumps({"model": "whatever", "max_tokens": 9, "temperature": 1,
                                                      "messages": [{"role": "user", "content": "hi"}]}).encode(),
                                     headers={"Content-Type": "application/json"})
        try:
            return urllib.request.urlopen(req, timeout=10).status
        except urllib.error.HTTPError as e:
            return e.code

    d = tempfile.mkdtemp()
    relay.begin(os.path.join(d, "t.jsonl"))
    script[:] = [(429, {"retry-after": "3"}), (200, {"x-ratelimit-remaining-tokens": "5000",
                                                    "x-ratelimit-reset-tokens": "1.5s"})]
    check("a short 429 is waited out and retried on the same key", call() == 200 and slept == [3.5]
          and seen[-1][0] == "Bearer sk-one" and seen[-2][0] == "Bearer sk-one")
    check("the request is pinned to the Groq preset", seen[-1][1]["model"] == "qwen/test"
          and seen[-1][1]["max_tokens"] == 2048 and seen[-1][1]["temperature"] == 0.2)
    script[:] = [(429, {"retry-after": "60"}), (200, {})]
    check("a per-minute 429 (60 s) is waited out on the SAME key", call() == 200 and seen[-1][0] == "Bearer sk-one")
    script[:] = [(429, {"retry-after": "3600"}), (200, {})]
    check("a long 429 falls back to the NEXT key, not a parallel one", call() == 200
          and seen[-1][0] == "Bearer sk-two")
    script[:] = [(413, {})]
    check("an oversize request is passed through and counted, not retried", call() == 413
          and relay.stats["oversize"] == 1)
    script[:] = [(503, {}), (200, {})]
    check("a 5xx is retried", call() == 200)
    st = relay.end()
    check("waits are counted for the run", st["wait_s"] >= 63.5 and st["rate_limited"] == 3)
    trace = open(os.path.join(d, "t.jsonl")).read()
    check("the trace names keys and never holds their values", "GROQ_API_KEY_2" in trace
          and "sk-one" not in trace and "sk-two" not in trace)
    check("the trace holds what the model saw and said", '"hi"' in trace and '"reply": "ok"' in trace)
    check("models endpoint advertises the pinned model", json.load(urllib.request.urlopen(
        "http://127.0.0.1:%d/v1/models" % port))["data"][0]["id"] == "qwen/test")
    check("duration parser", parse_duration("1m2.5s") == 62.5 and parse_duration("450ms") == 0.45)
    srv.shutdown(); up.shutdown()
    print("self-test " + ("green" if not fails else "RED (%d)" % fails))
    return 1 if fails else 0


if __name__ == "__main__":
    import sys
    sys.exit(self_test() if sys.argv[1:] == ["self-test"] else print(__doc__) or 2)
