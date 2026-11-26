Scope: RESEARCH.md's Axis H (H1-H4) and the Axis D amendment only (Custom
Elements/Shadow DOM, SSR/History API items) — nothing else in RESEARCH.md.
Do not use the Task tool or spawn subagents — work sequentially,
single-agent throughout this entire directive.

PHASE 1 — Research, this scope only.
2-3 searches per item (H1, H2, H3, H4, plus the two Axis D additions),
primary sources preferred (spec text, WPT, Servo source, real framework
docs for SSR/routing mechanics). State a disposition per item. H2
specifically is an architecture-design question, not a lookup — if the
searches don't settle it, mark it "insufficient evidence within budget,
deferred" rather than forcing a confident-sounding answer; that's expected
and acceptable here, not a failure. Write findings into RESEARCH.md's Axis
H / Axis D sections in the existing compact format. Commit.

PHASE 2 — Addendum to IMPLEMENTATION.md.
From Phase 1's dispositions only (not the rest of RESEARCH.md), write new
ordered entries in IMPLEMENTATION.md's existing format: concrete change,
seam it hooks into, dependencies/sequencing against existing entries,
acceptance check (WPT target with a freshly-pinned commit/date where
relevant, or a specific behavior demonstrated). Anything deferred in Phase
1 gets a plan entry that starts with the follow-up research needed, not an
implementation step. Note platform status per item per the existing
Ubuntu-only convention (verified on Linux / engineered-for-portability-
unverified-elsewhere). This is planning only — do not implement yet.
Commit.

PHASE 3 — Execute only the addendum.
Implement only the new entries just added in Phase 2, in the order they
specify, one at a time. Verify each against its acceptance check before
moving to the next. Respect CLAUDE.md's modification boundary absolutely —
stop and flag rather than proceed if anything would touch SpiderMonkey's
JIT/GC/sandbox. Commit after each item with a message naming the item, its
acceptance result, and platform-verification status. Update CLAUDE.md and
README as capabilities land, per existing maintenance discipline. Do not
touch or re-execute anything from the prior IMPLEMENTATION.md scope.

If a genuine blocker appears in Phase 3, stop that item, note it clearly,
move to the next independent item rather than stalling. Report progress at
each phase boundary, not only at the very end.