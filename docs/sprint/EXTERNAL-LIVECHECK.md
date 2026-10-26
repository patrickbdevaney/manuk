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
| X05 | Wolfram: derivative of x^2 at 5.6 → 11.2 | OK | Wolfram result-image alt text: `11.2` (input `(dx^2)/(dx) where x = 5.6`) |
| X06 | Wolfram: 3^71, 5 s.f. → 7.5095 | OK | result-image alt text: `7.5095×10^33` |
| X07 | Wolfram: ∫₀⁵ 3e^(2x) dx → 33038 | OK | result-image alt text: `integral_0^5 3 e^(2 x) dx = 3/2 (e^10 - 1)≈33038.` |
| X08 | Wolfram: days 2024-02-12 → 2050-08-09 → 9675 | OK | result-image alt text: `9675 days` |
| X09 | Wolfram: regular hexagon, side 7 cm → 127.3 | OK | result-image alt text: `(147 sqrt(3))/2 cm^2 (square centimeters)≈127.306 cm^2`; also `127.3 μa` |
| X10 | Wolfram: Σ 1/(n³+1) converges/diverges → converge | OK | result-image alt text: `sum_(n=1)^∞ 1/(n^3 + 1) converges` |

## How the Wolfram rows were observed (node R0.5.a.a)

Wolfram Alpha renders each result pod as an `<img>` whose `alt` attribute carries the answer text; `innerText` shows
only the pod titles ("Result:"), which is why R0.5.a saw empty pods. Path: pinned Chrome (`chrome_ver=145.0.7632.116`),
headless, ordinary desktop UA, `wolframalpha.com/input?i=<query>`, wait 15 s, read `img[alt]`. The query was phrased
as a short Wolfram input (not the verbatim WebVoyager goal) where the verbatim goal was misparsed; X10 needed the form
`Does the sum 1/(n^3+1) from n=1 to infinity converge`. Checked by `python3 -I bench/agent-tasks/check_livecheck.py`.

## Cross-check of the Wolfram ground truths

| ID | Frozen answer | Local value |
|---|---|---|
| X05 | 11.2 | 11.2 |
| X06 | 7.5095 | 7.5095e33 |
| X07 | 33038 | 33038.2 |
| X08 | 9675 | 9675 |
| X09 | 127.3 | 127.31 |
| X10 | converge | Σ1/(n³+1) converges (comparison with 1/n³) |

Result: no answer was DRIFTED. X01–X10 are all OK on the live sites.
