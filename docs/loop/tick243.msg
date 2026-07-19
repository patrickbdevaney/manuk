feat(loop tick 242): quirks mode — wiring a verdict the engine had already computed and thrown away

Tick 241 probed this row and found a DEAD-END WIRE, which is a more dangerous shape than a
missing feature: html5ever detected quirks correctly and engine/html/src/sink.rs stored the
verdict in a field that was WRITTEN AND NEVER READ; every Stylo call site hard-coded
QuirksMode::NoQuirks; and document.compatMode returned a constant "CSS1Compat" behind a comment
asserting "our documents are never quirks-mode". The engine had the answer and discarded it — a
failure no capability probe surfaces, because the feature appears present at every layer you
inspect.

THE CLASS THIS UNLOCKS IS THE PRE-STANDARDS WEB, and it is not a nostalgia category: the intranet
app, the government form, the university department page, the vendor manual, the CMS template
nobody has touched since 2008. Documents with no doctype, which every real browser renders in
quirks mode and we rendered in standards — so every unitless width=/height= in their inline
styles was DROPPED as invalid and their layouts collapsed to auto widths.

THE DESIGN DECISION THAT MADE THIS A TICK RATHER THAN A REFACTOR: THE VERDICT RIDES ON `Dom`. The
obvious shape — return it from parse() and thread it as a parameter — would have touched
manuk_html::parse/parse_bytes/StreamParser, Page::from_dom, the Page struct, and ALL 18
cascade_styles call sites in engine/page. A `quirks: bool` field on Dom changes NO SIGNATURE
ANYWHERE, because every consumer already receives a Dom: cascade_via_stylo(dom: &Dom, ..) reads
it, StyloDocument already holds &'a Dom, and doc_get_compat_mode reaches it through the existing
this_node(vp). The general rule worth keeping: a value every consumer already has a handle to
should ride on that handle. The explicit-parameter version is the one that looks more honest and
is the one that makes the change too big to land.

STYLO ALREADY IMPLEMENTS THE QUIRKS. QuirksMode is an INPUT we were failing to supply — unitless
lengths, case-insensitive id/class matching, the <font size> mapping table are all already in
there. That is what makes this plumbing rather than layout math.

TWO PARSE PATHS, AND WIRING ONE WAS NOT ENOUGH — the bug the gate caught mid-tick. After wiring
StyloStylesheet::from_str (<style> and linked CSS) the gate still failed at width=800:
parse_style_attribute, which handles the inline style= attribute, is a SEPARATE parse with its
own QuirksMode argument. And legacy markup — precisely the markup that lands in quirks mode — is
overwhelmingly INLINE-STYLED, so wiring only stylesheets would have shipped a quirks mode that
did nothing on the documents it exists for.

REPORTING AND RENDERING LANDED TOGETHER, ON PURPOSE. Flipping compatMode alone is a WORSE lie
than the constant it replaces, because it is actionable by the page: a site branching on
compatMode would take a quirks code path this engine does not honour. The gate asserts both
directions of both halves, plus a fifth claim that the two modes actually DIFFER — each of the
first four can be satisfied by a constant; that one cannot.

THREE RED PROBES EXECUTED, NOT ASSERTED (process rule 3):
  · sink writes `false` unconditionally -> compatMode claim FAILS (the sink write is load-bearing)
  · compatMode left as the constant     -> FAILED for real, mid-tick, before that half was wired
  · inline style= left at NoQuirks      -> width claim FAILED at 800, for real, mid-tick

THE GATE'S OWN HARNESS WAS A BUG FIRST, and it is worth recording because it impersonates Bar 0.
Written as three #[test]s, the binary SIGSEGV'd: each test calls Page::load, cargo runs them on
separate threads, and SpiderMonkey is not shared-thread-safe. Before crashing it produced a
subtler artifact — compatMode read back as the fixture's placeholder "-" on one test and the real
value on another, i.e. a script that silently did not run. Diagnosed with --test-threads=1 (clean
failures, no crash) plus a scratch probe, then consolidated to ONE sequential test. A gate whose
fixture races itself cannot tell a regression from its own harness, and that SIGSEGV would have
been easy to read as an engine crash.

THE REGRESSION RISK WAS REAL AND WAS MEASURED, NOT ASSUMED. This changes behaviour for every
doctype-less fixture in the tree, and many gates use them. Full manuk-page suite re-run:
122 passed, 1 failed — the single failure is the PRE-EXISTING hard_wall_detection lib test
documented at tick 239, unrelated to quirks. Modified crates green: manuk-dom 11, manuk-html 12,
manuk-layout 77, manuk-paint 17, manuk-css(stylo) 33.

Gate: engine/page/tests/g_quirks_mode.rs. Two fixtures differing ONLY by <!doctype html>, because
anything else varying would let a difference in the CONTENT impersonate a difference in the MODE.
Anchored on WPT quirks/unitless-length ({input:"1", q:"1px"}).

TICK SHAPE: thread the parser's quirks verdict into Stylo and compatMode, together.

RESIDUE, NAMED RATHER THAN IMPLIED: the ~9 MatchingContext/media-query NoQuirks sites in
stylo_engine.rs still say standards, so CASE-INSENSITIVE ID/CLASS MATCHING IS NOT YET ENABLED — a
real quirk, deliberately left for a follow-up rather than quietly claimed. LimitedQuirks folds to
false (it does not enable the unitless-length quirk). g_quirks_mode is NOT in the verify wall
(tick 239; docs/loop/GATE-COVERAGE.md).

WIKI: docs/wiki/css-cascade.md (+ INDEX.md)
