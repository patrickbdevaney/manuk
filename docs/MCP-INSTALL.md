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

Or point `MANUK_CHROME` at any Chrome binary. On Ubuntu 24.04 the host may block the user namespaces Chrome's sandbox
needs; see the next section before trying anything else.

## Ubuntu 24.04 (and 23.10+): user namespaces for Chrome's sandbox

Symptom: manuk-mcp fails to start its Chrome task, and the Chrome log shows `No usable sandbox!` (Chrome exits at
startup), or manuk-mcp reports that Chrome did not publish DevToolsActivePort. Cause: Ubuntu sets
`kernel.apparmor_restrict_unprivileged_userns=1`, which blocks the unprivileged user namespaces Chrome's sandbox
uses. Check it with `sysctl kernel.apparmor_restrict_unprivileged_userns` (`1` = blocked, `0` = allowed).

Chrome for Testing 145 has no setuid-sandbox fallback, so the fix is on the host, not in Chrome. Do not run Chrome with
`--no-sandbox`. Both fixes below need root.

**Option 1 (narrow, recommended): an AppArmor profile for the installed Chrome binary only.** This is the same kind of
profile Google's `.deb` ships in `/etc/apparmor.d`. It grants `userns` to the pinned Chrome for Testing path and to
nothing else. Replace `YOUR_USER` with your login name (AppArmor does not expand `$HOME`), then save as
`/etc/apparmor.d/manuk-chrome-cft`:

```
abi <abi/4.0>,
include <tunables/global>

profile manuk-chrome-cft /home/YOUR_USER/.local/chrome-cft/*/chrome-linux64/chrome{,.real} flags=(unconfined) {
  userns,
}
```

Load it, then re-run the smoke:

```bash
sudo apparmor_parser -r /etc/apparmor.d/manuk-chrome-cft
python3 -I scripts/mcp-smoke.py ~/.local/bin/manuk-mcp
```

**Option 2 (blunt): relax the restriction for every program on the machine.** This sets
`kernel.apparmor_restrict_unprivileged_userns=0` system-wide, so any program may create unprivileged user namespaces.
That is what the CI verify VM does. Apply it for the session with `sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0`,
or persist it with a file in `/etc/sysctl.d/` that contains `kernel.apparmor_restrict_unprivileged_userns=0`.

## Verify an install

`python3 -I scripts/mcp-smoke.py ~/.local/bin/manuk-mcp` lists the tools and completes one Chrome task (navigate, find,
click, observe the page change). `tests/sprint/test_ship2.sh` runs package, checksum, install and smoke end to end, and is red on a
tampered tarball, a junk SBOM, or a missing Chrome.

Status: the `verify-linux` job (clean `ubuntu:24.04` container, `claude mcp add`, smoke) passed in dispatch run
45718101927 (SHIP.2.a). Its CI VM uses Option 2 above.
