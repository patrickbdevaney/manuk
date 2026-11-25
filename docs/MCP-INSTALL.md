# Installing manuk-mcp

Release artifacts are built by the manual-dispatch workflow `.github/workflows/mcp-artifact.yml` (Linux x86_64 and
macOS arm64). Nothing is tagged or published by it; the owner approves the tag. Each artifact `manuk-mcp-<os>-<arch>`
comes as three files: `.tar.gz`, `.tar.gz.sha256`, and `.cdx.json` (CycloneDX 1.5 SBOM of the manuk-mcp dependency
graph). The binary is built with `cargo auditable`, so the dependency list is also embedded in it.

## One line

With the three files downloaded to a directory (or served at a URL) `$BASE`:

```bash
MANUK_BASE_URL=$BASE MANUK_INSTALL_CHROME=1 sh scripts/install-mcp.sh && claude mcp add manuk -- ~/.local/bin/manuk-mcp
```

The installer verifies the sha256 **before** unpacking and refuses on mismatch (nothing is installed). Manually:
`sha256sum -c manuk-mcp-linux-x86_64.tar.gz.sha256` (macOS: `shasum -a 256 -c`).

## The pinned Chrome (required)

The default backend drives the pinned Chrome for Testing build named in `CHROME_PIN` (inside the tarball; also
`docs/sprint/CHROME_PIN`), currently `145.0.7632.116`, with Chrome's sandbox on. `MANUK_INSTALL_CHROME=1` downloads it; by hand:

```bash
pin=$(cat CHROME_PIN)   # linux64 on Linux; mac-arm64 on Apple silicon
mkdir -p ~/.local/chrome-cft/$pin
curl -fsSL https://storage.googleapis.com/chrome-for-testing-public/$pin/linux64/chrome-linux64.zip -o cft.zip
unzip -q cft.zip -d ~/.local/chrome-cft/$pin
```

Or point `MANUK_CHROME` at any Chrome binary. On Ubuntu 24.04 the host restricts unprivileged user namespaces, which
Chrome's sandbox needs; lift the host restriction rather than Chrome's flag.

## Verify an install

`python3 -I scripts/mcp-smoke.py ~/.local/bin/manuk-mcp` lists the tools and completes one Chrome task (navigate, find,
click, observe the page change). `tests/sprint/test_ship2.sh` runs package, checksum, install and smoke end to end, and is red on a
tampered tarball, a junk SBOM, or a missing Chrome.

Status: the `verify-linux` job (clean `ubuntu:24.04` container, `claude mcp add`, smoke) has not yet been run from a dispatch.
Dispatching it is a network action for the owner or the follow-up node SHIP.2.a.
