# The proxy rewrites a HOST; a site is a set of hosts

*t1503. The instrument t1502 asked for, and the one mechanism it named on its first run.*

## The question that could not be asked

t1502 refuted three hypotheses about why `comix.to`, `house.udn.com` and `allticketscol.com` render
under our engine and produce a 4-tag reference under the one-origin proxy. It ended by naming the
reason no fourth hypothesis was worth writing: **`one_origin_reference` invokes Chrome with
`--dump-dom` and counts open tags, so a proxied app that throws on line one is indistinguishable from
one that renders four tags on purpose.**

Every remaining candidate — CORS, CSP, a service worker, a cookie flag, a `Secure` requirement — is a
different *message*, and we were reading a different *number*.

## The instrument

`--enable-logging=stderr --v=1` on the **proxied** invocation only, and `proxied_console_lines` to
separate the page's own messages from the browser's:

```rust
pub fn proxied_console_lines(stderr: &str) -> Vec<String>
```

Three properties, each of which is the whole value of one part of it:

- **`INFO:CONSOLE` is kept.** Chrome's own marker for a page-originated message. Mutation M1 —
  filtering on `ERROR:` alone, the obvious spelling — leaves *only* `ERROR:google_apis/gcm/...`,
  Chrome's GCM registration chatter. The instrument would have reported browser noise as the page's
  dying words.
- **Silence is reported as silence.** M2 returns `vec![]` for a quiet run; the caller then prints
  nothing, and "the page logged nothing" becomes indistinguishable from "we did not look". `comix.to`
  is the case that needs this: it renders four tags *while saying nothing at all*.
- **Capped at 6.** M3 removes the cap. A page in a boot loop emits thousands of identical lines, and a
  diagnostic that floods is the same failure as one that is silent — t1480's dedupe argument, reached
  from the other side.

All three PROVEN RED, clean green.

## ⭐⭐⭐ THE MECHANISM: THE PROXY REWRITES A HOST, AND A SITE IS A SET OF HOSTS

First run, `allticketscol.com`:

```text
  PROXIED CONSOLE: [...INFO:CONSOLE:359] "No hay Meta Pixel inicializado para detener."
  PROXIED CONSOLE: [...INFO:CONSOLE:0] "Access to fetch at 'https://back.allticketscol.com/api/eventos/even
  PROXIED CONSOLE: [...INFO:CONSOLE:0] "Access to fetch at 'https://back.allticketscol.com/api/eventos/ciud
```

The page is a client-rendered app whose data comes from **`back.allticketscol.com`** — a *sibling
subdomain*. `rewrite_document` rewrites references to the **document's own host**. `back.` is a
different host, so it is left alone, correctly and uselessly: from `http://127.0.0.1:PORT` those calls
are cross-origin, the API's `Access-Control-Allow-Origin` names the live origin, and every fetch is
refused. The app renders its shell and stops.

**The generalisation is the finding, not the site.** A modern site is not one host. It is an origin
for documents, another for its API, others for CDN and media. A same-host rewrite covers the first and
silently drops the rest — and the failure is invisible in a tag count, because the shell renders.

⚠ **This is the proxy's defect, not the engine's.** The row was already `oracle-module-shell`; t1502
established the engine boots these pages cleanly. t1503 says *why the oracle cannot be built* for one
of the three, which is what a refusal tag is supposed to mean.

## The two that stayed silent

| site | proxied console | what it rules out |
|---|---|---|
| `allticketscol.com` | CORS to `back.allticketscol.com` | — **named** |
| `comix.to` | Chrome-internal GCM only; page silent | it did **not** throw |
| `house.udn.com` | nothing at all | it did **not** throw |

A silent page that renders four tags is *deciding* to render four tags. That is a different class of
mechanism from a thrown exception — a routing guard, a feature detect, a bot check that returns a
shell — and the next probe for those two is the **response body**, not the console.

## The rule

> **A negative observation is only worth what its instrument could have shown.** Three hypotheses were
> refuted in t1502 by reading documents; the fourth needed the page's own voice, and one run of it
> named a mechanism no amount of further reading would have reached.

See also: [the-engine-boots-it-and-the-oracle-cannot](the-engine-boots-it-and-the-oracle-cannot.md),
[the-only-engine-owned-refusal-is-a-clock](the-only-engine-owned-refusal-is-a-clock.md).
