# `Content-Encoding` — the one decision between the wire and the parser

> Landed t1383 (surface audit #80). Gate:
> `content_encoding_decodes_what_it_claims_and_refuses_what_it_cannot` (`engine/net/src/lib.rs`).

## Why it needed a gate at all

A compressed HTTP body reaches the HTML parser through exactly one `match` on the `Content-Encoding`
header (`wrap_decoder`). Getting it wrong does not throw: it hands the parser bytes that are not
HTML, which renders as a blank or half-built page **with no error anywhere**.

**That dispatch had no gate the wall runs.** Its only test was `decodes_gzip` — `#[ignore]`d, because
it fetches httpbin. This is surface audit #79's ranked #1 in its purest form: *a rule whose only
exercise needs an input the harness cannot build.*

It can be built. `async-compression` ships the **encoders** under the same feature flags as the
decoders, so every row is produced in-process with no network and no fixture file.

## The battery

```text
                                                      asserted
  gzip / br / deflate round-trip                         yes
  None and "identity" pass through unchanged             yes
  INVALID bytes labelled `gzip` ERROR, not garbage       yes   ← the row with a consequence
  an UNKNOWN coding (zstd) passes through                NO    ← see below
```

⭐ **The invalid-gzip row is the one with a consequence.** A decoder that swallowed the error and
yielded what it had would hand a truncated document to the parser, and a truncated document looks
like a slow network rather than a bug. `stream_body_decoded`'s `?` is what makes it loud, and nothing
asserted that.

⚠ **Vacuity**: each encoder's output is asserted to DIFFER from the plain bytes, or all three
round-trip rows would pass against a decoder that does nothing.

## ⚠ The unknown-coding row, and why it is not asserted

`wrap_decoder`'s `_ =>` arm hands an unrecognised coding to the parser **as identity**.
`Content-Encoding: zstd` became **Baseline 2026** while this engine's v1 scope defers zstd, so the
arm is now reachable on the live web.

```text
  we advertise `Accept-Encoding: gzip, deflate, br`  →  a conforming server never sends zstd
                                                        NOTHING BREAKS TODAY
```

Both candidate behaviours are defensible:

- **pass through** — the Fetch Standard handles only known codings; an unknown one is not decoded;
- **fail the load** — what Chrome does for a coding it advertised (`ERR_CONTENT_DECODING_FAILED`),
  and the honest-failure axis this project holds itself to.

Deciding needs a server that emits the header. **This environment refuses to bind a listening
socket** — `OSError: [Errno 98] Address already in use` on every port tried, including fresh ones —
so headless Chrome could not be asked. Asserting either answer would bank a guess, so the row is
recorded and left for a tick that can measure it.

⚠ Note the shape of the misconfiguration that makes this non-obvious: `Content-Encoding: UTF-8` and
`Content-Encoding: none` occur in the wild on pages that otherwise work. Under "pass through" they
keep working; under "fail the load" they stop. That is why this is an arbitration and not a cleanup.

## The dependency question, for an owner

Supporting zstd is a dependency decision rather than an engine one: the C `zstd-sys` (build cost and
attack surface — the same objection that kept ffmpeg out of the media scope) versus the pure-Rust
`ruzstd`. Nothing breaks until we advertise it, so there is no schedule pressure.

## How it was proven red

- **N1** — unwrap the `gzip` arm: the round-trip row AND the invalid-gzip row both fail, which is
  what says the decode and the refusal are one decision.
- **N2** — swap the `br` and `deflate` arms: exactly those two rows fail and gzip stays green,
  because the dispatch is by NAME and a mislabelled decoder is silently wrong on precisely two
  codings.
