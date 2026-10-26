# The JS runtime is built while the document request is in flight

> Landed PERF.1.4 (partial: see docs/sprint/PERF.md "PERF.1.4 results").

## Mechanism

- `manuk_js::prewarm()` (re-exported as `manuk_page::prewarm_js`) creates the thread's SpiderMonkey `Runtime` and evaluates a trivial
  script in a throwaway global, so mozjs registers its lazy thread-locals. `AgentBrowser::load_url` runs it in a `join` after `fetch_html`
  has been polled, i.e. with the request already out.
- It does NOT arm the teardown guard. The guard must register after every thread-local the page's scripts create, else exit aborts in
  `__call_tls_dtors`. `prewarm_settle()` arms it; `load_url` calls it from a drop guard so every path (fetch error included) gets it.
- One JS thread per process: only call `prewarm` on the thread that will run the page's scripts. The shell's background prefetch thread
  must not.
- `MANUK_NO_PREWARM=1` disables it (A/B arm). `engine/page/tests/g_prewarm_exit.rs` goes red if settle is missing.

## Limits

example.com script phase 51 -> 41.5 ms. The per-document global (`install` ~10 ms, event-loop preludes ~14 ms, 8 ms of it the 487 KB
prelude parse) is still on the critical path.

## One JS thread per process (PERF.1.4.c)

`prewarm()` skips when another thread already owns the engine (`engine_owned_elsewhere`) or it is shut down. Without the guard,
the multi-threaded `cargo test -p manuk-agent --lib` harness built a second live runtime on a second test thread: "outstanding
JS engine handles" abort then SIGSEGV (128 tests, 3/3 runs red at 29de29c2, 3/3 green after).
