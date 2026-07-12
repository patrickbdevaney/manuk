#!/usr/bin/env bash
# Bank a demonstrable build.
#
# A milestone that exists only as a green gate is not a milestone anyone can *use*. This takes the
# current commit, builds the SHIPPING configuration (gui + Stylo + SpiderMonkey — the same binary a
# person would run, per ADR-011), and banks it outside the repo with the notes that say what it can
# now do that the last one could not.
#
# Each banked build is:
#   ~/manuk-builds/manuk-<slug>-<yyyy-mm-dd>-<shortsha>      the binary
#   ~/manuk-builds/manuk-<slug>-<yyyy-mm-dd>-<shortsha>.md   what it can do, and what it still can't
#   ~/manuk-builds/manuk-latest                              a symlink to the newest
#
# and the commit is tagged, so the binary and the source are welded together rather than merely
# adjacent.
#
#   scripts/bank-release.sh <slug> "<one-line headline>"
#
# The slug becomes the tag and the filename: keep it short and about the CAPABILITY, not the tick
# number. `usable-web` beats `tick-34`.
set -euo pipefail
cd "$(dirname "$0")/.."

SLUG="${1:-}"
HEADLINE="${2:-}"
if [ -z "$SLUG" ] || [ -z "$HEADLINE" ]; then
  echo "usage: scripts/bank-release.sh <slug> \"<one-line headline>\"" >&2
  exit 2
fi

DATE="$(date +%Y-%m-%d)"
SHA="$(git rev-parse --short HEAD)"
BANK="$HOME/manuk-builds"
NAME="manuk-${SLUG}-${DATE}-${SHA}"
mkdir -p "$BANK"

echo "▶ building the SHIPPING configuration (gui + stylo + spidermonkey) …"
cargo build --release -p manuk-shell

BIN="target/release/manuk"
[ -x "$BIN" ] || BIN="target/release/manuk-shell"
[ -x "$BIN" ] || { echo "no release binary found in target/release" >&2; exit 1; }

cp "$BIN" "$BANK/$NAME"
chmod +x "$BANK/$NAME"
ln -sfn "$BANK/$NAME" "$BANK/manuk-latest"

SIZE="$(du -h "$BANK/$NAME" | cut -f1)"

# The notes are generated from the range since the last banked tag, so they cannot drift from what
# actually shipped.
PREV_TAG="$(git tag --list 'bank-*' --sort=-creatordate | head -1 || true)"
RANGE="${PREV_TAG:+$PREV_TAG..}HEAD"

{
  echo "# manuk — $HEADLINE"
  echo
  echo "**Built** $DATE from \`$SHA\` · shipping config (GUI + Stylo + SpiderMonkey) · $SIZE"
  echo
  echo '```'
  echo "$BANK/$NAME  https://en.wikipedia.org/wiki/Rust_(programming_language)"
  echo '```'
  echo
  echo "## What landed in this build"
  echo
  git log --no-merges --format='- %s' "$RANGE" | grep -vE '^- (loop|docs|chore|test):' || true
  echo
  echo "## Verify it yourself"
  echo
  echo '```'
  echo "scripts/verify.sh --fast     # build · box parity · G1 render · G2 JS · G3 affordances · unit"
  echo '```'
} > "$BANK/$NAME.md"

git tag -f "bank-${SLUG}-${DATE}" -m "$HEADLINE" >/dev/null

echo
echo "  banked → $BANK/$NAME  ($SIZE)"
echo "  notes  → $BANK/$NAME.md"
echo "  latest → $BANK/manuk-latest"
echo "  tagged → bank-${SLUG}-${DATE}"
echo
echo "  run it:  $BANK/manuk-latest https://news.ycombinator.com"
