# W4.C4: action receipts (5 arms, Haiku verifier, scripted agent)

chrome_ver=145.0.7632.116; harness `bench/agent-tasks/c4_receipts.py`; data `receipts.tsv` (3 runs x 8 trials per arm).
Every `act` result now ends with `writes:` — storage keys (local/session storage, cookies; cookie values shown as size only)
that appeared (`+`), changed (`~`) or vanished (`-`), and non-safe requests the page sent (`sent POST /api/cart`), or
`writes: none`. `MANUK_NO_RECEIPTS=1` is the baseline arm.
Fixture: "Add Mug" always writes; "Add Lamp" silently fails in ~half the trials (seeded, identical across arms) while the UI
says "Added Lamp!" either way. A Haiku verifier sees only the task and the two click results and says whether the cart holds both.

| arm | verification correct | claimed "both" while lamp missing |
|---|---|---|
| manuk (receipts) | 96% (23/24) | 0/10 |
| manuk, no receipts | 42% | 0/10 |
| cdp-layer (receipts) | 83% | 1/10 |
| cdp-layer, no receipts | 38% | 3/10 |
| playwright | 42% | 0/10 |

Gate (receipts lift accuracy >= 20 pt over no-receipts): PASS, +54 pt. Mutation (verifier always YES, `MANUK_C4_MUTATE=1`): 75% vs 75%, GATE FAIL.
Unit gate: `cargo test -p manuk-mcp --test receipts` (add-to-cart reports the localStorage key and the POST; a click that wrote nothing says `writes: none`; baseline has no receipt).

## Reading (only what the numbers support)
- The receipt turns "did the click really change state?" from a coin flip into a read. The no-receipt arms lose because the act result says
  `no change` (the effect diff does not see the optimistic text node), so the verifier defaults to NO: its 38-42% is just the base rate of lamp-worked trials.
- **LAYER win, not an engine win.** cdp-layer with receipts (83%) also beats every no-receipt arm, and manuk's 96% vs 83% is 23/24 vs 20/24, inside the noise of 24 trials. No engine claim.
  Over Chrome the receipt has storage only (no network ledger: `writes_since` is empty), which is the gap; the engine ledger gives the POST.
- Not measured: the REAL x1 3-arm LLM run named in the gate (expensive; REAL's tasks are graded on end state, so the claim "state-verification failures drop" is only shown on this synthetic fixture). Follow-up W4.C4.a.
- Limits: IndexedDB writes are not in the receipt (engine has no IDB listing exposed); a sessionStorage write is only seen on manuk, not cdp-layer.
- Side finding: the effect diff reports `no change` for a click that rewrote a `<p>` text. Follow-up W4.C4.b.
