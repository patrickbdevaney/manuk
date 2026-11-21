# W6.R1.b.a — why manuk's opendining target count flips between 10 and 0

chrome_ver=145.0.7632.116 (Chrome side stayed 109 targets in 8 of 8 captures; this node is manuk-side only).

## What was measured

Instrument: `agent/src/bin/agf-probe.rs` now also prints, on stderr, `DIAG script_error_count`, the first 20 `DIAG script_error`
lines and `DIAG dom` (element count, `body.innerText` length). Network: `MANUK_NET_TRACE=<file>`. Raw: `docs/sprint/w6/raw/R1ba/`
(`out*.tsv` targets, `err*.txt` diagnostics, `net*.tsv` request traces). HEAD = 9315ef0c.

## Comparison: 0-target capture vs rendered ("10-target"-class) state

| | 0-target capture (agf-probe, 16 of 16 runs at HEAD) | rendered state (dom-eval, 60 s page time, HEAD) |
|---|---|---|
| targets | 0 | the page's links/buttons present (buttons=3, links=3) |
| console / script errors | 0 (`script_error_count 0` in 16/16) | none seen (dom-eval does not print them) |
| failed requests | 0: 25 requests, all HTTP 200, all bodies read, identical set in all 16 traces | same page, same requests |
| DOM root size | 85 elements, `body.innerText` 0 chars, `.MuiBox-root` empty (hydration ran: NEXT-ROUTE-ANNOUNCER present) | 259 elements, text 1381 chars, `.MuiBox-root` innerHTML 22171 chars |

The historical 10-target captures (`raw/R1/opendining_r1,r3.json`, `raw/R1b/opendining_r1,r3.json`) list `close, Home, Select time,
Select number of guests, ...`: the rendered page. They predate the diagnostics (no console or request data was kept), and the
engine of that moment cannot be rebuilt in this tree (the old Cargo graph fails against the current SpiderMonkey bindings), so a
same-binary 10-vs-0 pair does not exist. The comparison above is therefore 0-target runs vs the rendered state reached on the same
HEAD binary by giving the page more page time.

## The difference

The 0 and the rendered state differ in exactly one thing found: the DOM root. Errors: none. Requests: identical. The client
render is gated on page time: `dom-eval` wait sweep at HEAD (settles of 1.8 s virtual each):

| wait (s of page time) | 6 | 12 | 30 | 40 | 50 | 60 | 70 |
|---|---|---|---|---|---|---|---|
| buttons / links / text chars | 0/0/0 | 0/0/0 | 0/0/0 | 0/0/0 | 0/0/0 | 3/3/1381 | 2/3/971 |

So the page's content appears between 50.4 s and 61.2 s of page time and not before. Before W6.R1.a.a.a (30525dcf, 22:13), the
post-load virtual clock advanced by a count that depended on host speed (that commit's own note: ~18x real time, by a count that
varied); on a busy host a capture could cross this 50-60 s horizon (10 targets), on a faster one not (0). HEAD now gives 0 in
16 of 16 runs, with the clock capped at 6 s on ordinary pages. That is the classification: **client-render timer gate crossed or
not by a host-speed-dependent virtual clock**, not a failed request or JS error.

Caveat (honest limit): the link between the historical 10s and the old clock is inferred from the sweep plus the clock-change
note; no old-binary capture exists to confirm. Which timer (or other page-time-gated step) holds the render was not identified.

## Falsifier

"no difference between the 0 and 10 runs -> unexplained nondeterminism": a difference exists (85 vs 259 elements, 0 vs 1381 text
chars, same requests, no errors), so the falsifier does not fire for the DOM-size claim. For the *cause*, the old-clock link is
an inference, stated as such above.

## Consequence for R1

At HEAD manuk's opendining count is a stable 0 against Chrome's 109, i.e. a deterministic miss (page needs 50-60 s of page time
that Chrome's 4 s settle does not need), no longer a flip. Follow-up W6.R1.b.a.a: find what gates the render.
