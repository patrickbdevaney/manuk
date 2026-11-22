# RG.3.0.b.a.a.a.a.a.a.a.b.a.a.a.a.a.a.a.c.a.a.a.a — is the opendining-4 replay transcript still a valid frozen input under the aria-hidden contract? (decision, written BEFORE any change to the transcript)

chrome_ver=145.0.7632.116 (MANUK_CHROME_PIN). Contract version: **aria-hidden contract 7b556dab** (scroll contract `.b.b` in HEAD).

## Decision: KEEP the transcript. No re-recording under this node.

Frozen input pinned by sha256 (file `~/.cache/manuk-rgb2/transcripts/manuk_opendining-4_r1.jsonl`, recorded 2026-11-10, 211755 bytes):
`b42f5ff2ac6cb20c754d4554c4df3a663161963935acec4493356eea40da3f6d`

Pinned bar/scoring files at decision time (HEAD 8f9a5a61):
- `bench/agent-tasks/rg3baaaaaaa_spread.py` sha256 `9dabf65930eeefcfef35b9ced0e169fd3ddb42a48630e3189b23b0eb04c42767` (EXPECT=13, MIN_MANUK=9, MIN_CHROME=12)
- `bench/agent-tasks/rg3baaaaaaa_match.py` sha256 `a9abee0eea6b31fa9f8645275c63dff7acc4677fc464bb6359afb88c5904b94a`

## Why keep
- A frozen input is valid for the question it asks: "do the two backends resolve the same ordinal handles to the same role/state?". The
  re-measure (15107b73 rule, 8f9a5a61 runs) shows manuk 10/13 = chrome 10/13 on all three runs, strict equal: the backends agree
  step for step, including both failing on #13/#14. The agreement is the finding; the transcript still discriminates (the
  `--mutate shift-ordinal` run lowers it).
- #13/#14 failing on both is a property of the contract (the page withholds the picker comboboxes while its menu is open), i.e. a real
  agent-visible fact, not a transcript defect. Re-recording so the steps reference the open listbox would make the transcript a
  different task steered toward a pass, which SUCCESS.md forbids.
- The chrome >=12 bar stays RED (10<12). That is reported as is. It is not a reason to change the input.

## What a re-record would require (not done here)
A new, separately pre-registered node that (1) states before recording which steps change and why, (2) records against both
backends under the same contract and chrome_ver, (3) keeps the old transcript and its sha beside the new one, (4) never edits MIN_*/EXPECT
(a new transcript would get its own bars written before its numbers exist). This node does none of that.

## Gate (red conditions)
`bench/agent-tasks/rg3baaaaaaaaaaaaaaaacaaaa_pin.py`: exit 1 if the transcript sha256 differs from the pin above, or the spread/match script
sha256 differs, or the constants line `EXPECT, MIN_MANUK, MIN_CHROME = 13, 9, 12` is not present, or this doc lacks the pin / contract
version / "KEEP". Sensitivity shown with `--mutate transcript` (appends a byte in a temp copy) and `--mutate const` (edited temp copy).
