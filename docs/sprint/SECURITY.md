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
| P1 | web page -> local disk READ (iframe/xhr/img/script `file:`) | net initiator check@engine/page/tests/g_sop_floor.rs (mut); navigation policy@engine/page/tests/g_sop_floor.rs (mut); resolve_url guard@engine/page/tests/g_sop_floor.rs (mut); redirect-into-file block (mut)@engine/net/tests/g_redirect_into_file.rs; frame loader is_local_url (mut)@engine/page/src/lib.rs sec_did_2_loader_filter; landlock (mut) ($HOME unreadable, agent only)@mcp/tests/confine.rs | none (SEC.DID.2 pinned both) |
| P2 | web page -> local disk WRITE (download) | sanitize_filename + dedupe (no overwrite)@engine/net/src/downloads.rs; landlock (mut) write-only-cache (agent only)@mcp/tests/confine.rs. Shell: second layer = user-gesture requirement (downloads::gesture_permits, 60s window)@engine/net/tests/g_download_gesture.rs (mutant RED) | SEC.DID.3 done |
| P3 | web page -> localhost / RFC1918 / link-local (fetch, xhr, img, ws) | post-DNS GuardResolver (mut)@engine/net/tests/g_private_egress.rs; literal-host check (mut)@engine/net/tests/g_private_egress.rs; CORS preflight (mut)@engine/page/tests/g_page_fetch_preflight.rs; WS resolve_for_dial (mut)@engine/net/tests/g_private_egress.rs | - |
| P3b | web page -> private net via SOCKS proxy-resolved hostname | literal check (mut)@engine/net/tests/g_private_egress.rs; local-only names (localhost, *.local, *.localhost, *.internal, dotless) refused before the proxy is dialled, check_proxy_hostname (mut)@engine/net/tests/g_private_egress.rs. Residual: a public-looking name the proxy maps to a LAN IP is not visible to us | SEC.DID.4 done |
| P4 | web page -> cookie store (document.cookie, cross-site send) | HttpOnly filtered from script (mut)@engine/page/tests/g_cookie_attributes.rs; ephemeral jar per process (agent) (mut)@mcp/tests/isolation.rs; per-origin grant of human cookies (mut)@mcp/tests/isolation.rs | SEC.DID.1.a |
| P5 | web page -> password / credential store | NO SINK: no page-reachable password manager or autofill in shell/engine/mcp. Tripwire: scripts/sec-did5-tripwire.py fails on any credential/autofill identifier not in docs/sprint/SEC-DID5-ALLOWLIST.tsv (mutant-checked, not a .rs gate) | SEC.DID.5 |
| P6 | web page -> OS clipboard READ | host clipboard is never seeded in shell/mcp/bidi (set_host_clipboard has no non-test caller), so readText sees only page-written text@UNPINNED; SEC.DID.6 additionally rejects readText/read with no user gesture (g_clipboard_gesture) | SEC.DID.6 |
| P6b | web page -> OS clipboard WRITE (overwrite without gesture) | writeText/write with no user gesture reject and never reach the OS bridge (g_clipboard_gesture, mutant RED) | SEC.DID.6 |
| P7 | web page -> process exec | reviewed exec inventory of 6 sites in shipped Rust (agent confine selftest, engine/net secret provider `op`/`secret-tool`, mcp Chrome launch, mcp zygote fork), pinned by scripts/sec-did7-exec-tripwire.py (not a .rs gate; a new site or extra occurrence turns it RED, mutant-checked); landlock (mut) execve denied (agent only)@mcp/tests/confine.rs. Correction: the earlier "no Command::new" claim was wrong, the secret provider is shipped code. Residual: SEC.DID.7.a (agent-supplied secret name reaches argv) | SEC.DID.7 |
| P8 | cross-origin frame -> embedder DOM / embedder -> frame | contentDocument/contentWindow.document SecurityError@engine/page/tests/g_sop_floor.rs (mut); X-Frame-Options/frame-ancestors refusal (mut)@engine/page/tests/g_framing.rs | - |
| P9 | other tab / window.opener / named window access | not probed: shell tabs are separate Pages sharing the cookie jar and storage, no test of cross-tab reach | SEC.DID.8 |
| P10 | page script -> engine memory corruption (SpiderMonkey/JIT, wasm) | optimizing JIT off (baseline tiers on; `--jitless` = none)@engine/js/tests/hardening.rs (mut, MANUK_TEST_BREAK); landlock (mut)@mcp/tests/confine.rs (agent only). Shell: neither. SpiderMonkey pin is past EOL (SYNTHESIS) | SEC.DID.9 |
| P11 | page content -> agent tool calls (prompt injection) | hidden-text shield (mut)@mcp/tests/session.rs. Visible injected text: no layer. navigate/act/type have no origin allowlist, so an injected `navigate` to an attacker URL carrying data in its query is unblocked | SEC.DID.10 |
| P12 | agent `storage_state` path arg -> disk write/read | landlock (mut) write set@mcp/tests/confine.rs | SEC.DID.11 |
| P13 | page script injection (XSS-style) via CSP | script-src enforcement (mut)@engine/page/tests/g_csp.rs (other CSP directives unimplemented per csp.rs header) | SEC.DID.12 |

