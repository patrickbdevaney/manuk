# The system path is blocked by cost, not correctness

*Tick 1495. Three ticks said "not established". One diff and one failure line established it.*

## The open question

t1494 shipped `FontKey { weight: u16 }` and CSS Fonts §5.2 over the **declared** `@font-face` weight,
and deliberately left the SYSTEM path coarse: handing `fontdb::Query` the real weight instead of
`BOLD`/`NORMAL` took `css/css-fonts/variations` from **247 → 148**, twice each. The tick said so
plainly and named the cause as **not established**, with a hypothesis — that the suite's own faces
might be failing to load, making it score fallback behaviour.

## The hypothesis was wrong, and one probe said so

```text
  Page::webfonts() on at-font-face-font-matching.html  ->  (6, 6)
```

All six faces register and load. The suite is exercising real webfonts.

## And a per-file diff put the whole regression in one file

```text
  css/css-fonts/variations, per file, coarse query -> exact query
    at-font-face-font-matching.html    107/107  ->  8/107
    every other file                   unchanged
```

−99 of the −99, in one place.

## The failure line is the answer, and it is not a failure

```text
  FILE at-font-face-font-matching.html  (8/107)
    Timeout  Matching font-weight: '400' should prefer '501 550' over '502 560'
    NotRun   Matching font-weight: '430' should prefer '420 440' over '450 460'
```

**`Timeout`, then `NotRun`.** No assertion fails. The file runs out of time on its first case and
everything after it never runs.

**So the system-path change is a COST problem, not a correctness one** — which is a completely
different thing to fix, and three ticks of reading could not have told them apart.

## The mechanism

`FontContext` caches faces by `FontKey`. With `bold: bool` every weight on a page collapses to **two**
keys, so a family is looked up at most twice and `fontdb::query` — a scan over every installed face
with §5.2's matching on each — runs twice. With a numeric weight, **each distinct CSS weight is its
own key**, and this file deliberately uses dozens (400, 430, 501, 550, 502, 560, …) precisely because
it is testing weight matching. Distinct weights multiply full font-database scans.

⚠ A coarse retry when the exact query misses was tried and changed nothing (still 148), which is
consistent: the exact query **matches**, it is just called far more often.

## What this decides

The numeric key is not wrong on the system path; it is **unindexed**. The next attempt is a cost fix,
not a matching fix:

1. **Index the database by family once**, so a weight lookup is a scan over one family's faces rather
   than over every installed face. §5.2's comparator — which `weight_is_closer` already implements for
   the `@font-face` branch — then runs over a handful of candidates.
2. **Re-measure `at-font-face-font-matching.html` alone.** It is the whole of the regression, so it is
   also the whole of the verification, and it runs in seconds.
3. Only then re-attempt the system query, with that file as the gate.

⚠ And the thing that made this findable was `--show-failures` printing `Timeout` rather than a
failed assertion. *A refusal that names what it observed beats one that names a cause* — the same
lesson `ProbeBlocked` taught at t1486, one instrument over.
