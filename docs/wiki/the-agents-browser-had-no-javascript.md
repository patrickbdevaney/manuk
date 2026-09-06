# The agent's browser had no JavaScript

Two lines, in two files, neither of which reads as a defect:

```text
  agent/Cargo.toml   manuk-page.workspace = true   — `spidermonkey` is opt-in and was not taken
  agent/src/lib.rs   Page::load(...)               — the synchronous constructor: parses, lays out,
                                                     stops. No subresources, no lifecycle.
```

A `Cargo.toml` line that omits a feature looks like a smaller build. `Page::load` looks like loading
a page. Together they are **a browser that never finishes loading**, and it is the browser
`a11y-score`, `drive-probe`, `a11y-dump`, `agent-run` and ~30 `agent/tests/` gates all ran on.

## How it was found

Not by reading the manifest. `a11y-score` put Wikipedia at **25.9% precision**, and constitution
check #140 asked what the ~1,900 excess nodes *are* rather than assuming they were wrappers. One
multiset difference answered it:

```text
  OURS IN EXCESS                     CHROME IN EXCESS
    681  listitem  ""                   4  button  "[show]"
     90  list      ""                   1  columnheader "[show] v · t · e Timeline of web browsers"
     71  row       ""                  42  link    "Jump up"
     42  link      "↑"
```

**The two columns are the same fact from both sides.** Chrome has `[show]` buttons that MediaWiki's
`jquery.makeCollapsible` *creates*; we hold exactly the content those buttons hide. We had not run
the script. An a11y "precision defect" was a **JavaScript-execution gap**, and the area it was filed
under could not have said so.

## What it was worth

```text
                     precision   recall     F1        drivable  +landmark
  before                 63.5%    96.4%   76.6%          77.7%      81.1%
  after                  93.2%    96.4%   94.8%          80.7%      82.4%
```

**Track B's `>=90% node match` bar is met for the first time** — 94.8% F1. Per site, precision:
wikipedia 25.9 → 77.5, a11yproject 84.4 → 97.5, martinfowler 68.2 → 81.7, rust-lang 99.6 → 99.9.
Drive-probe's `ungrounded` count went 2 → 0.

⚠ Wikipedia's *drive rate* **fell**, 72.0% → 61.9%, and that is correct: its target count dropped
1229 → 501 as the phantoms vanished, so the surviving duplicates are a larger share of a smaller,
truer denominator. A rate can worsen because its denominator got honest.

## `stylo` was part of this change and was refused

Enabling it alongside `spidermonkey` turned `g_ax_tree_excludes_display_none` red: under Stylo's UA
sheet a collapsed `<select>`'s `<option>`s are hidden, and Chrome exposes both. That gate's `Option`
row exists for exactly this and says so in its own comment — *"if the UA sheet hid them the way it
hides a closed `<dialog>`, this tick would have deleted every dropdown from the agent's
perception."* The ratchet refuses a capability bought with a regression, so **only the JavaScript
half landed**, and the cascade half now has a named blocker instead of a preference.

## Two fixtures, because one could not tell the halves apart

The first mutation pass came back **green** when `load_async` was reverted to `load`: the
synchronous constructor runs an *inline* `<script>` perfectly well once SpiderMonkey is compiled in.
What it never does is **fetch** one. The discriminating fixture loads its script from a separate
file — the shape every real site uses — and without it the tick would have shipped an unproven
change.

⚠ And they must live in **separate test binaries**: two SpiderMonkey contexts in one binary abort on
drop with *"There are outstanding JS engine handles"*, which the harness reports as a SIGSEGV.

See also [[a-crate-that-omits-a-feature-substitutes-an-engine]], [[recall-is-not-node-match]],
[[role-plus-name-is-not-an-address]].
