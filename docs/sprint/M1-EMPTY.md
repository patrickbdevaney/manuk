# M1 EMPTY — why two corpus sites read EMPTY in the tri-sweep

Node R0.4.a.a.a. Run: `docs/bench/tri-sweep-20261025-020324.tsv`, `chrome_ver=145.0.7632.116`, `--m1-timeout 120`.
Both sites were re-run with `manuk-wpt fidelity --urls <url>` under the same pin and gave the same verdicts.
Saved stderr (ANSI stripped; `amazon` is verbatim, `nytimes` is an excerpt: only the classification, budget and warning-summary lines, because the full stderr is about 40 KB of repeated warnings) is in `docs/sprint/m1-empty/`.
Check: `scripts/m1-empty-check.sh` (each cited line must appear verbatim in its saved log).

| site | cause | cited line |
|---|---|---|
| amazon.com | blank origin response: the server answered 202 with a zero-byte body, so there was no document to measure. The instrument does not show a bot-wall label here, so "bot wall" is not established. | `UNMEASURABLE [empty-202]: the origin answered 202 with a ZERO-BYTE body` (`docs/sprint/m1-empty/amazon.m1.log`) |
| nytimes.com | bot wall: the origin answered 403 and refused the client. Rendering cannot move this. | `UNMEASURABLE [bot-wall-403]: the origin answered 403 and refused this client` (`docs/sprint/m1-empty/nytimes.m1.log`) |

Neither site is a blank render (the page never reached the renderer) or an oracle-empty case (the oracle was not consulted, since fidelity stopped at the origin response).

The EMPTY bucket is produced by `scripts/tri-classify.sh`'s catch-all, which does not read these `UNMEASURABLE [...]` labels. R0.4.a.a.a.a now tags them BOTWALL (bot-wall-NNN) and BLANK (empty-NNN) in `scripts/tri-sweep.sh`.

## Follow-up R0.4.a.a.a.b: plain curl of amazon.com

Evidence: `docs/sprint/m1-empty/amazon.curl.txt` (gate `scripts/m1-amazon-curl-check.sh`). Under plain curl the origin does not reproduce the 202 zero-byte answer:

- `curl -i https://amazon.com/` returns `HTTP/1.1 301 Moved Permanently`, 163 bytes, `Location: https://www.amazon.com/`.
- `curl -sSL https://amazon.com/` ends at `https://www.amazon.com/` with status 200 and 1,267,772 bytes (1 redirect).
- With a desktop-browser User-Agent, the 301 repeats (2 of 2 runs); one `-L` run ended in `Connection reset by peer`.

So the empty-202 is not visible to a plain HTTP client here. It is not shown to be an anti-bot response, and it is not shown to be absent under manuk's own request. The amazon row stays `[empty-202]` (blank origin response), not `[bot-wall]`. Follow-up: R0.4.a.a.a.b.a reproduces the request through manuk's own fetch.
