# manuk v1.3.0: the Chrome MCP server

**v1.3.0 is the first release that ships `manuk-mcp` as an installable artifact.** It is an MCP server for agents. By
default it drives a pinned Chrome for Testing (145.0.7632.116) with Chrome's sandbox on, and `--backend engine` opts in
to manuk's in-process Rust engine. Results, verdicts and known limits:
[`docs/RESULTS-AND-LIMITS.md`](RESULTS-AND-LIMITS.md).

## What's new since v1.2.0

v1.2.0 (2026-09-11) was an engine release. v1.3.0 adds the installable `manuk-mcp` server and lands the capability
gains below. Every number is from a published gate under `docs/sprint/`; comparative figures state their conditions,
and the axes where manuk is still behind are kept in [`docs/RESULTS-AND-LIMITS.md`](RESULTS-AND-LIMITS.md).

- **Parallel capacity.** In an 8 GB memory cage the in-process engine holds 59–70 concurrent agent sessions at
  success 1.00, where the Chrome-backed arms top out at 21–37 and OOM at N≥25–50 — an engine capacity of 2.38×+ over
  the best Chrome arm (`pw-shared`). At N=50 each successful agent costs 9.8–10.4 MB versus 25.6 MB for `pw-shared`
  (59–62% lower; Chrome measured in a 24 GB cage, deviation stated) at roughly 6–8× less CPU, sustaining about 100 ok
  traces/min against 45 at N=1. (`docs/sprint/conc5`, `tests/sprint/test_conc5.sh`.)
- **Lower per-call CPU, identical output.** `act` on a 1500-row page now builds one accessibility tree instead of
  seven (0.91 → 0.24 cpu-s on a quiet host); the first `observe` after a navigate on an unchanged page no longer
  re-cascades or re-lays-out; `navigate` on a large CSS page runs 4 cascades/4 layouts instead of 7/6 (−40% cpu-s) and
  an image-heavy page decodes 37% cheaper — each with byte-identical observe output and its own gate.
  (`docs/sprint/cpu2`, `cpu2a`, `cpu4a`, `cpu4b`.)
- **An honest axis register.** `docs/sprint/AXES.md` is generated from the published task TSVs: 18 axes comparing
  manuk's best arm to the best Chrome arm, with every win *and* every loss tied to a node id — including the axes where
  manuk still loses (single-call CPU and the 102-task mean pass rate). (`tests/sprint/test_axes_register.sh`.)
- **Model-neutral drivers.** The same task set runs over the in-process engine under Haiku, Sonnet and Opus
  (`claude -p`), Gemini (`agy`) and free OpenRouter models, with a missing key failing closed rather than silently
  skipping. (`docs/sprint/conc3`.)
- **Cross-platform, attestable artifacts.** Linux x86_64 and macOS arm64 builds are produced by a dispatch workflow and
  verified in a fresh `ubuntu:24.04` container (checksum, install, `claude mcp add`, 26 tools listed, one Chrome task
  with the sandbox on); the server also builds on Windows. Binaries are built with `cargo auditable` and ship a
  CycloneDX SBOM and a checksum-verifying installer. (`.github/workflows/mcp-artifact.yml` run 45718101927,
  `tests/sprint/test_ship2.sh`.)

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
