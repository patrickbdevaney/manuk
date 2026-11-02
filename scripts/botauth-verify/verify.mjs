// W3.19 independent verifier: Cloudflare's web-bot-auth package, run locally.
// usage: node verify.mjs <requests.json> <key-directory.json>
// requests.json: [{method, host, path, headers: {name: value}, body_b64}]  (as the server saw them)
// For each request: verify as received (must pass), check Content-Digest against the body, then verify
// again with the @authority tampered (must be rejected). Prints one JSON summary line.
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { verify } from "web-bot-auth";
import { verifierFromJWK } from "web-bot-auth/crypto";

const reqs = JSON.parse(readFileSync(process.argv[2], "utf8"));
const dir = JSON.parse(readFileSync(process.argv[3], "utf8"));
const verifier = await verifierFromJWK(dir.keys[0]);

async function check(r, host) {
  const headers = new Headers();
  for (const [k, v] of Object.entries(r.headers)) {
    if (k.toLowerCase() !== "host") headers.set(k, v);
  }
  const request = new Request(`http://${host}${r.path}`, { method: r.method, headers });
  const seen = new Set();
  await verify(request, {
    resolver: (c) => {
      if (c.keyid !== verifier.keyid) throw new Error("unknown key");
      return verifier;
    },
    validate: ({ nonce }) => {
      if (seen.has(nonce)) return false;
      seen.add(nonce);
    },
  });
}

const out = { total: reqs.length, verified: 0, digest_ok: 0, bodies: 0, tamper_rejected: 0, failures: [] };
for (const r of reqs) {
  const label = `${r.method} ${r.path}`;
  try {
    await check(r, r.host);
    out.verified++;
  } catch (e) {
    out.failures.push(`${label}: ${e.message}`);
    continue;
  }
  const body = Buffer.from(r.body_b64 || "", "base64");
  if (body.length) {
    out.bodies++;
    const want = `sha-256=:${createHash("sha256").update(body).digest("base64")}:`;
    if (r.headers["content-digest"] === want) out.digest_ok++;
    else out.failures.push(`${label}: content-digest does not match body`);
  }
  try {
    await check(r, "evil.test");
    out.failures.push(`${label}: tampered @authority was ACCEPTED`);
  } catch {
    out.tamper_rejected++;
  }
}
console.log(JSON.stringify(out));
process.exit(out.failures.length || out.verified !== out.total ? 1 : 0);
