# manuk v1.3.0: the Chrome MCP server

**v1.3.0 is the first release that ships `manuk-mcp` as an installable artifact.** It is an MCP server for agents. By
default it drives a pinned Chrome for Testing (145.0.7632.116) with Chrome's sandbox on, and `--backend engine` opts in
to manuk's in-process Rust engine. Results, verdicts and known limits:
[`docs/RESULTS-AND-LIMITS.md`](RESULTS-AND-LIMITS.md).

## Install

Each platform (Linux x86_64, macOS arm64) ships three release assets: `manuk-mcp-<os>-<arch>.tar.gz`, its `.sha256`
and a CycloneDX SBOM (`.cdx.json`). The binary is built with `cargo auditable`. The installer checks the sha256 before
it unpacks anything:

```bash
MANUK_BASE_URL=<release download URL> MANUK_INSTALL_CHROME=1 sh scripts/install-mcp.sh \
  && claude mcp add manuk -- ~/.local/bin/manuk-mcp
```

Full steps: [`docs/MCP-INSTALL.md`](MCP-INSTALL.md).

**Ubuntu 24.04 (and 23.10+): one root step is needed.** Ubuntu sets
`kernel.apparmor_restrict_unprivileged_userns=1`, which blocks the user namespaces Chrome's sandbox uses. For a
non-root user, Chrome then exits with `No usable sandbox`, and manuk-mcp reports that Chrome did not publish
DevToolsActivePort. `install-mcp.sh` does not change system settings. Before first use, do one of these as root (exact
text in `docs/MCP-INSTALL.md`): install a per-binary AppArmor profile for the pinned Chrome path and load it with
`apparmor_parser -r` (recommended), or set the machine-wide sysctl `kernel.apparmor_restrict_unprivileged_userns=0`.
manuk-mcp never turns Chrome's sandbox off.

## What is measured

These are the rows from `docs/sprint/SCOREBOARD.tsv` (chrome_ver=145.0.7632.116, driving model Haiku 5.5, 3 runs, pass
count per run):

| arm | own 40 tasks, runs 1/2/3 | external 10 tasks, runs 1/2/3 |
|---|---|---|
| Playwright MCP + Chrome | 39 / 40 / 38 of 40 | 8 / 7 / 7 of 10 |
| manuk-mcp `--backend chrome` | 40 / 40 / 40 of 40 | 7 / 8 / 7 of 10 |
| manuk-mcp (in-process engine) | 39 / 39 / 38 of 40 | 7 / 5 / 7 of 10 |

The bars and how each one is read are in `docs/RESULTS-AND-LIMITS.md`. These notes make no other comparative claims.

## Release verification

- The clean-container check is a dispatch run of `.github/workflows/mcp-artifact.yml` (run 45718101927). In a fresh
  ubuntu:24.04 container it verified the checksum, installed the artifact, ran `claude mcp add`, listed 26 tools and
  completed one Chrome task with the sandbox on.
- `tests/sprint/test_ship2.sh` packages the artifact, verifies it and smoke-tests the installed binary. It goes red
  on a tampered tarball, on a junk SBOM and when Chrome is missing.
