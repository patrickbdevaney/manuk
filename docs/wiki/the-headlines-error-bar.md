# The headline is an integer, and the same binary does not give the same integer twice

> Landed t1410. Gate: `the_headline_carries_its_own_run_to_run_band`
> (`tests/wpt/tests/g_certificate_band.rs`), 5 arms, red under 4 mutations.

## Found because `render-failed` would not sit still

`www.otomoto.pl`, probed three times in a row with nothing changed:

```text
  run1  SHAPE: 57.0%      run2  render-failed      run3  render-failed
```

Not a page property, not a fix — **the reason itself is intermittent.** The code already records this
shape for `css-starved` (*"a site alternates between its real shape and ~0 across runs, which reads
exactly like a layout regression and is a standing variance source in the headline"*). Written down
once, never measured.

## The measurement: same 40 sites, same binary, three times

```text
  run1   scored 26   shape>=0.75  12
  run2   scored 24   shape>=0.75  12
  run3   scored 24   shape>=0.75  13
  certificate headline over the three:  scored 17..19 · shape>=0.75 9..11 · BAND WIDTH 2 SITES
  2 sites flip scored/unscored · 1 flips the PASS
  otomoto.pl 0.000/0.000/0.630 · mobile.bg 0.432/0.443/0.106 · puentedemando 0.718/0.420/0.725
  median spread 0.002 — most sites are rock steady; the TAIL moves the count
```

> ⭐⭐⭐ **Two sites of scatter on a forty-site slice — and t1406 reported sixty-one ticks as "+1
> site".** The 200-site corpus contains five such slices. That reading is inside the noise. The
> *direction* of t1406's finding stands (Track A was dark; nothing moved the render bar), but the
> number should not have been quoted without repeats.

⚠ Both scored/unscored flips were `oracle-timeout` rows — the reference browser's variance, which
[t1409](timeout-names-whose-clock.md) taught the watchdog to name one tick earlier. Without that
attribution they read as the engine getting worse between two runs of the same binary. The two ticks
compose; neither is complete alone.

## The band belongs in the tool

```text
  manuk-wpt certificate --rows A --rows B --rows C
```

prints the band and the rule it implies: **a delta no larger than the band is not a movement.**

* `<=`, not `<` — a delta exactly equal to the observed scatter has been produced by doing nothing.
* **One run refuses nothing.** An unrepeated sweep has no evidence about its own scatter, and a band
  that dismissed small deltas on no evidence would be worse than the missing band it replaces.

## The session's fourth instrument found wrong about its own numbers

* t1405 — a live page's churn scored as engine error.
* t1407 — a stale stored row scored as this tick's work.
* t1409 — a wall-clock timeout scored as the engine's, while the engine was idle.
* t1410 — an integer printed for a quantity that has a ±2 band.

Every one was found by **repeating the measurement instead of reading its label.**
