# Role + name is not an address

`g_agent_drive_loop` (t1455) closed the agent's perceive → ground → actuate → observe loop on a
hermetic fixture and named its own gap: *"what this does not prove is that a real page's markup is
reachable this way."* `drive-probe` is that measurement.

An agent drives by **role + accessible name**, aims at a **box**, and clicks a **coordinate**. Each
step fails differently, so a target counts as drivable only if it survives all three:

| verdict | what went wrong | why the agent is stuck |
|---|---|---|
| `Ungrounded` | no bbox, or a zero-area one | perceivable but not aimable |
| `Ambiguous` | another target shares its (role, name) | resolution picks one **by chance** |
| `MisHit` | the centre hit-tests to an unrelated node | the click lands somewhere else |

```text
                          targets  drivable  ungrounded  ambiguous  mis-hit    rate  +landmark
  martinfowler.com            225       143           2         80        0   63.6%      67.1%
  news.ycombinator.com        198       142           0         56        0   71.7%      71.7%
  blog.rust-lang.org          416       403           0         13        0   96.9%      96.9%
  www.a11yproject.com          67        41           0         23        3   61.2%      77.6%
  danluu.com                  208       208           0          0        0  100.0%     100.0%
  en.wikipedia.org/…         1229       885           0        344        0   72.0%      76.9%
  TOTAL                      2343      1822           2        516        3   77.8%      81.1%
```

## The failure is almost entirely ambiguity

**516 of 521 non-drivable targets — 99% — are ambiguous.** Grounding is essentially solved (2 of
2,343) and occlusion is rare (3). The addressing scheme is the bottleneck, not the geometry.

And the duplicates are not a defect in our tree. Listing them on a11yproject.com shows what they
are:

```text
  Ambiguous  link  "Posts"        Ambiguous  link  "GitHub"
  Ambiguous  link  "Spotlight"    Ambiguous  link  "Sitemap"
  Ambiguous  link  "About"        Ambiguous  link  "Back to top"
```

Every one appears **twice: once in the header nav and once in the footer.** Chrome's tree contains
the same twins. This is not a projection bug — it is an addressing scheme that cannot express what a
human says without thinking about it: *the `Posts` link **in the navigation***.

## The landmark is the missing term, and it is worth 3.3 points

Re-keying the address as `(landmark, role, name)` — the enclosing `banner` / `navigation` / `main` /
`contentinfo` — lifts the corpus from **77.8% to 81.1%**, and much more on the sites whose
duplication is header-vs-footer (a11yproject 61.2% → 77.6%, wikipedia 72.0% → 76.9%).

⚠ It is **not** the whole answer. news.ycombinator.com does not move at all — it has no landmarks —
and blog.rust-lang.org's remaining 13 are duplicates *within one landmark*. The next term after the
landmark has to distinguish siblings that share a container: an ordinal, or the nearest heading.
Measuring that before building it is the same discipline that produced this row.

## Two defects this probe found on its way

**`inset: 0` does not size an absolutely positioned box.** An `position:absolute; inset:0` overlay
lays out `0x0`, and its sibling link came out 96×22 instead of 200×30. Found while writing the
gate's overlay row, which would otherwise have passed for the wrong reason.

**A positioned element with `z-index: auto` does not win a click against the in-flow content it
covers.** `A11yNode::z` models only an *explicit* `z-index` — it is `0` for "not positioned" and `0`
for "positioned, auto" alike — so the tie-break between unrelated subtrees falls through to *smaller
area*, and a 200×30 link beats the 300×60 banner on top of it. **A cookie banner is exactly this
markup**, and an agent would click the link underneath one and report success. Asserted in the gate
rather than fixed: `z` is set where the computed styles live and the change reaches every coordinate
click in the engine.

See also [[the-agent-drive-loop]], [[recall-is-not-node-match]].
