# Security

## Reporting a vulnerability

Do not open a public issue. Use GitHub's private vulnerability reporting for this repository (Security tab,
"Report a vulnerability"). Whether that channel is switched on is an owner setting that this file cannot verify;
if the button is missing, open an issue that says only "security contact requested" and contain no details.
No response-time commitment is made.

## Supported versions

Pre-release. Only `main` is supported. No release is tagged for this launch.

## What the agent tool defends, and where the evidence is

The per-path census, with the test file that pins each layer, is
[`docs/sprint/SECURITY.md`](docs/sprint/SECURITY.md); tool-by-backend behaviour and grant flags are in
[`docs/sprint/BACKENDS.md`](docs/sprint/BACKENDS.md). In summary, for `manuk-mcp`:

- Origin grants (`--read-origins`, `--write-origins`, `--confirm=...`); out-of-set origins are refused.
- Credential and payment fields are never typed by the agent; the tool hands off instead.
- A shield flags or withholds page text that is hidden from paint before it reaches the model.
- On the in-process engine: Landlock confinement (no exec, read-only filesystem outside the cache), an ephemeral
  cookie jar, and SpiderMonkey with the optimizing JIT off.
- Chrome backend: the pinned Chrome for Testing runs with its sandbox on (the server prints `sandbox=on` at start).
  `--unsafe-no-sandbox` exists and is off by default.

## Known residual risks (from the census; none are claimed fixed here)

- Visible injected text in a page has no filtering layer; only paint-hidden text is shielded.
- The SpiderMonkey pin is past upstream end-of-life.
- The in-process engine runs all page content in one process; the shell (GUI browser) has no Landlock and no JIT
  restriction.
- Several census rows are marked `UNPINNED` (a mechanism exists with no gate of its own).

## Supply chain

- Dependencies are policed by `cargo deny` (`deny.toml`).
- A CycloneDX 1.5 SBOM of the `manuk-mcp` dependency graph is committed at
  [`docs/sbom/manuk-mcp.cdx.json`](docs/sbom/manuk-mcp.cdx.json), regenerated with
  `cargo cyclonedx --manifest-path mcp/Cargo.toml --format json --spec-version 1.5`.
- Release binaries are built with `cargo auditable build --release -p manuk-mcp`, which embeds the dependency list
  in the binary (ELF section `.dep-v0`), readable by `cargo audit bin` and similar scanners.
- The SpiderMonkey prebuilt is fetched by `scripts/mozjs-fetch.sh`, which checks a sha256 pin and, when `gh` is
  authenticated, runs `gh attestation verify`.
