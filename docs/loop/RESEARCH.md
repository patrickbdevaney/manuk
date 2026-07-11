# Manuk — RESEARCH (frontier scan notes)

_Append findings here before/while implementing high-U items. Primary sources first (specs,
engine source), then reputable secondary. Dedup against this file before re-researching. See
[[LEDGER]] for the items these feed._

## Standing references

- DOM Events / `dispatchEvent` / `isTrusted` — MDN. Used for interactive-JS keystone.
- `window.open`/`opener`/`postMessage` — HTML spec §window-object; cross-window messaging is the
  OAuth-popup return path.
- Fetch/XHR — WHATWG Fetch; `fetch()` returns a Promise<Response>; our event loop already drains
  microtasks so Promise-based fetch is feasible once a native fetch binding lands.
- MutationObserver — DOM spec §mutation-observers; SPAs rely on it to react to their own DOM
  writes.

## Notes by item

_(seeded empty; the loop appends here per tick)_
