#!/usr/bin/env bash
# map-reconcile.sh — RECONCILE the capability map against reality (RELIABILITY-DOCTRINE.md Cat A, §3&4).
#
# The constellation's `status` column is HAND-SET, so it drifts from what the engine actually does — the
# root of TWO failure modes: re-implementing done work (status says `missing` but a gate is green) and
# FALSE PRESENCE (status says `works`/`gated` but nothing behaviour-exercising backs it). This check makes
# the map a PROJECTION that must reconcile: a row claiming capability MUST name a gate whose test actually
# EXISTS. It cannot re-run 300 gates (heavy/unsound — there is no per-gate result ledger), so it verifies
# the SOUND, LIGHT invariant — "the claim is backed by a real gate" — which is exactly what kills the two
# drift modes at the assertion layer. Read-only, no cargo: safe to run anytime, even mid-tick.
#
# ROLLOUT: report-only today (exit 0). Pre-existing drift must be driven to ZERO by the map's owner (the
# agent) BEFORE this is wired into verify.sh as a FAILING gate — flipping it strict now would brick every
# tick on legacy drift. `--strict` returns non-zero on any drift, for that future wiring + CI.
set -uo pipefail
cd "$(dirname "$0")/.." 2>/dev/null || exit 0
MAP=docs/loop/CONSTELLATION.tsv
STRICT=0; [ "${1:-}" = "--strict" ] && STRICT=1
[ -f "$MAP" ] || { echo "map-reconcile: $MAP not found"; exit 0; }

# Precompute the set of existing gate-test basenames (g_foo from .../tests/g_foo.rs) ONCE — the fuzzy
# match target. G_FOO in the map maps to a file whose basename == or STARTS WITH g_foo (e.g.
# G_BIDI_BASE -> g_bidi_base_direction.rs), so a prefix test over this set is the sound check.
BASENAMES=$(find engine agent -path '*/tests/*.rs' -printf '%f\n' 2>/dev/null | sed 's/\.rs$//' | sort -u)

gate_backed(){ # $1 = the WHOLE gate cell (may name several: "G_FORM, G_FORMDATA+G_X (note)").
  # 0 if AT LEAST ONE named G_ gate is backed by a real test — the sound false-presence test is "does
  # ANY real gate back this claim", so tokenize and check each; a cell fails only if NONE resolve.
  local tok lc
  for tok in $(printf '%s' "$1" | grep -oE 'G_[A-Z0-9_]+'); do
    lc=$(printf '%s' "$tok" | tr 'A-Z' 'a-z')
    printf '%s\n' "$BASENAMES" | grep -qE "^${lc}(_|$)" && return 0        # a test file g_foo*.rs
    grep -rqlF "$tok" --include='*.rs' engine agent 2>/dev/null && return 0  # or the const named in source
  done
  return 1
}

bare=0; nofile=0; ok=0; descriptive=0; missing_ok=0
BAREF=$(mktemp); NOFILEF=$(mktemp)
# columns: 1 class · 2 capability · 3 what_breaks · 4 status · 5 gate · 6 receipt
while IFS=$'\t' read -r cls cap brk status gate rest; do
  [ "$cls" = "class" ] && continue          # header
  [ -z "${cap:-}" ] && continue
  case "$status" in
    gated|works|partial)
      if [ "${gate:-}" = "-" ] || [ -z "${gate:-}" ]; then
        bare=$((bare+1)); printf '  %-10s %-34s claims "%s" with NO gate\n' "$status" "${cap:0:34}" "$status" >> "$BAREF"
      elif printf '%s' "$gate" | grep -qE 'G_[A-Z0-9_]+'; then
        if gate_backed "$gate"; then ok=$((ok+1)); else
          nofile=$((nofile+1)); printf '  %-10s %-30s names %-40s — NO test file/const found\n' "$status" "${cap:0:30}" "${gate:0:40}" >> "$NOFILEF"
        fi
      else
        descriptive=$((descriptive+1))       # F1/F2 etc — a real floor, not machine-checkable here
      fi ;;
    missing) missing_ok=$((missing_ok+1)) ;;  # correctly unbacked
  esac
done < "$MAP"

drift=$((bare+nofile))
echo "── map-reconcile: $(tail -n +2 "$MAP" | grep -c . ) rows ─────────────────────────────"
echo "  OK (claim ↔ real gate): $ok   ·   descriptive-floor: $descriptive   ·   missing (unbacked, fine): $missing_ok"
if [ "$bare" -gt 0 ]; then echo "  DRIFT · BARE ASSERTION ($bare) — status claims a capability with gate='-' (false-presence risk):"; cat "$BAREF"; fi
if [ "$nofile" -gt 0 ]; then echo "  DRIFT · DANGLING GATE ($nofile) — names a G_* gate with no backing test (unverifiable claim):"; cat "$NOFILEF"; fi
rm -f "$BAREF" "$NOFILEF"
echo "  ───────────────────────────────────────────────"
if [ "$drift" -eq 0 ]; then echo "  ✓ RECONCILED — every capability claim is backed by a real gate."; exit 0; fi
echo "  DRIFT TOTAL: $drift row(s). Owner (agent) drives to 0: add a real gate, or set status=missing/unmeasured."
[ "$STRICT" -eq 1 ] && exit 1
echo "  (report-only; run with --strict once drift=0 to wire into the wall)"
exit 0
