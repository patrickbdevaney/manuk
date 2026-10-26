# Agent mode runs SpiderMonkey jitless and the process landlocked

> Landed R1.6.

## Mechanism

- `manuk_js::hardening::enable_jitless()` is a process-wide switch (like `determinism::enable`). `install_host_hooks` (both the page path
  and bare `SpiderMonkeyRuntime`) then sets the global JIT options off: Ion, Baseline, Baseline interpreter, native regexp, both wasm JIT
  tiers. `JS::DisableJitBackend()` itself is not bound by mozjs and must precede `JS_Init`, so the option route is used.
- `manuk-mcp` calls it in `main`, and then `confine::confine` (landlock v1, raw syscalls via libc): read-only on system roots, cwd and the
  files it was handed; write only on the cache dir (`MANUK_CACHE_DIR` / `$XDG_CACHE_HOME/manuk` / `~/.cache/manuk`), `--profile`,
  `MANUK_ACTLOG_DIR` and `--allow-write <dir>`; `execve` is handled and never granted; `$HOME` is not readable. It runs before any thread
  exists, because v1 has no TSYNC and a domain is inherited only by threads created afterwards.
- `--no-confine` opts out. A kernel without landlock gets a stderr warning, not a refusal.
- Consequence: `storage_state export <path>` outside those dirs now fails; pass `--allow-write`.

## Gates

`engine/js/tests/hardening.rs` (child process per mode; `MANUK_TEST_BREAK=1` skips the switch and it goes red) and
`mcp/tests/confine.rs` (control run with `--no-confine` can write outside, exec and read `$HOME`).
