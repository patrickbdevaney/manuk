# RG.2.E1.a.a: the Beds & Baths menu no longer unmounts at HEAD; zilloft-2 0/3 is agent behaviour

chrome_ver=145.0.7632.116, judge openai/gpt-oss-120b, target/release/manuk-mcp at HEAD (5b020bca).

Repro driver docs/sprint/rg/S1aaaaa-replay.py (navigate, type San Francisco, Start search, click Beds & Baths):
- immediate click: 3 of 3 runs, menu opens (+19 ~0 -561, "Number of Bedrooms" buttons present)
- 8 s wait before the click (the earlier 3-of-4 failing variant): 4 of 4 runs, menu opens
- zilloft-2 real task x3 (docs/sprint/rg/E1aa-real-manuk.tsv): all three runs applied the filter; the page showed 147 results and a "3+ bd, 2+ ba" button. Pass 0/3.

So the open-then-unmount flake from RG.2.E1.a does not reproduce, and no engine defect was found to fix; a fixture gate that is RED before
cannot be built honestly. The 0/3 matches S1aaaa-zilloft-truth.md: all runs typed "San Francisco, CA" (token `ca` matches all 522 listings, 147 exact 3bd/2ba)
while the task expects the 16 from the query "San Francisco"; the answers also hedged about the consent-overlay and location. That is agent query choice + answer hedging.
