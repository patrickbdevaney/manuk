# EXTERNAL-LIVECHECK — suite v1.1 external ground truths (X01–X10)

Checked 2026-10-25 by node R0.5.a. The frozen suite files (`bench/agent-tasks/suite-v1-external.json`, sha pin
`061ee22b…`) are NOT edited. Any DRIFTED row is excluded from both arms symmetrically, per the node note.
Chrome pin: `chrome_ver=145.0.7632.116`.

Method: Cambridge and arXiv pages were fetched over HTTP (`curl`, `User-Agent: manuk-livecheck`) and the expected
answer was searched in the page text. Wolfram Alpha pages were loaded in the pinned Chrome (playwright-core,
headless) and `document.body.innerText` was read for 15–40 s.

| ID | Query / lookup | Status | Observed |
|---|---|---|---|
| X01 | Cambridge `english-spanish/ephemeral` → `efímero` | OK | `efímero` present, 2 hits, translation context |
| X02 | Cambridge `english-chinese-simplified/nostalgia` → 怀旧 | OK | `怀旧` present, 3 hits |
| X03 | Cambridge `sustainability`: 可持续性 (zh) and durabilité (fr) | OK | `可持续性` present (zh page, 3 hits); `durabilité` present (fr page, 15 hits) |
| X04 | arXiv 2303.08774 `[v3]` submission date → `27 Mar 2023` | OK | "revised 27 Mar 2023 (this version, v3)"; `Mon, 27 Mar 2023 17:46:54 UTC` |
| X05 | Wolfram: derivative of x^2 at 5.6 → 11.2 | UNVERIFIED-LIVE | result pod empty in headless Chrome (see below); local computation 2·5.6 = 11.2 |
| X06 | Wolfram: 3^71, 5 s.f. → 7.5095 | UNVERIFIED-LIVE | result pod empty; local 3^71 = 7.5095e33 |
| X07 | Wolfram: ∫₀⁵ 3e^(2x) dx → 33038 | UNVERIFIED-LIVE | result pod empty; local 1.5(e¹⁰−1) = 33038.2 |
| X08 | Wolfram: days 2024-02-12 → 2050-08-09 → 9675 | UNVERIFIED-LIVE | result pod empty; local date difference = 9675 |
| X09 | Wolfram: regular hexagon, side 7 cm → 127.3 | UNVERIFIED-LIVE | result pod empty; local 3√3/2·49 = 127.31 |
| X10 | Wolfram: Σ 1/(n³+1) converges/diverges → converge | UNVERIFIED-LIVE | a substring hit on `converge` was ONLY the echoed query text (`convergence`), not a result — not counted as OK |

## Why the Wolfram rows are not OK or DRIFTED

- Over plain HTTP, `wolframalpha.com/input?i=…` returns a ~50 KB JS shell with no answer text (X05–X10 all).
- In the pinned headless Chrome, the page renders its chrome and the echoed query, but `Input interpretation:`,
  `Result:` and `Derivative:` have no value. The answer pods never populate in this environment.
- A first pass flagged X05–X09 as DRIFTED. That was a misreading: the empty pods mean the answer is unobserved,
  not that it changed. The verdict was corrected here, and the DRIFTED label is not used for any row.
- Consequence: the live check cannot confirm X05–X10 on the real site from this box. The local computations agree
  with the frozen answers, so the frozen answers are correct as mathematics. Live reachability of the answer
  itself is still unproven. Follow-up: R0.5.a.a.

## Cross-check of the Wolfram ground truths

| ID | Frozen answer | Local value |
|---|---|---|
| X05 | 11.2 | 11.2 |
| X06 | 7.5095 | 7.5095e33 |
| X07 | 33038 | 33038.2 |
| X08 | 9675 | 9675 |
| X09 | 127.3 | 127.31 |
| X10 | converge | Σ1/(n³+1) converges (comparison with 1/n³) |

Result: no answer was DRIFTED. Six rows remain unobserved on the live site.
