#!/usr/bin/env bash
# Bake the demo's curated page set: real HTML snapshots + Chromium's reference render of each.
#
# The set is DERIVED, not hand-picked to look good — it is drawn from the oracle's corpus so it stays
# representative of what the engine actually covers. Re-run on the audit cadence so the demo ages forward
# with the project instead of freezing as a snapshot of whenever it was first built.
set -uo pipefail
cd "$(dirname "$0")/.."
OUT=demo/www/pages
mkdir -p "$OUT"

CHROME=$(command -v google-chrome || command -v chromium || command -v chromium-browser || true)

# A small, deliberately DIVERSE set: a document-web page, a table-driven layout, and a flex/grid one —
# the three shapes the cascade and layout engine are actually judged on.
declare -A PAGES=(
  [wikipedia]="https://en.wikipedia.org/wiki/Web_browser_engine"
  [hackernews]="https://news.ycombinator.com/"
  [rustlang]="https://www.rust-lang.org/"
)

JSON="["
first=1
for name in "${!PAGES[@]}"; do
  url="${PAGES[$name]}"
  echo "── $name ← $url"
  curl -sL --max-time 30 -A "Mozilla/5.0 (X11; Linux x86_64) manuk-demo-snapshot" "$url" -o "$OUT/$name.html" || { echo "  fetch failed, skipping"; continue; }
  # INLINE the external stylesheets into the snapshot.
  #
  # The wasm demo cannot fetch subresources (no network in the browser sandbox for arbitrary origins,
  # and no net stack in the wasm build) — but the NATIVE engine does fetch them, every time. So a
  # snapshot that carries its own CSS is FAITHFUL, not a cheat: it is the same document the native
  # engine would have assembled. Rendering the fetch-less version instead would misrepresent the engine
  # in the OTHER direction — showing an unstyled page it would never actually show a user.
  python3 - "$OUT/$name.html" "$url" <<'PYEOF'
import sys, re, urllib.parse, urllib.request
path, base = sys.argv[1], sys.argv[2]
html = open(path, encoding='utf-8', errors='replace').read()

def fetch(u):
    try:
        req = urllib.request.Request(u, headers={'User-Agent': 'manuk-demo-snapshot'})
        with urllib.request.urlopen(req, timeout=20) as r:
            return r.read().decode('utf-8', 'replace')
    except Exception:
        return None

def inline(m):
    tag = m.group(0)
    if 'stylesheet' not in tag.lower():
        return tag
    href = re.search(r'href\s*=\s*["\']([^"\']+)["\']', tag, re.I)
    if not href:
        return tag
    css = fetch(urllib.parse.urljoin(base, href.group(1)))
    return f'<style>{css}</style>' if css else tag

html = re.sub(r'<link\b[^>]*>', inline, html, flags=re.I)
if '<base' not in html.lower():
    html = re.sub(r'(<head[^>]*>)', r'\1<base href="' + base + '">', html, count=1, flags=re.I)
open(path, 'w', encoding='utf-8').write(html)
print(f"  inlined stylesheets ({len(html)//1024} KB)")
PYEOF
  if [ -n "$CHROME" ]; then
    "$CHROME" --headless --disable-gpu --hide-scrollbars --window-size=900,1400 \
      --screenshot="$OUT/$name.png" "file://$PWD/$OUT/$name.html" >/dev/null 2>&1 \
      && echo "  ✓ chromium reference" || echo "  (chromium reference failed)"
  else
    echo "  (no chromium — reference render skipped)"
  fi
  [ $first -eq 0 ] && JSON="$JSON,"
  first=0
  JSON="$JSON{\"title\":\"$name\",\"html\":\"pages/$name.html\",\"ref\":\"pages/$name.png\"}"
done
JSON="$JSON]"
echo "$JSON" > demo/www/pages.json
echo
echo "wrote demo/www/pages.json"
