#!/usr/bin/env python3
"""W3.19.b: Web Bot Auth operator kit. Never writes a private key anywhere but --out (default: refuses paths inside the repo).

  wba-kit.py keygen    --out KEY.jwk
  wba-kit.py directory --key KEY.jwk --authority agent.example.com --out DIR [--expires-hours 24]
  wba-kit.py check     --key KEY.jwk --signature-agent https://agent.example.com [--form dict|string] [--url URL]
"""
import argparse, base64, hashlib, json, os, sys, time, urllib.request, urllib.error
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives import serialization

CRAWLTEST = "https://crawltest.com/cdn-cgi/web-bot-auth"
DIR_PATH = "/.well-known/http-message-signatures-directory"
DIR_CT = "application/http-message-signatures-directory+json"
b64u = lambda b: base64.urlsafe_b64encode(b).rstrip(b"=").decode()


def repo_root():
    return os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))


def load(path):
    j = json.load(open(path))
    seed = base64.urlsafe_b64decode(j["d"] + "=" * (-len(j["d"]) % 4))
    k = Ed25519PrivateKey.from_private_bytes(seed)
    x = b64u(k.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw))
    return k, x


def thumbprint(x):
    return b64u(hashlib.sha256(('{"crv":"Ed25519","kty":"OKP","x":"%s"}' % x).encode()).digest())


def sign_params(components, created, keyid, tag, expires_hours):
    # expires is always present and capped at 24 h
    exp = created + int(min(max(expires_hours, 0.001), 24) * 3600)
    inner = " ".join(components)
    return f'({inner});created={created};expires={exp};keyid="{keyid}";alg="ed25519";nonce="{b64u(os.urandom(48))}";tag="{tag}"'


def cmd_keygen(a):
    out = os.path.abspath(a.out)
    if out.startswith(repo_root() + os.sep):
        sys.exit("refusing to write a private key inside the repo")
    k = Ed25519PrivateKey.generate()
    seed = k.private_bytes(serialization.Encoding.Raw, serialization.PrivateFormat.Raw, serialization.NoEncryption())
    x = b64u(k.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw))
    fd = os.open(out, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, "w") as f:
        json.dump({"kty": "OKP", "crv": "Ed25519", "d": b64u(seed), "x": x}, f)
    print(f"wrote {out} (mode 600); keyid={thumbprint(x)}")


def cmd_directory(a):
    k, x = load(a.key)
    os.makedirs(a.out, exist_ok=True)
    body = json.dumps({"keys": [{"kty": "OKP", "crv": "Ed25519", "x": x}]}, separators=(",", ":"))
    keyid = thumbprint(x)
    created = int(time.time())
    params = sign_params(['"@authority";req'], created, keyid, "http-message-signatures-directory", a.expires_hours)
    base = f'"@authority";req: {a.authority}\n"@signature-params": {params}'
    sig = base64.b64encode(k.sign(base.encode())).decode()
    open(os.path.join(a.out, "http-message-signatures-directory"), "w").write(body)
    hdrs = {"Content-Type": DIR_CT, "Signature-Input": f"sig1={params}", "Signature": f"sig1=:{sig}:"}
    json.dump(hdrs, open(os.path.join(a.out, "headers.json"), "w"), indent=1)
    # the signature depends on @authority of the *request*, so it is not static: re-run before expires.
    print(f"wrote {a.out}/http-message-signatures-directory + headers.json; serve at https://{a.authority}{DIR_PATH} with those headers; keyid={keyid}; re-run before expires")


def signed_headers(k, x, agent, form, authority, expires_hours=0.1):
    keyid = thumbprint(x)
    created = int(time.time())
    if form == "dict":
        sa, comp, line = f'sig1="{agent}"', '"signature-agent";key="sig1"', f'"signature-agent";key="sig1": "{agent}"'
    else:
        sa, comp, line = f'"{agent}"', '"signature-agent"', f'"signature-agent": "{agent}"'
    params = sign_params(['"@authority"', comp], created, keyid, "web-bot-auth", expires_hours)
    base = f'"@authority": {authority}\n{line}\n"@signature-params": {params}'
    sig = base64.b64encode(k.sign(base.encode())).decode()
    return {"Signature-Agent": sa, "Signature-Input": f"sig1={params}", "Signature": f"sig1=:{sig}:"}


def cmd_check(a):
    k, x = load(a.key)
    from urllib.parse import urlparse
    u = urlparse(a.url)
    h = signed_headers(k, x, a.signature_agent, a.form, u.netloc.lower())
    h["User-Agent"] = "wba-kit/1"
    try:
        r = urllib.request.urlopen(urllib.request.Request(a.url, headers=h), timeout=20)
        code, body = r.status, r.read(300)
    except urllib.error.HTTPError as e:
        code, body = e.code, e.read(300)
    print(f"form={a.form} status={code} body={body[:200]!r}")
    sys.exit(0 if code in (200, 401) else 1)  # 401 = well-formed but key unknown to Cloudflare; 400 = malformed


def main():
    p = argparse.ArgumentParser()
    s = p.add_subparsers(dest="c", required=True)
    g = s.add_parser("keygen"); g.add_argument("--out", required=True); g.set_defaults(f=cmd_keygen)
    d = s.add_parser("directory"); d.add_argument("--key", required=True); d.add_argument("--authority", required=True)
    d.add_argument("--out", required=True); d.add_argument("--expires-hours", type=float, default=24); d.set_defaults(f=cmd_directory)
    c = s.add_parser("check"); c.add_argument("--key", required=True); c.add_argument("--signature-agent", required=True)
    c.add_argument("--form", choices=["dict", "string"], default="string"); c.add_argument("--url", default=CRAWLTEST); c.set_defaults(f=cmd_check)
    a = p.parse_args(); a.f(a)


main()
