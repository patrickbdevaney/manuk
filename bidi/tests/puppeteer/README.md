# E4 acceptance — Puppeteer against Manuk's BiDi remote end

IMPLEMENTATION.md's stated behavioral acceptance for E4 is: **"Puppeteer 23+ connects
and navigates a page."** This directory holds the script that checks it, because a
Rust test cannot: it needs a Node runtime and the real `puppeteer-core` client.

## Run it

```sh
cargo run -p manuk-bidi --bin manuk-bidi -- --port 9222 &
npm install puppeteer-core@23
node bidi/tests/puppeteer/drive.mjs ws://127.0.0.1:9222
```

## Result on 2026-07-10 (puppeteer-core 23.11.1, Node v22.18.0, Linux x86_64)

```
CONNECTED
NAVIGATED data:text/html,<title>PptrOK</
SCREENSHOT bytes=3497
TITLE_ERROR: Protocol error (script.callFunction): unsupported operation script.* requires
             the JS engine (build with --features spidermonkey); not wired into the BiDi
             remote end yet
DONE
```

So: **connect, `newPage`, `goto`, and `screenshot` all work**. `page.title()` does not,
because Puppeteer implements it via `script.callFunction`, which we deliberately return
`unsupported operation` for rather than faking — it needs the JS engine wired into the
remote end. That is the documented E4 residual.

## What driving a real client taught us (all fixed)

1. `browser.getUserContexts` is required before any context work.
2. `browsingContext.contextCreated` must be emitted **before** the
   `browsingContext.create` reply. Clients build their context map from the event and
   look the id up the instant the command resolves; emitting after loses the race and
   the client reports *"failing to create a browsing context correctly"*.
3. Context info must be **complete** (`parent`, `userContext`): clients filter on
   `parent == null` (top-level only) and on `userContext`, silently dropping events
   that omit them — which looks identical to "the context was never created".
4. `browsingContext.setViewport` is issued by `newPage()`.
5. `goto` needs the full navigation lifecycle: `navigationStarted` **before** the reply
   (it creates the client's Navigation object), then `domContentLoaded` and `load`
   **carrying the same `navigation` id** and a numeric `timestamp`. Without the id, the
   client ignores the events and `goto` never resolves.
