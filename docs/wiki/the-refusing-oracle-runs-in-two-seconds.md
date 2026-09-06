# The refusing oracle runs in two seconds

t1468's paint-order change was Chrome-exact on eight hand-built rows, fixed three WPT subtests, and
was refused by a single line at the end of a ~50-minute wall:

```text
  ✗ clickability 83.2% — 62 links the browser cannot find
```

The whole tick was built, gated, documented and reverted because the only instrument that could see
the regression ran once, at the end, after everything else. That is not a property of the check — it
is a property of *where I was running it*.

```sh
curl -sL https://en.wikipedia.org/wiki/Terrier -o /tmp/manuk-g6.html
./target/release/manuk-wpt hittest --html /tmp/manuk-g6.html --url https://en.wikipedia.org/wiki/Terrier
#   links on page:      477
#   MISSED (unclickable): 0
#   CLICKABILITY: 100.0%
```

**Two seconds.** It is `verify.sh`'s G6 section, and G6 is one `manuk-wpt` subcommand against one
cached page. Nothing about it required the wall.

⭐ **The lesson generalises past this one check.** A wall gate that refuses a change is, almost
always, a command — and reading the harness to find that command is cheaper than the tick it saves.
The scope rule forbids *editing* `scripts/`; it has never forbidden *reading* it, and this is the
second time this session that reading it paid (the first was finding that `manuk-page` is absent
from the crate-suite loop).

## What it unblocks

The step-8 peer case is still open and its cost is known: **−62 clickable links** if closed by
tie-break alone, because the equal-layer area term is a proxy for the whole of Appendix E steps 4–7,
not for depth. Attempting it again needs an in-flow painting model — and now needs a two-second check
between each attempt instead of a fifty-minute one.

## The other half of this tick: five hypotheses ruled out

`martinfowler.com` is the corpus's second-worst site at 81.7% precision, and its excess is
concentrated — **32 unnamed `paragraph`** plus a duplicated navigation region (2 extra `navigation`,
its section headings and its links). Probing found the shape:

```text
  nav0[top-menu]      d=block  870x24     ← the one Chrome exposes
  nav1[top-navmenu]   d=none   0x0        ← correctly hidden
  nav2[navmenu]       d=block  0x0        ← inside nav1, and in OUR tree
  nav3[bottom-navmenu] d=none  0x0
  nav4[navmenu]       d=block  0x0        ← inside nav3, and in OUR tree
```

Five candidate mechanisms, each tested as a fixture against Chrome through `a11y-score --diff`, and
**every one came back 100% Chrome-exact**:

| hypothesis | result |
|---|---|
| a `display:none` subtree leaks into the tree | 100% — correctly excluded |
| a subtree hidden by a **media query** leaks | 100% — correctly excluded |
| XHTML-style `<button …/>` re-nests the document | identical to Chrome (`betaParent=BUTTON`) |
| Chrome drops empty `<p>` and we keep them | Chrome keeps them too |
| zero-area nodes are excluded by Chrome only | not yet separable from the above |

⚠ **Five ruled-out hypotheses are worth writing down.** The next tick on this site starts from a
narrowed field rather than repeating them, and the negative results are themselves five small
confirmations that general rules the tree depends on are correct.

See also [[nineteen-of-five-hundred-and-sixty-seven]], [[the-area-tie-break-was-a-proxy]].
