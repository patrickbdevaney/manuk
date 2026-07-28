instrument(sweep): the Phase-0 corpus sweep swept ZERO sites and printed a certificate

t704 owed the full corpus sweep. It does not run:

  === PHASE-0 EXIT CERTIFICATE ===
    sites 0 · scored 0 · shape >=0.75 on 0 (0.0%)
    h-overflow     clean on    0 sites (0.0%)
    CERTIFICATE NOT MET — shortfalls, in the order to work them:
        · shape >=0.75 on 0.0% of sites (bar 95%)      <- five of these
  MEAN VISUAL:    NaN%

Every column is 0.0% or NaN and not one word says the corpus was empty. Those shortfall lines are
byte-identical to what a sweep that really ran and really failed would print.

One character class. `--urls-file` reduces `category<TAB>url` with rsplit('\t'), and
docs/bench/oracle-corpus.txt is SPACE-ALIGNED:

  news         https://nytimes.com/

so every line came back whole, starts_with("http") rejected all 265, and the run swept the empty set.
corpus-v2.tsv is the tab-separated one; whichever sweep last worked must have used it.

This is the project's oldest failure shape in a new hat. t650: "100% of nothing is 100% — a metric
whose denominator comes from the thing measured is satisfied by measuring LESS, and the fix is a
SECOND POPULATION, never a stricter threshold." A count of parsed URLs cannot detect its own absence;
a count of parsed URLs NEXT TO the count of lines they came from can. So parse_corpus returns
{urls, candidates}; the caller hard-exits on urls.is_empty() && candidates > 0, and warns loudly when
urls.len() < candidates — a sweep of 12 sites reported as "the corpus" is how an optimistic number is
born.

The gate reads the REAL file rather than a literal of what I think it looks like:
the_real_corpus_file_parses_to_urls_not_to_silence opens docs/bench/oracle-corpus.txt from disk and
asserts urls.len() == candidates. A gate written against a string literal tests my belief about the
corpus; this one fails if somebody re-aligns those columns, HERE, in 27 seconds, instead of three
hours into a sweep that quietly measures nothing.

RED-proven by restoring the '\t' split: 2 of the 3 gates fail, including the real-file one.

Gates: corpus_parse_tests 3/3.

WIKI: none — the mechanism is a corpus-file parse in the test harness, not a browser behaviour;
docs/wiki describes what the engine does and this changes nothing it does. [no-pattern]
