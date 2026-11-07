# Registering a Web Bot Auth identity (owner steps)

Kit: `scripts/wba-kit/wba-kit.py` (needs python3 + `cryptography`). One key per operator; never share or commit it.
Never put the key inside the repo (keygen refuses).

1. Generate: `python3 scripts/wba-kit/wba-kit.py keygen --out ~/.config/manuk/wba.jwk` (mode 600).
2. Pick a domain you control, e.g. `agent.<yourdomain>`. Produce the directory:
   `python3 scripts/wba-kit/wba-kit.py directory --key ~/.config/manuk/wba.jwk --authority agent.<yourdomain> --out ./wba-dir`
   Serve `wba-dir/http-message-signatures-directory` at
   `https://agent.<yourdomain>/.well-known/http-message-signatures-directory` with the headers in `headers.json`
   (`Content-Type: application/http-message-signatures-directory+json`, `Signature-Input`, `Signature`).
   The signature expires within 24 h (cap enforced); re-run and re-serve before then (cron).
3. Sanity check: `python3 scripts/wba-kit/wba-kit.py check --key ~/.config/manuk/wba.jwk --signature-agent https://agent.<yourdomain> --form string`
   (and `--form dict`). Expect HTTP 401 "unknown public key" from crawltest.com until registered. 400 means malformed.
   `--form string` = bare `Signature-Agent: "https://..."` as Cloudflare documents; `--form dict` = `sig1="https://..."` per
   draft-ietf-webbotauth-httpsig-protocol-01. manuk-mcp currently emits the dict form.
4. Submit to Cloudflare: dashboard > Manage Account > Configurations > Bot Submission Form, Request Signature, label Intermediary,
   giving the directory URL.
5. After approval the same `check` returns 200; then run W3.19.a with `manuk-mcp --agent-key ~/.config/manuk/wba.jwk --signature-agent https://agent.<yourdomain>`.
AWS WAF has no outside-operator path yet.
