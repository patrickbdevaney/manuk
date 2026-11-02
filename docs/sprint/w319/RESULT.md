# W3.19 — Web Bot Auth signing + reachability

chrome_ver=145.0.7632.116 (the pin; the Chrome backend gate ran on it. The reachability runs below use
manuk_net only, no Chrome.)

## What shipped
`manuk-mcp --agent-key <jwk file|json> --signature-agent <url>` (Ed25519, RFC 9421, tag `web-bot-auth`).
Every page request carries `Signature-Agent`, `Signature-Input`, `Signature`, plus `Content-Digest`
(sha-256) when it has a body. Covered: `@authority`, `signature-agent;key="sig1"`, `content-digest`.
`--print-key-directory` prints the JWKS the operator hosts at
`<signature-agent>/.well-known/http-message-signatures-directory`.
- manuk backend: `manuk_net::botauth`, applied where every page-initiated request is built (`send_raw`).
- Chrome backend: CDP `Fetch.requestPaused` -> `Fetch.continueRequest` with the original headers plus the
  signature headers (covers redirect hops, subresources, fetch/XHR, form POST). Residual: a request whose
  body Chrome does not inline in `postData` (large uploads) is signed without a Content-Digest.
- Identity is the operator's key. Signing sends that identity to every origin the page touches, third
  parties included.

## Gate (mcp/tests/botauth.rs, scripts/botauth-verify/verify.mjs)
Independent verifier = Cloudflare's `web-bot-auth` npm package 0.2.0 run locally. Page with stylesheet,
script, image, fetch POST, XHR POST, form POST:
- manuk backend: 7/7 requests verified, 3/3 bodies Content-Digest ok, 7/7 tampered-@authority rejected.
- chrome backend: 7/7, 3/3, 7/7.
- red runs: unsigned manuk run fails the verifier; Chrome run with `MANUK_CDP_MUTATE=no-sign` fails it.

## Reachability experiment (docs/sprint/w319)
Cohort: the 172 URLs in docs/bench/oracle-corpus.txt that are not in oracle-corpus-reachable.txt
(the K010 oracle-walled set; 172 by this frame, not 189). Each run GETs the page through manuk_net
(`engine/net/examples/wba_reach.rs`); `ok` = 2xx without a challenge marker. Key: RFC 9421 test key,
`--signature-agent https://agent.test` (unregistered with any verifier).

| condition | run 1 | run 2 | run 3 |
|---|---|---|---|
| unsigned ok | 137 | 135 | 136 |
| signed ok   | 133 | 131 | 130 |

Sites never ok unsigned and always ok signed: 1 (reuters.com). Always ok unsigned and never ok signed: 3
(coinbase.com, nih.gov, pexels.com). 1 < 10, so per the gate: recorded, stopped. An unregistered
identity does not open walls at the HTTP layer; most of the "walled" set already answers a plain manuk_net
GET (the wall is in what Chrome runs, not in the request), so this measures HTTP-layer reachability only.
A registered key could behave differently and is not measured here.
