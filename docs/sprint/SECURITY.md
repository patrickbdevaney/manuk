# Defence-in-depth census (SEC.DID)

Source: code reading + the gate files named below, 2026-10-25. A "layer" is an independent mechanism that
alone stops the path. Layer cell syntax: `name@gate-file` (pinned by that file) or `name@UNPINNED` (exists,
no gate of its own). `scripts/sec-did-check.py` enforces: every gate file exists; a row with fewer than 2
layers, or any UNPINNED layer, names a SEC.DID.* node that exists in GRAPH.tsv. "Mutation-checked" is only
claimed where the gate file's own header records it (marked `mut`); otherwise `SEC.DID.1` does the runs.

Scope note: "agent" = manuk-mcp / manuk-agent / manuk-bidi (optimizing JIT off + landlock + ephemeral jar). "shell" = the GUI
browser, which has none of those three.

| id | path (untrusted source -> sink) | layers | node |
|----|---------------------------------|--------|------|
| P1 | web page -> local disk READ (iframe/xhr/img/script `file:`) | net initiator check@engine/page/tests/g_sop_floor.rs (mut); navigation policy@engine/page/tests/g_sop_floor.rs (mut); resolve_url guard@engine/page/tests/g_sop_floor.rs (mut); redirect-into-file block@UNPINNED; frame loader is_local_url@UNPINNED; landlock ($HOME unreadable, agent only)@mcp/tests/confine.rs | SEC.DID.2 |
| P2 | web page -> local disk WRITE (download) | sanitize_filename + dedupe (no overwrite)@engine/net/src/downloads.rs; landlock write-only-cache (agent only)@mcp/tests/confine.rs. Shell: first layer only, and a response triggers the save with no prompt | SEC.DID.3 |
| P3 | web page -> localhost / RFC1918 / link-local (fetch, xhr, img, ws) | post-DNS GuardResolver@engine/net/tests/g_private_egress.rs; literal-host check@engine/net/tests/g_private_egress.rs; CORS preflight@engine/page/tests/g_page_fetch_preflight.rs; WS resolve_for_dial@engine/net/tests/g_private_egress.rs | - |
| P3b | web page -> private net via SOCKS proxy-resolved hostname | literal check only; names are resolved by the proxy (documented gap in the W3.10 wiki page) | SEC.DID.4 |
| P4 | web page -> cookie store (document.cookie, cross-site send) | HttpOnly filtered from script@engine/page/tests/g_cookie_attributes.rs; ephemeral jar per process (agent)@mcp/tests/isolation.rs; per-origin grant of human cookies@mcp/tests/isolation.rs | - |
| P5 | web page -> password / credential store | NO SINK: grep finds no password manager or autofill in shell/engine/mcp. Tripwire absent | SEC.DID.5 |
| P6 | web page -> OS clipboard READ | host clipboard is never seeded in shell/mcp/bidi (set_host_clipboard has no non-test caller), so readText sees only page-written text@UNPINNED | SEC.DID.6 |
| P6b | web page -> OS clipboard WRITE (overwrite without gesture) | gesture gating not verified in this census | SEC.DID.6 |
| P7 | web page -> process exec | no Command::new in shipped shell/engine/mcp/agent/bidi code paths (test and dev-tool uses only)@UNPINNED; landlock execve denied (agent only)@mcp/tests/confine.rs | SEC.DID.7 |
| P8 | cross-origin frame -> embedder DOM / embedder -> frame | contentDocument/contentWindow.document SecurityError@engine/page/tests/g_sop_floor.rs (mut); X-Frame-Options/frame-ancestors refusal@engine/page/tests/g_framing.rs | - |
| P9 | other tab / window.opener / named window access | not probed: shell tabs are separate Pages sharing the cookie jar and storage, no test of cross-tab reach | SEC.DID.8 |
| P10 | page script -> engine memory corruption (SpiderMonkey/JIT, wasm) | optimizing JIT off (baseline tiers on; `--jitless` = none)@engine/js/tests/hardening.rs (mut, MANUK_TEST_BREAK); landlock@mcp/tests/confine.rs (agent only). Shell: neither. SpiderMonkey pin is past EOL (SYNTHESIS) | SEC.DID.9 |
| P11 | page content -> agent tool calls (prompt injection) | hidden-text shield@mcp/tests/session.rs. Visible injected text: no layer. navigate/act/type have no origin allowlist, so an injected `navigate` to an attacker URL carrying data in its query is unblocked | SEC.DID.10 |
| P12 | agent `storage_state` path arg -> disk write/read | landlock write set@mcp/tests/confine.rs | SEC.DID.11 |
| P13 | page script injection (XSS-style) via CSP | script-src enforcement@engine/page/tests/g_csp.rs (other CSP directives unimplemented per csp.rs header) | SEC.DID.12 |

## Not yet probed (no claim made)
mixed content / https downgrade, `javascript:` and `data:` top-level navigation, `window.open` features, WebRTC,
service-worker scope, extension/IPC surfaces of the shell. SEC.DID.12 covers the probe.

## JIT layer (JIT.BASE, owner decision 2026-10-29)

Agent mode runs SpiderMonkey with the Baseline interpreter and Baseline compiler ON; Ion/Warp, native regexp
and both wasm JIT tiers OFF. `--jitless` turns every tier off. Residual risk: the baseline compiler still emits
machine code, so a baseline-tier bug is reachable; the optimizing tier, where the recent SpiderMonkey JIT CVEs
sit, is not. Landlock (no exec, read-only fs) bounds what a compromise could do. Measured on a hot numeric loop:
full 30 ms, baseline 394 ms, jitless 3620 ms (9.2x), chrome_ver=145.0.7632.116.
