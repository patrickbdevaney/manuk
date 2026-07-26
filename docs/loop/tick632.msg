measure(cert): the load budget moves COVERAGE and SHAPE in OPPOSITE directions

TICK SHAPE: measurement. Bar 0 untouched; no ratchet floor moved; NO ENGINE SOURCE CHANGED — and
deliberately so: changing the budget would move both numbers and settle nothing.

t626 ranked placement as the largest remaining gap and named agoda (shape 0.0%) as the cheapest thread.
IT WAS THE WRONG THREAD AND I CHECKED BEFORE PULLING IT: agoda's n=13 is barely above the vacuity floor,
and its dx=8 dy=8 dw=16 signature looked like a body margin — so I tested that directly with a
`*{margin:0}` reset and got bodyX=0 bodyW=800 bodyMargin=0px, matching Chrome exactly. t575's
cascade-origin fix holds; the hypothesis was wrong and cost two minutes instead of a tick.

The real signal was keirin.jp: shape 10.8% over n=556 — a large sample with a very low score.

AND THE EYEBALL DIAGNOSED IT IN ONE LOOK. Our render is COMPLETELY UNSTYLED — default bullets, default
form widgets, no layout — and the page is displaying keirin.jp's own message:
  現在お使いの環境では当ページを正常に表示することができません
  ("This page cannot be displayed correctly in your current environment.")

I RULED OUT MY OWN RECENT CHANGE FIRST. t616 made a non-2xx subresource un-executable, so a broken CSS
fetch would have been mine. All six stylesheets answer 200 with real bytes. Not a regression.

THE CAUSE IS THE LOAD BUDGET, and the A/B is the tick:

                       default (12s)        MANUK_LOAD_BUDGET_MS=60000
  load                 17.8s                27.9s          (chromium 6.0s)
  SHAPE                10.4%                34.3%          <- 3.3x better
  median dx / dw       202 / 175            0 / 0          <- the CSS applied
  COVERAGE             83.3%                58.0%          <- 103 missing -> 277 missing

MORE TIME BUYS PLACEMENT AND COSTS COVERAGE. Shape more than triples and the median x/width offsets
collapse to ZERO — the stylesheets did apply, given time. And coverage falls 25 points, because more of
the page's own JavaScript runs and the document ends up with FEWER boxes (on this site, plausibly its
unsupported-environment path hiding content — that message is in the served HTML).

THE CONSEQUENCE FOR THE CERTIFICATE IS THE FINDING. t602 promoted performance to "a fidelity input" on
the evidence that a page painted incomplete scores as a layout failure. This is that, made precise and
given its opposite half: THE TWO HEADLINE NUMBERS MOVE IN OPPOSITE DIRECTIONS WITH THE BUDGET, so it
cannot be tuned to improve the certificate — only to choose which half to flatter. A 12s budget
under-reports placement; a 60s budget under-reports presence. Neither is the honest setting, and there
is no setting that is.

What the number actually says is that WE TAKE 17-28s WHERE CHROMIUM TAKES 6. That is the thing to fix,
and until it is, MEAN SHAPE 40.0% should be read as a LOWER BOUND CONTAMINATED BY OUR OWN LATENCY, not
as a pure layout measurement.

Gates: none — this tick measures a parameter's effect; the parameter already has a knob
(MANUK_LOAD_BUDGET_MS) and both settings are honest for different questions.
WIKI: docs/wiki/conformance-and-oracles.md — "the load budget trades coverage against shape".
PATTERN: [no-pattern] — no browser capability changed.

NEXT: our own latency is now the named blocker on the placement half — 17-28s against Chromium's 6s on
keirin.jp, and OURS IS SLOW fired on 10 of 14 sites at t606. That is a performance arc, and t602 already
ranked it as a fidelity input rather than a comfort metric. Other open thread unchanged: t629's part (1),
getBoundingClientRect() on an SVG child.