## Not yet probed (no claim made)
mixed content / https downgrade, `javascript:` and `data:` top-level navigation, `window.open` features, WebRTC,
service-worker scope, extension/IPC surfaces of the shell. SEC.DID.12 covers the probe.

## JIT layer (JIT.BASE, owner decision 2026-10-29)

Agent mode runs SpiderMonkey with the Baseline interpreter and Baseline compiler ON; Ion/Warp, native regexp
and both wasm JIT tiers OFF. `--jitless` turns every tier off. Residual risk: the baseline compiler still emits
machine code, so a baseline-tier bug is reachable; the optimizing tier, where the recent SpiderMonkey JIT CVEs
sit, is not. Landlock (no exec, read-only fs) bounds what a compromise could do. Measured on a hot numeric loop:
full 30 ms, baseline 394 ms, jitless 3620 ms (9.2x), chrome_ver=145.0.7632.116.

## Mutation results (SEC.DID.1, chrome_ver=145.0.7632.116)

Each row: delete or invert one layer in the working tree, run its named gate, restore with `git checkout --`.
Baseline: all gates green before mutation (g_private_egress 4/4, g_page_fetch_preflight 1/1, g_cookie_attributes 1/1,
g_framing 1/1, g_csp 1/1, confine 3/3, isolation 6/6, session 8/8). Result is RED only when the test binary built
and a test panicked; a compile error is not counted.

| layer | mutation | gate result |
|-------|----------|-------------|
| GuardResolver post-DNS | `egress.rs` `if !guarded \|\| !guard_active()` -> `if true` | RED (public_page_cannot_reach_private_network) |
| literal-host check | `check_url_literal` `if is_private_ip(ip)` -> `if false` | RED (public_page_cannot_reach_private_network) |
| WS / SOCKS resolve_for_dial | filter -> `\|_a\| true` | RED (WebSocket and SOCKS tests) |
| CORS preflight | `page/src/lib.rs` `if !allowed {` -> `if false {` | RED (headless_executor_applies_egress_guard_and_preflight) |
| HttpOnly filter | `net/src/lib.rs` `\|c\| !c.http_only` -> `\|_c\| true` | RED (cookie_flags_hold_across_the_js_and_wire_boundaries) |
| X-Frame-Options / frame-ancestors | `if !framing_allowed(` -> `if false {` | RED (a_site_that_refuses_framing_is_not_framed) |
| CSP script-src (URL) | `allows_script_url` returns true | RED (csp_script_src_blocks_what_it_forbids...) |
| CSP script-src (inline) | `allows_inline_script` returns true | RED (same test) |
| hidden-text shield | `session.rs` `if !self.include_hidden {` -> `if false {` (2 sites) | RED (injection_shield_withholds_invisible_text) |
| landlock confinement | `confine.rs` restrict_self replaced by `0` | RED (confined_process_is_read_only...) |
| per-origin human-cookie grant | `isolation.rs` grant loop -> `.take(0)` | RED (human_profile_cookie_is_invisible_until_a_per_origin_grant) |
| ephemeral jar | `isolation.rs` remove `manuk_net::set_ephemeral(true)` | RED under `ephemeral_session_does_not_load_the_human_jar` (read side, added by SEC.DID.1.a; the old on-disk check `ephemeral_session_leaves_nothing_on_disk` stays GREEN under this mutant, as it did before) |

Gate headers record each mutation. The ephemeral jar row is now pinned by SEC.DID.1.a (read-side gate, mutant RED, restored GREEN). No layer in the table is UNPINNED for this node.

Re-invocation 2026-11-01 (SEC.DID.1, chrome_ver=145.0.7632.116): baseline green for g_private_egress 4/4, g_cookie_attributes 1/1, g_csp 1/1, g_framing 1/1, g_page_fetch_preflight 1/1 (page gates built with `--features stylo,spidermonkey`, as verify.sh does), confine 3/3, session 8/8, isolation 6/6. Mutants re-run: HttpOnly filter -> RED (g_cookie_attributes:181), framing -> RED (g_framing:162); engine sources restored, `git diff` empty for them. The other mutations are the prior run's record and were not repeated.
