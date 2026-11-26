#!/usr/bin/env bash
# SHIP.3 pre-tag gate: v1.3.0 notes lead with the Chrome MCP, link RESULTS-AND-LIMITS, state the Ubuntu userns root
# step (notes + README), never suggest --no-sandbox, and quote only the README scoreboard rows.
# Usage: test_ship3_notes.sh [notes.md] [README.md]
set -u
notes=${1:-docs/RELEASE-v1.3.0.md}; readme=${2:-README.md}; fail=0
chk() { if eval "$2"; then echo "ok $1"; else echo "FAIL $1"; fail=1; fi; }
chk "notes exist" "[ -s '$notes' ]"
chk "title leads with the Chrome MCP" "head -1 '$notes' | grep -q 'Chrome MCP'"
chk "first paragraph names manuk-mcp" "sed -n 3p '$notes' | grep -q 'manuk-mcp'"
chk "links RESULTS-AND-LIMITS.md" "grep -q '](RESULTS-AND-LIMITS.md)' '$notes'"
chk "notes name the userns sysctl" "grep -q 'kernel.apparmor_restrict_unprivileged_userns' '$notes'"
chk "notes name the symptom" "grep -q 'No usable sandbox' '$notes'"
chk "README names the userns sysctl" "grep -q 'kernel.apparmor_restrict_unprivileged_userns' '$readme'"
chk "README names the symptom" "grep -q 'No usable sandbox' '$readme'"
chk "no --no-sandbox anywhere in notes" "! grep -q -- '--no-sandbox' '$notes'"
chk "no parity/parallel claims" "! grep -qiE 'parity|parallel|concurren|faster|lighter' '$notes'"
rows_notes=$(grep -E '^\| (Playwright|manuk-mcp)' "$notes")
rows_readme=$(sed -n '/scoreboard:begin/,/scoreboard:end/p' "$readme" | grep -E '^\| (Playwright|manuk-mcp)')
chk "notes table equals README scoreboard rows" "[ -n \"\$rows_notes\" ] && [ \"\$rows_notes\" = \"\$rows_readme\" ]"
exit $fail
