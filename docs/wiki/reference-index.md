# Reference Index — a second oracle over shipping-engine source (Chromium / Firefox / Servo / WebKit)

There is a **gitignored reference/tooling layer** (`reference/`) that maps the untracked shipping-engine
source checked out in this repo (chromium/ firefox/ servo/ stylo/ WebKit/). It exists to accelerate the
**hard 20%** — the specific levers where reading how a shipping engine already solved something shortcuts
"discover correct behavior one WPT failure at a time." It is a **retrieval tool consulted only when a
cluster maps to a lever category below** — not a standing input to every tick, and never conflated with this
project's own tracked knowledge.

## TWO HARD RULES (non-negotiable — a violation is a defect, reject it)
1. **Reference-and-reimplement, NEVER copy.** Read to understand why/how, then write clean Rust against WPT.
   A translated paste is a defect: firefox/ is MPL 2.0 (file-level copyleft), and Blink's C++ (raw pointers,
   Oilpan GC, its threading) does not map 1:1 to memory-safe Rust anyway.
2. **Reference is a SECOND ORACLE, not the authority.** WPT is the source of truth. Cross-check against spec
   + WPT; **if they disagree, WPT wins.** Do not let engine quirks / V8-isms leak in.

## USE IT FOR (only these levers)
- **Step-function crashers** — recursion-depth limits / iterative parsing to avoid native C-stack overflow
  on deeply-nested HTML. *(Already found: Blink caps the HTML-parser DOM-tree depth at
  `kMaximumHTMLParserDOMTreeDepth = 512` — the fix for the html/semantics crasher is a Rust depth-guard, not
  the JS stack quota. See reference/SOURCEMAP.md.)*
- **Exhaustive enumerated tables** — `reference/index/idl-reflected-attributes.tsv` (600 rows:
  interface · idl_attribute · idl_type · content_attribute · extras) instead of rediscovering IDL reflection
  one failure at a time.
- **Layout edge-case algorithms** — flex/grid geometry, margin collapsing, shrink-to-fit ("why 110 not 120").

## DON'T use it for
The exact-pixel-reftest tail (Bar 2), obscure spec corners, encoding minutiae — reading source doesn't
shortcut pixel precision or long-tail volume.

## HOW
    reference/bin/ref-lookup.sh --lever crasher|reflection|layout "<query>"   # prioritizes Rust-lineage (servo/stylo), prints the two rules
    grep -iP '\t<attr>\t' reference/index/idl-reflected-attributes.tsv
Full governing doc: `reference/DIRECTIVE.md` · path map + findings: `reference/SOURCEMAP.md`.
Prefer **Servo/Stylo/Gecko** patterns over Blink where both exist (Rust-lineage ports cleanly). [[js-engine]] [[text-layout]]
