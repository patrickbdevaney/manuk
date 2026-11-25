#!/bin/sh
# SHIP.2.a: the clean-container verify of the Linux release artifact. Runs INSIDE a fresh ubuntu:24.04 container,
# started by .github/workflows/mcp-artifact.yml (verify-linux). The container holds no Rust and no compiler.
# Inputs under /w: rel/ (the downloaded artifact), src/ (this checkout: installer, smoke, pin).
# Chrome's sandbox stays ON. Its namespace sandbox needs unprivileged user namespaces, which the runner host must allow
# (the host sets kernel.apparmor_restrict_unprivileged_userns=0 before starting this container; a container cannot).
set -eu
cd /w
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq >/dev/null
apt-get install -y -qq --no-install-recommends ca-certificates curl unzip python3 nodejs npm libstdc++6 \
  libnss3 libatk1.0-0t64 libatk-bridge2.0-0t64 libcups2t64 libdrm2 libxkbcommon0 libxcomposite1 libxdamage1 \
  libxfixes3 libxrandr2 libgbm1 libasound2t64 libpango-1.0-0 libcairo2 fonts-liberation procps >/dev/null
echo "== checksum"
(cd rel && sha256sum -c manuk-mcp-linux-x86_64.tar.gz.sha256)
echo "== install (verifies the checksum again; fetches the pinned Chrome for Testing into a shared HOME)"
mkdir -p /opt/manuk-home
HOME=/opt/manuk-home MANUK_BASE_URL=/w/rel MANUK_PREFIX=/usr/local MANUK_INSTALL_CHROME=1 sh src/scripts/install-mcp.sh
echo "== claude mcp add"
npm install -g @anthropic-ai/claude-code >/dev/null 2>&1
claude mcp add manuk -- /usr/local/bin/manuk-mcp
claude mcp list
echo "== smoke as an unprivileged user (Chrome refuses its sandbox as root)"
useradd -m smokeuser
chmod -R a+rX /opt/manuk-home
CH=$(find /opt/manuk-home/.local/chrome-cft -type f -name chrome | head -1)
test -x "$CH"
runuser -u smokeuser -- env HOME=/home/smokeuser MANUK_CHROME="$CH" \
  python3 -I src/scripts/mcp-smoke.py /usr/local/bin/manuk-mcp
echo "PASS clean-container verify: checksum, install, claude mcp add, tools listed, one Chrome task (sandbox on)"
