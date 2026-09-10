# A meta refresh is a redirect

*Tick 1486. The page that says "you are being redirected" and never was.*

## The gap

This engine handled exactly one `http-equiv` — `Content-Security-Policy` — and no other. A
declarative refresh is the oldest redirect on the web and it is still how corporate portals, payment
gateways, domain moves and half of enterprise software land the user somewhere else. On this engine
those pages rendered their stub and stopped, and the stub is usually an empty `<body>`.

**Three of the representative 200-site CrUX trend corpus are such a stub AS THEIR HOMEPAGE**:

```text
  secure.paymentech.com        222 bytes   <meta http-equiv="Refresh" content="0;URL=http://www.chasepaymentech.com/">
  www.datacareservices.com     104 bytes   <meta HTTP-EQUIV="REFRESH" CONTENT="0; URL=http://www.datacare.com">
  linxonline.co.pierce.wa.us   768 bytes
```

## The grammar, arbitrated against headless Chrome — one variant per page

```text
  http-equiv   content                     Chrome         why it is in the gate
  refresh      0;url=dest.html             navigates      the ordinary spelling
  refresh      0; URL=dest.html            navigates      `url=` is case-insensitive
  Refresh      0;dest.html                 navigates      `url=` is OPTIONAL
  REFRESH      0;url='dest.html'           navigates      quotes are stripped
  refresh      "  0 ;  url  =  x  "        navigates      whitespace everywhere
  refresh      dest.html                   DOES NOT       the TIME is required
  refresh      0                           reloads self   no url = this document
  refresh      1;url=dest.html             navigates      a delay is still a refresh
  not-refresh  0;url=dest.html             DOES NOT       only `refresh` counts
```

⚠ **The sixth row is the one a hand-written parser gets wrong.** `content="dest.html"` is the
obvious spelling and Chrome refuses it: the value must begin with a time. A parser that split on `;`
and took the last field would navigate here, and would drag a page off itself for any other use of
the `refresh` name.

## The host performs it, not the page

`Page::meta_refresh()` reports `(seconds, absolute url)`; it does not navigate. Same shape as
`take_scroll_requests` and `take_form_submits` — a `Page` does not own the tab it is displayed in, and
a navigation that bypassed the host would leave the omnibox, the back stack and the agent's own idea
of *where am I* all describing a document that is gone.

## ⚠ A refresh is the easiest infinite loop on the web

Nothing about it looks like one: no script runs, nothing throws, the page simply loads again. Three
guards, each answering a different loop:

* **the hop bound** — A → B → A → B …, reset only by a navigation the *user* initiated (so
  `goto_no_history`, which is what the refresh path itself uses, deliberately does not reset it — a
  counter a loop can reset is not a bound);
* **the self-target refusal** — `content="0"` means *reload this document*, which at zero delay is a
  spin with no exit;
* **the delay gate** — see the residue.

⚠ **RESIDUE: only a sub-second refresh is followed.** `content="5;url=…"` is a page asking to be
*read* first, and honouring it instantly would yank the document out from under the user — strictly
worse than not following it. Doing it properly needs a timer the shell does not yet expose to a
page-owned deadline. Every redirect stub in the corpus is `0` or `1`.

⚠ **The stub is not painted.** The refresh is followed *before* `rerender()` and before the deferred
scripts, so the user does not get a flash of an empty page they never asked to see.

## And the instrument was naming a cause it had never checked

All three sites were filed by `manuk-wpt fidelity` as `probe-blocked`, whose doc comment asserted
*"a page-supplied CSP, in every case observed so far."* Tested against all eight sites carrying that
tag on the corpus: **zero have a CSP**, in a `<meta>` or in a header. What three of them have is a
`<meta refresh>` — Chrome follows it and the injected probe goes with the document it left.

The reason is now stated as an **observation** rather than a mechanism. *A refusal must cite the
failing message* (t1222); *a comment is a checkable claim that dies silently* (t1303). This one had
been wrong for as long as it had been written down, and it steered every reader of the ranked backlog
toward a CSP problem that does not exist.

⚠ **The engine follows the refresh; the instrument does not.** `manuk-wpt` fetches the document with
`curl -sL`, which follows HTTP redirects and not `<meta>` ones, then probes the stub. Following it
there converts three sites; the other five need their mechanism *identified* rather than guessed at,
which is the whole point of the corrected note.

## The gate

`g_a_meta_refresh_is_a_redirect` — eleven rows, the Chrome grammar plus "the first meta wins", with
the ordinary `0;url=…` spelling as the vacuity arm: every *"does not navigate"* row is satisfied by a
parser that returns `None` for everything. Red under five mutations.
