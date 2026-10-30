# W3.18.b: REAL grader prerequisites

chrome_ver=145.0.7632.116 (MANUK_CHROME_PIN)
Checked 2026-10-29 by the headless sprint agent (node W3.18.b).

## 1. OpenAI grader credential

- Status: **ABSENT** in the node's environment (the OpenAI key variable unset; checked by presence only, the value was never read or printed).
- Why it matters: 64 of REAL's evals are `llm_boolean` and are graded by gpt-4.1 (VERDICT.md, gate 2 note). Without a key those 64 cannot be scored by REAL's unmodified grader.
- Not searched: no filesystem, env-file, or other-project search for a key was done, per the node's security rule.
- Consequence: W3.18.a.a (re-open W3.18 gate 2) stays blocked.

**Owner decision needed:** provide a grader key that is explicitly for this suite, or accept the deterministic subset. Under the deterministic subset, `llm_boolean` evals are marked NOT SCORED on both arms alike, and gate 2 is judged on the remaining evals only. The agent will not substitute a stub grader.

## 2. omnizon reachability

Probe: `curl -sS -L https://evals-omnizon.vercel.app`

- HTTP status: **451** (Unavailable For Legal Reasons), final URL unchanged, 103 bytes.
- Body: `This content has been blocked for legal reasons` / `DMCA_TAKEDOWN`.
- Title: none in the body (the gate's "title" field is empty).
- The `DMCA` text is what `real_boot.py` classifies as FAIL, consistent with `boot-manuk-after.tsv` and `boot-chrome.tsv`.

Alternate URLs were **not** searched for. The page is a legal takedown, and looking for a mirror to get around it is outside this node's scope. Omnizon therefore stays a named residual (10/11 boot on both backends). Owner decision: whether to drop omnizon from the REAL slice or to supply a lawful source.

## Result

Gate: key present/absent recorded, omnizon checked. The key is absent, so the node is **BLOCKED** on the owner decision in section 1.
